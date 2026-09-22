//! Gate 12, the significance labeler: AICD §15.
//!
//! # What AICD §15 says, verbatim
//!
//! "A significant modification is any merged change that alters a user
//! journey, a business rule, a data model, a contract, a permission, an
//! integration, or a non-functional characteristic. Changes to copy, styling,
//! layout, logs, documentation or dependency patch versions are not
//! significant. The lead agent marks the significance on each merged pull
//! request from the ticket category and the affected modules, and the QA
//! agent uses that mark as its trigger. The list of what counts as
//! significant is part of organizational knowledge and can be extended per
//! product."
//!
//! Criterion ORI-P1-027 (`spec/criteria/phase-1.md`, precondition "Ticket
//! Merged"): "`significant` set only if declared scope or category matches
//! the significant list; copy-only change is not significant". That sentence
//! is this module's contract; AICD §15's paragraph is read alongside it
//! because it is the only place "the significant list" is defined at all.
//!
//! # The policy does not exist, and this module says so rather than guessing
//! silently
//!
//! `spec/PRD.md` Q-03 calls the significant-modification list "organizational
//! policy, extensible per project, calibrated backwards from production
//! defects". `ops/phase-1-backlog.md` schedules that document,
//! `policies/significant-modification.md`, as ORI-T-0010, "in the
//! organizational repository" (AICD §34). That repository does not exist:
//! `ops/phase-1-backlog.md` states plainly, "ORI-T-0010 is held: its
//! precondition, the organizational repository, does not exist", and records
//! the same fact as an open `precondition_missing` escalation. This module's
//! own precondition, ORI-P1-027's "the significant list", is therefore
//! unwritten anywhere in this repository or any repository it can reach. This
//! is the `precondition_missing` shape CLAUDE.md's escalation trigger list
//! names ("A precondition your ticket names does not exist"): the belief that
//! a significant list exists to consult is checked, against
//! `ops/phase-1-backlog.md`, before anything is built on it, and it turns out
//! false, in a way already recorded rather than newly discovered here.
//!
//! What stands in for the missing policy, until ORI-T-0010 lands, is AICD
//! §15's own paragraph (quoted above, the one place a list is stated at all,
//! by the governing methodology text rather than by this crate) plus
//! `spec/RISK_MAP.md`, which is this repository's own per-module tiering and
//! is maintained with the code rather than filed away in a repository that
//! does not exist. `RISK_MAP_TIER_TWO` restates its tier 2 rows and the test
//! `tests::ori_t_0044_the_restated_risk_map_rows_agree_with_a_fresh_read_of_the_file`
//! checks the restatement against a fresh read of `spec/RISK_MAP.md` on every
//! run, so a row `spec/RISK_MAP.md` adds, removes or reworded and this module
//! has not caught up with fails loudly rather than silently drifting. A
//! hard-coded list with no such check would be a policy nobody approved,
//! asserted here and nowhere else; deriving it from a file this repository
//! already maintains and already checks against its own tests is the
//! defensible, checkable middle path between that and pushing the whole
//! question to an unwritten caller. What this module cannot do is invent the
//! organizational judgement calls AICD §15's prose leaves to "organizational
//! knowledge" and Q-03 leaves to a policy document: whether an ordinary
//! tier 1 feature that plainly "alters a user journey" is significant is
//! exactly that kind of call, and it is not decided here. See "What this
//! module does not decide" below.
//!
//! # The two signals, and why they are read straight off `Ticket`
//!
//! ORI-P1-027 names exactly two: declared scope and category. `Ticket` in
//! `ori_core::types` carries both as mandatory fields (`declared_scope`,
//! `category`), so a caller holding a real, merged `Ticket` always has both;
//! [`crate::significance::SignificanceInput::from_merged_ticket`] is that caller's entry point.
//! [`crate::significance::SignificanceInput::new`] exists for a second, poorer caller this ticket
//! does not build: the wiring gate 12 needs (see "What is not built here"
//! below) reads a merged pull request through "the repository API"
//! (`scripts/gates.sh`'s own words for gate 12), not through this engine's
//! typed `Ticket`, and a webhook payload or a PR label list can legitimately
//! fail to carry one of the two facts even though the domain type never
//! leaves either blank. Nothing about this ticket's declared scope requires
//! that reader; only the type it would need to hand this module.
//!
//! `Ticket.tier` is not read. `spec/RISK_MAP.md` and `Tier::Two` cover much of
//! the same ground as AICD §15's list, but ORI-P1-027 names declared scope and
//! category, not tier, and AICD §15 assigns the mark to "the ticket category
//! and the affected modules" in the same words. Reading the tier instead would
//! substitute a third, human-set field the criterion does not mention for the
//! two it does.
//!
//! # Why the answer is `Result<Verdict, SignificanceError>` and not
//! `Option<bool>`
//!
//! `Ticket.significant` is `Option<bool>` rather than `bool` precisely so that
//! "not yet labelled" cannot collapse into "labelled not significant" (its own
//! doc comment in `ori_core::types` says so, and criterion ORI-P1-027 is what
//! it names as the reason). Giving this module's own answer the same shape,
//! `Option<bool>`, would recreate that exact ambiguity one layer up: a `None`
//! here would have to mean both "I have not been asked" (never happens; this
//! function is always asked) and "I was asked and could not tell", which is
//! indistinguishable from a caller that never checked the return value at all.
//! A `bool` alone is worse: it forces a default direction the moment
//! information is missing, and that default can only ever be `false`, which
//! is the one direction AICD §15 exists to keep a labeller from taking
//! silently (see planted defect 6 below). `Result<Verdict, SignificanceError>`
//! keeps the two apart
//! structurally: `Ok` is a decided answer, richer than a bare `bool` because a
//! human reading a pull request wants to know *why* (which module, or which
//! category), and `Err` is a refusal that carries a [`ori_core::error::MethodologyRef`] like
//! every other refusal this codebase writes (CLAUDE.md rule 9), because "I
//! could not tell" is exactly the case AICD §15 exists to keep from being
//! silently treated as "not significant" (see planted defect 6 below).
//! [`crate::significance::Verdict::is_significant`] is the one call site away from the `bool` the
//! `Ticket.significant` field, and the `Merged { significant: bool }` event
//! `crates/ori-core/src/ticket.rs` already defines and tests
//! (`ori_core::ticket::tests::ori_t_0020_the_merge_event_is_what_sets_the_significance_label`), still
//! need; this module does not touch that event or that field, and its
//! `Result` does not contradict either.
//!
//! # What "copy-only" means mechanically
//!
//! `spec/RISK_MAP.md`'s own tier 0 row is `docs, README, copy | 0`.
//! [`crate::significance::is_copy_path`] reads a declared-scope module as a copy path when it is
//! `docs` or sits under it, or when its last `/`-separated segment is exactly
//! `README.md`; [`crate::significance::is_copy_only`] reads a declared scope as copy-only when it
//! declares at least one module and every module it declares is a copy path.
//! An empty declared scope is not copy-only by this definition: it is the
//! separate, decided case of a ticket that declared no modules at all
//! (`Scope::is_empty`, which `crates/ori-core/src/types.rs`'s own test
//! `ori_core::types::tests::ori_t_0019_an_empty_scope_overlaps_nothing` treats as a legitimate value,
//! not an error), and treating "nothing declared" and "only copy declared" as
//! the same thing would blur a reader's ability to tell "this ticket touched
//! nothing" from "this ticket touched only prose".
//!
//! The third word of the tier 0 row, "copy" on its own (product or UI text,
//! as opposed to the `docs` directory and `README` files), has no
//! corresponding path in this repository today: `apps/desktop/ui` carries no
//! strings or i18n file this module could name, and inventing one would be
//! guessing at a convention nobody has written yet. That third word is
//! therefore not resolved here; it is recorded as a gap this module owes,
//! alongside the three `spec/RISK_MAP.md` tier 2 rows `RISK_MAP_TIER_TWO`
//! leaves `Resolution::Unresolved` for the matching reason.
//!
//! Mechanically, a copy-only declared scope never drives [`crate::significance::label`] toward
//! [`crate::significance::Verdict::Significant`] on its own: `RISK_MAP_TIER_TWO` and
//! `SPEC_ESCALATED_TO_TIER_TWO` are the only sources of a scope match, and
//! the test
//! `tests::ori_t_0044_no_significant_module_prefix_is_itself_a_copy_path`
//! checks, structurally, that none of their entries is a copy path, so a copy
//! path can never accidentally be read as one of them. A ticket whose
//! *category* independently matches the significant list is still significant
//! even when its declared scope happens to be copy-only; scope and category
//! are the two independent signals ORI-P1-027 joins with "or", and a copy-only
//! scope is the shape scope takes when it contributes nothing, not a veto over
//! the other signal. [`crate::significance::Verdict::NotSignificant`] carries `copy_only` so a
//! reader (and a test) can tell which of the two reasons a "not significant"
//! answer rests on without re-deriving it.
//!
//! # Whether this is a gate that refuses or a labeller that annotates
//!
//! A labeller. `spec/CI_CD.md` section 1 item 12 reads "Significance labeler
//! (runs on merge; sets `significant`)", and `scripts/gates.sh`'s `gate_12`
//! reports "not available" with the reason "runs on merge and sets the
//! significant label through the repository API; there is no merge and no
//! label here", which is a different shape from the other thirteen gates: it
//! does not run against a candidate change before merge and does not refuse a
//! build. [`crate::significance::label`] does not refuse a merge either; ORI-P1-027's own
//! precondition is "Ticket Merged", so by the time this function has anything
//! to say the merge has already happened. What [`crate::significance::label`] refuses is *setting
//! a label it cannot stand behind*: an indeterminate input
//! ([`crate::significance::SignificanceError::Indeterminate`]) or a ticket that was never merged
//! ([`crate::significance::SignificanceError::NotMerged`]). That refusal borrows this codebase's
//! ordinary refusal shape, a [`ori_core::error::MethodologyRef`]-carrying error, because a
//! labeller that answers "not significant" when it does not actually know is
//! indistinguishable, downstream, from AICD §15's "dangerous direction": "a
//! labeller that never fires looks like a quiet repository, and AICD §15
//! exists so that significant modifications get the heavier path".
//!
//! # What is not built here
//!
//! Three things, named so a reader does not mistake their absence for an
//! oversight.
//!
//! 1. The repository-API call `scripts/gates.sh`'s gate 12 line names: reading
//!    a merged pull request's ticket record and setting a label back onto it.
//!    That is IO, and this module's declared scope is one new file inside
//!    `ori-gates`, which owns no adapter to any repository host
//!    (`spec/LLD.md` section 2's row for `ori-integrations`, not this crate,
//!    owns that). The wiring, when it is built, is: on merge, look up the
//!    ticket, call [`crate::significance::SignificanceInput::from_merged_ticket`], call [`crate::significance::label`],
//!    and on `Ok` apply `TicketEventKind::Merged { significant:
//!    verdict.is_significant() }` (already defined and tested in
//!    `crates/ori-core/src/ticket.rs`) if that event has not already been
//!    applied by the ordinary merge transition, or otherwise push the
//!    computed bool through whatever channel sets the GitHub-visible label
//!    `scripts/gates.sh` refers to; on `Err`, do not silently choose `false`
//!    to unblock the pipeline, which is exactly planted defect 6 below with
//!    the fail-safe removed. This module produces the verdict; it does not
//!    call anything.
//! 2. Wiring a `gate_12` runner into `scripts/gates.sh`. Stated out of scope
//!    by this ticket. `scripts/gates.sh`'s existing `gate_12` line is
//!    correct as written today ("not available", "there is no merge and no
//!    label here") and this module does not contradict it: nothing here runs
//!    on a pull request, so nothing here changes what a coder's local gate
//!    run reports.
//!  3. `GateDef` and `Runner` (`spec/LLD.md` section 2's row for `ori-gates`).
//!     They do not exist yet in this crate (`crates/ori-gates/src/coverage.rs`
//!     records the same absence for gate 4, in the same words), so this
//!     module is a function, not an installed gate, and cites nothing as
//!     protection it does not have.
//!
//! # Tier
//!
//! This file lives at `crates/ori-gates/src/significance.rs`. `spec/RISK_MAP.md`
//! ties tier 2 to two named files of this crate, `prover.rs` and
//! `liveness.rs` ("Gate integrity"), and tier 1 to everything else
//! ("runners"). This file is neither `prover.rs` nor `liveness.rs`, it does
//! not touch either, and the ticket's own backlog entry says 1. Tier 1 stands.
//!
//! Must not: report a gate installed without a proof (`spec/LLD.md` section
//! 2); this module makes that claim about itself, not about `scripts/gates.sh`.

