//! The budget meter: AICD §12.
//!
//! AICD §12's escalation protocol reads: "Every ticket carries a budget:
//! attempts, wall-clock time, tokens. When any limit is exceeded, the coder
//! stops, writes a blocked report [...] and hands off. It never keeps
//! grinding." `spec/LLD.md` section 2 gives this crate the "`Budget` meter",
//! and `crates/ori-orchestrator/src/budgets.rs`'s own module comment draws the
//! line precisely: "The meter counts. It is the thing that can see a clock, a
//! token accounting and the end of an attempt, and it is in the crate that
//! spawns the process those numbers come from. This module never reads a
//! clock, never counts an attempt and holds no state." [`Meter`] is that
//! meter.
//!
//! # Mirrored, not consumed, and why
//!
//! `spec/LLD.md` section 2's dependency graph draws `ori-orchestrator ->
//! ori-runtime`, not the other way: "Dependencies point downward only. A crate
//! may depend on those below it, never above or sideways unless listed", and
//! the diagram lists `ORC --> RT`. Depending on `ori-orchestrator` from this
//! crate would reverse that arrow, which is an architectural change and not a
//! matter of adding a line to `Cargo.toml`. `crates/ori-orchestrator/src/budgets.rs`'s
//! rule (`assess`, `remaining`, `spent_without_attempt`, the `Dimension` and
//! `Consumed` shapes) is therefore mirrored here rather than imported.
//! [`Dimension`], [`Consumed`], [`Verdict`], [`assess`], [`remaining`] and
//! [`spent_without_attempt`] are, by construction, the same rule stated twice;
//! a divergence between the two files is a defect in this one, since
//! `ori-orchestrator/src/budgets.rs` is the rule's specified home
//! (`spec/LLD.md` section 2 gives `ori-orchestrator` "budgets" at tier 1 in
//! `spec/RISK_MAP.md`) and this module's job is to feed it real readings, not
//! to redefine what a reading means. The mirror is deliberately not 1:1:
//! [`Verdict::StopAndReport`] here carries the spent [`Dimension`]s rather than
//! a whole second `Exhaustion` value, because the reading that produced them is
//! already on [`Meter`], and appendix A.5's blocked report itself is
//! `ori-orchestrator::budgets::BlockedReport`, which this module does not
//! reproduce.
//!
//! # The cadence obligation, and what discharges it
//!
//! `crates/ori-orchestrator/src/budgets.rs::spent_without_attempt` documents
//! itself as "a pure function cannot force its own call schedule" and names
//! its caller as whatever polls "on a cadence". [`Meter`] is that caller, and
//! [`Meter::possibly_stuck`] is answered fresh on every [`Meter::observe`] or
//! [`Meter::attempt_ended`] call by comparing the reading just replaced to the
//! one that replaces it. That discharges the obligation at the grain of "once
//! per observation, whenever one arrives"; it does not discharge "poll no less
//! often than every N seconds", because nothing in this module runs a timer.
//! Inventing one here would fabricate a cadence the specification does not
//! fix: `ori-runtime`'s `AcpClient` and `HeadlessAdapter` (`spec/LLD.md`
//! section 2) are the components that read a child process's output as it
//! arrives, and neither is built yet (the headless adapter is ORI-T-0033).
//! Whatever loop they end up running is the schedule this meter will be called
//! on; a meter cannot supply the loop that is supposed to call it, and a
//! best-guess timer bolted on here, in a ticket that does not own that loop,
//! would be exactly the kind of control that reads as protection without being
//! one (AICD §39).
//!
//! # What this module does not decide
//!
//! [`Meter::verdict`] says whether a reading is exhausted. It does not write a
//! blocked report (`ori-orchestrator::budgets::BlockedReport`), does not move
//! a [`crate::session::Session`] to [`crate::session::Outcome::Blocked`] (that
//! is `Session::end`, `crates/ori-runtime/src/session.rs`), and does not
//! revoke a credential or requeue a ticket. Those are ORI-P1-009's other
//! clauses and other crates' rows; see the criterion coverage note on
//! [`Meter`] for the split in full.
//!
//! Must not: read a clock (`ori-runtime` may; this type does not on its own,
//! it is handed a [`ori_core::types::Timestamp`] the way `Session` is), or
//! decide the outcome of a session.

