//! The freshness tracker: AICD §25.
//!
//! `spec/LLD.md` section 2 gives this crate `Freshness`; `spec/PRD.md` K-07
//! ("Freshness tracker and staleness exposure; drift audit integration") and
//! criterion ORI-P1-026 in `spec/criteria/phase-1.md` ("Product with stale
//! document (drift audit found divergence) | `ori readiness` | Not ready; the
//! stale document is listed with the divergence") are what this module
//! answers. Readiness (`ori-flows`) and the drift audit (ORI-T-0040) are not
//! this ticket's: this module is the tracker both of them query, one to
//! refuse launch, the other to record what it found.
//!
//! `spec/DATA_MODEL.md` section 2's `Document` row carries
//! `verified_against_code_at`, and its section 3 state machine has exactly
//! one transition into `Stale`: "`Approved --> Stale: drift audit finds
//! divergence`". This module represents both halves of that: the mechanical
//! half it can decide on its own (a document nobody has ever verified, or
//! one whose text changed after the verification it is still citing), and
//! the recorded half a semantic comparison (the drift audit's, not this
//! ticket's) hands it explicitly.
//!
//! # What makes a document stale, stated exactly
//!
//! A document tracked here is [`Freshness::Stale`] for exactly one of three
//! reasons, checked in this order:
//!
//! 1. [`StaleReason::Divergence`][]: the drift audit called
//!    [`FreshnessTracker::record_divergence`] and nothing has re-verified the
//!    document since. This is `spec/DATA_MODEL.md` section 3's transition,
//!    named plainly rather than inferred.
//! 2. [`StaleReason::ChangedSinceVerification`][]: the document was verified
//!    once, but the text passed to [`FreshnessTracker::check`] today does not
//!    equal the text that was true when [`FreshnessTracker::record_verification`]
//!    ran. A verification is a claim about one exact text; a document edited
//!    after that claim was made is not covered by it any more, whatever the
//!    edit was.
//! 3. [`StaleReason::NeverVerified`][]: [`FreshnessTracker::record_verification`]
//!    has never been called for this path. `verified_against_code_at` absent
//!    is not a "no data, so no news" case: trap 5 of this ticket is exactly a
//!    reader treating a missing timestamp as fresh, and the check below
//!    refuses that reading before it can happen, by making "never verified"
//!    its own named, always-stale branch rather than falling through.
//!
//! A document is [`Freshness::Fresh`] only when none of the three hold: it
//! has been verified, the verification's text still matches, and no
//! divergence is on record.
//!
//! # What a verification covers: the title and the body
//!
//! The text a verification covers, and the text compared against it, is a
//! document's title and its body together,
//! [`crate::indexer::IndexableDocument::freshness_text`], which the
//! indexer's [`crate::indexer::Indexer::freshness_corpus`] gives for every
//! document it holds. A section's heading is not in its body (the walk
//! keeps the heading's text as the title and the text under it as the
//! body), so a verification of the body alone did not cover the heading: a
//! review of round 12 found a heading edited without moving its anchor
//! (`# Retry, then stop` to `# Retry then stop!`, both at
//! `#retry-then-stop`) reading as ready over a verification of the old
//! heading. It is [`StaleReason::ChangedSinceVerification`] now
//! (`tests::ori_p1_026_a_title_edited_under_the_same_path_is_changed_since_verification`;
//! the indexer's own tests run it through a real walk).
//!
//! [`FreshnessTracker::list_stale`] has a fourth, for a record it has no
//! text for because the repository walk left that text out:
//! [`StaleReason::NotIndexed`], freshness unknown. The file is still in the
//! repository, so nothing on record for it is dropped: every record on
//! something the walk left out is listed with the reason the walk gave, and
//! with the drift audit's divergence when one is on record (`list_stale`'s
//! doc; the indexer's module doc, "A file left out is not a file removed",
//! has what the index holds for such a file: none of its documents).
//!
//! # Readiness is about content, not records
//!
//! Criterion ORI-P1-026's readiness must never read "ready" over
//! specification text nobody could check, and must be able to read "ready"
//! once every piece of specification text has been checked. Whether some
//! text could be checked is a fact about the text, not about what happens
//! to be on record for it. So every [`StaleReport`] carries, beside its
//! stale records, [`StaleReport::freshness_unknown`]: every file, symbolic
//! link or directory any specification text of which the repository walk
//! left out, whatever the entry's [`SkipReason::scope`] (a repeated
//! document path, whose later text is in no index, included), each with
//! the reason of every entry the walk left out on it, whether or not any
//! record exists on it. [`StaleReport::is_ready`] is the report's one
//! readiness answer: false while that list is not empty or any record is
//! stale, true otherwise. A file genuinely deleted is in no walk, so it is
//! neither freshness unknown nor listed.
//!
//! The list is built from every entry the walk left out,
//! [`crate::indexer::NotIndexed::entries`], each under its own file
//! ([`crate::indexer::NotIndexedEntry::file`]), never from one entry per
//! key: a review of round 12 found it read off a map keeping the first
//! entry of each key, so a sibling file named as a document of another
//! (`a.md#big` beside the `#big` section of `a.md`, past the cap) went
//! unlisted, and so did the second reason of a criterion row repeated
//! twice, once past the cap (the indexer's
//! `tests::ori_p1_026_entries_that_share_a_key_list_every_file_and_every_reason`).
//!
//! # Only specification text is freshness unknown
//!
//! An entry the walk left out makes its file freshness unknown only when
//! it may hold specification text, which
//! [`SkipReason::leaves_out_specification`] decides once, per reason, in a
//! match naming every reason, so a reason added later has to choose:
//!
//! - Markdown the walk could not index: [`SkipReason::DocumentTooLarge`],
//!   [`SkipReason::FileTooLarge`], [`SkipReason::FileDocumentLimit`],
//!   [`SkipReason::DuplicateDocumentPath`], [`SkipReason::NonUtf8Path`],
//!   [`SkipReason::WalkDocumentLimit`], [`SkipReason::WalkByteLimit`], and
//!   a `.md` file unreadable or not UTF-8 ([`SkipReason::Unreadable`]).
//! - An entry that could hold Markdown the walk did not read: a directory
//!   it could not list, or an entry whose type it could not read
//!   ([`SkipReason::Unreadable`]), and a symbolic link
//!   ([`SkipReason::Symlink`]), since a link named `*.md` or a link to a
//!   directory could, and the walk never looks at what a link points at.
//!
//! A file not named `.md` ([`SkipReason::NotMarkdown`], this repository's
//! own `spec/design/Ori Studio.html`, a `.DS_Store`) and an entry that is
//! not a regular file ([`SkipReason::NotARegularFile`], a named pipe, a
//! socket) are not specification text. [`FreshnessTracker::list_stale`]
//! reads such an entry as if the walk had never met it: it is never
//! freshness unknown, never covers a record, and never refuses readiness.
//! A review of round 12 found every entry refusing readiness, so this
//! repository could never read as ready (the indexer's
//! `tests::ori_p1_026_a_copy_of_this_repositorys_spec_tree_with_every_document_verified_is_ready`,
//! and
//! `tests::ori_p1_026_exactly_the_entries_that_leave_out_specification_text_refuse_readiness`
//! here).
//!
//! Rounds 8 to 11 of this ticket listed records on what the walk left out,
//! one shape of record at a time, and each review found content no listed
//! record reached. Round 11's found three, each read as ready: a file whose
//! own path is also a document of the corpus (a role file with no heading,
//! or a file with a short preamble) gaining a section past the document
//! cap, since a record at that path was checked against the indexed text
//! alone; an edited row repeating a criterion identifier, since the walk's
//! list dropped the repeat and the record at its path was checked against
//! the first row; and content left out that nothing had ever been recorded
//! on. The rule now covers the content itself, so no shape of record, and
//! no absence of one, can hide it
//! (`tests::ori_p1_026_every_file_left_out_is_freshness_unknown_whether_or_not_a_record_names_it`;
//! the indexer's own tests run each shape, and every skip reason, through a
//! real walk).
//!
//! # Freshness unknown is never fresh
//!
//! This tracker decides a record's freshness by comparing text. For a
//! record whose text the walk left out, it has nothing to compare: it
//! cannot tell whether the document changed after the verification on
//! record, so it cannot say the document is fresh, and saying nothing
//! would drop a record the drift audit still holds. Every such record is
//! therefore listed as [`StaleReason::NotIndexed`], this module's "freshness
//! unknown" reason, carrying the walk's own reason for leaving the text
//! out, whatever the record holds: a verification alone, a divergence, or
//! a divergence a later verification cleared. Its line
//! ([`StaleReason::divergence`]) says "freshness unknown" in so many words.
//! A review found round 10 dropping a verification alone, and a divergence
//! cleared by a re-verification, on a file the walk read but none of whose
//! documents it indexed, every one past the document cap (a risk map of
//! one heading and 70 KB, a runbook of two such sections, a criteria file
//! of one such row): the stale list was empty
//! (`tests::ori_p1_026_a_verification_alone_on_a_file_whose_documents_are_past_the_cap_is_listed_as_freshness_unknown`;
//! the indexer's own tests run the same three shapes through a real walk).
//! Each such file is in [`StaleReport::freshness_unknown`] as well, so the
//! report is not ready even where no record reaches it ("Readiness is about
//! content, not records", above).
//!
//! What is not listed is a record on something genuinely gone: a file no
//! longer in the repository, or a document no longer in a file the walk
//! read. The walk does not name those, and they have no text to be
//! unknown. Nor is a record on an entry that holds no specification text,
//! a file not named `.md` or an entry that is not a regular file: it is
//! read as gone, as if the walk had never met the entry ("Only
//! specification text is freshness unknown", above).
//!
//! # What a skip covers: one document, or a whole file
//!
//! Every entry the walk left out that leaves out specification text says
//! what it covers, through [`SkipReason::scope`], and a record is matched
//! to it by that scope and nothing else, never by the shape of the entry's
//! key. The file it makes freshness unknown is the entry's own file: a
//! file-scoped entry's own path, or the file a document-scoped entry's
//! document belongs to. Where two such entries share a key, every one of
//! them is read: each file is listed, and a record under a file-scoped
//! entry is covered even when a document-scoped entry holds the same key
//! first.
//!
//! - A file-scoped entry ([`SkipScope::File`]) is a file, a symbolic link
//!   or a directory the walk did not read. It covers a record at its path,
//!   at any document path of that file (`<file>#<anchor>`) and at anything
//!   under that directory: nothing there was read, so nothing there can be
//!   checked.
//! - A document-scoped entry ([`SkipScope::Document`]) is one document of a
//!   file the walk did read and split, left out for its size or as a
//!   repeat of a path an earlier document of the walk took. It covers a
//!   record at exactly that document path, when the corpus does not hold
//!   it (a repeat's path it always does, with the earlier document), and no
//!   other, even when that path is the file's bare path (the text above
//!   the file's first heading). The file was read, so every other document
//!   path of it is judged by the corpus: in it and checked, or not in it
//!   and removed. A section genuinely removed from a file whose preamble is
//!   past the cap is removed, never pinned as left out.
//!
//! A record at a file's own path that the corpus holds (the text above the
//! file's first heading, or a whole file with no heading) is that
//! document's record, checked against that document's text like any
//! other; the rest of the file, when the walk left some of it out, is in
//! [`StaleReport::freshness_unknown`], whatever that record holds.
//!
//! A record on a file's own path that the corpus does not hold (the drift
//! audit's verification or divergence on the file, the `Document` of
//! `spec/DATA_MODEL.md` section 2) names the whole file. The file was read,
//! and is still in the repository, when the corpus holds a document of it
//! or a document-scoped entry names one. Such a record is listed:
//!
//! - as [`StaleReason::Divergence`], beside the file's indexed documents,
//!   when a divergence is on record and the corpus holds a document of the
//!   file;
//! - otherwise, while a document-scoped entry names any document of the
//!   file, as [`StaleReason::NotIndexed`] with the first such entry's
//!   reason (in path order) and the divergence on record, if any, whether
//!   the record is a verification alone, a divergence, or a divergence a
//!   later verification cleared: part or all of the text the record names
//!   was left out, so its freshness is unknown ("Freshness unknown is never
//!   fresh", above). Every document the file split into left out for its
//!   size (a file of one section grown past the document cap, a file of
//!   several such sections, a criteria file of one such row) is this case.
//!
//! A verification alone on the path of a file every document of which is
//! in the corpus is not listed: each of those documents is checked against
//! its own record, one with none is [`StaleReason::NeverVerified`], and a
//! verification at the file's path makes none of them fresh. Any other
//! record no entry covers is gone, and not listed: one for a document no
//! longer in a file the walk read, every one on a file no longer in the
//! repository, and every one on an entry that holds no specification text,
//! which covers nothing.
//!
//! Every listing here can be cleared, and so can every file in
//! [`StaleReport::freshness_unknown`]: a file leaves that list once the
//! walk leaves nothing of it out (each document back under the cap, the
//! repeat resolved, the file readable, or the file removed). A record
//! under a document-scoped entry is checked like any other once its
//! document is back under the cap (fresh when its text is the verified
//! text, or once re-verified), and is removed once the document is removed
//! from its file. A record on a file's own path listed as not indexed
//! stops being listed once no document of the file is left out, its file's
//! documents then checked by their own records; a re-verification of the
//! file while a document of it is still left out ends its divergence, but
//! not the listing, since the text still cannot be checked. A file-level
//! divergence beside indexed documents ends with the file's next
//! verification, as it does anywhere.
//!
//! # What does not make a document stale
//!
//! Code changing elsewhere in the repository that this document does not
//! claim to describe is not inferred as staleness here: judging whether a
//! piece of code and a piece of prose still agree is a semantic comparison,
//! and that is the drift audit's job (ORI-T-0040), fed into this tracker
//! through [`FreshnessTracker::record_divergence`], not reconstructed by this
//! module from file timestamps or a code map it does not have. The passage
//! of time alone is not staleness either: nothing here reads a clock or
//! applies a expiry window, because no such window is specified anywhere in
//! `spec/`; inventing one would be this ticket deciding a number the
//! specification does not state. And one document's staleness never spreads
//! to another: each path is tracked and checked independently.
//!
//! # The vacuity trap
//!
//! "No stale documents" is trivially true of a tracker holding nothing, and
//! is indistinguishable, from the caller's side, from "no stale documents
//! because none were even considered". [`FreshnessTracker::list_stale`]
//! therefore reports [`StaleReport::documents_covered`], the size of the
//! corpus it was actually asked to check, on every call, so a caller (and
//! every test in this module) can assert that count is what it expects
//! before it trusts an empty `stale` list. A tracker holding nothing is
//! never ready over a corpus holding anything: every document of it is
//! [`StaleReason::NeverVerified`]. And an empty corpus is ready only when
//! the walk left no specification text out either, [`StaleReport::is_ready`]
//! reading [`StaleReport::freshness_unknown`] as well: a repository whose
//! every specification file was left out has no document to check and is
//! still not ready.
//!
//! Must not: return unsanitized production content in a package (`spec/LLD.md`
//! section 2); nothing here reads a file, a clock or an environment variable,
//! for the same reason `ori-core` gives its own value types none of those
//! (`crates/ori-core/src/types.rs`, `Timestamp`'s doc: "There is no
//! constructor that reads a clock"): every timestamp and every text this
//! module compares against is a value the caller already had, most naturally
//! a real `Document.verified_against_code_at`.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

