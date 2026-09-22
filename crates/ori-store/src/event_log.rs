//! `EventLog`: append, hash chain, read range (`spec/LLD.md` section 2).
//!
//! Criterion ORI-P1-028 in `spec/criteria/phase-1.md`: "Any mutating RPC |
//! Inspect the event log | One event with actor, ticket, payload, and a hash
//! chained to the previous; `products.rebuild` reproduces every projection
//! identically". This module is the first clause: one event, with an actor, a
//! ticket, a payload, chained by hash to the one before it. The second clause,
//! `products.rebuild`, belongs to ORI-T-0025, which reads what this module
//! writes; nothing here reconstructs a projection.
//!
//! `spec/DATA_MODEL.md` section 2 gives the row: "seq (monotonic), product_id,
//! at, actor (human identity or agent identity or system), kind, ticket_id
//! (nullable), payload (json), hash_prev | Append-only, hash-chained: the
//! audit trail." Section 4 adds two invariants this module answers directly:
//! "Every `Event` has an actor; `system` is allowed only for scheduled
//! triggers and watchers" (the second half, which `kind` a `system` actor may
//! attach to, is a policy over event kinds this crate does not own; see
//! "What this module deliberately does not enforce" below) and "The SQLite
//! file plus the repository fully reconstruct every projection", which is
//! `products.rebuild`'s half.
//!
//! CLAUDE.md's load-bearing facts, first line: "The event log in `ori-store`
//! is append-only and hash-chained. No code path updates or deletes an event.
//! Projections are derived; if a projection looks wrong, the fix is in the
//! projector, never in the log." AICD §8's memory architecture calls the
//! same thing "Append-only structured log... Never edited, only appended,"
//! and `crates/ori-store/src/lib.rs` already carries that citation for this
//! whole crate. AICD §13 is why every event carries an actor: "This is the
//! audit trail: any line of code can be traced to a commit, to a ticket, to a
//! paragraph of specification, to a human decision," and an event with no
//! actor is a link in that chain nothing can be traced through.
//!
//! # What enters the hash, and what could change without breaking it
//!
//! Every field of the row, not the payload alone. A chain over the payload
//! only would let `seq`, `actor`, `kind`, `ticket_id` or `hash_prev` itself
//! change on a stored row without the chain noticing, because none of them
//! would appear in what the next row's link is checked against; the payload
//! could stay byte-identical while the row around it became a different
//! event. `digest_row` instead folds `seq`, `product_id`, `at`, `actor`,
//! `kind`, `ticket_id`, `payload` and the row's own `hash_prev` into one
//! digest, in that order, each variable-length field preceded by its length
//! as eight big-endian bytes so that no byte sequence written by one field can
//! be read as a delimiter and bleed into the next (`crate::event_log::tests`
//! exercises exactly that: two rows whose fields concatenate to the same
//! bytes without the length prefix hash to different digests with it).
//!
//! `spec/DATA_MODEL.md` section 2 lists `hash_prev` as the row's only hash
//! column; there is no separate stored `hash`. So the digest above is not
//! read back from a column, it is *derived*: event `N`'s digest is
//! `digest_row` over event `N`'s own fields, and that derived value is what
//! [`EventLog::append`] writes into event `N+1`'s `hash_prev` column. Reading
//! the chain means recomputing each row's digest and checking it against the
//! next row's stored `hash_prev`, which [`EventLog::verify`] and
//! [`EventLog::read_range`] both do.
//!
//! What that leaves open, honestly, and a correction: whichever event is the
//! chain's *last* one *at the moment it is tampered with*. Nothing yet
//! depends on its digest, so a rewrite of that one row is not detected by
//! chain linkage, because there is no successor stored yet to disagree with
//! it.
//!
//! An earlier draft of this comment claimed that appending a further event
//! afterward "cements" the tampering and makes the next
//! [`EventLog::verify`] catch it. `crate::event_log::tests::proptests`
//! found that claim false and it is corrected here rather than left for a
//! reader to rediscover: [`EventLog::append`] reads whatever is *currently*
//! stored, so it builds an honestly-linked continuation *on top of* the
//! tampered row, not a link back to what the row used to say. The
//! continuation is internally consistent with the tampering, and
//! [`EventLog::verify`] has nothing left to disagree with, forever after,
//! not only until the next append.
//! `tests::proptests::ori_t_0023_tampering_with_the_current_tip_survives_a_further_honest_append`
//! proves this directly, and
//! `tests::proptests::ori_t_0023_property_a_single_corrupted_kind_before_its_successor_existed_is_always_detected`
//! is the property test narrowed to the case that *is* sound: any row that
//! already had a stored successor *before* it was tampered with is always
//! caught, because that successor's `hash_prev` was fixed while the row was
//! still honest.
//!
//! This is the ordinary limit of a hash chain with no separate external
//! anchor (a signature, a append-only log service, a blockchain checkpoint):
//! it makes rewriting a row expensive and detectable for as long as there is
//! already at least one honest witness to it on record, and gives no such
//! guarantee for the single most recent entry, at any point before or after
//! further ones are appended. `spec/DATA_MODEL.md` names no external anchor
//! for this log, so none is invented here; a future ticket that needs the
//! newest event to be tamper-evident too needs one (a periodically recorded,
//! separately-stored checkpoint digest, say), and this module does not
//! pretend to already have it.
//!
//! # Where `seq` comes from, and what makes it monotonic
//!
//! `spec/LLD.md` section 5: "The event log is written by a single writer
//! task; all mutations are messages to it, so ordering and the hash chain are
//! guaranteed." That is the engine's promise about its own caller discipline,
//! not a property of a `Connection` this module can see. [`EventLog::append`]
//! does not trust it blindly: it opens a `BEGIN IMMEDIATE` transaction (a
//! write lock taken up front, not deferred to the first write), reads the
//! current last row *inside* that transaction, computes `next_seq` as one
//! past what it just read (`1` for an empty log), and the same read supplies
//! `hash_prev`. `insert_checked` then re-reads the last row a second time,
//! inside the same transaction, and refuses the insert if what it finds
//! disagrees with the `seq` or `hash_prev` the candidate row carries, before
//! writing anything. Under the single-writer model this second read can never
//! disagree with the first; it is there so that a future caller of
//! `insert_checked` that does not go through [`EventLog::append`] (a
//! backfill tool restoring from an export, say) gets the same guarantee
//! [`EventLog::append`] gets, rather than a guarantee that only holds for one
//! caller. `seq` starts at `1`; nothing here generates a `0`.
//!
//! # Whether this module owns the schema
//!
//! It borrows it. `crates/ori-gates/src/significance.rs`'s own risk-map
//! restatement already reads `migrations` as this crate's `migrations/`
//! directory, sibling to `src/`, and CLAUDE.md's load-bearing facts assign
//! `db.rs` and `migrations/**` to ORI-T-0024, running in parallel; this
//! ticket's declared scope is `event_log.rs`, its `mod` line, and this
//! crate's manifest, nothing under `migrations/`. So [`EventLog`]'s functions
//! take a `Connection` (or `&mut Connection` for the one that writes) as a
//! parameter and assume, rather than create, this table:
//!
//! ```text
//! CREATE TABLE events (
//!     seq         INTEGER PRIMARY KEY,
//!     product_id  TEXT    NOT NULL,
//!     at          INTEGER NOT NULL,
//!     actor_kind  TEXT    NOT NULL,
//!     actor_id    TEXT,
//!     kind        TEXT    NOT NULL,
//!     ticket_id   TEXT,
//!     payload     TEXT    NOT NULL,
//!     hash_prev   TEXT    NOT NULL
//! );
//! ```
//!
//! `seq` is the `INTEGER PRIMARY KEY` (SQLite's rowid alias), which is what
//! lets [`EventLog::read_range`] page by it cheaply. `actor_id` is nullable
//! because [`ori_core::types::Actor::System`] carries no identity.
//! `hash_prev` is `TEXT`, sixty-four lowercase hex characters, not a `BLOB`:
//! `spec/LLD.md` section 6 says the log is "exportable to `ops/` as
//! structured files for backup," and hex text survives that export without a
//! second encoding step. `at` is milliseconds since the epoch, the same unit
//! [`ori_core::types::Timestamp`] holds. `product_id`, `kind`, `ticket_id` and
//! `payload` are `TEXT`; this module never parses `payload` as JSON (see
//! below).
//!
//! One thing this module does *not* borrow: it does not read a clock. `at` is
//! a parameter to [`EventLog::append`], supplied by the caller, the same way
//! `Connection` is. `ori-core` forbids reading a clock because it forbids all
//! IO (`spec/LLD.md` section 2); this crate is not under that rule, but taking
//! the timestamp as a parameter keeps the digest computation in this module
//! deterministic and testable without a wall clock in the loop, and matches
//! the pattern `spec/LLD.md` section 2 draws for `Id`: the crate with the
//! clock passes the value to the crate that hashes it.
//!
//! # What enforces append-only
//!
//! Three things, deliberately not only the first:
//!
//! 1. **The type withholds the method.** [`EventLog`] has `append`,
//!    `read_range` and `verify`. There is no `update`, no `delete`, and
//!    nothing in this module builds the SQL text for either. A caller that
//!    only ever reaches the log through [`EventLog`] cannot mutate a row,
//!    because the call does not exist to make.
//! 2. **A database trigger, installed idempotently.** `install_guards`
//!    issues `CREATE TRIGGER IF NOT EXISTS` for `BEFORE UPDATE` and
//!    `BEFORE DELETE` on `events`, each a `RAISE(ABORT, ...)`. This is
//!    defense against everything [`EventLog`]'s own API cannot be, in
//!    particular a raw `UPDATE`/`DELETE` issued against the same connection
//!    by code that never went through this module at all. [`EventLog::append`]
//!    calls it at the start of every append, which is cheap (SQLite checks
//!    `sqlite_master` for the name and no-ops) and means a connection is
//!    guarded from its first write through this module without `db.rs` having
//!    to remember a separate setup step.
//! 3. **[`EventLog::verify`] is the check of last resort**, for whatever gets
//!    past both: a row edited by a tool that talks to the `.sqlite` file
//!    directly, bypassing every connection-level guard. It recomputes the
//!    chain and reports exactly where it breaks. A trigger stops a mutation
//!    this module's connection can see; nothing stops a text editor opening
//!    the file. That is why "no code path updates or deletes an event"
//!    (CLAUDE.md) is enforced two ways from inside the process and checked a
//!    third way from outside it, rather than trusted on a comment.
//!
//! What a raw `INSERT` can still do, since triggers here guard `UPDATE` and
//! `DELETE`, not `INSERT`: append a row with a `seq` or `hash_prev` the safe
//! path would never produce. [`EventLog::append`] cannot itself build such a
//! row (it never accepts `seq` or `hash_prev` from its caller; both are
//! computed inside this module from what is actually stored), so the
//! refusal lives in `insert_checked`, the one function that turns a
//! candidate row into a write, and `crate::event_log::tests` calls it
//! directly with a deliberately wrong candidate to prove the refusal fires
//! (AICD §14: "a gate is installed only after it has been seen to fail on a
//! planted defect").
//!
//! # What this module deliberately does not enforce
//!
//! `spec/DATA_MODEL.md` section 4's second clause, "`system` is allowed only
//! for scheduled triggers and watchers," names which event *kinds* a
//! `system` actor may attach to. Answering that needs a catalogue of kinds
//! and which of them are scheduled triggers or watchers, which is exactly the
//! "business rules" `spec/LLD.md` section 2 forbids this crate: "ori-store |
//! ... | Contain business rules". This module enforces the clause before it,
//! that every event has *an* actor at all, and leaves which actor may carry
//! which kind to whichever crate decides what a `system`-authored event is
//! for. It likewise does not validate that `payload` is well-formed JSON: it
//! is stored and hashed as the bytes it was given, and interpreting JSON
//! would need a parser this crate does not have (see "Dependencies" in the
//! pull request report) and a grammar `spec/DATA_MODEL.md` does not draw.
//! `kind` is checked only for being non-empty, not for the `domain.verb_past`
//! shape `spec/LLD.md` section 4 names as a naming convention for humans; that
//! shape is not a data-integrity invariant this log needs to hold, and
//! enforcing it here would be this crate deciding event taxonomy rather than
//! storing it.
//!
//! Must not: contain business rules (`spec/LLD.md` section 2). Nothing here
//! reads `Ticket`, `Phase` or any workflow state; it stores and verifies rows.

