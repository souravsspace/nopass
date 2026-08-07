use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use age::secrecy::ExposeSecret;

use crate::config;
use crate::error::{Error, Result};
use crate::lock::{encrypt_slot, LockedIdentity, SecretString, Unlocker};

/// Pluggable encryption backend. The default is [`NativeCrypto`], which
/// needs nothing but the nopass binary itself; [`GpgCrypto`] is available
/// for users who prefer their existing gpg keys.
pub trait Crypto: Send + Sync {
    fn encrypt(&self, plaintext: &[u8], recipients: &[String], dest: &Path) -> Result<()>;
    fn decrypt(&self, src: &Path) -> Result<Vec<u8>>;
}

/// Built-in backend using age (X25519 + ChaCha20-Poly1305), fully in-process.
/// Recipients are `age1...` public keys; the secret identity lives in a
/// local identity file created by `nopass keygen`.
pub struct NativeCrypto {
    identity_file: PathBuf,
    unlocker: Option<Box<dyn Unlocker>>,
    /// The secret recovered by the last successful unlock, kept for the life
    /// of this process only. A single command can decrypt many entries —
    /// `grep` reads the whole store, `init` re-encrypts it — and asking for
    /// the passphrase once per file would be unusable. Nothing is written
    /// anywhere, so the next command asks again.
    unlocked: Mutex<Option<String>>,
}

impl NativeCrypto {
    pub fn new() -> Self {
        Self::with_identity_file(default_identity_file())
    }

    pub fn with_identity_file(identity_file: PathBuf) -> Self {
        Self {
            identity_file,
            unlocker: None,
            unlocked: Mutex::new(None),
        }
    }

    /// Attach an [`Unlocker`] so locked identity files can be opened by
    /// prompting for a passkey/passphrase. Without one, locked identities
    /// fail with [`Error::Locked`].
    pub fn with_unlocker(mut self, unlocker: Box<dyn Unlocker>) -> Self {
        self.unlocker = Some(unlocker);
        self
    }

    fn parse_identities(text: &str) -> Vec<age::x25519::Identity> {
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| l.parse().ok())
            .collect()
    }

    fn identities(&self) -> Result<Vec<age::x25519::Identity>> {
        let contents = std::fs::read_to_string(&self.identity_file)
            .map_err(|_| Error::NoIdentity(self.identity_file.clone()))?;

        // A locked identity must be unlocked (Touch ID / passphrase) before
        // its secret key is usable.
        if LockedIdentity::is_locked_file(&contents) {
            let mut cached = self.unlocked.lock().expect("unlock cache is not poisoned");
            if cached.is_none() {
                let locked = LockedIdentity::parse(&contents)
                    .map_err(|_| Error::MalformedLock(self.identity_file.clone()))?;
                let unlocker = self.unlocker.as_ref().ok_or(Error::Locked)?;
                *cached = Some(unlocker.unlock(&locked)?);
            }
            let ids = Self::parse_identities(cached.as_deref().expect("just unlocked"));
            if ids.is_empty() {
                return Err(Error::AuthFailed(
                    "unlocked data did not contain a valid identity".into(),
                ));
            }
            return Ok(ids);
        }

        let ids = Self::parse_identities(&contents);
        if ids.is_empty() {
            return Err(Error::NoIdentity(self.identity_file.clone()));
        }
        Ok(ids)
    }
}

impl Default for NativeCrypto {
    fn default() -> Self {
        Self::new()
    }
}

/// Identity location, in order of precedence: the `NOPASS_IDENTITY`
/// environment variable, the path recorded by first-run setup in the config
/// file, then `~/.config/nopass/identity.txt`.
pub fn default_identity_file() -> PathBuf {
    if let Some(path) = std::env::var_os("NOPASS_IDENTITY") {
        return PathBuf::from(path);
    }
    if let Some(path) = config::read_identity_path(&config::default_config_file()) {
        return path;
    }
    config::default_identity_location()
}

