-- Migration 0002: projections.
--
-- Ticket: ORI-T-0025. Spec: DATA_MODEL.md#4-invariants-across-entities.
--
-- Three tables, one per projector `crates/ori-store/src/projections/` builds:
-- `proj_tickets` (the Ticket entity), `proj_locks` (LockEntry) and
-- `proj_escalations` (Escalation). `spec/DATA_MODEL.md` section 2 gives the
-- fields; the columns below carry exactly the subset each projector can
-- honestly derive from the event log without a JSON library this crate does
-- not have (see `crates/ori-store/src/projections/payload.rs`'s module doc).
--
-- Never edited after it has run anywhere, the same rule `crates/ori-store/src/db.rs`
-- enforces for every migration by comparing the recorded text byte for byte
-- against what is compiled in. This file is new, never `0001_init.sql`, which
-- has already run.
--
-- Unlike `events`, these tables are derived state, not the log: nothing here
-- carries a trigger refusing UPDATE or DELETE. `crates/ori-store/src/rebuild.rs`
-- clears and rewrites every row of all three on every rebuild, and a projector
-- freely rewrites a row as later events arrive; that is what "projection"
-- means (`spec/DATA_MODEL.md` section 1: "the tables below are projections
-- rebuilt from the log").

-- `proj_tickets`: `spec/DATA_MODEL.md` section 2's Ticket row, the fields a
-- projector can derive from the event's own columns (`ticket_id`, `actor`,
-- `at`, `seq`) and the flat string fields
-- `crates/ori-store/src/projections/payload.rs` reads out of `payload`.
-- `title`, `spec_anchor`, `declared_scope`, `budget` and `phase_id` are not
-- columns here: deriving them needs a real JSON parser this crate does not
-- carry, and inventing one for five fields this ticket's criterion does not
-- exercise was judged out of scope; see the pull request report.
CREATE TABLE proj_tickets (
    ticket_id   TEXT PRIMARY KEY,
    product_id  TEXT NOT NULL,
    state       TEXT NOT NULL,
    category    TEXT,
    kind        TEXT,
    tier        TEXT,
    significant TEXT,
    last_seq    INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- `proj_locks`: `spec/DATA_MODEL.md` section 2's LockEntry row, "product_id,
-- module, ticket_id, session_id, acquired_at". Keyed by `module`, mirroring
-- `crates/ori-orchestrator/src/lock_table.rs`'s own in-memory `LockTable`
-- (`entries: BTreeMap<String, Entry>`), which is the live authority for the
-- invariant `spec/DATA_MODEL.md` section 4 states, "`LockEntry` modules for
-- two `InProgress` tickets never overlap": that invariant is enforced there,
-- before an event is ever appended, not re-decided by this projection (see
-- `crates/ori-store/src/projections/lock.rs`'s module doc, "must not contain
-- business rules", `spec/LLD.md` section 2).
CREATE TABLE proj_locks (
    module      TEXT PRIMARY KEY,
    product_id  TEXT NOT NULL,
    ticket_id   TEXT NOT NULL,
    session_id  TEXT,
    acquired_at INTEGER NOT NULL
);

-- `proj_escalations`: `spec/DATA_MODEL.md` section 2's Escalation row, "id,
-- ticket_id, trigger, question, recommendation, context_package_ref, state
-- (open, answered), answered_by, answer, answered_at". Keyed by the
-- escalation's own id (`escalation_id`), read out of `payload`, because one
-- ticket may raise more than one escalation over its lifetime
-- (`spec/DATA_MODEL.md` section 1: `Ticket ||--o{ Escalation : raises`) and
-- the event's own `ticket_id` column cannot key that. `context_package_ref`
-- and `answered_at` are not columns here for the reason `proj_tickets`'
-- comment gives; `updated_at` stands in for `answered_at` as the timestamp of
-- whichever event (raised or answered) last touched the row.
CREATE TABLE proj_escalations (
    escalation_id  TEXT PRIMARY KEY,
    product_id     TEXT NOT NULL,
    ticket_id      TEXT NOT NULL,
    trigger        TEXT,
    question       TEXT,
    recommendation TEXT,
    state          TEXT NOT NULL,
    answered_by    TEXT,
    answer         TEXT,
    last_seq       INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL
);
