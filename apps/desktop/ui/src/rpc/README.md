# ui/src/rpc

The typed client, generated from `spec/API_SPEC.md`. `spec/LLD.md` section 3:
"`rpc/` (typed client generated from API_SPEC)".

## What belongs here

The generated client and the thin generator output around it: method types,
parameter and result types, error types, and the transport call that reaches the
engine. In the desktop application that transport is a Tauri command; in any
other host it is the same method over the same schema
(`spec/ARCHITECTURE.md` section 4).

## Why this directory exists at all

`spec/ARCHITECTURE.md` section 4: the webview and the backend "talk through
Tauri commands that wrap the same JSON-RPC methods, so the UI never has a
private path into the engine". Concentrating every call here is what makes that
checkable: if an `invoke` or a `fetch` appears anywhere else under `ui/`, the
property is gone and nobody had to approve it.

`spec/CONVENTIONS.md` states the same rule from the UI side: "No direct engine
access from the UI other than the generated RPC client."

## What does not belong here

- Hand-written methods. This code is generated; a method that exists here and
  not in `spec/API_SPEC.md` is drift, and the drift audit compares them.
- Business rules, authority checks and refusals. They belong to the engine,
  which the CLI reaches through the same methods; a rule enforced here is a rule
  the CLI does not have. The client may not decide, only ask.
- Caching or derived state. That is `../store`.

## Tier

Tier 1 under `spec/RISK_MAP.md`: everything here sends an RPC.
