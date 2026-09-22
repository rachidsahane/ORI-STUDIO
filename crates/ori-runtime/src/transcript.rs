//! The session transcript: AICD §17.
//!
//! `spec/LLD.md` section 2 gives this crate `Transcript`, and
//! `spec/DATA_MODEL.md` section 2's `AgentSession` row notes it: "Transcript
//! stored as a file, referenced". [`Transcript`] is the value and the ordered,
//! append-only record; [`Transcript::write_to`] and [`Transcript::read_from`]
//! are the file half, taking a path the caller supplies rather than deriving
//! `<app data>/ori/products/<product_id>/sessions/<session_id>/` itself
//! (`spec/LLD.md` section 6): resolving that root needs the product id and the
//! application data directory, neither of which this crate is given anywhere,
//! and inventing a convention for it here would be a second, competing answer
//! to a question a future ticket has to answer once. The same "path the caller
//! chose" shape [`crate::worktree::Worktree::new`] already uses, for the same
//! reason.
//!
//! # Which audit trail this is, and which it is not
//!
//! AICD §17's principle: "Every agent action is written to an immutable audit
//! trail: which agent, which ticket, which tool, what input, what output,
//! when." A transcript is the per-session half of that sentence: the agent and
//! the ticket are already fixed by the session it belongs to
//! (`crate::session::Session::identity`, `Session::ticket`), so an
//! [`Entry`] carries the part that varies within one session, "which tool,
//! what input, what output, when", plus the attempt it belongs to, which
//! `crate::session::Session` does not track and ORI-P1-009 needs kept.
//!
//! It is not `ori-memory`'s operational memory. CLAUDE.md's load-bearing facts
//! name the crate that writes that: "Only `ori-memory` writes operational
//! memory." `spec/DATA_MODEL.md` section 2 draws the same line from the other
//! side: `Report` and `MemoryRecord` are both noted "also written to ops/" or
//! layered through `ori-memory`'s `Barrier` (`spec/LLD.md` section 2, "Return
//! unsanitized production content in a package" is that crate's must-not); the
//! `AgentSession` row's transcript note carries neither qualifier. A
//! transcript is therefore not sanitized by `Barrier` before anything reads
//! it, and this module cannot lean on that crate for the redaction question
//! below; whatever this module does not do here, nobody else does either.
//!
//! # What stops a credential from reaching a transcript
//!
//! AICD §27's secrets architecture: "A credential that appears anywhere it
//! should not (a log, a ticket, a pull request, a memory record) is rotated
//! immediately and the leak is an incident." A transcript is exactly one more
//! place it should not appear.
//!
//! What stops it here is structural, not a scan. CLAUDE.md's load-bearing
//! facts: "only `ori-runtime` injects an issued credential at spawn and holds
//! none beyond the session." The component that holds the value is the
//! `Injector` (`spec/LLD.md` section 2, not yet built), which places it in the
//! child process's environment or a file at spawn and is done with it;
//! [`Entry::new`] never receives that value from anywhere, because nothing
//! that calls it has it to give. There is no code path from "the broker issued
//! a secret" to "a transcript entry holds it", so there is nothing here for a
//! redaction pass to catch and remove after the fact.
//!
//! What this does **not** claim to stop: an agent's own recorded output
//! echoing a value it read from its environment (`cat .env`, `echo
//! $SOME_TOKEN`) is content this module has no way to recognise as a secret,
//! because a transcript entry is untyped text and this module is never told
//! what the secret's value was. A pattern-matching scan here would answer that
//! question with false confidence on some inputs and silence on others, which
//! is the "present but reporting nothing" defect AICD §39 names; this module
//! says plainly that it does not attempt one, rather than shipping one that
//! would not be believed if it were.
//!
//! Must not: write `spec/` or `ops/` (CLAUDE.md rule 5, `spec/LLD.md` section
//! 2), or hold a credential (CLAUDE.md rule 4; see above).

use core::fmt;
use std::error::Error;
use std::path::Path;
use std::path::PathBuf;

use ori_core::types::Timestamp;

/// One entry of a session transcript: AICD §17.
///
/// The per-action part of AICD §17's audit-trail sentence: "which tool, what
/// input, what output, when", plus the attempt it belongs to. `tool` is
/// absent for an entry that is the agent's own reasoning or a note rather than
/// a tool call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    attempt: u32,
    at: Timestamp,
    tool: Option<String>,
    input: String,
    output: String,
}