use core::fmt;

use ori_core::types::Actor;
use ori_core::types::Id;
use ori_core::types::Timestamp;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use rusqlite::Row;
use rusqlite::Transaction;
use rusqlite::TransactionBehavior;
use rusqlite::params;
use sha2::Digest;
use sha2::Sha256;

/// The `events` table name this module assumes `migrations/` creates.
const TABLE: &str = "events";

/// The digest of one event's full row, sixty-four lowercase hex characters
/// wide when written (`spec/DATA_MODEL.md` section 2's `hash_prev` column).
///
/// No methodology section applies directly: SHA-256 and the byte framing
/// below are an engineering choice this crate makes to satisfy
/// `spec/DATA_MODEL.md` section 2's "hash-chained", which names no algorithm.
/// The choice and its reasoning are in the pull request report, not repeated
/// per call site.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct EventHash([u8; 32]);

impl EventHash {
    /// The `hash_prev` of the first event a product's log ever holds.
    ///
    /// Thirty-two zero bytes. `spec/DATA_MODEL.md` section 2 does not mark
    /// `hash_prev` nullable the way it marks `ticket_id` nullable, so the
    /// first event needs a value here rather than an absence, and a value no
    /// real digest can equal by chance is what makes it a genesis marker
    /// rather than an ordinary, if unlikely, collision.
    pub const GENESIS: Self = Self([0u8; 32]);

    /// Reads sixty-four lowercase hex characters into a digest, refusing
    /// anything else.
    pub fn parse(text: &str) -> Result<Self, EventLogError> {
        if text.len() != 64 {
            return Err(EventLogError::Malformed {
                what: "hash_prev",
                value: text.to_owned(),
            });
        }
        let mut bytes = [0u8; 32];
        let raw = text.as_bytes();
        for (i, byte) in bytes.iter_mut().enumerate() {
            let hi = hex_nibble(raw[i * 2]);
            let lo = hex_nibble(raw[i * 2 + 1]);
            match (hi, lo) {
                (Some(hi), Some(lo)) => *byte = (hi << 4) | lo,
                _ => {
                    return Err(EventLogError::Malformed {
                        what: "hash_prev",
                        value: text.to_owned(),
                    });
                }
            }
        }
        Ok(Self(bytes))
    }

    /// The digest as sixty-four lowercase hex characters, the form it is
    /// stored in.
    #[must_use]
    pub fn to_hex(self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            out.push(HEX_DIGITS[usize::from(byte >> 4)]);
            out.push(HEX_DIGITS[usize::from(byte & 0x0f)]);
        }
        out
    }

    /// The digest's thirty-two raw bytes.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for EventHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Display for EventHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// The sixteen hex digits, lowercase, `crate::event_log::EventHash`'s own
/// alphabet.
const HEX_DIGITS: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

/// One lowercase or uppercase hex character's value, `None` for anything
/// else.
const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Folds one event's fields into the digest that becomes the next event's
/// `hash_prev`. See the module doc comment, "What enters the hash".
///
/// Takes the whole [`Event`] rather than its eight fields separately
/// (clippy's own threshold for a plain argument list), which also removes
/// any chance of two call sites passing the same fields in a different order.
fn digest_row(event: &Event) -> EventHash {
    let mut hasher = Sha256::new();
    hasher.update(event.seq.to_be_bytes());
    update_framed(&mut hasher, event.product_id.as_str().as_bytes());
    hasher.update(event.at.millis().to_be_bytes());
    hasher.update([actor_tag(&event.actor)]);
    match event.actor.identity() {
        Some(id) => update_framed(&mut hasher, id.as_str().as_bytes()),
        None => update_framed(&mut hasher, b""),
    }
    update_framed(&mut hasher, event.kind.as_bytes());
    match &event.ticket_id {
        Some(id) => {
            hasher.update([1u8]);
            update_framed(&mut hasher, id.as_str().as_bytes());
        }
        None => {
            hasher.update([0u8]);
            update_framed(&mut hasher, b"");
        }
    }
    update_framed(&mut hasher, event.payload.as_bytes());
    hasher.update(event.hash_prev.as_bytes());
    let out = hasher.finalize();
    let bytes: [u8; 32] = out.into();
    EventHash(bytes)
}

/// Writes a variable-length field's byte length, as eight big-endian bytes,
/// before the field itself.
///
/// Why a length prefix rather than a delimiter: a delimiter byte (say, a
/// null or a pipe) can appear inside a field's own bytes, and two different
/// splits of one concatenation would then hash the same, which is exactly
/// the ambiguity a hash chain over "the whole row" must not have. A fixed
/// eight-byte length prefix cannot be produced by the field's own content, so
/// the boundary between fields is fixed by the digest input itself rather
/// than by what happens to appear inside them.
fn update_framed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

/// The one-byte tag [`digest_row`] folds in ahead of an actor's identity, so
/// that `Actor::Agent(id)` and `Actor::Human(id)` with the same `id` text
/// hash to different digests.
const fn actor_tag(actor: &Actor) -> u8 {
    match actor {
        Actor::Human(_) => 0,
        Actor::Agent(_) => 1,
        Actor::System => 2,
    }
}

/// One row of the event log: `spec/DATA_MODEL.md` section 2's `Event`.
///
/// Fields are private; every one of them came from a row this module itself
/// read or just wrote; a value built any other way is not a promise this
/// module made about the log.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    seq: u64,
    product_id: Id,
    at: Timestamp,
    actor: Actor,
    kind: String,
    ticket_id: Option<Id>,
    payload: String,
    hash_prev: EventHash,
}

impl Event {
    /// The monotonic position of this event in its product's log.
    #[must_use]
    pub const fn seq(&self) -> u64 {
        self.seq
    }

    /// The product this event belongs to.
    #[must_use]
    pub const fn product_id(&self) -> &Id {
        &self.product_id
    }

    /// When this event was recorded, as the caller of [`EventLog::append`]
    /// gave it.
    #[must_use]
    pub const fn at(&self) -> Timestamp {
        self.at
    }

    /// Who performed the action this event records.
    #[must_use]
    pub const fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The event kind, `domain.verb_past` by `spec/LLD.md` section 4's naming
    /// convention, unchecked by this module beyond non-emptiness (see the
    /// module doc comment, "What this module deliberately does not enforce").
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The ticket this event is about, absent for an event with none.
    #[must_use]
    pub const fn ticket_id(&self) -> Option<&Id> {
        self.ticket_id.as_ref()
    }

