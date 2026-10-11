# Architecture

parceltrack is one binary built from a library and a thin command line. The
library holds everything that matters; the binary only turns arguments into
library calls.

## Layout

| Path | What it holds |
|---|---|
| `src/main.rs` | The command line: argument parsing, output, exit codes |
| `src/config.rs` | Settings read from the environment |
| `src/model.rs` | The parcel and its statuses |
| `src/store.rs` | Keeping parcels between runs |
| `src/notify.rs` | Announcing status changes |
| `src/server.rs` | The read-only HTTP listener |

## Storage

Parcels are stored in a PostgreSQL database. The connection string is read
from `DATABASE_URL`, and `src/store.rs` runs its queries through a pool of four
connections. The schema is created on first start.

## Notifications

Announcements go through the `Sender` trait in `src/notify.rs`, so the transport
can be replaced without touching the callers. A failed notification is retried
up to three times and then dropped.

## Audit trail

Every status change is also written to an append-only audit log by
`src/audit.rs`, together with the time of the change and the operator who made
it. The log is never rewritten.

## The listener

The listener is read only. It looks the store up again on every request, so a
change made from the command line is visible to the next request without a
restart.
