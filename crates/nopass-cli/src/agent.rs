//! The passphrase cache.
//!
//! Unlocking an identity is deliberately slow, and a day of `nopass show`
//! means typing the same passphrase over and over. The agent is a small
//! background process that holds unlocked identities **in memory** for a
//! configured number of seconds — nothing about the cache ever touches the
//! disk, so there is no file to steal and no state to survive a reboot.
//!
//! It listens on a unix socket in a directory only its owner can reach, and
//! the first command that unlocks something starts it. Reads may use it;
//! anything that changes the store asks the user directly (see
//! [`crate::auth`]), so a warm cache never authorises a write.
//!
//! Entries expire against the wall clock rather than a monotonic one: a
//! laptop that spends the night asleep should wake up with an empty cache.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use anyhow::{bail, Context, Result};

/// Overrides where the agent listens. Mostly for tests.
const SOCKET_ENV: &str = "NOPASS_AGENT_SOCK";

/// How long a freshly started agent waits for the caller that spawned it.
const STARTUP_GRACE: Duration = Duration::from_secs(30);

/// One cached identity: the unlocked secret and when it stops counting.
struct Entry {
    secret: String,
    expires: SystemTime,
}

/// The unlocked identities this agent is holding.
#[derive(Default)]
struct Cache {
    entries: HashMap<String, Entry>,
    /// Whether anything was ever stored. An agent that has served its
    /// purpose exits; one that has not yet been given anything waits.
    used: bool,
}

impl Cache {
    fn put(&mut self, key: &str, secret: &str, ttl: u64) {
        self.used = true;
        self.entries.insert(
            key.to_string(),
            Entry {
                secret: secret.to_string(),
                expires: SystemTime::now() + Duration::from_secs(ttl),
            },
        );
    }

    fn get(&mut self, key: &str) -> Option<&str> {
        self.prune();
        self.entries.get(key).map(|e| e.secret.as_str())
    }

    /// Forget everything that has run out of time.
    fn prune(&mut self) {
        let now = SystemTime::now();
        self.entries.retain(|_, e| e.expires > now);
    }

    /// Seconds until the last entry expires, for `nopass agent status`.
    fn longest_remaining(&self) -> u64 {
        let now = SystemTime::now();
        self.entries
            .values()
            .filter_map(|e| e.expires.duration_since(now).ok())
            .map(|d| d.as_secs())
            .max()
            .unwrap_or(0)
    }
}

/// Where the agent listens, or `None` when this machine offers nowhere
/// private to put a socket. A cache the neighbours can bind to would be
/// worse than no cache, so a shared `/tmp` is never used.
fn socket_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(SOCKET_ENV) {
        return Some(PathBuf::from(path));
    }
    let base = match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) => dir,
        // macOS has no XDG_RUNTIME_DIR, but every login session gets its own
        // 0700 temporary directory, which serves the same purpose.
        None if cfg!(target_os = "macos") => std::env::var_os("TMPDIR")?,
        None => return None,
    };
    Some(PathBuf::from(base).join("nopass/agent.sock"))
}

/// Make sure the socket's directory exists and is ours alone.
fn private_dir(socket: &Path) -> Result<()> {
    let dir = socket.parent().context("the socket needs a directory")?;
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    let meta = std::fs::metadata(dir)?;
    if meta.uid() != our_uid() {
        bail!("{} belongs to someone else", dir.display());
    }
    Ok(())
}

fn our_uid() -> u32 {
    // Safe: getuid() takes no arguments, touches no memory, cannot fail.
    unsafe { libc::getuid() }
}

/// Send one request and return the reply line, or `None` if no agent is
/// listening. One request per connection keeps the protocol trivial.
fn request(socket: &Path, line: &str) -> Option<String> {
    let mut stream = UnixStream::connect(socket).ok()?;
    let timeout = Some(Duration::from_secs(5));
    stream.set_read_timeout(timeout).ok()?;
    stream.set_write_timeout(timeout).ok()?;
    stream.write_all(line.as_bytes()).ok()?;
    stream.write_all(b"\n").ok()?;
    stream.flush().ok()?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).ok()?;
    Some(reply.trim_end_matches('\n').to_string())
}

/// A request that is pointless to start an agent for: if nobody is
/// listening, there is nothing cached.
fn ask(line: &str) -> Option<String> {
    let socket = socket_path()?;
    private_dir(&socket).ok()?;
    request(&socket, line)
}

/// The secret cached under `key`, if any.
pub fn get(key: &str) -> Option<String> {
    ask(&format!("GET {key}"))?
        .strip_prefix("OK ")
        .map(str::to_string)
}

/// Cache `secret` under `key` for `ttl` seconds, starting the agent if it is
/// not running yet. Best effort: a machine with nowhere private to listen
/// simply keeps asking for the passphrase.
pub fn put(key: &str, secret: &str, ttl: u64) {
    let Some(socket) = socket_path() else { return };
    if private_dir(&socket).is_err() {
        return;
    }
    let line = format!("PUT {key} {ttl} {secret}");
    if request(&socket, &line).is_some() {
        return;
    }
    spawn();
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(25));
        if request(&socket, &line).is_some() {
            return;
        }
    }
}

/// Forget everything. True if an agent was there to tell.
pub fn clear() -> bool {
    ask("DROP").is_some()
}

