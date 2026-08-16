//! nopass as a WebAuthn authenticator.
//!
//! A passkey is a keypair a site knows the public half of. Registering one
//! makes the pair; signing in proves possession by signing what the site
//! asked, over data that names the site — so a signature made for one origin
//! is worthless at another, which is the property that makes passkeys
//! phishing-resistant.
//!
//! Everything here is the authenticator's half of
//! [WebAuthn level 3](https://www.w3.org/TR/webauthn-3/), built to the byte
//! layouts that specification fixes:
//!
//! - **authenticator data** — `rpIdHash(32) ‖ flags(1) ‖ signCount(4, big
//!   endian)`, followed at registration by the attested credential data.
//! - **attested credential data** — `aaguid(16) ‖ credentialIdLength(2, big
//!   endian) ‖ credentialId ‖ COSE public key`.
//! - **COSE key** — a CBOR map: `1: 2` (EC2), `3: -7` (ES256), `-1: 1`
//!   (P-256), `-2: x`, `-3: y`.
//! - **attestation object** — a CBOR map of `fmt`, `attStmt`, `authData`.
//! - **assertion signature** — ECDSA over
//!   `authenticatorData ‖ SHA-256(clientDataJSON)`, DER encoded.
//!
//! Attestation is `none`. Attestation exists to tell a site what hardware it
//! is talking to, and the honest answer for a software authenticator holding
//! its key in an age-encrypted file is "nothing you should trust differently".
//!
//! The private key lives in the store like any other secret: encrypted to the
//! same recipients, synced by the same git, readable only after the same
//! unlock.

use base64::Engine;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::elliptic_curve::rand_core::OsRng;
use p256::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// COSE algorithm identifier for ECDSA over P-256 with SHA-256.
pub const ES256: i32 = -7;

/// Length of a credential id, in bytes. Sixteen is what most authenticators
/// emit and is comfortably beyond guessing.
const CREDENTIAL_ID_LEN: usize = 16;

/// The AAGUID: sixteen zero bytes.
///
/// A real authenticator identifies its make and model here. A software one
/// claiming a model would be claiming properties it does not have, and the
/// specification reserves all-zero for exactly this.
const AAGUID: [u8; 16] = [0; 16];

/// Flag bits of authenticator data.
const FLAG_USER_PRESENT: u8 = 0x01;
const FLAG_USER_VERIFIED: u8 = 0x04;
const FLAG_ATTESTED: u8 = 0x40;

/// A passkey, as it is kept and as it is used.
pub struct Passkey {
    key: SigningKey,
}

/// What a site is told when a passkey is created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    /// The credential id, which the site stores and later asks for by name.
    pub credential_id: Vec<u8>,
    /// CBOR: `fmt`, `attStmt`, `authData`.
    pub attestation_object: Vec<u8>,
    pub client_data_json: Vec<u8>,
}

/// What a site is told when a passkey signs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assertion {
    pub authenticator_data: Vec<u8>,
    pub client_data_json: Vec<u8>,
    /// DER-encoded ECDSA signature.
    pub signature: Vec<u8>,
    /// The counter as it now stands, to be written back to the entry.
    pub counter: u32,
}

/// Base64url, unpadded — the encoding WebAuthn uses everywhere.
pub fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn unb64(text: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(text.trim())
        .map_err(|error| Error::Encrypt(format!("not base64url: {error}")))
}

impl Passkey {
    /// A new keypair, from the OS CSPRNG.
    pub fn generate() -> Passkey {
        Passkey {
            key: SigningKey::random(&mut OsRng),
        }
    }

    /// The private key as it is stored: PKCS#8 DER, base64url.
    pub fn to_secret(&self) -> Result<String> {
        let der = self
            .key
            .to_pkcs8_der()
            .map_err(|error| Error::Encrypt(format!("cannot encode the key: {error}")))?;
        Ok(b64(der.as_bytes()))
    }

