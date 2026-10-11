//! The operational log: the read side of AICD §8's layer 3, over the
//! append-only structured records the sanitization barrier writes.
//!
//! `spec/PRD.md` section 4.13, row K-01: "operational (append-only structured
//! records)". `spec/DATA_MODEL.md` section 2: `MemoryRecord | id, product_id,
//! layer (operational), kind, structured (json, sanitized), provenance
//! (source, identity, ticket, time), untrusted (bool)`. AICD §8 layer 3:
//! "Never edited, only appended."
//!
//! # What this module owes beyond `crate::barrier`
//!
//! `crate::barrier::read_records` already projects every
//! `memory.record_created` event of a product's log back into a typed
//! [`crate::barrier::MemoryRecord`], in `seq` order, through
//! [`ori_store::event_log::EventLog::verify`] first. What it does not give
//! is where in the log a record sits: a `MemoryRecord` has no `seq`, and
//! provenance in an append-only log is its position. A package that tells an
//! operator "this item is a record" without saying which event of the log it
//! is cannot be checked against the log without re-running the query
//! (`spec/PRD.md` K-06, "inspectable by the operator"). This module owes
//! exactly that, and the lookups a caller needs without re-deriving them:
//!
//! - **Log position.** Every [`OplogEntry`] carries the `seq` of the event it
//!   was read from and its ordinal among the product's operational records.
//!   The `seq` is read from the log's own events and paired with the
//!   barrier's records one to one; the pairing is refused
//!   ([`OplogError::Inconsistent`]) if the two reads disagree on the number
//!   of records or on any record's actor, time or ticket, so an entry never
//!   carries another record's `seq`.
//! - **Log order.** [`OperationalLog::entries`] is in the order the records
//!   were appended, oldest first. Nothing here sorts by anything else.
//! - **By ticket, by kind, by id.** [`OperationalLog::for_ticket`],
//!   [`OperationalLog::of_kind`] and [`OperationalLog::with_id`]. A record id
//!   is supplied by the report's submitter and nothing makes it unique
//!   (`crate::scope`'s module doc, "Raw evidence, and which blobs are a
//!   record's"), so `with_id` yields every entry carrying it, never the
//!   first.
//! - **The tip.** [`OperationalLog::tip_seq`], the last `seq` of the log the
//!   records were read at, so a package can say what it was assembled as of.
//!
//! # What this module deliberately does not do
//!
//! - **No lookup by module.** A record carries the ticket it was filed on and
//!   no module (`spec/DATA_MODEL.md` section 2's provenance: "source,
//!   identity, ticket, time"). Which modules a ticket touched is the lock
//!   history, and `crate::scope` already decides, once, which records lie
//!   inside a coder's declared scope ("Which records are inside a declared
//!   scope"). A second reading of the lock history here would be a second
//!   rule that could disagree with the first about what a coder may read, so
//!   there is none: a per-module view of the log is
//!   [`crate::scope::Authorization::filter`] applied to
//!   [`OplogEntry::candidate`]s, which is what `crate::retrieval` does.
//! - **No write.** There is no method here that appends, updates or deletes
//!   anything, and none can be added without leaving this module's role: the
//!   only writer of a record is `crate::barrier::submit_report`, and no code
//!   path updates or deletes an event (`CLAUDE.md`, "Load-bearing facts").
//!   Every function takes the database as `&mut ProductDb` only because
//!   `crate::barrier::read_records` does.
//! - **No raw evidence.** An entry holds a record, whose fields are the
//!   barrier's sanitized, length-capped text. It holds no
//!   [`crate::barrier::EvidenceBlob`] and calls no `read_evidence`: raw
//!   evidence reaches a caller only through
//!   [`crate::scope::Authorization::evidence`] on an explicit request.
//! - **No filtering by what a reader may see.** Every record of the product
//!   is here, because the log is the product's own. Whether a reader may see
//!   one is [`crate::scope::ScopeEnforcer`]'s decision, applied to the
//!   result, never to this module's output by this module.
//!
//! ```mermaid
//! flowchart LR
//!   L[memory.record_created events] --> B[barrier::read_records]
//!   L --> E[the events' own seq, actor, time, ticket]
//!   B --> P{paired one to one and agreeing?}
//!   E --> P
//!   P -->|no| X[OplogError::Inconsistent]
//!   P -->|yes| O[OperationalLog: entries in log order]
//!   O --> C[OplogEntry::candidate]
//!   C --> F[Authorization::filter, in crate::retrieval]
//! ```
//!
//! Must not: return unsanitized production content in a package
//! (`spec/LLD.md` section 2). Everything this module hands out is a
//! `crate::barrier::MemoryRecord`, which only the barrier builds.

