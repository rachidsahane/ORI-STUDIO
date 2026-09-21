//! The JSON-RPC surface clients speak: AICD §28.
//!
//! Owns `Server`, the method registry, `Transport` (uds, named pipe, websocket)
//! and `Subscriptions` (`spec/LLD.md` section 2). The methods themselves are
//! `spec/API_SPEC.md`.
//!
//! Must not: contain logic beyond validation and dispatch
//! (`spec/LLD.md` section 2).