use core::fmt;

use ori_core::error::MethodologyRef;
use ori_core::types::Category;
use ori_core::types::Scope;
use ori_core::types::Ticket;
use ori_core::types::TicketState;

/// One row of `spec/RISK_MAP.md` whose Tier column reads exactly `2`, and how
/// this module resolves its Path column into declared-scope prefixes it can
/// check a real module path against, or the reason it cannot.
///
/// Restated here as code, and checked against a fresh read of the file by the
/// test
/// `tests::ori_t_0044_the_restated_risk_map_rows_agree_with_a_fresh_read_of_the_file`,
/// the same "restate and check the restatement" shape
/// `SPELLED_BY_SECTION_2` uses in `crates/ori-core/src/types.rs` for
/// `spec/DATA_MODEL.md`'s value lists, for the reason that module gives: a
/// crate that read the file directly at call time would be doing IO on every
/// label, and the check that the restatement still agrees with the source
/// belongs in a test, not in the function a caller runs at merge time.
struct RiskMapRow {
    /// The Path column, exactly as `spec/RISK_MAP.md` writes it. Read only by
    /// the cross-check test
    /// (`tests::ori_t_0044_the_restated_risk_map_rows_agree_with_a_fresh_read_of_the_file`),
    /// never by `matching_significant_module`, which is why the plain `cargo
    /// build` of this crate's library target (no `#[cfg(test)]`) sees no
    /// reader for it; `#[allow(dead_code)]` says that is expected rather than
    /// leaving the warning to be rediscovered.
    #[allow(dead_code)]
    text: &'static str,
    /// What this module does with it.
    resolution: Resolution,
}

