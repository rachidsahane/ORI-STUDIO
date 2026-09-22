//! The error enum whose refusals carry a methodology-section reason
//! (`spec/LLD.md` section 2), and the reference they carry.
//!
//! # What this module owes
//!
//! Criterion ORI-P1-033 in `spec/criteria/phase-1.md`: given any refusal by the
//! engine, inspecting the error shows a `MethodologyRef` "with a section that
//! resolves in the methodology index". `spec/LLD.md` section 4 fixes the shape:
//! "Every error that refuses an action includes `MethodologyRef { section: u8,
//! subsection: Option<String> }` so the UI can show the 'explain' action". PRD
//! A-10 is the feature that reads it.
//!
//! The shape is held here exactly as `spec/LLD.md` section 4 writes it. What is
//! added is a constructor that refuses a section the methodology does not carry,
//! so that an unresolvable reference cannot be built by accident, and
//! [`RefusalKind`], which gives every refusal this crate can make one reason
//! chosen once rather than at each call site.
//!
//! # Why the index is duplicated here rather than read
//!
//! "Resolves in the methodology index" means `methodology/sections.json`, which
//! `crates/ori-gates/src/sections.rs` generates from the methodology document.
//! This crate may not read a file and may not import another workspace crate
//! (`spec/LLD.md` section 2, and CLAUDE.md's load-bearing facts), so it cannot
//! consult that index. [`SECTION_COUNT`] and [`NUMBERED_SUBSECTIONS`] therefore
//! restate what the index holds, and the restatement is what the tests here
//! check refusals against. A restatement that drifted would leave every test in
//! this module green while the criterion they serve was false, so the
//! restatement is itself checked, and necessarily not from here.
//!
//! # What checks the restatement, and where
//!
//! The comparison lives in the one place allowed to read both sides:
//! `ori_p1_033_every_restatement_of_the_index_agrees_with_the_index`, in
//! `crates/ori-gates/src/sections.rs`. That crate parses
//! `methodology/AICD_Methodology_v0.3.html` and reads the two constants below
//! out of this file's Rust source as text, which is how it compares them
//! without either side importing the other. The generated
//! `methodology/sections.json` is tied to the same parse by
//! `committed_sections_json_matches_a_fresh_parse_of_the_methodology` in that
//! same file, so agreeing with the parse is agreeing with the index.
//!
//! When the two disagree that test fails and names every disagreement it found,
//! not the first: a [`SECTION_COUNT`] above what the document carries as the
//! references it would admit that resolve against no heading, one below it as
//! the headings no refusal could then cite, and a [`NUMBERED_SUBSECTIONS`]
//! entry as a heading this file omits or a pair the document has no heading
//! for. Renaming, removing, duplicating or reshaping either constant fails it
//! too, rather than reading as agreement: a reader that finds nothing and
//! reports a pass is the "present but reporting nothing" defect of AICD §39.
//! A second restatement elsewhere in the repository fails it until registered
//! there. All of it runs with the workspace test suite, gate 2 of
//! `spec/CI_CD.md` section 1.
//!
//! # What that check does not cover
//!
//! The appendix subsections `A.1` to `A.5` sit outside the comparison by
//! design: `spec/LLD.md` section 4 types `MethodologyRef::section` as `u8`, so
//! no appendix reference can be built, and the comparison drops index entries
//! whose section is a letter. Nothing mechanically confirms that exclusion is
//! still the right one; the doc comment on [`NUMBERED_SUBSECTIONS`] carries the
//! ruling it rests on, and a later methodology that numbered subsections
//! outside section 24 and appendix A would need that ruling revisited.
//!
//! This prose is not checked either, beyond its `AICD §<n>` citations resolving
//! against the index, which
//! `citations_resolve_everywhere_but_the_recorded_design_artifact` does for
//! every file in the repository. That scan reads citations and never reads the
//! constants, so it cannot stand in for the comparison above. These paragraphs
//! replace ones that outlived their subject and went on describing the gap
//! after it had been closed, which is AICD §39's defect class seen from the
//! other side: a claim about what is verified that is the opposite of the
//! truth.
//!
//! # Why the enum is written out rather than derived
//!
//! `spec/CONVENTIONS.md` names `thiserror` as the house error convention. Ruling
//! R20 in `ops/rulings.md` defers adopting it and states what stands until then:
//! "a hand-written `Display` and `std::error::Error` implementation stands, with
//! a comment naming the conversion". This is that implementation. Converting it
//! is mechanical: each [`Display`](fmt::Display) arm below becomes a
//! `#[error("...")]` attribute on its variant.
//!
//! Must not: do IO, or import any other workspace crate (`spec/LLD.md` section
//! 2).