/// Write `contents` to `path` with 0600 permissions, creating parent dirs.
/// Every file holding key material goes through here.
pub fn write_secret_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Generate a fresh keypair, write the secret identity to `path` (0600),
/// and return the public recipient string.
pub fn generate_identity(path: &Path) -> Result<String> {
    let identity = age::x25519::Identity::generate();
    let public = identity.to_public().to_string();
    write_secret_file(
        path,
        &identity_file_contents(&public, &identity_secret(&identity)),
    )?;
    Ok(public)
}

/// Generate a fresh keypair and write it to `path` already locked behind
/// `passphrase`, so the secret key never exists on disk in the clear.
/// Returns the public recipient string.
pub fn generate_identity_locked(path: &Path, passphrase: &SecretString) -> Result<String> {
    let identity = age::x25519::Identity::generate();
    let public = identity.to_public().to_string();
    let locked = LockedIdentity {
        public: Some(public.clone()),
        passphrase_slot: Some(encrypt_slot(&identity_secret(&identity), passphrase)?),
        ..Default::default()
    };
    write_secret_file(path, &locked.serialize())?;
    Ok(public)
}

fn identity_secret(identity: &age::x25519::Identity) -> String {
    identity.to_string().expose_secret().to_string()
}

fn identity_file_contents(public: &str, secret: &str) -> String {
    format!(
        "# nopass identity file — keep this secret\n# public key: {public}\n{}\n",
        secret.trim()
    )
}

/// Read the plaintext `AGE-SECRET-KEY-...` string from an *unlocked*
/// identity file. Returns [`Error::Locked`] if the file is already locked.
pub fn read_identity_secret(path: &Path) -> Result<String> {
    let contents =
        std::fs::read_to_string(path).map_err(|_| Error::NoIdentity(path.to_path_buf()))?;
    if LockedIdentity::is_locked_file(&contents) {
        return Err(Error::Locked);
    }
    contents
        .lines()
        .map(str::trim)
        .find(|l| {
            !l.is_empty() && !l.starts_with('#') && l.parse::<age::x25519::Identity>().is_ok()
        })
        .map(str::to_string)
        .ok_or_else(|| Error::NoIdentity(path.to_path_buf()))
}

/// Derive the public recipient (`age1...`) from a secret-key string.
pub fn public_from_secret(secret: &str) -> Option<String> {
    secret
        .trim()
        .parse::<age::x25519::Identity>()
        .ok()
        .map(|id| id.to_public().to_string())
}

/// Write a plaintext identity file (0600) holding `secret`, returning its
/// public recipient. Inverse of locking — used by `passkey disable`.
pub fn write_plaintext_identity(path: &Path, secret: &str) -> Result<String> {
    let public = public_from_secret(secret)
        .ok_or_else(|| Error::AuthFailed("not a valid age secret key".into()))?;
    write_secret_file(path, &identity_file_contents(&public, secret))?;
    Ok(public)
}

/// Read the public recipient corresponding to the identity file, if present.
/// Works for locked identities too: encrypting to yourself never requires
/// unlocking anything.
pub fn identity_recipient(path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    if LockedIdentity::is_locked_file(&contents) {
        return LockedIdentity::parse(&contents).ok()?.public;
    }
    contents
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .find_map(|l| {
            l.parse::<age::x25519::Identity>()
                .ok()
                .map(|id| id.to_public().to_string())
        })
}

impl Crypto for NativeCrypto {
    fn encrypt(&self, plaintext: &[u8], recipients: &[String], dest: &Path) -> Result<()> {
        let parsed: Vec<age::x25519::Recipient> = recipients
            .iter()
            .map(|r| {
                r.parse()
                    .map_err(|e: &str| Error::Encrypt(format!("bad recipient {r}: {e}")))
            })
            .collect::<Result<_>>()?;
        let encryptor =
            age::Encryptor::with_recipients(parsed.iter().map(|r| r as &dyn age::Recipient))
                .map_err(|e| Error::Encrypt(e.to_string()))?;
        let mut ciphertext = Vec::new();
        let mut writer = encryptor
            .wrap_output(&mut ciphertext)
            .map_err(|e| Error::Encrypt(e.to_string()))?;
        writer
            .write_all(plaintext)
            .and_then(|_| writer.finish().map(|_| ()))
            .map_err(|e| Error::Encrypt(e.to_string()))?;
        std::fs::write(dest, ciphertext)?;
        Ok(())
    }