use core::fmt;

use ori_core::error::MethodologyRef;
use ori_core::types::Id;
use ori_store::db::ProductDb;
use ori_store::event_log::Event;
use ori_store::event_log::EventLog;
use ori_store::event_log::EventLogError;

use crate::barrier;
use crate::barrier::BarrierError;
use crate::barrier::MemoryRecord;
use crate::barrier::RecordKind;
use crate::scope::Candidate;

/// The event kind `crate::barrier::submit_report` appends a record under.
const RECORD_CREATED: &str = "memory.record_created";

/// AICD §8, the memory architecture and its layer 3.
const OPERATIONAL_LAYER_SECTION: u8 = 8;

/// One operational record and where the log holds it: AICD §8 layer 3.
///
/// Built only by [`OperationalLog::read`]; there is no public constructor, so
/// an entry's `seq` is always one read from the log it names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OplogEntry {
    position: usize,
    seq: u64,
    record: MemoryRecord,
}

impl OplogEntry {
    /// The ordinal of this record among the product's operational records,
    /// from zero, in log order: AICD §8.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    /// The `seq` of the `memory.record_created` event this record was read
    /// from: AICD §8, "Never edited, only appended".
    #[must_use]
    pub const fn seq(&self) -> u64 {
        self.seq
    }

    /// The sanitized record: AICD §8.
    #[must_use]
    pub const fn record(&self) -> &MemoryRecord {
        &self.record
    }

    /// The ticket the record was filed on, when there was one: AICD §8.
    #[must_use]
    pub const fn ticket_id(&self) -> Option<&Id> {
        self.record.provenance().ticket_id()
    }

    /// This record as a result still to be filtered: AICD §25. The filter
    /// ([`crate::scope::Authorization::filter`]) is the only thing that
    /// decides whether a reader may have it.
    #[must_use]
    pub fn candidate(&self) -> Candidate {
        Candidate::Record(self.record.clone())
    }
}

/// Every operational record of one product, in log order, each with where it
/// sits in the log: AICD §8 layer 3, `spec/PRD.md` K-01.
///
/// A snapshot: [`OperationalLog::read`] reads the log once and this holds
/// what it read. It never writes. See the module doc, "What this module owes
/// beyond `crate::barrier`".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationalLog {
    entries: Vec<OplogEntry>,
    tip_seq: Option<u64>,
}

impl OperationalLog {
    /// Reads every operational record of `db`'s product: AICD §8.
    ///
    /// The records come from `crate::barrier::read_records`, which verifies
    /// the hash chain first; their `seq`s come from the log's own events and
    /// are paired with them one to one. Appends nothing.
    ///
    /// The two reads are not one transaction. A record appended between them
    /// makes the counts disagree and is refused as
    /// [`OplogError::Inconsistent`], never paired with the wrong event; the
    /// caller reads again.
    ///
    /// # Errors
    ///
    /// [`OplogError::Barrier`] when the barrier cannot read the records
    /// (an unverifiable log, or a record payload it cannot read back);
    /// [`OplogError::EventLog`] when the events cannot be read or verified;
    /// [`OplogError::Inconsistent`] when the two reads disagree.
    pub fn read(db: &mut ProductDb) -> Result<Self, OplogError> {
        let records = barrier::read_records(db)?;
        let report = EventLog::verify(db.connection())?;
        let events = match report.tip_seq {
            Some(tip) => EventLog::read_range(db.connection(), 1, tip)?,
            None => Vec::new(),
        };
        let recorded: Vec<&Event> = events
            .iter()
            .filter(|event| event.kind() == RECORD_CREATED)
            .collect();
        if recorded.len() != records.len() {
            return Err(OplogError::Inconsistent {
                detail: format!(
                    "the barrier read {} records and the log holds {} record events",
                    records.len(),
                    recorded.len()
                ),
            });
        }
        let mut entries = Vec::with_capacity(records.len());
        for (position, (record, event)) in records.into_iter().zip(recorded).enumerate() {
            let provenance = record.provenance();
            if event.actor() != provenance.actor()
                || event.at() != provenance.at()
                || event.ticket_id() != provenance.ticket_id()
            {
                return Err(OplogError::Inconsistent {
                    detail: format!(
                        "the record at position {position} does not agree with the event at seq {} on actor, time or ticket",
                        event.seq()
                    ),
                });
            }
            entries.push(OplogEntry {
                position,
                seq: event.seq(),
                record,
            });
        }
        Ok(Self {
            entries,
            tip_seq: report.tip_seq,
        })
    }