use core::fmt;

use crate::types::Category;
use crate::types::DocumentState;
use crate::types::Seat;
use crate::types::TicketState;
use crate::types::Tier;

/// The result this crate returns.
///
/// No methodology section applies: this is the ordinary Rust alias for
/// [`Error`], not an implementation of a methodology rule.
pub type Result<T> = core::result::Result<T, Error>;

/// How many numbered sections the methodology carries.
///
/// `methodology/sections.json` reports `counts.section` as 40 for
/// `methodology/AICD_Methodology_v0.3.html`, the version under `methodology/`
/// that `spec/LLD.md` section 1 registers. A reference above this resolves
/// against nothing, which is the defect class AICD §39 records as "fabricated
/// section references".
pub const SECTION_COUNT: u8 = 40;

/// Every subsection the methodology numbers, as (section, subsection).
///
/// Ruling R1 in `ops/rulings.md` settles the granularity: the methodology
/// carries numbered subsections in exactly two places, `AICD §24.1` to
/// `AICD §24.8` and appendix `A.1` to `A.5`. Only the first can be written as a
/// [`MethodologyRef`], because `spec/LLD.md` section 4 types `section` as `u8`
/// and an appendix is a letter. A refusal citing an appendix cannot be
/// expressed by this type; none does.
pub const NUMBERED_SUBSECTIONS: &[(u8, &str)] = &[
    (24, "1"),
    (24, "2"),
    (24, "3"),
    (24, "4"),
    (24, "5"),
    (24, "6"),
    (24, "7"),
    (24, "8"),
];

/// The methodology section a refusal is made under: AICD §39.
///
/// Derived from AICD §39, which records the failure that made references
/// checkable ("four drafting agents produced eleven fabricated section
/// references") and the rule adopted from it, "References are checked
/// mechanically. The methodology's section index is machine-readable". This type
/// is that reference in code rather than in prose, and criterion ORI-P1-033 is
/// what requires every refusal to carry one.
///
/// The fields are public because `spec/LLD.md` section 4 writes the type as
/// `MethodologyRef { section: u8, subsection: Option<String> }`, and that shape
/// is a contract other crates and the client API read. [`MethodologyRef::at`]
/// and [`MethodologyRef::within`] are the constructors that cannot build an
/// unresolvable reference, and [`MethodologyRef::resolves`] answers for a value
/// built any other way.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MethodologyRef {
    /// The numbered section, 1 to [`SECTION_COUNT`].
    pub section: u8,
    /// The numbered subsection within it, when the reference names one.
    pub subsection: Option<String>,
}

impl MethodologyRef {
    /// A reference to a whole section, refusing a number the methodology does
    /// not carry.
    pub fn at(section: u8) -> Result<Self> {
        let candidate = Self {
            section,
            subsection: None,
        };
        if candidate.resolves() {
            Ok(candidate)
        } else {
            Err(Error::malformed(
                "MethodologyRef section",
                section.to_string(),
            ))
        }
    }

    /// A reference to a numbered subsection, refusing a pair the methodology
    /// does not carry.
    pub fn within(section: u8, subsection: &str) -> Result<Self> {
        let candidate = Self {
            section,
            subsection: Some(subsection.to_owned()),
        };
        if candidate.resolves() {
            Ok(candidate)
        } else {
            Err(Error::malformed(
                "MethodologyRef subsection",
                format!("{section}.{subsection}"),
            ))
        }
    }

    /// Whether this reference names a heading the methodology carries, which is
    /// what criterion ORI-P1-033 requires of every refusal.
    #[must_use]
    pub fn resolves(&self) -> bool {
        if self.section == 0 || self.section > SECTION_COUNT {
            return false;
        }
        match &self.subsection {
            None => true,
            Some(subsection) => NUMBERED_SUBSECTIONS
                .iter()
                .any(|(parent, numbered)| *parent == self.section && *numbered == subsection),
        }
    }
}

