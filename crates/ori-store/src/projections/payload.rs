//! Reading fields out of an event's `payload`: a JSON reader for the escalation
//! and ticket projections, and the first-key reader the lock projection keeps.
//!
//! The event log stores and hashes `payload` as opaque, non-empty text and
//! never interprets it (`crates/ori-store/src/event_log.rs`'s module doc);
//! reading one is the projections' job. This module has two readers.
//!
//! - [`object`] parses the payload as JSON, through `serde_json` (approved
//!   workspace-wide by `ops/escalations/E-0008-json-serialization.md`), and
//!   gives back its top-level keys. The escalation and ticket projections use
//!   it, because the text they read, an escalation's question and answer above
//!   all, is to be written as JSON (S9's desk contract fixes serde for
//!   `escalation.raised`; ORI-T-0070 writes the events), and a JSON writer
//!   escapes `"`, `\` and every control character below U+0020: a reader that
//!   undoes none of that hands the human queue a cut-short or doubled-backslash
//!   question.
//! - [`field`] is the first-key reader of ORI-T-0025: it finds the first
//!   `"key"` in the text and cuts the value at the next `"`. Only the lock
//!   projection (`crate::projections::lock`) uses it.
//!
//! # Why the lock projection keeps `field`
//!
//! `ori-memory`'s lock fold reads `module` out of a `lock.claimed` payload
//! through its own copy of `field`, and a tier 2 test there holds that fold to
//! the same rows the lock projection keeps. A lock projection that read
//! `module` as JSON would fail that test, and so would a copy moved along with
//! it; changing either is a `test_modified` escalation and a tier 2 change.
//! The desk can write no `module` value on which the two readers differ
//! (`ModulePath::parse` refuses a backslash and a control character, and the
//! plan tool's handler in `ori-mcp` refuses a `"`), so nothing is wrong in the
//! meantime. Whether to move the lock projection and `ori-memory`'s copy
//! together is ORI-T-0193, a decision for the operator.
//!
//! No methodology section applies to the reader itself: it is a parsing
//! convenience, not a control. `crates/ori-store/src/projections/mod.rs`'s
//! module doc cites the sections that govern what callers do with what this
//! reads.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use serde::Deserializer;
use serde::de::Error as _;
use serde::de::MapAccess;
use serde::de::Visitor;
use serde_json::Value;

/// One event payload read as a JSON object: the top-level keys and their
/// values, each key once.
///
/// Built by [`object`] and nowhere else. [`Object::text`] and
/// [`Object::boolean`] read a key as the JSON type it was written as, and read
/// nothing out of another key's value: a value that spells a key name is only
/// a value.
///
/// # Why a duplicate key is refused
///
/// Two values for one key have no single reading. A first-wins reader (`field`,
/// and every reader like it) and a last-wins reader (`serde_json::Value`) would
/// disagree on the same row, and refusing is the only rule that cannot
/// disagree with another reader. The cost is that such an event blocks every
/// rebuild of its product (`crate::rebuild::rebuild` stops at the first
/// refusal). No writer contract written so far would produce one:
///
/// - the one contract that fixes how such a payload is serialized, S9's for
///   `escalation.raised` (`crates/ori-mcp/src/ports.rs`), serializes a struct
///   through serde, which writes each field once;
/// - the other contracts name a row and leave its payload open: S5's
///   `ticket.category_set` (`crates/ori-mcp/src/tools/desk.rs`), and S9's
///   `ticket.escalated`, whose payload no projection reads. ORI-T-0070, which
///   writes them, must write each key once;
/// - a `#[serde(flatten)]` map, two fields renamed onto one name, or a
///   hand-written `Serialize` could repeat a key, and S9's contract uses none
///   of them.
///
/// `ori-memory`'s `LockClaimWire` likewise refuses a `lock.claimed` payload
/// that gives `module` or `session_id` twice, through serde's derive.
///
/// Only the top level is checked for a repeated key: a duplicate inside a
/// nested value (S9's `evidence` array, say) is that value's own business and
/// is not looked at.
///
/// `ProjectionError::UnreadablePayload` is the refusal a projection gives when
/// [`object`] returns `None`.
#[derive(Debug)]
pub(crate) struct Object {
    fields: BTreeMap<String, Value>,
}

