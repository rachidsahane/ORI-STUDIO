//! Cross-model refusal at identity creation: ADR-0001 "Model family", AICD §7,
//! criterion ORI-P1-035.
//!
//! `spec/LLD.md` section 2 gives `ori-broker` "`Identity`, `Issuance`,
//! `Keychain` (keyring), `ForbiddenActionTest`"; this module is not a fifth
//! entity of its own but the rule ADR-0001's "Model family" decision states
//! about the first of those four: "`broker.identity.create` refuses a lead
//! identity whose family equals that of the coders it reviews", and, in the
//! same decision's closing sentence, "A lead identity and the coders it
//! reviews never share a model family, and the check is enforced at identity
//! creation, not at review time."
//!
//! # ORI-T-0108: what moved, and what is still not wired in
//!
//! The operator's ruling on escalation 6 (`ops/phase-1-backlog.md`) settled
//! where the family lives: on [`crate::identity::AgentIdentity`], as a
//! required [`ori_core::types::ModelFamily`] value, never on
//! `ProviderBinding`. Two things follow from that ruling and are done here:
//! [`ModelFamily`] wraps [`ori_core::types::ModelFamily`] rather than holding
//! its own `String` (see "A wrapper, not a re-export" below), and this
//! module's doc comment no longer calls escalation 6 unresolved.
//!
//! One thing does not follow, and is not done: `AgentIdentity::create` does
//! not yet take a `family` parameter or call the two functions below.
//! `AgentIdentity::create` is called at 12 existing, unmodified call sites (9
//! in `identity.rs`'s own tests, 2 in `keychain.rs`'s, 1 in `issuance.rs`'s);
//! adding a required parameter changes the arity of every one, and no design
//! that leaves them compiling can supply a family at those call sites without
//! either making the field optional or defaulting it, both the fail-open
//! shape this module exists to prevent. This is CLAUDE.md absolute rule 3 (no
//! existing test is modified), reported as this ticket's stop point rather
//! than applied.
//!
//! # A wrapper, not a re-export
//!
//! [`ModelFamily`] here is `ModelFamily(ori_core::types::ModelFamily)`, not a
//! `pub use` of the core type. A `pub use` was tried first, per this ticket's
//! instructions, and rejected: [`ModelFamily::declare`] must return
//! [`FamilyError`], and Rust's orphan rule lets only the crate that defines a
//! type add an inherent impl to it, so once the type is defined in
//! `ori-core`, only `ori-core` can define an `impl ModelFamily { .. }` block
//! at all, on any name. `ori-core` cannot return `ori-broker`'s `FamilyError`
//! (`ori-core` depends on nothing in the workspace), so a re-exported type
//! cannot carry a `declare` that returns `FamilyError`, and this module's own
//! test `tests::ori_t_0028_model_family_declare_refuses_empty_or_whitespace` (which
//! this ticket does not modify) matches its result against exactly that type.
//! Wrapping keeps `ModelFamily` locally defined here, so `declare` stays legal
//! to define, while [`ModelFamily::as_core`] and [`ModelFamily::from_core`]
//! are the seam `identity.rs` would use, once ORI-T-0108's stop point is
//! resolved, to move a value between the type `AgentIdentity` stores and the
//! type these functions compare.
//!
//! # Where the family comes from, and why this module never infers one
//!
//! ADR-0001 rejected exact model ids for exactly this check: "a version bump
//! or a renamed snapshot of the same model reads as a different model, and
//! the separation is lost without anyone noticing." A family parsed out of a
//! model string (a prefix before a dash, a lookup table of known names) is
//! the same defect wearing a disguise: it guesses lineage instead of reading
//! a declared one, and silently mis-groups the first model name nobody
//! anticipated. So [`ModelFamily`] is never derived from
//! [`crate::identity::AgentIdentity::model`] anywhere in this module; the
//! only way to obtain one is [`ModelFamily::declare`], which a caller reaches
//! for with a value it already holds, never with a value this module worked
//! out for it.
//!
//! ADR-0001's decision row says the family is "declared by each runtime
//! adapter through its `RuntimeCaps` and recorded on the `ProviderBinding`".
//! Neither half of that sentence is buildable from this ticket's declared
//! scope today:
//!
//! - `RuntimeCaps` names nothing in this workspace (checked directly: no
//!   type, trait or field of that name exists anywhere under `crates/`). The
//!   one place it is named at all is `spec/API_SPEC.md`'s sketch of
//!   `AgentRuntime::capabilities`, which lives in `ori-runtime`, a crate
//!   `spec/LLD.md` section 2's dependency diagram has depending on
//!   `ori-broker`, not the other way around; this module could not call into
//!   it even if the type existed.
//! - `ProviderBinding` (`crates/ori-broker/src/keychain.rs`) carries no
//!   `model` field, only `product_id` and `provider`, so two identities on
//!   the same provider but different models would resolve to the same
//!   binding and be refused wrongly were the family read from there.
//!   `ops/phase-1-backlog.md`'s escalation 6 (`spec_conflict`) recorded
//!   exactly this and recommended recording the family on `AgentIdentity`
//!   instead; the operator's ruling on that escalation adopted the
//!   recommendation, which is ORI-T-0108's mandate ("the ruling I
//!   implement" in that ticket's own text). The escalation is resolved as of
//!   ORI-T-0108; what is not yet done is wiring `AgentIdentity::create` to
//!   read it, for the reason "ORI-T-0108: what moved, and what is still not
//!   wired in" above states, so as of this ticket this module still edits
//!   neither `identity.rs` nor `keychain.rs`.
//!
//! The first bullet (`RuntimeCaps`) is unaffected by the ruling and remains a
//! `precondition_missing` finding, carried in this ticket's report (the
//! `aicd_escalate` tool named in CLAUDE.md's escalation table does not exist
//! in this repository, escalation E-0005). What this module builds instead,
//! and what does not depend on `RuntimeCaps` existing: [`ModelFamily`], a
//! typed, caller-declared value (never a bare [`String`], so a caller cannot
//! pass an unvalidated one by accident), and the two comparison functions
//! below, which take every family they compare as an explicit parameter and
//! store nothing. A future ticket that wires `AgentIdentity::create` to call
//! these functions reaches a real source for `coder_families` /
//! `lead_families` (`RuntimeCaps` once it exists is still how a runtime
//! adapter's own declaration would reach a caller; what "the coders it
//! reviews" reads from is a separate, larger question this ticket's report
//! addresses directly) through to these same functions without this module
//! changing: the seam is the function boundary, not a field this module
//! reads.
//!
//! # The undeclared family: refused, never compared as distinct
//!
//! [`refuse_lead_sharing_family_with_coders`] and
//! [`refuse_coder_sharing_family_with_leads`] each take every family (the
//! identity being created, and every identity on the other side of the
//! lead/coder boundary already known to the caller) as `Option<&ModelFamily>`,
//! because a caller that has no `RuntimeCaps` to ask, today, may honestly
//! have nothing to pass. Reading `None` as "no family declared, therefore
//! different from any [`Some`]" is the vacuous pass CLAUDE.md's own text for
//! this ticket names directly, and the dominant defect class of AICD §39,
//! "present but reporting nothing": the check would run, report success, and
//! mean nothing, on every input where a declaration is simply missing, which
//! is every input this workspace can produce until `RuntimeCaps` exists.
//! `None` on either side of a comparison here refuses, with the same
//! [`FamilyRefusalKind::Undeclared`] reason a shared family carries, rather
//! than ever being treated as "different". An identity this module is asked
//! to admit therefore proves it differs from everyone on the other side of
//! the boundary; it never gets to default into being taken at its word.
//!
//! # A lead with no coders yet
//!
//! [`refuse_lead_sharing_family_with_coders`] with an empty `coder_families`
//! slice: the loop that would find a shared family never runs, so the call
//! succeeds, provided the lead's own family is declared (still checked
//! first, unconditionally, per the paragraph above). This is a deliberate
//! choice, not an oversight: refusing the very first identity of a lineage
//! because nothing yet exists to compare it against would make it
//! impossible to ever create a first lead or a first coder at all, which
//! nothing in ADR-0001 or criterion ORI-P1-035 asks for.
//!
//! It is not the loophole the ADR guards against, because the loophole would
//! need an *undeclared* family to be let through and then relied on later:
//! this module refuses that unconditionally, regardless of whether there is
//! anyone yet to compare against. What lets the invariant hold once a second
//! identity does arrive is that the first identity's family was declared and
//! recorded (by whatever caller holds the list this module is handed), so
//! the reverse-path check below always has a real, non-empty family to
//! compare the second identity against; it is never comparing against an
//! entry this module let through unproven.
//!
//! # The reverse path: enforced, not only the lead's creation
//!
//! Criterion ORI-P1-035 exercises only "`broker.identity.create` for the
//! lead". ADR-0001's own invariant is symmetric: "A lead identity and the
//! coders it reviews never share a model family", which a coder created
//! *after* the lead, on the lead's family, breaks exactly as surely as the
//! order ORI-P1-035 tests. [`refuse_coder_sharing_family_with_leads`] is that
//! direction, built and tested under its own name
//! (`tests::ori_t_0028_reverse_path_*`) rather than left as a gap ORI-P1-035
//! does not happen to probe.
//!
//! # Flow
//!
//! ```mermaid
//! flowchart TD
//!   subgraph create_lead["broker.identity.create, role = lead"]
//!     L1["lead_family: Option<&ModelFamily>"] --> L2{"declared?"}
//!     L2 -- no --> LR["Refused: Undeclared, AICD §7"]
//!     L2 -- yes --> L3{"shares a family with\nany coder it reviews?"}
//!     L3 -- "yes (incl. undeclared coder)" --> LR2["Refused: SharedFamily\nor Undeclared, AICD §7"]
//!     L3 -- no --> LOK["Identity created"]
//!   end
//!   subgraph create_coder["broker.identity.create, role = coder (reverse path)"]
//!     C1["coder_family: Option<&ModelFamily>"] --> C2{"declared?"}
//!     C2 -- no --> CR["Refused: Undeclared, AICD §7"]
//!     C2 -- yes --> C3{"shares a family with\nany lead reviewing it?"}
//!     C3 -- "yes (incl. undeclared lead)" --> CR2["Refused: SharedFamily\nor Undeclared, AICD §7"]
//!     C3 -- no --> COK["Identity created"]
//!   end
//! ```
//!
//! Must not: read [`crate::identity::AgentIdentity::model`] as a source of
//! family, or modify `identity.rs` or `keychain.rs` (this ticket's own
//! instructions). Nothing here does either.

