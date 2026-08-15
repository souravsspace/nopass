//! Turning a request into a reply.

use nopass_core::fields;
use nopass_core::record::Record;
use nopass_core::{generate, Error as CoreError, Store};
use serde_json::Value;

use crate::entry;
use crate::origin::{self, Candidate};
use crate::proto::{
    self, ErrorCode, Field, Item, Kind, LockState, Match, Request, StoreState, Success, Yes,
    KNOWN_VERBS, MUTATING_VERBS, PROTOCOL_VERSION,
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

            Request::Get { entry: name, .. } => match self.read(&name) {
                Ok(record) => proto::success(Success::Get {
                    id,
                    ok: Yes,
                    entry: entry::to_secret(name, &record),
                }),
                Err(error) => proto::failure(id, code_for(&error), error.to_string()),
            },

            Request::Items { kinds, .. } => self.items(id, kinds.as_deref()),

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
                kind,
                secret,
                fields,
                ..
            } => self.insert(
                id,
                entry,
                kind.unwrap_or(Kind::Login),
                secret.as_deref(),
                fields.as_deref().unwrap_or_default(),
            ),

            Request::Update {
                entry,
                secret,
                fields,
                ..
            } => self.update(
                id,
                entry,
                secret.as_deref(),
                fields.as_deref().unwrap_or_default(),
            ),
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
        kind: Kind,
        secret: Option<&str>,
        fields: &[Field],
    ) -> Value {
        if let Some(refusal) = self.refuse_a_write(id, "adding an entry") {
            return refusal;
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

        let mut record = Record::new(kind.as_record_kind());
        if let Err(error) = entry::apply(&mut record, secret, fields) {
            return proto::failure(id, ErrorCode::BadRequest, error.to_string());
        }

        let body = record.render();
        match self.store.insert(&name, body.as_bytes()) {
            Ok(()) => proto::success(Success::Insert {
                id,
                ok: Yes,
                entry: name,
            }),
            Err(error) => proto::failure(id, code_for(&error), error.to_string()),
        }
    }

    /// Rewrite an entry that is already there, field by field.
    ///
    /// The bound that makes this safe is not the one on `insert`: this verb
    /// exists to overwrite. What holds instead is that it cannot create, so a
    /// name is never claimed by surprise; it cannot remove an entry, only
    /// fields of one; and it never rewrites a line it was not told about, so
    /// a field a newer nopass wrote survives an edit made from an older popup
    /// (ADR-0009). The user confirms the replacement in the UI, which the
    /// host cannot check and does not pretend to.
    fn update(&self, id: u32, name: String, secret: Option<&str>, fields: &[Field]) -> Value {
        if let Some(refusal) = self.refuse_a_write(id, "changing an entry") {
            return refusal;
        }

        if !self.store.entry_exists(&name) {
            return proto::failure(
                id,
                ErrorCode::NotFound,
                format!("{name} is not in the store; `insert` creates one"),
            );
        }

        let mut record = match self.read(&name) {
            Ok(record) => record,
            Err(error) => return proto::failure(id, code_for(&error), error.to_string()),
        };
        if let Err(error) = entry::apply(&mut record, secret, fields) {
            return proto::failure(id, ErrorCode::BadRequest, error.to_string());
        }

        match self.store.insert(&name, record.render().as_bytes()) {
            Ok(()) => proto::success(Success::Update {
                id,
                ok: Yes,
                entry: name,
            }),
            Err(error) => proto::failure(id, code_for(&error), error.to_string()),
        }
    }

    /// Every entry of the kinds asked for, as a row and never as a secret.
    ///
    /// This is how a card reaches a checkout page: cards and identities are
    /// not offered by origin, because they belong to no site. Each row carries
    /// what it takes to choose between them — a masked tail, a name — and
    /// nothing that would matter if it were read.
    fn items(&self, id: u32, kinds: Option<&[Kind]>) -> Value {
        let names = match self.store.list("") {
            Ok(names) => names,
            Err(error) => return proto::failure(id, code_for(&error), error.to_string()),
        };

        let items = names
            .into_iter()
            .filter_map(|name| {
                let record = self.read(&name).ok()?;
                let kind = Kind::from_record_kind(&record.kind());
                if kinds.is_some_and(|wanted| !wanted.contains(&kind)) {
                    return None;
                }
                Some(Item {
                    hint: entry::hint(kind, &record),
                    kind,
                    name,
                })
            })
            .collect();

        proto::success(Success::Items { id, ok: Yes, items })
    }

    /// Read and parse one entry.
    fn read(&self, name: &str) -> nopass_core::Result<Record> {
        let body = self.store.show(name)?;
        Ok(Record::parse(&String::from_utf8_lossy(&body)))
    }

    /// What every write has in common: there has to be a store, and a human
    /// has to have unlocked it inside the current lease.
    fn refuse_a_write(&self, id: u32, doing: &str) -> Option<Value> {
        if self.store_state() == StoreState::Missing {
            return Some(proto::failure(
                id,
                ErrorCode::StoreMissing,
                "there is no store to write to",
            ));
        }
        if self.session.state().0 == LockState::Locked {
            return Some(proto::failure(
                id,
                ErrorCode::Locked,
                format!("the store is locked; unlock before {doing}"),
            ));
        }
        None
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
    /// and the only thing that works on a locked store. The body is then read
    /// for the username the popup shows, and for the `url:` line, which is
    /// matched too: an entry filed under a name that says nothing about the
    /// host — `work/mail`, with `url: https://mail.google.com` — is the case
    /// the name rule alone silently drops, and the popup already tells users
    /// that line is what makes a fill happen. A body that will not decrypt
    /// leaves the row offered on its name, because knowing an entry exists is
    /// not a secret the way its password is.
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
            .filter_map(|name| {
                let by_name = origin::matches(
                    &Candidate {
                        name: name.clone(),
                        url: None,
                    },
                    &page_host,
                );
                let record = self.read(&name).ok();
                let kind = record
                    .as_ref()
                    .map_or(Kind::Login, |record| Kind::from_record_kind(&record.kind()));

                // A card and an identity belong to no site, so no origin can
                // claim them. They are offered through `items`, on a field
                // that asked for one.
                if matches!(kind, Kind::Card | Kind::Identity) {
                    return None;
                }

                let login = record.as_ref().map(fields::login);
                let url = login.as_ref().and_then(|login| login.url.clone());

                let by_url = url.is_some()
                    && origin::matches(
                        &Candidate {
                            name: name.clone(),
                            url: url.clone(),
                        },
                        &page_host,
                    );
                if !(by_name || by_url) {
                    return None;
                }

                Some(Match {
                    name,
                    kind,
                    username: login.and_then(|login| login.username),
                    url,
                })
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