/// Reads `payload` as one JSON object whose top-level keys are distinct, or
/// `None` when it is not one.
///
/// Refused by `serde_json` as pinned in `crates/ori-store/Cargo.toml`:
///
/// - text that is not JSON;
/// - JSON whose top level is not an object: an array, a string, a number, a
///   boolean or `null`. `serde_json`'s `deserialize_map` reports an invalid
///   type, using the visitor's `expecting` text and never its `visit_map`: it
///   reads a literal, a number or a string to report it, and refuses an array
///   on its `[` alone, so a defect inside the array is never reached. A
///   top-level number too large for `f64`, or a top-level string with a lone
///   surrogate escape, is refused while it is read, as below;
/// - an object followed by anything but JSON whitespace;
/// - nesting 128 levels deep or more, counting the top object. The pin leaves
///   `serde_json`'s `unbounded_depth` off, which bounds how deep the parser
///   recurses; it does not bound a payload's size, and a flat payload is
///   parsed whole, with every top-level value kept;
/// - a raw control character inside a string, a lone surrogate escape, and a
///   number too large for `f64` (an underflow such as `1e-400` reads as 0).
///
/// Refused by this module's visitor: a key given twice at the top level,
/// whatever the two values.
///
/// Accepted: every other top-level object with distinct keys, nested values
/// and keys the caller does not read included, with JSON
/// whitespace (space, tab, line feed, carriage return) around the object and
/// between tokens. Any other character around it, a byte order mark or a
/// no-break space included, is refused.
///
/// A limit: `serde_json` writes a value of any depth, so a payload that
/// carried free-form JSON nested 128 levels deep or more, counting the top
/// object, would be written and then refused here. No writer contract carries
/// one: S9's `evidence` makes a payload three levels deep, counting the top
/// object.
#[must_use]
pub(crate) fn object(payload: &str) -> Option<Object> {
    serde_json::from_str::<Object>(payload).ok()
}

impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ObjectVisitor)
    }
}

/// Builds an [`Object`] from a JSON map, refusing the first key it sees twice.
struct ObjectVisitor;

impl<'de> Visitor<'de> for ObjectVisitor {
    type Value = Object;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one JSON object whose keys are distinct")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Object, A::Error> {
        let mut fields = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            // The repeat is refused before its value is read, so the second
            // value never has to be parsed to be turned away.
            if fields.contains_key(&key) {
                return Err(A::Error::custom("a key is given twice"));
            }
            let value = map.next_value::<Value>()?;
            fields.insert(key, value);
        }
        Ok(Object { fields })
    }
}

impl Object {
    /// The value of `key` when it is a JSON string, with every escape undone;
    /// `None` for an absent key and for a number, boolean, `null`, array or
    /// object.
    #[must_use]
    pub(crate) fn text(&self, key: &str) -> Option<&str> {
        self.fields.get(key).and_then(Value::as_str)
    }

    /// The value of `key` when it is a JSON boolean; `None` for an absent key
    /// and for any other type, a string `"true"` included.
    #[must_use]
    pub(crate) fn boolean(&self, key: &str) -> Option<bool> {
        self.fields.get(key).and_then(Value::as_bool)
    }
}

/// Reads one string-valued field, `"key":"value"`, out of a flat JSON-shaped
/// object, wherever it appears in `payload`.
///
/// Not a JSON parser (see the module doc). The needle is `"key"` with both
/// quotes, so a key that is a prefix of another (`"tier"` inside
/// `"tier_set"`) cannot match: the character right after the needle in
/// `"tier_set"` is `_`, not the closing quote the needle itself supplies.
/// Returns `None` for an absent key, a key whose value is not a quoted
/// string, or an unterminated value.
///
/// It cuts a value at the next `"` and undoes no escape, so a value that holds
/// a `"` cannot be represented, and a backslash sequence comes back as the
/// backslash and the characters after it. It also takes the first `"key"` it
/// finds anywhere in the text, values included. Only
/// `crate::projections::lock` calls it, for the reason in the module doc, "Why
/// the lock projection keeps `field`".
#[must_use]
pub(crate) fn field<'a>(payload: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let start = payload.find(needle.as_str())?;
    let after_key = &payload[start + needle.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    let after_open_quote = after_colon.strip_prefix('"')?;
    let end = after_open_quote.find('"')?;
    Some(&after_open_quote[..end])
}