use core::fmt;
use std::error::Error;

use ori_core::error::MethodologyRef;
use ori_core::types::Budget;
use ori_core::types::Timestamp;

/// One of the three things a budget is measured in: AICD §12.
///
/// Mirrors `ori_orchestrator::budgets::Dimension` in the order AICD §12 names
/// them: "attempts, wall-clock time, tokens". See the module doc for why this
/// is a mirror and not an import.
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
    /// Every dimension, in AICD §12's order.
    pub const ALL: [Self; 3] = [Self::Attempts, Self::WallClock, Self::Tokens];

    /// The spelling `spec/DATA_MODEL.md` section 2 gives it in the Ticket
    /// row's `budget (attempts, wall_clock_s, tokens)`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Attempts => "attempts",
            Self::WallClock => "wall_clock_s",
            Self::Tokens => "tokens",
        }
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A reading from the meter: what a session has spent so far: AICD §12.
///
/// Mirrors `ori_orchestrator::budgets::Consumed`. `attempts` widens to `u64`
/// nowhere in this module because nothing here compares it against anything
/// wider; it stays the `u32` `ori_core::types::Budget::attempts` already is.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Consumed {
    /// Attempts made, counted by [`Meter::attempt_ended`] as they end.
    pub attempts: u32,
    /// Wall clock seconds spent, counted by [`Meter`] from the timestamps it
    /// is given.
    pub wall_clock_s: u64,
    /// Tokens spent, as reported by whatever adapter is spending them.
    pub tokens: u64,
}

impl Consumed {
    /// A session that has spent nothing.
    pub const NOTHING: Self = Self {
        attempts: 0,
        wall_clock_s: 0,
        tokens: 0,
    };