/// How one `RiskMapRow` is turned into a declared-scope check, or why it is
/// not.
///
/// `Clone` and `Copy` because every field is a `&'static` reference, so
/// matching on `row.resolution` (a field behind a shared `&RiskMapRow`) reads
/// as a copy of a handful of pointers and widths rather than a move out of a
/// borrow, which keeps every match below single-reference instead of the
/// double reference match ergonomics would otherwise bind.
#[derive(Clone, Copy)]
enum Resolution {
    /// Every declared-scope module that is one of these paths, or that sits
    /// in a directory one of these paths names, is on the significant list.
    /// Checked with `under`, the same `/`-bounded rule
    /// `ori_core::types::Scope::overlaps` applies between two whole scopes,
    /// restated as a free function here (see `under`'s own comment for why
    /// it is not `Scope::overlaps` itself).
    Boundary(&'static [&'static str]),
    /// As `Resolution::Boundary`, plus one literal prefix for the row
    /// `spec/RISK_MAP.md` writes with a glob. `ori_core::types::Scope::new`
    /// refuses to store a `*` rather than expand it unexpanded, and its own
    /// doc comment gives the reason: an unexpanded glob would answer
    /// "overlaps" with a confident `false`. The one glob this table carries
    /// is matched with `str::starts_with` here instead, outside `Scope`
    /// entirely, which is a real limitation and not a silent one: a declared
    /// scope broader than the glob (for example the bare directory `scripts`)
    /// is not detected by it. See
    /// `tests::ori_t_0044_the_glob_row_is_matched_by_a_plain_prefix_not_a_scope`.
    BoundaryAndGlob {
        /// The concrete paths.
        boundary: &'static [&'static str],
        /// The glob's fixed part, checked with `starts_with`.
        glob_prefix: &'static str,
    },
    /// The row narrows a crate below a directory or file this module can name
    /// without guessing at a convention that is not written down. Not on the
    /// significant list; a declared scope naming the crate as a whole would
    /// therefore not be read as significant by this row, which is a real gap
    /// and is reported as one rather than resolved by a guess. The reason
    /// string is read only by
    /// `tests::ori_t_0044_three_risk_map_tier_two_rows_are_recorded_as_unresolved_and_not_silently`;
    /// see `RiskMapRow`'s `text` field comment for why that makes it dead
    /// code in a non-test build.
    Unresolved(#[allow(dead_code)] &'static str),
}

/// `spec/RISK_MAP.md`'s tier 2 rows, twelve of them, each with the concrete
/// declared-scope prefixes it resolves to, or the reason it does not.
///
/// The `spec/` row (tier 1, escalating to tier 2 for four named documents) and
/// the `docs, README, copy` row (tier 0) are not here: neither reads `2` in
/// the Tier column, so the test
/// `tests::ori_t_0044_the_restated_risk_map_rows_agree_with_a_fresh_read_of_the_file`
/// counts this array against rows that do and would fail if either were
/// folded in. They are `SPEC_ESCALATED_TO_TIER_TWO` and [`is_copy_path`]
/// respectively.
const RISK_MAP_TIER_TWO: &[RiskMapRow] = &[
    RiskMapRow {
        text: "crates/ori-core (state machines, permission function)",
        // `crates/ori-core/src/phase.rs`'s own doc comment quotes this exact
        // row and names itself and its siblings as the answer: "spec/LLD.md
        // section 2 gives this crate ... 'state machines as pure functions'".
        // `crates/ori-core/src/types.rs`'s doc comment names the four
        // concretely: "the transition functions are ORI-T-0020 (Ticket) and
        // ORI-T-0021 (Document, Phase), and the permission function is
        // ORI-T-0022", which are `ticket.rs`, `document.rs`, `phase.rs` and
        // `permission.rs`. `error.rs` and `types.rs` are what the sibling row
        // "crates/ori-core (other types) | 1" means by "other types".
        resolution: Resolution::Boundary(&[
            "crates/ori-core/src/ticket.rs",
            "crates/ori-core/src/document.rs",
            "crates/ori-core/src/phase.rs",
            "crates/ori-core/src/permission.rs",
        ]),
    },
    RiskMapRow {
        text: "crates/ori-store/src/event_log.rs, migrations/",
        // `event_log.rs` is unambiguous. `migrations/` is read as a sibling
        // of `src/` inside the crate, the conventional placement for a
        // migrations directory next to a crate's source (`spec/LLD.md`'s own
        // tree comment lists "migrations" under `ori-store/` without a
        // second path segment). Neither the file nor the directory exists in
        // this tree yet ("ls crates/ori-store/src" holds only `lib.rs` today,
        // checked while writing this module), which is not a defect in this
        // row: `spec/RISK_MAP.md` tiers paths before their first commit, and
        // a coder's declared scope can name a file that does not exist yet.
        // The sibling-of-src reading is the more likely of two, argued rather
        // than verified, and is named as such in the pull request report.
        resolution: Resolution::Boundary(&[
            "crates/ori-store/src/event_log.rs",
            "crates/ori-store/migrations",
        ]),
    },
    RiskMapRow {
        text: "crates/ori-memory/src/barrier.rs, scope.rs",
        resolution: Resolution::Boundary(&[
            "crates/ori-memory/src/barrier.rs",
            "crates/ori-memory/src/scope.rs",
        ]),
    },
    RiskMapRow {
        text: "crates/ori-broker",
        resolution: Resolution::Boundary(&["crates/ori-broker"]),
    },
    RiskMapRow {
        text: "crates/ori-runtime/src/container.rs, injector.rs",
        resolution: Resolution::Boundary(&[
            "crates/ori-runtime/src/container.rs",
            "crates/ori-runtime/src/injector.rs",
        ]),
    },
    RiskMapRow {
        text: "crates/ori-gates/src/prover.rs, liveness.rs",
        resolution: Resolution::Boundary(&[
            "crates/ori-gates/src/prover.rs",
            "crates/ori-gates/src/liveness.rs",
        ]),
    },
    RiskMapRow {
        text: "crates/ori-orchestrator/src/merge_queue.rs, lifecycle.rs",
        resolution: Resolution::Boundary(&[
            "crates/ori-orchestrator/src/merge_queue.rs",
            "crates/ori-orchestrator/src/lifecycle.rs",
        ]),
    },
    RiskMapRow {
        text: "crates/ori-watch/src/attribution.rs",
        resolution: Resolution::Boundary(&["crates/ori-watch/src/attribution.rs"]),
    },
    RiskMapRow {
        text: "crates/ori-mcp (server tool scopes)",
        // `crates/ori-mcp/src` holds only `lib.rs` today (checked while
        // writing this module); there is no `Server` or `ToolScopes` module
        // yet for "server tool scopes" to resolve to, and guessing a file
        // name ahead of the crate that has not been built would be a
        // fabricated path, the class of error the ticket's own "Verify what
        // I wrote" warns against.
        resolution: Resolution::Unresolved(
            "crates/ori-mcp carries only lib.rs; no file yet holds \"server tool scopes\"",
        ),
    },
    RiskMapRow {
        text: "crates/ori-integrations (merge exposure, webhook verification)",
        resolution: Resolution::Unresolved(
            "crates/ori-integrations carries only lib.rs; no file yet holds \"merge exposure, \
             webhook verification\"",
        ),
    },
    RiskMapRow {
        text: "crates/ori-rpc (auth, transports)",
        resolution: Resolution::Unresolved(
            "crates/ori-rpc carries only lib.rs; no file yet holds \"auth, transports\"",
        ),
    },
    RiskMapRow {
        text: ".github/workflows, scripts/release*",
        resolution: Resolution::BoundaryAndGlob {
            boundary: &[".github/workflows"],
            glob_prefix: "scripts/release",
        },
    },
];

