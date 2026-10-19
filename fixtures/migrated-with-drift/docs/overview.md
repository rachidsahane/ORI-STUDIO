# parceltrack

parceltrack records parcels and follows them through a carrier network. A
sender registers a parcel with a destination, the parcel moves forward one
status at a time, and anyone can look its status up from the command line or
over HTTP.

## Quick start

```sh
cargo build --release
./target/release/parceltrack add PK-1 "12 Quay Road"
./target/release/parceltrack advance PK-1
./target/release/parceltrack serve
```

The server listens on port 8080 by default. Set `PARCEL_PORT` to use another
port.

## Life of a parcel

A parcel starts out registered and moves forward until it is delivered. It can
be returned to its sender at any point until it is delivered. Once a parcel is
delivered or returned, it does not change again.

## Continuous integration

Every pull request to `main` runs the full test suite in CI before it can be
merged. The workflow is `.github/workflows/ci.yml`.

## More documentation

- [Architecture](architecture.md): how the program is put together.
- [HTTP API](api.md): the endpoints the listener answers.
- [Command line](cli.md): every command, its output and its exit codes.
- [Runbook](runbook.md): running parceltrack in staging and production.

## Licence

MIT.
