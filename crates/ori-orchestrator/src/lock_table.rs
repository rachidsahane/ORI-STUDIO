//! The lock table of modules claimed by in-flight tickets: AICD §12.
//!
//! AICD §12, "Conflict handling at scale", is the whole specification of this
//! module: "AICD does not rely on serialization; it relies on declared scope.
//! Every implementation plan declares the modules it will touch. The lead agent
//! maintains a lock table of modules currently claimed by in-flight tickets and
//! refuses to start a ticket whose declared scope overlaps one already claimed;
//! non-overlapping tickets start in parallel without any ordering decision. A
//! coder that discovers mid-ticket that it must touch an undeclared module stops
//! and re-declares, which is an escalation trigger if the module is claimed."
//!
//! Four sentences, and each one is a decision below: [`LockTable::claim`] is the
//! refusal, the admission and the re-declaration; [`LockTable::release`] is what
//! "in-flight" stops being; [`LockTable::entries`] is the table itself.
//!
//! # What this module owes
//!
//! Criterion ORI-P1-008 in `spec/criteria/phase-1.md`, verbatim: "Two validated
//! tickets with overlapping declared scope | Lead assigns both | Second returns
//! `E_SCOPE_LOCKED`; lock table shows one entry per module; after the first
//! closes, the second starts". `spec/PRD.md` section 4 states the same as F-02,
//! "Lock table of modules claimed by in-flight tickets; refusal of overlapping
//! starts; re-declaration mid-ticket as an escalation trigger". `spec/LLD.md`
//! section 2 puts `LockTable` in this crate, and `ori_core::types::Scope`'s own
//! documentation points here for the table that applies its predicate.
//!
//! Three observables are named there and each is answered by a signature rather
//! than by a convention: the refusal is `E_SCOPE_LOCKED`, which is
//! `RefusalKind::ScopeLocked`'s code and not a string written here; "one entry
//! per module" is the key of the map this type holds, so a module cannot appear
//! twice; and "after the first closes, the second starts" is
//! [`LockTable::release`] followed by a [`LockTable::claim`] that is now
//! admitted.
//!
//! ```mermaid
//! flowchart TB
//!   D[a plan declares its scope] --> Q{does it overlap a module<br/>another ticket holds?}
//!   Q -- yes --> R[refused E_SCOPE_LOCKED<br/>the claimed module is named<br/>nothing at all is recorded]
//!   Q -- no --> A[admitted<br/>one entry per declared module]
//!   A --> E[the same ticket re-declares<br/>the entries are added to]
//!   E --> Q
//!   A --> C[the ticket stops being in flight]
//!   C --> L[released by ticket<br/>every entry it holds, together]
//! ```
//!
//! # What a claim is, and why the table is keyed by module
//!
//! A claim is one module path held by one ticket. The unit is the module and not
//! the ticket because the criterion asks the table to show "one entry per
//! module", and because a map keyed by module makes that a property of the type:
//! two tickets cannot hold one module, whatever the admission rule does. A
//! ticket that declares three modules produces three entries, all naming it.
//!
//! The holder is `ori_core::types::Id`, which is what `Ticket.id` is. The
//! human-readable ticket keys in `ops/lock-table.md` are a different identifier
//! space and are not what this type holds.
//!
//! # Why a value with pure transitions, and not a mutable structure
//!
//! [`LockTable::claim`] and [`LockTable::release`] take `&self` and return a new
//! table. Three reasons, in descending order of weight.
//!
//! A refused claim must leave the table exactly as it was, and a multi-module
//! claim can be refused halfway through its modules. With a value, "record
//! nothing" is what the signature already says: the caller is handed an
//! `Err` and no table. With `&mut self`, not recording the first module while
//! refusing on the third is a thing the author must remember, and forgetting it
//! leaves a module held by a ticket that was never started. That is the control
//! failing closed on a module nobody is working, which is a lock nobody can
//! release.
//!
//! A lock table is derived state. CLAUDE.md's load-bearing facts put the truth
//! in the append-only event log and make every projection derived from it, so
//! this table is a fold over the ticket events that claim and release, and a
//! fold wants `(state, event) -> state`. A structure that mutated in place would
//! have to be rebuilt to be replayed.
//!
//! Every other state machine this workspace owns is a pure transition
//! (`spec/LLD.md` section 2 types them `Ticket::apply(event) -> Result<Ticket>`),
//! and a control that looked different from the machines around it would be read
//! less carefully.
//!
//! This module does no IO. The crate may, and the table that survives restarts
//! is the projection built from the log by whatever calls this; that is not this
//! file.
//!
//! # One ticket, two modules, one of them claimed
//!
//! The whole declaration is refused and nothing is recorded, not the half that
//! would have fitted. AICD §12 refuses "a ticket whose declared scope overlaps
//! one already claimed", and the subject of that sentence is the ticket. A
//! partial grant would start a coder against a plan it did not declare, and the
//! plan is the only thing the lead reviewed.
//!
//! # Whether a ticket may extend a claim it already holds
//!
//! It may, and this is AICD §12's own sentence rather than a liberty taken here:
//! "A coder that discovers mid-ticket that it must touch an undeclared module
//! stops and re-declares, which is an escalation trigger if the module is
//! claimed." Re-declaring is therefore an ordinary call to [`LockTable::claim`]
//! by a ticket that already holds entries. Its own entries are skipped when the
//! overlap is computed, because a ticket cannot conflict with itself, and the
//! new modules are checked against every other holder exactly as a first claim
//! is. When one of them is held elsewhere the call returns `E_SCOPE_LOCKED`,
//! which is the fact the lead escalates on with trigger `scope_conflict`; this
//! module refuses and opens nothing, because opening escalations is not its job.
//!
//! An extension adds and never removes. Re-declaring a smaller scope does not
//! give a module back, because by the time a coder re-declares it has already
//! written to what it declared first, and a lock that can be surrendered by
//! declaring less is the control failing open on a file with uncommitted work in
//! it. Shrinking is [`LockTable::release`], which is by ticket and total.
//!
//! This project's own table is why the rule is written down: `ops/lock-table.md`
//! records an extension granted mid-ticket under ruling R28 and never written
//! into the table, and a reviewer correctly reported the result as an unrecorded
//! scope violation. An extension that is a function call is an extension that
//! cannot be granted and left unrecorded.
//!
//! # Why release is by ticket and never by module
//!
//! A claim ends when the ticket stops being in flight, so the argument is the
//! ticket and every entry it holds goes together. Releasing one module of a
//! ticket that is still running would admit a second coder to a file the first
//! is still editing, which is the one outcome this table exists to prevent, and
//! no caller has a reason to ask for it: `spec/PRD.md` section 4's F-08 orders
//! the kill path "revokes credentials first, terminates second, releases locks
//! third", and the locks there are all of that session's.
//!
//! # Globs are not expressible, and that is on purpose
//!
//! `Scope::new` refuses any entry containing `*`, so no pattern ever reaches
//! this table. `ops/lock-table.md` records claims as `templates/**`,
//! `apps/desktop/**` and six more in that shape. All eight are the trailing
//! `/**` form, which is exactly what a bare directory path already means here:
//! an entry claims the paths under it on `/` boundaries. The conversion is
//! therefore total for every glob that record contains, and lossless. What does
//! not convert is a pattern that is not a whole directory, `scripts/release*`
//! being the shape `spec/RISK_MAP.md` uses for tiers; that one names no
//! directory and this type cannot hold it.