/// Reads one boolean-valued field, `"key":true` or `"key":false` (unquoted,
/// JSON's own spelling), out of a payload that is one JSON object.
///
/// `None` for an absent key, a value that is not a JSON boolean, and a payload
/// that [`object`] refuses.
///
/// Kept, test-only, for its two ORI-T-0025 tests, which `CLAUDE.md` rule 3
/// forbids changing: no projection calls it any more, because the ticket
/// projection reads `significant` through [`Object::boolean`].
#[cfg(test)]
#[must_use]
pub(crate) fn bool_field(payload: &str, key: &str) -> Option<bool> {
    object(payload)?.boolean(key)
}

#[cfg(test)]
mod tests {
    use super::bool_field;
    use super::field;
    use super::object;

    #[test]
    fn ori_t_0025_field_reads_a_simple_string_value() {
        assert_eq!(
            field(r#"{"category":"auto","kind":"defect"}"#, "category"),
            Some("auto")
        );
        assert_eq!(
            field(r#"{"category":"auto","kind":"defect"}"#, "kind"),
            Some("defect")
        );
    }

    #[test]
    fn ori_t_0025_field_is_none_for_an_absent_key() {
        assert_eq!(field(r#"{"category":"auto"}"#, "tier"), None);
        assert_eq!(field("{}", "category"), None);
        assert_eq!(field("", "category"), None);
    }

    #[test]
    fn ori_t_0025_field_does_not_confuse_a_key_with_a_key_it_prefixes() {
        // "tier" must not match inside "tier_set": the byte right after
        // "tier" in the source is '_', not the closing quote the needle
        // requires.
        assert_eq!(field(r#"{"tier_set":"1"}"#, "tier"), None);
        assert_eq!(field(r#"{"tier":"1","tier_set":"2"}"#, "tier"), Some("1"));
        assert_eq!(
            field(r#"{"tier":"1","tier_set":"2"}"#, "tier_set"),
            Some("2")
        );
    }

    #[test]
    fn ori_t_0025_field_tolerates_whitespace_after_the_colon() {
        assert_eq!(field(r#"{"category":   "auto"}"#, "category"), Some("auto"));
    }

    #[test]
    fn ori_t_0025_field_finds_the_key_wherever_it_sits_in_the_object() {
        assert_eq!(
            field(r#"{"a":"1","category":"auto","b":"2"}"#, "category"),
            Some("auto")
        );
    }

    #[test]
    fn ori_t_0025_field_is_none_when_the_value_is_not_a_quoted_string() {
        assert_eq!(field(r#"{"significant":true}"#, "significant"), None);
        assert_eq!(field(r#"{"category":}"#, "category"), None);
        assert_eq!(field(r#"{"category":"unterminated}"#, "category"), None);
    }

    #[test]
    fn ori_t_0025_bool_field_reads_true_and_false() {
        assert_eq!(
            bool_field(r#"{"significant":true}"#, "significant"),
            Some(true)
        );
        assert_eq!(
            bool_field(r#"{"significant":false}"#, "significant"),
            Some(false)
        );
    }

    #[test]
    fn ori_t_0025_bool_field_is_none_for_absent_or_non_boolean() {
        assert_eq!(bool_field("{}", "significant"), None);
        assert_eq!(bool_field(r#"{"significant":"true"}"#, "significant"), None);
    }

    #[test]
    fn ori_t_0189_object_reads_top_level_strings_and_booleans_by_key() {
        let read = object(r#"{"a":"x","b":true,"c":false,"d":1,"e":null,"f":[],"g":{}}"#)
            .expect("one object with distinct keys");
        assert_eq!(read.text("a"), Some("x"));
        for key in ["b", "d", "e", "f", "g", "absent"] {
            assert_eq!(read.text(key), None, "text({key:?}) is not a JSON string");
        }
        assert_eq!(read.boolean("b"), Some(true));
        assert_eq!(read.boolean("c"), Some(false));
        assert_eq!(read.boolean("a"), None);
    }

    #[test]
    fn ori_t_0189_object_undoes_every_json_escape() {
        // The three `\u` escapes (U+00E9, then a surrogate pair that makes
        // U+1F600) are spelled with `\x5c`, a backslash, in a plain string: the
        // compiler turns each `\x5c` into a backslash, so no tool on the way can
        // decode a typed escape early, and `object` decodes them at run time.
        let payload = concat!(
            r#"{"q":"\"\\\/\n\t\r\b\f"#,
            "\x5cu00e9\x5cud83d\x5cude00",
            r#""}"#
        );
        let read = object(payload).expect("an object");
        // Rust has no `\b` or `\f` escape, so those two are spelled `\u{..}`.
        assert_eq!(read.text("q"), Some("\"\\/\n\t\r\u{8}\u{c}\u{e9}\u{1F600}"));
    }

    #[test]
    fn ori_t_0189_object_does_not_read_a_key_out_of_a_value() {
        // The first-key reader gives `recommendation` the value "E1" here.
        let read =
            object(r#"{"question":"recommendation","escalation_id":"E1","recommendation":"r"}"#)
                .expect("an object");
        assert_eq!(read.text("recommendation"), Some("r"));
        assert_eq!(read.text("question"), Some("recommendation"));

        // The first-key reader gives `tier` the value "2" here.
        let read = object(r#"{"note":"tier","x":"2","tier":"1"}"#).expect("an object");
        assert_eq!(read.text("tier"), Some("1"));

        // The first-key boolean reader gives `significant` `Some(true)` here.
        let read = object(r#"{"a":"significant","b":true}"#).expect("an object");
        assert_eq!(read.boolean("significant"), None);
    }

    #[test]
    fn ori_t_0189_object_refuses_what_is_not_one_object_with_distinct_keys() {
        let refused = [
            "",
            "not json",
            r#"["E1","adr_area","q","r"]"#,
            r#""text""#,
            "1",
            "true",
            "null",
            r#"{"a":"1"} x"#,
            r#"{"a":"1"}{"b":"2"}"#,
            r#"{"a":"1","a":"1"}"#,
            r#"{"a":"1","a":"2"}"#,
            r#"{"a":"1","b":"2","a":3}"#,
            r#"{"a":"1",}"#,
            r#"{a:"1"}"#,
            "{\"a\":\"x\ny\"}",
            r#"{"a":"\ud800"}"#,
            r#"{"a":1e400}"#,
            // A form feed, a no-break space and a byte order mark are not JSON
            // whitespace.
            "\x0c{\"a\":\"1\"}",
            "{\"a\":\"1\"}\u{a0}",
            "\u{feff}{\"a\":\"1\"}",
        ];
        for payload in refused {
            assert!(
                object(payload).is_none(),
                "{payload:?} must be refused, got {:?}",
                object(payload)
            );
        }

        assert!(
            object(r#"  {"a":"1"}  "#).is_some(),
            "whitespace around the object is accepted"
        );
        assert!(
            object("\t{\"a\":\"1\"}\r\n").is_some(),
            "tab, carriage return and line feed around the object are JSON whitespace"
        );
        assert!(
            object(r#"{"a":{"b":"1","b":"2"}}"#).is_some(),
            "a duplicate inside a nested value is that value's own business"
        );
    }

    #[test]
    fn ori_t_0189_object_refuses_nesting_deeper_than_serde_jsons_bound() {
        let nested = |depth: usize| format!("{{\"a\":{}{}}}", "[".repeat(depth), "]".repeat(depth));
        assert!(
            object(&nested(200)).is_none(),
            "200 levels exceed serde_json's recursion bound"
        );
        assert!(object(&nested(100)).is_some(), "100 levels are accepted");
    }
}
