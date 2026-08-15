//! The nopass native messaging contract, in serde.
//!
//! `packages/protocol/fixtures/messages.json` is the source of truth. These
//! types and the Zod schemas in `packages/protocol` are both tested against
//! it, so neither can drift without the other going red.

use anyhow::{bail, Result};
use nopass_core::record;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// Bumped whenever a change would break an older extension.
pub const PROTOCOL_VERSION: u32 = 2;

/// Bounds on `generate`, so a typo cannot ask the host for a megabyte.
pub const MIN_PASSWORD_LENGTH: usize = 8;
pub const MAX_PASSWORD_LENGTH: usize = 1024;

/// The verbs this host knows about. Anything else is `unsupported_verb`.
pub const KNOWN_VERBS: &[&str] = &[
    "hello", "status", "unlock", "lock", "list", "search", "get", "generate", "items", "insert",
    "update",
];

/// Verbs a caller might reasonably expect but which this host refuses on
/// principle rather than by accident (ADR-0006). Named so the refusal can say
/// *read-only* instead of *never heard of it*.
///
/// `insert` is deliberately not among them: creating an entry needs only the
/// recipients' public keys, so it exposes no secret the browser did not
/// already have. Everything here would rewrite, move or destroy something that
/// is already in the store, which is the part a compromised extension must
/// never reach.
pub const MUTATING_VERBS: &[&str] = &[
    // `update` is how a field is rewritten now, one named field at a time,
    // behind a confirmation (ADR-0009). `edit` stayed a whole-body replace
    // with no such gate, and is still refused under that name.
    "edit",
    "rm",
    "remove",
    "delete",
    "mv",
    "rename",
    "cp",
    "copy",
    "init",
    "generate_into",
];

/// Serialises as `true` and refuses to deserialise from anything else — the
/// serde counterpart of Zod's `z.literal(true)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Yes;

/// Serialises as `false` and refuses to deserialise from anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct No;

macro_rules! literal_bool {
    ($name:ident, $value:literal) => {
        impl Serialize for $name {
            fn serialize<S: Serializer>(
                &self,
                serializer: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                serializer.serialize_bool($value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(
                deserializer: D,
            ) -> std::result::Result<Self, D::Error> {
                if bool::deserialize(deserializer)? == $value {
                    Ok($name)
                } else {
                    Err(serde::de::Error::custom(concat!(
                        "expected ",
                        stringify!($value)
                    )))
                }
            }
        }
    };
}

literal_bool!(Yes, true);
literal_bool!(No, false);

/// What an entry holds. Absent on the wire means [`Kind::Login`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Login,
    Card,
    Identity,
    Passkey,
}

impl Kind {
    /// Whether this kind's first line is a secret. An identity has none.
    fn bears_a_secret(self) -> bool {
        !matches!(self, Kind::Identity)
    }

    pub fn as_record_kind(self) -> record::Kind {
        match self {
            Kind::Login => record::Kind::Login,
            Kind::Card => record::Kind::Card,
            Kind::Identity => record::Kind::Identity,
            Kind::Passkey => record::Kind::Passkey,
        }
    }

    /// A kind read off an entry. A `type:` line this build does not know
    /// cannot be described on the wire, so the entry travels as a login and
    /// keeps its own line untouched on the way back.
    pub fn from_record_kind(kind: &record::Kind) -> Kind {
        match kind {
            record::Kind::Card => Kind::Card,
            record::Kind::Identity => Kind::Identity,
            record::Kind::Passkey => Kind::Passkey,
            _ => Kind::Login,
        }
    }
}

/// One `key: value` line, in the canonical spelling.
///
/// The host resolves an entry's own spelling — `email:`, `user:` — into the
/// canonical key on the way out, and writes back whichever spelling the entry
/// already used, so the extension never needs an alias table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub key: String,
    pub value: String,
}