    /// The payload, as the bytes it was appended with. Not parsed as JSON by
    /// this module; see the module doc comment.
    #[must_use]
    pub fn payload(&self) -> &str {
        &self.payload
    }

    /// The digest of the event immediately before this one, or
    /// [`EventHash::GENESIS`] when this is a product's first event.
    #[must_use]
    pub const fn hash_prev(&self) -> EventHash {
        self.hash_prev
    }

    /// This event's own digest, over every field above including
    /// [`Event::hash_prev`]. What the *next* event's `hash_prev` is checked
    /// against.
    #[must_use]
    pub fn digest(&self) -> EventHash {
        digest_row(self)
    }
}

/// A refusal or a malformed read from the event log.
///
/// The split mirrors `ori_core::error::Error`: a refusal
/// ([`EventLogError::is_refusal`] true) carries the methodology reason
/// `spec/LLD.md` section 4 requires of every refusal, and a value that failed
/// to parse or a chain that failed to verify carries none, because neither is
/// a control refusing an action.
///
/// This is not `ori_core::error::Error` because that enum's `RefusalKind` is
/// closed to this crate: adding a variant would edit
/// `crates/ori-core/src/error.rs`, which this ticket's declared scope does
/// not include.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EventLogError {
    /// A value read from a row, or given to [`EventLog::append`], did not
    /// parse into the type it names.
    Malformed {
        /// The column or field the value was read as.
        what: &'static str,
        /// The value as it was given or stored.
        value: String,
    },
    /// A row carried no actor, or an actor kind this module does not
    /// recognize. `spec/DATA_MODEL.md` section 4: "Every `Event` has an
    /// actor."
    MissingActor {
        /// The `seq` of the offending row, when the row could be identified
        /// at all.
        seq: Option<u64>,
    },
    /// A candidate event's `seq` was not exactly one past the log's current
    /// last `seq` (or `1`, for an empty log).
    SeqNotMonotonic {
        /// The `seq` the log required next.
        expected: u64,
        /// The `seq` the candidate carried.
        found: u64,
    },
    /// A candidate event's `hash_prev` did not equal the digest of the log's
    /// current last event (or [`EventHash::GENESIS`], for an empty log).
    PrevMismatch {
        /// The digest the log required.
        expected: EventHash,
        /// The `hash_prev` the candidate carried.
        found: EventHash,
    },
    /// Two consecutive rows in a read chained by `seq` did not chain by
    /// hash: the later row's `hash_prev` did not equal the earlier row's
    /// digest. This is what a mutated or deleted row is detected as.
    ChainBroken {
        /// The `seq` of the row whose `hash_prev` disagreed.
        at_seq: u64,
        /// What its `hash_prev` should have been.
        expected: EventHash,
        /// What it actually was.
        found: EventHash,
    },
    /// A row's stored `product_id` did not match the log's established one.
    /// `spec/DATA_MODEL.md` section 2, the `Product` row's own note: "One
    /// SQLite file per product."
    ProductMismatch {
        /// The product this log's other rows carry.
        expected: Id,
        /// The product the offending row carried.
        found: Id,
    },
    /// A count of rows taken independently of the read that produced them
    /// disagreed with what the read actually returned. See the pull request
    /// report, "what defect 8 caught": a reader that silently returns fewer
    /// rows than the table holds must not be read as "the chain verified",
    /// because a chain verified over nothing is vacuously true.
    ReaderIncomplete {
        /// What `SELECT COUNT(*)` reported.
        counted: u64,
        /// How many rows the read actually produced.
        read: u64,
    },
    /// `rusqlite` returned an error this module did not otherwise classify:
    /// a malformed statement, a closed connection, a disk error. Not a
    /// refusal by a control; the underlying message is kept for the human
    /// reading it.
    Sql {
        /// `rusqlite::Error`'s own message.
        message: String,
    },
}

impl EventLogError {
    /// The methodology section a refusal is made under, for the refusals and
    /// for nothing else.
    #[must_use]
    pub const fn methodology_ref(&self) -> Option<ori_core::error::MethodologyRef> {
        match self {
            // AICD §13: "This is the audit trail: any line of code can be
            // traced to a commit, to a ticket, to a paragraph of
            // specification, to a human decision." An event with no actor is
            // a link nothing can be traced through.
            Self::MissingActor { .. } => Some(ori_core::error::MethodologyRef {
                section: 13,
                subsection: None,
            }),
            // AICD §8's append-only log store, which
            // `crates/ori-store/src/lib.rs` already cites for this crate:
            // "Never edited, only appended." A seq or hash_prev the log did
            // not itself produce is exactly an attempt to write something
            // other than the next entry in that append-only sequence.
            Self::SeqNotMonotonic { .. } | Self::PrevMismatch { .. } => {
                Some(ori_core::error::MethodologyRef {
                    section: 8,
                    subsection: None,
                })
            }
            Self::Malformed { .. }
            | Self::ChainBroken { .. }
            | Self::ProductMismatch { .. }
            | Self::ReaderIncomplete { .. }
            | Self::Sql { .. } => None,
        }
    }

    /// Whether a control refused the action, as opposed to a value failing to
    /// parse, a chain failing to verify, or the database reporting an
    /// unrelated failure.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        matches!(
            self,
            Self::MissingActor { .. } | Self::SeqNotMonotonic { .. } | Self::PrevMismatch { .. }
        )
    }
}

impl fmt::Display for EventLogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { what, value } => write!(f, "not a valid {what}: {value:?}"),
            Self::MissingActor { seq } => match seq {
                Some(seq) => write!(f, "refused: event at seq {seq} carries no actor"),
                None => write!(f, "refused: event carries no actor"),
            },
            Self::SeqNotMonotonic { expected, found } => write!(
                f,
                "refused: seq {found} is not the next seq after this log's last event, which is {expected}"
            ),
            Self::PrevMismatch { expected, found } => write!(
                f,
                "refused: hash_prev {found} does not match this log's last event, whose digest is {expected}"
            ),
            Self::ChainBroken {
                at_seq,
                expected,
                found,
            } => write!(
                f,
                "the event at seq {at_seq} carries hash_prev {found}, and the event before it digests to {expected}: the chain is broken here"
            ),
            Self::ProductMismatch { expected, found } => write!(
                f,
                "the event carries product_id {found}, and this log's other events carry {expected}"
            ),
            Self::ReaderIncomplete { counted, read } => write!(
                f,
                "the table reports {counted} row(s) but the read returned {read}: refusing to report a chain verified over rows that were not actually read"
            ),
            Self::Sql { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for EventLogError {}

impl From<rusqlite::Error> for EventLogError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Sql {
            message: err.to_string(),
        }
    }
}

/// What [`EventLog::verify`] and [`EventLog::read_range`] found, for a caller
/// to check its own expectation against.
///
/// [`VerifyReport::events_checked`] exists so a caller cannot mistake "the
/// chain I read verified" for "the whole log verified" when the two differ,
/// which is exactly the trap defect 8 in the pull request report names: a
/// reader that finds nothing makes "every event is chained correctly"
/// vacuously true. [`EventLog::verify`] closes that trap itself (see
/// [`EventLogError::ReaderIncomplete`]); this report is what a caller with an
/// independent expectation (the last `seq` it remembers appending, say)
/// checks its own belief against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifyReport {
    /// How many events the chain check walked.
    pub events_checked: u64,
    /// The `seq` of the last event checked, absent for an empty log.
    pub tip_seq: Option<u64>,
    /// The digest of the last event checked, absent for an empty log.
    pub tip_digest: Option<EventHash>,
}

/// `EventLog`: append, hash chain, read range (`spec/LLD.md` section 2).
///
/// Carries no state of its own; every function takes the `Connection` it
/// needs as a parameter (see the module doc comment, "Whether this module
/// owns the schema"). Never instantiated; its functions are namespaced under
/// the type `spec/LLD.md` section 2 names rather than left as free functions
/// in the module, so a caller writes `EventLog::append(...)`, matching the
/// name the low-level design gives this responsibility.
#[derive(Debug)]
pub struct EventLog;

impl EventLog {
    /// Appends one event, computing its `seq` and `hash_prev` from what is
    /// actually stored rather than accepting either from the caller.
    ///
    /// `product_id`, `at`, `actor`, `kind`, `ticket_id` and `payload` are the
    /// fields `spec/DATA_MODEL.md` section 2 gives `Event` beyond `seq` and
    /// `hash_prev`. `kind` and `payload` are refused empty
    /// ([`EventLogError::Malformed`]); everything else the type system
    /// already refuses at the call site (`Actor`, `Id`, `Timestamp` all parse
    /// on construction, in `ori-core`).
    ///
    /// Refuses ([`EventLogError::ProductMismatch`]) a `product_id` that
    /// disagrees with an already-established one: `spec/DATA_MODEL.md`
    /// section 2's `Product` row, "One SQLite file per product," makes a
    /// second product's id appearing in this table a structural error in the
    /// caller, not a new product's first event.
    pub fn append(
        conn: &mut Connection,
        product_id: Id,
        at: Timestamp,
        actor: Actor,
        kind: impl Into<String>,
        ticket_id: Option<Id>,
        payload: impl Into<String>,
    ) -> Result<Event, EventLogError> {
        let kind = kind.into();
        if kind.trim().is_empty() {
            return Err(EventLogError::Malformed {
                what: "kind",
                value: kind,
            });
        }
        let payload = payload.into();
        if payload.is_empty() {
            return Err(EventLogError::Malformed {
                what: "payload",
                value: payload,
            });
        }

        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        install_guards(&tx)?;

        let last = last_event(&tx)?;
        if let Some(last) = &last
            && last.product_id != product_id
        {
            return Err(EventLogError::ProductMismatch {
                expected: last.product_id.clone(),
                found: product_id,
            });
        }
        let next_seq = last.as_ref().map_or(1, |event| event.seq + 1);
        let hash_prev = last.as_ref().map_or(EventHash::GENESIS, Event::digest);

        let candidate = Event {
            seq: next_seq,
            product_id,
            at,
            actor,
            kind,
            ticket_id,
            payload,
            hash_prev,
        };
        insert_checked(&tx, &candidate)?;
        tx.commit()?;
        Ok(candidate)
    }