impl fmt::Display for MethodologyRef {
    /// Writes the reference in the one form `spec/CONVENTIONS.md` admits:
    /// "Every methodology reference is `AICD §<n>` and must resolve".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AICD §{}", self.section)?;
        match &self.subsection {
            Some(subsection) => write!(f, ".{subsection}"),
            None => Ok(()),
        }
    }
}

/// A refusal the engine can make from this crate, and the reason it carries.
///
/// Each value is one control refusing one action. The reason is attached here,
/// once, rather than at each call site, so that two call sites cannot cite two
/// different sections for the same refusal.
///
/// This list grows as the controls arrive. It holds no permission refusal yet:
/// the permission function is ORI-T-0022, and the resource and action types its
/// decision needs are its to design, not this ticket's to guess.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum RefusalKind {
    /// A ticket was moved between two states its lifecycle does not connect.
    TicketTransition {
        /// Where the ticket was.
        from: TicketState,
        /// Where it was asked to go.
        to: TicketState,
    },
    /// An agent tried to lower a ticket's category.
    CategoryDowngrade {
        /// The category the ticket carries.
        from: Category,
        /// The lower category that was asked for.
        to: Category,
    },
    /// An agent tried to lower a ticket's risk tier.
    TierDowngrade {
        /// The tier the ticket carries.
        from: Tier,
        /// The lower tier that was asked for.
        to: Tier,
    },
    /// A ticket was asked to close with no specification update recorded and no
    /// explicit statement that none was needed.
    SpecUpdateMissing,
    /// A defect ticket was asked to close with no accepted criterion covering
    /// it.
    CriterionMissing,
    /// A declared scope overlaps a module an in-flight ticket already claims.
    ScopeLocked {
        /// The claimed module the declaration overlaps.
        module: String,
    },
    /// A pull request modified an existing test with no escalation open.
    TestModifiedWithoutEscalation,
    /// A document was moved between two states its review cycle does not
    /// connect.
    DocumentTransition {
        /// Where the document was.
        from: DocumentState,
        /// Where it was asked to go.
        to: DocumentState,
    },
    /// A seat that does not own a document tried to sign it.
    DocumentSeatMismatch {
        /// The seat that owns the document.
        owner: Seat,
        /// The seat that tried to sign it.
        signer: Seat,
    },
}

impl RefusalKind {
    /// The methodology section this refusal is made under.
    ///
    /// Every arm names the sentence it was derived from. The sections are
    /// derived from the methodology text and from the criterion that names the
    /// refusal, never from another crate's table: rulings R10, R12 and R13 in
    /// `ops/rulings.md` record three citations taken that way which were wrong.
    #[must_use]
    pub fn reason(&self) -> MethodologyRef {
        let section = match self {
            // AICD §11's "Lifecycle" is the line a ticket walks, and
            // spec/DATA_MODEL.md section 3 draws it.
            Self::TicketTransition { .. } => 11,
            // AICD §11, "Rule: upgrade only": the lead "may upgrade a category
            // but never downgrade it. Only a human can downgrade."
            Self::CategoryDowngrade { .. } => 11,
            // AICD §11's upgrade-only rule, applied to the risk tier by ruling
            // R11 in ops/rulings.md: "only a human may lower it".
            Self::TierDowngrade { .. } => 11,
            // AICD §11's closing rule: "'Closed' requires the documentation
            // agent's specification update (or its explicit 'no change
            // needed')". Criterion ORI-P1-006 names the same section.
            Self::SpecUpdateMissing => 11,
            // AICD §16's loop: "a fix without new acceptance criteria produces a
            // bug that can return. Both steps are required for a ticket to
            // close." Criterion ORI-P1-007 names the same section.
            Self::CriterionMissing => 16,
            // AICD §12, "Conflict handling at scale": the lead "refuses to start
            // a ticket whose declared scope overlaps one already claimed".
            Self::ScopeLocked { .. } => 12,
            // AICD §12's escalation triggers: "a test had to be modified or
            // removed for the build to pass" is one of them, so a pull request
            // that modified one without an open escalation is refused.
            Self::TestModifiedWithoutEscalation => 12,
            // AICD §9: "Specification changes are pull requests, reviewed and
            // merged under the same risk tiers as code", which is the review
            // cycle spec/DATA_MODEL.md section 3 draws.
            Self::DocumentTransition { .. } => 9,
            // AICD §9's document table gives every document an owner, and
            // spec/DATA_MODEL.md section 3 states the invariant: "only the
            // owning seat may sign".
            Self::DocumentSeatMismatch { .. } => 9,
        };
        MethodologyRef {
            section,
            subsection: None,
        }
    }

