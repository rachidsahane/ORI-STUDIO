-- Migration 0001: init.
--
-- Ticket: ORI-T-0024. Spec: DATA_MODEL.md#2-entities.
--
-- This is the only migration a fresh `product.sqlite` ever needs to reach
-- schema version 1. `crates/ori-store/src/db.rs` embeds this file with
-- `include_str!` at compile time (never reads it from disk at runtime, so a
-- missing or misplaced directory on a user's machine cannot make the reader
-- find zero migrations) and records it, verbatim, in `_ori_migrations` once
-- applied, so a later run of this same binary can prove the file has not
-- changed since (`db.rs`'s module doc explains what enforces that and why a
-- text comparison rather than a hash).
--
-- `_ori_product` is a singleton table: `CHECK (id = 1)` on an
-- `INTEGER PRIMARY KEY` means a second row can never be inserted, only the
-- first row's `id = 1` ever satisfies both the primary key and the check.
-- `ProductDb::open` stamps it with the product_id it was asked to open on the
-- first open of a fresh file, and compares it on every open after that, which
-- is what refuses two products sharing one `product.sqlite`
-- (ORI-P1-036: "a ticket in one is invisible to the other").
CREATE TABLE _ori_product (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    product_id TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

-- `events`: `spec/DATA_MODEL.md` section 2's row, "seq (monotonic),
-- product_id, at, actor (human identity or agent identity or system), kind,
-- ticket_id (nullable), payload (json), hash_prev".
--
-- `seq` is `INTEGER PRIMARY KEY AUTOINCREMENT` rather than a plain
-- `INTEGER PRIMARY KEY`: without `AUTOINCREMENT`, SQLite reuses the highest
-- rowid that ever existed in the table once its row is gone, so a sequence
-- that must never repeat a value needs the one keyword that says so, even
-- though this table's rows are never deleted in the first place (see the
-- triggers below): the guarantee should not depend on that also being true.
--
-- `product_id` here is redundant with which file this is, on purpose: it
-- lets a query, a backup restored to the wrong place, or a future join
-- notice a row that does not belong, rather than trusting the filename.
--
-- `actor_kind` / `actor_id` is `Actor` from `ori_core::types` (`Human(Id)`,
-- `Agent(Id)`, `System`) written as two columns because SQLite has no sum
-- type: the `CHECK` at the end of the table is the constraint
-- `Actor::identity` states in Rust ("`None` for `Actor::System`"), stated
-- again here so a row inserted by any future code path that is not
-- `ori_core::types::Actor` still cannot violate it.
--
-- `payload` is `CHECK (json_valid(payload))`, a format check (every payload
-- is well-formed JSON), not a business rule (this crate owns no opinion on
-- what a payload contains): `spec/LLD.md` section 2's "Must not: contain
-- business rules" is about what a kind of event means, which stays in
-- `ori-core` and the sibling `event_log.rs`.
--
-- `hash_prev` is `NOT NULL`: this migration fixes that the column exists and
-- is always populated; what a first event's "previous" hash is (a fixed
-- genesis value, or some other convention) is the hash chain's own algorithm,
-- which is `event_log.rs`'s decision, not this migration's.
CREATE TABLE events (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id TEXT NOT NULL,
    at INTEGER NOT NULL,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('human', 'agent', 'system')),
    actor_id TEXT,
    kind TEXT NOT NULL,
    ticket_id TEXT,
    payload TEXT NOT NULL CHECK (json_valid(payload)),
    hash_prev TEXT NOT NULL,
    CHECK (
        (actor_kind = 'system' AND actor_id IS NULL)
        OR (actor_kind != 'system' AND actor_id IS NOT NULL)
    )
);

-- A query pattern ORI-P1-036 names directly, "a ticket in one is invisible
-- to the other": once it is answered by "which product's file this is", the
-- remaining half is "which of this product's events are this ticket's",
-- which is this index. Partial (`WHERE ticket_id IS NOT NULL`) because most
-- events are not ticket-scoped and indexing every `NULL` buys nothing.
CREATE INDEX events_ticket_id_idx ON events (ticket_id) WHERE ticket_id IS NOT NULL;

-- CLAUDE.md's load-bearing fact: "The event log in `ori-store` is
-- append-only and hash-chained. No code path updates or deletes an event."
-- That rule lives in this crate's code today (the sibling ticket for
-- `event_log.rs` is the interface that is supposed to expose no UPDATE or
-- DELETE at all), but a rule that only one interface honours is a rule one
-- bug away from being false. These triggers make the database itself refuse
-- either statement against `events`, from any connection, including a raw
-- one a future crate opens directly, which no amount of care in one crate's
-- Rust code can promise on its own.
CREATE TRIGGER events_no_update
BEFORE UPDATE ON events
BEGIN
    SELECT RAISE(ABORT, 'events is append-only: no code path updates a row');
END;

CREATE TRIGGER events_no_delete
BEFORE DELETE ON events
BEGIN
    SELECT RAISE(ABORT, 'events is append-only: no code path deletes a row');
END;