use ori_core::types::Timestamp;

use crate::indexer::NotIndexed;
use crate::indexer::NotIndexedEntry;
use crate::indexer::SkipReason;
use crate::indexer::SkipScope;
use crate::indexer::document_file;

/// Whether one document's last verification against the code still holds:
/// AICD §25.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Freshness {
    /// Verified, the verified text still matches, and no divergence is on
    /// record.
    Fresh,
    /// Not fresh, for the reason carried.
    Stale(StaleReason),
}

impl Freshness {
    /// Whether this is the [`Freshness::Stale`] case, for a caller that only
    /// needs the yes/no answer (`ori readiness`'s "Not ready" branch).
    #[must_use]
    pub const fn is_stale(&self) -> bool {
        matches!(self, Self::Stale(_))
    }
}

/// Why a document is stale: the module doc's three named cases, in the order
/// they are checked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StaleReason {
    /// [`FreshnessTracker::record_verification`] has never run for this
    /// path.
    NeverVerified,
    /// The document was verified at `verified_at`, but the text checked
    /// today is not the text that was true then.
    ChangedSinceVerification {
        /// When the (now superseded) verification ran.
        verified_at: Timestamp,
    },
    /// The drift audit filed a divergence that nothing has re-verified
    /// since.
    Divergence {
        /// The drift audit's own account of what disagreed. This is the
        /// text ORI-P1-026 asks `ori readiness` to list the stale document
        /// with.
        description: String,
        /// When the drift audit found it.
        detected_at: Timestamp,
    },
    /// Freshness unknown: the repository walk left out what the record
    /// names, specification text or an entry that may hold some
    /// ([`SkipReason::leaves_out_specification`]), so its text could not be
    /// checked against anything on record, while the file is still in the
    /// repository: the document itself (a
    /// document-scoped skip), its file or a directory holding it (a
    /// file-scoped skip), or, for a record on a file's own path, one or
    /// more of the documents the file split into, left out for their size
    /// (the module doc's "What a skip covers"). Reported for every such
    /// record, a verification alone included, since a text nobody could
    /// check is never fresh (the module doc's "Freshness unknown is never
    /// fresh"). Only [`FreshnessTracker::list_stale`] reports this, for a
    /// path it holds a record for.
    NotIndexed {
        /// Why the walk left it out, in the walk's own terms.
        skipped: SkipReason,
        /// The drift audit's divergence on record for the path, if one is:
        /// its description and when it was found, kept and listed, never
        /// dropped because the text could not be read.
        divergence: Option<(String, Timestamp)>,
    },
}

impl StaleReason {
    /// A human-readable line for a query result: the drift audit's own
    /// wording for [`StaleReason::Divergence`] (and for a
    /// [`StaleReason::NotIndexed`] path with a divergence on record, beside
    /// the walk's reason), and this module's own statement of the
    /// mechanical reason otherwise. `list_stale` uses this
    /// so every stale document it reports carries one, matching ORI-P1-026's
    /// "the stale document is listed with the divergence" for the two
    /// reasons this module can determine on its own as well as the one the
    /// drift audit determines.
    #[must_use]
    pub fn divergence(&self) -> String {
        match self {
            Self::NeverVerified => {
                "never verified against the code: no verified_against_code_at is on record"
                    .to_owned()
            }
            Self::ChangedSinceVerification { verified_at } => format!(
                "the document changed after its last verification against the code, at {verified_at}"
            ),
            Self::Divergence {
                description,
                detected_at,
            } => format!("{description} (drift audit, at {detected_at})"),
            Self::NotIndexed {
                skipped,
                divergence: None,
            } => format!(
                "freshness unknown: not in the index, so its text could not be checked against \
                 the code: {skipped}"
            ),
            Self::NotIndexed {
                skipped,
                divergence: Some((description, detected_at)),
            } => format!(
                "{description} (drift audit, at {detected_at}); freshness unknown: not in the \
                 index, so its text could not be checked against the code: {skipped}"
            ),
        }
    }
}

/// What is on record for one tracked document.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TrackedDocument {
    /// Set by [`FreshnessTracker::record_verification`]; cleared by nothing,
    /// carried forward so [`StaleReason::ChangedSinceVerification`] can name
    /// when the now-superseded verification ran.
    verified_at: Option<Timestamp>,
    /// The exact text that was true when `verified_at` was recorded, so a
    /// later `check` can tell whether the document has moved on from it.
    verified_text: Option<String>,
    /// Set by [`FreshnessTracker::record_divergence`]; cleared by the next
    /// [`FreshnessTracker::record_verification`], mirroring
    /// `spec/DATA_MODEL.md` section 3's `Stale --> UnderReview` (a document
    /// leaves `Stale` by being re-reviewed, which is what a fresh
    /// verification is).
    divergence: Option<(String, Timestamp)>,
}

/// One stale document, as [`FreshnessTracker::list_stale`] reports it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaleDocument {
    /// The document's path: a key of the corpus `list_stale` was given;
    /// the path of a file whose sections that corpus holds, for a
    /// divergence filed against the whole file; or a path on record that
    /// the walk left out ([`StaleReason::NotIndexed`]; `list_stale`'s doc).
    pub path: String,
    /// Why it is stale.
    pub reason: StaleReason,
}