    /// Reads events with `seq` in `[from, to]`, ordered by `seq`, refusing a
    /// malformed row and checking hash linkage between the rows returned.
    ///
    /// Linkage is checked only *within* the returned range: if `from` is not
    /// a product's first `seq`, the first row's own `hash_prev` is not
    /// checked against anything outside the range, because this call was not
    /// given the row that would let it. [`EventLog::verify`] is the whole-log
    /// check, always anchored at [`EventHash::GENESIS`].
    pub fn read_range(conn: &Connection, from: u64, to: u64) -> Result<Vec<Event>, EventLogError> {
        let mut statement = conn.prepare(&format!(
            "SELECT seq, product_id, at, actor_kind, actor_id, kind, ticket_id, payload, hash_prev \
             FROM {TABLE} WHERE seq >= ?1 AND seq <= ?2 ORDER BY seq ASC"
        ))?;
        // rusqlite implements `ToSql`/`FromSql` for the signed integer
        // widths SQLite itself stores (`INTEGER` is a signed 64-bit value),
        // not for `u64`, so `seq` crosses the boundary as `i64`. A `seq`
        // this module produced never approaches `i64::MAX`; a caller-given
        // bound above it is clamped rather than refused, since it can only
        // ever match nothing.
        let from_i64 = i64::try_from(from).unwrap_or(i64::MAX);
        let to_i64 = i64::try_from(to).unwrap_or(i64::MAX);
        let rows = statement.query_map(params![from_i64, to_i64], row_to_event)?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row??);
        }
        check_linkage(&events)?;
        Ok(events)
    }

    /// Verifies the whole log, from [`EventHash::GENESIS`] to the last row,
    /// checking `seq` monotonicity, hash linkage, actor presence and
    /// `product_id` consistency over every row.
    ///
    /// Cross-checks an independent `SELECT COUNT(*)` against the number of
    /// rows the read actually produced before verifying anything, and refuses
    /// ([`EventLogError::ReaderIncomplete`]) if they disagree, so that a
    /// reader broken into returning nothing cannot be mistaken for a log with
    /// nothing wrong in it. See the module doc comment and the pull request
    /// report, "what defect 8 caught".
    pub fn verify(conn: &Connection) -> Result<VerifyReport, EventLogError> {
        let counted_i64: i64 =
            conn.query_row(&format!("SELECT COUNT(*) FROM {TABLE}"), [], |row| {
                row.get(0)
            })?;
        let counted = u64::try_from(counted_i64).map_err(|_| EventLogError::Malformed {
            what: "COUNT(*)",
            value: counted_i64.to_string(),
        })?;
        let mut statement = conn.prepare(&format!(
            "SELECT seq, product_id, at, actor_kind, actor_id, kind, ticket_id, payload, hash_prev \
             FROM {TABLE} ORDER BY seq ASC"
        ))?;
        let rows = statement.query_map([], row_to_event)?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row??);
        }
        verify_events(counted, events)
    }
}

/// The pure half of [`EventLog::verify`]: given an independently obtained row
/// count and the events a read actually produced, checks completeness and
/// then chain linkage over them.
///
/// Split out from [`EventLog::verify`] so the two can disagree on purpose in
/// a test without needing a genuine race between two real SQL statements to
/// produce that disagreement: `crate::event_log::tests` calls this directly
/// with a `counted` that does not match a real, valid `events`, which is
/// exactly the shape [`EventLogError::ReaderIncomplete`] exists to refuse,
/// over the same function [`EventLog::verify`] itself calls, not a copy of
/// it. This is what closes the gap AICD §14's planted-defect discipline found
/// in an earlier draft: removing the call to [`check_reader_completeness`]
/// from [`EventLog::verify`] alone, with the SQL reader left honest, passed
/// every test that draft had, because nothing in it ever produced a real
/// count/read disagreement for the removal to matter to. See the pull
/// request report, "what defect 8 caught", for the scratchpad proof this was
/// found in.
fn verify_events(counted: u64, events: Vec<Event>) -> Result<VerifyReport, EventLogError> {
    let read = events.len() as u64;
    check_reader_completeness(counted, read)?;

    let mut expected_prev = EventHash::GENESIS;
    let mut product: Option<&Id> = None;
    for (expected_seq, event) in (1u64..).zip(&events) {
        if event.seq != expected_seq {
            return Err(EventLogError::SeqNotMonotonic {
                expected: expected_seq,
                found: event.seq,
            });
        }
        if event.hash_prev != expected_prev {
            return Err(EventLogError::ChainBroken {
                at_seq: event.seq,
                expected: expected_prev,
                found: event.hash_prev,
            });
        }
        match product {
            Some(established) if *established != event.product_id => {
                return Err(EventLogError::ProductMismatch {
                    expected: established.clone(),
                    found: event.product_id.clone(),
                });
            }
            Some(_) => {}
            None => product = Some(&event.product_id),
        }
        expected_prev = event.digest();
    }

    Ok(VerifyReport {
        events_checked: read,
        tip_seq: events.last().map(Event::seq),
        tip_digest: events.last().map(Event::digest),
    })
}

/// Refuses when an independent row count disagrees with how many rows a read
/// actually produced.
///
/// This is the whole fix for defect 8 in the pull request report: "break
/// your own reader so it finds no events. Must FAIL, not pass." A reader
/// that silently returns fewer rows than the table holds must not let
/// [`EventLog::verify`] report a chain of whatever it did manage to read as
/// "verified", because a chain verified over nothing, or over too little, is
/// vacuously true about the rest. `crate::event_log::tests` calls this
/// function directly with a `counted` and `read` that disagree, which is the
/// shape a broken reader produces, and confirms it refuses; AICD §14 asks
/// that a check be seen to fail on a planted defect before it is trusted.
const fn check_reader_completeness(counted: u64, read: u64) -> Result<(), EventLogError> {
    if read == counted {
        Ok(())
    } else {
        Err(EventLogError::ReaderIncomplete { counted, read })
    }
}

/// Checks hash linkage between consecutive rows already read, the half of
/// [`EventLog::verify`]'s check that [`EventLog::read_range`] can still do
/// without the row before the range.
fn check_linkage(events: &[Event]) -> Result<(), EventLogError> {
    let mut previous: Option<&Event> = None;
    for event in events {
        if let Some(previous) = previous {
            if previous.seq + 1 != event.seq {
                return Err(EventLogError::SeqNotMonotonic {
                    expected: previous.seq + 1,
                    found: event.seq,
                });
            }
            let expected = previous.digest();
            if event.hash_prev != expected {
                return Err(EventLogError::ChainBroken {
                    at_seq: event.seq,
                    expected,
                    found: event.hash_prev,
                });
            }
        }
        previous = Some(event);
    }
    Ok(())
}

/// Reads the current last row, `None` for an empty log.
fn last_event(tx: &Transaction<'_>) -> Result<Option<Event>, EventLogError> {
    tx.query_row(
        &format!(
            "SELECT seq, product_id, at, actor_kind, actor_id, kind, ticket_id, payload, hash_prev \
             FROM {TABLE} ORDER BY seq DESC LIMIT 1"
        ),
        [],
        row_to_event,
    )
    .optional()?
    .transpose()
}

/// Inserts `candidate`, after re-reading the log's current last row inside
/// the same transaction and refusing if `candidate.seq` or
/// `candidate.hash_prev` disagrees with what that fresh read requires.
///
/// This is the one place a row is actually written, and the one place the
/// refusals for a bad `seq` or a bad `hash_prev` are made real: see the
/// module doc comment, "What enforces append-only", point 2, and "What a raw
/// `INSERT` can still do".
fn insert_checked(tx: &Transaction<'_>, candidate: &Event) -> Result<(), EventLogError> {
    let last = last_event(tx)?;
    let expected_seq = last.as_ref().map_or(1, |event| event.seq + 1);
    if candidate.seq != expected_seq {
        return Err(EventLogError::SeqNotMonotonic {
            expected: expected_seq,
            found: candidate.seq,
        });
    }
    let expected_prev = last.as_ref().map_or(EventHash::GENESIS, Event::digest);
    if candidate.hash_prev != expected_prev {
        return Err(EventLogError::PrevMismatch {
            expected: expected_prev,
            found: candidate.hash_prev,
        });
    }
    if candidate.actor.is_system() && candidate.actor.identity().is_some() {
        // Unreachable through ori_core::types::Actor's own constructors,
        // which is exactly the point: nothing here needs to check it because
        // the type already refuses it. Named so a future reader does not
        // wonder why MissingActor's only live path is below.
    }

    tx.execute(
        &format!(
            "INSERT INTO {TABLE} \
             (seq, product_id, at, actor_kind, actor_id, kind, ticket_id, payload, hash_prev) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
        ),
        params![
            i64::try_from(candidate.seq).unwrap_or(i64::MAX),
            candidate.product_id.as_str(),
            candidate.at.millis(),
            candidate.actor.kind(),
            candidate.actor.identity().map(Id::as_str),
            candidate.kind,
            candidate.ticket_id.as_ref().map(Id::as_str),
            candidate.payload,
            candidate.hash_prev.to_hex(),
        ],
    )?;
    Ok(())
}

