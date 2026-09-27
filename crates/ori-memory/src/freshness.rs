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
//! [`FreshnessTracker::list_stale`] has a fourth, for a document it has no
//! text for because the repository walk left it out:
//! [`StaleReason::NotIndexed`], freshness unknown. The file is still in the
//! repository, so nothing on record for it is dropped: every record on
//! something the walk left out is listed with the reason the walk gave, and
//! with the drift audit's divergence when one is on record (`list_stale`'s
//! doc; the indexer's module doc, "A file left out is not a file removed",
//! has what the index holds for such a file: none of its documents).
//!
//! # Freshness unknown is never fresh
//!
//! This tracker decides freshness by comparing text. For a record whose
//! text the walk left out, it has nothing to compare: it cannot tell
//! whether the document changed after the verification on record, so it
//! cannot say the document is fresh, and saying nothing would let a caller
//! read "no stale documents" over a document nobody could check. Every
//! such record is therefore listed as [`StaleReason::NotIndexed`], this
//! module's "freshness unknown" reason, carrying the walk's own reason for
//! leaving the text out, whatever the record holds: a verification alone,
//! a divergence, or a divergence a later verification cleared. Its line
//! ([`StaleReason::divergence`]) says "freshness unknown" in so many
//! words. A review found round 10 dropping a verification alone, and a
//! divergence cleared by a re-verification, on a file the walk read but
//! none of whose documents it indexed, every one past the document cap (a
//! risk map of one heading and 70 KB, a runbook of two such sections, a
//! criteria file of one such row): the stale list was empty, and a
//! product read as ready over a file whose text nothing had checked
//! (`tests::ori_p1_026_a_verification_alone_on_a_file_whose_documents_are_past_the_cap_is_listed_as_freshness_unknown`;
//! the indexer's own tests run the same three shapes through a real walk).
//!
//! What is not listed is a record on something genuinely gone: a file no
//! longer in the repository, or a document no longer in a file the walk
//! read. The walk does not name those, and they have no text to be
//! unknown.
//!
//! # What a skip covers: one document, or a whole file
//!
//! Every entry the walk left out says what it covers, through
//! [`SkipReason::scope`], and a record is matched to it by that scope and
//! nothing else, never by the shape of the entry's key:
//!
//! - A file-scoped entry ([`SkipScope::File`]) is a file, a symbolic link
//!   or a directory the walk did not read. It covers a record at its path,
//!   at any document path of that file (`<file>#<anchor>`) and at anything
//!   under that directory: nothing there was read, so nothing there can be
//!   checked.
//! - A document-scoped entry ([`SkipScope::Document`]) is one document of a
//!   file the walk did read and split, left out for its size. It covers a
//!   record at exactly that document path and no other, even when that
//!   path is the file's bare path (the text above the file's first
//!   heading). The file was read, so every other document path of it is
//!   judged by the corpus: in it and checked, or not in it and removed. A
//!   section genuinely removed from a file whose preamble is past the cap
//!   is removed, never pinned as left out.
//!
//! A record on a file's own path that is not a document path (the drift
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
//! longer in a file the walk read, and every one on a file no longer in the
//! repository.
//!
//! Every listing here can be cleared. A record under a document-scoped
//! entry is checked like any other once its document is back under the cap
//! (fresh when its text is the verified text, or once re-verified), and is
//! removed once the document is removed from its file. A record on a
//! file's own path listed as not indexed stops being listed once no
//! document of the file is left out (each back under the cap, or removed
//! from the file), its file's documents then checked by their own records;
//! a re-verification of the file while a document of it is still left out
//! ends its divergence, but not the listing, since the text still cannot be
//! checked. A file-level divergence beside indexed documents ends with the
//! file's next verification, as it does anywhere.
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
//! before it trusts an empty `stale` list.
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