/// One file, symbolic link or directory some or all of whose
/// specification text the repository walk left out, as
/// [`FreshnessTracker::list_stale`] reports it whether or not any record
/// names it: its freshness is unknown (the module doc's "Readiness is
/// about content, not records"): AICD §25.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreshnessUnknown {
    /// The file, link or directory, under the repository root and
    /// `/`-joined as a document path is: the entry's own file
    /// ([`crate::indexer::NotIndexedEntry::file`]), a file-scoped entry's
    /// own path, or the file a document-scoped entry's document belongs to.
    /// Two files whose names differ only in bytes that are not UTF-8 are
    /// listed apart, though each is spelled here with U+FFFD alike.
    pub path: String,
    /// Why the walk left its content out: the reason of every entry the
    /// walk left out on it that leaves out specification text, in walk
    /// order, two entries sharing a key included, never empty.
    pub skipped: Vec<SkipReason>,
}

impl FreshnessUnknown {
    /// A human-readable line for a query result, saying "freshness
    /// unknown" in so many words and giving every reason the walk left the
    /// content out for, in [`FreshnessUnknown::skipped`]'s order.
    #[must_use]
    pub fn line(&self) -> String {
        let reasons: Vec<String> = self.skipped.iter().map(ToString::to_string).collect();
        format!(
            "freshness unknown: the repository walk left content of it out, so that text could \
             not be checked against the code: {}",
            reasons.join("; ")
        )
    }
}

/// The result of one [`FreshnessTracker::list_stale`] call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaleReport {
    /// How many documents this call actually checked, so "the stale list is
    /// empty" and "nothing was checked" can never be read as the same
    /// answer (the module doc's vacuity trap).
    pub documents_covered: usize,
    /// Every stale document found, each carrying its reason and, through
    /// [`StaleReason::divergence`], a line describing what diverged.
    pub stale: Vec<StaleDocument>,
    /// Every file, link or directory any specification text of which the
    /// walk left out, in path order, whether or not any record names it:
    /// what makes [`StaleReport::is_ready`] false even where `stale` is
    /// empty (the module doc's "Readiness is about content, not records").
    /// A file not named `.md`, or an entry that is not a regular file, is
    /// never here (the module doc's "Only specification text is freshness
    /// unknown").
    pub freshness_unknown: Vec<FreshnessUnknown>,
}

impl StaleReport {
    /// The report's one readiness answer, the freshness half of `ori
    /// readiness` (criterion ORI-P1-026): false while any record is stale
    /// or any specification text the walk left out is freshness unknown,
    /// true only when both lists are empty. Never read off `stale` alone:
    /// a file the walk left out may have no record on it at all (the module
    /// doc's "Readiness is about content, not records"). AICD §25.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.stale.is_empty() && self.freshness_unknown.is_empty()
    }
}

/// Tracks, per document path, when it was last verified against the code,
/// what its text was then, and any divergence the drift audit has filed:
/// AICD §25.
///
/// Holds no repository state and reads no clock or file itself
/// (`ori-core`'s own rule for a value type, carried here even though this is
/// not `ori-core`: every timestamp is one the caller already had, from the
/// same event that also produced it).
#[derive(Clone, Debug, Default)]
pub struct FreshnessTracker {
    documents: BTreeMap<String, TrackedDocument>,
}

impl FreshnessTracker {
    /// An empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that `path` was verified against the code at `at`, with the
    /// text `content` at that moment: for a document of the index, its
    /// title and its body,
    /// [`crate::indexer::IndexableDocument::freshness_text`], the text
    /// [`FreshnessTracker::list_stale`]'s corpus holds for it (the module
    /// doc's "What a verification covers").
    ///
    /// Clears any divergence on record: a fresh verification is a review
    /// (`spec/DATA_MODEL.md` section 3, `Stale --> UnderReview`), and this is
    /// this module's read of "the document has been looked at again", which
    /// is what ends the state a `Divergence` reason names. If the drift
    /// audit's re-review still finds the same problem, the caller calls
    /// [`FreshnessTracker::record_divergence`] again, so the divergence text
    /// on record is always the current finding, not a stale one nobody
    /// re-asserted.
    pub fn record_verification(
        &mut self,
        path: impl Into<String>,
        at: Timestamp,
        content: impl Into<String>,
    ) {
        let entry = self.documents.entry(path.into()).or_default();
        entry.verified_at = Some(at);
        entry.verified_text = Some(content.into());
        entry.divergence = None;
    }

    /// Records that the drift audit found `path` and the code disagree, with
    /// `description` as its account of the divergence.
    ///
    /// Does not require a prior [`FreshnessTracker::record_verification`]: a
    /// document can be found diverging on its very first audit, and this
    /// call is what a caller reaches for at that point too, not a special
    /// case of it.
    pub fn record_divergence(
        &mut self,
        path: impl Into<String>,
        description: impl Into<String>,
        at: Timestamp,
    ) {
        let entry = self.documents.entry(path.into()).or_default();
        entry.divergence = Some((description.into(), at));
    }

    /// The freshness of one document, given the text it holds right now:
    /// for a document of the index, its title and its body
    /// ([`crate::indexer::IndexableDocument::freshness_text`]).
    ///
    /// `content` is required, not optional: a caller with no text for `path`
    /// has nothing to compare against `verified_text`, and passing an empty
    /// string in that case would silently report
    /// [`StaleReason::ChangedSinceVerification`] instead of the caller's own
    /// "this path does not exist" case, which is not this module's to guess
    /// at.
    #[must_use]
    pub fn check(&self, path: &str, content: &str) -> Freshness {
        let Some(entry) = self.documents.get(path) else {
            return Freshness::Stale(StaleReason::NeverVerified);
        };
        if let Some((description, detected_at)) = &entry.divergence {
            return Freshness::Stale(StaleReason::Divergence {
                description: description.clone(),
                detected_at: *detected_at,
            });
        }
        let (Some(verified_at), Some(verified_text)) = (entry.verified_at, &entry.verified_text)
        else {
            return Freshness::Stale(StaleReason::NeverVerified);
        };
        if verified_text != content {
            return Freshness::Stale(StaleReason::ChangedSinceVerification { verified_at });
        }
        Freshness::Fresh
    }

    /// Checks every document in `current`, keyed by path with its text right
    /// now, and reports the stale ones with their divergence, in path order;
    /// reports every record on specification text the repository walk left
    /// out, named in `not_indexed`, rather than dropping it; and reports
    /// every file any specification text of which the walk left out as
    /// freshness unknown, whether or not any record names it, so that
    /// [`StaleReport::is_ready`] is false while one is.
    ///
    /// `current` is the live corpus a caller hands in at call time, not a
    /// set this tracker maintains itself: each document's path, with the
    /// text a verification of it covers, its title and its body
    /// ([`crate::indexer::IndexableDocument::freshness_text`]), which the
    /// indexer's [`crate::indexer::Indexer::freshness_corpus`] gives. A
    /// verification is recorded at the same text (the module doc's "What a
    /// verification covers"). `not_indexed` is what the walk the index was
    /// synced from left out, [`crate::indexer::RepoWalk::not_indexed`] of
    /// [`crate::indexer::Indexer::collect_from_repo`]'s walk: every entry
    /// the walk did not read or did not index, a repeated document path
    /// included, each keyed by the document path, file path or directory
    /// path it names, with its file and its reason, none merged with
    /// another at the same key. The pairing is one walk's documents to the
    /// sync that produced `current` and the same walk's list here. A caller
    /// with no walk, and so nothing left out, passes an empty list.
    ///
    /// Only an entry that leaves out specification text counts
    /// ([`SkipReason::leaves_out_specification`]; the module doc's "Only
    /// specification text is freshness unknown"): every other entry, a file
    /// not named `.md` or an entry that is not a regular file, is read
    /// below as if the walk had never met it.
    ///
    /// Content first (the module doc's "Readiness is about content, not
    /// records"): every entry that counts names a file, its own
    /// [`crate::indexer::NotIndexedEntry::file`] (the file a
    /// document-scoped entry's document belongs to, for such an entry), and
    /// each such file is in [`StaleReport::freshness_unknown`] with the
    /// reason of every entry on it, whatever is or is not on record. A
    /// review found round 11 reading readiness off records alone, and three
    /// shapes of left-out content that no record reached (the module doc
    /// has them); a review of round 12 found two entries sharing a key
    /// listed as one, since the list was read off a map of one entry per
    /// key.
    ///
    /// Then records, each listed in `stale` where it is correct to list it:
    ///
    /// - Every document of `current` that [`FreshnessTracker::check`] finds
    ///   stale, including one whose path is a file's bare path, checked
    ///   against that document's text.
    /// - A divergence on record for a file whose documents `current` holds,
    ///   under the file's path, whether or not `current` has a key at
    ///   exactly that path (the index is keyed by section, the drift audit
    ///   files against the file, the `Document` of `spec/DATA_MODEL.md`
    ///   section 2). A review found round 7 dropping it as if the file had
    ///   been removed. A verification recorded at a file's path says nothing
    ///   about any one section's text and makes none of them fresh.
    /// - Every record `current` does not hold that an entry of
    ///   `not_indexed` covers by its scope (the module doc's "What a skip
    ///   covers"), as [`StaleReason::NotIndexed`] with the walk's reason and
    ///   any divergence on record, whatever the record holds: a file-scoped
    ///   entry covers a record at its path, at a document of that file, or
    ///   under that directory; a document-scoped entry covers a record at
    ///   exactly its document path, never a sibling. The reason is the
    ///   first entry's at the record's own path, in walk order, or else the
    ///   nearest enclosing file-scoped entry's, found among every entry at
    ///   that key. A review found round 8 dropping the divergence on a file
    ///   one non-UTF-8 byte, or an ADR grown past the document cap, had
    ///   taken out of the index.
    /// - Every other record on the path of a file a document-scoped entry
    ///   names a document of, that `current` does not hold, a verification
    ///   alone included, as [`StaleReason::NotIndexed`] with the first such
    ///   entry's reason (in path order) and any divergence on record; the
    ///   one exception is the divergence beside indexed documents, above.
    ///   Reviews found round 9 dropping the divergence on a single-section
    ///   file grown past the cap, and round 10 a verification alone there.
    ///
    /// A verification alone on the path of a file every document of which
    /// is in `current` is not listed: those documents are each checked
    /// against their own records. A record no entry covers, whose file has
    /// nothing in `current` and no document-scoped entry, is gone and not
    /// reported at all; so is a record for a document removed from a file
    /// the walk read, and a file genuinely deleted is in neither list. That
    /// matches the indexer's own rule that a removed document disappears
    /// rather than lingers. A review found round 9 pinning such a record for
    /// good when the file's preamble was past the document cap, by reading
    /// the preamble's entry, keyed at the file's bare path, as the whole
    /// file.
    #[must_use]
    pub fn list_stale(
        &self,
        current: &BTreeMap<String, String>,
        not_indexed: &NotIndexed,
    ) -> StaleReport {
        // Only an entry that left out specification text counts, for the
        // content and for the records alike; every other is read as if the
        // walk had never met it.
        let mut by_key: BTreeMap<&str, Vec<&NotIndexedEntry>> = BTreeMap::new();
        // Content: every file any specification text of which the walk
        // left out, with the reason of every entry on it, whatever is on
        // record. Grouped by the entry's own file, never by its key, so two
        // entries sharing a key are each listed under their own file.
        let mut unknown: BTreeMap<(&str, &Path), Vec<SkipReason>> = BTreeMap::new();
        for entry in not_indexed.entries() {
            if !entry.reason.leaves_out_specification() {
                continue;
            }
            by_key.entry(entry.key.as_str()).or_default().push(entry);
            unknown
                .entry((entry.file.as_str(), entry.origin.as_path()))
                .or_default()
                .push(entry.reason.clone());
        }
        let freshness_unknown = unknown
            .into_iter()
            .map(|((path, _), skipped)| FreshnessUnknown {
                path: path.to_owned(),
                skipped,
            })
            .collect();

        // Records: each where it is correct to list it.
        let mut stale = Vec::new();
        for (path, content) in current {
            if let Freshness::Stale(reason) = self.check(path, content) {
                stale.push(StaleDocument {
                    path: path.clone(),
                    reason,
                });
            }
        }
        let files: BTreeSet<&str> = current.keys().map(|key| document_file(key)).collect();
        // Each file the walk read and split, one or more of whose documents
        // it left out: the file's path, with the first such entry's reason
        // in path order.
        let mut split_files: BTreeMap<&str, &SkipReason> = BTreeMap::new();
        for entry in by_key.values().flatten() {
            if entry.reason.scope() == SkipScope::Document {
                split_files
                    .entry(entry.file.as_str())
                    .or_insert(&entry.reason);
            }
        }
        for (path, entry) in &self.documents {
            // Checked above, against the text the corpus holds at its path.
            if current.contains_key(path) {
                continue;
            }
            if let Some(skipped) = left_out(&by_key, path) {
                stale.push(StaleDocument {
                    path: path.clone(),
                    reason: StaleReason::NotIndexed {
                        skipped: skipped.clone(),
                        divergence: entry.divergence.clone(),
                    },
                });
                continue;
            }
            let reason = match (&entry.divergence, files.contains(path.as_str())) {
                // A divergence on a file the corpus holds documents of.
                (Some((description, detected_at)), true) => StaleReason::Divergence {
                    description: description.clone(),
                    detected_at: *detected_at,
                },
                // Any record on a file part or all of whose text was left
                // out, a verification alone included: freshness unknown,
                // never dropped (the module doc's "Freshness unknown is
                // never fresh").
                (divergence, _) => match split_files.get(path.as_str()) {
                    Some(skipped) => StaleReason::NotIndexed {
                        skipped: (*skipped).clone(),
                        divergence: divergence.clone(),
                    },
                    // Checked document by document through the corpus, or
                    // gone.
                    None => continue,
                },
            };
            stale.push(StaleDocument {
                path: path.clone(),
                reason,
            });
        }
        stale.sort_by(|left, right| left.path.cmp(&right.path));
        StaleReport {
            documents_covered: current.len(),
            stale,
            freshness_unknown,
        }
    }