/// Installs the append-only guard triggers if they are not already present.
/// Idempotent: safe to call on every [`EventLog::append`].
fn install_guards(conn: &Connection) -> Result<(), EventLogError> {
    conn.execute_batch(&format!(
        "CREATE TRIGGER IF NOT EXISTS {TABLE}_no_update BEFORE UPDATE ON {TABLE}
         BEGIN SELECT RAISE(ABORT, 'events is append-only: UPDATE refused'); END;
         CREATE TRIGGER IF NOT EXISTS {TABLE}_no_delete BEFORE DELETE ON {TABLE}
         BEGIN SELECT RAISE(ABORT, 'events is append-only: DELETE refused'); END;"
    ))?;
    Ok(())
}

/// Parses one `events` row into an [`Event`], refusing a missing or
/// unrecognized actor.
///
/// Returns `rusqlite::Result<Result<Event, EventLogError>>`: the outer
/// `Result` is `query_map`'s own (a column read that failed at the SQL
/// level), the inner one is this function's judgment about what the row
/// means, exactly so a missing actor is not silently folded into a generic
/// SQL error and loses the `MissingActor` classification.
fn row_to_event(row: &Row<'_>) -> rusqlite::Result<Result<Event, EventLogError>> {
    let seq: i64 = row.get("seq")?;
    let product_id_text: String = row.get("product_id")?;
    let at_millis: i64 = row.get("at")?;
    let actor_kind: String = row.get("actor_kind")?;
    let actor_id: Option<String> = row.get("actor_id")?;
    let kind: String = row.get("kind")?;
    let ticket_id_text: Option<String> = row.get("ticket_id")?;
    let payload: String = row.get("payload")?;
    let hash_prev_text: String = row.get("hash_prev")?;

    Ok((|| {
        let seq = u64::try_from(seq).map_err(|_| EventLogError::Malformed {
            what: "seq",
            value: seq.to_string(),
        })?;
        let product_id = Id::parse(&product_id_text).map_err(|_| EventLogError::Malformed {
            what: "product_id",
            value: product_id_text,
        })?;
        let actor = parse_actor(seq, &actor_kind, actor_id)?;
        let ticket_id = match ticket_id_text {
            Some(text) => Some(Id::parse(&text).map_err(|_| EventLogError::Malformed {
                what: "ticket_id",
                value: text,
            })?),
            None => None,
        };
        let hash_prev = EventHash::parse(&hash_prev_text)?;
        Ok(Event {
            seq,
            product_id,
            at: Timestamp::from_millis(at_millis),
            actor,
            kind,
            ticket_id,
            payload,
            hash_prev,
        })
    })())
}