use core::fmt;

use ori_core::error::MethodologyRef;

// ---------------------------------------------------------------------------
// ModelFamily
// ---------------------------------------------------------------------------

/// A declared model family: ADR-0001 "Model family", "the provider's own
/// grouping of models sharing a lineage; stable across version changes, and
/// the coarsest unit at which two reviewers are genuinely different."
///
/// A wrapper around [`ori_core::types::ModelFamily`], the value type
/// ORI-T-0108 moved into `ori-core` (this module's own doc comment, "A
/// wrapper, not a re-export", says why this is a wrapper and not that type
/// re-exported under this name). Built only by [`ModelFamily::declare`],
/// which a caller reaches for with a value it already holds (from a runtime
/// adapter's `RuntimeCaps`, once that exists, or any other declared source),
/// never one this module infers from a model id string; see this module's own
/// doc comment, "Where the family comes from". Holding this as a typed
/// newtype rather than a bare [`String`] is itself part of that: a function
/// that took `&str` here could be handed an unvalidated, un-declared-on-purpose
/// value at any call site, while a function that takes `&ModelFamily` can only
/// be handed one that passed [`ModelFamily::declare`]'s refusal of an empty or
/// whitespace-only value.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ModelFamily(ori_core::types::ModelFamily);

impl ModelFamily {
    /// Declares a model family, refusing one that is empty or only
    /// whitespace, the same shape [`crate::identity::AgentIdentity::create`]
    /// uses for `model`. Delegates the validation itself to
    /// [`ori_core::types::ModelFamily::parse`], so there is exactly one place
    /// in this workspace that decides what a well-formed family looks like.
    ///
    /// # Errors
    ///
    /// [`FamilyError::Malformed`] when `family` is empty or only whitespace.
    pub fn declare(family: impl Into<String>) -> Result<Self, FamilyError> {
        let family = family.into();
        ori_core::types::ModelFamily::parse(&family)
            .map(Self)
            .map_err(|_| FamilyError::Malformed {
                what: "model family",
                value: family,
            })
    }