use std::collections::BTreeMap;

use ori_core::error::{Error, RefusalKind, Result};
use ori_core::types::{Id, Scope};

/// The modules claimed by in-flight tickets, and the refusal that keeps two
/// tickets off one module: AICD §12.
///
/// Derived from AICD §12's "Conflict handling at scale", which gives the lead a
/// "lock table of modules currently claimed by in-flight tickets" and has it
/// refuse "a ticket whose declared scope overlaps one already claimed". The
/// module-level doc records which sentence each method answers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LockTable {
    /// Keyed by module path, so one module has one holder by construction,
    /// which is criterion ORI-P1-008's "one entry per module".
    entries: BTreeMap<String, Entry>,
}

/// What the table holds against one module.
///
/// No methodology section applies: this is the storage behind [`LockTable`], not
/// a rule of its own. The `Scope` is the single-module scope of the key, kept so
/// that overlap is decided by `Scope::overlaps` and the path arithmetic behind
/// AICD §12's "overlaps" lives in exactly one place in the workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    ticket: Id,
    module: Scope,
}

/// One row of the table: a module, and the in-flight ticket holding it.
///
/// Derived from AICD §12 by way of criterion ORI-P1-008's "lock table shows one
/// entry per module", which is the observable this type is the shape of. It
/// borrows from the table rather than owning, because a row outliving the table
/// it was read from is a claim that cannot be checked against anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Claim<'a> {
    module: &'a str,
    ticket: &'a Id,
}

