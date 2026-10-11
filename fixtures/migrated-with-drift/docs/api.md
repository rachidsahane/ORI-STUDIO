# HTTP API

## Endpoints

### `GET /v1/parcels/{id}`

Returns one parcel as JSON.

```json
{"id": "PK-1", "status": "shipped"}
```

An id that no parcel has is answered with status 404 and the body `not found`.

## Statuses

A parcel's `status` is one of `created`, `shipped`, `delivered` or `lost`.

## Methods

Any method other than `GET` is answered with status 405.
