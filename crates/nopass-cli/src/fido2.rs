//! FIDO2 security keys (passkeys) as an unlock slot.
//!
//! The mechanism is the CTAP2 `hmac-secret` extension, the same one
//! `systemd-cryptenroll` uses for disk encryption. At enrollment we ask the
//! authenticator to create a credential with `hmac-secret` enabled, then ask
//! it — with a random 32-byte salt — for `HMAC-SHA256(credRandom, salt)`.
//! That output is a uniform 256-bit key, and it is what seals the identity
//! slot. Only the credential id and the salt are stored; `credRandom` never
//! leaves the authenticator, so the slot is unopenable without the physical
//! key (plus its PIN, if one was required at enrollment).
//!
//! Two backends implement [`SecurityKey`]:
//!
//! * `hid` — real USB/NFC authenticators, behind the `security-key` feature
//!   (it pulls in a C HID stack, so it is opt-in exactly like `touchid`).
//! * `mock` — a file-backed software authenticator, active only when
//!   `NOPASS_FIDO2_MOCK` names a state file. It exists so the enroll/unlock
//!   flows are testable without hardware, mirroring the existing
//!   `NOPASS_BACKEND=plain` test backend.

use nopass_core::lock::HMAC_SECRET_LEN;
use nopass_core::{Error as CoreError, Result as CoreResult};

/// Relying-party id our credentials are created under. Not a real domain:
/// nopass is not a website, and the value only has to be stable so that a
/// key can find its own credential again.
pub const RP_ID: &str = "nopass.local";

/// Names a state file for the software test authenticator.
pub const MOCK_ENV: &str = "NOPASS_FIDO2_MOCK";

/// A key released by an authenticator: the raw `hmac-secret` output.
pub type HmacSecret = [u8; HMAC_SECRET_LEN];

/// What a fresh enrollment yields.
pub struct Enrollment {
    pub credential_id: Vec<u8>,
    pub secret: HmacSecret,
}

/// An authenticator we can enroll into and later derive keys from.
pub trait SecurityKey {
    /// How to describe this backend to the user.
    fn describe(&self) -> &'static str;

    /// Create a credential and immediately derive its secret for `salt`.
    /// On real hardware this asks for two touches: CTAP only returns an
    /// `hmac-secret` output from an assertion, never from registration.
    fn enroll(&self, rp_id: &str, salt: &HmacSecret, pin: Option<&str>) -> CoreResult<Enrollment>;

    /// Re-derive the secret for an existing credential.
    fn derive(
        &self,
        rp_id: &str,
        credential_id: &[u8],
        salt: &HmacSecret,
        pin: Option<&str>,
    ) -> CoreResult<HmacSecret>;
}

/// Pick the authenticator backend for this process, if there is one.
pub fn detect() -> Option<Box<dyn SecurityKey>> {
    if let Some(path) = std::env::var_os(MOCK_ENV) {
        eprintln!(
            "warning: {MOCK_ENV} is set — using a software test authenticator. \
             Slots enrolled this way are NOT hardware-backed."
        );
        return Some(Box::new(mock::MockSecurityKey::new(path.into())));
    }
    #[cfg(feature = "security-key")]
    {
        Some(Box::new(hid::HidSecurityKey))
    }
    #[cfg(not(feature = "security-key"))]
    {
        None
    }
}

/// Why no authenticator is usable, phrased for the user.
pub fn unavailable_reason() -> String {
    if cfg!(feature = "security-key") {
        "no FIDO2 security key was found (is it plugged in?)".to_string()
    } else {
        "this build has no security-key support; rebuild with --features security-key".to_string()
    }
}

/// A fresh random salt for a new slot.
pub fn random_salt() -> HmacSecret {
    use rand::RngCore;
    let mut salt = [0u8; HMAC_SECRET_LEN];
    rand::rng().fill_bytes(&mut salt);
    salt
}

/// Software authenticator used by the test suite.
mod mock {
    use super::*;
    use base64::Engine;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use std::path::PathBuf;

    fn b64() -> base64::engine::general_purpose::GeneralPurpose {
        base64::engine::general_purpose::STANDARD
    }

    /// One "credential" the fake device holds: the per-credential seed the
    /// real hardware would keep in its secure element, plus the PIN that
    /// must be presented to use it.
    struct Credential {
        id: Vec<u8>,
        seed: Vec<u8>,
        pin: Option<String>,
    }

    /// A file-backed stand-in for a hardware authenticator. The state file
    /// *is* the device: point two runs at the same path and they share one
    /// key; point them at different paths and they are different keys.
    pub struct MockSecurityKey {
        path: PathBuf,
    }

    impl MockSecurityKey {
        pub fn new(path: PathBuf) -> Self {
            Self { path }
        }

