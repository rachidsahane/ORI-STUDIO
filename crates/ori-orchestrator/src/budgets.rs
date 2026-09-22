//! The budget rule and the blocked report it obliges: AICD §12.
//!
//! AICD §12, "Budgets and the blocked report", is one paragraph and this module
//! is that paragraph in code: "Every ticket carries a budget: attempts,
//! wall-clock time, tokens. When any limit is exceeded, the coder stops, writes
//! a blocked report (what it tried, why it believes the approaches fail, what it
//! would try differently, what it needs from a human) and hands off. It never
//! keeps grinding." `spec/PRD.md` section 4 carries the same rule as F-05.
//!
//! # What is here and what is in `ori-runtime`
//!
//! `spec/LLD.md` section 2 gives `ori-runtime` the "`Budget` meter" and gives
//! this crate `Escalation` beside the lifecycle and the lock table;
//! `spec/RISK_MAP.md` tiers "crates/ori-orchestrator (lock table, escalation,
//! budgets)" at 1, and F-05 says budgets are "enforced by the runtime". Read
//! together, the split is between counting and deciding.
//!
//! The meter counts. It is the thing that can see a clock, a token accounting
//! and the end of an attempt, and it is in the crate that spawns the process
//! those numbers come from. This module never reads a clock, never counts an
//! attempt and holds no state: every function here takes the reading as a value
//! and answers a question about it. That is why a reading is a parameter and
//! not a field anywhere below.
//!
//! ```mermaid
//! flowchart LR
//!   RT["ori-runtime: the meter, which counts"] -->|"Consumed, a reading"| A["assess"]
//!   T["the ticket's budget, an Allowance"] --> A
//!   A --> V{"Verdict"}
//!   V -->|MayContinue| RT
//!   V -->|StopAndReport| B["BlockedReport::new"]
//!   B --> M["operational memory (AICD §12)"]
//! ```
//!
//! # Why the allowance is a trait, and what implements it
//!
//! The value is `Budget` in `crates/ori-core/src/types.rs`, whose own doc
//! comment says the meter that spends it lives elsewhere. The alternative to a
//! trait was a second struct with the same three fields, which is a copy that
//! drifts without anything noticing.
//! [`Allowance`] is the trait, and `Budget`
//! implements it here in six lines; if `Budget` is ever reshaped, that impl
//! stops compiling rather than going quietly wrong. The trait is also what lets
//! every rule below be written once over the dimensions instead of three times
//! over three fields, and what a caller holding an allowance from somewhere
//! else implements rather than converts.
//!
//! [`Dimension`] is not a copy of `Budget`'s fields.
//! Both derive from the same sentence of AICD §12, which names three
//! dimensions, and this module carries them as an enum so that every rule below
//! is total over them: a fourth dimension would fail to compile here rather
//! than be silently unchecked.
//!
//! # What a refusal here carries
//!
//! `spec/LLD.md` section 4 and `spec/CONVENTIONS.md` require every refusal to
//! carry a `MethodologyRef`, and
//! [`BlockedReportError::reason`]
//! is that reference. Every refusal below is made under AICD §12, the section
//! that requires the report to exist.
//!
//! Appendix A.5 fixes the report's fields and no refusal cites it as a section:
//! `crates/ori-core/src/error.rs` records that an appendix cannot be written as
//! a `MethodologyRef`, because `spec/LLD.md` section 4 types the section as
//! `u8` and an appendix is a letter. The refusals name the appendix in their
//! message where the field list is the point.
//!
//! No variant of `RefusalKind` in `crates/ori-core/src/error.rs` describes a
//! blocked report with a field missing, and none was borrowed to look tidy.
//! This crate carries its own error, which is what `spec/CONVENTIONS.md` asks
//! for, "`thiserror` enums per crate".
//!
//! Must not: read a clock, spawn anything, or keep a running total
//! (`spec/LLD.md` section 2 puts all three in `ori-runtime`).

use core::fmt;

use ori_core::error::MethodologyRef;
use ori_core::types::Budget;

/// One of the three things a budget is measured in: AICD §12.
///
/// Derived from AICD §12's "Every ticket carries a budget: attempts,
/// wall-clock time, tokens", in the order that sentence names them.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Dimension {
    /// Attempts at the ticket.
    Attempts,
    /// Wall clock seconds spent on it.
    WallClock,
    /// Tokens spent on it.
    Tokens,
}

impl Dimension {
    /// How many dimensions a budget has: AICD §12.
    ///
    /// Three, because AICD §12's sentence names three. Used as the width of the
    /// per-dimension arrays below so that adding a dimension is a compile error
    /// and not a silent gap.
    pub const COUNT: usize = 3;

    /// Every dimension, in the order AICD §12 names them.
    pub const ALL: [Self; Self::COUNT] = [Self::Attempts, Self::WallClock, Self::Tokens];

