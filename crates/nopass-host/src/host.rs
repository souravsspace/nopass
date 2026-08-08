//! Turning a request into a reply.

use nopass_core::{generate, Error as CoreError, Store};
use serde_json::Value;

use crate::entry;
use crate::origin::{self, Candidate};
use crate::proto::{
    self, ErrorCode, LockState, Match, Request, Secret, StoreState, Success, Yes, KNOWN_VERBS,
    MUTATING_VERBS, PROTOCOL_VERSION,
};
use crate::session::Session;

pub struct Host {
    store: Store,
    session: Session,
}

impl Host {
    /// A host over `store`, unlocking through the default identity file.
    pub fn new(store: Store) -> Self {
        Self {
            store,
            session: Session::default(),
        }
    }

    pub fn with_session(store: Store, session: Session) -> Self {
        Self { store, session }
    }

    /// Answer one request. Never panics and never returns nothing: the
    /// browser is waiting on a reply for every id it sent.
    pub fn handle(&mut self, raw: &Value) -> Value {
        let id = raw.get("id").and_then(Value::as_u64).unwrap_or(0) as u32;
        let verb = raw.get("verb").and_then(Value::as_str).unwrap_or_default();

        // Refused on principle, and said so plainly: a caller who asks to
        // rewrite or remove something should learn that this host will not,
        // rather than that it has never heard of the word (ADR-0006).
        if MUTATING_VERBS.contains(&verb) {
            return proto::failure(
                id,
                ErrorCode::ReadOnly,
                format!("`{verb}` would change an existing entry; this host only creates"),
            );
        }

        match proto::parse_request(raw) {
            Ok(request) => self.dispatch(request),
            Err(error) if KNOWN_VERBS.contains(&verb) => {
                proto::failure(id, ErrorCode::BadRequest, error.to_string())
            }
            Err(_) => proto::failure(
                id,
                ErrorCode::UnsupportedVerb,
                format!("`{verb}` is not a verb this host understands"),
            ),
        }
    }

    fn dispatch(&mut self, request: Request) -> Value {
        let id = request.id();
        match request {
            Request::Hello { .. } => proto::success(Success::Hello {
                id,
                ok: Yes,
                version: PROTOCOL_VERSION,
                store: self.store_state(),
            }),

            Request::Status { .. } => {
                let (state, expires_in) = self.session.state();
                proto::success(Success::Status {
                    id,
                    ok: Yes,
                    state,
                    expires_in,
                })
            }

            Request::Unlock { passphrase, .. } => match self.session.unlock(&passphrase) {
                Ok(expires_in) => proto::success(Success::Unlock {
                    id,
                    ok: Yes,
                    state: LockState::Unlocked,
                    expires_in,
                }),
                Err(error) => proto::failure(id, ErrorCode::Locked, error.to_string()),
            },

            Request::Lock { .. } => {
                self.session.lock();
                proto::success(Success::Lock {
                    id,
                    ok: Yes,
                    state: LockState::Locked,
                })
            }

            Request::List { .. } => match self.store.list("") {
                Ok(entries) => proto::success(Success::List {
                    id,
                    ok: Yes,
                    entries,
                }),
                Err(error) => proto::failure(id, code_for(&error), error.to_string()),
            },

            Request::Search { origin, .. } => self.search(id, &origin),

            Request::Get { entry: name, .. } => match self.store.show(&name) {
                Ok(body) => {
                    let fields = entry::parse(&String::from_utf8_lossy(&body));
                    proto::success(Success::Get {
                        id,
                        ok: Yes,
                        entry: Secret {
                            name,
                            password: fields.password,
                            username: fields.username,
                            url: fields.url,
                            totp: fields.totp,
                        },
                    })
                }
                Err(error) => proto::failure(id, code_for(&error), error.to_string()),
            },

            Request::Generate {
                length, symbols, ..
            } => match generate::password(length, &generate::charset(!symbols)) {
                Ok(password) => proto::success(Success::Generate {
                    id,
                    ok: Yes,
                    password,
                }),
                Err(error) => proto::failure(id, ErrorCode::Internal, error.to_string()),
            },

            Request::Insert {
                entry,
                password,
                username,
                url,
                ..
            } => self.insert(id, entry, &password, username.as_deref(), url.as_deref()),
        }
    }