    pub fn from_secret(secret: &str) -> Result<Passkey> {
        let der = unb64(secret)?;
        let key = SigningKey::from_pkcs8_der(&der)
            .map_err(|error| Error::Encrypt(format!("cannot read the key: {error}")))?;
        Ok(Passkey { key })
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        *self.key.verifying_key()
    }

    /// The public key as a COSE_Key map, which is what goes to the site.
    pub fn cose_public_key(&self) -> Result<Vec<u8>> {
        let point = self.verifying_key().to_encoded_point(false);
        let x = point
            .x()
            .ok_or_else(|| Error::Encrypt("the public key has no x".into()))?;
        let y = point
            .y()
            .ok_or_else(|| Error::Encrypt("the public key has no y".into()))?;

        // Written by hand rather than through a struct: CBOR maps with
        // negative integer keys are not something serde will express, and the
        // key order here is the canonical one sites expect.
        let map = [
            (1, Value::Int(2)),
            (3, Value::Int(i64::from(ES256))),
            (-1, Value::Int(1)),
            (-2, Value::Bytes(x.to_vec())),
            (-3, Value::Bytes(y.to_vec())),
        ];
        Ok(cbor_map(&map))
    }

    /// Create a credential for `request`, as `navigator.credentials.create`
    /// would.
    pub fn register(&self, request: &Ceremony) -> Result<Registration> {
        let credential_id = random_bytes(CREDENTIAL_ID_LEN);
        let client_data_json = client_data("webauthn.create", request);

        let mut attested = Vec::new();
        attested.extend_from_slice(&AAGUID);
        attested.extend_from_slice(&(credential_id.len() as u16).to_be_bytes());
        attested.extend_from_slice(&credential_id);
        attested.extend_from_slice(&self.cose_public_key()?);

        let flags = FLAG_USER_PRESENT | FLAG_ATTESTED | verified(request);
        let auth_data = authenticator_data(&request.rp_id, flags, 0, Some(&attested));

        let attestation_object = cbor_attestation(&auth_data);
        Ok(Registration {
            attestation_object,
            client_data_json,
            credential_id,
        })
    }

    /// Sign a challenge, as `navigator.credentials.get` would.
    ///
    /// The counter goes up by one and is returned so the caller can write it
    /// back: a site that sees it go backwards knows the credential has been
    /// cloned, which is the whole reason the field exists.
    pub fn assert(&self, request: &Ceremony, counter: u32) -> Result<Assertion> {
        let next = counter.saturating_add(1);
        let client_data_json = client_data("webauthn.get", request);
        let flags = FLAG_USER_PRESENT | verified(request);
        let authenticator_data = authenticator_data(&request.rp_id, flags, next, None);

        let mut signed = authenticator_data.clone();
        signed.extend_from_slice(&Sha256::digest(&client_data_json));
        let signature: Signature = self.key.sign(&signed);

        Ok(Assertion {
            authenticator_data,
            client_data_json,
            counter: next,
            signature: signature.to_der().as_bytes().to_vec(),
        })
    }
}

/// What a site asked for, in either ceremony.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ceremony {
    /// The relying party id — a domain, never a URL.
    pub rp_id: String,
    /// The origin the page is on, which the site checks against its own.
    pub origin: String,
    /// The site's challenge, exactly as it sent it.
    pub challenge: Vec<u8>,
    /// Whether the user was verified — a PIN, a biometric, a passphrase.
    pub user_verified: bool,
}

fn verified(request: &Ceremony) -> u8 {
    if request.user_verified {
        FLAG_USER_VERIFIED
    } else {
        0
    }
}

/// `clientDataJSON`, built the way a browser builds it.
///
/// Field order matters: a site verifies the signature over these exact bytes,
/// so this is written rather than serialised from a map whose order could
/// drift.
fn client_data(ceremony: &str, request: &Ceremony) -> Vec<u8> {
    format!(
        r#"{{"type":"{}","challenge":"{}","origin":"{}","crossOrigin":false}}"#,
        ceremony,
        b64(&request.challenge),
        request.origin
    )
    .into_bytes()
}