    /// What was spent in one dimension.
    #[must_use]
    pub fn spent(self, dimension: Dimension) -> u64 {
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

/// What a reading obliges: AICD §12.
///
/// Mirrors `ori_orchestrator::budgets::Verdict`, with `StopAndReport` carrying
/// the spent dimensions directly rather than a second `Exhaustion` value; see
/// the module doc.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// Every dimension has something left; the session may continue.
    MayContinue,
    /// At least one dimension is spent, in AICD §12's order.
    StopAndReport(Vec<Dimension>),
}

impl Verdict {
    /// Whether this verdict says to stop.
    #[must_use]
    pub const fn must_stop(&self) -> bool {
        matches!(self, Self::StopAndReport(_))
    }
}

/// What this dimension allows, read off `ori_core::types::Budget` directly.
///
/// No trait: `ori_orchestrator::budgets::Allowance` exists so a caller there
/// can hold an allowance other than `Budget`; this module has exactly one
/// caller and one allowance type, so a trait would be an abstraction with one
/// implementation and nothing to justify it.
fn allowed(allowance: Budget, dimension: Dimension) -> u64 {
    match dimension {
        Dimension::Attempts => u64::from(allowance.attempts),
        Dimension::WallClock => allowance.wall_clock_s,
        Dimension::Tokens => allowance.tokens,
    }
}

/// Whether a reading leaves a session free to continue: AICD §12.
///
/// Mirrors `ori_orchestrator::budgets::assess`, including its reading of
/// "exceeded": a dimension is spent when consumption has *reached* the
/// allowance, not only once it has passed it, for the reason that function's
/// doc comment gives (an allowance of two attempts is two attempts, and none
/// is left after the second; criterion ORI-P1-009 fixes this reading).
#[must_use]
pub fn assess(allowance: Budget, consumed: Consumed) -> Verdict {
    let spent: Vec<Dimension> = Dimension::ALL
        .into_iter()
        .filter(|dimension| consumed.spent(*dimension) >= allowed(allowance, *dimension))
        .collect();
    if spent.is_empty() {
        Verdict::MayContinue
    } else {
        Verdict::StopAndReport(spent)
    }
}

/// What is left in one dimension: AICD §12.
///
/// Mirrors `ori_orchestrator::budgets::remaining`, saturating at zero for the
/// same reason: a meter can observe a reading past the allowance between two
/// calls, and "less than nothing left" is not a value a caller can act on
/// differently from zero.
#[must_use]
pub fn remaining(allowance: Budget, consumed: Consumed, dimension: Dimension) -> u64 {
    allowed(allowance, dimension).saturating_sub(consumed.spent(dimension))
}

/// Whether a session spent something without finishing an attempt.
///
/// Mirrors `ori_orchestrator::budgets::spent_without_attempt` exactly: no AICD
/// section applies to this function either, for the same reason its mirror
/// gives. See the module doc's "cadence obligation" section for what calls it
/// and how often.
#[must_use]
pub fn spent_without_attempt(previous: Consumed, current: Consumed) -> bool {
    current.attempts == previous.attempts
        && (current.wall_clock_s > previous.wall_clock_s || current.tokens > previous.tokens)
}

/// The meter that counts a session's spend against its ticket's budget:
/// AICD §12.
///
/// # What of ORI-P1-009 this type is answerable for
///
/// The criterion: "Coder session with budget attempts=2 | Headless adapter
/// fails twice | Session ends Blocked; a blocked report record exists with the
/// two attempts; credentials revoked; ticket returns to Queued on re-plan."
/// Five clauses, and this type owns exactly the trigger for the first and
/// nothing else:
///
/// - "Session ends Blocked": [`Meter::verdict`] is what a caller consults to
///   decide when to call [`crate::session::Session::end`] with
///   [`crate::session::Outcome::Blocked`]; the session itself is
///   `crates/ori-runtime/src/session.rs`'s and already claims this clause with
///   its own `ori_p1_031_*` tests for the sibling criterion's shape, so the
///   `Blocked` outcome itself is exercised there, not duplicated here.
/// - "a blocked report record exists with the two attempts": is
///   `ori_orchestrator::budgets::BlockedReport`, already built and tested
///   (`ori_p1_009_*` in `crates/ori-orchestrator/src/budgets.rs`). **This
///   criterion is double-claimed**: this module's own `ori_p1_009_*` tests
///   below cover only that the meter itself notices the exhaustion after two
///   attempts against an allowance of two, which is upstream of and does not
///   duplicate the report-construction tests in that crate.
/// - "credentials revoked": `ori-broker`'s `Issuance`.
/// - "ticket returns to Queued on re-plan": `ori-core`'s `Ticket::apply`,
///   the `Blocked -> Queued` edge `spec/DATA_MODEL.md` section 3 draws.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Meter {
    allowance: Budget,
    started_at: Timestamp,
    consumed: Consumed,
    previous: Consumed,
}

impl Meter {
    /// A meter for a session with this allowance, starting now.
    #[must_use]
    pub const fn new(allowance: Budget, started_at: Timestamp) -> Self {
        Self {
            allowance,
            started_at,
            consumed: Consumed::NOTHING,
            previous: Consumed::NOTHING,
        }
    }

    /// The allowance this meter was given.
    #[must_use]
    pub const fn allowance(&self) -> Budget {
        self.allowance
    }

    /// When the session this meter counts for started.
    #[must_use]
    pub const fn started_at(&self) -> Timestamp {
        self.started_at
    }

    /// The latest reading.
    #[must_use]
    pub const fn consumed(&self) -> Consumed {
        self.consumed
    }