    /// The name this dimension is written under: AICD §12.
    ///
    /// The spellings are `spec/DATA_MODEL.md` section 2's, which writes the
    /// Ticket row's field as "budget (attempts, wall_clock_s, tokens)"; its
    /// AgentSession row spells the middle one `seconds` in "budget_used
    /// (attempts, seconds, tokens)", and the Ticket spelling is taken because
    /// it is the one `crates/ori-core/src/types.rs` already carries.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Attempts => "attempts",
            Self::WallClock => "wall_clock_s",
            Self::Tokens => "tokens",
        }
    }

    /// Where this dimension sits in a per-dimension array: no AICD section
    /// applies, it is an implementation detail of the arrays below.
    fn index(self) -> usize {
        match self {
            Self::Attempts => 0,
            Self::WallClock => 1,
            Self::Tokens => 2,
        }
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a ticket's budget allows, read one dimension at a time: AICD §12.
///
/// Derived from AICD §12's "Every ticket carries a budget"; the budget is a
/// property of the ticket, so the implementor is the ticket's budget value and
/// not anything in this crate. The module comment above says why this is a
/// trait rather than a struct.
///
/// ```
/// use ori_core::types::Budget;
/// use ori_orchestrator::budgets::{Consumed, Verdict, assess};
///
/// let budget = Budget { attempts: 2, wall_clock_s: 2700, tokens: 200_000 };
///
/// let after_one_failure = Consumed { attempts: 1, wall_clock_s: 320, tokens: 20_500 };
/// assert!(matches!(assess(&budget, after_one_failure), Verdict::MayContinue));
///
/// let after_two_failures = Consumed { attempts: 2, wall_clock_s: 640, tokens: 41_000 };
/// assert!(matches!(assess(&budget, after_two_failures), Verdict::StopAndReport(_)));
/// ```
pub trait Allowance {
    /// What this budget allows in one dimension.
    fn allowed(&self, dimension: Dimension) -> u64;
}

/// The budget a ticket carries, read one dimension at a time: AICD §12.
///
/// The one implementation the workspace needs, on the value
/// `crates/ori-core/src/types.rs` holds and `spec/DATA_MODEL.md` section 2
/// stores as the Ticket row's `budget (attempts, wall_clock_s, tokens)`.
/// Attempts widen from `u32` to the `u64` every dimension is compared in, which
/// loses nothing.
impl Allowance for Budget {
    fn allowed(&self, dimension: Dimension) -> u64 {
        match dimension {
            Dimension::Attempts => u64::from(self.attempts),
            Dimension::WallClock => self.wall_clock_s,
            Dimension::Tokens => self.tokens,
        }
    }
}

/// A reading from the meter: what a session has spent so far: AICD §12.
///
/// Derived from AICD §12's budget sentence and from appendix A.5, whose
/// "Budget consumed" field is "Attempts, time, tokens", the same three. The
/// meter that produces it is `ori-runtime`'s (`spec/LLD.md` section 2); this is
/// the value it hands over, and `spec/DATA_MODEL.md` section 2 is where it is
/// stored, as the AgentSession row's `budget_used`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Consumed {
    /// Attempts made, counted by the runtime as they end.
    pub attempts: u32,
    /// Wall clock seconds spent.
    pub wall_clock_s: u64,
    /// Tokens spent.
    pub tokens: u64,
}

impl Consumed {
    /// A session that has spent nothing: AICD §12.
    ///
    /// The reading a session starts at, named so that a caller need not write
    /// three zeroes and a reader need not check that it did.
    pub const NOTHING: Self = Self {
        attempts: 0,
        wall_clock_s: 0,
        tokens: 0,
    };

    /// What was spent in one dimension: AICD §12.
    ///
    /// The other half of [`Allowance::allowed`], so that a rule over a
    /// dimension can be written once rather than three times.
    pub fn spent(&self, dimension: Dimension) -> u64 {
        match dimension {
            Dimension::Attempts => u64::from(self.attempts),
            Dimension::WallClock => self.wall_clock_s,
            Dimension::Tokens => self.tokens,
        }
    }
}

impl fmt::Display for Consumed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} attempt(s), {}s, {} token(s)",
            self.attempts, self.wall_clock_s, self.tokens
        )
    }
}

/// A budget with nothing left in at least one dimension: AICD §12.
///
/// Derived from AICD §12's "When any limit is exceeded, the coder stops": one
/// dimension is enough, which is why this carries the set of dimensions that
/// are spent rather than a single one. It carries the whole reading with it
/// because appendix A.5's "Budget consumed" field is all three numbers, not
/// only the one that ran out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Exhaustion {
    consumed: Consumed,
    spent_out: [bool; Dimension::COUNT],
}

impl Exhaustion {
    /// The reading this exhaustion was found in: AICD §12.
    ///
    /// Appendix A.5 requires all three numbers in the report, so all three
    /// travel with the verdict rather than being fetched again from a meter
    /// that has moved on.
    pub fn consumed(&self) -> Consumed {
        self.consumed
    }