/// Requests. Two mutating verbs exist: `insert` can only ever create, and
/// `update` can only ever rewrite an entry that is already there. There is
/// still no variant for `rm`, `mv` or `cp`, so nothing on this wire can move
/// or destroy what is in the store (ADR-0006, ADR-0009).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "verb", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Hello {
        id: u32,
        version: u32,
    },
    Status {
        id: u32,
    },
    Unlock {
        id: u32,
        passphrase: String,
    },
    Lock {
        id: u32,
    },
    List {
        id: u32,
    },
    Search {
        id: u32,
        origin: String,
    },
    Get {
        id: u32,
        entry: String,
    },
    Generate {
        id: u32,
        length: usize,
        symbols: bool,
    },
    Items {
        id: u32,
        /// Absent asks for every kind.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kinds: Option<Vec<Kind>>,
    },
    Insert {
        id: u32,
        entry: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<Kind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secret: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fields: Option<Vec<Field>>,
    },
    Update {
        id: u32,
        entry: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secret: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fields: Option<Vec<Field>>,
    },
}

impl Request {
    pub fn id(&self) -> u32 {
        match *self {
            Self::Hello { id, .. }
            | Self::Status { id }
            | Self::Unlock { id, .. }
            | Self::Lock { id }
            | Self::List { id }
            | Self::Search { id, .. }
            | Self::Get { id, .. }
            | Self::Generate { id, .. }
            | Self::Items { id, .. }
            | Self::Insert { id, .. }
            | Self::Update { id, .. } => id,
        }
    }

    /// The rules serde cannot express: non-empty strings and bounded lengths.
    fn validate(&self) -> Result<()> {
        match self {
            Self::Hello { version, .. } if *version == 0 => bail!("version must be positive"),
            Self::Unlock { passphrase, .. } if passphrase.is_empty() => {
                bail!("passphrase must not be empty")
            }
            Self::Search { origin, .. } if origin.is_empty() => bail!("origin must not be empty"),
            Self::Get { entry, .. } if entry.is_empty() => bail!("entry must not be empty"),
            Self::Generate { length, .. }
                if !(MIN_PASSWORD_LENGTH..=MAX_PASSWORD_LENGTH).contains(length) =>
            {
                bail!("length must be between {MIN_PASSWORD_LENGTH} and {MAX_PASSWORD_LENGTH}")
            }
            Self::Insert {
                entry,
                kind,
                secret,
                fields,
                ..
            } => {
                if entry.is_empty() {
                    bail!("entry must not be empty");
                }
                if kind.unwrap_or(Kind::Login).bears_a_secret()
                    && secret.as_deref().unwrap_or_default().is_empty()
                {
                    bail!("this kind of entry needs a secret on its first line");
                }
                check_body(secret.as_deref(), fields.as_deref())
            }
            Self::Update {
                entry,
                secret,
                fields,
                ..
            } => {
                if entry.is_empty() {
                    bail!("entry must not be empty");
                }
                // An update that names nothing would decrypt and rewrite an
                // entry to leave it exactly as it was.
                if secret.is_none() && fields.as_ref().is_none_or(|f| f.is_empty()) {
                    bail!("an update must change something");
                }
                check_body(secret.as_deref(), fields.as_deref())
            }
            _ => Ok(()),
        }
    }
}

/// The rules a body has to keep, whichever verb carries it.
///
/// An entry body is `secret\nkey: value\n…`, so a value carrying a newline
/// could forge a second field — a `url:` line pointing somewhere the user
/// never typed. That is a phishing primitive, not a formatting slip, so it is
/// refused at the edge rather than escaped further in (ADR-0006). A key that
/// is not a bare token could do the same with a colon, and `type:` is the line
/// the kind owns, so a field may not write it.
fn check_body(secret: Option<&str>, fields: Option<&[Field]>) -> Result<()> {
    if secret.is_some_and(|value| value.contains(['\n', '\r'])) {
        bail!("the secret must not contain a line break");
    }
    for field in fields.unwrap_or_default() {
        if !is_field_key(&field.key) {
            bail!("`{}` is not a field key", field.key);
        }
        if field.key == record::TYPE_KEY {
            bail!("the kind is not a field");
        }
        if field.value.contains(['\n', '\r']) {
            bail!("`{}` must not contain a line break", field.key);
        }
    }
    Ok(())
}