impl Entry {
    /// One entry, refusing an attempt numbered zero, a blank tool name given
    /// as `Some`, and an entry with neither input nor output.
    ///
    /// An attempt numbered from one matches
    /// `ori_orchestrator::budgets::Approach::new`'s same refusal, for the same
    /// reason: criterion ORI-P1-009 turns on the attempts being distinguishable
    /// and counted, and a zeroth attempt is not a thing that happened. An entry
    /// with nothing in either field is a line that would occupy a slot in the
    /// record and prove nothing, which is exactly the record this criterion
    /// asks not to be empty.
    pub fn new(
        attempt: u32,
        at: Timestamp,
        tool: Option<&str>,
        input: &str,
        output: &str,
    ) -> Result<Self, TranscriptError> {
        if attempt == 0 {
            return Err(TranscriptError::AttemptNotNumbered);
        }
        let tool = match tool {
            Some(name) if name.trim().is_empty() => {
                return Err(TranscriptError::Blank { field: "tool" });
            }
            Some(name) => Some(name.trim().to_owned()),
            None => None,
        };
        if input.trim().is_empty() && output.trim().is_empty() {
            return Err(TranscriptError::Blank {
                field: "input or output",
            });
        }
        Ok(Self {
            attempt,
            at,
            tool,
            input: input.to_owned(),
            output: output.to_owned(),
        })
    }

    /// Which attempt this entry belongs to, numbered from one.
    #[must_use]
    pub const fn attempt(&self) -> u32 {
        self.attempt
    }

    /// When this entry happened.
    #[must_use]
    pub const fn at(&self) -> Timestamp {
        self.at
    }

    /// The tool this entry is about, absent for the agent's own reasoning.
    #[must_use]
    pub fn tool(&self) -> Option<&str> {
        self.tool.as_deref()
    }

    /// What went in.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    /// What came out.
    #[must_use]
    pub fn output(&self) -> &str {
        &self.output
    }
}

/// A session transcript: an ordered, append-only record of [`Entry`] values:
/// AICD §17.
///
/// # What of ORI-P1-009 this type is answerable for
///
/// The criterion: "Coder session with budget attempts=2 | Headless adapter
/// fails twice | Session ends Blocked; a blocked report record exists with the
/// two attempts; credentials revoked; ticket returns to Queued on re-plan."
/// **This criterion is double-claimed**: `ori_orchestrator::budgets`'s
/// `ori_p1_009_*` tests already prove that a `BlockedReport` built from two
/// numbered approaches is accepted and one missing an attempt is refused. What
/// this module owns is upstream of that: that the record of what happened
/// during each attempt is never lost between the attempt ending and whatever
/// builds the report from it. [`Transcript::record`] refuses nothing about the
/// report's shape (it does not know one exists); it refuses losing what it was
/// already given, which is the property the criterion's "with the two
/// attempts" depends on existing somewhere before a report can be built from
/// it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Transcript {
    entries: Vec<Entry>,
}

impl Transcript {
    /// An empty transcript.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every entry, in the order they were recorded.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// How many entries are recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing has been recorded yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every attempt with at least one entry, sorted and without repeats.
    #[must_use]
    pub fn attempts(&self) -> Vec<u32> {
        let mut seen: Vec<u32> = self.entries.iter().map(Entry::attempt).collect();
        seen.sort_unstable();
        seen.dedup();
        seen
    }

    /// Appends one entry, refusing one timestamped before the last entry
    /// recorded, or numbered for an attempt earlier than the last entry's.
    ///
    /// Both refusals are the same shape as `crate::session::Teardown::record`'s
    /// ordering check: a transcript is a record of what happened, in the order
    /// it happened, and a caller handing entries out of order is either
    /// replaying a stale reading or has two attempts confused with each other.
    pub fn record(&self, entry: Entry) -> Result<Self, TranscriptError> {
        if let Some(last) = self.entries.last() {
            if entry.at < last.at {
                return Err(TranscriptError::OutOfOrder {
                    at: entry.at,
                    last: last.at,
                });
            }
            if entry.attempt < last.attempt {
                return Err(TranscriptError::AttemptWentBackward {
                    attempt: entry.attempt,
                    last: last.attempt,
                });
            }
        }
        let mut next = self.entries.clone();
        next.push(entry);
        Ok(Self { entries: next })
    }

    /// Writes every entry to `path`, overwriting whatever was there.
    ///
    /// Whole-file rewrite rather than a true append is deliberate: it makes
    /// every write idempotent and leaves no way for a file on disk to hold an
    /// entry this value does not, which is the same property
    /// [`Transcript::read_from`] depends on to round-trip.
    pub fn write_to(&self, path: &Path) -> Result<(), TranscriptError> {
        let mut content = String::new();
        for entry in &self.entries {
            content.push_str(&render(entry));
        }
        std::fs::write(path, content).map_err(|source| TranscriptError::Io {
            path: path.to_path_buf(),
            message: source.to_string(),
        })
    }