    /// The family as declared text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Wraps an already-validated [`ori_core::types::ModelFamily`] (the type
    /// [`crate::identity::AgentIdentity`] would store, per the operator's
    /// ruling on escalation 6) so it can be passed to
    /// [`refuse_lead_sharing_family_with_coders`] or
    /// [`refuse_coder_sharing_family_with_leads`]. Infallible: a value of the
    /// core type already passed the one validation this module would apply.
    #[must_use]
    pub const fn from_core(family: ori_core::types::ModelFamily) -> Self {
        Self(family)
    }

    /// The core value type this family wraps, the type
    /// [`crate::identity::AgentIdentity`] would store, per the operator's
    /// ruling on escalation 6.
    #[must_use]
    pub const fn as_core(&self) -> &ori_core::types::ModelFamily {
        &self.0
    }

    /// Consumes this value for the core type it wraps.
    #[must_use]
    pub fn into_core(self) -> ori_core::types::ModelFamily {
        self.0
    }
}

impl fmt::Display for ModelFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

// ---------------------------------------------------------------------------
// FamilyError
// ---------------------------------------------------------------------------

/// Why a model family declaration, or the cross-model check, did not
/// succeed.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FamilyError {
    /// A value did not parse into the type or shape named by `what`. Not a
    /// refusal in the [`ori_core::error::Error::Refused`] sense (a control
    /// refusing a governed action): input that failed to parse carries no
    /// [`MethodologyRef`], the same split `crate::identity::IdentityError`
    /// and `crate::keychain::KeychainError` both draw for themselves.
    Malformed {
        /// The field or type the value was read as.
        what: &'static str,
        /// The value as it was given.
        value: String,
    },
    /// `broker.identity.create` refused the identity: ADR-0001 "Model
    /// family", AICD §7, criterion ORI-P1-035. Always carries `reason`, so a
    /// caller (and a test) can inspect the methodology section this refusal
    /// is made under rather than only that a refusal occurred.
    Refused {
        /// Which shape of refusal this is.
        kind: FamilyRefusalKind,
        /// AICD §7, always: see [`FamilyRefusalKind::reason`].
        reason: MethodologyRef,
    },
}