    /// Create one entry, and only ever create.
    ///
    /// Encryption here needs the recipients' public keys and nothing else, so
    /// no secret of the user's passes through the browser to make this happen
    /// — which is why it can be allowed at all (ADR-0006). What it does hand
    /// the browser is the ability to add to the store, so the two things that
    /// bound the damage are enforced here and nowhere else:
    ///
    /// - the store must be **unlocked**, which is a human having typed the
    ///   passphrase into this machine within the lease;
    /// - the name must be **free**. An overwrite would let a compromised
    ///   extension replace a login with one it knows, which is the attack this
    ///   verb would otherwise be worth mounting.
    fn insert(
        &self,
        id: u32,
        name: String,
        password: &str,
        username: Option<&str>,
        url: Option<&str>,
    ) -> Value {
        if self.store_state() == StoreState::Missing {
            return proto::failure(
                id,
                ErrorCode::StoreMissing,
                "there is no store to add an entry to",
            );
        }

        if self.session.state().0 == LockState::Locked {
            return proto::failure(
                id,
                ErrorCode::Locked,
                "the store is locked; unlock before adding an entry",
            );
        }

        // Checked before writing rather than relying on the store, which
        // would happily replace the file.
        if self.store.entry_exists(&name) {
            return proto::failure(
                id,
                ErrorCode::Exists,
                format!("{name} is already in the store"),
            );
        }

        let body = entry::render(password, username, url);
        match self.store.insert(&name, body.as_bytes()) {
            Ok(()) => proto::success(Success::Insert {
                id,
                ok: Yes,
                entry: name,
            }),
            Err(error) => proto::failure(id, code_for(&error), error.to_string()),
        }
    }

    fn store_state(&self) -> StoreState {
        if self.store.root().is_dir() {
            StoreState::Ready
        } else {
            StoreState::Missing
        }
    }

    /// Entries that may be offered on `origin`.
    ///
    /// Names are matched first, which is both the `web/google.com` convention
    /// and the only thing that works on a locked store. A match is then
    /// decrypted to fill in the username the popup wants to show; if that
    /// fails the row is still offered, because knowing an entry exists is not
    /// a secret the way its password is.
    fn search(&self, id: u32, origin: &str) -> Value {
        let Some(page_host) = origin::host_of(origin) else {
            return proto::success(Success::Search {
                id,
                ok: Yes,
                matches: Vec::new(),
            });
        };

        let names = match self.store.list("") {
            Ok(names) => names,
            Err(error) => return proto::failure(id, code_for(&error), error.to_string()),
        };

        let matches = names
            .into_iter()
            .filter(|name| {
                origin::matches(
                    &Candidate {
                        name: name.clone(),
                        url: None,
                    },
                    &page_host,
                )
            })
            .map(|name| {
                let fields = self
                    .store
                    .show(&name)
                    .ok()
                    .map(|body| entry::parse(&String::from_utf8_lossy(&body)));
                Match {
                    name,
                    username: fields.as_ref().and_then(|f| f.username.clone()),
                    url: fields.and_then(|f| f.url),
                }
            })
            .collect();

        proto::success(Success::Search {
            id,
            ok: Yes,
            matches,
        })
    }
}

/// Map a core failure onto something the extension can branch on.
fn code_for(error: &CoreError) -> ErrorCode {
    match error {
        CoreError::NotInStore(_) => ErrorCode::NotFound,
        CoreError::SneakyPath => ErrorCode::BadRequest,
        CoreError::EmptyStore | CoreError::NoRecipients | CoreError::NoIdentity(_) => {
            ErrorCode::StoreMissing
        }
        CoreError::Locked | CoreError::AuthFailed(_) | CoreError::AuthUnavailable(_) => {
            ErrorCode::Locked
        }
        _ => ErrorCode::Internal,
    }
}
