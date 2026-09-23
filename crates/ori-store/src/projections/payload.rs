//! Reading one flat field out of an event's `payload`, without a JSON parser.
//!
//! `crates/ori-store/src/event_log.rs`'s own module doc, "What this module
//! deliberately does not enforce", already states the reason this crate has no
//! JSON library: `payload` is stored and hashed as opaque, non-empty text,
//! "interpreting JSON would need a parser this crate does not have ... and a
//! grammar `spec/DATA_MODEL.md` does not draw". `crates/ori-store/Cargo.toml`
//! lists exactly three dependencies, `rusqlite`, `sha2`, and the dev-only
//! `proptest`; adding a fourth is `CLAUDE.md` absolute rule 6, an escalation
//! this ticket does not raise because it does not need to (see the pull
//! request report, "Dependencies").
//!
//! What the three projectors in this module's parent need from a payload is
//! narrower than JSON: one flat, single-level object of string-valued fields,
//! a convention this ticket defines for its own three event kinds because
//! nothing in the workspace writes these events yet (`crates/ori-orchestrator`'s
//! `Ticket::apply`, `LockTable` and `Escalation` are pure functions over their
//! own types today, wired to no writer; see the pull request report, "Whether
//! this payload convention is load-bearing elsewhere"). [`field`] and
//! [`bool_field`] read exactly that shape and nothing more: a value containing
//! a literal `"` cannot be represented, and every payload this module's own
//! tests and the parent's projectors write never needs one.
//!
//! No methodology section applies to the reader itself: it is a parsing
//! convenience, not a control. `crates/ori-store/src/projections/mod.rs`'s
//! module doc cites the sections that govern what callers do with what this
//! reads.

/// Reads one string-valued field, `"key":"value"`, out of a flat JSON-shaped
/// object, wherever it appears in `payload`.
///
/// Not a JSON parser (see the module doc). The needle is `"key"` with both
/// quotes, so a key that is a prefix of another (`"tier"` inside
/// `"tier_set"`) cannot match: the character right after the needle in
/// `"tier_set"` is `_`, not the closing quote the needle itself supplies.
/// Returns `None` for an absent key, a key whose value is not a quoted
/// string, or an unterminated value.
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
/// JSON's own spelling), out of a flat JSON-shaped object.
///
/// `None` for an absent key or a value that is neither literal.
#[must_use]
pub(crate) fn bool_field(payload: &str, key: &str) -> Option<bool> {
    let needle = format!("\"{key}\"");
    let start = payload.find(needle.as_str())?;
    let after_key = &payload[start + needle.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    if after_colon.starts_with("true") {
        Some(true)
    } else if after_colon.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::bool_field;
    use super::field;

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
}