    /// Whether this dimension is one of the spent ones: AICD §12.
    pub fn includes(&self, dimension: Dimension) -> bool {
        self.spent_out[dimension.index()]
    }

    /// Every dimension with nothing left, in AICD §12's order.
    ///
    /// Two dimensions can run out at once and both are named, because a human
    /// reading the blocked report is owed the difference between a ticket that
    /// ran out of attempts and one that ran out of attempts and time together.
    pub fn dimensions(&self) -> Vec<Dimension> {
        Dimension::ALL
            .into_iter()
            .filter(|dimension| self.includes(*dimension))
            .collect()
    }
}

impl fmt::Display for Exhaustion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self
            .dimensions()
            .into_iter()
            .map(Dimension::as_str)
            .collect();
        write!(
            f,
            "budget spent in {} (consumed {})",
            names.join(" and "),
            self.consumed
        )
    }
}

/// What a reading obliges: AICD §12.
///
/// Derived from AICD §12's two outcomes and no third: either the session is
/// within its budget, or "the coder stops, writes a blocked report ... and
/// hands off. It never keeps grinding." There is deliberately no variant for
/// "over budget but carrying on", because that state is the one the paragraph
/// exists to forbid, and a type that cannot express it cannot be asked to
/// return it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// Every dimension has something left; the session may continue.
    MayContinue,
    /// At least one dimension is spent; the session stops and a blocked report
    /// is written.
    StopAndReport(Exhaustion),
}

/// Whether a reading leaves a session free to continue: AICD §12.
///
/// Derived from AICD §12's "When any limit is exceeded". Two readings of
/// "exceeded" were available and the narrow one is taken: a dimension is spent
/// when what was consumed has reached its allowance, not when it has passed it.
/// Criterion ORI-P1-009 in `spec/criteria/phase-1.md` is what settles it, with
/// a budget of `attempts=2` and an adapter that "fails twice" ending the
/// session Blocked; on the wider reading a second failure would leave the
/// session running and the criterion false. It is also the reading that makes
/// the rule true of the thing being counted: an allowance of two attempts is
/// two attempts, and none is left after the second.
///
/// An allowance of zero in a dimension is therefore spent before anything is
/// spent in it. That is a ticket filed with a budget that permits nothing, and
/// answering "may continue" to it would be answering a question nobody asked.
pub fn assess<A>(allowance: &A, consumed: Consumed) -> Verdict
where
    A: Allowance + ?Sized,
{
    let mut spent_out = [false; Dimension::COUNT];
    for dimension in Dimension::ALL {
        spent_out[dimension.index()] = consumed.spent(dimension) >= allowance.allowed(dimension);
    }

    if spent_out.iter().any(|dimension| *dimension) {
        Verdict::StopAndReport(Exhaustion {
            consumed,
            spent_out,
        })
    } else {
        Verdict::MayContinue
    }
}

/// What is left in one dimension: AICD §12.
///
/// Derived from the same sentence as [`assess`], and saturating at zero because
/// a meter can overshoot its allowance between two readings and a budget with
/// less than nothing left is not a thing a caller can act on differently.
pub fn remaining<A>(allowance: &A, consumed: Consumed, dimension: Dimension) -> u64
where
    A: Allowance + ?Sized,
{
    allowance
        .allowed(dimension)
        .saturating_sub(consumed.spent(dimension))
}

/// Whether a session spent something without finishing an attempt: no AICD
/// section applies.
///
/// This is an observation offered to a caller, not a rule of the methodology,
/// and it refuses nothing. AICD §12's budgets are written for an agent that is
/// failing: each failure ends an attempt, attempts run out, the report gets
/// written. An agent that believes it is making progress ends no attempt, so
/// the attempts dimension never moves, and if a caller only asks [`assess`] at
/// the end of an attempt it is never asked at all. The ticket this module was
/// written for named a coder session that ran six attempts over seventy-seven
/// minutes and wrote no blocked report because it was stuck rather than
/// failing; no record under `ops/` carries that session, so it is repeated here
/// as the shape of the problem and not as a citation.
///
/// The two dimensions that do not depend on the agent's own account of itself
/// are wall clock and tokens, and they are what catches this, but only if the
/// reading is taken on a cadence rather than at attempt boundaries. This
/// function is the cheap signal for that caller: between two readings, seconds
/// or tokens went up and attempts did not. It cannot tell a stuck agent from a
/// thorough one, and nothing in this crate can; what it can do is make the
/// pattern visible to something that can ask.
pub fn spent_without_attempt(previous: Consumed, current: Consumed) -> bool {
    current.attempts == previous.attempts
        && (current.wall_clock_s > previous.wall_clock_s || current.tokens > previous.tokens)
}

/// One attempt at the ticket and why it failed: AICD §12.
///
/// Derived from appendix A.5's "Approaches tried: each approach and why it
/// failed", and from AICD §12's "what it tried, why it believes the approaches
/// fail". The attempt number is not in either list and is added here because
/// criterion ORI-P1-009 requires the record of a session that failed twice to
/// hold "the two attempts": without a number, two entries both describing the
/// first attempt satisfy a count and answer nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Approach {
    attempt: u32,
    what: String,
    why_it_failed: String,
}