/// `spec/RISK_MAP.md`'s conditional row, `spec/ | 1 (2 for SECURITY_NOTES,
/// ENV_SETUP, RISK_MAP, ADRs)`: base tier 1, tier 2 for exactly the four named
/// documents. Its Tier column is not the literal text `2`, so it is not part
/// of `RISK_MAP_TIER_TWO` and is checked separately by the test
/// `tests::ori_t_0044_the_restated_risk_map_rows_agree_with_a_fresh_read_of_the_file`.
///
/// `ADRs` resolves to the directory `spec/adr`, confirmed to exist and hold
/// `ADR-0001-stack.md` and `ADR-0002-single-operator.md` while writing this
/// module.
const SPEC_ESCALATED_TO_TIER_TWO: &[&str] = &[
    "spec/SECURITY_NOTES.md",
    "spec/ENV_SETUP.md",
    "spec/RISK_MAP.md",
    "spec/adr",
];

/// The exact Path and Tier column text of `spec/RISK_MAP.md`'s conditional
/// `spec/` row, checked against a fresh read of the file alongside
/// `RISK_MAP_TIER_TWO`. Read only by the cross-check test; see
/// `RiskMapRow`'s `text` field comment for why that is dead code in a
/// non-test build of this crate's library target.
#[allow(dead_code)]
const SPEC_ROW_TEXT: &str = "spec/";
/// See `SPEC_ROW_TEXT`.
#[allow(dead_code)]
const SPEC_ROW_TIER_TEXT: &str = "1 (2 for SECURITY_NOTES, ENV_SETUP, RISK_MAP, ADRs)";

/// The exact Path and Tier column text of `spec/RISK_MAP.md`'s tier 0 copy
/// row, checked the same way.
#[allow(dead_code)]
const COPY_ROW_TEXT: &str = "docs, README, copy";
/// See `COPY_ROW_TEXT`.
#[allow(dead_code)]
const COPY_ROW_TIER_TEXT: &str = "0";

/// The directory `spec/RISK_MAP.md`'s copy row names.
const COPY_DIRECTORY: &str = "docs";

/// Whether `path` is `prefix` itself or sits in a directory `prefix` names.
///
/// The same `/`-bounded rule `ori_core::types::Scope::overlaps` applies
/// between two whole scopes (that type's own private `covers` helper),
/// restated here because it is private to that module and because building a
/// one-off `ori_core::types::Scope` for each comparison would trade this
/// five-line, infallible comparison for a fallible `Scope::new` call this
/// module has no good place to send the error from: `spec/CONVENTIONS.md`
/// "Rust" forbids `unwrap`, `expect` and `panic!` outside tests, and every
/// prefix this module owns is a compile-time constant already known not to be
/// empty or to hold a `*`, so the fallibility `Scope::new` carries would have
/// nothing left to report.
fn under(prefix: &str, path: &str) -> bool {
    let path = path.trim().trim_end_matches('/');
    let prefix = prefix.trim().trim_end_matches('/');
    path == prefix
        || (path.len() > prefix.len()
            && path.starts_with(prefix)
            && path.as_bytes()[prefix.len()] == b'/')
}

/// Whether `path` is a copy path: `spec/RISK_MAP.md`'s `docs, README, copy`
/// row, the `docs` and `README` two thirds of it (see the module doc comment
/// for the third, unresolved). `path` is copy when it is `COPY_DIRECTORY` or
/// sits under it, or when its last `/`-separated segment is exactly
/// `README.md`.
#[must_use]
pub fn is_copy_path(path: &str) -> bool {
    let path = path.trim().trim_end_matches('/');
    under(COPY_DIRECTORY, path) || path.rsplit('/').next() == Some("README.md")
}

/// Whether a declared scope is copy-only: it names at least one module, and
/// every module it names is [`is_copy_path`]. An empty scope is not copy-only;
/// see the module doc comment's "What 'copy-only' means mechanically".
#[must_use]
pub fn is_copy_only(scope: &Scope) -> bool {
    !scope.is_empty() && scope.modules().all(is_copy_path)
}