    /// What the latest reading obliges.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        assess(self.allowance, self.consumed)
    }

    /// What is left in one dimension, at the latest reading.
    #[must_use]
    pub fn remaining(&self, dimension: Dimension) -> u64 {
        remaining(self.allowance, self.consumed, dimension)
    }

    /// Whether the latest reading moved wall clock or tokens without an
    /// attempt ending: the stuck-agent signal. See the module doc.
    #[must_use]
    pub fn possibly_stuck(&self) -> bool {
        spent_without_attempt(self.previous, self.consumed)
    }

    /// A reading taken while the current attempt is still running: attempts
    /// does not move.
    ///
    /// `now` must not be before this meter's `started_at`, and `tokens` must
    /// not be lower than the tokens already recorded: both would mean a clock
    /// or a token count ran backward, which is not a thing that happens to a
    /// running session and is refused rather than silently absorbed into a
    /// reading nobody can then trust.
    pub fn observe(&self, now: Timestamp, tokens: u64) -> Result<Self, BudgetError> {
        let wall_clock_s = self.elapsed(now)?;
        self.reading(Consumed {
            attempts: self.consumed.attempts,
            wall_clock_s,
            tokens,
        })
    }

    /// One attempt finished, whether it succeeded or failed: attempts moves by
    /// exactly one.
    ///
    /// The same two refusals as [`Meter::observe`] apply, for the same reason.
    pub fn attempt_ended(&self, now: Timestamp, tokens: u64) -> Result<Self, BudgetError> {
        let wall_clock_s = self.elapsed(now)?;
        self.reading(Consumed {
            attempts: self.consumed.attempts + 1,
            wall_clock_s,
            tokens,
        })
    }

    /// Seconds since this meter started, refusing a `now` before it.
    fn elapsed(&self, now: Timestamp) -> Result<u64, BudgetError> {
        let millis = now.millis() - self.started_at.millis();
        if millis < 0 {
            return Err(BudgetError::BeforeStart {
                now,
                started_at: self.started_at,
            });
        }
        Ok((millis / 1000).unsigned_abs())
    }

    /// Replaces the reading, refusing one that moves a dimension backward.
    fn reading(&self, next: Consumed) -> Result<Self, BudgetError> {
        for dimension in Dimension::ALL {
            let previous = self.consumed.spent(dimension);
            let candidate = next.spent(dimension);
            if candidate < previous {
                return Err(BudgetError::WentBackward {
                    dimension,
                    previous,
                    next: candidate,
                });
            }
        }
        Ok(Self {
            allowance: self.allowance,
            started_at: self.started_at,
            consumed: next,
            previous: self.consumed,
        })
    }
}

impl fmt::Display for Meter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "meter at {} against {:?}", self.consumed, self.allowance)
    }
}

/// Everything this module refuses: AICD §12.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BudgetError {
    /// A reading was taken at a time before the meter started.
    BeforeStart {
        /// The time asked for.
        now: Timestamp,
        /// When the meter started.
        started_at: Timestamp,
    },
    /// A reading moved a dimension backward.
    WentBackward {
        /// The dimension that moved backward.
        dimension: Dimension,
        /// What it was.
        previous: u64,
        /// What it was asked to become.
        next: u64,
    },
}

impl BudgetError {
    /// The methodology section this refusal rests on: AICD §12, the section
    /// that makes the reading a control in the first place.
    ///
    /// `spec/CONVENTIONS.md` names `thiserror` as the house convention; ruling
    /// R20 in `ops/rulings.md` defers it and states what stands until then, "a
    /// hand-written `Display` and `std::error::Error` implementation [...],
    /// with a comment naming the conversion", which is what this is.
    #[must_use]
    pub const fn methodology_ref(&self) -> MethodologyRef {
        MethodologyRef {
            section: 12,
            subsection: None,
        }
    }
}

impl fmt::Display for BudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeforeStart { now, started_at } => {
                write!(f, "{now} is before the meter started at {started_at}")
            }
            Self::WentBackward {
                dimension,
                previous,
                next,
            } => write!(
                f,
                "'{dimension}' moved from {previous} to {next}, which is backward"
            ),
        }
    }
}

impl Error for BudgetError {}

#[cfg(test)]
mod tests {
    use super::*;

    const START: Timestamp = Timestamp::from_millis(1_700_000_000_000);

    fn at(offset_s: i64) -> Timestamp {
        Timestamp::from_millis(START.millis() + offset_s * 1000)
    }

    /// The budget of ORI-P1-009's precondition, "budget attempts=2", with the
    /// other two dimensions wide enough that only attempts can run out.
    fn criterion_budget() -> Budget {
        Budget {
            attempts: 2,
            wall_clock_s: 2_700,
            tokens: 200_000,
        }
    }

    // ---------------------------------------------------------------------
    // ORI-P1-009 (double-claimed: see the doc comment on `Meter`)
    // ---------------------------------------------------------------------