impl Approach {
    /// One approach, refusing an entry that records nothing: AICD §12.
    ///
    /// The attempt is numbered from one, because a zeroth attempt is not a
    /// thing that happened. A blank description or a blank reason is refused:
    /// an approach with no account of why it failed is the shape of a report
    /// that exists and says nothing, which is what AICD §12 asks the report for
    /// in the first place.
    pub fn new(attempt: u32, what: &str, why_it_failed: &str) -> Result<Self, BlockedReportError> {
        if attempt == 0 {
            return Err(BlockedReportError::AttemptNotNumbered);
        }
        if what.trim().is_empty() {
            return Err(BlockedReportError::Blank {
                field: "Approaches tried: the approach",
            });
        }
        if why_it_failed.trim().is_empty() {
            return Err(BlockedReportError::Blank {
                field: "Approaches tried: why it failed",
            });
        }

        Ok(Self {
            attempt,
            what: what.trim().to_owned(),
            why_it_failed: why_it_failed.trim().to_owned(),
        })
    }

    /// Which attempt this was, numbered from one: AICD §12.
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// What was tried: appendix A.5's "each approach", under AICD §12.
    pub fn what(&self) -> &str {
        &self.what
    }

    /// Why it failed: appendix A.5's "and why it failed", under AICD §12.
    pub fn why_it_failed(&self) -> &str {
        &self.why_it_failed
    }
}

/// What an agent writes instead of continuing: AICD §12.
///
/// Derived from AICD §12, "the coder stops, writes a blocked report ... and
/// hands off", with the fields of appendix A.5, which are the six below in the
/// order the appendix lists them: ticket, budget consumed, approaches tried,
/// hypothesis, what it would try next, what it needs from a human.
///
/// # A record here, and a rendering elsewhere
///
/// `spec/DATA_MODEL.md` section 2 stores it as a Report row with `kind` blocked
/// and a `content`, and as a MemoryRecord with `kind` blocked_report and a
/// `structured (json, sanitized)`; AICD §12 says it is "written to operational
/// memory so the next attempt starts informed". Those are a rendering and a
/// place to put it, and both need something to render. This is that something:
/// the fields as values, with the checks that a record claiming to be a blocked
/// report is one. The Markdown of `templates/blocked-report.md` is not produced
/// here and no field below is a formatted string.
///
/// Building one cannot be made conditional on an [`Exhaustion`], because
/// `templates/blocked-report.md` opens the same report to an agent that "cannot
/// proceed" for reasons that are not a budget. What is checked instead is that
/// the report accounts for every attempt the reading it carries says was spent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockedReport {
    ticket: String,
    consumed: Consumed,
    approaches: Vec<Approach>,
    hypothesis: String,
    next: String,
    needs: String,
}

impl BlockedReport {
    /// A blocked report, refusing one that is not complete: AICD §12.
    ///
    /// The parameters are in appendix A.5's field order, which is the only
    /// order this many strings has a reason to be in.
    ///
    /// Three kinds of refusal, each of which is a way for a report to exist and
    /// discharge nothing:
    ///
    /// 1. A blank field. Every field of appendix A.5 is required, and the
    ///    template that reproduces it marks all six so.
    /// 2. An attempt nobody accounted for. Criterion ORI-P1-009 requires the
    ///    record of two failed attempts to hold the two attempts, so the
    ///    approaches must number exactly `1` to the attempts the reading
    ///    carries, with no gap and no repeat.
    /// 3. No approach at all, when the reading says no attempt finished. A
    ///    budget cannot run out on nothing, so an agent stopped with a reading
    ///    of zero attempts still tried something, and appendix A.5 requires it
    ///    to say what.
    pub fn new(
        ticket: &str,
        consumed: Consumed,
        approaches: Vec<Approach>,
        hypothesis: &str,
        next: &str,
        needs: &str,
    ) -> Result<Self, BlockedReportError> {
        let required: Vec<(&'static str, &str)> = vec![
            ("Ticket", ticket),
            ("Hypothesis", hypothesis),
            ("What it would try next", next),
            ("What it needs from a human", needs),
        ];
        for (field, value) in required {
            if value.trim().is_empty() {
                return Err(BlockedReportError::Blank { field });
            }
        }

        let accounted_for: Vec<u32> = {
            let mut numbers: Vec<u32> = approaches
                .iter()
                .map(|approach| approach.attempt())
                .collect();
            numbers.sort_unstable();
            numbers
        };
        let owed: Vec<u32> = (1..=consumed.attempts.max(1)).collect();
        if accounted_for != owed {
            return Err(BlockedReportError::AttemptsUnaccountedFor {
                owed,
                accounted_for,
            });
        }

        Ok(Self {
            ticket: ticket.trim().to_owned(),
            consumed,
            approaches,
            hypothesis: hypothesis.trim().to_owned(),
            next: next.trim().to_owned(),
            needs: needs.trim().to_owned(),
        })
    }