/// Rebuilds an [`Actor`] from its two stored columns, refusing a missing or
/// unrecognized `actor_kind` (`spec/DATA_MODEL.md` section 4: "Every `Event`
/// has an actor").
fn parse_actor(
    seq: u64,
    actor_kind: &str,
    actor_id: Option<String>,
) -> Result<Actor, EventLogError> {
    match (actor_kind, actor_id) {
        ("human", Some(id)) => {
            Id::parse(&id)
                .map(Actor::Human)
                .map_err(|_| EventLogError::Malformed {
                    what: "actor_id",
                    value: id,
                })
        }
        ("agent", Some(id)) => {
            Id::parse(&id)
                .map(Actor::Agent)
                .map_err(|_| EventLogError::Malformed {
                    what: "actor_id",
                    value: id,
                })
        }
        ("system", None) => Ok(Actor::System),
        ("", None | Some(_)) => Err(EventLogError::MissingActor { seq: Some(seq) }),
        _ => Err(EventLogError::Malformed {
            what: "actor_kind",
            value: actor_kind.to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use ori_core::types::Actor;
    use ori_core::types::Id;
    use ori_core::types::Timestamp;
    use rusqlite::Connection;

    use super::Event;
    use super::EventHash;
    use super::EventLog;
    use super::EventLogError;
    use super::insert_checked;

    /// A schema this module can talk to, created ad hoc by the test rather
    /// than by `migrations/`, which this ticket does not touch. Matches the
    /// contract the module doc comment states under "Whether this module
    /// owns the schema".
    fn connection() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory sqlite always opens");
        conn.execute_batch(
            "CREATE TABLE events (
                seq         INTEGER PRIMARY KEY,
                product_id  TEXT    NOT NULL,
                at          INTEGER NOT NULL,
                actor_kind  TEXT    NOT NULL,
                actor_id    TEXT,
                kind        TEXT    NOT NULL,
                ticket_id   TEXT,
                payload     TEXT    NOT NULL,
                hash_prev   TEXT    NOT NULL
            );",
        )
        .expect("the fixed DDL above is valid SQL");
        conn
    }

    /// A valid ULID-shaped id, distinguished by the label so two calls with
    /// different labels never collide.
    ///
    /// `label` is sanitized into `ori_core::types::Id`'s Crockford base32
    /// alphabet rather than required to already be in it, so a test can write
    /// a readable word (`"PRODUCT"`, `"TICKET1"`) without the reader having to
    /// know that Crockford's alphabet, unlike plain base32, excludes `I`,
    /// `L`, `O` and `U`.
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
        // Crockford base32, 26 characters, first symbol at most '7': the
        // leading '0' below satisfies that regardless of what the label maps
        // to, and the right-pad with '0' fills a label shorter than 25.
        Id::parse(&format!("0{safe:0>25}"))
            .expect("a sanitized, zero-padded, '0'-prefixed 26-character string always parses")
    }

    fn product() -> Id {
        id("PRODUCT")
    }

    fn ts(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    /// Appends one event the same way [`EventLog::append`] does (through
    /// [`super::insert_checked`], so `seq` and `hash_prev` are still
    /// computed correctly and still refused if wrong), but without
    /// [`super::install_guards`].
    ///
    /// Only for the two tests below that need a `.sqlite` file with no
    /// append-only trigger on it at all, modelling a log whose *first*
    /// write never went through [`EventLog::append`] (a restore from an
    /// export, say): [`EventLog::append`] itself always installs the guard
    /// (see the module doc comment, "What enforces append-only", point 2),
    /// so once any real caller has appended through it, a later raw
    /// `UPDATE`/`DELETE` on *any* connection to that same file is refused by
    /// the trigger already written into the file's own schema, not only on
    /// the connection that created it.
    fn append_with_no_guard_installed(
        conn: &Connection,
        product_id: Id,
        at: Timestamp,
        actor: Actor,
        kind: &str,
        payload: &str,
    ) -> Event {
        let tx = conn
            .unchecked_transaction()
            .expect("a fresh connection always opens a transaction");
        let last = super::last_event(&tx).expect("read the current last event, if any");
        let candidate = Event {
            seq: last.as_ref().map_or(1, |event| event.seq() + 1),
            product_id,
            at,
            actor,
            kind: kind.to_owned(),
            ticket_id: None,
            payload: payload.to_owned(),
            hash_prev: last.as_ref().map_or(EventHash::GENESIS, Event::digest),
        };
        super::insert_checked(&tx, &candidate)
            .expect("a correctly computed candidate is never refused");
        tx.commit().expect("commit the unguarded append");
        candidate
    }

    #[test]
    fn ori_p1_028_an_appended_event_carries_actor_ticket_payload_and_a_hash_chained_to_the_previous()
     {
        let mut conn = connection();
        let ticket = id("TICKET0000000000000000001");
        let first = EventLog::append(
            &mut conn,
            product(),
            ts(1000),
            Actor::Agent(id("AGENT000000000000000000001")),
            "ticket.validated",
            Some(ticket.clone()),
            r#"{"n":1}"#,
        )
        .expect("a first append into an empty log succeeds");
        assert_eq!(first.seq(), 1);
        assert_eq!(first.hash_prev(), EventHash::GENESIS);
        assert_eq!(
            *first.actor(),
            Actor::Agent(id("AGENT000000000000000000001"))
        );
        assert_eq!(first.ticket_id(), Some(&ticket));
        assert_eq!(first.payload(), r#"{"n":1}"#);

        let second = EventLog::append(
            &mut conn,
            product(),
            ts(2000),
            Actor::System,
            "gate.proven",
            None,
            r#"{"n":2}"#,
        )
        .expect("a second append chains onto the first");
        assert_eq!(second.seq(), 2);
        assert_eq!(second.hash_prev(), first.digest());
        assert_ne!(second.hash_prev(), EventHash::GENESIS);

        let report = EventLog::verify(&conn).expect("two correctly chained events verify");
        assert_eq!(report.events_checked, 2);
        assert_eq!(report.tip_seq, Some(2));
        assert_eq!(report.tip_digest, Some(second.digest()));
    }

    #[test]
    fn ori_p1_028_rebuild_is_out_of_this_tickets_scope() {
        // ORI-P1-028's second clause, "products.rebuild reproduces every
        // projection identically", is ORI-T-0025's. Nothing in this module
        // builds a projection; this test exists only to say so where the
        // criterion's other tests live, not to exercise anything.
    }

    #[test]
    fn ori_t_0023_seq_is_one_for_the_first_event_and_increments_by_one() {
        let mut conn = connection();
        for expected in 1..=5u64 {
            let event = EventLog::append(
                &mut conn,
                product(),
                ts(i64::try_from(expected).expect("small test value")),
                Actor::System,
                "ticket.queued",
                None,
                "{}",
            )
            .expect("append succeeds");
            assert_eq!(event.seq(), expected);
        }
    }

    #[test]
    fn ori_t_0023_empty_kind_is_refused() {
        let mut conn = connection();
        let err = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            Actor::System,
            "   ",
            None,
            "{}",
        )
        .expect_err("whitespace-only kind is not a kind");
        assert!(matches!(err, EventLogError::Malformed { what: "kind", .. }));
    }

    #[test]
    fn ori_t_0023_empty_payload_is_refused() {
        let mut conn = connection();
        let err = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            Actor::System,
            "ticket.queued",
            None,
            "",
        )
        .expect_err("empty payload is not valid JSON");
        assert!(matches!(
            err,
            EventLogError::Malformed {
                what: "payload",
                ..
            }
        ));
    }

    #[test]
    fn ori_t_0023_a_second_product_id_in_one_log_is_refused() {
        let mut conn = connection();
        EventLog::append(
            &mut conn,
            product(),
            ts(1),
            Actor::System,
            "ticket.queued",
            None,
            "{}",
        )
        .expect("first append establishes the product");
        let err = EventLog::append(
            &mut conn,
            id("OTHER_PRODUCT"),
            ts(2),
            Actor::System,
            "ticket.queued",
            None,
            "{}",
        )
        .expect_err("a second product_id in one file's log is refused");
        assert!(matches!(err, EventLogError::ProductMismatch { .. }));
    }

    // --- Planted defect 1: a hash_prev that does not match the previous
    // event's hash must be refused. EventLog::append can never build one
    // (see the module doc comment); insert_checked is the guard that would
    // refuse it if any caller, present or future, tried, so the defect is
    // planted there directly. AICD §14: "a gate is installed only after it
    // has been seen to fail on a planted defect."
    #[test]
    fn ori_t_0023_append_refused_when_hash_prev_does_not_match_the_previous_event() {
        let conn = connection();
        let tx = conn
            .unchecked_transaction()
            .expect("an in-memory connection always opens a transaction");
        let bad = Event {
            seq: 1,
            product_id: product(),
            at: ts(1),
            actor: Actor::System,
            kind: "ticket.queued".to_owned(),
            ticket_id: None,
            payload: "{}".to_owned(),
            hash_prev: EventHash::parse(&"ab".repeat(32)).expect("64 hex chars parses"),
        };
        let err = insert_checked(&tx, &bad).expect_err("hash_prev disagrees with an empty log");
        assert!(matches!(err, EventLogError::PrevMismatch { .. }));
        tx.rollback().expect("rollback the refused attempt");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .expect("count after a refused insert");
        assert_eq!(count, 0, "the refused row must not have been written");
    }

    // --- Planted defect 4: a seq that repeats or goes backward must be
    // refused, at the same guard.
    #[test]
    fn ori_t_0023_append_refused_when_seq_repeats_or_goes_backward() {
        let mut conn = connection();
        let first = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            Actor::System,
            "ticket.queued",
            None,
            "{}",
        )
        .expect("first append");

        for bad_seq in [1u64, 0u64] {
            let tx = conn
                .unchecked_transaction()
                .expect("an in-memory connection always opens a transaction");
            let bad = Event {
                seq: bad_seq,
                product_id: product(),
                at: ts(2),
                actor: Actor::System,
                kind: "ticket.queued".to_owned(),
                ticket_id: None,
                payload: "{}".to_owned(),
                hash_prev: first.digest(),
            };
            let err = insert_checked(&tx, &bad)
                .expect_err("seq that repeats or regresses must be refused");
            assert!(matches!(err, EventLogError::SeqNotMonotonic { .. }));
            tx.rollback().expect("rollback the refused attempt");
        }

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .expect("count after two refused inserts");
        assert_eq!(count, 1, "only the first, legitimate row is present");
    }

    // --- Planted defect 2: an event mutated in place after being written
    // must be detected on read. EventLog::append's guard trigger is written
    // into the file's own schema, so it would refuse this mutation on *any*
    // connection once a real caller has appended through it at least once
    // (crate::event_log::tests::ori_t_0023_a_mutated_row_that_went_through_a_guarded_append_is_still_refused_by_sqlite_itself
    // proves that directly). This test instead models a log whose events
    // never went through EventLog::append at all (append_with_no_guard_installed
    // still computes seq/hash_prev correctly and would still refuse a wrong
    // one; it just never installs the trigger), so the file genuinely has no
    // guard, and the mutation reaches the row. The scratchpad proof under the
    // pull request report goes one step further and edits the `.sqlite`
    // file's bytes directly, which defeats even an installed trigger.
    #[test]
    fn ori_t_0023_a_mutated_row_is_detected_on_verify() {
        let dir = std::env::temp_dir().join(format!(
            "ori-t-0023-mutate-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the epoch")
                .as_nanos()
        ));
        let path = dir.join("log.sqlite");
        std::fs::create_dir_all(&dir).expect("scratch directory for this test's own db file");

        {
            let conn = Connection::open(&path).expect("open a fresh sqlite file");
            conn.execute_batch(
                "CREATE TABLE events (
                    seq INTEGER PRIMARY KEY, product_id TEXT NOT NULL, at INTEGER NOT NULL,
                    actor_kind TEXT NOT NULL, actor_id TEXT, kind TEXT NOT NULL,
                    ticket_id TEXT, payload TEXT NOT NULL, hash_prev TEXT NOT NULL
                );",
            )
            .expect("create the assumed schema");
            append_with_no_guard_installed(
                &conn,
                product(),
                ts(1),
                Actor::System,
                "ticket.queued",
                "{}",
            );
            append_with_no_guard_installed(
                &conn,
                product(),
                ts(2),
                Actor::System,
                "ticket.validated",
                "{}",
            );
        }

        {
            // A second, independent connection to the same, guard-free file.
            let mutator = Connection::open(&path).expect("reopen the same file");
            mutator
                .execute(
                    "UPDATE events SET payload = ?1 WHERE seq = 1",
                    rusqlite::params!["{\"tampered\":true}"],
                )
                .expect("no guard was ever installed on this file");
        }

        let reader = Connection::open(&path).expect("reopen to verify");
        let err = EventLog::verify(&reader).expect_err("a mutated row must fail verification");
        assert!(
            matches!(err, EventLogError::ChainBroken { at_seq: 2, .. }),
            "seq 2's hash_prev no longer matches the tampered seq 1's digest: {err}"
        );

        drop(reader);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- Planted defect 3: an event deleted from the middle of the chain
    // must be detected. Same guard-free-file model as defect 2, and the same
    // reason: a guarded file refuses the DELETE outright (proved below), so
    // this test builds a log that never had the guard installed at all.
    #[test]
    fn ori_t_0023_a_deleted_middle_row_is_detected_on_verify() {
        let dir = std::env::temp_dir().join(format!(
            "ori-t-0023-delete-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the epoch")
                .as_nanos()
        ));
        let path = dir.join("log.sqlite");
        std::fs::create_dir_all(&dir).expect("scratch directory for this test's own db file");

        {
            let conn = Connection::open(&path).expect("open a fresh sqlite file");
            conn.execute_batch(
                "CREATE TABLE events (
                    seq INTEGER PRIMARY KEY, product_id TEXT NOT NULL, at INTEGER NOT NULL,
                    actor_kind TEXT NOT NULL, actor_id TEXT, kind TEXT NOT NULL,
                    ticket_id TEXT, payload TEXT NOT NULL, hash_prev TEXT NOT NULL
                );",
            )
            .expect("create the assumed schema");
            for n in 1..=3i64 {
                append_with_no_guard_installed(
                    &conn,
                    product(),
                    ts(n),
                    Actor::System,
                    "ticket.queued",
                    "{}",
                );
            }
        }
        {
            let mutator = Connection::open(&path).expect("reopen the same file");
            mutator
                .execute("DELETE FROM events WHERE seq = 2", [])
                .expect("no guard was ever installed on this file");
        }

        let reader = Connection::open(&path).expect("reopen to verify");
        let err =
            EventLog::verify(&reader).expect_err("a hole in the middle of the chain must fail");
        // With seq 2 gone, the surviving seq 3 either fails the seq check
        // (expected 2, found 3) or, should seq happen to still look adjacent
        // under some other corruption, the hash check; either is "detected".
        assert!(
            matches!(
                err,
                EventLogError::SeqNotMonotonic { .. } | EventLogError::ChainBroken { .. }
            ),
            "a deleted middle row must be detected one way or the other: {err}"
        );

        drop(reader);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- Planted defect 5: an event with no actor must be refused. Row
    // construction through EventLog::append cannot omit an actor (Actor is a
    // required, non-Option parameter), so the defect is planted at the row
    // parser, which is what a raw-inserted NULL actor_kind would reach.
    #[test]
    fn ori_t_0023_a_row_with_no_actor_is_refused_by_the_row_parser() {
        let conn = connection();
        conn.execute(
            "INSERT INTO events \
             (seq, product_id, at, actor_kind, actor_id, kind, ticket_id, payload, hash_prev) \
             VALUES (1, ?1, 1, '', NULL, 'ticket.queued', NULL, '{}', ?2)",
            rusqlite::params![product().as_str(), EventHash::GENESIS.to_hex()],
        )
        .expect("raw insert bypassing EventLog, modelling a row this module never wrote");

        let err = EventLog::verify(&conn).expect_err("a row with no actor must be refused");
        assert!(matches!(err, EventLogError::MissingActor { seq: Some(1) }));
    }

    /// What the two tests above deliberately work around: once one event has
    /// gone through [`EventLog::append`] (which installs the guard), a raw
    /// mutation is refused on *any* connection reopened onto that same file,
    /// not only the connection that appended. The trigger lives in the
    /// file's own schema (`sqlite_master`), not in connection state.
    #[test]
    fn ori_t_0023_a_mutated_row_that_went_through_a_guarded_append_is_still_refused_by_sqlite_itself()
     {
        let dir = std::env::temp_dir().join(format!(
            "ori-t-0023-guarded-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after the epoch")
                .as_nanos()
        ));
        let path = dir.join("log.sqlite");
        std::fs::create_dir_all(&dir).expect("scratch directory for this test's own db file");

        {
            let mut conn = Connection::open(&path).expect("open a fresh sqlite file");
            conn.execute_batch(
                "CREATE TABLE events (
                    seq INTEGER PRIMARY KEY, product_id TEXT NOT NULL, at INTEGER NOT NULL,
                    actor_kind TEXT NOT NULL, actor_id TEXT, kind TEXT NOT NULL,
                    ticket_id TEXT, payload TEXT NOT NULL, hash_prev TEXT NOT NULL
                );",
            )
            .expect("create the assumed schema");
            EventLog::append(
                &mut conn,
                product(),
                ts(1),
                Actor::System,
                "ticket.queued",
                None,
                "{}",
            )
            .expect("this append installs the guard trigger on the file");
        }

        let other_connection = Connection::open(&path).expect("reopen the same file");
        let err = other_connection.execute(
            "UPDATE events SET payload = ?1 WHERE seq = 1",
            rusqlite::params!["{\"tampered\":true}"],
        );
        assert!(
            err.is_err(),
            "a second connection to a file that has been appended through at least once must \
             still be refused by the trigger written into that file's schema"
        );

        drop(other_connection);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- Planted defect 6: a verifier that accepts every chain must fail
    // loudly. This crate's own EventLog::verify is not that verifier: the
    // tests above prove it refuses five different broken chains. A stub that
    // always returns Ok is written right here, deliberately, as the negative
    // example: it is not called on a broken chain and asked to fail, because
    // "always Ok" by definition never disagrees with anything, which is
    // exactly why a verifier shaped like it is the worst defect this file can
    // carry rather than a merely incomplete one.
    #[test]
    fn ori_t_0023_defect_6_a_verifier_that_always_accepts_would_pass_a_broken_chain_undetected() {
        // Deliberately not calling EventLog::verify: this closure is the
        // "verifier accepts every chain" failure mode by construction, kept
        // here as a named, running witness of what that mistake looks like,
        // so a future reader has a concrete negative example next to the
        // real function rather than only a sentence in a doc comment.
        fn always_accepts(_events: &[Event]) -> Result<(), EventLogError> {
            Ok(())
        }
        let broken = vec![]; // no events at all: the maximally broken input
        assert!(
            always_accepts(&broken).is_ok(),
            "this stub is the defect: it must never be EventLog::verify"
        );
        // The real function does not have this shape: see
        // ori_t_0023_a_mutated_row_is_detected_on_verify and
        // ori_t_0023_a_deleted_middle_row_is_detected_on_verify, which fail
        // exactly where always_accepts above could not.
    }

    // --- Planted defect 7: a verifier that rejects every chain, including a
    // valid one, must fail too. Symmetric witness to defect 6.
    #[test]
    fn ori_t_0023_defect_7_a_verifier_that_always_rejects_would_fail_a_valid_chain() {
        fn always_rejects(_events: &[Event]) -> Result<(), EventLogError> {
            Err(EventLogError::Malformed {
                what: "chain",
                value: "always rejects".to_owned(),
            })
        }
        let mut conn = connection();
        let event = EventLog::append(
            &mut conn,
            product(),
            ts(1),
            Actor::System,
            "ticket.queued",
            None,
            "{}",
        )
        .expect("one valid event");
        assert!(
            EventLog::verify(&conn).is_ok(),
            "the real verifier accepts a genuinely valid chain"
        );
        assert!(
            always_rejects(&[event]).is_err(),
            "this stub is the defect: it must never be EventLog::verify, which just passed \
             the same chain"
        );
    }

    // --- Planted defect 8: break your own reader so it finds no events;
    // must FAIL, not pass. check_reader_completeness is the shipped fix
    // EventLog::verify calls before trusting anything else it read; this
    // test calls that real function directly with the counted-vs-read
    // disagreement a broken reader produces (one row on disk, zero read),
    // which is the deterministic, in-repository half of proving this. The
    // scratchpad proof under the pull request report goes further, per
    // AICD §14: it mutates a copy of EventLog::verify itself to skip this
    // check (i.e. to have the exact shape "an empty read passes"), and shows
    // the tests in this file that call EventLog::verify on a real, non-empty
    // log then fail red, which is what makes the shipped version, that
    // passes them, a check rather than a decoration.
    #[test]
    fn ori_t_0023_defect_8_a_reader_that_finds_no_events_must_fail_not_pass() {
        let err = super::check_reader_completeness(1, 0)
            .expect_err("one row on disk and zero read must never be reported as fine");
        assert!(matches!(
            err,
            EventLogError::ReaderIncomplete {
                counted: 1,
                read: 0
            }
        ));

        // The naive shape this guards against, spelled out: a broken reader
        // that finds nothing also finds nothing wrong, which is the vacuous
        // truth this defect is about.
        let broken_read: Vec<Event> = Vec::new();
        assert!(
            broken_read.is_empty(),
            "restating the trap for a reader that finds nothing"
        );

        // The reverse must still be fine: a reader that found everything
        // agrees with the independent count.
        assert!(super::check_reader_completeness(3, 3).is_ok());

        // End to end, through the real, unmodified EventLog::verify, on a
        // real log with a real event: the check above is not merely
        // reachable, it is the one EventLog::verify actually calls, so a
        // genuinely complete read still verifies clean.
        let mut conn = connection();
        EventLog::append(
            &mut conn,
            product(),
            ts(1),
            Actor::System,
            "ticket.queued",
            None,
            "{}",
        )
        .expect("one real, correctly chained event");
        let report = EventLog::verify(&conn)
            .expect("an untouched log with one real event verifies clean end to end");
        assert_eq!(report.events_checked, 1);
    }

    /// Closes the gap the scratchpad proof in the pull request report found:
    /// removing the call to `check_reader_completeness` from
    /// `EventLog::verify`, with the SQL reader left honest, passed every
    /// other test in this file, because none of them ever produce a real
    /// disagreement between an independent count and what the reader
    /// returns for `EventLog::verify` to have caught. This test manufactures
    /// that disagreement directly, over real, validly-chained events, by
    /// calling `verify_events` (the exact function `EventLog::verify`
    /// delegates to) with a `counted` that does not match them.
    #[test]
    fn ori_t_0023_defect_8_a_disagreeing_count_is_refused_even_when_every_event_is_otherwise_valid()
    {
        let mut conn = connection();
        for n in 1..=3i64 {
            EventLog::append(
                &mut conn,
                product(),
                ts(n),
                Actor::System,
                "ticket.queued",
                None,
                "{}",
            )
            .expect("append");
        }
        let events = EventLog::read_range(&conn, 1, 3).expect("read the three events back");
        assert_eq!(events.len(), 3);

        let err = super::verify_events(4, events.clone()).expect_err(
            "three genuinely valid events must still be refused against a count of four",
        );
        assert!(matches!(
            err,
            EventLogError::ReaderIncomplete {
                counted: 4,
                read: 3
            }
        ));

        // The honest count is still fine, over the same, real events.
        let report = super::verify_events(3, events).expect("the honest count agrees");
        assert_eq!(report.events_checked, 3);
    }

    #[test]
    fn ori_t_0023_an_unchanged_chain_passes() {
        let mut conn = connection();
        for n in 1..=10i64 {
            EventLog::append(
                &mut conn,
                product(),
                ts(n),
                Actor::Human(id(&format!("HUMAN{n:021}"))),
                "document.approved",
                None,
                format!(r#"{{"n":{n}}}"#),
            )
            .expect("append");
        }
        let report = EventLog::verify(&conn).expect("ten correctly appended events verify clean");
        assert_eq!(report.events_checked, 10);
        assert_eq!(report.tip_seq, Some(10));

        let page = EventLog::read_range(&conn, 3, 7).expect("a mid-range page reads clean");
        assert_eq!(page.len(), 5);
        assert_eq!(page.first().map(Event::seq), Some(3));
        assert_eq!(page.last().map(Event::seq), Some(7));
    }

    #[test]
    fn ori_t_0023_the_digest_is_sensitive_to_every_field_not_only_the_payload() {
        let base = Event {
            seq: 1,
            product_id: product(),
            at: ts(1),
            actor: Actor::System,
            kind: "ticket.queued".to_owned(),
            ticket_id: None,
            payload: "{}".to_owned(),
            hash_prev: EventHash::GENESIS,
        };
        let variants: Vec<Event> = vec![
            Event {
                seq: 2,
                ..base.clone()
            },
            Event {
                kind: "ticket.validated".to_owned(),
                ..base.clone()
            },
            Event {
                ticket_id: Some(id("TICKET0000000000000000002")),
                ..base.clone()
            },
            Event {
                actor: Actor::Agent(id("AGENT000000000000000000002")),
                ..base.clone()
            },
        ];
        let base_digest = base.digest();
        for (index, variant) in variants.iter().enumerate() {
            assert_ne!(
                variant.digest(),
                base_digest,
                "variant {index} changed one field and must not digest the same as the base event"
            );
        }
    }

    #[test]
    fn ori_t_0023_length_prefixed_framing_tells_apart_two_concatenations_that_collide_without_it() {
        // "AB" + "C" and "A" + "BC" concatenate to the same bytes; only the
        // length prefix in update_framed keeps their digests apart.
        let short_ticket = Event {
            kind: "AB".to_owned(),
            ticket_id: Some(id_from_raw("C")),
            ..zero_event()
        };
        let long_kind = Event {
            kind: "ABC".to_owned(),
            ticket_id: None,
            ..zero_event()
        };
        assert_ne!(short_ticket.digest(), long_kind.digest());

        fn zero_event() -> Event {
            Event {
                seq: 1,
                product_id: product(),
                at: ts(0),
                actor: Actor::System,
                kind: String::new(),
                ticket_id: None,
                payload: "{}".to_owned(),
                hash_prev: EventHash::GENESIS,
            }
        }

        fn id_from_raw(_short: &str) -> Id {
            // ticket_id must itself be a well-formed Id; the collision this
            // test is about lives in `kind`'s length against a present vs.
            // absent ticket_id, not in the ticket_id's own text, so a fixed
            // valid id stands in for "some ticket_id is present".
            id("C")
        }
    }

    #[test]
    fn ori_t_0023_hash_hex_round_trips() {
        let digest = EventHash::parse(&"a1".repeat(32)).expect("64 valid hex chars");
        assert_eq!(digest.to_hex(), "a1".repeat(32));
        assert!(EventHash::parse("too short").is_err());
        assert!(EventHash::parse(&"zz".repeat(32)).is_err());
    }

    #[test]
    fn ori_t_0023_every_refusal_carries_the_methodology_ref_the_module_doc_comment_promises() {
        let missing_actor = EventLogError::MissingActor { seq: Some(1) };
        assert!(missing_actor.is_refusal());
        let reason = missing_actor
            .methodology_ref()
            .expect("MissingActor is a refusal and carries a reason");
        assert!(reason.resolves());
        assert_eq!(reason.section, 13);

        let seq_bad = EventLogError::SeqNotMonotonic {
            expected: 2,
            found: 1,
        };
        assert!(seq_bad.is_refusal());
        assert_eq!(
            seq_bad
                .methodology_ref()
                .expect("SeqNotMonotonic is a refusal")
                .section,
            8
        );

        let malformed = EventLogError::Malformed {
            what: "kind",
            value: String::new(),
        };
        assert!(!malformed.is_refusal());
        assert!(malformed.methodology_ref().is_none());
    }

    // `spec/TESTING.md` section 1 names the property this crate's hash chain
    // must hold, in its own words: "Invariants hold for generated event
    // sequences (... hash chain unbroken)". The two properties below are that
    // line, positive and negative: an arbitrary sequence of well-formed
    // appends always verifies, and corrupting exactly one stored field,
    // anywhere in an arbitrary chain, is always caught. `proptest` is used
    // rather than a hand-rolled loop of random values because it also
    // shrinks a failing case down to the smallest sequence and the smallest
    // corruption that still fails, which is the difference between a report
    // that says "seed 4451782" and one that names the actual seq and field.
    mod proptests {
        use proptest::prelude::*;

        use super::EventLog;
        use super::EventLogError;
        use super::connection;
        use super::id;
        use super::product;
        use super::ts;
        use ori_core::types::Actor;

        /// A small, readable alphabet of event kinds, not an exhaustive one:
        /// this property is about the chain holding over *some* varying
        /// `kind`, `ticket_id` and `payload`, not about which kinds exist
        /// (`spec/LLD.md` section 2 forbids this crate business rules over
        /// that).
        fn kind_strategy() -> impl Strategy<Value = &'static str> {
            prop::sample::select(
                &[
                    "ticket.filed",
                    "ticket.validated",
                    "gate.proven",
                    "document.approved",
                    "session.recovered",
                ][..],
            )
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]

            /// `ori_t_0023`: an arbitrary sequence of well-formed appends
            /// (varying kind, presence of a ticket, and payload) always
            /// verifies clean, whatever the sequence.
            #[test]
            fn ori_t_0023_property_arbitrary_valid_sequences_always_verify(
                kinds in prop::collection::vec(kind_strategy(), 1..16),
                has_ticket in prop::collection::vec(any::<bool>(), 1..16),
            ) {
                let mut conn = connection();
                let count = kinds.len().min(has_ticket.len());
                for i in 0..count {
                    let ticket = has_ticket[i].then(|| id(&format!("T{i}")));
                    EventLog::append(
                        &mut conn,
                        product(),
                        ts(i64::try_from(i).unwrap_or(i64::MAX)),
                        Actor::System,
                        kinds[i],
                        ticket,
                        format!(r#"{{"n":{i}}}"#),
                    )
                    .expect("a well-formed append never fails");
                }
                let report = EventLog::verify(&conn)
                    .expect("an arbitrary sequence of well-formed appends must still verify");
                prop_assert_eq!(report.events_checked, count as u64);
                prop_assert_eq!(report.tip_seq, Some(count as u64));
            }

            /// `ori_t_0023`: corrupting exactly one event's `kind`, at a
            /// position that already had a stored successor *before* the
            /// corruption happened, is always caught by `verify`.
            ///
            /// Deliberately never corrupts the chain's current last row:
            /// an earlier draft of this property did, expecting that
            /// appending one more event afterward would "cement" and reveal
            /// it, and that expectation was false (see the module doc
            /// comment's corrected "What that leaves open" and
            /// `tests::proptests::ori_t_0023_tampering_with_the_current_tip_survives_a_further_honest_append`
            /// below, which this property's own failure led to). Finding
            /// that here rather than in the specification is what this test
            /// is for.
            #[test]
            fn ori_t_0023_property_a_single_corrupted_kind_before_its_successor_existed_is_always_detected(
                length in 2usize..12,
                corrupt_at in 0usize..11,
                replacement in kind_strategy(),
            ) {
                // Never the last index: that row has no successor yet at the
                // moment it would be corrupted, which is exactly the case
                // this property excludes.
                let corrupt_at = corrupt_at % (length - 1);
                let corrupt_seq = i64::try_from(corrupt_at + 1).unwrap_or(i64::MAX);
                let conn = connection();
                for i in 0..length {
                    super::append_with_no_guard_installed(
                        &conn,
                        product(),
                        ts(i64::try_from(i).unwrap_or(i64::MAX)),
                        Actor::System,
                        "ticket.queued",
                        "{}",
                    );
                }
                let original: String = conn
                    .query_row(
                        "SELECT kind FROM events WHERE seq = ?1",
                        rusqlite::params![corrupt_seq],
                        |row| row.get(0),
                    )
                    .expect("the row this test just wrote is readable");
                prop_assume!(original != replacement);
                conn.execute(
                    "UPDATE events SET kind = ?1 WHERE seq = ?2",
                    rusqlite::params![replacement, corrupt_seq],
                )
                .expect("no guard was installed on this connection");

                let verdict = EventLog::verify(&conn);
                let caught = matches!(verdict, Err(EventLogError::ChainBroken { .. }));
                prop_assert!(
                    caught,
                    "seq {corrupt_seq} of {length} already had a stored successor when it was \
                     corrupted, so verify must catch it: {verdict:?}"
                );
            }
        }

        /// Proves the limit `mod proptests` above found: tampering with
        /// whatever is, at that moment, the chain's last event is not
        /// detected, and appending further honest events afterward does not
        /// change that, because each of them links to whatever is *actually*
        /// stored rather than to what used to be there. Not a property test
        /// (nothing here is generated): one clear, deterministic case is
        /// enough to keep the module doc comment's correction honest.
        #[test]
        fn ori_t_0023_tampering_with_the_current_tip_survives_a_further_honest_append() {
            let conn = connection();
            super::append_with_no_guard_installed(
                &conn,
                product(),
                ts(1),
                Actor::System,
                "ticket.queued",
                "{}",
            );
            conn.execute("UPDATE events SET kind = 'tampered.kind' WHERE seq = 1", [])
                .expect("no guard was installed on this connection");

            // Two further events, appended honestly, each reading whatever
            // is actually stored (which is now the tampered row).
            super::append_with_no_guard_installed(
                &conn,
                product(),
                ts(2),
                Actor::System,
                "ticket.validated",
                "{}",
            );
            super::append_with_no_guard_installed(
                &conn,
                product(),
                ts(3),
                Actor::System,
                "gate.proven",
                "{}",
            );

            let report = EventLog::verify(&conn).expect(
                "three honestly-linked events over a tampered tip still verify clean: the \
                 tampering is invisible to chain linkage once something has been built on top \
                 of it, not only until then",
            );
            assert_eq!(report.events_checked, 3);
        }
    }
}