        fn load(&self) -> Vec<Credential> {
            let Ok(text) = std::fs::read_to_string(&self.path) else {
                return Vec::new();
            };
            text.lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|line| {
                    let mut parts = line.split_whitespace();
                    let id = b64().decode(parts.next()?).ok()?;
                    let seed = b64().decode(parts.next()?).ok()?;
                    let pin = match parts.next() {
                        None | Some("-") => None,
                        Some(p) => Some(p.to_string()),
                    };
                    Some(Credential { id, seed, pin })
                })
                .collect()
        }

        fn append(&self, cred: &Credential) -> CoreResult<()> {
            use std::io::Write;
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            writeln!(
                file,
                "{} {} {}",
                b64().encode(&cred.id),
                b64().encode(&cred.seed),
                cred.pin.as_deref().unwrap_or("-")
            )?;
            Ok(())
        }
    }

    /// The one real piece of CTAP semantics worth reproducing:
    /// `HMAC-SHA256(credRandom, salt)`, bound to the relying party so the
    /// same credential under a different rp id yields a different key.
    fn hmac_secret(seed: &[u8], rp_id: &str, salt: &HmacSecret) -> HmacSecret {
        let mut mac = <Hmac<Sha256>>::new_from_slice(seed).expect("hmac accepts any key length");
        mac.update(rp_id.as_bytes());
        mac.update(&[0]);
        mac.update(salt);
        mac.finalize().into_bytes().into()
    }

    impl SecurityKey for MockSecurityKey {
        fn describe(&self) -> &'static str {
            "software test authenticator"
        }

        fn enroll(
            &self,
            rp_id: &str,
            salt: &HmacSecret,
            pin: Option<&str>,
        ) -> CoreResult<Enrollment> {
            use rand::RngCore;
            let mut rng = rand::rng();
            let mut id = vec![0u8; 32];
            let mut seed = vec![0u8; 32];
            rng.fill_bytes(&mut id);
            rng.fill_bytes(&mut seed);

            let cred = Credential {
                id,
                seed,
                pin: pin.map(str::to_string),
            };
            self.append(&cred)?;
            Ok(Enrollment {
                secret: hmac_secret(&cred.seed, rp_id, salt),
                credential_id: cred.id,
            })
        }

        fn derive(
            &self,
            rp_id: &str,
            credential_id: &[u8],
            salt: &HmacSecret,
            pin: Option<&str>,
        ) -> CoreResult<HmacSecret> {
            let cred = self
                .load()
                .into_iter()
                .find(|c| c.id == credential_id)
                .ok_or_else(|| {
                    CoreError::AuthFailed("this security key does not hold that credential".into())
                })?;
            if cred.pin.is_some() && cred.pin.as_deref() != pin {
                return Err(CoreError::AuthFailed("wrong security key PIN".into()));
            }
            Ok(hmac_secret(&cred.seed, rp_id, salt))
        }
    }
}

/// Real authenticators over USB HID.
#[cfg(feature = "security-key")]
mod hid {
    use super::*;
    use ctap_hid_fido2::fidokey::get_assertion::get_assertion_params::Extension as GetExtension;
    use ctap_hid_fido2::fidokey::make_credential::make_credential_params::Extension as MakeExtension;
    use ctap_hid_fido2::fidokey::{GetAssertionArgsBuilder, MakeCredentialArgsBuilder};
    use ctap_hid_fido2::{verifier, Cfg, FidoKeyHid, FidoKeyHidFactory};

    pub struct HidSecurityKey;

    fn open() -> CoreResult<FidoKeyHid> {
        FidoKeyHidFactory::create(&Cfg::init()).map_err(|e| {
            CoreError::AuthUnavailable(format!("could not open a FIDO2 security key: {e}"))
        })
    }

    impl SecurityKey for HidSecurityKey {
        fn describe(&self) -> &'static str {
            "FIDO2 security key"
        }

        fn enroll(
            &self,
            rp_id: &str,
            salt: &HmacSecret,
            pin: Option<&str>,
        ) -> CoreResult<Enrollment> {
            let device = open()?;
            let challenge = verifier::create_challenge();
            let mut builder = MakeCredentialArgsBuilder::new(rp_id, &challenge)
                .extensions(&[MakeExtension::HmacSecret(Some(true))]);
            builder = match pin {
                Some(pin) => builder.pin(pin),
                None => builder.without_pin_and_uv(),
            };
            let attestation = device
                .make_credential_with_args(&builder.build())
                .map_err(|e| CoreError::AuthFailed(format!("registration failed: {e}")))?;
            let credential_id = attestation.credential_descriptor.id;

            // CTAP only hands out hmac-secret output on an assertion, so the
            // freshly created credential has to be exercised once here.
            let secret = self.derive(rp_id, &credential_id, salt, pin)?;
            Ok(Enrollment {
                credential_id,
                secret,
            })
        }