    /// The error code the client API returns for this refusal, where
    /// `spec/API_SPEC.md` names one.
    ///
    /// `spec/API_SPEC.md` names exactly two, `E_UPGRADE_ONLY` (section 1,
    /// `tickets.setCategory`) and `E_SCOPE_LOCKED` (section 3,
    /// `aicd_plan_submit`). The rest return `None`. Inventing a third here would
    /// add a value to the client API, which CLAUDE.md makes an escalation with
    /// trigger `contract_change`, so each is added when `spec/API_SPEC.md` names
    /// it.
    #[must_use]
    pub const fn code(&self) -> Option<&'static str> {
        match self {
            Self::CategoryDowngrade { .. } => Some("E_UPGRADE_ONLY"),
            Self::ScopeLocked { .. } => Some("E_SCOPE_LOCKED"),
            _ => None,
        }
    }
}

impl fmt::Display for RefusalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TicketTransition { from, to } => {
                write!(f, "a ticket cannot move from {from} to {to}")
            }
            Self::CategoryDowngrade { from, to } => {
                write!(
                    f,
                    "a category may be raised and not lowered, and {to} is below {from}"
                )
            }
            Self::TierDowngrade { from, to } => {
                write!(
                    f,
                    "a risk tier may be raised and not lowered, and {to} is below {from}"
                )
            }
            Self::SpecUpdateMissing => f.write_str(
                "a ticket cannot close until its specification update, or an explicit statement \
                 that none is needed, is recorded",
            ),
            Self::CriterionMissing => f.write_str(
                "a defect ticket cannot close until an accepted criterion covers the case",
            ),
            Self::ScopeLocked { module } => {
                write!(
                    f,
                    "the module {module} is claimed by a ticket already in progress"
                )
            }
            Self::TestModifiedWithoutEscalation => {
                f.write_str("an existing test was modified and no escalation is open for it")
            }
            Self::DocumentTransition { from, to } => {
                write!(f, "a document cannot move from {from} to {to}")
            }
            Self::DocumentSeatMismatch { owner, signer } => {
                write!(
                    f,
                    "the document is owned by {owner} and {signer} cannot sign it"
                )
            }
        }
    }
}

/// Everything this crate refuses or cannot read.
///
/// The enum separates the two so that criterion ORI-P1-033 is answered by the
/// shape rather than by a convention: a refusal is [`Error::Refused`] and always
/// carries a reason, and input that did not parse is [`Error::Malformed`] and
/// never does. Malformed input is not a control refusing an action; citing a
/// methodology section for it would put a reference in front of a human that
/// explains nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// A control refused the action, under the methodology section its
    /// [`RefusalKind::reason`] names.
    Refused {
        /// Which control refused, and why.
        kind: RefusalKind,
        /// What this occurrence was, for the human reading it.
        detail: Option<String>,
    },
    /// A value did not parse into the type it was read as.
    Malformed {
        /// The type the value was read as.
        what: &'static str,
        /// The value as it was given.
        value: String,
    },
}

impl Error {
    /// A refusal with no further detail.
    #[must_use]
    pub const fn refused(kind: RefusalKind) -> Self {
        Self::Refused { kind, detail: None }
    }

    /// A refusal, with what this occurrence was.
    pub fn refused_with(kind: RefusalKind, detail: impl Into<String>) -> Self {
        Self::Refused {
            kind,
            detail: Some(detail.into()),
        }
    }

    /// A value that did not parse.
    pub fn malformed(what: &'static str, value: impl Into<String>) -> Self {
        Self::Malformed {
            what,
            value: value.into(),
        }
    }

