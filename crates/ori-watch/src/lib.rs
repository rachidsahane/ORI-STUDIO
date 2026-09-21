//! The working tree and repository watcher: AICD §13, AICD §23.
//!
//! Owns `TreeWatcher`, the `GitHooks` installer, `Attribution` and
//! `UnattributedChange` (`spec/LLD.md` section 2). This is the only crate that
//! reads the working tree outside a session.
//!
//! AICD §13 makes git the spine and the audit trail, so every change enters
//! through a branch, a commit naming its ticket and specification section, and
//! a pull request. This crate watches the tree and attributes what it finds; a
//! change it cannot attribute is the manual edit the inherited golden rule of
//! AICD §23 forbids, because it puts the system outside what the agents know.
//!
//! Must not: modify the tree (`spec/LLD.md` section 2).