/// The one `Category` this module reads off `spec/RISK_MAP.md` and AICD §15
/// as licensing significance by itself.
///
/// `Category::Decisional`'s own doc comment in `crates/ori-core/src/types.rs`
/// reads "Touches the specification, the architecture, the data model,
/// security, external cost, a contract, or any tier 2 area", which overlaps
/// AICD §15's significant list (a data model, a contract) and names "any
/// tier 2 area" directly, the same area `RISK_MAP_TIER_TWO` restates. The
/// other three categories' doc comments name no such overlap:
/// `Category::Auto` is bounded to "stays inside the current specification",
/// `Category::Behavioral` is "changes observable behavior" with no mention of
/// contract, data model or tier, and `Category::ProductSignal` "never enters
/// the coder queue directly" and so is not expected to reach `Merged` at all.
/// AICD §15's own list is broader than `Decisional` alone, most visibly "a
/// user journey", which `Category::Behavioral` licenses by its own
/// definition; including `Behavioral` here would make nearly every merged
/// behavioral ticket significant regardless of declared scope, which is the
/// organizational judgement call `spec/PRD.md` Q-03's unwritten policy owns
/// and this module does not make on its own. See the module doc comment's
/// "What this module does not decide".
const SIGNIFICANT_CATEGORIES: &[Category] = &[Category::Decisional];

/// Whether `category` is on `SIGNIFICANT_CATEGORIES`.
#[must_use]
fn significant_category(category: Category) -> bool {
    SIGNIFICANT_CATEGORIES.contains(&category)
}

/// The first significant-list entry a declared scope matches, checked against
/// `RISK_MAP_TIER_TWO` and `SPEC_ESCALATED_TO_TIER_TWO`, or `None`.
///
/// Checked in both directions with `under` for `Resolution::Boundary`
/// entries and `SPEC_ESCALATED_TO_TIER_TWO`, the same as
/// `ori_core::types::Scope::overlaps`: a declared-scope module narrower than
/// a significant path (a file inside a significant directory) matches, and so
/// does one broader than it (a directory declared as scope that contains a
/// significant file), because either shape is a plan whose actual diff can
/// reach the significant path.
fn matching_significant_module(scope: &Scope) -> Option<&'static str> {
    for module in scope.modules() {
        for prefix in SPEC_ESCALATED_TO_TIER_TWO.iter().copied() {
            if under(prefix, module) || under(module, prefix) {
                return Some(prefix);
            }
        }
        for row in RISK_MAP_TIER_TWO {
            match row.resolution {
                Resolution::Boundary(prefixes) => {
                    for prefix in prefixes.iter().copied() {
                        if under(prefix, module) || under(module, prefix) {
                            return Some(prefix);
                        }
                    }
                }
                Resolution::BoundaryAndGlob {
                    boundary,
                    glob_prefix,
                } => {
                    for prefix in boundary.iter().copied() {
                        if under(prefix, module) || under(module, prefix) {
                            return Some(prefix);
                        }
                    }
                    if module.starts_with(glob_prefix) {
                        return Some(glob_prefix);
                    }
                }
                Resolution::Unresolved(_) => {}
            }
        }
    }
    None
}

/// Why a declared scope or a category matched the significant list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignificantBecause {
    /// The category was on `SIGNIFICANT_CATEGORIES`.
    Category(Category),
    /// A declared-scope module matched this significant-list prefix.
    Scope(String),
}

impl fmt::Display for SignificantBecause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Category(category) => write!(f, "category {category}"),
            Self::Scope(module) => write!(f, "declared scope matching {module}"),
        }
    }
}

/// Gate 12's answer for one merged ticket: AICD §15, ORI-P1-027.
///
/// A richer type than `bool`, and richer than `Option<bool>`: see the module
/// doc comment's "Why the answer is `Result<Verdict, SignificanceError>` and
/// not `Option<bool>`". [`Verdict::is_significant`] is the one place this
/// reduces to the `bool` that `TicketEventKind::Merged` in
/// `crates/ori-core/src/ticket.rs` carries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// Neither the declared scope nor the category matched the significant
    /// list.
    NotSignificant {
        /// Whether the declared scope, on its own, was copy-only
        /// ([`is_copy_only`]). See the module doc comment's "What
        /// 'copy-only' means mechanically".
        copy_only: bool,
    },
    /// The declared scope or the category matched the significant list.
    Significant(SignificantBecause),
}

impl Verdict {
    /// The `bool` `Ticket.significant` eventually carries.
    #[must_use]
    pub const fn is_significant(&self) -> bool {
        matches!(self, Self::Significant(_))
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSignificant { copy_only: true } => {
                f.write_str("not significant (copy-only declared scope)")
            }
            Self::NotSignificant { copy_only: false } => f.write_str("not significant"),
            Self::Significant(because) => write!(f, "significant ({because})"),
        }
    }
}

/// The two facts ORI-P1-027 checks, read with whatever confidence the caller
/// actually has.
///
/// `None` is not "false" or "empty": it is "unknown", which is why
/// `declared_scope` is `Option<Scope>` and not `Scope` even though `Scope`
/// itself can already represent "declared nothing" ([`Scope::is_empty`]).
/// [`SignificanceInput::from_merged_ticket`] never produces a `None`, because
/// `Ticket` never carries one; [`SignificanceInput::new`] is for a caller
/// that can. See the module doc comment's "The two signals, and why they are
/// read straight off `Ticket`".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignificanceInput {
    category: Option<Category>,
    declared_scope: Option<Scope>,
}

impl SignificanceInput {
    /// Reads the two facts off a ticket already in `TicketState::Merged`,
    /// refusing one that is not: ORI-P1-027's stated precondition.
    ///
    /// # Errors
    ///
    /// [`SignificanceError::NotMerged`] when `ticket.state` is not
    /// `TicketState::Merged`.
    pub fn from_merged_ticket(ticket: &Ticket) -> Result<Self, SignificanceError> {
        if ticket.state != TicketState::Merged {
            return Err(SignificanceError::NotMerged {
                state: ticket.state,
            });
        }
        Ok(Self {
            category: Some(ticket.category),
            declared_scope: Some(ticket.declared_scope.clone()),
        })
    }

    /// Builds an input directly from whatever a less trustworthy reader
    /// recovered, which may be less than [`SignificanceInput::from_merged_ticket`]
    /// ever produces. See the module doc comment's "The two signals, and why
    /// they are read straight off `Ticket`" for who this is for.
    #[must_use]
    pub const fn new(category: Option<Category>, declared_scope: Option<Scope>) -> Self {
        Self {
            category,
            declared_scope,
        }
    }
}

