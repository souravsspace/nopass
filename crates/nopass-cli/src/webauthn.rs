//! Passkeys, from a terminal.
//!
//! A site's passkey ceremony has three parties: the site, the browser, and
//! an authenticator. These commands are the authenticator — they make the
//! keypair, and they answer challenges — and they print exactly the JSON a
//! browser would hand back, so what comes out of `register` and `assert` can
//! be posted to a site as it stands.
//!
//! The keypair lives in the store like every other secret: one entry,
//! encrypted to the same recipients, synced by the same git. What sets it
//! apart is that the site keeps the public half, so losing the store means
//! losing the account rather than merely a password.
//!
//! `verify` is the site's half of the work, run locally. It is not needed to
//! sign in to anything; it exists so a signature can be watched to hold.

use std::io::Read;

use anyhow::{bail, Context, Result};
use nopass_core::fields;
use nopass_core::passkey::{self, Ceremony, Passkey};
use nopass_core::record::{Kind, Record};
use nopass_core::Store;

/// The challenge used when a site did not give one.
///
/// Registration challenges exist to bind a ceremony to a request; a passkey
/// made from a terminal has no request to bind to, and a placeholder that
/// says so is more honest than sixteen random bytes pretending otherwise.
const NO_CHALLENGE: &[u8] = b"nopass-local-ceremony";

/// Create a passkey and print what a site would be told about it.
pub fn register(
    store: &Store,
    name: &str,
    rp: &str,
    user: Option<&str>,
    origin: Option<&str>,
    challenge: Option<&str>,
) -> Result<()> {
    store.authenticate()?;
    if store.entry_exists(name) {
        bail!("Error: {name} is already in the store.");
    }

    let key = Passkey::generate();
    let ceremony = ceremony_for(rp, origin, challenge)?;
    let created = key.register(&ceremony)?;

    let mut record = Record::new(Kind::Passkey);
    record.set_primary(&key.to_secret()?)?;
    record.set("rp", rp)?;
    if let Some(user) = user {
        record.set("user", user)?;
    }
    record.set("credential-id", &passkey::b64(&created.credential_id))?;
    record.set("alg", &passkey::ES256.to_string())?;
    record.set("counter", "0")?;
    store.insert(name, record.render().as_bytes())?;

    print!(
        "{}",
        json(&[
            ("type", Json::Text("public-key")),
            ("id", Json::Owned(passkey::b64(&created.credential_id))),
            ("rawId", Json::Owned(passkey::b64(&created.credential_id))),
            (
                "response",
                Json::Object(vec![
                    (
                        "attestationObject",
                        Json::Owned(passkey::b64(&created.attestation_object)),
                    ),
                    (
                        "clientDataJSON",
                        Json::Owned(passkey::b64(&created.client_data_json)),
                    ),
                ]),
            ),
        ])
    );
    Ok(())
}

/// Every passkey in the store: where it signs in, and how often it has.
pub fn list(store: &Store) -> Result<()> {
    for name in store.list("")? {
        let Ok(record) = read(store, &name) else {
            continue;
        };
        if record.kind() != Kind::Passkey {
            continue;
        }

        let rp = record.get(fields::PASSKEY_RP_KEYS).unwrap_or("—");
        let user = record.get(fields::PASSKEY_USER_KEYS).unwrap_or("—");
        let counter = record.get(fields::PASSKEY_COUNTER_KEYS).unwrap_or("0");
        println!("{name}\t{rp}\t{user}\tused {counter}");
    }
    Ok(())
}