    /// How many paths this tracker holds any record for at all (verified,
    /// diverging, or both), for a caller or test that wants to state the
    /// tracker is not itself empty independent of any particular
    /// `list_stale` call.
    #[must_use]
    pub fn tracked_count(&self) -> usize {
        self.documents.len()
    }
}

/// Why the walk left out `path`, if an entry of `by_key` (every entry that
/// left out specification text, by key, each key's entries in walk order)
/// covers it by that entry's [`SkipReason::scope`]: the first entry of
/// either scope keyed at `path` itself, or else the first file-scoped entry
/// keyed at the file `path` belongs to ([`document_file`]) or at a
/// directory holding that file, the nearest first. A document-scoped entry
/// keyed at the file's bare path (the text above its first heading) is that
/// one document, and never covers a sibling; nor does it hide a file-scoped
/// entry at the same key, which every entry of the key is searched for.
fn left_out<'a>(
    by_key: &BTreeMap<&str, Vec<&'a NotIndexedEntry>>,
    path: &str,
) -> Option<&'a SkipReason> {
    if let Some(entry) = by_key.get(path).and_then(|entries| entries.first()) {
        return Some(&entry.reason);
    }
    let mut enclosing = document_file(path);
    loop {
        if let Some(entry) = by_key.get(enclosing).and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.reason.scope() == SkipScope::File)
        }) {
            return Some(&entry.reason);
        }
        enclosing = enclosing.rsplit_once('/')?.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a walk that left nothing out hands `list_stale`.
    fn nothing_left_out() -> NotIndexed {
        NotIndexed::default()
    }

    fn corpus(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect()
    }

    // -----------------------------------------------------------------
    // ORI-P1-026 (the freshness half): "Product with stale document (drift
    // audit found divergence) | `ori readiness` | Not ready; the stale
    // document is listed with the divergence"
    // -----------------------------------------------------------------

    #[test]
    fn ori_p1_026_a_divergence_the_drift_audit_filed_is_listed_with_its_text() {
        let mut tracker = FreshnessTracker::new();
        tracker.record_verification(
            "spec/PRD.md",
            Timestamp::from_millis(1_000),
            "the original text",
        );
        tracker.record_divergence(
            "spec/PRD.md",
            "K-02 describes an indexer the code does not have",
            Timestamp::from_millis(2_000),
        );

        let current = corpus(&[("spec/PRD.md", "the original text")]);
        let report = tracker.list_stale(&current, &nothing_left_out());

        assert_eq!(
            report.documents_covered, 1,
            "list_stale must report how many documents it actually checked"
        );
        assert_eq!(report.stale.len(), 1);
        assert_eq!(report.stale[0].path, "spec/PRD.md");
        assert!(
            report.stale[0]
                .reason
                .divergence()
                .contains("K-02 describes an indexer the code does not have"),
            "the drift audit's own divergence text must be in the reported line: {}",
            report.stale[0].reason.divergence()
        );
    }

    #[test]
    fn ori_p1_026_a_document_with_no_divergence_and_an_unchanged_verification_is_not_listed() {
        let mut tracker = FreshnessTracker::new();
        tracker.record_verification(
            "spec/LLD.md",
            Timestamp::from_millis(1_000),
            "unchanged text",
        );

        let current = corpus(&[("spec/LLD.md", "unchanged text")]);
        let report = tracker.list_stale(&current, &nothing_left_out());

        assert_eq!(
            report.documents_covered, 1,
            "the count must be non-zero before the absence of staleness means anything"
        );
        assert!(report.stale.is_empty());
    }

    #[test]
    fn ori_p1_026_readiness_denominator_is_the_whole_corpus_not_only_the_stale_ones() {
        let mut tracker = FreshnessTracker::new();
        tracker.record_verification("spec/PRD.md", Timestamp::from_millis(1_000), "fresh");
        tracker.record_verification("spec/LLD.md", Timestamp::from_millis(500), "fresh once");
        tracker.record_divergence("spec/LLD.md", "drifted", Timestamp::from_millis(2_000));

        let current = corpus(&[("spec/PRD.md", "fresh"), ("spec/LLD.md", "fresh once")]);
        let report = tracker.list_stale(&current, &nothing_left_out());

        assert_eq!(report.documents_covered, 2);
        assert_eq!(report.stale.len(), 1);
        assert_eq!(report.stale[0].path, "spec/LLD.md");
    }

    // -----------------------------------------------------------------
    // ORI-T-0035: the three ways to be stale, and the one way to be fresh
    // -----------------------------------------------------------------

    #[test]
    fn ori_t_0035_a_document_never_verified_is_stale_even_with_no_divergence_on_record() {
        let tracker = FreshnessTracker::new();
        let freshness = tracker.check("spec/RISK_MAP.md", "anything");
        assert_eq!(
            freshness,
            Freshness::Stale(StaleReason::NeverVerified),
            "a missing verified_against_code_at must never read as fresh (trap 5 of this ticket)"
        );
    }

    #[test]
    fn ori_t_0035_a_document_edited_after_its_verification_is_stale() {
        let mut tracker = FreshnessTracker::new();
        tracker.record_verification(
            "spec/PRD.md",
            Timestamp::from_millis(1_000),
            "the text at verification time",
        );
        let freshness = tracker.check("spec/PRD.md", "the text after someone edited it");
        assert_eq!(
            freshness,
            Freshness::Stale(StaleReason::ChangedSinceVerification {
                verified_at: Timestamp::from_millis(1_000)
            })
        );
    }

    #[test]
    fn ori_t_0035_a_verified_unedited_document_is_fresh() {
        let mut tracker = FreshnessTracker::new();
        tracker.record_verification("spec/PRD.md", Timestamp::from_millis(1_000), "steady text");
        assert_eq!(
            tracker.check("spec/PRD.md", "steady text"),
            Freshness::Fresh
        );
    }

    #[test]
    fn ori_t_0035_a_fresh_reverification_clears_a_prior_divergence() {
        let mut tracker = FreshnessTracker::new();
        tracker.record_divergence(
            "spec/PRD.md",
            "found drifting",
            Timestamp::from_millis(1_000),
        );
        assert!(tracker.check("spec/PRD.md", "text").is_stale());

        tracker.record_verification("spec/PRD.md", Timestamp::from_millis(2_000), "text");
        assert_eq!(
            tracker.check("spec/PRD.md", "text"),
            Freshness::Fresh,
            "spec/DATA_MODEL.md section 3's Stale --> UnderReview is a re-review; a fresh \
             verification must end the divergence, not leave it stacked on top of a new one"
        );
    }

    #[test]
    fn ori_t_0035_a_divergence_filed_with_no_prior_verification_is_stale_with_its_own_text() {
        let mut tracker = FreshnessTracker::new();
        tracker.record_divergence(
            "spec/PRD.md",
            "first audit ever, already drifting",
            Timestamp::from_millis(1_000),
        );
        let freshness = tracker.check("spec/PRD.md", "text");
        assert_eq!(
            freshness,
            Freshness::Stale(StaleReason::Divergence {
                description: "first audit ever, already drifting".to_owned(),
                detected_at: Timestamp::from_millis(1_000),
            })
        );
    }

    #[test]
    fn ori_t_0035_list_stale_never_reports_a_document_absent_from_the_current_corpus() {
        let mut tracker = FreshnessTracker::new();
        tracker.record_divergence(
            "docs/REMOVED.md",
            "stale before removal",
            Timestamp::from_millis(1_000),
        );

        let current = corpus(&[("spec/PRD.md", "text")]);
        tracker_record_fresh(
            &mut tracker,
            "spec/PRD.md",
            Timestamp::from_millis(1_000),
            "text",
        );
        let report = tracker.list_stale(&current, &nothing_left_out());

        assert_eq!(report.documents_covered, 1);
        assert!(
            report
                .stale
                .iter()
                .all(|document| document.path != "docs/REMOVED.md"),
            "a document no longer in the live corpus must not linger in a staleness report"
        );
    }

    fn tracker_record_fresh(tracker: &mut FreshnessTracker, path: &str, at: Timestamp, text: &str) {
        tracker.record_verification(path, at, text);
    }

    #[test]
    fn ori_t_0035_tracked_count_reflects_every_path_ever_recorded() {
        let mut tracker = FreshnessTracker::new();
        assert_eq!(tracker.tracked_count(), 0);
        tracker.record_verification("a", Timestamp::from_millis(1), "x");
        tracker.record_divergence("b", "drift", Timestamp::from_millis(2));
        assert_eq!(tracker.tracked_count(), 2);
    }

    // -----------------------------------------------------------------
    // Round 8, item 4: the index's corpus is keyed by section, a
    // divergence by file.
    // -----------------------------------------------------------------

    #[test]
    fn ori_p1_026_a_divergence_filed_against_a_file_is_listed_when_the_corpus_is_the_indexs_sections()
     {
        // The review: list_stale fed from Indexer::all_documents, as its doc
        // says, gets keys of the form <file>#<anchor>, while the drift
        // audit files a divergence against the file, the Document of
        // spec/DATA_MODEL.md section 2. Round 7 read the file's path as no
        // longer in the repository: with every indexed section verified,
        // the divergences filed against the product requirements and the
        // criteria file were dropped, and the report read as ready.
        use crate::indexer::DocumentKind;
        use crate::indexer::IndexableDocument;
        use crate::indexer::Indexer;

        let prd = ["spec", "PRD.md"].join("/");
        let criteria = ["spec", "criteria", "phase-1.md"].join("/");
        let role = ["spec", "agents", "qa.md"].join("/");
        let hashed = ["spec", "runbooks", "a#b.md"].join("/");
        let documents = vec![
            IndexableDocument::new(
                format!("{prd}#1-users"),
                DocumentKind::Section,
                "1. Users",
                "users text",
            ),
            IndexableDocument::new(
                format!("{prd}#2-seats"),
                DocumentKind::Section,
                "2. Seats",
                "seats text",
            ),
            IndexableDocument::new(
                format!("{criteria}#ORI-P1-026"),
                DocumentKind::Criterion,
                "ORI-P1-026",
                "| ORI-P1-026 | F |",
            ),
            IndexableDocument::new(
                role.clone(),
                DocumentKind::Section,
                role.clone(),
                "role text",
            ),
            IndexableDocument::new(
                format!("{hashed}#steps"),
                DocumentKind::Section,
                "Steps",
                "steps text",
            ),
        ];
        let mut indexer = Indexer::open_in_memory().expect("an in-memory index opens");
        indexer.full_rebuild(&documents).expect("build the index");
        let current: BTreeMap<String, String> = indexer
            .all_documents()
            .expect("read the corpus back")
            .into_iter()
            .map(|(document, _)| (document.path, document.body))
            .collect();
        assert!(
            !current.contains_key(&prd) && !current.contains_key(&criteria),
            "the precondition: neither file is itself a key of the index's corpus"
        );

        let mut tracker = FreshnessTracker::new();
        for (path, body) in &current {
            tracker.record_verification(path.clone(), Timestamp::from_millis(1_000), body.clone());
        }
        let found = Timestamp::from_millis(2_000);
        tracker.record_divergence(prd.clone(), "PRD-DIVERGENCE: K-02 drifted", found);
        tracker.record_divergence(criteria.clone(), "a criterion the code misses", found);
        // Gone: a file with nothing in the corpus, and the path a file
        // whose own name holds a '#' would give if cut at that '#'.
        tracker.record_divergence(["docs", "REMOVED.md"].join("/"), "gone", found);
        tracker.record_divergence(["spec", "runbooks", "a"].join("/"), "no such file", found);

        let report = tracker.list_stale(&current, &nothing_left_out());
        assert_eq!(report.documents_covered, 5);
        let listed: Vec<&str> = report.stale.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(
            listed,
            [prd.as_str(), criteria.as_str()],
            "each file-level divergence is listed under its file: {report:?}"
        );
        assert!(
            report.stale[0]
                .reason
                .divergence()
                .contains("PRD-DIVERGENCE: K-02 drifted"),
            "with the drift audit's own text: {report:?}"
        );

        // A file whose own name holds a '#' is the file of its sections.
        tracker.record_divergence(hashed.clone(), "hashed divergence", found);
        assert!(
            tracker
                .list_stale(&current, &nothing_left_out())
                .stale
                .iter()
                .any(|document| document.path == hashed)
        );

        // A verification of the file is the re-review that ends it.
        tracker.record_verification(prd.clone(), Timestamp::from_millis(3_000), "the whole file");
        let listed: Vec<String> = tracker
            .list_stale(&current, &nothing_left_out())
            .stale
            .into_iter()
            .map(|document| document.path)
            .collect();
        assert_eq!(listed, [criteria, hashed]);
    }

    // -----------------------------------------------------------------
    // Round 9, item 1: a record on something the walk left out, a file
    // still in the repository, is listed with the walk's reason and any
    // divergence, never dropped as if the file were gone.
    // -----------------------------------------------------------------

    #[test]
    fn ori_p1_026_every_record_on_something_the_walk_left_out_is_listed_with_the_reason_and_the_divergence()
     {
        let file = |name: &str| ["spec", name].join("/");
        let unreadable = file("EVENTS.md");
        let adr = ["spec", "adr", "ADR-0002-single.md"].join("/");
        let linked = file("LINKED.md");
        let locked_dir = ["spec", "runbooks"].join("/");
        let in_locked_dir = format!("{locked_dir}/deploy.md#deploy");
        let untracked = file("UNTRACKED.md");
        let removed = file("REMOVED.md");
        let indexed = format!("{}#prd", file("PRD.md"));

        let mut tracker = FreshnessTracker::new();
        let verified = Timestamp::from_millis(1_000);
        let found = Timestamp::from_millis(2_000);
        tracker.record_verification(format!("{unreadable}#stream"), verified, "stream text");
        tracker.record_divergence(unreadable.clone(), "EVENTS-DIVERGENCE", found);
        tracker.record_verification(adr.clone(), verified, "adr text");
        tracker.record_divergence(adr.clone(), "ADR-DIVERGENCE", found);
        tracker.record_divergence(linked.clone(), "LINKED-DIVERGENCE", found);
        tracker.record_verification(in_locked_dir.clone(), verified, "deploy text");
        tracker.record_divergence(removed.clone(), "gone with its file", found);
        tracker.record_verification(indexed.clone(), verified, "prd text");

        let current = corpus(&[(indexed.as_str(), "prd text")]);
        let not_indexed: NotIndexed = [
            (
                unreadable.clone(),
                SkipReason::Unreadable {
                    error: "stream did not contain valid UTF-8".to_owned(),
                },
            ),
            (
                adr.clone(),
                SkipReason::DocumentTooLarge {
                    path: adr.clone(),
                    byte_len: 70_000,
                },
            ),
            (linked.clone(), SkipReason::Symlink),
            (
                locked_dir.clone(),
                SkipReason::Unreadable {
                    error: "permission denied".to_owned(),
                },
            ),
            (
                untracked.clone(),
                SkipReason::FileTooLarge { byte_len: 2 << 20 },
            ),
        ]
        .into_iter()
        .collect();

        let report = tracker.list_stale(&current, &not_indexed);
        assert_eq!(report.documents_covered, 1);
        let listed: Vec<(&str, &StaleReason)> = report
            .stale
            .iter()
            .map(|document| (document.path.as_str(), &document.reason))
            .collect();
        let divergence = |text: &str| Some((text.to_owned(), found));
        let expected: Vec<(String, StaleReason)> = vec![
            (
                unreadable.clone(),
                StaleReason::NotIndexed {
                    skipped: not_indexed[&unreadable].clone(),
                    divergence: divergence("EVENTS-DIVERGENCE"),
                },
            ),
            (
                format!("{unreadable}#stream"),
                StaleReason::NotIndexed {
                    skipped: not_indexed[&unreadable].clone(),
                    divergence: None,
                },
            ),
            (
                linked.clone(),
                StaleReason::NotIndexed {
                    skipped: SkipReason::Symlink,
                    divergence: divergence("LINKED-DIVERGENCE"),
                },
            ),
            (
                adr.clone(),
                StaleReason::NotIndexed {
                    skipped: not_indexed[&adr].clone(),
                    divergence: divergence("ADR-DIVERGENCE"),
                },
            ),
            (
                in_locked_dir.clone(),
                StaleReason::NotIndexed {
                    skipped: not_indexed[&locked_dir].clone(),
                    divergence: None,
                },
            ),
        ];
        assert_eq!(
            listed,
            expected
                .iter()
                .map(|(path, reason)| (path.as_str(), reason))
                .collect::<Vec<_>>(),
            "every record on something left out is listed, in path order, with the \
             walk's reason and its divergence; the removed file's and the fresh \
             document's are not, and no record is invented for an untracked left-out file"
        );
        // The content rule: every entry left out is freshness unknown, the
        // untracked file with no record on it included, so the report is
        // not ready; the removed file is in no walk, and is not.
        let unknown: Vec<(&str, &[SkipReason])> = report
            .freshness_unknown
            .iter()
            .map(|file| (file.path.as_str(), file.skipped.as_slice()))
            .collect();
        let mut expected_unknown: Vec<(&str, &[SkipReason])> = not_indexed
            .entries()
            .iter()
            .map(|entry| (entry.key.as_str(), std::slice::from_ref(&entry.reason)))
            .collect();
        expected_unknown.sort_by(|left, right| left.0.cmp(right.0));
        assert_eq!(unknown, expected_unknown);
        assert!(
            unknown.iter().any(|(path, _)| *path == untracked)
                && unknown.iter().all(|(path, _)| *path != removed),
            "{unknown:?}"
        );
        assert!(!report.is_ready(), "{report:?}");
        let line = report.stale[0].reason.divergence();
        assert!(
            line.contains("EVENTS-DIVERGENCE") && line.contains("valid UTF-8"),
            "the line carries the drift audit's text and the walk's reason: {line}"
        );
        assert!(
            report.stale[1]
                .reason
                .divergence()
                .contains("not in the index"),
            "{:?}",
            report.stale[1]
        );

        // The same records with nothing left out: the round 8 reading, in
        // which each of these files is gone, for comparison. Only there is
        // the report ready.
        let gone = tracker.list_stale(&current, &nothing_left_out());
        assert!(gone.stale.is_empty() && gone.freshness_unknown.is_empty());
        assert!(gone.is_ready(), "{gone:?}");

        // A re-review that verifies the file again ends its divergence; the
        // record is still listed while the walk leaves the file out.
        tracker.record_verification(unreadable.clone(), Timestamp::from_millis(3_000), "x");
        let after = tracker.list_stale(&current, &not_indexed);
        let events = after
            .stale
            .iter()
            .find(|document| document.path == unreadable)
            .expect("still listed");
        assert_eq!(
            events.reason,
            StaleReason::NotIndexed {
                skipped: not_indexed[&unreadable].clone(),
                divergence: None,
            }
        );
    }

    // -----------------------------------------------------------------
    // Round 10: a skip entry says whether it covers one document or a
    // whole file, and a record is matched by that.
    // -----------------------------------------------------------------

    #[test]
    fn ori_p1_026_a_document_scoped_skip_covers_its_own_document_and_its_files_divergence_never_a_sibling()
     {
        let too_large = |path: &str| SkipReason::DocumentTooLarge {
            path: path.to_owned(),
            byte_len: 70_000,
        };
        assert_eq!(too_large("x").scope(), SkipScope::Document);
        assert_eq!(SkipReason::Symlink.scope(), SkipScope::File);
        assert_eq!(
            SkipReason::FileTooLarge { byte_len: 2 << 20 }.scope(),
            SkipScope::File
        );

        // A file of one section, past the cap, with a divergence on the
        // file; a file whose preamble is past the cap, with one section
        // kept and one removed; and a file of two sections, one past the
        // cap, with a divergence on the file.
        let single = ["spec", "RISK_MAP.md"].join("/");
        let single_section = format!("{single}#risk-map");
        let preamble = ["spec", "PREAMBLE.md"].join("/");
        let kept = format!("{preamble}#kept");
        let removed = format!("{preamble}#removed");
        let split = ["spec", "SPLIT.md"].join("/");
        let split_indexed = format!("{split}#small");
        let split_large = format!("{split}#large");

        let mut tracker = FreshnessTracker::new();
        let verified = Timestamp::from_millis(1_000);
        let found = Timestamp::from_millis(2_000);
        tracker.record_verification(single_section.clone(), verified, "risk text");
        tracker.record_divergence(single.clone(), "SINGLE-DIVERGENCE", found);
        tracker.record_verification(preamble.clone(), verified, "preamble text");
        tracker.record_verification(kept.clone(), verified, "kept text");
        tracker.record_verification(removed.clone(), verified, "removed text");
        tracker.record_divergence(removed.clone(), "REMOVED-DIVERGENCE", found);
        tracker.record_verification(split_indexed.clone(), verified, "small text");
        tracker.record_divergence(split.clone(), "SPLIT-DIVERGENCE", found);

        let current = corpus(&[
            (kept.as_str(), "kept text"),
            (split_indexed.as_str(), "small text"),
        ]);
        let not_indexed: NotIndexed = [
            (single_section.clone(), too_large(&single_section)),
            (preamble.clone(), too_large(&preamble)),
            (split_large.clone(), too_large(&split_large)),
        ]
        .into_iter()
        .collect();

        let report = tracker.list_stale(&current, &not_indexed);
        assert_eq!(report.documents_covered, 2);
        let divergence = |text: &str| Some((text.to_owned(), found));
        let expected = vec![
            StaleDocument {
                path: preamble.clone(),
                reason: StaleReason::NotIndexed {
                    skipped: too_large(&preamble),
                    divergence: None,
                },
            },
            StaleDocument {
                path: single.clone(),
                reason: StaleReason::NotIndexed {
                    skipped: too_large(&single_section),
                    divergence: divergence("SINGLE-DIVERGENCE"),
                },
            },
            StaleDocument {
                path: single_section.clone(),
                reason: StaleReason::NotIndexed {
                    skipped: too_large(&single_section),
                    divergence: None,
                },
            },
            StaleDocument {
                path: split.clone(),
                reason: StaleReason::Divergence {
                    description: "SPLIT-DIVERGENCE".to_owned(),
                    detected_at: found,
                },
            },
        ];
        assert_eq!(
            report.stale, expected,
            "the preamble and the single section are listed as left out, the single \
             file's divergence with them; the split file's divergence beside its indexed \
             section; the section removed from the preamble's file is removed"
        );

        // The same entries read as whole files would pin the removed
        // section to the preamble's skip; read as documents they do not.
        assert!(
            report.stale.iter().all(|document| document.path != removed),
            "{report:?}"
        );

        // A re-review of the single file ends its divergence while its one
        // section is still past the cap; the section's record stays, and so
        // does the file's, its freshness unknown (round 10's review: the
        // file's record was dropped here).
        tracker.record_verification(single.clone(), Timestamp::from_millis(3_000), "the file");
        let listed: Vec<String> = tracker
            .list_stale(&current, &not_indexed)
            .stale
            .into_iter()
            .map(|document| document.path)
            .collect();
        assert_eq!(listed, [preamble, single, single_section, split]);
    }

    // -----------------------------------------------------------------
    // Round 11: freshness unknown is never fresh. Every record on a file
    // part or all of whose text the walk left out for its size is listed,
    // a verification alone included.
    // -----------------------------------------------------------------

    #[test]
    fn ori_p1_026_a_verification_alone_on_a_file_whose_documents_are_past_the_cap_is_listed_as_freshness_unknown()
     {
        let too_large = |path: &str| SkipReason::DocumentTooLarge {
            path: path.to_owned(),
            byte_len: 80_000,
        };
        // The round 10 check's three shapes, every document of each file
        // left out for its size and keyed the way the walk keys it; a file
        // with one section indexed and one left out; a file wholly indexed;
        // and a file genuinely gone.
        let risk = ["spec", "RISK_MAP.md"].join("/");
        let risk_section = format!("{risk}#risk-map");
        let runbook = ["spec", "runbooks", "recover-engine.md"].join("/");
        let (one, two) = (format!("{runbook}#one"), format!("{runbook}#two"));
        let criteria = ["spec", "criteria", "phase-1.md"].join("/");
        let row = format!("{criteria}#ORI-P1-026");
        let split = ["spec", "SPLIT.md"].join("/");
        let (small, large, removed) = (
            format!("{split}#small"),
            format!("{split}#large"),
            format!("{split}#removed"),
        );
        let whole = ["spec", "WHOLE.md"].join("/");
        let whole_section = format!("{whole}#whole");
        let gone = ["spec", "GONE.md"].join("/");

        let current = corpus(&[
            (small.as_str(), "small text"),
            (whole_section.as_str(), "whole text"),
        ]);
        let not_indexed: NotIndexed = [&risk_section, &one, &two, &row, &large]
            .into_iter()
            .map(|path| (path.clone(), too_large(path)))
            .collect();

        let verified = Timestamp::from_millis(1_000);
        let found = Timestamp::from_millis(2_000);
        let reviewed = Timestamp::from_millis(3_000);
        let files = [&risk, &runbook, &criteria, &split, &whole, &gone];
        let mut verification_alone = FreshnessTracker::new();
        let mut divergence_cleared = FreshnessTracker::new();
        for tracker in [&mut verification_alone, &mut divergence_cleared] {
            tracker.record_verification(small.clone(), verified, "small text");
            tracker.record_verification(whole_section.clone(), verified, "whole text");
            tracker.record_verification(removed.clone(), verified, "removed text");
        }
        for file in files {
            verification_alone.record_verification(file.clone(), reviewed, "the file");
            divergence_cleared.record_divergence(file.clone(), "found drifting", found);
            divergence_cleared.record_verification(file.clone(), reviewed, "the file");
        }

        let unknown = |file: &String, first: &String| StaleDocument {
            path: file.clone(),
            reason: StaleReason::NotIndexed {
                skipped: too_large(first),
                divergence: None,
            },
        };
        let expected = vec![
            unknown(&risk, &risk_section),
            unknown(&split, &large),
            unknown(&criteria, &row),
            unknown(&runbook, &one),
        ];
        for (label, tracker) in [
            ("a verification alone", &verification_alone),
            (
                "a divergence cleared by a re-verification",
                &divergence_cleared,
            ),
        ] {
            let report = tracker.list_stale(&current, &not_indexed);
            assert_eq!(report.documents_covered, 2, "{label}");
            assert_eq!(
                report.stale, expected,
                "{label}: each file with a document left out for its size is listed as \
                 freshness unknown, with the first such document's reason; the file wholly \
                 indexed, the file gone and the section removed are not"
            );
            assert!(
                report.stale.iter().all(|document| document
                    .reason
                    .divergence()
                    .starts_with("freshness unknown: ")),
                "{label}: {report:?}"
            );

            // With nothing left out, the same files read as removed, the
            // split and whole files' records as checked through their
            // indexed documents: nothing listed. The walk's list is what
            // tells the two apart.
            assert!(
                tracker
                    .list_stale(&current, &nothing_left_out())
                    .stale
                    .is_empty(),
                "{label}"
            );
        }

        // A divergence still on record on the split file is the divergence
        // it is, beside its indexed section; on the risk map, with nothing
        // of it indexed, it is carried with the walk's reason.
        verification_alone.record_divergence(split.clone(), "SPLIT-DIVERGENCE", found);
        verification_alone.record_divergence(risk.clone(), "RISK-DIVERGENCE", found);
        let report = verification_alone.list_stale(&current, &not_indexed);
        let reason_of = |path: &String| {
            report
                .stale
                .iter()
                .find(|document| &document.path == path)
                .map(|document| document.reason.clone())
        };
        assert_eq!(
            reason_of(&split),
            Some(StaleReason::Divergence {
                description: "SPLIT-DIVERGENCE".to_owned(),
                detected_at: found,
            })
        );
        assert_eq!(
            reason_of(&risk),
            Some(StaleReason::NotIndexed {
                skipped: too_large(&risk_section),
                divergence: Some(("RISK-DIVERGENCE".to_owned(), found)),
            })
        );
    }

    // -----------------------------------------------------------------
    // Round 12: readiness is about content, not records. Every file any
    // content of which the walk left out is freshness unknown, whether or
    // not a record names it, and the report is not ready while one is.
    // -----------------------------------------------------------------

    #[test]
    fn ori_p1_026_every_file_left_out_is_freshness_unknown_whether_or_not_a_record_names_it() {
        let file = |name: &str| ["spec", name].join("/");
        // Round 11's shapes: a role file with no heading, indexed whole at
        // its bare path, grown a section past the cap; a file with a short
        // preamble grown a section past the cap between two indexed ones;
        // and a criteria file whose repeated row is left out, its path held
        // in the corpus by the first row. Beside them: a file of two
        // sections both past the cap, a file left out whole, and a
        // directory left out whole.
        let role = ["spec", "agents", "qa.md"].join("/");
        let role_section = format!("{role}#checklist");
        let preamble = file("TESTING.md");
        let (a, b, z) = (
            format!("{preamble}#a"),
            format!("{preamble}#b"),
            format!("{preamble}#z"),
        );
        let criteria = ["spec", "criteria", "phase-9.md"].join("/");
        let (row_1, row_2) = (
            format!("{criteria}#ORI-P9-001"),
            format!("{criteria}#ORI-P9-002"),
        );
        let risk = file("RISK_MAP.md");
        let (one, two) = (format!("{risk}#one"), format!("{risk}#two"));
        let unreadable = file("EVENTS.md");
        let locked = ["spec", "runbooks"].join("/");
        let in_locked = format!("{locked}/deploy.md");
        let kept = format!("{}#prd", file("PRD.md"));

        let current = corpus(&[
            (role.as_str(), "the role text"),
            (preamble.as_str(), "intro"),
            (a.as_str(), "a text"),
            (z.as_str(), "z text"),
            (row_1.as_str(), "| ORI-P9-001 | first |"),
            (row_2.as_str(), "| ORI-P9-002 | other |"),
            (kept.as_str(), "prd text"),
        ]);
        let too_large = |path: &str| SkipReason::DocumentTooLarge {
            path: path.to_owned(),
            byte_len: 70_000,
        };
        let repeat = SkipReason::DuplicateDocumentPath {
            path: row_1.clone(),
        };
        let bad_bytes = SkipReason::Unreadable {
            error: "stream did not contain valid UTF-8".to_owned(),
        };
        let denied = SkipReason::Unreadable {
            error: "permission denied".to_owned(),
        };
        let not_indexed: NotIndexed = [
            (role_section.clone(), too_large(&role_section)),
            (b.clone(), too_large(&b)),
            (row_1.clone(), repeat.clone()),
            (one.clone(), too_large(&one)),
            (two.clone(), too_large(&two)),
            (unreadable.clone(), bad_bytes.clone()),
            (locked.clone(), denied.clone()),
        ]
        .into_iter()
        .collect();
        let mut expected = vec![
            FreshnessUnknown {
                path: role.clone(),
                skipped: vec![too_large(&role_section)],
            },
            FreshnessUnknown {
                path: preamble.clone(),
                skipped: vec![too_large(&b)],
            },
            FreshnessUnknown {
                path: criteria.clone(),
                skipped: vec![repeat],
            },
            FreshnessUnknown {
                path: risk.clone(),
                skipped: vec![too_large(&one), too_large(&two)],
            },
            FreshnessUnknown {
                path: unreadable.clone(),
                skipped: vec![bad_bytes],
            },
            FreshnessUnknown {
                path: locked.clone(),
                skipped: vec![denied],
            },
        ];
        expected.sort_by(|left, right| left.path.cmp(&right.path));

        // No record at all; every indexed document verified at its text and
        // nothing else; and that, with a divergence a re-verification
        // cleared on every file left out (on the role file and the
        // preamble's file, whose bare paths are documents of the corpus,
        // re-verified at the corpus's text).
        let verified = Timestamp::from_millis(1_000);
        let found = Timestamp::from_millis(2_000);
        let reviewed = Timestamp::from_millis(3_000);
        let no_record = FreshnessTracker::new();
        let mut corpus_verified = FreshnessTracker::new();
        for (path, text) in &current {
            corpus_verified.record_verification(path.clone(), verified, text.clone());
        }
        let mut files_reviewed = corpus_verified.clone();
        for path in [&role, &preamble, &criteria, &risk, &unreadable, &in_locked] {
            files_reviewed.record_divergence(path.clone(), "found drifting", found);
            let text = current.get(path).cloned().unwrap_or_default();
            files_reviewed.record_verification(path.clone(), reviewed, text);
        }

        for (label, tracker) in [
            ("no record at all", &no_record),
            ("every indexed document verified", &corpus_verified),
            ("every file left out re-reviewed too", &files_reviewed),
        ] {
            let report = tracker.list_stale(&current, &not_indexed);
            assert_eq!(report.documents_covered, current.len(), "{label}");
            assert_eq!(
                report.freshness_unknown, expected,
                "{label}: every file any content of which was left out, with every reason, \
                 whatever is on record"
            );
            assert!(!report.is_ready(), "{label}: {report:?}");
            for unknown in &report.freshness_unknown {
                let line = unknown.line();
                assert!(
                    line.starts_with("freshness unknown: ")
                        && unknown
                            .skipped
                            .iter()
                            .all(|reason| line.contains(&reason.to_string())),
                    "{label}: {line}"
                );
            }
        }

        // The records alone: with every indexed document verified, no
        // record is stale, so only the content rule refuses readiness.
        let report = corpus_verified.list_stale(&current, &not_indexed);
        assert!(report.stale.is_empty(), "{report:?}");
        // With the files re-reviewed, the records on the role file and the
        // preamble's file are those documents' own, checked against the
        // indexed text and fresh, the shape round 11 read as ready; the
        // others are listed as not indexed.
        let report = files_reviewed.list_stale(&current, &not_indexed);
        let listed: Vec<&str> = report.stale.iter().map(|d| d.path.as_str()).collect();
        let mut expected_listed = vec![
            criteria.as_str(),
            risk.as_str(),
            unreadable.as_str(),
            in_locked.as_str(),
        ];
        expected_listed.sort_unstable();
        assert_eq!(listed, expected_listed, "{report:?}");

        // Nothing left out: ready over the verified corpus, whatever is on
        // record for files no longer there.
        for tracker in [&corpus_verified, &files_reviewed] {
            let report = tracker.list_stale(&current, &nothing_left_out());
            assert!(report.freshness_unknown.is_empty(), "{report:?}");
            assert!(report.is_ready(), "{report:?}");
        }
    }

    #[test]
    fn ori_p1_026_is_ready_is_false_while_a_record_is_stale_or_any_content_is_freshness_unknown() {
        let prd = "spec/PRD.md";
        let left_out = ["spec", "EVENTS.md"].join("/");
        // Round 12 left out a file not named .md here, which is not
        // specification text and no longer refuses readiness; a .md file
        // that is not UTF-8 is, and does.
        let unreadable = SkipReason::Unreadable {
            error: "stream did not contain valid UTF-8".to_owned(),
        };
        let something_left_out: NotIndexed = [(left_out.clone(), unreadable.clone())]
            .into_iter()
            .collect();
        let current = corpus(&[(prd, "text")]);
        let mut fresh = FreshnessTracker::new();
        fresh.record_verification(prd, Timestamp::from_millis(1_000), "text");
        let mut stale = FreshnessTracker::new();
        stale.record_verification(prd, Timestamp::from_millis(1_000), "older text");

        for (tracker, walk, ready) in [
            (&fresh, &nothing_left_out(), true),
            (&stale, &nothing_left_out(), false),
            (&fresh, &something_left_out, false),
            (&stale, &something_left_out, false),
        ] {
            let report = tracker.list_stale(&current, walk);
            assert_eq!(report.documents_covered, 1);
            assert_eq!(
                report.is_ready(),
                ready,
                "ready exactly when nothing is stale and nothing is left out: {report:?}"
            );
            assert_eq!(
                report.is_ready(),
                report.stale.is_empty() && report.freshness_unknown.is_empty()
            );
        }

        // An empty corpus is ready only when nothing was left out either:
        // a repository whose every file was left out has nothing to check,
        // and is not ready.
        let empty = BTreeMap::new();
        let report = fresh.list_stale(&empty, &nothing_left_out());
        assert_eq!(report.documents_covered, 0);
        assert!(report.is_ready(), "{report:?}");
        let report = fresh.list_stale(&empty, &something_left_out);
        assert_eq!(
            report.freshness_unknown,
            [FreshnessUnknown {
                path: left_out,
                skipped: vec![unreadable],
            }]
        );
        assert!(!report.is_ready(), "{report:?}");
    }

    // -----------------------------------------------------------------
    // Round 13: only specification text is freshness unknown; every entry
    // the walk left out is listed, those sharing a key included; and a
    // verification covers a document's title as well as its body.
    // -----------------------------------------------------------------

    /// One of each [`SkipReason`], each with the key the walk gives an
    /// entry of it, and whether it leaves out specification text: the
    /// module doc's rule, restated here by hand, with no wildcard, so a
    /// reason added later does not compile until this test says what it
    /// is.
    fn one_entry_of_each_reason() -> Vec<(String, SkipReason, bool)> {
        let file = |name: &str| ["spec", name].join("/");
        let big = format!("{}#big", file("BIG.md"));
        let reasons = vec![
            (file("LINKED.md"), SkipReason::Symlink),
            (file("pipe.md"), SkipReason::NotARegularFile),
            (
                ["spec", "design", "Ori Studio.html"].join("/"),
                SkipReason::NotMarkdown,
            ),
            (file("caf\u{FFFD}.md"), SkipReason::NonUtf8Path),
            (
                file("EVENTS.md"),
                SkipReason::Unreadable {
                    error: "stream did not contain valid UTF-8".to_owned(),
                },
            ),
            (
                format!(
                    "{}#ORI-P9-001",
                    ["spec", "criteria", "phase-9.md"].join("/")
                ),
                SkipReason::DuplicateDocumentPath {
                    path: format!(
                        "{}#ORI-P9-001",
                        ["spec", "criteria", "phase-9.md"].join("/")
                    ),
                },
            ),
            (
                file("HUGE.md"),
                SkipReason::FileTooLarge { byte_len: 2 << 20 },
            ),
            (
                big.clone(),
                SkipReason::DocumentTooLarge {
                    path: big,
                    byte_len: 70_000,
                },
            ),
            (
                file("OVER.md"),
                SkipReason::WalkDocumentLimit { remaining: 0 },
            ),
            (
                file("MANY.md"),
                SkipReason::FileDocumentLimit { limit: 1_024 },
            ),
            (
                file("PAST.md"),
                SkipReason::WalkByteLimit {
                    byte_len: 1 << 20,
                    remaining: 0,
                },
            ),
        ];
        reasons
            .into_iter()
            .map(|(key, reason)| {
                let specification = match reason {
                    SkipReason::NotMarkdown | SkipReason::NotARegularFile => false,
                    SkipReason::Symlink
                    | SkipReason::NonUtf8Path
                    | SkipReason::Unreadable { .. }
                    | SkipReason::DuplicateDocumentPath { .. }
                    | SkipReason::FileTooLarge { .. }
                    | SkipReason::DocumentTooLarge { .. }
                    | SkipReason::WalkDocumentLimit { .. }
                    | SkipReason::FileDocumentLimit { .. }
                    | SkipReason::WalkByteLimit { .. } => true,
                };
                (key, reason, specification)
            })
            .collect()
    }

    #[test]
    fn ori_p1_026_exactly_the_entries_that_leave_out_specification_text_refuse_readiness() {
        let entries = one_entry_of_each_reason();
        assert_eq!(entries.len(), 11, "one entry of each reason");
        let prd = "spec/PRD.md";
        let current = corpus(&[(prd, "text")]);
        let verified = Timestamp::from_millis(1_000);
        let found = Timestamp::from_millis(2_000);

        for (key, reason, specification) in &entries {
            assert_eq!(
                reason.leaves_out_specification(),
                *specification,
                "{reason:?}"
            );
            let file = match reason.scope() {
                SkipScope::File => key.clone(),
                SkipScope::Document => document_file(key).to_owned(),
            };
            let walk: NotIndexed = [(key.clone(), reason.clone())].into_iter().collect();
            // No record on the entry; a verification alone on it and on its
            // file; and a divergence on each.
            let mut none = FreshnessTracker::new();
            none.record_verification(prd, verified, "text");
            let mut verification = none.clone();
            verification.record_verification(key.clone(), verified, "its text");
            verification.record_verification(file.clone(), verified, "its file");
            let mut divergence = verification.clone();
            divergence.record_divergence(key.clone(), "drifted", found);
            divergence.record_divergence(file.clone(), "drifted", found);

            for (label, tracker) in [
                ("no record", &none),
                ("verified", &verification),
                ("diverged", &divergence),
            ] {
                let report = tracker.list_stale(&current, &walk);
                assert_eq!(report.documents_covered, 1, "{reason:?}, {label}");
                assert_eq!(
                    report.is_ready(),
                    !specification,
                    "{reason:?}, {label}: {report:?}"
                );
                if *specification {
                    assert_eq!(
                        report.freshness_unknown,
                        [FreshnessUnknown {
                            path: file.clone(),
                            skipped: vec![reason.clone()],
                        }],
                        "{reason:?}, {label}"
                    );
                } else {
                    // Not specification text: read as if the walk had
                    // never met it, so nothing is unknown and a record on
                    // it is on nothing the tracker checks.
                    assert!(
                        report.freshness_unknown.is_empty() && report.stale.is_empty(),
                        "{reason:?}, {label}: {report:?}"
                    );
                }
            }
        }

        // All eleven at once: not ready, and exactly the nine files that
        // hold specification text are listed, each with its reason.
        let walk: NotIndexed = entries
            .iter()
            .map(|(key, reason, _)| (key.clone(), reason.clone()))
            .collect();
        let mut none = FreshnessTracker::new();
        none.record_verification(prd, verified, "text");
        let report = none.list_stale(&current, &walk);
        let mut expected: Vec<(String, Vec<SkipReason>)> = entries
            .iter()
            .filter(|(_, _, specification)| *specification)
            .map(|(key, reason, _)| {
                let file = match reason.scope() {
                    SkipScope::File => key.clone(),
                    SkipScope::Document => document_file(key).to_owned(),
                };
                (file, vec![reason.clone()])
            })
            .collect();
        expected.sort_by(|left, right| left.0.cmp(&right.0));
        let listed: Vec<(String, Vec<SkipReason>)> = report
            .freshness_unknown
            .iter()
            .map(|file| (file.path.clone(), file.skipped.clone()))
            .collect();
        assert_eq!(listed, expected);
        assert_eq!(listed.len(), 9);
        assert!(!report.is_ready());
    }

    #[test]
    fn ori_p1_026_entries_sharing_a_key_are_each_listed_and_each_cover_their_records() {
        // A file with a section past the cap, keyed a.md#big, beside a
        // directory the walk could not list named a.md#big; and a criterion
        // row repeated twice, once past the cap. Round 12 kept the first
        // entry of each key: the directory went unlisted, and the record
        // under it with it, and so did the second repeat's reason.
        let file = ["spec", "a.md"].join("/");
        let big = format!("{file}#big");
        let under = format!("{big}/deploy.md");
        let criteria = ["spec", "criteria", "phase-9.md"].join("/");
        let row = format!("{criteria}#ORI-P9-001");
        let too_large = |path: &str| SkipReason::DocumentTooLarge {
            path: path.to_owned(),
            byte_len: 70_000,
        };
        let denied = SkipReason::Unreadable {
            error: "permission denied".to_owned(),
        };
        let repeat = SkipReason::DuplicateDocumentPath { path: row.clone() };
        let walk: NotIndexed = [
            (big.clone(), too_large(&big)),
            (big.clone(), denied.clone()),
            (row.clone(), too_large(&row)),
            (row.clone(), repeat.clone()),
        ]
        .into_iter()
        .collect();
        assert_eq!(walk.len(), 4);
        assert_eq!(
            walk.keys().map(String::as_str).collect::<Vec<_>>(),
            [big.as_str(), row.as_str()]
        );
        assert_eq!(walk.get(&big), Some(&too_large(&big)));

        let current = corpus(&[(row.as_str(), "| ORI-P9-001 | first |")]);
        let mut tracker = FreshnessTracker::new();
        tracker.record_verification(
            row.clone(),
            Timestamp::from_millis(1_000),
            "| ORI-P9-001 | first |",
        );
        let found = Timestamp::from_millis(2_000);
        tracker.record_divergence(under.clone(), "UNDER-DIVERGENCE", found);
        let report = tracker.list_stale(&current, &walk);

        let mut expected = vec![
            FreshnessUnknown {
                path: file,
                skipped: vec![too_large(&big)],
            },
            FreshnessUnknown {
                path: big,
                skipped: vec![denied.clone()],
            },
            FreshnessUnknown {
                path: criteria,
                skipped: vec![too_large(&row), repeat],
            },
        ];
        expected.sort_by(|left, right| left.path.cmp(&right.path));
        assert_eq!(report.freshness_unknown, expected);
        assert_eq!(
            report.stale,
            [StaleDocument {
                path: under,
                reason: StaleReason::NotIndexed {
                    skipped: denied,
                    divergence: Some(("UNDER-DIVERGENCE".to_owned(), found)),
                },
            }],
            "the record under the directory is covered by it, though a document's \
             entry holds the directory's key first"
        );
        assert!(!report.is_ready());
    }

    #[test]
    fn ori_p1_026_a_title_edited_under_the_same_path_is_changed_since_verification() {
        use crate::indexer::DocumentKind;
        use crate::indexer::IndexableDocument;

        let path = format!(
            "{}#retry-then-stop",
            ["spec", "runbooks", "retry.md"].join("/")
        );
        let section = |title: &str, body: &str| {
            IndexableDocument::new(path.clone(), DocumentKind::Section, title, body)
        };
        let before = section("Retry, then stop", "the steps\n");
        let after = section("Retry then stop!", "the steps\n");
        assert_eq!(
            before.body, after.body,
            "the precondition: the body is unchanged"
        );

        let verified = Timestamp::from_millis(1_000);
        let mut tracker = FreshnessTracker::new();
        tracker.record_verification(path.clone(), verified, before.freshness_text());
        assert_eq!(
            tracker.check(&path, &before.freshness_text()),
            Freshness::Fresh
        );
        assert_eq!(
            tracker.check(&path, &after.freshness_text()),
            Freshness::Stale(StaleReason::ChangedSinceVerification {
                verified_at: verified
            })
        );
        let report = tracker.list_stale(
            &corpus(&[(path.as_str(), after.freshness_text().as_str())]),
            &nothing_left_out(),
        );
        assert_eq!(
            report.stale,
            [StaleDocument {
                path: path.clone(),
                reason: StaleReason::ChangedSinceVerification {
                    verified_at: verified
                },
            }]
        );
        assert!(!report.is_ready());

        // Nor does a line moved between the title and the body give the
        // same text.
        assert_ne!(
            section("A", "B\nC").freshness_text(),
            section("A\nB", "C").freshness_text()
        );
    }
}