impl fmt::Display for FamilyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { what, value } => write!(f, "not a valid {what}: {value:?}"),
            Self::Refused { kind, reason } => write!(f, "{kind} ({reason})"),
        }
    }
}

impl std::error::Error for FamilyError {}

// ---------------------------------------------------------------------------
// FamilyRefusalKind
// ---------------------------------------------------------------------------

/// Which shape of cross-model refusal this is: ADR-0001 "Model family",
/// AICD §7.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FamilyRefusalKind {
    /// The identity being created and an identity on the other side of the
    /// lead/coder boundary declare the same model family.
    SharedFamily {
        /// The family both identities declare.
        family: String,
    },
    /// A model family this check needed was not declared: either the
    /// identity being created, or an identity already on the other side of
    /// the boundary. See this module's own doc comment, "The undeclared
    /// family: refused, never compared as distinct", for why this is a
    /// refusal and not a pass.
    Undeclared,
}

impl FamilyRefusalKind {
    /// The methodology section this refusal is made under: AICD §7, "The
    /// roles", which is the section ADR-0001's "Model family" decision cites
    /// as the reason this separation exists ("because AICD §7 separates a
    /// lead from its coders at exactly this level"). Both variants cite the
    /// same section: an undeclared family is refused for the same reason a
    /// shared one is, because this module cannot show the separation AICD §7
    /// requires holds without a declared value on both sides to compare.
    ///
    /// Constructed by field literal, not [`MethodologyRef::at`]: section 7 is
    /// a fixed, always-valid constant here, known at the call site, and
    /// `spec/CONVENTIONS.md`'s "no `unwrap`, `expect` or `panic!` outside
    /// tests and the binary entry points" forbids the alternative of calling
    /// the fallible constructor and unwrapping its `Result`. [`MethodologyRef`]'s
    /// fields are public for exactly this: `spec/LLD.md` section 4 fixes the
    /// shape as a plain struct, so a value already known to resolve can be
    /// built directly.
    #[must_use]
    pub const fn reason(&self) -> MethodologyRef {
        MethodologyRef {
            section: 7,
            subsection: None,
        }
    }
}