    /// Appendix A.5's "Ticket", under AICD §12.
    pub fn ticket(&self) -> &str {
        &self.ticket
    }

    /// Appendix A.5's "Budget consumed", under AICD §12.
    pub fn consumed(&self) -> Consumed {
        self.consumed
    }

    /// Appendix A.5's "Approaches tried", under AICD §12, in attempt order.
    pub fn approaches(&self) -> &[Approach] {
        &self.approaches
    }

    /// Appendix A.5's "Hypothesis", under AICD §12.
    pub fn hypothesis(&self) -> &str {
        &self.hypothesis
    }

    /// Appendix A.5's "What it would try next", under AICD §12.
    pub fn next(&self) -> &str {
        &self.next
    }

    /// Appendix A.5's "What it needs from a human", under AICD §12.
    pub fn needs(&self) -> &str {
        &self.needs
    }
}

/// Why a blocked report was refused: AICD §12.
///
/// `spec/CONVENTIONS.md` names `thiserror` as the house error convention and
/// ruling R20 in `ops/rulings.md` defers it, stating what stands until then: "a
/// hand-written `Display` and `std::error::Error` implementation stands, with a
/// comment naming the conversion". This is that implementation, and the
/// conversion is mechanical: each arm of the `Display` below becomes an
/// `#[error("...")]` on its variant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockedReportError {
    /// A required field of appendix A.5 was blank.
    Blank {
        /// The field, named as appendix A.5 names it.
        field: &'static str,
    },
    /// An approach was recorded against attempt zero.
    AttemptNotNumbered,
    /// The approaches do not account for the attempts the reading carries.
    AttemptsUnaccountedFor {
        /// The attempt numbers the reading obliges, from one.
        owed: Vec<u32>,
        /// The attempt numbers the approaches carry, sorted.
        accounted_for: Vec<u32>,
    },
}

impl BlockedReportError {
    /// The methodology section this refusal is made under: AICD §12.
    ///
    /// AICD §12 is what requires the report, so it is what every refusal to
    /// accept an incomplete one is made under, and the module comment says why
    /// appendix A.5, which fixes the fields, is not what is cited. Built as a
    /// value rather than through `MethodologyRef::at`, which is fallible, in
    /// the shape `RefusalKind::reason` in `crates/ori-core/src/error.rs`
    /// already uses for the same reason.
    pub fn reason(&self) -> MethodologyRef {
        MethodologyRef {
            section: 12,
            subsection: None,
        }
    }
}

impl fmt::Display for BlockedReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blank { field } => write!(
                f,
                "a blocked report needs \"{field}\", which appendix A.5 requires and this one \
                 leaves blank (AICD §12)"
            ),
            Self::AttemptNotNumbered => write!(
                f,
                "an approach is recorded against attempt 0, and attempts are numbered from 1 \
                 (AICD §12)"
            ),
            Self::AttemptsUnaccountedFor {
                owed,
                accounted_for,
            } => write!(
                f,
                "the reading says attempt(s) {owed:?} were spent and the approaches account for \
                 {accounted_for:?}; a blocked report holds every attempt it is written about \
                 (AICD §12)"
            ),
        }
    }
}