    /// Reads a transcript back from a file [`Transcript::write_to`] wrote.
    ///
    /// Every line is parsed into an [`Entry`] and recorded through
    /// [`Transcript::record`], so a file whose lines are not in the order this
    /// module would have written them is refused the same way an out-of-order
    /// [`Transcript::record`] call is, rather than silently accepted.
    pub fn read_from(path: &Path) -> Result<Self, TranscriptError> {
        let content = std::fs::read_to_string(path).map_err(|source| TranscriptError::Io {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?;
        let mut transcript = Self::new();
        for (index, line) in content.lines().enumerate() {
            let entry = parse(line, path, index + 1)?;
            transcript = transcript
                .record(entry)
                .map_err(|_| TranscriptError::Corrupt {
                    path: path.to_path_buf(),
                    line: index + 1,
                    reason: "entries are not in the order this module records them",
                })?;
        }
        Ok(transcript)
    }
}

/// One field, escaped so a tab or a newline in it cannot be mistaken for the
/// field separator or the line separator [`render`] uses.
fn escape(field: &str) -> String {
    field
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// The inverse of [`escape`].
fn unescape(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// One entry as one line: `attempt`, `at` in milliseconds, the tool field
/// (`T<escaped name>` or `N`), the escaped input, the escaped output, each
/// separated by a tab, ending in a newline.
fn render(entry: &Entry) -> String {
    let tool_field = match entry.tool() {
        Some(name) => format!("T{}", escape(name)),
        None => "N".to_owned(),
    };
    format!(
        "{}\t{}\t{}\t{}\t{}\n",
        entry.attempt,
        entry.at.millis(),
        tool_field,
        escape(entry.input()),
        escape(entry.output())
    )
}

/// The inverse of [`render`] for one line, naming the file and the line number
/// in every refusal so a human reading [`TranscriptError::Corrupt`] knows
/// where to look.
fn parse(line: &str, path: &Path, line_no: usize) -> Result<Entry, TranscriptError> {
    let corrupt = |reason: &'static str| TranscriptError::Corrupt {
        path: path.to_path_buf(),
        line: line_no,
        reason,
    };
    let mut fields = line.splitn(5, '\t');
    let attempt: u32 = fields
        .next()
        .ok_or_else(|| corrupt("missing the attempt field"))?
        .parse()
        .map_err(|_| corrupt("the attempt field is not a number"))?;
    let millis: i64 = fields
        .next()
        .ok_or_else(|| corrupt("missing the timestamp field"))?
        .parse()
        .map_err(|_| corrupt("the timestamp field is not a number"))?;
    let tool_field = fields
        .next()
        .ok_or_else(|| corrupt("missing the tool field"))?;
    let tool = match tool_field.as_bytes().first() {
        Some(b'T') => Some(unescape(&tool_field[1..])),
        Some(b'N') => None,
        _ => return Err(corrupt("the tool field carries no T or N marker")),
    };
    let input = unescape(
        fields
            .next()
            .ok_or_else(|| corrupt("missing the input field"))?,
    );
    let output = unescape(
        fields
            .next()
            .ok_or_else(|| corrupt("missing the output field"))?,
    );
    Entry::new(
        attempt,
        Timestamp::from_millis(millis),
        tool.as_deref(),
        &input,
        &output,
    )
    .map_err(|_| corrupt("the recorded fields do not form a valid entry"))
}

/// Everything this module refuses or cannot read: AICD §17.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TranscriptError {
    /// An entry was recorded against attempt zero.
    AttemptNotNumbered,
    /// A required field was blank.
    Blank {
        /// The field, named as [`Entry::new`]'s doc comment names it.
        field: &'static str,
    },
    /// An entry was timestamped before the last one recorded.
    OutOfOrder {
        /// The time the new entry claims.
        at: Timestamp,
        /// The time the last recorded entry claims.
        last: Timestamp,
    },
    /// An entry's attempt number is lower than the last one recorded.
    AttemptWentBackward {
        /// The attempt the new entry claims.
        attempt: u32,
        /// The attempt the last recorded entry claims.
        last: u32,
    },
    /// The filesystem refused the read or the write.
    Io {
        /// The path involved.
        path: PathBuf,
        /// What the operating system reported.
        message: String,
    },
    /// A file [`Transcript::read_from`] read does not parse as one this
    /// module would have written.
    Corrupt {
        /// The file.
        path: PathBuf,
        /// The line, numbered from one.
        line: usize,
        /// What about it did not parse.
        reason: &'static str,
    },
}

impl TranscriptError {
    /// The methodology section this refusal rests on: AICD §17, the section
    /// that makes the record a control in the first place. The two variants
    /// that report a filesystem or a parse failure rather than an ordering
    /// rule return `None`, the same split `crate::session::SessionError`
    /// draws between a refusal and a value failing to parse.
    ///
    /// `spec/CONVENTIONS.md` names `thiserror` as the house convention; ruling
    /// R20 in `ops/rulings.md` defers it and states what stands until then, "a
    /// hand-written `Display` and `std::error::Error` implementation [...],
    /// with a comment naming the conversion", which is what this is.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        !matches!(self, Self::Io { .. } | Self::Corrupt { .. })
    }
}

impl fmt::Display for TranscriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AttemptNotNumbered => f.write_str(
                "an entry is recorded against attempt 0, and attempts are numbered from 1",
            ),
            Self::Blank { field } => write!(
                f,
                "a transcript entry needs \"{field}\" and this one leaves it blank"
            ),
            Self::OutOfOrder { at, last } => {
                write!(
                    f,
                    "an entry at {at} is before the last one recorded, at {last}"
                )
            }
            Self::AttemptWentBackward { attempt, last } => write!(
                f,
                "an entry for attempt {attempt} follows one for attempt {last}, which is backward"
            ),
            Self::Io { path, message } => {
                write!(
                    f,
                    "{} could not be read or written: {message}",
                    path.display()
                )
            }
            Self::Corrupt { path, line, reason } => {
                write!(
                    f,
                    "{}:{line} does not parse as a transcript entry: {reason}",
                    path.display()
                )
            }
        }
    }
}

