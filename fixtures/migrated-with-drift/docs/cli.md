# Command line

```
parceltrack <command>
```

Parcels are read from and written to the file named by `PARCEL_DATA_FILE`,
which is `parcels.tsv` in the current directory when the variable is not set.

## Commands

| Command | What it does | What it prints |
|---|---|---|
| `add <id> <destination>` | Registers a parcel. The id may not hold whitespace. | `registered <id>` |
| `status <id>` | Shows one parcel. | `<id>`, status and destination, separated by tabs |
| `advance <id>` | Moves a parcel to its next status. | `<id>` and the new status, separated by a tab |
| `return <id>` | Marks a parcel as returned to its sender. | `<id>` and `returned`, separated by a tab |
| `list` | Shows every parcel, in the order they were registered. | One line per parcel, as for `status` |
| `serve` | Answers status lookups over HTTP. | `listening on 127.0.0.1:<port>` |

`serve` binds the loopback interface on the port named by `PARCEL_PORT`, or on
port 7070 when the variable is not set.

`advance` also announces the change on the standard error stream, prefixed
with `notify:`.

## Refusals

- `add` refuses an id that is already registered.
- `advance` and `return` refuse a parcel that is delivered or returned.
- `status`, `advance` and `return` refuse an id that no parcel has.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | The command succeeded. |
| 1 | The command failed: it was refused, the data file could not be used, or `serve` could not bind its port. |
| 2 | The command line was not understood. The usage text is printed. |