impl fmt::Display for FamilyRefusalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SharedFamily { family } => {
                write!(
                    f,
                    "a lead and a coder it reviews both declare the model family {family:?}"
                )
            }
            Self::Undeclared => f.write_str("a model family this check needed was not declared"),
        }
    }
}

// ---------------------------------------------------------------------------
// The checks
// ---------------------------------------------------------------------------

/// Refuses creating a lead identity whose declared model family equals the
/// declared family of any coder it reviews: ADR-0001 "Model family", AICD
/// §7, criterion ORI-P1-035.
///
/// `lead_family` is the family declared for the lead identity being created;
/// `coder_families` is the declared family of every coder identity the lead
/// reviews, in whatever order the caller holds them (a caller that has no
/// coders yet declared passes an empty slice; see this module's own doc
/// comment, "A lead with no coders yet"). Both are `Option` because a caller
/// without a `RuntimeCaps` to ask may have no family to give; see "The
/// undeclared family" above for why `None` refuses rather than compares as
/// distinct.
///
/// # Errors
///
/// [`FamilyError::Refused`] with [`FamilyRefusalKind::Undeclared`] when
/// `lead_family` or any entry of `coder_families` is `None`;
/// [`FamilyError::Refused`] with [`FamilyRefusalKind::SharedFamily`] when a
/// declared `lead_family` equals a declared entry of `coder_families`.
pub fn refuse_lead_sharing_family_with_coders(
    lead_family: Option<&ModelFamily>,
    coder_families: &[Option<&ModelFamily>],
) -> Result<(), FamilyError> {
    refuse_shared_family_across_the_boundary(lead_family, coder_families)
}

/// The reverse path: refuses creating a coder identity whose declared model
/// family equals the declared family of any lead that reviews it. ADR-0001's
/// invariant is symmetric ("A lead identity and the coders it reviews never
/// share a model family"), and a coder created after its lead, on the
/// lead's family, breaks it exactly as surely as the order criterion
/// ORI-P1-035 exercises; see this module's own doc comment, "The reverse
/// path", for why this is built and tested even though no accepted
/// criterion names it.
///
/// `coder_family` is the family declared for the coder identity being
/// created; `lead_families` is the declared family of every lead that
/// reviews it. Both `Option`, for the same reason
/// [`refuse_lead_sharing_family_with_coders`]'s parameters are.
///
/// # Errors
///
/// The same two shapes [`refuse_lead_sharing_family_with_coders`] returns,
/// with the roles reversed.
pub fn refuse_coder_sharing_family_with_leads(
    coder_family: Option<&ModelFamily>,
    lead_families: &[Option<&ModelFamily>],
) -> Result<(), FamilyError> {
    refuse_shared_family_across_the_boundary(coder_family, lead_families)
}