impl Error for TranscriptError {}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    use super::*;

    const START: Timestamp = Timestamp::from_millis(1_700_000_000_000);

    fn at(offset_s: i64) -> Timestamp {
        Timestamp::from_millis(START.millis() + offset_s * 1000)
    }

    fn entry(attempt: u32, offset_s: i64, output: &str) -> Entry {
        Entry::new(
            attempt,
            at(offset_s),
            Some("headless-adapter"),
            "run the tests",
            output,
        )
        .expect("a well-formed entry")
    }

    /// An absolute path, on every platform, that names nothing on disk, under
    /// a fresh scratch directory this test owns. Mirrors
    /// `crate::worktree::tests::absolute` and `crate::session::tests::absent`:
    /// the same product shipped a Windows failure once from a fixture written
    /// `/nowhere/...`, which is absolute on unix and not on Windows.
    fn scratch_file(label: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir();
        assert!(
            root.is_absolute(),
            "the temporary directory is absolute on every platform this runs on: {}",
            root.display()
        );
        root.join(format!(
            "ori-t-0034-transcript-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    // ---------------------------------------------------------------------
    // Prove it 7: a transcript that loses an attempt must fail
    // ---------------------------------------------------------------------

    #[test]
    fn ori_p1_009_a_transcript_holds_both_of_two_failed_attempts() {
        let transcript = Transcript::new()
            .record(entry(1, 10, "the adapter exited 1: assertion failed"))
            .expect("first attempt recorded")
            .record(entry(
                2,
                620,
                "the adapter exited 1: a different assertion failed",
            ))
            .expect("second attempt recorded");
        assert_eq!(
            transcript.attempts(),
            vec![1, 2],
            "ORI-P1-009 turns on a record existing with the two attempts"
        );
        assert_eq!(transcript.len(), 2);
    }

    #[test]
    fn ori_t_0034_recording_a_third_attempt_does_not_disturb_the_first_two() {
        let transcript = Transcript::new()
            .record(entry(1, 10, "first"))
            .expect("first")
            .record(entry(2, 20, "second"))
            .expect("second")
            .record(entry(3, 30, "third"))
            .expect("third");
        assert_eq!(transcript.attempts(), vec![1, 2, 3]);
        assert_eq!(transcript.entries()[0].output(), "first");
        assert_eq!(transcript.entries()[1].output(), "second");
    }

    // ---------------------------------------------------------------------
    // Refusals
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_an_entry_for_attempt_zero_is_refused() {
        let refusal = Entry::new(0, START, None, "x", "y").expect_err("attempts start at 1");
        assert_eq!(refusal, TranscriptError::AttemptNotNumbered);
        assert!(refusal.is_refusal());
    }

    #[test]
    fn ori_t_0034_an_entry_with_neither_input_nor_output_is_refused() {
        let refusal = Entry::new(1, START, None, "  ", "").expect_err("nothing recorded");
        assert_eq!(
            refusal,
            TranscriptError::Blank {
                field: "input or output"
            }
        );
    }

    #[test]
    fn ori_t_0034_a_blank_tool_name_given_as_some_is_refused() {
        let refusal = Entry::new(1, START, Some("  "), "in", "out").expect_err("blank tool");
        assert_eq!(refusal, TranscriptError::Blank { field: "tool" });
    }

    #[test]
    fn ori_t_0034_an_entry_absent_a_tool_is_the_agents_own_note() {
        let note = Entry::new(1, START, None, "thinking about the failure", "").expect("a note");
        assert_eq!(note.tool(), None);
    }

    #[test]
    fn ori_t_0034_an_out_of_order_entry_is_refused() {
        let transcript = Transcript::new()
            .record(entry(1, 100, "first"))
            .expect("first");
        let refusal = transcript
            .record(entry(1, 50, "earlier"))
            .expect_err("time does not run backward in a transcript");
        assert_eq!(
            refusal,
            TranscriptError::OutOfOrder {
                at: at(50),
                last: at(100),
            }
        );
    }

    #[test]
    fn ori_t_0034_an_attempt_number_going_backward_is_refused() {
        let transcript = Transcript::new()
            .record(entry(2, 10, "second"))
            .expect("second");
        let refusal = transcript
            .record(entry(1, 20, "first, arriving late"))
            .expect_err("attempts do not renumber backward");
        assert_eq!(
            refusal,
            TranscriptError::AttemptWentBackward {
                attempt: 1,
                last: 2
            }
        );
    }

    #[test]
    fn ori_t_0034_a_refused_record_call_leaves_the_transcript_with_what_it_had() {
        let transcript = Transcript::new()
            .record(entry(1, 100, "first"))
            .expect("first");
        let refusal = transcript.record(entry(1, 50, "earlier"));
        assert!(refusal.is_err());
        assert_eq!(
            transcript.attempts(),
            vec![1],
            "the caller's copy is untouched"
        );
    }

    // ---------------------------------------------------------------------
    // File round trip
    // ---------------------------------------------------------------------

    #[test]
    fn ori_t_0034_a_transcript_written_and_read_back_is_the_same_transcript() {
        let path = scratch_file("roundtrip");
        let transcript = Transcript::new()
            .record(entry(1, 10, "the adapter exited 1: assertion failed"))
            .expect("first")
            .record(
                Entry::new(
                    2,
                    at(620),
                    None,
                    "",
                    "no tool this time, a bare note\nwith a newline\tand a tab",
                )
                .expect("a note with tricky characters"),
            )
            .expect("second");

        transcript
            .write_to(&path)
            .expect("the scratch directory is writable");
        let read_back = Transcript::read_from(&path).expect("a file this module wrote");
        let _ = std::fs::remove_file(&path);

        assert_eq!(read_back, transcript);
        assert_eq!(read_back.attempts(), vec![1, 2]);
        assert_eq!(
            read_back.entries()[1].output(),
            "no tool this time, a bare note\nwith a newline\tand a tab"
        );
    }

    #[test]
    fn ori_t_0034_reading_a_missing_file_is_an_io_error_not_an_empty_transcript() {
        let path = scratch_file("absent");
        let refusal = Transcript::read_from(&path).expect_err("nothing is there to read");
        assert!(matches!(refusal, TranscriptError::Io { .. }));
        assert!(
            !refusal.is_refusal(),
            "a missing file is not a control refusing an action"
        );
    }

    #[test]
    fn ori_t_0034_a_line_with_a_stray_tool_marker_is_reported_corrupt_with_its_line_number() {
        let path = scratch_file("corrupt");
        std::fs::write(&path, "1\t0\tX\tin\tout\n").expect("writable");
        let refusal = Transcript::read_from(&path).expect_err("X is neither T nor N");
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            refusal,
            TranscriptError::Corrupt {
                path: path.clone(),
                line: 1,
                reason: "the tool field carries no T or N marker",
            }
        );
    }

    #[test]
    fn ori_t_0034_write_to_is_idempotent_and_does_not_duplicate_entries() {
        let path = scratch_file("idempotent");
        let transcript = Transcript::new()
            .record(entry(1, 10, "first"))
            .expect("first");
        transcript.write_to(&path).expect("writable");
        transcript.write_to(&path).expect("writable again");
        let read_back = Transcript::read_from(&path).expect("readable");
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            read_back.len(),
            1,
            "the second write overwrote, not appended"
        );
    }
}