    /// The methodology section this error was made under, which criterion
    /// ORI-P1-033 requires of every refusal and of nothing else.
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        match self {
            Self::Refused { kind, .. } => Some(kind.reason()),
            Self::Malformed { .. } => None,
        }
    }

    /// Whether a control refused the action, as opposed to a value failing to
    /// parse.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        matches!(self, Self::Refused { .. })
    }

    /// The error code the client API returns, where `spec/API_SPEC.md` names
    /// one for this error.
    #[must_use]
    pub const fn code(&self) -> Option<&'static str> {
        match self {
            Self::Refused { kind, .. } => kind.code(),
            Self::Malformed { .. } => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused { kind, detail } => {
                write!(f, "refused: {kind} ({})", kind.reason())?;
                match detail {
                    Some(detail) => write!(f, ": {detail}"),
                    None => Ok(()),
                }
            }
            Self::Malformed { what, value } => {
                write!(f, "not a {what}: {value:?}")
            }
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    /// One value of every [`RefusalKind`], which is what the tests below are
    /// run over.
    ///
    /// [`refusal_tag`] is what keeps this list honest: it matches without a
    /// wildcard, so a variant added to [`RefusalKind`] and not added here stops
    /// the build rather than escaping the tests.
    fn every_refusal() -> Vec<RefusalKind> {
        vec![
            RefusalKind::TicketTransition {
                from: TicketState::Merged,
                to: TicketState::Queued,
            },
            RefusalKind::CategoryDowngrade {
                from: Category::Decisional,
                to: Category::Auto,
            },
            RefusalKind::TierDowngrade {
                from: Tier::Two,
                to: Tier::One,
            },
            RefusalKind::SpecUpdateMissing,
            RefusalKind::CriterionMissing,
            RefusalKind::ScopeLocked {
                module: "crates/ori-core/src/types.rs".to_owned(),
            },
            RefusalKind::TestModifiedWithoutEscalation,
            RefusalKind::DocumentTransition {
                from: DocumentState::Missing,
                to: DocumentState::Approved,
            },
            RefusalKind::DocumentSeatMismatch {
                owner: Seat::Architect,
                signer: Seat::ProductOwner,
            },
        ]
    }

    /// A number per variant, matched without a wildcard so that adding a
    /// variant to [`RefusalKind`] fails to compile until it is listed in
    /// [`every_refusal`] too.
    fn refusal_tag(kind: &RefusalKind) -> usize {
        match kind {
            RefusalKind::TicketTransition { .. } => 0,
            RefusalKind::CategoryDowngrade { .. } => 1,
            RefusalKind::TierDowngrade { .. } => 2,
            RefusalKind::SpecUpdateMissing => 3,
            RefusalKind::CriterionMissing => 4,
            RefusalKind::ScopeLocked { .. } => 5,
            RefusalKind::TestModifiedWithoutEscalation => 6,
            RefusalKind::DocumentTransition { .. } => 7,
            RefusalKind::DocumentSeatMismatch { .. } => 8,
        }
    }

    #[test]
    fn ori_p1_033_every_refusal_this_crate_can_make_is_covered_by_these_tests() {
        let refusals = every_refusal();
        let mut tags: Vec<usize> = refusals.iter().map(refusal_tag).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(
            tags.len(),
            refusals.len(),
            "every_refusal lists one value per RefusalKind variant and no variant twice"
        );
    }

    #[test]
    fn ori_p1_033_every_refusal_carries_a_methodology_ref() {
        for kind in every_refusal() {
            let error = Error::refused(kind.clone());
            assert!(error.is_refusal(), "{kind:?} is a refusal");
            assert!(
                error.methodology_ref().is_some(),
                "{kind:?} refuses an action without naming why"
            );
        }
    }

    #[test]
    fn ori_p1_033_every_refusal_reason_resolves_in_the_methodology_index() {
        for kind in every_refusal() {
            let reason = kind.reason();
            assert!(
                reason.resolves(),
                "{kind:?} cites {reason}, which the methodology index does not carry"
            );
            assert!(reason.section >= 1 && reason.section <= SECTION_COUNT);
        }
    }

    #[test]
    fn ori_p1_033_a_detail_does_not_displace_the_reason() {
        let error = Error::refused_with(
            RefusalKind::ScopeLocked {
                module: "crates/ori-core/src/error.rs".to_owned(),
            },
            "claimed by ORI-T-0019",
        );
        let reason = error.methodology_ref().expect("a refusal carries a reason");
        assert!(reason.resolves());
        assert!(error.to_string().contains("AICD §12"));
        assert!(error.to_string().contains("claimed by ORI-T-0019"));
    }

    #[test]
    fn ori_p1_033_a_section_the_methodology_does_not_carry_cannot_be_built() {
        for section in [0u8, SECTION_COUNT + 1, 55, u8::MAX] {
            assert!(
                MethodologyRef::at(section).is_err(),
                "section {section} resolves against nothing and must be refused"
            );
        }
        for section in 1..=SECTION_COUNT {
            assert!(
                MethodologyRef::at(section).is_ok(),
                "section {section} is in the index"
            );
        }
    }

    #[test]
    fn ori_p1_033_a_subsection_the_methodology_does_not_number_cannot_be_built() {
        // Ruling R1 in ops/rulings.md: the methodology numbers subsections in
        // exactly two places, and only one of them fits a numeric section.
        assert!(MethodologyRef::within(24, "2").is_ok());
        assert!(MethodologyRef::within(24, "8").is_ok());
        assert!(MethodologyRef::within(24, "9").is_err());
        // Four of the twelve unresolvable citations ops/rulings.md R19 records
        // in the design mockup, which must stay unbuildable here.
        assert!(MethodologyRef::within(11, "7").is_err());
        assert!(MethodologyRef::within(17, "9").is_err());
        assert!(MethodologyRef::within(27, "2").is_err());
        assert!(MethodologyRef::within(39, "3").is_err());
    }

    #[test]
    fn ori_p1_033_a_reference_built_around_the_constructors_still_answers_honestly() {
        // The fields are public because spec/LLD.md section 4 makes them part
        // of the contract, so resolves() has to answer for a struct literal too.
        let fabricated = MethodologyRef {
            section: 55,
            subsection: None,
        };
        assert!(!fabricated.resolves());
        let good = MethodologyRef {
            section: 12,
            subsection: None,
        };
        assert!(good.resolves());
    }

    #[test]
    fn ori_p1_033_the_reference_is_displayed_in_the_form_conventions_fixes() {
        assert_eq!(
            MethodologyRef::at(12).expect("in the index").to_string(),
            "AICD §12"
        );
        assert_eq!(
            MethodologyRef::within(24, "2")
                .expect("in the index")
                .to_string(),
            "AICD §24.2"
        );
    }

    #[test]
    fn ori_p1_033_a_value_that_did_not_parse_is_not_a_refusal_and_carries_no_reason() {
        let error = Error::malformed("Tier", "3");
        assert!(!error.is_refusal());
        assert!(error.methodology_ref().is_none());
        assert!(error.code().is_none());
    }

    #[test]
    fn ori_p1_033_the_only_error_codes_returned_are_the_two_api_spec_names() {
        let mut codes: Vec<&'static str> = every_refusal()
            .iter()
            .filter_map(RefusalKind::code)
            .collect();
        codes.sort_unstable();
        assert_eq!(codes, vec!["E_SCOPE_LOCKED", "E_UPGRADE_ONLY"]);
    }

    #[test]
    fn ori_p1_033_the_two_named_codes_sit_on_the_refusals_the_criteria_describe() {
        // ORI-P1-005: an agent setting a category back receives E_UPGRADE_ONLY.
        assert_eq!(
            RefusalKind::CategoryDowngrade {
                from: Category::Decisional,
                to: Category::Auto,
            }
            .code(),
            Some("E_UPGRADE_ONLY")
        );
        // ORI-P1-008: the second overlapping ticket receives E_SCOPE_LOCKED.
        assert_eq!(
            RefusalKind::ScopeLocked {
                module: "crates/ori-core".to_owned(),
            }
            .code(),
            Some("E_SCOPE_LOCKED")
        );
    }

    #[test]
    fn ori_p1_033_a_refusal_reads_as_a_sentence_with_its_section() {
        let error = Error::refused(RefusalKind::CriterionMissing);
        let rendered = error.to_string();
        assert!(rendered.starts_with("refused: "), "{rendered}");
        assert!(rendered.contains("AICD §16"), "{rendered}");
        // std::error::Error is implemented, which is what callers box.
        let boxed: Box<dyn std::error::Error> = Box::new(error);
        assert!(boxed.source().is_none());
    }
}