/// Why [`label`] refused to set a value, rather than the value it set.
///
/// `spec/CONVENTIONS.md` "Rust" names `thiserror` as the house convention;
/// ruling R20 in `ops/rulings.md` defers adopting it and states what stands
/// until then, a hand-written `Display` and `std::error::Error`
/// implementation with a comment naming the conversion. This is that
/// implementation; each `Display` arm below becomes a `thiserror` `#[error]`
/// attribute on the matching variant when R20 is revisited.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SignificanceError {
    /// The ticket was not in `TicketState::Merged`, which ORI-P1-027 states
    /// as this criterion's precondition.
    NotMerged {
        /// Where the ticket actually was.
        state: TicketState,
    },
    /// Neither fact resolved to a match, and at least one of the two
    /// ORI-P1-027 names, declared scope or category, was not read at all
    /// ([`SignificanceInput::new`] built with a `None`), so "no match" cannot
    /// be told apart from "unknown, possibly a match". A known fact that
    /// already matches never reaches this refusal: [`label`] returns
    /// [`Verdict::Significant`] as soon as either side proves true, so an
    /// unknown fact is only ever asked to justify a `false`, which it cannot.
    Indeterminate {
        /// Whether the category was read at all.
        category_known: bool,
        /// Whether the declared scope was read at all.
        scope_known: bool,
    },
}

impl SignificanceError {
    /// The methodology section this refusal is made under: AICD §15, which is
    /// what "the significant list" and the labeller both come from, and which
    /// ORI-P1-027 (a criterion, not a methodology section, and so not a valid
    /// `MethodologyRef` value; see `spec/LLD.md` section 4's
    /// `MethodologyRef { section: u8, ... }`) implements.
    ///
    /// Built as a value rather than through `MethodologyRef::at`, which is
    /// fallible, in the shape `RefusalKind::reason` in
    /// `crates/ori-core/src/error.rs` already uses for the same reason.
    #[must_use]
    pub const fn reason(&self) -> MethodologyRef {
        MethodologyRef {
            section: 15,
            subsection: None,
        }
    }
}

impl fmt::Display for SignificanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotMerged { state } => write!(
                f,
                "ORI-P1-027's precondition is a ticket in state \"merged\"; this ticket is in \
                 state \"{state}\" (AICD §15)"
            ),
            Self::Indeterminate {
                category_known,
                scope_known,
            } => write!(
                f,
                "neither the declared scope nor the category matched the significant list, and \
                 category_known={category_known}, scope_known={scope_known}: at least one of \
                 the two facts ORI-P1-027 checks was not read, so \"no match\" cannot be told \
                 apart from \"unknown, possibly a match\", and defaulting to \"not significant\" \
                 here is the direction AICD §15 exists to refuse (AICD §15)"
            ),
        }
    }
}

impl std::error::Error for SignificanceError {}

