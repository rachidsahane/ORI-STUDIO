//! Ticket orchestration: AICD §11, AICD §12, AICD §13.
//!
//! Owns `Lifecycle` (validated transitions), `LockTable`, `MergeQueue`,
//! `Escalation` and `ClosingRules` (`spec/LLD.md` section 2). `MergeQueue` is the
//! only caller of `VcsHost::merge`.
//!
//! Must not: call adapters directly except `VcsHost::merge` through `MergeQueue`
//! (`spec/LLD.md` section 2).

pub mod budgets;
pub mod closing;
pub mod escalation;
pub mod lifecycle;
pub mod lock_table;
pub mod merge_queue;
