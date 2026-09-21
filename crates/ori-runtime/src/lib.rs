//! Agent sessions and isolation: AICD §7, AICD §12.
//!
//! Owns `Session`, `Worktree`, `Container`, `AcpClient`, the `HeadlessAdapter`
//! trait and its implementations, the `Budget` meter, `Transcript` and `Injector`
//! (`spec/LLD.md` section 2). This is the only crate that spawns processes.
//!
//! Must not: merge, write `spec/` or `ops/`, or hold credentials beyond a session
//! (`spec/LLD.md` section 2).