/// Number of cached identities and seconds until the last one expires.
pub fn status() -> Option<(usize, u64)> {
    let reply = ask("STATUS")?;
    let mut parts = reply.strip_prefix("OK ")?.split_whitespace();
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// Forget everything and shut the agent down.
pub fn stop() -> bool {
    ask("STOP").is_some()
}

/// Start an agent in the background. It gets its own session so that a
/// Ctrl-C in the terminal that happened to start it does not take the cache
/// down with it.
fn spawn() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = Command::new(exe);
    cmd.args(["agent", "serve"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let _ = cmd.spawn();
}

/// Run the agent. Returns when another one is already listening.
pub fn serve() -> Result<()> {
    let socket = socket_path().context("no private directory to listen in")?;
    private_dir(&socket)?;

    let listener = match UnixListener::bind(&socket) {
        Ok(listener) => listener,
        Err(_) => {
            // Either an agent is already there, or one died and left its
            // socket file behind.
            if UnixStream::connect(&socket).is_ok() {
                return Ok(());
            }
            std::fs::remove_file(&socket).ok();
            UnixListener::bind(&socket)?
        }
    };
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;

    let cache = Arc::new(Mutex::new(Cache::default()));
    reap(cache.clone(), socket.clone());

    for stream in listener.incoming().flatten() {
        serve_one(stream, &cache, &socket);
    }
    Ok(())
}

/// Drop expired entries once a second, and exit once there is nothing left
/// to hold — an agent with an empty cache is only a process to leak.
fn reap(cache: Arc<Mutex<Cache>>, socket: PathBuf) {
    let started = SystemTime::now();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let mut cache = cache.lock().expect("the cache lock is not poisoned");
        cache.prune();
        let waiting_for_first = !cache.used
            && started
                .elapsed()
                .map(|d| d < STARTUP_GRACE)
                .unwrap_or(false);
        if cache.entries.is_empty() && !waiting_for_first {
            shutdown(&socket);
        }
    });
}

fn shutdown(socket: &Path) -> ! {
    std::fs::remove_file(socket).ok();
    std::process::exit(0);
}

fn serve_one(stream: UnixStream, cache: &Mutex<Cache>, socket: &Path) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut cache = cache.lock().expect("the cache lock is not poisoned");
    let reply = match parse(line.trim_end_matches('\n')) {
        Some(Request::Get(key)) => match cache.get(&key) {
            Some(secret) => format!("OK {secret}"),
            None => "NONE".to_string(),
        },
        Some(Request::Put { key, ttl, secret }) => {
            cache.put(&key, &secret, ttl);
            "OK".to_string()
        }
        Some(Request::Drop) => {
            cache.entries.clear();
            "OK".to_string()
        }
        Some(Request::Status) => {
            cache.prune();
            format!("OK {} {}", cache.entries.len(), cache.longest_remaining())
        }
        Some(Request::Stop) => {
            cache.entries.clear();
            let _ = reader.get_mut().write_all(b"OK\n");
            shutdown(socket);
        }
        None => "ERR".to_string(),
    };
    let _ = writeln!(reader.get_mut(), "{reply}");
}

enum Request {
    Get(String),
    Put {
        key: String,
        ttl: u64,
        secret: String,
    },
    Drop,
    Status,
    Stop,
}

/// The wire format: one line, fields separated by single spaces. Keys are
/// hex and secrets hold no whitespace, so the secret can simply be the rest
/// of the line.
fn parse(line: &str) -> Option<Request> {
    let (verb, rest) = match line.split_once(' ') {
        Some((verb, rest)) => (verb, rest),
        None => (line, ""),
    };
    match verb {
        "GET" if !rest.is_empty() => Some(Request::Get(rest.to_string())),
        "PUT" => {
            let mut parts = rest.splitn(3, ' ');
            Some(Request::Put {
                key: parts.next()?.to_string(),
                ttl: parts.next()?.parse().ok()?,
                secret: parts.next()?.to_string(),
            })
        }
        "DROP" => Some(Request::Drop),
        "STATUS" => Some(Request::Status),
        "STOP" => Some(Request::Stop),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_is_returned_until_it_expires() {
        let mut cache = Cache::default();
        cache.put("k", "AGE-SECRET-KEY-1", 60);
        assert_eq!(cache.get("k"), Some("AGE-SECRET-KEY-1"));
        assert_eq!(cache.get("other"), None);

        // A zero-second lifetime is over before it is asked about.
        cache.put("k", "AGE-SECRET-KEY-1", 0);
        assert_eq!(cache.get("k"), None);
    }

    #[test]
    fn pruning_keeps_only_what_is_still_live() {
        let mut cache = Cache::default();
        cache.put("live", "s1", 60);
        cache.put("stale", "s2", 0);
        cache.prune();
        assert_eq!(cache.entries.len(), 1);
        assert!(cache.entries.contains_key("live"));
        assert!((1..=60).contains(&cache.longest_remaining()));
    }

    #[test]
    fn requests_parse_and_reject() {
        assert!(matches!(parse("GET abc"), Some(Request::Get(k)) if k == "abc"));
        assert!(matches!(
            parse("PUT abc 30 AGE-SECRET-KEY-1"),
            Some(Request::Put { key, ttl, secret })
                if key == "abc" && ttl == 30 && secret == "AGE-SECRET-KEY-1"
        ));
        assert!(matches!(parse("DROP"), Some(Request::Drop)));
        assert!(matches!(parse("STATUS"), Some(Request::Status)));
        assert!(matches!(parse("STOP"), Some(Request::Stop)));
        assert!(parse("GET").is_none());
        assert!(parse("PUT abc 30").is_none());
        assert!(parse("PUT abc soon secret").is_none());
        assert!(parse("nonsense").is_none());
    }
}