    fn decrypt(&self, src: &Path) -> Result<Vec<u8>> {
        let data = std::fs::read(src)?;
        let identities = self.identities()?;
        let decryptor =
            age::Decryptor::new_buffered(&data[..]).map_err(|e| Error::Decrypt(e.to_string()))?;
        let mut reader = decryptor
            .decrypt(identities.iter().map(|i| i as &dyn age::Identity))
            .map_err(|e| Error::Decrypt(e.to_string()))?;
        let mut plaintext = Vec::new();
        reader
            .read_to_end(&mut plaintext)
            .map_err(|e| Error::Decrypt(e.to_string()))?;
        Ok(plaintext)
    }
}

/// Optional backend that shells out to `gpg` (or `gpg2`) for users who
/// already manage GPG keys. Recipients are GPG key ids/fingerprints.
pub struct GpgCrypto {
    program: String,
    extra_opts: Vec<String>,
}

impl GpgCrypto {
    pub fn new() -> Self {
        let program = if Command::new("gpg2")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            "gpg2".to_string()
        } else {
            "gpg".to_string()
        };
        let extra_opts = std::env::var("NOPASS_GPG_OPTS")
            .map(|v| v.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        Self {
            program,
            extra_opts,
        }
    }

    fn base_cmd(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.extra_opts);
        cmd.args([
            "--quiet",
            "--yes",
            "--compress-algo=none",
            "--no-encrypt-to",
            "--batch",
        ]);
        cmd
    }
}

impl Default for GpgCrypto {
    fn default() -> Self {
        Self::new()
    }
}

impl Crypto for GpgCrypto {
    fn encrypt(&self, plaintext: &[u8], recipients: &[String], dest: &Path) -> Result<()> {
        let mut cmd = self.base_cmd();
        cmd.arg("-e");
        for r in recipients {
            cmd.args(["-r", r]);
        }
        cmd.arg("-o").arg(dest);
        cmd.stdin(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| Error::Encrypt(e.to_string()))?;
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(plaintext)?;
        let out = child.wait_with_output()?;
        if !out.status.success() {
            return Err(Error::Encrypt(String::from_utf8_lossy(&out.stderr).into()));
        }
        Ok(())
    }

    fn decrypt(&self, src: &Path) -> Result<Vec<u8>> {
        let out = self
            .base_cmd()
            .arg("-d")
            .arg(src)
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| Error::Decrypt(e.to_string()))?;
        if !out.status.success() {
            return Err(Error::Decrypt(String::from_utf8_lossy(&out.stderr).into()));
        }
        Ok(out.stdout)
    }
}

/// Test backend: writes a header naming the "recipients" followed by the
/// plaintext. Enabled via NOPASS_BACKEND=plain. Never use for real secrets.
pub struct PlainCrypto;

const PLAIN_MAGIC: &str = "NOPASS-PLAIN:";

impl Crypto for PlainCrypto {
    fn encrypt(&self, plaintext: &[u8], recipients: &[String], dest: &Path) -> Result<()> {
        let mut data = format!("{PLAIN_MAGIC}{}\n", recipients.join(",")).into_bytes();
        data.extend_from_slice(plaintext);
        std::fs::write(dest, data)?;
        Ok(())
    }

    fn decrypt(&self, src: &Path) -> Result<Vec<u8>> {
        let data = std::fs::read(src)?;
        let text = String::from_utf8_lossy(&data);
        if !text.starts_with(PLAIN_MAGIC) {
            return Err(Error::Decrypt("not a nopass-plain file".into()));
        }
        let body_start = data
            .iter()
            .position(|&b| b == b'\n')
            .map(|i| i + 1)
            .unwrap_or(data.len());
        Ok(data[body_start..].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("x.np");
        let recipients = vec!["KEY1".to_string()];
        PlainCrypto
            .encrypt(b"secret\nline2\n", &recipients, &file)
            .unwrap();
        assert_eq!(PlainCrypto.decrypt(&file).unwrap(), b"secret\nline2\n");
    }

    #[test]
    fn plain_rejects_foreign_files() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("x.np");
        std::fs::write(&file, b"garbage").unwrap();
        assert!(PlainCrypto.decrypt(&file).is_err());
    }

