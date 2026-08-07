//! The nopass native messaging host.
//!
//! The browser launches this binary and exchanges length-prefixed JSON with it
//! over stdio, so the extension can read the store without a listening port
//! and without ever holding an identity (ADR-0001).
//!
//! It is read-only on purpose: nopass requires an interactive prompt for every
//! mutation and a process spawned by the browser has no terminal to run one in
//! (ADR-0002).

pub mod entry;
pub mod frame;
pub mod manifest;
pub mod origin;
pub mod proto;
pub mod session;

mod host;

pub use host::Host;
pub use session::Session;