    #[test]
    fn ori_p1_009_the_meter_may_continue_after_one_failed_attempt() {
        let meter = Meter::new(criterion_budget(), START)
            .attempt_ended(at(300), 20_000)
            .expect("the first attempt ends within its allowance");
        assert_eq!(meter.consumed().attempts, 1);
        assert_eq!(meter.verdict(), Verdict::MayContinue);
    }

    #[test]
    fn ori_p1_009_the_meter_stops_and_reports_after_the_second_failed_attempt() {
        let meter = Meter::new(criterion_budget(), START)
            .attempt_ended(at(300), 20_000)
            .expect("first attempt")
            .attempt_ended(at(620), 41_000)
            .expect("second attempt");
        assert_eq!(meter.consumed().attempts, 2);
        assert_eq!(
            meter.verdict(),
            Verdict::StopAndReport(vec![Dimension::Attempts]),
            "attempts=2 against an allowance of 2 is exhausted, and only that \
             dimension, since the other two are nowhere near their allowance"
        );
    }

    // ---------------------------------------------------------------------
    // Prove it 1: a budget exhausted mid-session must be noticed
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_a_reading_that_reaches_the_wall_clock_allowance_mid_attempt_is_noticed() {
        let budget = Budget {
            attempts: 100,
            wall_clock_s: 600,
            tokens: 1_000_000,
        };
        let meter = Meter::new(budget, START)
            .observe(at(600), 10)
            .expect("a reading at exactly the allowance");
        assert_eq!(
            meter.verdict(),
            Verdict::StopAndReport(vec![Dimension::WallClock]),
            "600s consumed against a 600s allowance is spent, mid-attempt, \
             with attempts nowhere near its own limit"
        );
    }

    #[test]
    fn ori_t_0034_a_reading_that_reaches_the_token_allowance_mid_attempt_is_noticed() {
        let budget = Budget {
            attempts: 100,
            wall_clock_s: 100_000,
            tokens: 50_000,
        };
        let meter = Meter::new(budget, START)
            .observe(at(10), 50_000)
            .expect("a reading at exactly the token allowance");
        assert_eq!(
            meter.verdict(),
            Verdict::StopAndReport(vec![Dimension::Tokens])
        );
    }

    #[test]
    fn ori_t_0034_two_dimensions_exhausted_together_are_both_named() {
        let budget = Budget {
            attempts: 1,
            wall_clock_s: 10,
            tokens: 1_000_000,
        };
        let meter = Meter::new(budget, START)
            .attempt_ended(at(10), 5)
            .expect("one attempt, exactly at the wall clock allowance too");
        assert_eq!(
            meter.verdict(),
            Verdict::StopAndReport(vec![Dimension::Attempts, Dimension::WallClock]),
            "AICD §12's order is attempts, wall-clock, tokens"
        );
    }

    // ---------------------------------------------------------------------
    // The cadence signal
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_wall_clock_moving_without_an_attempt_ending_is_possibly_stuck() {
        let meter = Meter::new(criterion_budget(), START)
            .observe(at(60), 100)
            .expect("mid-attempt reading");
        assert!(
            meter.possibly_stuck(),
            "attempts did not move and wall clock did"
        );
    }

    #[test]
    fn ori_t_0034_tokens_moving_without_an_attempt_ending_is_possibly_stuck() {
        let meter = Meter::new(criterion_budget(), START)
            .observe(at(0), 500)
            .expect("mid-attempt reading");
        assert!(meter.possibly_stuck());
    }

    #[test]
    fn ori_t_0034_a_reading_that_repeats_exactly_is_not_possibly_stuck() {
        let meter = Meter::new(criterion_budget(), START)
            .observe(at(0), 0)
            .expect("a reading identical to the start");
        assert!(
            !meter.possibly_stuck(),
            "nothing moved, so there is nothing to call stuck"
        );
    }

    #[test]
    fn ori_t_0034_an_attempt_ending_is_never_possibly_stuck_however_much_it_spent() {
        let meter = Meter::new(criterion_budget(), START)
            .attempt_ended(at(2_000), 190_000)
            .expect("one large attempt");
        assert!(
            !meter.possibly_stuck(),
            "attempts moved, which is exactly what distinguishes a session \
             that is failing from one that never finishes anything"
        );
    }