fn is_field_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Whether the store exists at all, so the popup can offer `nopass init`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StoreState {
    Ready,
    Missing,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LockState {
    Locked,
    Unlocked,
}

/// A row in the popup or the inline dropdown. Never carries a secret.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Match {
    pub name: String,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// One entry, handed over for a single fill or for the entry screen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Secret {
    pub name: String,
    pub kind: Kind,
    /// The first line: a password, a card number, or empty for an identity.
    pub secret: String,
    pub fields: Vec<Field>,
}

/// A row for an entry that is not offered by origin — a card, an identity.
///
/// `hint` is what the row reads: a masked card tail, a person's name. The host
/// builds it precisely so that offering the row costs no secret.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub name: String,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "verb", rename_all = "snake_case", deny_unknown_fields)]
pub enum Success {
    Hello {
        id: u32,
        ok: Yes,
        version: u32,
        store: StoreState,
    },
    Status {
        id: u32,
        ok: Yes,
        state: LockState,
        #[serde(rename = "expiresIn")]
        expires_in: u64,
    },
    Unlock {
        id: u32,
        ok: Yes,
        state: LockState,
        #[serde(rename = "expiresIn")]
        expires_in: u64,
    },
    Lock {
        id: u32,
        ok: Yes,
        state: LockState,
    },
    List {
        id: u32,
        ok: Yes,
        entries: Vec<String>,
    },
    Search {
        id: u32,
        ok: Yes,
        matches: Vec<Match>,
    },
    Get {
        id: u32,
        ok: Yes,
        entry: Secret,
    },
    Generate {
        id: u32,
        ok: Yes,
        password: String,
    },
    Items {
        id: u32,
        ok: Yes,
        items: Vec<Item>,
    },
    /// The name back and nothing else: the popup asked for this write, so it
    /// already holds everything a fuller reply could tell it.
    Insert {
        id: u32,
        ok: Yes,
        entry: String,
    },
    Update {
        id: u32,
        ok: Yes,
        entry: String,
    },
}

/// Machine-readable failure reasons. The extension branches on the code; the
/// message is for the log, not for the UI.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadRequest,
    /// A name already taken. Distinct from `BadRequest` because the popup
    /// answers it by offering another name rather than by saying "that is
    /// wrong" — and because refusing to overwrite is the whole shape of the
    /// write this host allows (ADR-0006).
    Exists,
    Internal,
    Locked,
    NotFound,
    ReadOnly,
    StoreMissing,
    UnsupportedVerb,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    pub id: u32,
    pub ok: No,
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum Response {
    Success(Success),
    Failure(Failure),
}

/// Parse and validate a request. Both halves matter: serde rejects the wrong
/// shape, [`Request::validate`] rejects the wrong values.
pub fn parse_request(value: &Value) -> Result<Request> {
    let request: Request = serde_json::from_value(value.clone())?;
    request.validate()?;
    Ok(request)
}

/// Parse a response. Used by the host's own tests to prove every reply it
/// emits is on-contract, and by the e2e stub host.
pub fn parse_response(value: &Value) -> Result<Response> {
    Ok(serde_json::from_value(value.clone())?)
}

/// Build a failure reply. Kept here so every refusal has the same shape.
pub fn failure(id: u32, code: ErrorCode, message: impl Into<String>) -> Value {
    serde_json::to_value(Failure {
        id,
        ok: No,
        error: ErrorBody {
            code,
            message: message.into(),
        },
    })
    .expect("a failure always serialises")
}

/// Build a success reply.
pub fn success(value: Success) -> Value {
    serde_json::to_value(value).expect("a success always serialises")
}