    /// Every entry, oldest first, in the order the records were appended:
    /// AICD §8.
    #[must_use]
    pub fn entries(&self) -> &[OplogEntry] {
        &self.entries
    }

    /// How many operational records the log held when it was read: AICD §8.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log held no operational record when it was read: AICD §8.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The last `seq` of the product's whole log when this was read, absent
    /// for an empty log: AICD §8.
    #[must_use]
    pub const fn tip_seq(&self) -> Option<u64> {
        self.tip_seq
    }

    /// The entries filed on `ticket_id`, in log order: AICD §8.
    pub fn for_ticket<'a>(
        &'a self,
        ticket_id: &'a Id,
    ) -> impl Iterator<Item = &'a OplogEntry> + 'a {
        self.entries
            .iter()
            .filter(move |entry| entry.ticket_id() == Some(ticket_id))
    }

    /// The entries of `kind`, in log order: AICD §8.
    pub fn of_kind(&self, kind: RecordKind) -> impl Iterator<Item = &OplogEntry> + '_ {
        self.entries
            .iter()
            .filter(move |entry| entry.record.kind() == kind)
    }

    /// Every entry whose record carries `record_id`, in log order: AICD §8.
    /// More than one when a submitter reused an id; never only the first.
    pub fn with_id<'a>(&'a self, record_id: &'a Id) -> impl Iterator<Item = &'a OplogEntry> + 'a {
        self.entries
            .iter()
            .filter(move |entry| entry.record.id() == record_id)
    }
}

/// Why the operational log could not be read: AICD §8.
///
/// `ori-core` has no `thiserror` yet (ruling R20); the hand-written `Display`
/// and `Error` follow `crate::barrier::BarrierError`.
#[derive(Debug)]
#[non_exhaustive]
pub enum OplogError {
    /// The barrier could not read the records back.
    Barrier(BarrierError),
    /// The event log could not be verified or read.
    EventLog(EventLogError),
    /// The barrier's records and the log's events did not pair up.
    Inconsistent {
        /// What disagreed.
        detail: String,
    },
}

impl OplogError {
    /// The methodology section a failure to read the operational log falls
    /// under: AICD §8.
    #[must_use]
    pub fn methodology_ref(&self) -> MethodologyRef {
        MethodologyRef {
            section: OPERATIONAL_LAYER_SECTION,
            subsection: None,
        }
    }
}

impl fmt::Display for OplogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Barrier(inner) => write!(f, "operational records unreadable: {inner}"),
            Self::EventLog(inner) => write!(f, "operational log unreadable: {inner}"),
            Self::Inconsistent { detail } => {
                write!(f, "operational log read inconsistently: {detail}")
            }
        }
    }
}

impl std::error::Error for OplogError {}

impl From<BarrierError> for OplogError {
    fn from(err: BarrierError) -> Self {
        Self::Barrier(err)
    }
}