impl std::error::Error for BlockedReportError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// The budget of ORI-P1-009's precondition, "budget attempts=2", with the
    /// two other dimensions set wide enough that nothing but attempts can run
    /// out in the criterion's scenario.
    fn criterion_budget() -> Budget {
        Budget {
            attempts: 2,
            wall_clock_s: 2_700,
            tokens: 200_000,
        }
    }

    /// The three prose fields of appendix A.5, filled, so that a test about one
    /// thing is not about them.
    const HYPOTHESIS: &str = "the headless adapter rejects the spawn arguments this ticket needs";
    const NEXT: &str = "pin the adapter version and re-run the two failing spawns";
    const NEEDS: &str = "a decision on whether the adapter may be pinned";

    fn approach(attempt: u32) -> Approach {
        Approach::new(
            attempt,
            "spawn the headless adapter with the ticket's arguments",
            "the adapter exited non-zero before the session opened",
        )
        .expect("this approach is well formed")
    }

    // ---- ORI-P1-009: the criterion's own scenario ----

    #[test]
    fn ori_p1_009_two_failed_attempts_against_a_budget_of_two_end_the_session_blocked() {
        let budget = criterion_budget();
        let after_one = Consumed {
            attempts: 1,
            wall_clock_s: 320,
            tokens: 20_500,
        };
        let after_two = Consumed {
            attempts: 2,
            wall_clock_s: 640,
            tokens: 41_000,
        };

        assert_eq!(
            assess(&budget, after_one),
            Verdict::MayContinue,
            "one failure of two allowed leaves an attempt, and a session stopped there would \
             have been stopped early"
        );

        let Verdict::StopAndReport(exhaustion) = assess(&budget, after_two) else {
            panic!(
                "ORI-P1-009: a budget of attempts=2 against two failed attempts must end the \
                 session Blocked, and this reading was answered may-continue"
            );
        };
        assert_eq!(
            exhaustion.dimensions(),
            vec![Dimension::Attempts],
            "attempts is what ran out; the other two had room"
        );
        assert_eq!(
            exhaustion.consumed(),
            after_two,
            "appendix A.5 asks for all three numbers, so all three travel with the verdict"
        );
    }

    #[test]
    fn ori_p1_009_the_blocked_report_holds_the_two_attempts() {
        let consumed = Consumed {
            attempts: 2,
            wall_clock_s: 640,
            tokens: 41_000,
        };
        let report = BlockedReport::new(
            "ORI-T-0051",
            consumed,
            vec![approach(1), approach(2)],
            HYPOTHESIS,
            NEXT,
            NEEDS,
        )
        .expect("a report accounting for both attempts is complete");

        assert_eq!(report.consumed(), consumed);
        assert_eq!(
            report.approaches().len(),
            2,
            "ORI-P1-009: the record exists with the two attempts"
        );
        assert_eq!(
            report
                .approaches()
                .iter()
                .map(Approach::attempt)
                .collect::<Vec<u32>>(),
            vec![1, 2],
            "both attempts are accounted for, and not the same one twice"
        );
        assert_eq!(report.ticket(), "ORI-T-0051");
        assert_eq!(report.hypothesis(), HYPOTHESIS);
        assert_eq!(report.next(), NEXT);
        assert_eq!(report.needs(), NEEDS);
    }

    #[test]
    fn ori_p1_009_a_report_that_does_not_account_for_every_attempt_is_refused() {
        let consumed = Consumed {
            attempts: 2,
            wall_clock_s: 640,
            tokens: 41_000,
        };

        // Every way two attempts can fail to be two accounted-for attempts,
        // enumerated: none, one, the second alone, the same one twice, and one
        // too many.
        let wrong: Vec<Vec<Approach>> = vec![
            vec![],
            vec![approach(1)],
            vec![approach(2)],
            vec![approach(1), approach(1)],
            vec![approach(1), approach(2), approach(3)],
        ];

        for approaches in wrong {
            let carried: Vec<u32> = approaches.iter().map(Approach::attempt).collect();
            let refused =
                BlockedReport::new("ORI-T-0051", consumed, approaches, HYPOTHESIS, NEXT, NEEDS);
            assert!(
                matches!(
                    refused,
                    Err(BlockedReportError::AttemptsUnaccountedFor { .. })
                ),
                "ORI-P1-009: two attempts were consumed and the approaches carry {carried:?}, \
                 which is not the two attempts; the report was accepted anyway"
            );
        }
    }

    #[test]
    fn ori_p1_009_a_report_with_a_blank_field_of_appendix_a_5_is_refused() {
        let consumed = Consumed {
            attempts: 1,
            wall_clock_s: 320,
            tokens: 20_500,
        };

        // The four prose fields appendix A.5 requires and this constructor
        // takes as text, each blanked in turn. "Budget consumed" is a value and
        // cannot be blank; "Approaches tried" has its own test above.
        let cases: [(&str, &str, &str, &str, &str); 4] = [
            ("Ticket", "   ", HYPOTHESIS, NEXT, NEEDS),
            ("Hypothesis", "ORI-T-0051", "", NEXT, NEEDS),
            (
                "What it would try next",
                "ORI-T-0051",
                HYPOTHESIS,
                " ",
                NEEDS,
            ),
            (
                "What it needs from a human",
                "ORI-T-0051",
                HYPOTHESIS,
                NEXT,
                "\t",
            ),
        ];

        for (field, ticket, hypothesis, next, needs) in cases {
            let refused =
                BlockedReport::new(ticket, consumed, vec![approach(1)], hypothesis, next, needs);
            assert_eq!(
                refused,
                Err(BlockedReportError::Blank { field }),
                "appendix A.5 requires \"{field}\" and a report with it blank was accepted"
            );
        }
    }

    #[test]
    fn ori_p1_009_an_approach_that_records_nothing_is_refused() {
        assert_eq!(
            Approach::new(1, "  ", "it failed"),
            Err(BlockedReportError::Blank {
                field: "Approaches tried: the approach"
            })
        );
        assert_eq!(
            Approach::new(1, "the approach", "\n"),
            Err(BlockedReportError::Blank {
                field: "Approaches tried: why it failed"
            })
        );
        assert_eq!(
            Approach::new(0, "the approach", "it failed"),
            Err(BlockedReportError::AttemptNotNumbered)
        );
    }

    #[test]
    fn ori_p1_009_a_session_that_finished_no_attempt_still_says_what_it_tried() {
        let consumed = Consumed {
            attempts: 0,
            wall_clock_s: 2_700,
            tokens: 41_000,
        };

        let refused = BlockedReport::new("ORI-T-0051", consumed, vec![], HYPOTHESIS, NEXT, NEEDS);
        assert!(
            matches!(
                refused,
                Err(BlockedReportError::AttemptsUnaccountedFor { .. })
            ),
            "a budget cannot run out on nothing, so a report with no approach at all is empty \
             whatever the attempt count says"
        );

        assert!(
            BlockedReport::new(
                "ORI-T-0051",
                consumed,
                vec![approach(1)],
                HYPOTHESIS,
                NEXT,
                NEEDS
            )
            .is_ok(),
            "one approach is what a reading of zero finished attempts obliges"
        );
    }

    // ---- the rule over the three dimensions ----

    #[test]
    fn ori_t_0051_every_boundary_triple_names_exactly_the_spent_dimensions() {
        let allowed: [u64; Dimension::COUNT] = [2, 2_700, 200_000];
        let budget = Budget {
            attempts: 2,
            wall_clock_s: 2_700,
            tokens: 200_000,
        };

        // Three positions per dimension, one below the allowance, at it and one
        // above, which is every way a comparison against a limit can be wrong
        // by one. 27 triples, all of them.
        let offsets: [i64; 3] = [-1, 0, 1];
        let mut seen = 0_usize;

        for attempts in offsets {
            for seconds in offsets {
                for tokens in offsets {
                    let at = |allowance: u64, offset: i64| -> u64 {
                        if offset < 0 {
                            allowance - 1
                        } else {
                            allowance + u64::try_from(offset).expect("offset 0 or 1 fits")
                        }
                    };
                    let consumed = Consumed {
                        attempts: u32::try_from(at(allowed[0], attempts))
                            .expect("the attempt counts here fit"),
                        wall_clock_s: at(allowed[1], seconds),
                        tokens: at(allowed[2], tokens),
                    };
                    let expected: Vec<Dimension> = Dimension::ALL
                        .into_iter()
                        .filter(|dimension| {
                            consumed.spent(*dimension) >= budget.allowed(*dimension)
                        })
                        .collect();

                    match assess(&budget, consumed) {
                        Verdict::MayContinue => assert!(
                            expected.is_empty(),
                            "{consumed} was answered may-continue and {expected:?} is/are at or \
                             past the allowance"
                        ),
                        Verdict::StopAndReport(exhaustion) => assert_eq!(
                            exhaustion.dimensions(),
                            expected,
                            "{consumed} against a budget of {:?}",
                            allowed
                        ),
                    }
                    seen += 1;
                }
            }
        }

        assert_eq!(seen, 27, "three positions in each of three dimensions");
    }

    #[test]
    fn ori_t_0051_any_one_spent_dimension_stops_the_session_and_no_spent_dimension_does_not() {
        let budget = Budget {
            attempts: 2,
            wall_clock_s: 2_700,
            tokens: 200_000,
        };

        // Every subset of the three dimensions, by whether each is at its
        // allowance or one below it. Eight, and the empty one is the only one
        // that may continue.
        for pattern in 0..8_u8 {
            let spent = |bit: u8| pattern & (1 << bit) != 0;
            let consumed = Consumed {
                attempts: if spent(0) { 2 } else { 1 },
                wall_clock_s: if spent(1) { 2_700 } else { 2_699 },
                tokens: if spent(2) { 200_000 } else { 199_999 },
            };
            let expected: Vec<Dimension> = Dimension::ALL
                .into_iter()
                .enumerate()
                .filter(|(bit, _)| spent(u8::try_from(*bit).expect("three bits")))
                .map(|(_, dimension)| dimension)
                .collect();

            match assess(&budget, consumed) {
                Verdict::MayContinue => assert!(
                    expected.is_empty(),
                    "pattern {pattern:b} has {expected:?} spent and the session was let continue"
                ),
                Verdict::StopAndReport(exhaustion) => {
                    assert!(
                        !expected.is_empty(),
                        "pattern {pattern:b} has nothing spent and the session was stopped"
                    );
                    assert_eq!(exhaustion.dimensions(), expected);
                    for dimension in Dimension::ALL {
                        assert_eq!(
                            exhaustion.includes(dimension),
                            expected.contains(&dimension),
                            "{dimension} in pattern {pattern:b}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn ori_t_0051_an_allowance_of_zero_is_spent_before_anything_is() {
        let nothing_allowed = Budget {
            attempts: 0,
            wall_clock_s: 0,
            tokens: 0,
        };

        let Verdict::StopAndReport(exhaustion) = assess(&nothing_allowed, Consumed::NOTHING) else {
            panic!("a budget that permits nothing has nothing left at the start of the session");
        };
        assert_eq!(exhaustion.dimensions(), Dimension::ALL.to_vec());

        // One dimension at zero is enough, and the other two having room does
        // not save it.
        let no_attempts = Budget {
            attempts: 0,
            wall_clock_s: 2_700,
            tokens: 200_000,
        };
        let Verdict::StopAndReport(exhaustion) = assess(&no_attempts, Consumed::NOTHING) else {
            panic!("a budget of zero attempts permits no attempt");
        };
        assert_eq!(exhaustion.dimensions(), vec![Dimension::Attempts]);
    }

    #[test]
    fn ori_t_0051_what_is_left_never_wraps_when_the_meter_overshoots() {
        let budget = Budget {
            attempts: 2,
            wall_clock_s: 2_700,
            tokens: 200_000,
        };
        let over = Consumed {
            attempts: 9,
            wall_clock_s: 9_000,
            tokens: 900_000,
        };

        for dimension in Dimension::ALL {
            assert_eq!(
                remaining(&budget, over, dimension),
                0,
                "{dimension} is overspent, and what is left of it is none, not a very large number"
            );
        }

        let untouched = Consumed::NOTHING;
        assert_eq!(remaining(&budget, untouched, Dimension::Attempts), 2);
        assert_eq!(remaining(&budget, untouched, Dimension::WallClock), 2_700);
        assert_eq!(remaining(&budget, untouched, Dimension::Tokens), 200_000);
    }

    // ---- the agent that is stuck rather than failing ----

    #[test]
    fn ori_t_0051_spending_that_crosses_no_attempt_boundary_is_visible() {
        let at_the_start = Consumed {
            attempts: 1,
            wall_clock_s: 300,
            tokens: 20_000,
        };

        // Time passed and tokens went, and the agent declared no attempt over.
        // This is the shape of a session that is stuck rather than failing.
        let later = Consumed {
            attempts: 1,
            wall_clock_s: 4_800,
            tokens: 310_000,
        };
        assert!(spent_without_attempt(at_the_start, later));

        // An attempt ended, so the attempts dimension is moving and AICD §12's
        // budget works as written.
        let attempt_ended = Consumed {
            attempts: 2,
            wall_clock_s: 4_800,
            tokens: 310_000,
        };
        assert!(!spent_without_attempt(at_the_start, attempt_ended));

        // Nothing moved at all. A session that is spending nothing is not the
        // subject of this signal, and answering true here would fire it on
        // every idle reading.
        assert!(!spent_without_attempt(at_the_start, at_the_start));

        // And the wall clock alone is enough: a session can be stuck without
        // spending a token.
        let only_time = Consumed {
            wall_clock_s: 4_800,
            ..at_the_start
        };
        assert!(spent_without_attempt(at_the_start, only_time));
    }

    #[test]
    fn ori_t_0051_a_stuck_session_runs_out_of_the_two_dimensions_it_does_not_control() {
        let budget = Budget {
            attempts: 6,
            wall_clock_s: 2_700,
            tokens: 200_000,
        };

        // Seventy-seven minutes, one attempt declared. The attempts dimension
        // says five attempts are left; the session is over all the same.
        let stuck = Consumed {
            attempts: 1,
            wall_clock_s: 4_620,
            tokens: 90_000,
        };

        assert_eq!(
            remaining(&budget, stuck, Dimension::Attempts),
            5,
            "the dimension the agent itself advances says there is plenty left"
        );
        let Verdict::StopAndReport(exhaustion) = assess(&budget, stuck) else {
            panic!(
                "a session at 4620s against an allowance of 2700s is over, whatever its attempt \
                 count says"
            );
        };
        assert_eq!(exhaustion.dimensions(), vec![Dimension::WallClock]);
    }

    // ---- the dimensions themselves ----

    #[test]
    fn ori_t_0051_the_three_dimensions_are_distinct_and_named() {
        let names: Vec<&str> = Dimension::ALL.into_iter().map(Dimension::as_str).collect();
        assert_eq!(names, vec!["attempts", "wall_clock_s", "tokens"]);
        assert_eq!(Dimension::ALL.len(), Dimension::COUNT);

        // Every dimension reads a different field of a reading, so a reading
        // with three different numbers in it answers three different numbers.
        let reading = Consumed {
            attempts: 1,
            wall_clock_s: 2,
            tokens: 3,
        };
        let spent: Vec<u64> = Dimension::ALL
            .into_iter()
            .map(|dimension| reading.spent(dimension))
            .collect();
        assert_eq!(spent, vec![1, 2, 3]);
    }

    #[test]
    fn ori_t_0051_every_refusal_names_the_methodology_section_it_is_made_under() {
        let refusals = [
            BlockedReportError::Blank { field: "Ticket" },
            BlockedReportError::AttemptNotNumbered,
            BlockedReportError::AttemptsUnaccountedFor {
                owed: vec![1, 2],
                accounted_for: vec![1],
            },
        ];

        for refusal in refusals {
            assert_eq!(
                refusal.reason(),
                MethodologyRef {
                    section: 12,
                    subsection: None
                },
                "AICD §12 is what requires the report, so it is what a refusal to accept an \
                 incomplete one is made under"
            );
            let sentence = refusal.to_string();
            assert!(
                sentence.contains("AICD §12"),
                "a refusal a human reads says what it is made under: {sentence}"
            );
        }
    }
}