/// Sign a challenge, and move the counter on.
pub fn assert(
    store: &Store,
    name: &str,
    challenge: Option<&str>,
    origin: Option<&str>,
) -> Result<()> {
    let mut record = read(store, name)?;
    if record.kind() != Kind::Passkey {
        bail!("Error: {name} is not a passkey.");
    }

    let rp = record
        .get(fields::PASSKEY_RP_KEYS)
        .context("this passkey does not say which site it is for")?
        .to_string();
    let credential = record
        .get(fields::PASSKEY_CREDENTIAL_KEYS)
        .unwrap_or_default()
        .to_string();
    let counter: u32 = record
        .get(fields::PASSKEY_COUNTER_KEYS)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    let key = Passkey::from_secret(&record.primary)?;
    let signed = key.assert(&ceremony_for(&rp, origin, challenge)?, counter)?;

    // Written back before it is printed: a counter that went out but was not
    // recorded is exactly the state a site reads as a cloned credential.
    record.set("counter", &signed.counter.to_string())?;
    store.insert(name, record.render().as_bytes())?;

    print!(
        "{}",
        json(&[
            ("type", Json::Text("public-key")),
            ("id", Json::Owned(credential.clone())),
            ("rawId", Json::Owned(credential)),
            (
                "response",
                Json::Object(vec![
                    (
                        "authenticatorData",
                        Json::Owned(passkey::b64(&signed.authenticator_data)),
                    ),
                    (
                        "clientDataJSON",
                        Json::Owned(passkey::b64(&signed.client_data_json)),
                    ),
                    ("signature", Json::Owned(passkey::b64(&signed.signature))),
                ]),
            ),
        ])
    );
    Ok(())
}

/// Check an assertion against the passkey that made it, the way a site would.
pub fn verify(store: &Store, name: &str) -> Result<()> {
    let record = read(store, name)?;
    if record.kind() != Kind::Passkey {
        bail!("Error: {name} is not a passkey.");
    }

    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let authenticator_data = passkey::unb64(&field_of(&input, "authenticatorData")?)?;
    let client_data_json = passkey::unb64(&field_of(&input, "clientDataJSON")?)?;
    let signature = passkey::unb64(&field_of(&input, "signature")?)?;

    let key = Passkey::from_secret(&record.primary)?;
    if passkey::verify(
        &key.verifying_key(),
        &authenticator_data,
        &client_data_json,
        &signature,
    ) {
        let counter = passkey::counter_of(&authenticator_data).unwrap_or(0);
        println!("The signature is valid. Signed for {name}, counter {counter}.");
        Ok(())
    } else {
        bail!("Error: that signature was not made by {name}.")
    }
}

fn read(store: &Store, name: &str) -> Result<Record> {
    let body = store.show(name)?;
    Ok(Record::parse(&String::from_utf8_lossy(&body)))
}

fn ceremony_for(rp: &str, origin: Option<&str>, challenge: Option<&str>) -> Result<Ceremony> {
    Ok(Ceremony {
        challenge: match challenge {
            Some(text) => passkey::unb64(text)?,
            None => NO_CHALLENGE.to_vec(),
        },
        origin: origin.map_or_else(|| format!("https://{rp}"), str::to_string),
        rp_id: rp.to_string(),
        // The passphrase, or the security key, that opened the store is the
        // verification. Saying so is the difference between a site asking for
        // a second factor and not.
        user_verified: true,
    })
}

/*
 * Just enough JSON to print and to read back.
 *
 * Printing: what these commands emit is copied into a browser console or
 * curl, so it has to be a person-readable version of a fixed shape — two
 * levels, string values only. Reading: `verify` takes back what `assert`
 * printed, and pulling three known keys out of it needs less than a parser.
 */
enum Json {
    Text(&'static str),
    Owned(String),
    Object(Vec<(&'static str, Json)>),
}

fn json(entries: &[(&str, Json)]) -> String {
    format!("{}\n", write_object(entries, 0))
}

fn write_object(entries: &[(&str, Json)], depth: usize) -> String {
    let pad = "  ".repeat(depth + 1);
    let close = "  ".repeat(depth);
    let body: Vec<String> = entries
        .iter()
        .map(|(key, value)| match value {
            Json::Text(text) => format!("{pad}\"{key}\": \"{text}\""),
            Json::Owned(text) => format!("{pad}\"{key}\": \"{text}\""),
            Json::Object(inner) => {
                format!("{pad}\"{key}\": {}", write_object(inner, depth + 1))
            }
        })
        .collect();
    format!("{{\n{}\n{close}}}", body.join(",\n"))
}

/// One `"key": "value"` out of what `assert` printed.
fn field_of(input: &str, key: &str) -> Result<String> {
    let marker = format!("\"{key}\"");
    let at = input
        .find(&marker)
        .with_context(|| format!("the assertion has no {key}"))?;
    let rest = &input[at + marker.len()..];
    let open = rest
        .find('"')
        .with_context(|| format!("{key} has no value"))?;
    let value = &rest[open + 1..];
    let end = value
        .find('"')
        .with_context(|| format!("{key} is not closed"))?;
    Ok(value[..end].to_string())
}
