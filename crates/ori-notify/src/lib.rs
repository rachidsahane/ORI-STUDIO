//! Notification routing: AICD §12, AICD §16.
//!
//! Owns `Router` (interrupt, window, digest), `Desktop` and `Digest`
//! (`spec/LLD.md` section 2).
//!
//! The three routes implement one rule stated twice. AICD §12 puts blocked and
//! escalated items in a human review queue processed at a fixed cadence, for
//! example twice a day, and lets only incidents interrupt a human outside that
//! cadence; AICD §16 states the same limit from the other side, that incidents
//! are the one case where a human is interrupted outside the review cadence. So
//! the window route holds what belongs to that queue (escalations, blocked
//! reports, approvals, QA summaries, drift) until the next window, the digest is
//! that queue delivered when the window opens, and interrupt is reserved for
//! incidents. What Ori Studio also interrupts for, an unattributed change, an
//! inert gate, an exposed credential, opens an incident first, so it is not a
//! fourth category. Routing anything else to interrupt recreates the always-on
//! human bottleneck AICD §12 exists to prevent.
//!
//! Must not: bypass the router (`spec/LLD.md` section 2).