/// The comparison both directions share: `candidate` is the identity being
/// created, `existing` is every identity already known on the other side of
/// the lead/coder boundary. Pure: reads its parameters, decides nothing
/// about a keychain, a store or an event.
fn refuse_shared_family_across_the_boundary(
    candidate: Option<&ModelFamily>,
    existing: &[Option<&ModelFamily>],
) -> Result<(), FamilyError> {
    let Some(candidate_family) = candidate else {
        return Err(FamilyError::Refused {
            kind: FamilyRefusalKind::Undeclared,
            reason: FamilyRefusalKind::Undeclared.reason(),
        });
    };
    for other in existing.iter().copied() {
        let Some(other_family) = other else {
            return Err(FamilyError::Refused {
                kind: FamilyRefusalKind::Undeclared,
                reason: FamilyRefusalKind::Undeclared.reason(),
            });
        };
        if other_family == candidate_family {
            let kind = FamilyRefusalKind::SharedFamily {
                family: candidate_family.as_str().to_owned(),
            };
            let reason = kind.reason();
            return Err(FamilyError::Refused { kind, reason });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn family(text: &str) -> ModelFamily {
        ModelFamily::declare(text).expect("a non-empty family declares")
    }

    // -----------------------------------------------------------------------
    // ModelFamily::declare
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0028_model_family_declare_refuses_empty_or_whitespace() {
        for bad in ["", "   ", "\t\n"] {
            let err = ModelFamily::declare(bad).expect_err("an empty family is refused");
            assert!(matches!(
                err,
                FamilyError::Malformed {
                    what: "model family",
                    ..
                }
            ));
        }
    }

    #[test]
    fn ori_t_0028_model_family_declare_trims_and_compares_by_value() {
        let a = ModelFamily::declare("  claude-3  ").expect("a padded family declares");
        let b = ModelFamily::declare("claude-3").expect("the trimmed equivalent declares");
        assert_eq!(a.as_str(), "claude-3");
        assert_eq!(a, b);
    }

    // -----------------------------------------------------------------------
    // ORI-P1-035: "Lead identity on the same model as its coder |
    // broker.identity.create for the lead | Refused, reason cites AICD §7"
    // -----------------------------------------------------------------------

    #[test]
    fn ori_p1_035_lead_refused_when_a_coder_declares_the_same_family() {
        let lead = family("anthropic-claude-3");
        let coder = family("anthropic-claude-3");
        let err = refuse_lead_sharing_family_with_coders(Some(&lead), &[Some(&coder)])
            .expect_err("a lead sharing its coder's family is refused");
        assert!(matches!(
            err,
            FamilyError::Refused {
                kind: FamilyRefusalKind::SharedFamily { .. },
                ..
            }
        ));
    }

    /// The trap this criterion invites: a check that compared exact model
    /// ids would pass two different ids as "different", exactly what
    /// ADR-0001 rejected ("a version bump or a renamed snapshot of the same
    /// model reads as a different model, and the separation is lost without
    /// anyone noticing"). `refuse_lead_sharing_family_with_coders` never
    /// takes a model id at all, only a declared [`ModelFamily`], so it
    /// cannot make that comparison by construction; this test still proves
    /// the outcome directly, the way two runtime adapters would declare it:
    /// a lead on what would be `claude-3-opus-20240229` and a coder on what
    /// would be `claude-3-opus-20240307`, two different model ids, both
    /// declaring the same family.
    #[test]
    fn ori_p1_035_lead_refused_when_coder_shares_the_family_despite_different_model_ids() {
        let lead_on_a_february_snapshot = family("anthropic-claude-3-opus");
        let coder_on_a_march_snapshot = family("anthropic-claude-3-opus");
        let err = refuse_lead_sharing_family_with_coders(
            Some(&lead_on_a_february_snapshot),
            &[Some(&coder_on_a_march_snapshot)],
        )
        .expect_err(
            "two different model ids in the same family must still be refused as the same family",
        );
        assert_eq!(
            err,
            FamilyError::Refused {
                kind: FamilyRefusalKind::SharedFamily {
                    family: "anthropic-claude-3-opus".to_owned(),
                },
                reason: MethodologyRef {
                    section: 7,
                    subsection: None,
                },
            }
        );
    }

    #[test]
    fn ori_p1_035_refusal_cites_aicd_section_7() {
        let lead = family("anthropic-claude-3");
        let coder = family("anthropic-claude-3");
        let err = refuse_lead_sharing_family_with_coders(Some(&lead), &[Some(&coder)])
            .expect_err("same family is refused");
        let FamilyError::Refused { reason, .. } = err else {
            panic!("expected a Refused error, got {err:?}");
        };
        assert_eq!(reason.section, 7);
        assert_eq!(reason.subsection, None);
        assert_eq!(reason.to_string(), "AICD §7");
    }

    #[test]
    fn ori_p1_035_lead_allowed_when_the_coders_family_differs() {
        let lead = family("anthropic-claude-3");
        let coder = family("openai-gpt-4");
        refuse_lead_sharing_family_with_coders(Some(&lead), &[Some(&coder)])
            .expect("distinct declared families are allowed");
    }

    #[test]
    fn ori_p1_035_lead_refused_when_any_one_of_several_coders_shares_its_family() {
        let lead = family("anthropic-claude-3");
        let other_coder = family("openai-gpt-4");
        let same_family_coder = family("anthropic-claude-3");
        let err = refuse_lead_sharing_family_with_coders(
            Some(&lead),
            &[Some(&other_coder), Some(&same_family_coder)],
        )
        .expect_err("one shared family among several coders is still refused");
        assert!(matches!(
            err,
            FamilyError::Refused {
                kind: FamilyRefusalKind::SharedFamily { .. },
                ..
            }
        ));
    }

    // -----------------------------------------------------------------------
    // The undeclared family: refused, never compared as distinct.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0028_lead_creation_refused_when_the_leads_own_family_is_undeclared() {
        let coder = family("anthropic-claude-3");
        let err = refuse_lead_sharing_family_with_coders(None, &[Some(&coder)])
            .expect_err("an undeclared lead family is refused, not treated as distinct");
        assert!(matches!(
            err,
            FamilyError::Refused {
                kind: FamilyRefusalKind::Undeclared,
                ..
            }
        ));
    }

    #[test]
    fn ori_t_0028_lead_creation_refused_when_an_existing_coders_family_is_undeclared() {
        let lead = family("anthropic-claude-3");
        let err = refuse_lead_sharing_family_with_coders(Some(&lead), &[None])
            .expect_err("an undeclared coder family is refused, not treated as distinct");
        assert!(matches!(
            err,
            FamilyError::Refused {
                kind: FamilyRefusalKind::Undeclared,
                ..
            }
        ));
    }

    #[test]
    fn ori_t_0028_an_undeclared_family_never_reads_as_different_from_an_empty_string() {
        // The trap named directly: were the family type a bare `String`
        // rather than `ModelFamily`, an empty string could stand in for
        // "undeclared" and `"" != "anthropic-claude-3"` would read as
        // "different, allowed". ModelFamily::declare refuses an empty
        // string outright, so it can never reach this comparison at all;
        // the only way to express "unknown" is `None`, which this module
        // refuses rather than compares.
        assert!(ModelFamily::declare("").is_err());
        let lead = family("anthropic-claude-3");
        let err = refuse_lead_sharing_family_with_coders(Some(&lead), &[None])
            .expect_err("None must not be treated as distinct from any declared family");
        assert!(matches!(
            err,
            FamilyError::Refused {
                kind: FamilyRefusalKind::Undeclared,
                ..
            }
        ));
    }

    // -----------------------------------------------------------------------
    // A lead with no coders yet.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0028_lead_with_no_coders_yet_is_allowed_when_its_own_family_is_declared() {
        let lead = family("anthropic-claude-3");
        refuse_lead_sharing_family_with_coders(Some(&lead), &[])
            .expect("a declared lead family with no coders yet declared is allowed");
    }

    #[test]
    fn ori_t_0028_lead_with_no_coders_yet_is_still_refused_if_its_own_family_is_undeclared() {
        // The loophole this guards: allowing an undeclared lead through
        // "because nothing exists yet to compare it against" would leave
        // no recorded family for a later coder-creation check to compare
        // against, silently letting a same-family pair through once a
        // coder does arrive. Refusing here, unconditionally, closes that
        // before it can open.
        let err = refuse_lead_sharing_family_with_coders(None, &[])
            .expect_err("an undeclared family is refused even with nobody yet to compare to");
        assert!(matches!(
            err,
            FamilyError::Refused {
                kind: FamilyRefusalKind::Undeclared,
                ..
            }
        ));
    }

    // -----------------------------------------------------------------------
    // The reverse path: a coder created after its lead.
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0028_reverse_path_coder_refused_when_it_shares_its_leads_family() {
        let existing_lead = family("anthropic-claude-3");
        let new_coder = family("anthropic-claude-3");
        let err = refuse_coder_sharing_family_with_leads(Some(&new_coder), &[Some(&existing_lead)])
            .expect_err("a coder created on its lead's family is refused");
        assert!(matches!(
            err,
            FamilyError::Refused {
                kind: FamilyRefusalKind::SharedFamily { .. },
                ..
            }
        ));
    }

    #[test]
    fn ori_t_0028_reverse_path_refusal_also_cites_aicd_section_7() {
        let existing_lead = family("anthropic-claude-3");
        let new_coder = family("anthropic-claude-3");
        let err = refuse_coder_sharing_family_with_leads(Some(&new_coder), &[Some(&existing_lead)])
            .expect_err("shared family is refused");
        let FamilyError::Refused { reason, .. } = err else {
            panic!("expected a Refused error, got {err:?}");
        };
        assert_eq!(reason.section, 7);
    }

    #[test]
    fn ori_t_0028_reverse_path_coder_allowed_when_its_family_differs_from_every_lead() {
        let existing_lead = family("anthropic-claude-3");
        let new_coder = family("openai-gpt-4");
        refuse_coder_sharing_family_with_leads(Some(&new_coder), &[Some(&existing_lead)])
            .expect("a coder whose family differs from every reviewing lead is allowed");
    }

    #[test]
    fn ori_t_0028_reverse_path_coder_refused_when_its_own_family_is_undeclared() {
        let existing_lead = family("anthropic-claude-3");
        let err = refuse_coder_sharing_family_with_leads(None, &[Some(&existing_lead)])
            .expect_err("an undeclared coder family is refused, not treated as distinct");
        assert!(matches!(
            err,
            FamilyError::Refused {
                kind: FamilyRefusalKind::Undeclared,
                ..
            }
        ));
    }

    #[test]
    fn ori_t_0028_reverse_path_coder_refused_when_a_reviewing_leads_family_is_undeclared() {
        let new_coder = family("anthropic-claude-3");
        let err = refuse_coder_sharing_family_with_leads(Some(&new_coder), &[None])
            .expect_err("an undeclared lead family is refused, not treated as distinct");
        assert!(matches!(
            err,
            FamilyError::Refused {
                kind: FamilyRefusalKind::Undeclared,
                ..
            }
        ));
    }

    #[test]
    fn ori_t_0028_coder_with_no_leads_yet_is_allowed_when_its_own_family_is_declared() {
        let coder = family("anthropic-claude-3");
        refuse_coder_sharing_family_with_leads(Some(&coder), &[])
            .expect("a declared coder family with no reviewing leads yet is allowed");
    }

    // -----------------------------------------------------------------------
    // Display
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0028_family_error_display_names_the_shared_family() {
        let lead = family("anthropic-claude-3");
        let coder = family("anthropic-claude-3");
        let err = refuse_lead_sharing_family_with_coders(Some(&lead), &[Some(&coder)])
            .expect_err("same family is refused");
        let text = err.to_string();
        assert!(text.contains("anthropic-claude-3"));
        assert!(text.contains("AICD §7"));
    }

    #[test]
    fn ori_t_0028_family_error_display_names_the_undeclared_case() {
        let lead = family("anthropic-claude-3");
        let err = refuse_lead_sharing_family_with_coders(Some(&lead), &[None])
            .expect_err("undeclared coder family is refused");
        let text = err.to_string();
        assert!(text.contains("not declared"));
        assert!(text.contains("AICD §7"));
    }
}