/// `rpIdHash ‖ flags ‖ signCount ‖ attestedCredentialData?`
fn authenticator_data(rp_id: &str, flags: u8, counter: u32, attested: Option<&[u8]>) -> Vec<u8> {
    let mut data = Vec::with_capacity(37);
    data.extend_from_slice(&Sha256::digest(rp_id.as_bytes()));
    data.push(flags);
    data.extend_from_slice(&counter.to_be_bytes());
    if let Some(attested) = attested {
        data.extend_from_slice(attested);
    }
    data
}

/// Verify an assertion the way a relying party would.
///
/// Not needed to sign in — the site does this — but `nopass webauthn verify`
/// runs it so a user can watch a signature hold, and the tests use it as the
/// relying party they otherwise would not have.
pub fn verify(
    public_key: &VerifyingKey,
    authenticator_data: &[u8],
    client_data_json: &[u8],
    signature: &[u8],
) -> bool {
    use p256::ecdsa::signature::Verifier;

    let Ok(signature) = Signature::from_der(signature) else {
        return false;
    };
    let mut signed = authenticator_data.to_vec();
    signed.extend_from_slice(&Sha256::digest(client_data_json));
    public_key.verify(&signed, &signature).is_ok()
}

/// The rp id hash out of authenticator data, for a caller checking a site.
pub fn rp_id_hash(authenticator_data: &[u8]) -> Option<&[u8]> {
    authenticator_data.get(..32)
}

/// The signature counter out of authenticator data.
pub fn counter_of(authenticator_data: &[u8]) -> Option<u32> {
    let bytes: [u8; 4] = authenticator_data.get(33..37)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
}

fn random_bytes(length: usize) -> Vec<u8> {
    use rand::Rng;
    let mut rng = rand::rng();
    (0..length).map(|_| rng.random()).collect()
}

/// The little of CBOR this needs, written out rather than pulled in.
enum Value {
    Int(i64),
    Bytes(Vec<u8>),
}

fn cbor_head(major: u8, argument: u64, out: &mut Vec<u8>) {
    let major = major << 5;
    match argument {
        0..=23 => out.push(major | argument as u8),
        24..=0xff => {
            out.push(major | 24);
            out.push(argument as u8);
        }
        0x100..=0xffff => {
            out.push(major | 25);
            out.extend_from_slice(&(argument as u16).to_be_bytes());
        }
        _ => {
            out.push(major | 26);
            out.extend_from_slice(&(argument as u32).to_be_bytes());
        }
    }
}

fn cbor_int(value: i64, out: &mut Vec<u8>) {
    if value < 0 {
        cbor_head(1, (-1 - value) as u64, out);
    } else {
        cbor_head(0, value as u64, out);
    }
}

fn cbor_map(entries: &[(i64, Value)]) -> Vec<u8> {
    let mut out = Vec::new();
    cbor_head(5, entries.len() as u64, &mut out);
    for (key, value) in entries {
        cbor_int(*key, &mut out);
        match value {
            Value::Int(number) => cbor_int(*number, &mut out),
            Value::Bytes(bytes) => {
                cbor_head(2, bytes.len() as u64, &mut out);
                out.extend_from_slice(bytes);
            }
        }
    }
    out
}

/// `{ "fmt": "none", "attStmt": {}, "authData": <bytes> }`, in CBOR.
fn cbor_attestation(auth_data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    cbor_head(5, 3, &mut out);

    cbor_text("fmt", &mut out);
    cbor_text("none", &mut out);

    cbor_text("attStmt", &mut out);
    cbor_head(5, 0, &mut out);

    cbor_text("authData", &mut out);
    cbor_head(2, auth_data.len() as u64, &mut out);
    out.extend_from_slice(auth_data);
    out
}