    #[test]
    fn ori_t_0034_possibly_stuck_compares_the_two_most_recent_readings_only() {
        // Three observations: the first moves wall clock (stuck), the second
        // is an attempt ending (not stuck, attempts moved), the third moves
        // wall clock again relative to the second (stuck again). This is the
        // "one comparison per observation" cadence the module doc describes:
        // each call answers about the pair either side of it, not about the
        // session's whole history.
        let meter = Meter::new(criterion_budget(), START)
            .observe(at(10), 10)
            .expect("first");
        assert!(meter.possibly_stuck());

        let meter = meter.attempt_ended(at(20), 20).expect("second");
        assert!(!meter.possibly_stuck());

        let meter = meter.observe(at(30), 30).expect("third");
        assert!(meter.possibly_stuck());
    }

    // ---------------------------------------------------------------------
    // Refusals
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_a_reading_before_the_meter_started_is_refused() {
        let meter = Meter::new(criterion_budget(), START);
        let refusal = meter
            .observe(Timestamp::from_millis(START.millis() - 1), 0)
            .expect_err("a clock does not run backward");
        assert!(matches!(refusal, BudgetError::BeforeStart { .. }));
        assert_eq!(refusal.methodology_ref().section, 12);
    }

    #[test]
    fn ori_t_0034_tokens_moving_backward_is_refused() {
        let meter = Meter::new(criterion_budget(), START)
            .observe(at(10), 500)
            .expect("first reading");
        let refusal = meter
            .observe(at(20), 100)
            .expect_err("tokens spent do not go down");
        assert_eq!(
            refusal,
            BudgetError::WentBackward {
                dimension: Dimension::Tokens,
                previous: 500,
                next: 100,
            }
        );
    }

    #[test]
    fn ori_t_0034_wall_clock_moving_backward_is_refused() {
        let meter = Meter::new(criterion_budget(), START)
            .observe(at(100), 0)
            .expect("first reading");
        let refusal = meter
            .observe(at(50), 0)
            .expect_err("wall clock does not run backward");
        assert!(matches!(
            refusal,
            BudgetError::WentBackward {
                dimension: Dimension::WallClock,
                ..
            }
        ));
    }

    // ---------------------------------------------------------------------
    // `remaining`
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_remaining_counts_down_and_saturates_at_zero() {
        let meter = Meter::new(criterion_budget(), START)
            .attempt_ended(at(100), 50_000)
            .expect("one attempt");
        assert_eq!(meter.remaining(Dimension::Attempts), 1);
        assert_eq!(meter.remaining(Dimension::WallClock), 2_600);
        assert_eq!(meter.remaining(Dimension::Tokens), 150_000);

        let meter = meter
            .attempt_ended(at(50_000), 500_000)
            .expect("an attempt that overshoots every dimension");
        assert_eq!(meter.remaining(Dimension::Attempts), 0);
        assert_eq!(meter.remaining(Dimension::WallClock), 0);
        assert_eq!(meter.remaining(Dimension::Tokens), 0);
    }

    // ---------------------------------------------------------------------
    // `assess` and `remaining` as free functions agree with `Meter`
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_the_free_functions_and_the_meter_agree() {
        let budget = criterion_budget();
        let consumed = Consumed {
            attempts: 2,
            wall_clock_s: 640,
            tokens: 41_000,
        };
        let meter = Meter::new(budget, START)
            .attempt_ended(at(320), 20_500)
            .expect("first")
            .attempt_ended(at(640), 41_000)
            .expect("second");
        assert_eq!(meter.consumed(), consumed);
        assert_eq!(assess(budget, consumed), meter.verdict());
        for dimension in Dimension::ALL {
            assert_eq!(
                remaining(budget, consumed, dimension),
                meter.remaining(dimension)
            );
        }
    }

    #[test]
    fn ori_t_0034_every_dimension_display_matches_the_data_model_spelling() {
        assert_eq!(Dimension::Attempts.to_string(), "attempts");
        assert_eq!(Dimension::WallClock.to_string(), "wall_clock_s");
        assert_eq!(Dimension::Tokens.to_string(), "tokens");
    }
}