    #[test]
    fn native_keygen_encrypt_decrypt_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let identity_file = tmp.path().join("identity.txt");
        let public = generate_identity(&identity_file).unwrap();
        assert!(public.starts_with("age1"), "{public}");
        assert_eq!(identity_recipient(&identity_file).unwrap(), public);

        let crypto = NativeCrypto::with_identity_file(identity_file);
        let file = tmp.path().join("x.np");
        crypto.encrypt(b"top secret\n", &[public], &file).unwrap();
        let raw = std::fs::read(&file).unwrap();
        assert_ne!(raw, b"top secret\n");
        assert_eq!(crypto.decrypt(&file).unwrap(), b"top secret\n");
    }

    #[test]
    fn native_decrypt_fails_with_wrong_identity() {
        let tmp = tempfile::tempdir().unwrap();
        let alice_file = tmp.path().join("alice.txt");
        let bob_file = tmp.path().join("bob.txt");
        let alice_pub = generate_identity(&alice_file).unwrap();
        generate_identity(&bob_file).unwrap();

        let file = tmp.path().join("x.np");
        NativeCrypto::with_identity_file(alice_file)
            .encrypt(b"for alice\n", &[alice_pub], &file)
            .unwrap();
        assert!(NativeCrypto::with_identity_file(bob_file)
            .decrypt(&file)
            .is_err());
    }

    #[test]
    fn native_missing_identity_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let crypto = NativeCrypto::with_identity_file(tmp.path().join("nope.txt"));
        let file = tmp.path().join("x.np");
        std::fs::write(&file, b"whatever").unwrap();
        assert!(matches!(crypto.decrypt(&file), Err(Error::NoIdentity(_))));
    }

    #[test]
    fn locked_identity_needs_unlocker_then_decrypts() {
        use crate::lock::{decrypt_slot, encrypt_slot, LockedIdentity, Unlocker};
        use age::secrecy::SecretString;

        let tmp = tempfile::tempdir().unwrap();
        let id_file = tmp.path().join("identity.txt");
        let public = generate_identity(&id_file).unwrap();
        let secret = read_identity_secret(&id_file).unwrap();

        // Lock the identity into a passphrase slot.
        let locked = LockedIdentity {
            passphrase_slot: Some(
                encrypt_slot(&secret, &SecretString::from("pw".to_owned())).unwrap(),
            ),
            keychain_slot: None,
            ..Default::default()
        };
        std::fs::write(&id_file, locked.serialize()).unwrap();

        // Encryption to the public recipient needs no identity.
        let file = tmp.path().join("x.np");
        NativeCrypto::with_identity_file(id_file.clone())
            .encrypt(b"hi\n", &[public], &file)
            .unwrap();

        // Without an unlocker, decryption is refused.
        let locked_crypto = NativeCrypto::with_identity_file(id_file.clone());
        assert!(matches!(locked_crypto.decrypt(&file), Err(Error::Locked)));

        // With an unlocker that knows the passphrase, it succeeds.
        struct PwUnlocker;
        impl Unlocker for PwUnlocker {
            fn unlock(&self, locked: &LockedIdentity) -> Result<String> {
                decrypt_slot(
                    locked.passphrase_slot.as_ref().unwrap(),
                    &SecretString::from("pw".to_owned()),
                )
            }
        }
        let unlocked =
            NativeCrypto::with_identity_file(id_file).with_unlocker(Box::new(PwUnlocker));
        assert_eq!(unlocked.decrypt(&file).unwrap(), b"hi\n");
    }

    #[test]
    fn generated_locked_identity_never_touches_disk_in_the_clear() {
        use crate::lock::decrypt_slot;

        let tmp = tempfile::tempdir().unwrap();
        let id_file = tmp.path().join("identity.txt");
        let passphrase = SecretString::from("master".to_owned());
        let public = generate_identity_locked(&id_file, &passphrase).unwrap();

        assert!(public.starts_with("age1"), "{public}");
        let on_disk = std::fs::read_to_string(&id_file).unwrap();
        assert!(LockedIdentity::is_locked_file(&on_disk), "{on_disk}");
        assert!(!on_disk.contains("AGE-SECRET-KEY"), "{on_disk}");
        assert!(matches!(read_identity_secret(&id_file), Err(Error::Locked)));

        // The passphrase recovers exactly the key the public half names.
        let locked = LockedIdentity::parse(&on_disk).unwrap();
        let secret = decrypt_slot(locked.passphrase_slot.as_ref().unwrap(), &passphrase).unwrap();
        assert_eq!(public_from_secret(&secret).unwrap(), public);
        assert!(decrypt_slot(
            locked.passphrase_slot.as_ref().unwrap(),
            &SecretString::from("wrong".to_owned())
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn identity_files_are_private_to_their_owner() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let plain = tmp.path().join("plain.txt");
        generate_identity(&plain).unwrap();
        let secret = read_identity_secret(&plain).unwrap();

        let locked = tmp.path().join("sub/locked.txt");
        generate_identity_locked(&locked, &SecretString::from("pw".to_owned())).unwrap();

        let restored = tmp.path().join("restored.txt");
        write_plaintext_identity(&restored, &secret).unwrap();

        for path in [plain, locked, restored] {
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(
                mode & 0o777,
                0o600,
                "{} should be 0600, was {mode:o}",
                path.display()
            );
        }
    }

    #[test]
    fn one_command_unlocks_the_identity_only_once() {
        use crate::lock::decrypt_slot;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let tmp = tempfile::tempdir().unwrap();
        let id_file = tmp.path().join("identity.txt");
        let public =
            generate_identity_locked(&id_file, &SecretString::from("pw".to_owned())).unwrap();

        // Three entries, as `grep` or a re-encrypt would walk.
        let files: Vec<PathBuf> = (0..3).map(|i| tmp.path().join(format!("{i}.np"))).collect();
        let writer = NativeCrypto::with_identity_file(id_file.clone());
        for file in &files {
            writer
                .encrypt(b"secret\n", std::slice::from_ref(&public), file)
                .unwrap();
        }

        struct CountingUnlocker(Arc<AtomicUsize>);
        impl Unlocker for CountingUnlocker {
            fn unlock(&self, locked: &LockedIdentity) -> Result<String> {
                self.0.fetch_add(1, Ordering::SeqCst);
                decrypt_slot(
                    locked.passphrase_slot.as_ref().unwrap(),
                    &SecretString::from("pw".to_owned()),
                )
            }
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let crypto = NativeCrypto::with_identity_file(id_file.clone())
            .with_unlocker(Box::new(CountingUnlocker(calls.clone())));
        for file in &files {
            assert_eq!(crypto.decrypt(file).unwrap(), b"secret\n");
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "decrypting many entries must prompt for authentication only once"
        );

        // Nothing is cached beyond this backend, so the next command (a new
        // process, a new NativeCrypto) authenticates again.
        let fresh = NativeCrypto::with_identity_file(id_file)
            .with_unlocker(Box::new(CountingUnlocker(calls.clone())));
        fresh.decrypt(&files[0]).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn read_identity_secret_refuses_locked() {
        use crate::lock::{encrypt_slot, LockedIdentity};
        use age::secrecy::SecretString;

        let tmp = tempfile::tempdir().unwrap();
        let id_file = tmp.path().join("identity.txt");
        generate_identity(&id_file).unwrap();
        let secret = read_identity_secret(&id_file).unwrap();
        assert!(secret.parse::<age::x25519::Identity>().is_ok());

        let locked = LockedIdentity {
            passphrase_slot: Some(
                encrypt_slot(&secret, &SecretString::from("pw".to_owned())).unwrap(),
            ),
            keychain_slot: None,
            ..Default::default()
        };
        std::fs::write(&id_file, locked.serialize()).unwrap();
        assert!(matches!(read_identity_secret(&id_file), Err(Error::Locked)));
    }
}