        fn derive(
            &self,
            rp_id: &str,
            credential_id: &[u8],
            salt: &HmacSecret,
            pin: Option<&str>,
        ) -> CoreResult<HmacSecret> {
            let device = open()?;
            let challenge = verifier::create_challenge();
            let mut builder = GetAssertionArgsBuilder::new(rp_id, &challenge)
                .add_credential_id(credential_id)
                .extensions(&[GetExtension::HmacSecret(Some(*salt))]);
            builder = match pin {
                Some(pin) => builder.pin(pin),
                None => builder.without_pin_and_uv(),
            };
            let assertions = device
                .get_assertion_with_args(&builder.build())
                .map_err(|e| CoreError::AuthFailed(format!("authentication failed: {e}")))?;

            assertions
                .iter()
                .flat_map(|a| a.extensions.iter())
                .find_map(|ext| match ext {
                    GetExtension::HmacSecret(Some(output)) => Some(*output),
                    _ => None,
                })
                .ok_or_else(|| {
                    CoreError::AuthFailed(
                        "the security key returned no hmac-secret output (does it support the \
                         extension?)"
                            .into(),
                    )
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock device backed by a fresh temp file, plus the tempdir keeping
    /// it alive.
    fn device() -> (Box<dyn SecurityKey>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let key: Box<dyn SecurityKey> =
            Box::new(mock::MockSecurityKey::new(dir.path().join("device")));
        (key, dir)
    }

    #[test]
    fn enrolled_credential_re_derives_the_same_secret() {
        let (key, _dir) = device();
        let salt = random_salt();
        let enrolled = key.enroll(RP_ID, &salt, None).unwrap();
        let again = key
            .derive(RP_ID, &enrolled.credential_id, &salt, None)
            .unwrap();
        assert_eq!(enrolled.secret, again);
    }

    #[test]
    fn a_different_salt_yields_a_different_secret() {
        let (key, _dir) = device();
        let enrolled = key.enroll(RP_ID, &random_salt(), None).unwrap();
        let other = key
            .derive(RP_ID, &enrolled.credential_id, &random_salt(), None)
            .unwrap();
        assert_ne!(enrolled.secret, other);
    }

    #[test]
    fn secrets_are_bound_to_the_relying_party() {
        let (key, _dir) = device();
        let salt = random_salt();
        let enrolled = key.enroll(RP_ID, &salt, None).unwrap();
        let elsewhere = key
            .derive("someone-else", &enrolled.credential_id, &salt, None)
            .unwrap();
        assert_ne!(enrolled.secret, elsewhere);
    }

    #[test]
    fn each_enrollment_gets_its_own_credential_and_secret() {
        let (key, _dir) = device();
        let salt = random_salt();
        let first = key.enroll(RP_ID, &salt, None).unwrap();
        let second = key.enroll(RP_ID, &salt, None).unwrap();
        assert_ne!(first.credential_id, second.credential_id);
        assert_ne!(first.secret, second.secret);
        // Both remain usable: the device holds several credentials.
        assert_eq!(
            key.derive(RP_ID, &first.credential_id, &salt, None)
                .unwrap(),
            first.secret
        );
        assert_eq!(
            key.derive(RP_ID, &second.credential_id, &salt, None)
                .unwrap(),
            second.secret
        );
    }

    #[test]
    fn an_unknown_credential_is_refused() {
        let (key, _dir) = device();
        assert!(matches!(
            key.derive(RP_ID, b"not-a-credential", &random_salt(), None),
            Err(CoreError::AuthFailed(_))
        ));
    }

    #[test]
    fn another_device_cannot_derive_our_secret() {
        let (mine, _mine_dir) = device();
        let (theirs, _their_dir) = device();
        let salt = random_salt();
        let enrolled = mine.enroll(RP_ID, &salt, None).unwrap();
        // The credential id is public, but the seed behind it is not.
        assert!(theirs
            .derive(RP_ID, &enrolled.credential_id, &salt, None)
            .is_err());
    }

    #[test]
    fn a_pin_protected_credential_needs_that_pin() {
        let (key, _dir) = device();
        let salt = random_salt();
        let enrolled = key.enroll(RP_ID, &salt, Some("1234")).unwrap();

        assert_eq!(
            key.derive(RP_ID, &enrolled.credential_id, &salt, Some("1234"))
                .unwrap(),
            enrolled.secret
        );
        for wrong in [None, Some("0000")] {
            assert!(
                key.derive(RP_ID, &enrolled.credential_id, &salt, wrong)
                    .is_err(),
                "should have rejected pin {wrong:?}"
            );
        }
    }

    #[test]
    fn credentials_survive_across_processes() {
        // A new backend object over the same state file is the same device.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("device");
        let salt = random_salt();
        let enrolled = mock::MockSecurityKey::new(path.clone())
            .enroll(RP_ID, &salt, None)
            .unwrap();
        let secret = mock::MockSecurityKey::new(path)
            .derive(RP_ID, &enrolled.credential_id, &salt, None)
            .unwrap();
        assert_eq!(enrolled.secret, secret);
    }
}