/// Gate 12: labels one merged ticket significant or not, from its declared
/// scope and its category, or refuses when it cannot tell.
///
/// # Errors
///
/// [`SignificanceError::Indeterminate`] when neither fact resolves to a match
/// and at least one of the two was not read at all. Never
/// [`SignificanceError::NotMerged`]; that refusal belongs to
/// [`SignificanceInput::from_merged_ticket`], which is where a
/// not-yet-merged ticket is turned away before this function is ever called.
#[must_use = "an indeterminate answer must not be treated as not significant"]
pub fn label(input: &SignificanceInput) -> Result<Verdict, SignificanceError> {
    if let Some(category) = input.category
        && significant_category(category)
    {
        return Ok(Verdict::Significant(SignificantBecause::Category(category)));
    }
    if let Some(scope) = &input.declared_scope {
        if let Some(matched) = matching_significant_module(scope) {
            return Ok(Verdict::Significant(SignificantBecause::Scope(
                matched.to_owned(),
            )));
        }
        if input.category.is_some() {
            return Ok(Verdict::NotSignificant {
                copy_only: is_copy_only(scope),
            });
        }
    }
    Err(SignificanceError::Indeterminate {
        category_known: input.category.is_some(),
        scope_known: input.declared_scope.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use ori_core::types::Budget;
    use ori_core::types::Id;
    use ori_core::types::SpecAnchor;
    use ori_core::types::TicketKind;

    use super::*;

    fn id(text: &str) -> Id {
        Id::parse(text).expect("a canonical ULID")
    }

    /// A declared scope of `modules`, built with array arguments (`scope([])`,
    /// `scope(["a", "b"])`) so a call site never has to spell `&[...]`.
    fn scope<const N: usize>(modules: [&str; N]) -> Scope {
        Scope::new(modules).expect("a well formed declared scope")
    }

    /// A merged ticket with `category` and `declared_scope`, everything else
    /// filled with a value [`label`] never reads.
    fn merged(category: Category, declared_scope: Scope) -> Ticket {
        Ticket {
            id: id("01D78XYFJ1PRM1WPBCBT3VHMNV"),
            product_id: id("01F8MECHZX3TBDSZ7XR8H8JHAF"),
            title: "a merged ticket".to_owned(),
            category,
            kind: TicketKind::Feature,
            tier: ori_core::types::Tier::One,
            state: TicketState::Merged,
            spec_anchor: SpecAnchor::parse("CI_CD.md#1-pipeline-on-every-pull-request")
                .expect("an anchor"),
            declared_scope,
            budget: Budget {
                attempts: 3,
                wall_clock_s: 2700,
                tokens: 0,
            },
            filed_by: ori_core::types::Actor::System,
            phase_id: id("01G65Z755AFWAKHE12NY0CQ9FH"),
            significant: None,
            incident_id: None,
        }
    }

    // -----------------------------------------------------------------------
    // ORI-P1-027: the criterion's own clauses.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_027_a_declared_scope_touching_a_tier_2_module_is_significant() {
        let ticket = merged(Category::Auto, scope(["crates/ori-broker/src/keychain.rs"]));
        let input = SignificanceInput::from_merged_ticket(&ticket).expect("a merged ticket");
        let verdict = label(&input).expect("a decided answer");
        assert!(verdict.is_significant());
        assert_eq!(
            verdict,
            Verdict::Significant(SignificantBecause::Scope("crates/ori-broker".to_owned()))
        );
    }

    #[test]
    fn ori_p1_027_a_declared_scope_broader_than_a_tier_2_file_still_matches() {
        // The declared scope is the directory, not the file: overlap must be
        // checked in both directions, the same as Scope::overlaps.
        let ticket = merged(Category::Auto, scope(["crates/ori-watch"]));
        let input = SignificanceInput::from_merged_ticket(&ticket).expect("a merged ticket");
        let verdict = label(&input).expect("a decided answer");
        assert!(verdict.is_significant());
    }

    #[test]
    fn ori_p1_027_a_decisional_category_is_significant_regardless_of_scope() {
        let ticket = merged(Category::Decisional, scope(["crates/ori-cli/src/main.rs"]));
        let input = SignificanceInput::from_merged_ticket(&ticket).expect("a merged ticket");
        let verdict = label(&input).expect("a decided answer");
        assert_eq!(
            verdict,
            Verdict::Significant(SignificantBecause::Category(Category::Decisional))
        );
    }

    #[test]
    fn ori_p1_027_a_copy_only_change_is_not_significant() {
        let ticket = merged(
            Category::Auto,
            scope(["docs/getting-started.md", "README.md"]),
        );
        let input = SignificanceInput::from_merged_ticket(&ticket).expect("a merged ticket");
        let verdict = label(&input).expect("a decided answer");
        assert_eq!(verdict, Verdict::NotSignificant { copy_only: true });
        assert!(!verdict.is_significant());
    }

    #[test]
    fn ori_p1_027_an_ordinary_tier_one_change_is_not_significant() {
        // Auto category, a module on neither the significant list nor the
        // copy list: the floor that "everything is significant" (planted
        // defect 4) must fail against.
        let ticket = merged(Category::Auto, scope(["crates/ori-cli/src/main.rs"]));
        let input = SignificanceInput::from_merged_ticket(&ticket).expect("a merged ticket");
        let verdict = label(&input).expect("a decided answer");
        assert_eq!(verdict, Verdict::NotSignificant { copy_only: false });
    }

    #[test]
    fn ori_p1_027_a_ticket_not_merged_refuses_a_label() {
        for state in [
            TicketState::InReview,
            TicketState::InProgress,
            TicketState::Deployed,
            TicketState::Closed,
        ] {
            let mut ticket = merged(Category::Auto, scope(["crates/ori-broker"]));
            ticket.state = state;
            let error = SignificanceInput::from_merged_ticket(&ticket)
                .expect_err("a ticket not in state Merged is refused a label");
            assert_eq!(error, SignificanceError::NotMerged { state });
            assert_eq!(error.reason(), MethodologyRef::at(15).expect("AICD §15"));
        }
    }

    #[test]
    fn ori_p1_027_indeterminate_input_refuses_rather_than_defaults_to_not_significant() {
        // Planted defect 6: break the reader so it sees no scope and no
        // category. An empty scope alone (Scope::new([])) is a decided fact,
        // "nothing declared"; this is the stronger case where neither fact
        // was read at all, which is what a repository-API parse failure
        // looks like.
        let input = SignificanceInput::new(None, None);
        let error = label(&input).expect_err("neither fact is known");
        assert_eq!(
            error,
            SignificanceError::Indeterminate {
                category_known: false,
                scope_known: false,
            }
        );
        assert_eq!(error.reason(), MethodologyRef::at(15).expect("AICD §15"));
        assert!(
            error.to_string().contains("unknown, possibly a match"),
            "{error}"
        );
    }

    #[test]
    fn ori_p1_027_a_scope_only_partially_known_still_refuses_rather_than_guessing() {
        // A known non-matching category alone can never justify "not
        // significant" when the scope, the other named signal, was not read:
        // the unread scope could hide a tier 2 module.
        let category_only = SignificanceInput::new(Some(Category::Auto), None);
        assert_eq!(
            label(&category_only).expect_err("scope unread"),
            SignificanceError::Indeterminate {
                category_known: true,
                scope_known: false,
            }
        );

        // Symmetric: a known, non-matching, non-copy-only scope alone cannot
        // justify "not significant" when the category was not read.
        let scope_only = SignificanceInput::new(None, Some(scope(["crates/ori-cli/src/main.rs"])));
        assert_eq!(
            label(&scope_only).expect_err("category unread"),
            SignificanceError::Indeterminate {
                category_known: false,
                scope_known: true,
            }
        );
    }

    #[test]
    fn ori_p1_027_an_unknown_scope_does_not_block_a_category_match() {
        // The OR is safe in the significant direction: an unknown scope never
        // suppresses a category that already proves significance.
        let input = SignificanceInput::new(Some(Category::Decisional), None);
        let verdict = label(&input).expect("category alone decides it");
        assert!(verdict.is_significant());
    }

    #[test]
    fn ori_p1_027_an_empty_declared_scope_with_a_known_category_is_decided_not_indeterminate() {
        // "Declared nothing" is a decided fact, not "unknown".
        let ticket = merged(Category::Auto, scope([]));
        let input = SignificanceInput::from_merged_ticket(&ticket).expect("a merged ticket");
        let verdict = label(&input).expect("declaring nothing is a decided fact");
        assert_eq!(verdict, Verdict::NotSignificant { copy_only: false });
    }

    // -----------------------------------------------------------------------
    // ORI-T-0044: supporting behavior.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0044_the_spec_escalated_documents_are_significant_and_the_rest_of_spec_is_not() {
        for document in SPEC_ESCALATED_TO_TIER_TWO.iter().copied() {
            let ticket = merged(Category::Auto, scope([document]));
            let input = SignificanceInput::from_merged_ticket(&ticket).expect("a merged ticket");
            assert!(
                label(&input).expect("a decided answer").is_significant(),
                "{document} is one of spec/RISK_MAP.md's four escalated documents"
            );
        }

        let ticket = merged(Category::Auto, scope(["spec/PRD.md"]));
        let input = SignificanceInput::from_merged_ticket(&ticket).expect("a merged ticket");
        assert!(
            !label(&input).expect("a decided answer").is_significant(),
            "spec/PRD.md is not one of the four documents spec/RISK_MAP.md escalates"
        );
    }

    #[test]
    fn ori_t_0044_the_glob_row_is_matched_by_a_plain_prefix_not_a_scope() {
        let ticket = merged(Category::Auto, scope(["scripts/release-notes.sh"]));
        let input = SignificanceInput::from_merged_ticket(&ticket).expect("a merged ticket");
        assert!(label(&input).expect("a decided answer").is_significant());

        // The documented limitation: a scope broader than the glob's fixed
        // prefix is not detected by it.
        let ticket = merged(Category::Auto, scope(["scripts"]));
        let input = SignificanceInput::from_merged_ticket(&ticket).expect("a merged ticket");
        assert!(!label(&input).expect("a decided answer").is_significant());
    }

    #[test]
    fn ori_t_0044_no_significant_module_prefix_is_itself_a_copy_path() {
        for prefix in SPEC_ESCALATED_TO_TIER_TWO.iter().copied() {
            assert!(
                !is_copy_path(prefix),
                "{prefix} is on the significant list and must never also read as a copy path"
            );
        }
        for row in RISK_MAP_TIER_TWO {
            let prefixes: &'static [&'static str] = match row.resolution {
                Resolution::Boundary(prefixes) => prefixes,
                Resolution::BoundaryAndGlob { boundary, .. } => boundary,
                Resolution::Unresolved(_) => &[],
            };
            for prefix in prefixes.iter().copied() {
                assert!(!is_copy_path(prefix), "{prefix} ({})", row.text);
            }
        }
    }

    #[test]
    fn ori_t_0044_is_copy_path_reads_docs_and_readme_and_nothing_else_named_in_the_row() {
        assert!(is_copy_path("docs"));
        assert!(is_copy_path("docs/getting-started.md"));
        assert!(is_copy_path("README.md"));
        assert!(is_copy_path("crates/ori-gates/README.md"));
        assert!(!is_copy_path("crates/ori-gates/src/lib.rs"));
        assert!(
            !is_copy_path("docstore/foo.rs"),
            "a directory boundary, not a bare string prefix"
        );
        assert!(
            !is_copy_path("NOTICE"),
            "not named by spec/RISK_MAP.md's copy row"
        );
    }

    #[test]
    fn ori_t_0044_is_copy_only_requires_a_non_empty_scope_of_only_copy_paths() {
        assert!(is_copy_only(&scope(["docs/a.md", "README.md"])));
        assert!(!is_copy_only(&scope(["docs/a.md", "crates/ori-cli/x.rs"])));
        assert!(
            !is_copy_only(&scope([])),
            "declaring nothing is not copy-only"
        );
    }

    #[test]
    fn ori_t_0044_a_verdict_carries_the_reason_a_human_reads_on_the_pull_request() {
        let by_category = Verdict::Significant(SignificantBecause::Category(Category::Decisional));
        assert!(by_category.to_string().contains("category"));
        let by_scope =
            Verdict::Significant(SignificantBecause::Scope("crates/ori-broker".to_owned()));
        assert!(by_scope.to_string().contains("crates/ori-broker"));
        let not_copy = Verdict::NotSignificant { copy_only: true };
        assert!(not_copy.to_string().contains("copy-only"));
    }

    #[test]
    fn ori_t_0044_a_malformed_or_missing_significant_list_entry_is_not_this_modules_shape() {
        // significant_category and matching_significant_module are total: no
        // input value causes a panic. Exercised implicitly by every test
        // above running to completion under a #[test] harness that would
        // report a panic as a failure; recorded here as a named assertion of
        // the property, not a new mechanism.
        for category in Category::ALL {
            let _ = significant_category(*category);
        }
    }

    // -----------------------------------------------------------------------
    // ORI-T-0044: the restatement of spec/RISK_MAP.md, checked against a
    // fresh read of the file.
    // -----------------------------------------------------------------------

    /// The repository root, two levels above `crates/ori-gates`, the same
    /// derivation `crates/ori-gates/src/coverage.rs`'s own `repo_root` uses,
    /// restated here rather than imported because it is `#[cfg(test)]`
    /// private to that module's own test block and widening it would be an
    /// edit to an existing test module (`spec/CONVENTIONS.md` "Tests").
    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("a crate lives two levels below the repository root")
            .to_path_buf()
    }

    /// One row of `spec/RISK_MAP.md`'s table, as read off the file rather
    /// than restated in this module.
    struct ParsedRow {
        path: String,
        tier: String,
    }

    /// Reads every row of `spec/RISK_MAP.md`'s one table, skipping the header
    /// and the separator.
    fn read_risk_map_rows() -> Vec<ParsedRow> {
        let path = repo_root().join("spec").join("RISK_MAP.md");
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("reading {}: {error}", path.display()));
        let mut rows = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if !line.starts_with('|') {
                continue;
            }
            let cells: Vec<&str> = line
                .trim_start_matches('|')
                .trim_end_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            if cells.len() < 2 {
                continue;
            }
            if cells[0] == "Path" || cells[0].chars().all(|c| c == '-') {
                continue;
            }
            rows.push(ParsedRow {
                path: cells[0].to_owned(),
                tier: cells[1].to_owned(),
            });
        }
        rows
    }

    #[test]
    fn ori_t_0044_the_restated_risk_map_rows_agree_with_a_fresh_read_of_the_file() {
        let rows = read_risk_map_rows();
        assert!(
            !rows.is_empty(),
            "spec/RISK_MAP.md yielded no row; the reader is broken or the table moved, and \
             either way this test would otherwise pass vacuously"
        );

        let tier_two: Vec<&ParsedRow> = rows.iter().filter(|row| row.tier == "2").collect();
        assert_eq!(
            tier_two.len(),
            RISK_MAP_TIER_TWO.len(),
            "spec/RISK_MAP.md carries {} rows whose Tier column is exactly \"2\" and this module \
             restates {}; a row was added, removed or reworded on one side and not the other",
            tier_two.len(),
            RISK_MAP_TIER_TWO.len()
        );

        let restated: std::collections::BTreeSet<&str> =
            RISK_MAP_TIER_TWO.iter().map(|row| row.text).collect();
        assert_eq!(
            restated.len(),
            RISK_MAP_TIER_TWO.len(),
            "a row's text is registered twice, so the count above stands for fewer than \
             RISK_MAP_TIER_TWO.len() rows"
        );
        for row in &tier_two {
            assert!(
                restated.contains(row.path.as_str()),
                "spec/RISK_MAP.md's tier 2 row \"{}\" is not restated in RISK_MAP_TIER_TWO",
                row.path
            );
        }
        for text in &restated {
            assert!(
                tier_two.iter().any(|row| row.path == *text),
                "RISK_MAP_TIER_TWO restates \"{text}\" and no row of spec/RISK_MAP.md with Tier \
                 \"2\" carries that Path text; it was reworded, removed, or never matched"
            );
        }

        let spec_row = rows
            .iter()
            .find(|row| row.path == SPEC_ROW_TEXT)
            .unwrap_or_else(|| {
                panic!("spec/RISK_MAP.md carries no row with Path \"{SPEC_ROW_TEXT}\"")
            });
        assert_eq!(spec_row.tier, SPEC_ROW_TIER_TEXT);

        let copy_row = rows
            .iter()
            .find(|row| row.path == COPY_ROW_TEXT)
            .unwrap_or_else(|| {
                panic!("spec/RISK_MAP.md carries no row with Path \"{COPY_ROW_TEXT}\"")
            });
        assert_eq!(copy_row.tier, COPY_ROW_TIER_TEXT);
    }

    #[test]
    fn ori_t_0044_three_risk_map_tier_two_rows_are_recorded_as_unresolved_and_not_silently() {
        let unresolved: Vec<&RiskMapRow> = RISK_MAP_TIER_TWO
            .iter()
            .filter(|row| matches!(row.resolution, Resolution::Unresolved(_)))
            .collect();
        assert_eq!(
            unresolved.len(),
            3,
            "ori-mcp, ori-integrations and ori-rpc's tier 2 rows are the three this module \
             cannot resolve to a concrete path without guessing; a fourth appearing here \
             un-reviewed, or one of these three disappearing, changes what \
             \"the significant list\" actually covers and is worth a human's attention"
        );
        for row in unresolved {
            if let Resolution::Unresolved(reason) = row.resolution {
                assert!(!reason.is_empty(), "{}", row.text);
            }
        }
    }
}