fn cbor_text(text: &str, out: &mut Vec<u8>) {
    cbor_head(3, text.len() as u64, out);
    out.extend_from_slice(text.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ceremony() -> Ceremony {
        Ceremony {
            challenge: b"a-challenge-from-the-site".to_vec(),
            origin: "https://example.com".to_string(),
            rp_id: "example.com".to_string(),
            user_verified: true,
        }
    }

    #[test]
    fn a_key_survives_being_written_down_and_read_back() {
        let key = Passkey::generate();
        let stored = key.to_secret().expect("the key encodes");
        let same = Passkey::from_secret(&stored).expect("the key decodes");

        assert_eq!(same.verifying_key(), key.verifying_key());
    }

    #[test]
    fn a_stored_key_is_one_line_of_base64url() {
        let stored = Passkey::generate().to_secret().expect("the key encodes");

        assert!(!stored.contains('\n'), "an entry field is one line");
        assert!(
            stored
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "{stored}"
        );
    }

    #[test]
    fn the_public_key_is_a_cose_es256_key() {
        let cose = Passkey::generate()
            .cose_public_key()
            .expect("the key encodes");

        // A five-entry map, then `1: 2` — kty EC2 — as its first pair.
        assert_eq!(cose[0], 0xa5, "a CBOR map of five");
        assert_eq!(&cose[1..3], &[0x01, 0x02], "kty is EC2");
        // `3: -7`, the algorithm, written as CBOR negative six.
        assert_eq!(&cose[3..5], &[0x03, 0x26], "alg is ES256");
        // 1 map head, three small pairs at 2 bytes each, and two 32-byte
        // coordinates each with a key byte and a two-byte string head: 77.
        assert_eq!(cose.len(), 1 + 2 + 2 + 2 + 35 + 35);
    }

    #[test]
    fn registration_names_the_site_it_was_made_for() {
        let request = ceremony();
        let created = Passkey::generate()
            .register(&request)
            .expect("a credential is created");

        // The attestation object is a map of three, and the last of them is
        // the authenticator data, which begins with the hash of the rp id.
        let auth_data = auth_data_of(&created.attestation_object);
        assert_eq!(
            rp_id_hash(auth_data).expect("a hash"),
            Sha256::digest(request.rp_id.as_bytes()).as_slice()
        );
    }

    #[test]
    fn registration_says_the_user_was_present_and_a_key_is_attached() {
        let created = Passkey::generate()
            .register(&ceremony())
            .expect("a credential is created");
        let flags = auth_data_of(&created.attestation_object)[32];

        assert_eq!(flags & FLAG_USER_PRESENT, FLAG_USER_PRESENT);
        assert_eq!(flags & FLAG_USER_VERIFIED, FLAG_USER_VERIFIED);
        assert_eq!(flags & FLAG_ATTESTED, FLAG_ATTESTED, "no credential data");
    }

    #[test]
    fn a_ceremony_without_verification_says_so() {
        let mut request = ceremony();
        request.user_verified = false;
        let created = Passkey::generate()
            .register(&request)
            .expect("a credential is created");

        let flags = auth_data_of(&created.attestation_object)[32];
        assert_eq!(flags & FLAG_USER_VERIFIED, 0);
    }

    #[test]
    fn registration_carries_the_credential_id_it_returned() {
        let created = Passkey::generate()
            .register(&ceremony())
            .expect("a credential is created");
        let auth_data = auth_data_of(&created.attestation_object);

        // aaguid(16) then a two-byte length then the id itself.
        let length = u16::from_be_bytes([auth_data[37 + 16], auth_data[37 + 17]]);
        assert_eq!(usize::from(length), created.credential_id.len());
        let at = 37 + 18;
        assert_eq!(
            &auth_data[at..at + created.credential_id.len()],
            &created.credential_id[..]
        );
    }

    #[test]
    fn client_data_is_what_the_site_will_hash() {
        let created = Passkey::generate()
            .register(&ceremony())
            .expect("a credential is created");
        let json = String::from_utf8(created.client_data_json).expect("utf-8");

        assert!(json.contains(r#""type":"webauthn.create""#), "{json}");
        assert!(json.contains(r#""origin":"https://example.com""#), "{json}");
        assert!(
            json.contains(&format!(
                r#""challenge":"{}""#,
                b64(b"a-challenge-from-the-site")
            )),
            "{json}"
        );
    }

    #[test]
    fn an_assertion_verifies_against_the_public_key() {
        let key = Passkey::generate();
        let signed = key.assert(&ceremony(), 0).expect("a signature");

        assert!(verify(
            &key.verifying_key(),
            &signed.authenticator_data,
            &signed.client_data_json,
            &signed.signature
        ));
    }

    #[test]
    fn an_assertion_signed_for_one_site_does_not_verify_for_another() {
        let key = Passkey::generate();
        let mut elsewhere = ceremony();
        elsewhere.rp_id = "evil.example".to_string();
        elsewhere.origin = "https://evil.example".to_string();

        let signed = key.assert(&elsewhere, 0).expect("a signature");
        let honest = key.assert(&ceremony(), 0).expect("a signature");

        // The signature covers the data naming the site, so swapping either
        // half breaks it — this is why a passkey cannot be phished.
        assert!(!verify(
            &key.verifying_key(),
            &honest.authenticator_data,
            &signed.client_data_json,
            &signed.signature
        ));
    }

    #[test]
    fn a_tampered_challenge_does_not_verify() {
        let key = Passkey::generate();
        let signed = key.assert(&ceremony(), 0).expect("a signature");
        let mut client_data = signed.client_data_json.clone();
        let at = client_data.len() - 2;
        client_data[at] ^= 0x20;

        assert!(!verify(
            &key.verifying_key(),
            &signed.authenticator_data,
            &client_data,
            &signed.signature
        ));
    }

    #[test]
    fn another_key_does_not_verify() {
        let signed = Passkey::generate()
            .assert(&ceremony(), 0)
            .expect("a signature");

        assert!(!verify(
            &Passkey::generate().verifying_key(),
            &signed.authenticator_data,
            &signed.client_data_json,
            &signed.signature
        ));
    }

    #[test]
    fn the_counter_goes_up_by_one_and_is_written_into_the_data() {
        let key = Passkey::generate();

        let first = key.assert(&ceremony(), 7).expect("a signature");
        assert_eq!(first.counter, 8);
        assert_eq!(counter_of(&first.authenticator_data), Some(8));

        let second = key.assert(&ceremony(), first.counter).expect("a signature");
        assert_eq!(second.counter, 9);
    }

    #[test]
    fn an_assertion_carries_no_attested_credential_data() {
        let signed = Passkey::generate()
            .assert(&ceremony(), 0)
            .expect("a signature");

        assert_eq!(signed.authenticator_data.len(), 37, "sign-in adds no key");
        assert_eq!(signed.authenticator_data[32] & FLAG_ATTESTED, 0);
    }

    #[test]
    fn base64url_round_trips_without_padding() {
        let bytes = b"\x00\x01\xfe\xff any bytes at all";
        let text = b64(bytes);

        assert!(!text.contains('='), "{text}");
        assert_eq!(unb64(&text).expect("decodes"), bytes);
    }

    /// The `authData` value out of an attestation object.
    ///
    /// The map is written by [`cbor_attestation`] in a fixed order, so the
    /// value is whatever follows the `authData` key and its byte-string head.
    fn auth_data_of(attestation: &[u8]) -> &[u8] {
        let marker = b"authData";
        let at = attestation
            .windows(marker.len())
            .position(|window| window == marker)
            .expect("an attestation object names its authData");
        let head = at + marker.len();

        // A byte string of this length: 0x58 for one length byte, 0x59 for two.
        match attestation[head] {
            0x58 => &attestation[head + 2..],
            0x59 => &attestation[head + 3..],
            other => panic!("unexpected authData head {other:#x}"),
        }
    }
}