impl<'a> Claim<'a> {
    /// The module path claimed.
    ///
    /// No methodology section applies: this reads a field of AICD §12's table
    /// row.
    #[must_use]
    pub const fn module(self) -> &'a str {
        self.module
    }

    /// The in-flight ticket holding it.
    ///
    /// No methodology section applies: this reads a field of AICD §12's table
    /// row.
    #[must_use]
    pub const fn ticket(self) -> &'a Id {
        self.ticket
    }
}

impl LockTable {
    /// An empty table: nothing is in flight, so nothing is claimed.
    ///
    /// No methodology section applies to emptiness itself; what the table is for
    /// is AICD §12.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Claims every module of `scope` for `ticket`, or refuses the whole
    /// declaration: AICD §12.
    ///
    /// AICD §12 has the lead refuse "a ticket whose declared scope overlaps one
    /// already claimed", and this is that refusal. It is also the re-declaration
    /// the same section describes: a ticket that already holds entries does not
    /// conflict with itself, and an admitted call adds to what it holds.
    ///
    /// # Errors
    ///
    /// `RefusalKind::ScopeLocked`, whose code is `E_SCOPE_LOCKED`
    /// (`spec/API_SPEC.md` section 3), naming the already-claimed module and, in
    /// the detail, the ticket holding it and the declared path that reached it.
    /// When several modules conflict the first in sorted order is named, so that
    /// one declaration always produces one message. Nothing is recorded when a
    /// claim is refused, however many of its modules would have fitted.
    ///
    /// Malformed, if a module path already in the table stops being a module
    /// path. Every key came from a `Scope` and cannot, and the alternative to
    /// passing the case on is a panic in a control.
    pub fn claim(&self, ticket: &Id, scope: &Scope) -> Result<Self> {
        // One single-module scope per declared path, built before anything is
        // examined so that the whole declaration is known to be readable before
        // any of it is judged.
        let mut declared: Vec<(&str, Scope)> = Vec::with_capacity(scope.len());
        for module in scope.modules() {
            declared.push((module, Scope::new([module])?));
        }

        // Entries iterate in sorted order, so the module named in the refusal is
        // the first claimed one that conflicts and is the same on every run.
        for (claimed, entry) in &self.entries {
            if entry.ticket == *ticket {
                continue;
            }
            for (path, one) in &declared {
                if entry.module.overlaps(one) {
                    return Err(Error::refused_with(
                        RefusalKind::ScopeLocked {
                            module: claimed.clone(),
                        },
                        format!(
                            "ticket {} holds {claimed}, which ticket {ticket}'s declaration of \
                             {path} overlaps",
                            entry.ticket
                        ),
                    ));
                }
            }
        }

        let mut entries = self.entries.clone();
        for (path, one) in declared {
            entries.insert(
                path.to_owned(),
                Entry {
                    ticket: ticket.clone(),
                    module: one,
                },
            );
        }
        Ok(Self { entries })
    }