impl From<EventLogError> for OplogError {
    fn from(err: EventLogError) -> Self {
        Self::EventLog(err)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    use ori_core::types::Actor;
    use ori_core::types::Timestamp;

    use super::*;
    use crate::barrier::BarrierConfig;
    use crate::barrier::FieldName;
    use crate::barrier::NewReport;
    use crate::barrier::NewReportField;
    use crate::barrier::submit_report;

    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "ori-t-0039-oplog-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("a fresh scratch directory can be created");
            Self { path }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn id(label: &str) -> Id {
        let mut safe = String::with_capacity(25);
        for ch in label.chars().take(25) {
            let mapped = match ch.to_ascii_uppercase() {
                upper @ ('0'..='9' | 'A'..='H' | 'J' | 'K' | 'M' | 'N' | 'P'..='T' | 'V'..='Z') => {
                    upper
                }
                'I' | 'L' => '1',
                'O' => '0',
                'U' => 'V',
                _ => '0',
            };
            safe.push(mapped);
        }
        Id::parse(&format!("0{safe:0>25}"))
            .expect("a sanitized, zero-padded, '0'-prefixed 26-character string always parses")
    }

    const fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    struct Product {
        db: ProductDb,
        id: Id,
        _scratch: Scratch,
    }

    fn product(label: &str) -> Product {
        let scratch = Scratch::new(label);
        let product_id = id("PRODUCTA");
        let db = ProductDb::open(&scratch.path, product_id.as_str(), at(1))
            .expect("a fresh product database opens");
        Product {
            db,
            id: product_id,
            _scratch: scratch,
        }
    }

    fn other_event(p: &mut Product, kind: &str) {
        EventLog::append(
            p.db.connection(),
            p.id.clone(),
            at(50),
            Actor::Human(id("OPERATOR")),
            kind,
            None,
            "{}".to_owned(),
        )
        .expect("a fixture event appends");
    }

    fn file(p: &mut Product, label: &str, kind: RecordKind, ticket: Option<&Id>) -> Id {
        let record_id = id(label);
        submit_report(
            &mut p.db,
            at(200),
            Actor::Agent(id("AUTHOR")),
            NewReport {
                record_id: record_id.clone(),
                kind,
                ticket_id: ticket.cloned(),
                fields: vec![NewReportField {
                    name: FieldName::parse("summary").expect("a field name"),
                    raw_text: format!("report {label}"),
                    evidence_id: id(&format!("EV{label}")),
                }],
            },
            &BarrierConfig::default(),
        )
        .expect("a fixture record is submitted");
        record_id
    }

    fn record_payload(record_id: &Id, ticket: Option<&Id>) -> String {
        serde_json::json!({
            "id": record_id.as_str(),
            "product_id": id("PRODUCTA").as_str(),
            "layer": "operational",
            "kind": "finding",
            "ticket_id": ticket.map(Id::as_str),
            "provenance": {"source": "agent", "identity": id("AUTHOR").as_str(), "at": 7},
            "untrusted": true,
            "structured": {"summary": "planted"},
        })
        .to_string()
    }

    #[test]
    fn ori_t_0039_the_operational_log_is_in_log_order_with_each_records_seq() {
        let mut p = product("order");
        let ticket_a = id("TICKETA");
        let ticket_b = id("TICKETB");
        other_event(&mut p, "ticket.filed");
        let first = file(&mut p, "FIRST", RecordKind::ClosingReport, Some(&ticket_a));
        other_event(&mut p, "ticket.queued");
        other_event(&mut p, "ticket.in_progress");
        let second = file(&mut p, "SECOND", RecordKind::Finding, Some(&ticket_b));
        let third = file(&mut p, "THIRD", RecordKind::Incident, None);

        let log = OperationalLog::read(&mut p.db).expect("the log reads");
        let ids: Vec<&Id> = log.entries().iter().map(|e| e.record().id()).collect();
        assert_eq!(ids, vec![&first, &second, &third], "oldest first");
        assert_eq!(log.len(), 3);
        assert!(!log.is_empty());
        for (index, entry) in log.entries().iter().enumerate() {
            assert_eq!(entry.position(), index);
        }
        // Each record's own event: one evidence event precedes it, so the
        // seqs are not consecutive, and each is the record event's.
        let events = EventLog::read_range(p.db.connection(), 1, 20).expect("the events read");
        for entry in log.entries() {
            let event = events
                .iter()
                .find(|event| event.seq() == entry.seq())
                .expect("the entry's seq names an event");
            assert_eq!(event.kind(), RECORD_CREATED);
            assert!(event.payload().contains(entry.record().id().as_str()));
        }
        let seqs: Vec<u64> = log.entries().iter().map(OplogEntry::seq).collect();
        assert!(seqs.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            log.tip_seq(),
            seqs.last().copied(),
            "the last event is the last record"
        );
    }

    #[test]
    fn ori_t_0039_the_operational_log_filters_by_ticket_kind_and_id_without_dropping_a_reused_id() {
        let mut p = product("lookups");
        let ticket_a = id("TICKETA");
        let ticket_b = id("TICKETB");
        let a1 = file(&mut p, "A1", RecordKind::ClosingReport, Some(&ticket_a));
        file(&mut p, "B1", RecordKind::Finding, Some(&ticket_b));
        file(&mut p, "A2", RecordKind::Finding, Some(&ticket_a));
        // A submitter reuses an id: nothing makes it unique.
        file(&mut p, "A1", RecordKind::BlockedReport, Some(&ticket_b));

        let log = OperationalLog::read(&mut p.db).expect("the log reads");
        let for_a: Vec<usize> = log
            .for_ticket(&ticket_a)
            .map(OplogEntry::position)
            .collect();
        assert_eq!(for_a, vec![0, 2]);
        let findings: Vec<usize> = log
            .of_kind(RecordKind::Finding)
            .map(OplogEntry::position)
            .collect();
        assert_eq!(findings, vec![1, 2]);
        let reused: Vec<usize> = log.with_id(&a1).map(OplogEntry::position).collect();
        assert_eq!(
            reused,
            vec![0, 3],
            "every entry carrying the id, never only the first"
        );
        assert_eq!(log.for_ticket(&id("NOSUCHTICKET")).count(), 0);
        assert_eq!(log.of_kind(RecordKind::PostMortem).count(), 0);
        let candidate = log.entries()[0].candidate();
        assert!(matches!(&candidate, Candidate::Record(record) if record.id() == &a1));
    }

    #[test]
    fn ori_t_0039_reading_the_operational_log_appends_nothing_and_changes_no_event() {
        let mut p = product("read-only");
        file(&mut p, "ONE", RecordKind::ClosingReport, None);
        file(&mut p, "TWO", RecordKind::Finding, None);
        let before = EventLog::verify(p.db.connection()).expect("the log verifies");
        let first = OperationalLog::read(&mut p.db).expect("the log reads");
        let second = OperationalLog::read(&mut p.db).expect("the log reads again");
        let after = EventLog::verify(p.db.connection()).expect("the log verifies");
        assert_eq!(before, after, "reading appended or changed an event");
        assert_eq!(first, second, "two reads of an unchanged log agree");
    }

    #[test]
    fn ori_t_0039_an_empty_operational_log_reads_as_empty() {
        let mut p = product("empty");
        let log = OperationalLog::read(&mut p.db).expect("an empty log reads");
        assert!(log.is_empty());
        assert_eq!(log.tip_seq(), None);
        other_event(&mut p, "ticket.filed");
        let log = OperationalLog::read(&mut p.db).expect("a log with no record reads");
        assert!(log.is_empty());
        assert_eq!(
            log.tip_seq(),
            Some(1),
            "the tip is the log's, records or not"
        );
    }

    #[test]
    fn ori_t_0039_a_record_event_the_barrier_cannot_read_fails_the_whole_read() {
        let mut p = product("malformed");
        file(&mut p, "GOOD", RecordKind::ClosingReport, None);
        EventLog::append(
            p.db.connection(),
            p.id.clone(),
            at(60),
            Actor::System,
            RECORD_CREATED,
            None,
            "{}".to_owned(),
        )
        .expect("the malformed event appends");
        let err =
            OperationalLog::read(&mut p.db).expect_err("one unreadable record fails the read");
        assert!(matches!(err, OplogError::Barrier(_)), "{err}");
        assert_eq!(err.methodology_ref().section, 8);
    }

    #[test]
    fn ori_t_0039_a_record_event_that_disagrees_with_its_payload_is_refused() {
        // The payload names agent AUTHOR at time 7 on no ticket; the event
        // is system's. The barrier reads the payload; the log's event says
        // otherwise; an entry would carry a seq of a record that is not it.
        let mut p = product("disagree-actor");
        EventLog::append(
            p.db.connection(),
            p.id.clone(),
            at(7),
            Actor::System,
            RECORD_CREATED,
            None,
            record_payload(&id("PLANTED"), None),
        )
        .expect("the planted event appends");
        let err = OperationalLog::read(&mut p.db).expect_err("the pairing disagrees");
        assert!(matches!(err, OplogError::Inconsistent { .. }), "{err}");

        let mut q = product("disagree-ticket");
        let ticket = id("TICKETA");
        EventLog::append(
            q.db.connection(),
            q.id.clone(),
            at(7),
            Actor::Agent(id("AUTHOR")),
            RECORD_CREATED,
            None,
            record_payload(&id("PLANTED"), Some(&ticket)),
        )
        .expect("the planted event appends");
        let err = OperationalLog::read(&mut q.db).expect_err("the ticket column disagrees");
        assert!(matches!(err, OplogError::Inconsistent { .. }), "{err}");

        // The same event with everything agreeing reads.
        let mut r = product("agree");
        EventLog::append(
            r.db.connection(),
            r.id.clone(),
            at(7),
            Actor::Agent(id("AUTHOR")),
            RECORD_CREATED,
            None,
            record_payload(&id("PLANTED"), None),
        )
        .expect("the event appends");
        let log = OperationalLog::read(&mut r.db).expect("an agreeing record reads");
        assert_eq!(log.len(), 1);
        assert_eq!(log.entries()[0].seq(), 1);
    }
}
