# ORI-T-0189: the escalation and ticket projections read their payloads as JSON (Sonnet: the lead decided every rule below; implement it)

| Field | Value |
|---|---|
| Worktree | `/home/user/wt/ORI-T-0189` |
| Branch | `fix/ORI-T-0189-payload-json`, at `<BASE>` (the session head; its last commit records this ticket's claim 119 and allocates ORI-T-0193 in `ops/lock-table.md`) |
| Tier | **1** (`spec/RISK_MAP.md`, row `crates/ori-store (projections)`). Nothing in this scope is a tier 2 path: not `event_log.rs`, not `migrations/`. No row names `crates/ori-store/Cargo.toml` or `Cargo.lock`, and neither is on gate 12's tier 2 list |
| Declared scope (claim 119) | In `crates/ori-store/src/projections/`: `payload.rs` (all of it but its existing tests), `escalation.rs` and `ticket.rs` (the code that reads a payload, the docs this ticket makes false, and new tests), `mod.rs` (one new `ProjectionError` variant with its arms in `methodology_ref`, `is_refusal` and `Display`; the docs of `MalformedPayload` and of its `field`; one phrase of the module doc section "What a payload convention this ticket invents means for \"identically\""; the `# Errors` text of `Projection::apply`; one new test). `crates/ori-store/Cargo.toml`: two dependency lines with their comment. `Cargo.lock`: ori-store's own entry only, as cargo writes it. **Not** `lock.rs`, not `event_log.rs`, not `migrations/`, not `rebuild.rs`, no other crate. **No existing test changes** |
| Spec trailer | `Spec: DATA_MODEL.md#2-entities` |
| Ticket trailer | `Ticket: ORI-T-0189`, **one per commit** |
| Commit types | only `feat`, `fix`, `docs`, `ci`, `chore`. **Never `test(...)`** |

## Why

`crates/ori-store/src/projections/payload.rs` reads a payload without a JSON parser. `field` takes the first `"key"` it finds anywhere in the text, values included, cuts the value at the next `"`, and undoes no escape. A serde writer escapes `"`, `\` and every control character below U+0020, tab, line feed and carriage return included.

So an escalation question, recommendation or answer reaches the human queue:
- cut short at its first quote;
- with every backslash doubled;
- or with literal `\n` sequences.

A value equal to a key name can also be read as that key: `{"question":"recommendation","escalation_id":"E1",...}` gives `recommendation` the value `E1`. S9's desk contract orders `escalation.raised`'s keys to soften that second hazard; the first remains (`ops/lock-table.md`, the record that allocated this ticket).

Nothing writes these events in production yet (ORI-T-0070 will), so the rule can be fixed now, before the first row exists.

Read first, on your base:
- `crates/ori-store/src/projections/payload.rs`, `escalation.rs`, `ticket.rs`, `lock.rs`, and `mod.rs` (`ProjectionError`, `Projection::apply`'s docs, the module doc);
- `crates/ori-store/src/rebuild.rs`: `rebuild` (one transaction, rolled back at the first refusal) and its property test's generator;
- `crates/ori-store/Cargo.toml`;
- `ops/escalations/E-0008-json-serialization.md`, all of it;
- `crates/ori-runtime/Cargo.toml` and `crates/ori-memory/Cargo.toml`: how the two crates pin `serde` and `serde_json`, and their comments;
- the lead's proof of Decision 2's reader against the pinned serde 1.0.229 and serde_json 1.0.151: `/tmp/claude-0/-home-user-ORI-STUDIO/74689e95-cfc6-57f1-9c51-28798b55f386/scratchpad/proof-0189/src/main.rs` (it prints `ALL CLAIMS HOLD`). It covers every refusal and acceptance in Decision 2 except the number range and the `\u` escapes, which the lead's fact-check verified separately (`{"a":1e400}` is refused, `1e300` accepted; `\u00e9\ud83d\ude00` decodes to U+00E9 U+1F600);
- in `crates/ori-memory/src/scope.rs`: `projection_field`, `LockClaimWire`, and the test `ori_p1_022_the_lock_fold_keeps_the_rows_the_lock_projection_keeps`. You read them to see why `lock.rs` is left alone. You change nothing there.

## Decision 1: what changes, and what does not

- **The escalation and ticket projections read their payloads as JSON**, through the new reader of Decision 2.
- **The lock projection keeps `payload::field`, unchanged.** ori-memory's lock fold reads `module` through its own copy of that reader (`projection_field`). Its tier 2 test `ori_p1_022_the_lock_fold_keeps_the_rows_the_lock_projection_keeps` fails if the projection reads `module` any other way. A fact-check measured it: with `lock.rs` on the JSON reader, that test fails and the other 543 ori-memory tests pass. Moving ori-memory's copy along with it would fail two more of its tests. Either is a `test_modified` escalation and a tier 2 change.

  `ModulePath::parse` refuses a backslash and control characters, and S9 refuses `"`, so the desk can write no `module` value on which the two readers differ. Moving the lock projection and ori-memory together is ORI-T-0193, allocated for the operator in the record on your base. Do not touch `lock.rs`.
- **`field` stays exactly as it is**, still called by `lock.rs`. Only its doc changes (Decision 5).
- **`bool_field` keeps its signature and its two tests.** It is re-implemented on the new reader (Decision 2) and marked `#[cfg(test)]`, because no projection calls it any more. Otherwise rustc's `dead_code` lint, which gate 1's `clippy -- -D warnings` turns into an error, would fail the gate.

## Decision 2: the reader (`payload.rs`)

Add a JSON reader beside `field`:

- `pub(crate) struct Object`, holding a private `fields: BTreeMap<String, serde_json::Value>`. It derives `Debug`, which the tests print; add `PartialEq` only if a test compares two objects.
- `#[must_use] pub(crate) fn object(payload: &str) -> Option<Object>`: `serde_json::from_str::<Object>(payload).ok()`.
- `impl<'de> serde::Deserialize<'de> for Object`, written by hand (no derive): it calls `deserializer.deserialize_map(ObjectVisitor)`. `ObjectVisitor` implements only `expecting` and `visit_map`.
  - `visit_map` reads each key as a `String`. If the key is already in the map, it returns `serde::de::Error::custom(...)` **before** it reads that key's value. Otherwise it reads the value as a `serde_json::Value` and inserts it.
  - A JSON array, string, number, boolean or `null` at the top is refused. serde_json's `deserialize_map` reports an invalid type, naming `expecting`'s text, for anything that does not begin with `{`, and never calls the visitor. Say it that way in any doc you write.
- `impl Object`:
  - `#[must_use] pub(crate) fn text(&self, key: &str) -> Option<&str>`: the value if it is a JSON string, else `None` (absent key, number, boolean, null, array, object).
  - `#[must_use] pub(crate) fn boolean(&self, key: &str) -> Option<bool>`: the value if it is a JSON boolean, else `None`.
- `bool_field(payload, key)` becomes `object(payload)?.boolean(key)`, under `#[cfg(test)]`. Its doc says it is kept, test-only, for its two ORI-T-0025 tests, which `CLAUDE.md` rule 3 forbids changing, and that no projection calls it. Do not intra-doc-link `bool_field` from any doc that is compiled outside tests: under `cargo doc` such a link no longer resolves.

**What `object` refuses** (returns `None`), each by serde_json 1.0.151 as pinned:
- text that is not JSON;
- JSON whose top level is not an object;
- an object followed by anything but whitespace;
- a key given twice at the top level, whatever the two values;
- nesting 128 levels deep or more, counting the top object (the lead measured 127 levels accepted and 128 refused);
- a raw control character inside a string, a lone surrogate escape, a number outside `f64`'s range.

**What it accepts**: any top-level object with distinct keys, whatever the values. That covers nested values such as S9's `evidence` array, and keys this projection does not read. A duplicate inside a nested value is that value's own business, and is not looked at. Whitespace around the object and between tokens is accepted.

**Why a duplicate key is refused.** State this in the doc of `Object`:
- Two values for one key have no single reading. A first-wins reader (`field`, and every reader like it) and a last-wins reader (`serde_json::Value`) would disagree on the same row. Refusing is the only rule that cannot disagree with another reader.
- The cost: such an event blocks every rebuild of its product (`crate::rebuild::rebuild` stops at the first refusal). But no writer the engine has or plans would produce one:
  - every planned writer serializes one derived struct through serde, which writes each field once (S9's desk contract for `escalation.raised`, `crates/ori-mcp/src/ports.rs`);
  - a `#[serde(flatten)]` map, two fields renamed onto one name, or a hand-written `Serialize` could repeat a key, and no planned payload uses any of them.
- ori-memory's `LockClaimWire` likewise refuses a `lock.claimed` payload that gives `module` or `session_id` twice, through serde's derive.

## Decision 3: a new refusal, `ProjectionError::UnreadablePayload { seq: u64 }`

Put it after `MalformedPayload`. Its doc: "From the ticket and escalation projections, for an event kind that reads payload fields: the payload is not one JSON object whose keys are distinct. That is text that is not JSON, JSON of another shape (an array, a string, a number), text after the object, or a key given twice." The field `seq` gets the same doc as `MalformedPayload`'s.

- `methodology_ref`: AICD §8, the same section and the same reason as `MalformedPayload`. Put the variant in that arm (`Self::MalformedPayload { .. } | Self::UnreadablePayload { .. }`), and extend the arm's comment by one clause.
- `is_refusal`: `true`.
- `Display`: `refused: event at seq {seq}'s payload is not one JSON object whose keys are distinct`.
- `MalformedPayload` keeps its shape. **The lock projection still returns it for a payload that is not JSON at all** (`lock.rs` reads `module` with `payload::field`), and ori-memory's tests rely on that. Its doc says:
  - from the ticket and escalation projections, the payload is a readable object that lacks a required field or carries it as another JSON type;
  - from the lock projection, `payload::field` finds no such field in the payload.

  Its `field` doc: "The field name that was required and absent, or of another type."
- `Projection::apply`'s `# Errors`, the sentence on `MalformedPayload` replaced by:
  - "[`ProjectionError::UnreadablePayload`] when the ticket or escalation projection, for an event kind that reads payload fields, finds that `event.payload()` is not one JSON object whose keys are distinct.
  - [`ProjectionError::MalformedPayload`] when a required field is absent from, or of the wrong type in, `event.payload()`. The lock projection reads its fields with `payload::field`, not as JSON, so it reports a payload that is not JSON as `MalformedPayload` too."

  `payload::field` is `pub(crate)` and `Projection` is public: do not bracket-link it.

`ProjectionError` is `#[non_exhaustive]`. No crate outside ori-store matches it exhaustively; confirm that with a grep and report it.

## Decision 4: the two projections

**The read order and the error order are fixed by existing tests, and stay:**
- `MissingTicketId` is checked before the payload is looked at;
- each field is read in today's order;
- the first field absent or of the wrong type is the one `MalformedPayload` names.

**Parse once per event, inside the match arm that reads fields, and only there.** The kinds that read no field must keep accepting any payload, JSON or not:
- `ticket.rejected`, `validated`, `queued`, `in_progress`, `blocked`, `escalated`, `in_review`, `deployed` and `closed`;
- any `ticket.*` or `escalation.*` suffix this projection does not know.

A refusal of such an event would block every future rebuild of its product, for a field nobody reads.

In **each** of `escalation.rs` and `ticket.rs`, replace `required_field` (and, in `ticket.rs`, `required_bool_field`) with:
- `fn payload_object(event: &Event) -> Result<payload::Object, ProjectionError>`, giving `UnreadablePayload { seq: event.seq() }` when `payload::object` gives `None`;
- `fn required_field<'a>(event: &Event, object: &'a payload::Object, key: &'static str) -> Result<&'a str, ProjectionError>`, giving `MalformedPayload { seq, field: key }` when `object.text(key)` gives `None`;
- in `ticket.rs` only, `fn required_bool_field(event: &Event, object: &payload::Object, key: &'static str) -> Result<bool, ProjectionError>`, the same with `object.boolean(key)`.

The arms that read fields:
- **escalation:** `raised` reads `escalation_id`, `trigger`, `question`, `recommendation`; `answered` reads `escalation_id`, `answered_by`, `answer`;
- **ticket:** `filed` reads `category`, `kind`; `categorized` and `category_set` read `category`; `tier_set` reads `tier`; `merged` reads `significant` as a boolean.

Each such arm makes `let object = payload_object(event)?;` its first statement (in `ticket.rs`, before `row.state = ...`), then reads its fields from that object in today's order. Leave everything else exactly as it is: the state assignments and the reads keep their relative order, and the rows, the SQL and the `.to_owned()` calls do not change.

**Docs in these two files:**
- `ticket.rs`'s table header "Required payload field" stays. Below the table's paragraph, add one sentence: the payload of a kind that reads a field must be one JSON object whose keys are distinct (`UnreadablePayload` otherwise), and a kind that reads none is not parsed.
- `escalation.rs`'s "The event kinds this projection owns": add the same sentence. Also say that keys this projection does not read, such as `evidence`, are accepted and ignored, because `proj_escalations` has no column for them.

## Decision 5: the docs of `payload.rs` and `mod.rs`

**`payload.rs`'s module doc**, rewritten. It says:
- This module has two readers. `object` parses JSON (serde_json; E-0008), and the escalation and ticket projections use it. `field` is the first-key reader of ORI-T-0025, and only the lock projection uses it.
- **Why the lock projection keeps `field`:**
  - ori-memory's lock fold reads `module` through a copy of it;
  - a tier 2 test there holds the two to the same rows;
  - the desk can write no `module` value on which the two readers differ;
  - moving both is ORI-T-0193.
- The old text's claims that the crate has no JSON library and that it lists exactly three dependencies are no longer true: remove them.
- The claim that a value cannot hold a `"` stays true of `field`: move it into `field`'s doc.
- Keep the last paragraph: no methodology section applies to the reader itself.

**`field`'s doc:** keep its "Not a JSON parser" sentence and the mechanism text. Add:
- that it cuts a value at the next `"` and undoes no escape;
- that only `crate::projections::lock` calls it, for the reason in the module doc.

**`mod.rs`'s module doc**, section "What a payload convention this ticket invents means for \"identically\"": the phrase "the flat-field payload shape `crate::projections::payload` reads" becomes "the payload fields `crate::projections::payload` reads (one JSON object for the escalation and ticket projections)". Leave the rest of the section; it is out of this ticket's concern. Its "nothing in the workspace" means no production writer.

**A doc that names a test** writes it as `tests::<name>`, never the bare name. The doc-citation check in `crates/ori-gates/src/sections.rs` fails the workspace suite on a bare test name it does not register, and its register is out of scope.

**Stale text outside this scope.** Leave it, and list it in your report as not changed. ORI-T-0193 takes it.
- `event_log.rs`'s "a parser this crate does not have" passage (`:207-210`).
- `migrations/0001_init.sql`'s and `0002_projections.sql`'s comments that there is no JSON parser.
- `crates/ori-mcp/src/ports.rs`'s first-`"key"` sentences, and `crates/ori-mcp/src/tools/orchestration.rs`'s passage on how the queue shows escalation text.
- `crates/ori-broker/src/keychain.rs:162-165`: serde is now in ori-broker's dependency graph, through ori-store.
- `rebuild.rs`'s `interpret` doc, which lists the refusal paths without `UnreadablePayload`.

ori-memory's docs describe the lock reader, which this ticket keeps, and stay true.

## Decision 6: the dependencies

Add to `crates/ori-store/Cargo.toml`, under `[dependencies]` and after `sha2`:

```toml
serde = { version = "=1.0.229", default-features = false, features = ["std"] }
serde_json = { version = "=1.0.151", default-features = false, features = ["std"] }
```

Above them, write a comment in the file's own style. It says:
- **Basis:** E-0008, Answer: "Approved: `serde` and `serde_json`, pinned, workspace-wide." This ticket adds them to ori-store under that answer, on the exact versions `crates/ori-runtime/Cargo.toml` pins. That keeps `deny.toml`'s `multiple-versions = "deny"` satisfied and adds no package to `Cargo.lock`.
- **Why JSON:** the escalation and ticket projections read agent and human text written by a serde writer, which escapes characters the old reader cannot undo. E-0008's rejected-option table names this very reader as one that "already states it is not a JSON parser and names what it gets wrong".
- **Why `serde` without `derive`:** `payload.rs` implements `Deserialize` by hand for one type, so as to refuse a duplicated key, and that needs only `serde`'s traits. In a workspace build Cargo unifies serde's features, so the `derive` that ori-runtime, ori-memory and ori-mcp enable is still compiled once; ori-store asks only for what it uses.
- **Why these features:** none of `arbitrary_precision`, `raw_value`, `preserve_order`, `float_roundtrip` or `unbounded_depth` is enabled in the normal dependency graph. tree-sitter's build script enables `preserve_order` on its own host build, which does not reach this crate. Without `unbounded_depth`, serde_json refuses nesting 128 levels deep or more, which bounds what one payload can make this reader do.

**`Cargo.lock`:** run one `cargo build -p ori-store` without `--locked` so cargo adds the two edges. Then check with `git diff Cargo.lock` that the only change is two lines in ori-store's `dependencies` list, with no package added, removed or re-versioned. Paste that diff into your report. Every later command uses `--locked`.

This is not a new escalation: E-0008's answer covers it, and the lead so rules for this ticket. Report it under "Dependencies" in your report.

## Tests

All new, named for what they hold, each beginning `ori_t_0189_`. **No existing test changes.**
- Event payloads in tests are Rust raw strings; build an exact escaped JSON text where a test needs one.
- Every event in these tests carries a ticket id (`Some(ticket)`), because `MissingTicketId` is checked first.

**In `payload.rs`'s test module** (add `use super::object;`):
1. `ori_t_0189_object_reads_top_level_strings_and_booleans_by_key`:
   - `{"a":"x","b":true,"c":false,"d":1,"e":null,"f":[],"g":{}}`;
   - `text("a")` is `Some("x")`; `text` of `b`, `d`, `e`, `f`, `g` and of an absent key is `None`;
   - `boolean("b")` is `Some(true)` and `boolean("c")` is `Some(false)`; `boolean("a")` is `None`.
2. `ori_t_0189_object_undoes_every_json_escape`: the payload `r#"{"q":"\"\\\/\n\t\r\b\f\u00e9\ud83d\ude00"}"#` gives `text("q") == Some("\"\\/\n\t\r\u{8}\u{c}\u{e9}\u{1F600}")`. Rust has no `\b` or `\f` escape, so the expected value is spelled with `\u{..}`.
3. `ori_t_0189_object_does_not_read_a_key_out_of_a_value`:
   - `{"question":"recommendation","escalation_id":"E1","recommendation":"r"}`: `text("recommendation")` is `Some("r")` (the old reader gives `"E1"`);
   - `{"note":"tier","x":"2","tier":"1"}`: `text("tier")` is `Some("1")` (the old reader gives `"2"`);
   - `{"a":"significant","b":true}`: `boolean("significant")` is `None` (the old `bool_field` gives `Some(true)`).
4. `ori_t_0189_object_refuses_what_is_not_one_object_with_distinct_keys`: `object(...)` is `None` for each of:
   - `""`, `not json`, `["E1","adr_area","q","r"]`, `"text"`, `1`, `true`, `null`;
   - `{"a":"1"} x`, `{"a":"1"}{"b":"2"}`;
   - `{"a":"1","a":"1"}` (same value), `{"a":"1","a":"2"}`, `{"a":"1","b":"2","a":3}`;
   - `{"a":"1",}`, `{a:"1"}`;
   - a string holding a raw line feed;
   - `{"a":"\ud800"}`;
   - `{"a":1e400}`.

   And `object(...)` is `Some` for `  {"a":"1"}  ` and for `{"a":{"b":"1","b":"2"}}` (the nested duplicate is not looked at).
5. `ori_t_0189_object_refuses_nesting_deeper_than_serde_jsons_bound`:
   - `{"a":` + 200 × `[` + 200 × `]` + `}` is `None`;
   - the same with 100 is `Some`.

**In `escalation.rs`'s test module:**

6. `ori_t_0189_raised_and_answered_keep_escaped_text_whole`:
   - `escalation.raised` with `question` = `Line one.\nShe said \"stop\" at C:\\tmp\tnow.` as JSON text, and `recommendation` = `a\r\nb`;
   - then `escalation.answered` with `answer` = `ok \"go\"`;
   - read the row; `question`, `recommendation` and `answer` are the unescaped strings, exactly.
7. `ori_t_0189_a_value_equal_to_a_key_name_is_not_read_as_that_key`: `escalation.raised` with `{"question":"recommendation","trigger":"question","escalation_id":"E1","recommendation":"stop"}` stores `trigger = question`, `question = recommendation`, `recommendation = stop` (the old reader gives `recommendation = question`).
8. `ori_t_0189_keys_this_projection_does_not_read_are_ignored_wherever_they_sit`: `escalation.raised` with an `evidence` array of two `{what, found_at}` objects placed **first**, then the four keys, projects the row as if `evidence` were absent.
9. `ori_t_0189_an_escalation_payload_that_is_not_one_object_is_refused_and_writes_nothing`:
   - for each payload in [`not json`, `["E1","adr_area","q","r"]`, a `raised` payload with `question` twice, a `raised` payload followed by ` x`], `apply` gives `UnreadablePayload { seq }` with the event's own seq;
   - `proj_escalations` holds no row after each;
   - `escalation.answered` with `not json` gives `UnreadablePayload` too;
   - an unknown `escalation.withdrawn` with `not json` is applied with `Ok`, and `proj_escalations` holds no row.
10. `ori_t_0189_a_field_of_the_wrong_type_is_named`: `{"escalation_id":"E1","trigger":"t","question":5,"recommendation":"r"}` gives `MalformedPayload { field: "question", .. }`, and no row.

**In `ticket.rs`'s test module:**

11. `ori_t_0189_ticket_fields_are_read_as_json`:
    - `ticket.filed` with `{"note":"category","kind":"feature","category":"auto"}` stores `category = auto` and `kind = feature` (the old reader gives `category = feature`);
    - `ticket.categorized` with `{"note":"category","x":"auto","category":"behavioral"}` stores `category = behavioral` (the old reader gives `auto`);
    - `ticket.category_set` with `{"note":"category","x":"auto","category":"decisional"}` stores `category = decisional` (the old reader gives `auto`);
    - `ticket.tier_set` with `{"note":"tier","x":"2","tier":"1"}` stores `tier = 1` (the old reader gives `2`);
    - `ticket.merged` with `{"a":"significant","b":true,"significant" : false}` stores `significant` as `false` (the old reader gives `true`). Read it back the way the existing ORI-T-0025 merged test reads it.
12. `ori_t_0189_a_kind_that_reads_no_field_accepts_any_payload`:
    - after a `ticket.filed`, `ticket.queued` with payload `not json` is applied with `Ok` and the state is `Queued`;
    - `ticket.closed` with payload `["x"]` is applied with `Ok` and the state is `Closed`;
    - an unknown `ticket.reopened` with `not json` is `Ok` and changes nothing.
13. `ori_t_0189_a_ticket_payload_that_is_not_one_object_is_refused`:
    - `ticket.filed` with `{"category":"auto","kind":"defect","kind":"chore"}` gives `UnreadablePayload` and writes no row;
    - `ticket.merged` with `true` gives `UnreadablePayload`;
    - `ticket.merged` with `{"significant":"true"}` gives `MalformedPayload { field: "significant", .. }`.

**In `mod.rs`'s test module:**

14. `ori_t_0189_an_unreadable_payload_is_a_refusal_under_aicd_8`:
    - `is_refusal()` is true;
    - `methodology_ref()` is section 8, no subsection;
    - `to_string()` contains `seq 7` and `JSON object`.

**No rebuild test is added:** `rebuild.rs` is outside the declared scope. Every test above goes through `apply`, which `rebuild` and the incremental path share.

## Mutation proofs

Run each in a `git archive` copy under your worktree's `target/`, with its own `CARGO_TARGET_DIR`. Edit the copy in place for each mutation and undo it afterwards: never re-extract over a built copy, because the extracted files' old mtimes make cargo skip the rebuild. Delete the copy when done.

| # | Mutation | Must fail |
|---|---|---|
| m1 | `visit_map` inserts without the duplicate check | tests 4, 9, 13 |
| m2 | `object` deserializes `serde_json::Map<String, Value>` instead of `Object` | tests 4 (duplicates), 9, 13 |
| m3 | escalation's `raised` reads `question` through `payload::field` | test 6 |
| m4 | ticket parses the payload once before the `match`, for every kind | test 12 |
| m4e | escalation parses the payload once before the `match`, for every suffix | test 9 (the unknown kind) |
| m5 | `payload_object` (escalation) maps `None` to `MalformedPayload { field: "escalation_id" }` | test 9 |
| m6 | in the copy, add a temporary `pub(crate) fn has(&self, key: &str) -> bool { self.fields.contains_key(key) }` to `Object`; make escalation's `required_field` return `UnreadablePayload` when `object.has(key) && object.text(key).is_none()` | test 10 |
| m7 | `UnreadablePayload` is left out of `is_refusal` | test 14 |
| m8 | ticket's `merged` reads `significant` with the old `bool_field` body (first `"significant"` in the text, then `true`/`false` after the next `:`), inlined | test 11 |
| m9 | ticket's `tier_set` reads `tier` through `payload::field` | test 11 |

If a mutation cannot be written as stated, write the nearest one that tests the same thing and say so.

## Checks

- The commands in the **Gates.** bullet of "Standing facts" below, the Windows compile check included.
- `cargo test -p ori-store --locked` and `cargo test -p ori-memory --locked`, each run twice. ori-memory's lock tests read ori-store's lock projection, which this ticket must leave exactly as it was.
- `cargo test -p ori-store --doc --locked`.
- Gate 13 against `<BASE>`.
- `git diff <BASE> -- crates/ori-store/src/projections/lock.rs crates/ori-store/src/event_log.rs crates/ori-store/migrations` is empty.
- Every test function that exists at `<BASE>` is byte-identical on your head. Compare the bodies with a script of your own, and report the command and its result.

Report every exit status as a number. List what is not tested and why:
- a real writer (ORI-T-0070);
- a rebuild, end to end, of a log holding an unreadable payload (`rebuild.rs` is outside this scope, and `apply` is the shared path).