    /// Releases every module `ticket` holds: AICD §12.
    ///
    /// The claim in AICD §12 belongs to an "in-flight" ticket, so it ends when
    /// the ticket does and all of it ends at once. Criterion ORI-P1-008's "after
    /// the first closes, the second starts" is this call followed by the claim
    /// that was refused before it.
    ///
    /// Releasing a ticket that holds nothing returns an equal table rather than
    /// an error: `spec/PRD.md` section 4's F-08 has the kill path release locks
    /// unconditionally, and a control that refused to release a ticket with no
    /// claims would fail that path for the tickets that were refused a claim in
    /// the first place.
    #[must_use]
    pub fn release(&self, ticket: &Id) -> Self {
        Self {
            entries: self
                .entries
                .iter()
                .filter(|(_, entry)| entry.ticket != *ticket)
                .map(|(module, entry)| (module.clone(), entry.clone()))
                .collect(),
        }
    }

    /// The table: one entry per module, in sorted path order: AICD §12.
    ///
    /// This is the "lock table shows one entry per module" of criterion
    /// ORI-P1-008. Sorted, because a table a human reviews in a different order
    /// on each read is a table differences hide in.
    pub fn entries(&self) -> impl Iterator<Item = Claim<'_>> {
        self.entries.iter().map(|(module, entry)| Claim {
            module,
            ticket: &entry.ticket,
        })
    }

    /// The ticket claiming `path`, whether it named `path` or a directory above
    /// it: AICD §12.
    ///
    /// Directory semantics, because that is what AICD §12's "overlaps" means
    /// here and what `Scope::claims` implements. Asking about a path no entry
    /// covers answers `None`.
    #[must_use]
    pub fn holder_of(&self, path: &str) -> Option<&Id> {
        self.entries
            .values()
            .find(|entry| entry.module.claims(path))
            .map(|entry| &entry.ticket)
    }

    /// The modules `ticket` holds, in sorted path order: AICD §12.
    ///
    /// The row-per-ticket view of AICD §12's table, which is what a re-declaring
    /// coder and a closing lead both need to see.
    pub fn modules_of<'a>(&'a self, ticket: &'a Id) -> impl Iterator<Item = &'a str> {
        self.entries
            .iter()
            .filter(move |(_, entry)| entry.ticket == *ticket)
            .map(|(module, _)| module.as_str())
    }

    /// Whether `ticket` holds anything at all: AICD §12.
    ///
    /// "In flight" as this table sees it. A ticket that declared an empty scope
    /// holds nothing and is not in flight here, which is the truth about it.
    #[must_use]
    pub fn holds(&self, ticket: &Id) -> bool {
        self.entries.values().any(|entry| entry.ticket == *ticket)
    }

    /// How many modules are claimed.
    ///
    /// No methodology section applies: this counts the rows of AICD §12's table.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is claimed.
    ///
    /// No methodology section applies: this reads the size of AICD §12's table.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ULID that differs from the others by its last symbol, so that a test
    /// reads as the tickets it is about.
    fn ticket(tag: char) -> Id {
        Id::parse(&format!("01ARZ3NDEKTSV4RRFFQ69G5FA{tag}")).expect("a ULID")
    }

    fn scope(modules: &[&str]) -> Scope {
        Scope::new(modules).expect("module paths")
    }

    /// The paths every enumerating test below runs over. Chosen to cover the
    /// shapes `Scope::overlaps` distinguishes: a directory and a file under it,
    /// two levels of nesting, two disjoint trees, and a pair whose shared prefix
    /// ends at a byte that is not `/`.
    const PATHS: [&str; 7] = [
        "crates",
        "crates/ori-core",
        "crates/ori-core/src",
        "crates/ori-core/src/types.rs",
        "crates/ori-corex",
        "crates/ori-orchestrator",
        "docs",
    ];

    #[test]
    fn ori_p1_008_the_second_overlapping_ticket_is_refused_and_starts_once_the_first_closes() {
        let first = ticket('1');
        let second = ticket('2');
        let table = LockTable::new()
            .claim(&first, &scope(&["crates/ori-core/src/types.rs"]))
            .expect("the first ticket claims an unclaimed module");

        // Lead assigns both. The second declares a directory above the file the
        // first holds, which is an overlap.
        let refusal = table
            .claim(&second, &scope(&["crates/ori-core"]))
            .expect_err("the second ticket overlaps the first");
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(12),
            "{refusal}"
        );
        match &refusal {
            Error::Refused { kind, .. } => {
                assert_eq!(kind.code(), Some("E_SCOPE_LOCKED"));
                assert_eq!(
                    *kind,
                    RefusalKind::ScopeLocked {
                        module: "crates/ori-core/src/types.rs".to_owned(),
                    }
                );
            }
            other => panic!("a refusal, not {other:?}"),
        }

        // The lock table shows one entry per module, and the refused claim put
        // nothing in it.
        let rows: Vec<_> = table
            .entries()
            .map(|claim| (claim.module().to_owned(), claim.ticket().clone()))
            .collect();
        assert_eq!(
            rows,
            vec![("crates/ori-core/src/types.rs".to_owned(), first.clone())]
        );

        // After the first closes, the second starts.
        let closed = table.release(&first);
        assert!(!closed.holds(&first));
        let started = closed
            .claim(&second, &scope(&["crates/ori-core"]))
            .expect("the module is free once the first ticket has closed");
        assert_eq!(
            started.holder_of("crates/ori-core/src/types.rs"),
            Some(&second)
        );
    }

    #[test]
    fn ori_p1_008_non_overlapping_tickets_start_in_parallel() {
        let first = ticket('1');
        let second = ticket('2');
        let table = LockTable::new()
            .claim(&first, &scope(&["crates/ori-core"]))
            .expect("an unclaimed module")
            .claim(&second, &scope(&["crates/ori-orchestrator"]))
            .expect("AICD 12: non-overlapping tickets start in parallel");
        assert_eq!(table.len(), 2);
        assert_eq!(table.holder_of("crates/ori-core/src/lib.rs"), Some(&first));
        assert_eq!(
            table.holder_of("crates/ori-orchestrator/src/lib.rs"),
            Some(&second)
        );
    }

    #[test]
    fn ori_p1_008_one_entry_per_module_and_a_module_cannot_be_held_twice() {
        let first = ticket('1');
        let second = ticket('2');
        let table = LockTable::new()
            .claim(
                &first,
                &scope(&["docs", "crates/ori-core", "crates/ori-cli"]),
            )
            .expect("three unclaimed modules");
        let modules: Vec<_> = table.entries().map(Claim::module).collect();
        assert_eq!(modules, vec!["crates/ori-cli", "crates/ori-core", "docs"]);
        assert_eq!(table.len(), 3);

        // The same module, claimed by someone else, is refused rather than
        // written a second time.
        let refusal = table
            .claim(&second, &scope(&["docs"]))
            .expect_err("docs is held");
        assert!(refusal.to_string().contains("docs"), "{refusal}");
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn ori_p1_008_the_refusal_names_the_claimed_module_and_its_holder() {
        let first = ticket('1');
        let second = ticket('2');
        let table = LockTable::new()
            .claim(&first, &scope(&["crates/ori-core"]))
            .expect("an unclaimed module");
        let refusal = table
            .claim(&second, &scope(&["crates/ori-core/src/error.rs"]))
            .expect_err("a file under a claimed directory is claimed");
        let rendered = refusal.to_string();
        assert!(rendered.contains("crates/ori-core"), "{rendered}");
        assert!(rendered.contains(first.as_str()), "{rendered}");
        assert!(rendered.contains("AICD §12"), "{rendered}");
    }

    #[test]
    fn ori_t_0050_a_declaration_with_one_claimed_module_records_none_of_them() {
        let first = ticket('1');
        let second = ticket('2');
        let table = LockTable::new()
            .claim(&first, &scope(&["crates/ori-core"]))
            .expect("an unclaimed module");
        let before = table.clone();
        let refusal = table
            .claim(
                &second,
                &scope(&["docs", "crates/ori-core/src", "templates"]),
            )
            .expect_err("one of the three is held");
        assert_eq!(
            refusal.methodology_ref().map(|reason| reason.section),
            Some(12)
        );
        // The two free modules were not recorded, and the table is untouched.
        assert_eq!(table, before);
        assert_eq!(table.holder_of("docs"), None);
        assert_eq!(table.holder_of("templates"), None);
        assert!(!table.holds(&second));
    }

    #[test]
    fn ori_t_0050_a_ticket_may_extend_a_claim_it_already_holds() {
        // The case ops/lock-table.md records under ruling R28: a claim granted
        // mid-ticket and never written into the table.
        let holder = ticket('1');
        let table = LockTable::new()
            .claim(&holder, &scope(&["fixtures/planted/gate-7"]))
            .expect("an unclaimed module")
            .claim(
                &holder,
                &scope(&["deny.toml", "scripts/secret-scan.sh", "scripts/gates.sh"]),
            )
            .expect("a ticket does not conflict with itself");
        let modules: Vec<_> = table.modules_of(&holder).collect();
        assert_eq!(
            modules,
            vec![
                "deny.toml",
                "fixtures/planted/gate-7",
                "scripts/gates.sh",
                "scripts/secret-scan.sh",
            ]
        );
        assert_eq!(table.len(), 4);
    }

    #[test]
    fn ori_t_0050_extending_onto_a_module_another_ticket_holds_is_refused() {
        let holder = ticket('1');
        let other = ticket('2');
        let table = LockTable::new()
            .claim(&holder, &scope(&["fixtures/planted/gate-7"]))
            .expect("an unclaimed module")
            .claim(&other, &scope(&["scripts/gates.sh"]))
            .expect("a disjoint module");
        let refusal = table
            .claim(&holder, &scope(&["scripts/gates.sh"]))
            .expect_err("AICD 12: re-declaring onto a claimed module is refused");
        match &refusal {
            Error::Refused { kind, .. } => assert_eq!(
                *kind,
                RefusalKind::ScopeLocked {
                    module: "scripts/gates.sh".to_owned(),
                }
            ),
            other => panic!("a refusal, not {other:?}"),
        }
        assert_eq!(table.modules_of(&holder).count(), 1);
    }

    #[test]
    fn ori_t_0050_re_declaring_a_module_it_already_holds_changes_nothing() {
        let holder = ticket('1');
        let table = LockTable::new()
            .claim(&holder, &scope(&["crates/ori-core"]))
            .expect("an unclaimed module");
        let again = table
            .claim(&holder, &scope(&["crates/ori-core"]))
            .expect("its own module");
        assert_eq!(table, again);
    }

    #[test]
    fn ori_t_0050_re_declaring_less_does_not_give_a_module_back() {
        let holder = ticket('1');
        let other = ticket('2');
        let table = LockTable::new()
            .claim(&holder, &scope(&["crates/ori-core", "docs"]))
            .expect("two unclaimed modules")
            .claim(&holder, &scope(&["crates/ori-core"]))
            .expect("a narrower re-declaration");
        assert_eq!(table.modules_of(&holder).count(), 2);
        assert!(
            table.claim(&other, &scope(&["docs"])).is_err(),
            "a shrinking re-declaration must not release docs"
        );
    }

    #[test]
    fn ori_t_0050_release_is_by_ticket_and_takes_every_module_together() {
        let first = ticket('1');
        let second = ticket('2');
        let table = LockTable::new()
            .claim(&first, &scope(&["crates/ori-core", "docs"]))
            .expect("two unclaimed modules")
            .claim(&second, &scope(&["crates/ori-orchestrator"]))
            .expect("a disjoint module");
        let released = table.release(&first);
        assert!(!released.holds(&first));
        assert_eq!(released.holder_of("crates/ori-core"), None);
        assert_eq!(released.holder_of("docs"), None);
        // Nobody else's claim moved.
        assert_eq!(released.holder_of("crates/ori-orchestrator"), Some(&second));
        assert_eq!(released.len(), 1);
    }

    #[test]
    fn ori_t_0050_releasing_a_ticket_that_holds_nothing_is_not_a_refusal() {
        let holder = ticket('1');
        let stranger = ticket('2');
        let table = LockTable::new()
            .claim(&holder, &scope(&["crates/ori-core"]))
            .expect("an unclaimed module");
        assert_eq!(table.release(&stranger), table);
        assert_eq!(LockTable::new().release(&stranger), LockTable::new());
    }

    #[test]
    fn ori_t_0050_declaring_nothing_is_admitted_and_records_nothing() {
        let holder = ticket('1');
        let table = LockTable::new()
            .claim(&holder, &Scope::default())
            .expect("declaring nothing is allowed");
        assert!(table.is_empty());
        assert!(!table.holds(&holder));
    }

    #[test]
    fn ori_t_0050_the_refusal_names_the_first_claimed_module_in_sorted_order() {
        let first = ticket('1');
        let second = ticket('2');
        let table = LockTable::new()
            .claim(&first, &scope(&["docs", "crates/ori-core"]))
            .expect("two unclaimed modules");
        // Both declared modules conflict; the message must be the same one every
        // time, and it is the first claimed module in sorted order.
        for _ in 0..4 {
            let refusal = table
                .claim(&second, &scope(&["docs", "crates/ori-core"]))
                .expect_err("both are held");
            match &refusal {
                Error::Refused { kind, .. } => assert_eq!(
                    *kind,
                    RefusalKind::ScopeLocked {
                        module: "crates/ori-core".to_owned(),
                    }
                ),
                other => panic!("a refusal, not {other:?}"),
            }
        }
    }

    #[test]
    fn ori_t_0050_a_shared_prefix_that_is_not_a_path_boundary_is_not_an_overlap() {
        let first = ticket('1');
        let second = ticket('2');
        let table = LockTable::new()
            .claim(&first, &scope(&["crates/ori-core"]))
            .expect("an unclaimed module")
            .claim(&second, &scope(&["crates/ori-corex"]))
            .expect("a different directory that shares a prefix");
        assert_eq!(table.len(), 2);
        assert_eq!(table.holder_of("crates/ori-corex/src"), Some(&second));
    }

    #[test]
    fn ori_t_0050_admission_agrees_with_scope_overlaps_on_every_pair_of_paths() {
        // The space of pairs over PATHS is finite and small, so it is walked
        // rather than sampled. Both outcomes are required to occur: a table that
        // admitted everything and a table that refused everything would each
        // pass an assertion that only ever saw one of them.
        let first = ticket('1');
        let second = ticket('2');
        let (mut admitted, mut refused) = (0, 0);
        for held in PATHS {
            for declared in PATHS {
                let table = LockTable::new()
                    .claim(&first, &scope(&[held]))
                    .expect("an empty table claims anything");
                let outcome = table.claim(&second, &scope(&[declared]));
                let overlaps = scope(&[held]).overlaps(&scope(&[declared]));
                assert_eq!(
                    outcome.is_err(),
                    overlaps,
                    "holding {held}, declaring {declared}"
                );
                if let Ok(table) = outcome {
                    admitted += 1;
                    assert_eq!(table.len(), 2, "holding {held}, declaring {declared}");
                } else {
                    refused += 1;
                    assert_eq!(table.len(), 1, "holding {held}, declaring {declared}");
                }
            }
        }
        assert_eq!(admitted + refused, PATHS.len() * PATHS.len());
        assert!(admitted > 0, "a table that never admits is not a control");
        assert!(refused > 0, "a table that never refuses is not a control");
    }

    #[test]
    fn ori_t_0050_no_two_tickets_hold_overlapping_modules_after_any_sequence() {
        // Every sequence of three claims over two tickets and PATHS, refusals
        // included and ignored as a lead would ignore them. The invariant is the
        // one AICD 12 asks the table for, checked after each step.
        let tickets = [ticket('1'), ticket('2')];
        let mut sequences = 0;
        for first in 0..tickets.len() * PATHS.len() {
            for second in 0..tickets.len() * PATHS.len() {
                for third in 0..tickets.len() * PATHS.len() {
                    let mut table = LockTable::new();
                    for step in [first, second, third] {
                        let holder = &tickets[step / PATHS.len()];
                        let path = PATHS[step % PATHS.len()];
                        if let Ok(next) = table.claim(holder, &scope(&[path])) {
                            table = next;
                        }
                        assert_no_cross_ticket_overlap(&table);
                    }
                    sequences += 1;
                }
            }
        }
        assert_eq!(sequences, (tickets.len() * PATHS.len()).pow(3));
    }

    fn assert_no_cross_ticket_overlap(table: &LockTable) {
        let rows: Vec<_> = table.entries().collect();
        for (index, row) in rows.iter().enumerate() {
            for other in &rows[index + 1..] {
                assert_ne!(row.module(), other.module(), "a module held twice");
                if row.ticket() != other.ticket() {
                    let overlap = scope(&[row.module()]).overlaps(&scope(&[other.module()]));
                    assert!(
                        !overlap,
                        "{} and {} are held by different tickets and overlap",
                        row.module(),
                        other.module()
                    );
                }
            }
        }
    }

    #[test]
    fn ori_t_0050_a_glob_never_reaches_the_table_and_the_directory_form_does() {
        // ops/lock-table.md records claims as `templates/**`. Scope refuses the
        // pattern, and the directory it means claims the same paths.
        assert!(Scope::new(["templates/**"]).is_err());
        let holder = ticket('1');
        let table = LockTable::new()
            .claim(&holder, &scope(&["templates"]))
            .expect("the directory form");
        assert_eq!(table.holder_of("templates/plan.md"), Some(&holder));
        assert_eq!(table.holder_of("templates"), Some(&holder));
        assert_eq!(table.holder_of("templatesx"), None);
    }

    #[test]
    fn ori_t_0050_holder_of_answers_with_directory_semantics() {
        let holder = ticket('1');
        let table = LockTable::new()
            .claim(&holder, &scope(&["crates/ori-core/src"]))
            .expect("an unclaimed module");
        assert_eq!(table.holder_of("crates/ori-core/src"), Some(&holder));
        assert_eq!(
            table.holder_of("crates/ori-core/src/types.rs"),
            Some(&holder)
        );
        assert_eq!(table.holder_of("crates/ori-core/src/"), Some(&holder));
        assert_eq!(table.holder_of("crates/ori-core"), None);
        assert_eq!(table.holder_of("crates/ori-core/Cargo.toml"), None);
    }

    #[test]
    fn ori_t_0050_an_empty_table_holds_nothing_and_refuses_nothing() {
        let table = LockTable::new();
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
        assert_eq!(table.entries().count(), 0);
        assert_eq!(table.holder_of("crates/ori-core"), None);
        assert_eq!(table, LockTable::default());
        assert!(
            table
                .claim(&ticket('1'), &scope(&["crates/ori-core"]))
                .is_ok(),
            "an empty table refuses nothing"
        );
    }
}