use ori_core::types::Timestamp;

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
    /// names, so its text could not be checked against anything on record,
    /// while the file is still in the repository: the document itself (a
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
    /// text `content` at that moment.
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

    /// The freshness of one document, given the text it holds right now.
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
    /// and reports every record on something the repository walk left out,
    /// named in `not_indexed`, rather than dropping it.
    ///
    /// `current` is the live corpus a caller (the indexer's
    /// [`crate::indexer::Indexer::all_documents`], most naturally) hands in
    /// at call time, not a set this tracker maintains itself. `not_indexed`
    /// is what the walk the index was synced from left out,
    /// [`crate::indexer::RepoWalk::not_indexed`] of
    /// [`crate::indexer::Indexer::collect_from_repo`]'s walk: every entry
    /// still in the repository that the walk did not read, keyed by the
    /// document path, file path or directory path it names, with the
    /// reason. A caller with no walk, and so nothing left out, passes an
    /// empty map.
    ///
    /// The index's corpus is keyed by section, `<file>#<anchor>` (and a
    /// file's bare path for the text above its first heading), while the
    /// `Document` of `spec/DATA_MODEL.md` section 2, which the drift audit
    /// files a divergence against, is the file. So a divergence on record
    /// for a file whose sections `current` holds is listed, under the
    /// file's path, whether or not `current` has a key at exactly that
    /// path: the file is still in the repository. A review found round 7
    /// dropping it as if the file had been removed, so a product whose
    /// product requirements diverged read as ready, with every one of its
    /// indexed sections verified. A verification recorded at a file's path
    /// says nothing about any one section's text and makes none of them
    /// fresh; each section is judged by its own record.
    ///
    /// Something the walk left out is still in the repository too, though
    /// none of its text is in `current`. Every record whose path is not in
    /// `current` and that an entry of `not_indexed` covers, by that entry's
    /// [`SkipReason::scope`] (the module doc's "What a skip covers"), is
    /// listed as [`StaleReason::NotIndexed`], with the walk's reason and
    /// any divergence on record, whether that record is a verification, a
    /// divergence or both: its text could not be checked, so it is not
    /// fresh, and nothing the drift audit filed disappears while the file
    /// exists. A file-scoped entry covers a record at its path, at a
    /// document of that file, or under that directory; a document-scoped
    /// entry covers a record at exactly its document path, and never a
    /// sibling document of the same file, even when its key is the file's
    /// bare path. A review found round 8 dropping the divergence on a file
    /// one non-UTF-8 byte, or an ADR grown past the document cap, had taken
    /// out of the index, so the product read as ready. An entry left out
    /// with no record on it is not listed: nothing about it was ever
    /// verified or filed, and the walk's own list names it.
    ///
    /// A document-scoped entry also says its file was read and is still in
    /// the repository, and that part of its text was left out. So every
    /// record on that file's own path, a verification alone, a divergence,
    /// or a divergence a later verification cleared, is listed while any
    /// document of the file is left out for its size, even when every one
    /// was and nothing of the file is in `current`: as
    /// [`StaleReason::NotIndexed`], freshness unknown, carrying the first
    /// such entry's reason (in path order) and the divergence on record, if
    /// any. The one exception is a divergence on a file `current` also
    /// holds a document of, listed as [`StaleReason::Divergence`], above. A
    /// review found round 9 dropping the divergence: a single-section file
    /// grown past the document cap left an entry keyed `<file>#<anchor>`,
    /// which the record at the file's path never matched. A review found
    /// round 10 still dropping a verification alone there, and a divergence
    /// a re-verification had cleared, so the stale list was empty over a
    /// file whose text nothing had checked (the module doc's "Freshness
    /// unknown is never fresh").
    ///
    /// A verification alone on the path of a file every document of which
    /// is in `current` is not listed: those documents are each checked
    /// against their own records. Only a record that no entry covers, whose
    /// file has nothing in `current` and no document-scoped entry, is gone,
    /// and not reported at all; so is a record for a document removed from
    /// a file the walk read. That matches the indexer's own rule that a
    /// removed document disappears rather than lingers. A review found
    /// round 9 pinning such a record for good when the file's preamble was
    /// past the document cap, by reading the preamble's entry, keyed at the
    /// file's bare path, as the whole file.
    #[must_use]
    pub fn list_stale(
        &self,
        current: &BTreeMap<String, String>,
        not_indexed: &BTreeMap<String, SkipReason>,
    ) -> StaleReport {
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
        // it left out: the file's path, with the first such entry's reason.
        let mut split_files: BTreeMap<&str, &SkipReason> = BTreeMap::new();
        for (key, reason) in not_indexed {
            if reason.scope() == SkipScope::Document {
                split_files.entry(document_file(key)).or_insert(reason);
            }
        }
        for (path, entry) in &self.documents {
            if current.contains_key(path) {
                continue;
            }
            if let Some(skipped) = left_out(not_indexed, path) {
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
                // out for its size, a verification alone included:
                // freshness unknown, never dropped (the module doc's
                // "Freshness unknown is never fresh").
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

/// Why the walk left out `path`, if an entry of `not_indexed` covers it by
/// that entry's [`SkipReason::scope`]: an entry of either scope keyed at
/// `path` itself, or a file-scoped entry keyed at the file `path` belongs
/// to ([`document_file`]) or at a directory holding that file. A
/// document-scoped entry keyed at the file's bare path (the text above its
/// first heading) is that one document, and never covers a sibling.
fn left_out<'a>(
    not_indexed: &'a BTreeMap<String, SkipReason>,
    path: &str,
) -> Option<&'a SkipReason> {
    if let Some(reason) = not_indexed.get(path) {
        return Some(reason);
    }
    let mut enclosing = document_file(path);
    loop {
        if let Some(reason) = not_indexed
            .get(enclosing)
            .filter(|reason| reason.scope() == SkipScope::File)
        {
            return Some(reason);
        }
        enclosing = enclosing.rsplit_once('/')?.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a walk that left nothing out hands `list_stale`.
    fn nothing_left_out() -> BTreeMap<String, SkipReason> {
        BTreeMap::new()
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
        let not_indexed: BTreeMap<String, SkipReason> = [
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
            (untracked, SkipReason::FileTooLarge { byte_len: 2 << 20 }),
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
             document's are not, and an untracked left-out file is not invented"
        );
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
        // which each of these files is gone, for comparison.
        assert!(
            tracker
                .list_stale(&current, &nothing_left_out())
                .stale
                .is_empty()
        );

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
        let not_indexed: BTreeMap<String, SkipReason> = [
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
        let not_indexed: BTreeMap<String, SkipReason> = [&risk_section, &one, &two, &row, &large]
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
}
