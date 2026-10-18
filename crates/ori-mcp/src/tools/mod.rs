//! The tool catalogue, the schemas that describe and check arguments, and the
//! seam a tool's handler plugs into: AICD §17, AICD §27.
//!
//! # What is here
//!
//! - **The catalogue** ([`crate::tools::catalogue`]): all eleven of Ori's own
//!   tools (`spec/API_SPEC.md` section 3), each with a title, a description, an
//!   input schema and annotations, so that no later slice edits this file to
//!   describe its tool. The tools built in phase 1 are listed to an agent; the
//!   three that are not ([`crate::scopes::Tool::in_phase_1`]) are described
//!   here and never listed or dispatched.
//! - **The schema tree** ([`crate::tools::Schema`]): one typed tree of nodes
//!   that generates **both** the JSON Schema a client reads
//!   ([`crate::tools::Schema::to_json`]) and the validator the server runs
//!   ([`crate::tools::Schema::validate`]), so what a tool publishes and what it
//!   enforces are one source of truth. A test walks every published schema and
//!   fails on a keyword that is not one the validator enforces.
//! - **The handler seam** ([`crate::tools::ToolHandler`],
//!   [`crate::tools::Handlers`]): what slices S3 to S9 implement. A handler is
//!   called only after every control of the server's pipeline passed and
//!   `mcp.tool_called` was appended, so no effect happens without its record
//!   (`spec/API_SPEC.md` section 3).
//!
//! ```mermaid
//! flowchart LR
//!   tree["Schema tree: one node type"] --> json["to_json: the inputSchema a client reads"]
//!   tree --> check["validate: the check 6 the server runs"]
//!   tree --> bytes["worst_case_bytes: proves the byte cap holds"]
//!   check --> canon["id-typed values rewritten to canonical form"]
//!   canon --> handler["ToolHandler::call receives the canonical arguments"]
//! ```
//!
//! # The byte cap is part of the contract
//!
//! `crate::server::ServerConfig::max_arguments_bytes` defaults to 512 KiB
//! (524 288 bytes of serialized arguments), which stays below
//! [`crate::protocol::MAX_LINE_BYTES`] (1 MiB) minus the request envelope. Every
//! maximum in this catalogue was chosen so that the **largest schema-valid
//! argument object of every tool fits under that default**, counted at 6 bytes
//! per character, which is what `serde_json` writes for a control character
//! (`\u00XX`). [`crate::tools::Schema::worst_case_bytes`] computes that figure
//! from the tree, keys and punctuation included, and a test pins it for every
//! tool. A schema-valid call is therefore never refused for its size, and the
//! cap is checked before the schema so that an oversized object is never walked.
//!
//! # What a published schema says, and what the server enforces
//!
//! The same, with one stated exception: an `id` node is published as a string of
//! exactly 26 characters, and the validator also requires
//! [`ori_core::types::Id::parse`] to accept it (Crockford base 32, first symbol
//! at most `7`). The published schema says less than the server enforces, and a
//! client that satisfies the published schema alone may still be refused.
//!
//! Lengths are counted in characters (`chars().count()`), as JSON Schema counts
//! them. `description` and `title` are annotations, never constraints.
//!
//! # Tier
//!
//! Tier 2: `spec/RISK_MAP.md` lists `crates/ori-mcp/src/tools/` (a directory
//! module, so that gate 12 tiers it), "What agents can do".

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::sync::OnceLock;

use ori_core::error::MethodologyRef;
use ori_core::types::{Category, Id, TicketKind, Timestamp};
use ori_store::db::ProductDb;
use serde_json::Value;

use crate::ports::PortError;
use crate::protocol::Tool as ToolDefinition;
use crate::protocol::{CallToolResult, JsonObject, ToolAnnotations};
use crate::scopes::Tool;
use crate::server::Binding;

/// The length of an identifier in characters: a ULID.
const ID_LENGTH: usize = 26;

/// The most bytes a character takes once serialized: `serde_json` writes a
/// control character as `\u00XX`, six bytes, and every other character in at
/// most four.
const WORST_BYTES_PER_CHARACTER: usize = 6;

/// The name of the argument that names a ticket, in every tool that has one.
pub const TICKET_ARGUMENT: &str = "ticket_id";

// ---------------------------------------------------------------------------
// The schema tree
// ---------------------------------------------------------------------------

/// Why an argument object is not valid, as the first violation the validator
/// found: AICD §27.
///
/// `field` is an RFC 6901 JSON pointer (`""` is the root) built **only from
/// schema property names and array indices**. It stops at the last segment that
/// is a schema property name: a violation inside a map value reports the map's
/// pointer, an unexpected key reports the pointer of the object that contains
/// it, and an agent-chosen key is never in a pointer. `why` is a fixed string.
/// Neither holds text the agent wrote.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    /// The JSON pointer of the node that failed.
    pub field: String,
    /// A fixed description of what is wrong with it.
    pub why: &'static str,
}

/// The `why` of a value of the wrong JSON type for an object node.
pub const WHY_NOT_AN_OBJECT: &str = "must be an object";
/// The `why` of a value of the wrong JSON type for a string, enum or id node.
pub const WHY_NOT_A_STRING: &str = "must be a string";
/// The `why` of a value of the wrong JSON type for an array node.
pub const WHY_NOT_AN_ARRAY: &str = "must be an array";
/// The `why` of an object that holds a key its schema does not list.
pub const WHY_UNEXPECTED_PROPERTY: &str = "unexpected property";
/// The `why` of an object that lacks a property its schema requires.
pub const WHY_MISSING_PROPERTY: &str = "required property missing";
/// The `why` of a string shorter than its `minLength`.
pub const WHY_TOO_SHORT: &str = "too short";
/// The `why` of a string longer than its `maxLength`.
pub const WHY_TOO_LONG: &str = "too long";
/// The `why` of a string that is not one of an enum's values.
pub const WHY_NOT_AN_ALLOWED_VALUE: &str = "not an allowed value";
/// The `why` of a string of the right length that is not an identifier.
pub const WHY_NOT_AN_IDENTIFIER: &str = "not a valid identifier";
/// The `why` of an array with fewer items than its `minItems`.
pub const WHY_TOO_FEW_ITEMS: &str = "too few items";
/// The `why` of an array with more items than its `maxItems`.
pub const WHY_TOO_MANY_ITEMS: &str = "too many items";
/// The `why` of a map with more properties than its `maxProperties`.
pub const WHY_TOO_MANY_PROPERTIES: &str = "too many properties";
/// The `why` of a map key shorter than one character.
pub const WHY_PROPERTY_NAME_TOO_SHORT: &str = "property name too short";
/// The `why` of a map key longer than its `propertyNames` `maxLength`.
pub const WHY_PROPERTY_NAME_TOO_LONG: &str = "property name too long";

/// One property of an object node: its name, whether it is required, and its
/// schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Property {
    /// The property's name, as the tool's signature in `spec/API_SPEC.md`
    /// section 3 spells it.
    pub name: &'static str,
    /// Whether an object without it is refused.
    pub required: bool,
    /// The property's schema.
    pub schema: Schema,
}

impl Property {
    /// A property an object must carry.
    #[must_use]
    pub const fn required(name: &'static str, schema: Schema) -> Self {
        Self {
            name,
            required: true,
            schema,
        }
    }

    /// A property an object may omit.
    #[must_use]
    pub const fn optional(name: &'static str, schema: Schema) -> Self {
        Self {
            name,
            required: false,
            schema,
        }
    }
}

/// What a node of the schema tree constrains: the six kinds of node the
/// catalogue needs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaKind {
    /// An object with named properties and `additionalProperties: false`, at
    /// every depth.
    Object {
        /// The properties, in the order they are published and validated.
        properties: Vec<Property>,
    },
    /// A string with `minLength` and `maxLength`, in characters.
    String {
        /// The fewest characters.
        min_length: usize,
        /// The most characters.
        max_length: usize,
    },
    /// A string that is one of a fixed list. Every value is plain ASCII, so it
    /// serializes to its own length plus two quotes.
    Enum {
        /// The allowed values.
        values: Vec<&'static str>,
    },
    /// An identifier: a ULID. **Published** as a string of exactly 26
    /// characters, and the validator **also** requires
    /// [`ori_core::types::Id::parse`] to accept it, then rewrites the value to
    /// its canonical upper-case form. The published schema therefore says less
    /// than the server enforces.
    Id,
    /// An array of one kind of item, with `minItems` and `maxItems`.
    Array {
        /// The schema of every item.
        items: Box<Schema>,
        /// The fewest items.
        min_items: usize,
        /// The most items.
        max_items: usize,
    },
    /// An object whose **keys are chosen by the agent**: published with
    /// `propertyNames` (`minLength` 1, `maxLength` `key_max_length`), a value
    /// schema as `additionalProperties`, and `maxProperties`. A violation inside
    /// it reports the map's own pointer, never a key.
    Map {
        /// The most characters in a key. A key has at least one.
        key_max_length: usize,
        /// The schema of every value.
        value: Box<Schema>,
        /// The most properties.
        max_properties: usize,
    },
}

/// One node of the typed schema tree: AICD §27.
///
/// The tree is the one source of truth for a tool's arguments. It generates the
/// JSON a client reads ([`Schema::to_json`]), the validator the server runs
/// ([`Schema::validate`]) and the worst-case serialized size
/// ([`Schema::worst_case_bytes`]). A node may carry a `description`, and the
/// root a `title`; they are annotations, never constraints.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Schema {
    title: Option<&'static str>,
    description: Option<&'static str>,
    kind: SchemaKind,
}

impl Schema {
    const fn of(kind: SchemaKind) -> Self {
        Self {
            title: None,
            description: None,
            kind,
        }
    }

    /// An object node with `additionalProperties: false`.
    #[must_use]
    pub const fn object(properties: Vec<Property>) -> Self {
        Self::of(SchemaKind::Object { properties })
    }

    /// A string node bounded in characters.
    #[must_use]
    pub const fn string(min_length: usize, max_length: usize) -> Self {
        Self::of(SchemaKind::String {
            min_length,
            max_length,
        })
    }

    /// An enum node of strings.
    #[must_use]
    pub const fn enumeration(values: Vec<&'static str>) -> Self {
        Self::of(SchemaKind::Enum { values })
    }

    /// An identifier node; see [`SchemaKind::Id`].
    #[must_use]
    pub const fn id() -> Self {
        Self::of(SchemaKind::Id)
    }

    /// An array node.
    #[must_use]
    pub fn array(items: Self, min_items: usize, max_items: usize) -> Self {
        Self::of(SchemaKind::Array {
            items: Box::new(items),
            min_items,
            max_items,
        })
    }

    /// A map node: an object with agent-chosen keys.
    #[must_use]
    pub fn map(key_max_length: usize, value: Self, max_properties: usize) -> Self {
        Self::of(SchemaKind::Map {
            key_max_length,
            value: Box::new(value),
            max_properties,
        })
    }

    /// The same node with a `description` annotation.
    #[must_use]
    pub fn described(mut self, description: &'static str) -> Self {
        self.description = Some(description);
        self
    }

    /// The same node with a `title` annotation (the root's).
    #[must_use]
    pub fn titled(mut self, title: &'static str) -> Self {
        self.title = Some(title);
        self
    }

    /// What the node constrains.
    #[must_use]
    pub const fn kind(&self) -> &SchemaKind {
        &self.kind
    }

    /// The schema of an object node's property called `name`, or `None` for any
    /// other node or a name it does not list.
    #[must_use]
    pub fn property(&self, name: &str) -> Option<&Schema> {
        match &self.kind {
            SchemaKind::Object { properties } => properties
                .iter()
                .find(|property| property.name == name)
                .map(|property| &property.schema),
            SchemaKind::String { .. }
            | SchemaKind::Enum { .. }
            | SchemaKind::Id
            | SchemaKind::Array { .. }
            | SchemaKind::Map { .. } => None,
        }
    }

    /// The JSON Schema this node publishes.
    ///
    /// The only keywords it writes are `type`, `properties`, `required`,
    /// `additionalProperties`, `minLength`, `maxLength`, `enum`, `items`,
    /// `minItems`, `maxItems`, `propertyNames` and `maxProperties`, which the
    /// validator enforces, and the annotations `description` and `title`.
    #[must_use]
    pub fn to_json(&self) -> JsonObject {
        let mut out = JsonObject::new();
        match &self.kind {
            SchemaKind::Object { properties } => {
                out.insert("type".to_owned(), Value::from("object"));
                let mut published = JsonObject::new();
                let mut required = Vec::new();
                for property in properties {
                    published.insert(
                        property.name.to_owned(),
                        Value::Object(property.schema.to_json()),
                    );
                    if property.required {
                        required.push(Value::from(property.name));
                    }
                }
                out.insert("properties".to_owned(), Value::Object(published));
                if !required.is_empty() {
                    out.insert("required".to_owned(), Value::Array(required));
                }
                out.insert("additionalProperties".to_owned(), Value::Bool(false));
            }
            SchemaKind::String {
                min_length,
                max_length,
            } => {
                out.insert("type".to_owned(), Value::from("string"));
                out.insert("minLength".to_owned(), Value::from(*min_length));
                out.insert("maxLength".to_owned(), Value::from(*max_length));
            }
            SchemaKind::Enum { values } => {
                out.insert("type".to_owned(), Value::from("string"));
                out.insert(
                    "enum".to_owned(),
                    Value::Array(values.iter().map(|value| Value::from(*value)).collect()),
                );
            }
            SchemaKind::Id => {
                out.insert("type".to_owned(), Value::from("string"));
                out.insert("minLength".to_owned(), Value::from(ID_LENGTH));
                out.insert("maxLength".to_owned(), Value::from(ID_LENGTH));
            }
            SchemaKind::Array {
                items,
                min_items,
                max_items,
            } => {
                out.insert("type".to_owned(), Value::from("array"));
                out.insert("items".to_owned(), Value::Object(items.to_json()));
                out.insert("minItems".to_owned(), Value::from(*min_items));
                out.insert("maxItems".to_owned(), Value::from(*max_items));
            }
            SchemaKind::Map {
                key_max_length,
                value,
                max_properties,
            } => {
                out.insert("type".to_owned(), Value::from("object"));
                let mut names = JsonObject::new();
                names.insert("minLength".to_owned(), Value::from(1_usize));
                names.insert("maxLength".to_owned(), Value::from(*key_max_length));
                out.insert("propertyNames".to_owned(), Value::Object(names));
                out.insert(
                    "additionalProperties".to_owned(),
                    Value::Object(value.to_json()),
                );
                out.insert("maxProperties".to_owned(), Value::from(*max_properties));
            }
        }
        if let Some(title) = self.title {
            out.insert("title".to_owned(), Value::from(title));
        }
        if let Some(description) = self.description {
            out.insert("description".to_owned(), Value::from(description));
        }
        out
    }

    /// Checks `value` against this node and returns the first violation.
    ///
    /// **Rewrites** every id-typed value it accepts to its canonical
    /// [`ori_core::types::Id::as_str`] form, so what a handler reads has one
    /// spelling. On a violation `value` may be partly rewritten and must be
    /// dropped.
    ///
    /// The order is fixed: an object is checked for its type, then for a key its
    /// schema does not list, then each schema property in schema order (a
    /// missing required property reports that property's own pointer); an array
    /// for its type, its length, then each item in order; a map for its type, its
    /// size, then every key, then every value.
    ///
    /// # Errors
    ///
    /// The first [`Violation`], whose pointer holds only schema property names
    /// and array indices.
    pub fn validate(&self, value: &mut Value) -> Result<(), Violation> {
        let mut pointer = Pointer::root();
        self.walk(value, &mut pointer)
    }

    fn walk(&self, value: &mut Value, pointer: &mut Pointer) -> Result<(), Violation> {
        match &self.kind {
            SchemaKind::Object { properties } => {
                let Value::Object(object) = value else {
                    return Err(pointer.violation(WHY_NOT_AN_OBJECT));
                };
                if object
                    .keys()
                    .any(|key| !properties.iter().any(|property| property.name == key))
                {
                    return Err(pointer.violation(WHY_UNEXPECTED_PROPERTY));
                }
                for property in properties {
                    let mark = pointer.push_name(property.name);
                    match object.get_mut(property.name) {
                        Some(inner) => property.schema.walk(inner, pointer)?,
                        None if property.required => {
                            return Err(pointer.violation(WHY_MISSING_PROPERTY));
                        }
                        None => {}
                    }
                    pointer.truncate(mark);
                }
                Ok(())
            }
            SchemaKind::String {
                min_length,
                max_length,
            } => {
                let Value::String(text) = value else {
                    return Err(pointer.violation(WHY_NOT_A_STRING));
                };
                let length = text.chars().count();
                if length < *min_length {
                    return Err(pointer.violation(WHY_TOO_SHORT));
                }
                if length > *max_length {
                    return Err(pointer.violation(WHY_TOO_LONG));
                }
                Ok(())
            }
            SchemaKind::Enum { values } => {
                let Value::String(text) = value else {
                    return Err(pointer.violation(WHY_NOT_A_STRING));
                };
                if values.contains(&text.as_str()) {
                    Ok(())
                } else {
                    Err(pointer.violation(WHY_NOT_AN_ALLOWED_VALUE))
                }
            }
            SchemaKind::Id => {
                let Value::String(text) = value else {
                    return Err(pointer.violation(WHY_NOT_A_STRING));
                };
                let length = text.chars().count();
                if length < ID_LENGTH {
                    return Err(pointer.violation(WHY_TOO_SHORT));
                }
                if length > ID_LENGTH {
                    return Err(pointer.violation(WHY_TOO_LONG));
                }
                match Id::parse(text) {
                    Ok(id) => {
                        id.as_str().clone_into(text);
                        Ok(())
                    }
                    Err(_) => Err(pointer.violation(WHY_NOT_AN_IDENTIFIER)),
                }
            }
            SchemaKind::Array {
                items,
                min_items,
                max_items,
            } => {
                let Value::Array(entries) = value else {
                    return Err(pointer.violation(WHY_NOT_AN_ARRAY));
                };
                if entries.len() < *min_items {
                    return Err(pointer.violation(WHY_TOO_FEW_ITEMS));
                }
                if entries.len() > *max_items {
                    return Err(pointer.violation(WHY_TOO_MANY_ITEMS));
                }
                for (index, entry) in entries.iter_mut().enumerate() {
                    let mark = pointer.push_index(index);
                    items.walk(entry, pointer)?;
                    pointer.truncate(mark);
                }
                Ok(())
            }
            SchemaKind::Map {
                key_max_length,
                value: value_schema,
                max_properties,
            } => {
                let Value::Object(object) = value else {
                    return Err(pointer.violation(WHY_NOT_AN_OBJECT));
                };
                if object.len() > *max_properties {
                    return Err(pointer.violation(WHY_TOO_MANY_PROPERTIES));
                }
                for key in object.keys() {
                    let length = key.chars().count();
                    if length < 1 {
                        return Err(pointer.violation(WHY_PROPERTY_NAME_TOO_SHORT));
                    }
                    if length > *key_max_length {
                        return Err(pointer.violation(WHY_PROPERTY_NAME_TOO_LONG));
                    }
                }
                // Everything inside a map value reports the map's own pointer:
                // a key the agent chose is never part of one.
                let was_frozen = pointer.freeze();
                for entry in object.values_mut() {
                    value_schema.walk(entry, pointer)?;
                }
                pointer.thaw(was_frozen);
                Ok(())
            }
        }
    }

    /// The most bytes a value this node accepts can take once serialized as
    /// compact JSON, counting 6 bytes for every character of every string and
    /// of every agent-chosen key, and every key, quote, brace, bracket, colon
    /// and comma of the structure around it.
    ///
    /// An enum value and an id are exact, because the first is a fixed ASCII
    /// word and the second is 26 ASCII characters ([`SchemaKind::Id`]). The
    /// figure is what the byte cap is proven against.
    #[must_use]
    pub fn worst_case_bytes(&self) -> usize {
        match &self.kind {
            SchemaKind::Object { properties } => {
                let members = properties.iter().fold(0_usize, |total, property| {
                    total
                        .saturating_add(property.name.len().saturating_add(2))
                        .saturating_add(1)
                        .saturating_add(property.schema.worst_case_bytes())
                });
                2_usize
                    .saturating_add(members)
                    .saturating_add(properties.len().saturating_sub(1))
            }
            SchemaKind::String { max_length, .. } => {
                2_usize.saturating_add(max_length.saturating_mul(WORST_BYTES_PER_CHARACTER))
            }
            SchemaKind::Enum { values } => {
                2_usize.saturating_add(values.iter().map(|value| value.len()).max().unwrap_or(0))
            }
            SchemaKind::Id => 2 + ID_LENGTH,
            SchemaKind::Array {
                items, max_items, ..
            } => 2_usize
                .saturating_add(max_items.saturating_mul(items.worst_case_bytes()))
                .saturating_add(max_items.saturating_sub(1)),
            SchemaKind::Map {
                key_max_length,
                value,
                max_properties,
            } => {
                let member = 2_usize
                    .saturating_add(key_max_length.saturating_mul(WORST_BYTES_PER_CHARACTER))
                    .saturating_add(1)
                    .saturating_add(value.worst_case_bytes());
                2_usize
                    .saturating_add(max_properties.saturating_mul(member))
                    .saturating_add(max_properties.saturating_sub(1))
            }
        }
    }
}

/// The JSON pointer being built during a walk, which cannot hold an
/// agent-chosen segment.
struct Pointer {
    text: String,
    frozen: bool,
}

impl Pointer {
    const fn root() -> Self {
        Self {
            text: String::new(),
            frozen: false,
        }
    }

    /// Appends a schema property name (RFC 6901 escaped) and returns the length
    /// to truncate back to. Does nothing inside a map value.
    fn push_name(&mut self, name: &str) -> usize {
        let mark = self.text.len();
        if !self.frozen {
            self.text.push('/');
            for character in name.chars() {
                match character {
                    '~' => self.text.push_str("~0"),
                    '/' => self.text.push_str("~1"),
                    other => self.text.push(other),
                }
            }
        }
        mark
    }

    /// Appends an array index and returns the length to truncate back to. Does
    /// nothing inside a map value.
    fn push_index(&mut self, index: usize) -> usize {
        let mark = self.text.len();
        if !self.frozen {
            self.text.push('/');
            self.text.push_str(&index.to_string());
        }
        mark
    }

    fn truncate(&mut self, mark: usize) {
        self.text.truncate(mark);
    }

    /// Stops the pointer growing, and returns whether it already was stopped.
    const fn freeze(&mut self) -> bool {
        let was = self.frozen;
        self.frozen = true;
        was
    }

    const fn thaw(&mut self, was_frozen: bool) {
        self.frozen = was_frozen;
    }

    fn violation(&self, why: &'static str) -> Violation {
        Violation {
            field: self.text.clone(),
            why,
        }
    }
}

// ---------------------------------------------------------------------------
// The catalogue
// ---------------------------------------------------------------------------

/// The sentence every description ends with, for a cap of `max_arguments_bytes`
/// bytes: "Arguments over 512 KiB serialized are refused." at the default.
///
/// The sentence follows the **configured** cap, not a constant, so that a
/// description never misstates the limit in force; at the default it is the
/// sentence the catalogue's decision gives. A cap that is a whole number of KiB
/// is written in KiB, any other in bytes.
#[must_use]
pub fn cap_sentence(max_arguments_bytes: usize) -> String {
    if max_arguments_bytes != 0 && max_arguments_bytes.is_multiple_of(1024) {
        format!(
            "Arguments over {} KiB serialized are refused.",
            max_arguments_bytes / 1024
        )
    } else {
        format!("Arguments over {max_arguments_bytes} bytes serialized are refused.")
    }
}

/// One of Ori's eleven tools as the catalogue describes it: AICD §17.
///
/// Holds everything about a tool that does not depend on a session: its title,
/// its one-sentence effect (from `spec/API_SPEC.md` section 3's effect column),
/// its input schema tree and its behaviour hints.
#[derive(Clone, Debug)]
pub struct ToolSpec {
    tool: Tool,
    title: &'static str,
    effect: &'static str,
    schema: Schema,
    read_only: bool,
    open_world: bool,
}

impl ToolSpec {
    /// The tool this describes.
    #[must_use]
    pub const fn tool(&self) -> Tool {
        self.tool
    }

    /// The human title, also the root schema's `title`.
    #[must_use]
    pub const fn title(&self) -> &'static str {
        self.title
    }

    /// The tool's one-sentence effect, without the cap sentence.
    #[must_use]
    pub const fn effect(&self) -> &'static str {
        self.effect
    }

    /// The input schema tree, which both publishes and validates the arguments.
    #[must_use]
    pub const fn schema(&self) -> &Schema {
        &self.schema
    }

    /// Whether the tool changes nothing: `aicd_context`, `aicd_search` and
    /// `aicd_evidence`.
    #[must_use]
    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    /// Whether the tool reaches an external system: `aicd_spec_propose`, which
    /// opens a pull request on the external host.
    #[must_use]
    pub const fn open_world(&self) -> bool {
        self.open_world
    }

    /// Whether the tool takes a `ticket_id` argument, from the schema itself.
    #[must_use]
    pub fn takes_ticket(&self) -> bool {
        self.schema.property(TICKET_ARGUMENT).is_some()
    }

    /// The MCP definition `tools/list` returns for this tool, whose description
    /// is the tool's effect followed by the cap sentence for
    /// `max_arguments_bytes` ([`cap_sentence`]). The cap sentence follows the
    /// configured `max_arguments_bytes`; at the default it is the sentence the
    /// catalogue's decision gives, "Arguments over 512 KiB serialized are
    /// refused."
    ///
    /// `destructive_hint` is `false` for every tool, and `output_schema` is
    /// absent in phase 1.
    #[must_use]
    pub fn definition(&self, max_arguments_bytes: usize) -> ToolDefinition {
        ToolDefinition {
            name: self.tool.wire_name().to_owned(),
            title: Some(self.title.to_owned()),
            description: Some(format!(
                "{} {}",
                self.effect,
                cap_sentence(max_arguments_bytes)
            )),
            input_schema: self.schema.to_json(),
            output_schema: None,
            annotations: Some(ToolAnnotations {
                title: None,
                read_only_hint: Some(self.read_only),
                destructive_hint: Some(false),
                idempotent_hint: None,
                open_world_hint: Some(self.open_world),
            }),
            execution: None,
        }
    }
}

/// The eight AICD §12 escalation triggers, spelled as `CLAUDE.md`'s escalation
/// list spells them.
pub const ESCALATION_TRIGGERS: [&str; 8] = [
    "adr_area",
    "test_modified",
    "new_dependency",
    "spec_conflict",
    "scope_conflict",
    "contract_change",
    "security",
    "precondition_missing",
];

/// The kinds of report `aicd_report` takes (`spec/API_SPEC.md` section 3,
/// "Report kinds").
pub const REPORT_KINDS: [&str; 4] = ["closing", "blocked", "qa_run", "drift_audit"];

fn ticket_id_property() -> Property {
    Property::required(
        TICKET_ARGUMENT,
        Schema::id().described("The ticket's identifier, a 26-character ULID."),
    )
}

/// The `evidence` of `aicd_escalate` and `aicd_ticket_create`: 1 to 16 entries,
/// each `{what, found_at}` (`spec/API_SPEC.md` section 3, "Escalation
/// evidence").
fn evidence_schema() -> Schema {
    Schema::array(
        Schema::object(vec![
            Property::required("what", Schema::string(1, 2048).described("What was found.")),
            Property::required(
                "found_at",
                Schema::string(1, 1024).described("Where it was found."),
            ),
        ]),
        1,
        16,
    )
    .described("What stands behind the request: 1 to 16 findings, each with where it was found.")
}

fn category_values() -> Vec<&'static str> {
    Category::ALL
        .iter()
        .map(|category| category.as_str())
        .collect()
}

/// Builds the catalogue entry of one tool. One exhaustive `match`, no wildcard:
/// a new tool does not compile until it is described.
fn build(tool: Tool) -> ToolSpec {
    let (title, effect, schema, read_only, open_world) = match tool {
        Tool::Context => (
            "Context package",
            "Returns the bounded context package for a ticket (AICD §25).",
            Schema::object(vec![ticket_id_property()]),
            true,
            false,
        ),
        Tool::Search => (
            "Search",
            "Searches the layers of memory the caller's role may read.",
            Schema::object(vec![Property::required(
                "query",
                Schema::string(1, 256).described("What to search for."),
            )]),
            true,
            false,
        ),
        Tool::Evidence => (
            "Evidence",
            "Returns raw evidence, labeled untrusted and logged.",
            Schema::object(vec![Property::required(
                "record_id",
                Schema::id().described("The evidence record's identifier, a 26-character ULID."),
            )]),
            true,
            false,
        ),
        Tool::PlanSubmit => (
            "Submit plan",
            "Stores the implementation plan for a ticket and runs the scope lock check, which \
             may return E_SCOPE_LOCKED.",
            Schema::object(vec![
                ticket_id_property(),
                Property::required(
                    "plan",
                    Schema::string(1, 65_536).described("The implementation plan."),
                ),
                Property::required(
                    "declared_scope",
                    Schema::array(
                        Schema::string(1, 256).described("A module path the plan will touch."),
                        1,
                        64,
                    )
                    .described("The modules the plan declares it will touch."),
                ),
            ]),
            false,
            false,
        ),
        Tool::Report => (
            "Submit report",
            "Stores a closing, blocked, QA run or drift audit report through the sanitization \
             barrier.",
            Schema::object(vec![
                ticket_id_property(),
                Property::required(
                    "kind",
                    Schema::enumeration(REPORT_KINDS.to_vec()).described("Which report this is."),
                ),
                Property::required(
                    "content",
                    Schema::map(
                        64,
                        Schema::string(0, 8192).described("The text of one field."),
                        10,
                    )
                    .described("The report's named string fields, as its kind defines them."),
                ),
            ]),
            false,
            false,
        ),
        Tool::Escalate => (
            "Escalate",
            "Opens an escalation with its evidence, and suspends the session if configured.",
            Schema::object(vec![
                ticket_id_property(),
                Property::required(
                    "trigger",
                    Schema::enumeration(ESCALATION_TRIGGERS.to_vec())
                        .described("Which escalation trigger applies."),
                ),
                Property::required(
                    "question",
                    Schema::string(1, 8192).described("The question for the human."),
                ),
                Property::required(
                    "recommendation",
                    Schema::string(1, 8192).described("What the agent recommends."),
                ),
                Property::required("evidence", evidence_schema()),
            ]),
            false,
            false,
        ),
        Tool::TicketCreate => (
            "Create ticket",
            "Files a ticket, with its category and kind as proposals only.",
            Schema::object(vec![
                Property::required(
                    "title",
                    Schema::string(1, 256).described("The ticket's title."),
                ),
                Property::required(
                    "category",
                    Schema::enumeration(category_values()).described("The proposed category."),
                ),
                Property::required(
                    "kind",
                    Schema::enumeration(TicketKind::ALL.iter().map(|kind| kind.as_str()).collect())
                        .described("The proposed kind."),
                ),
                Property::required(
                    "spec_anchor",
                    Schema::string(1, 512).described("The specification section it concerns."),
                ),
                Property::required("evidence", evidence_schema()),
            ]),
            false,
            false,
        ),
        Tool::TicketUpgrade => (
            "Upgrade ticket category",
            "Raises a ticket's category; a category is never lowered.",
            Schema::object(vec![
                ticket_id_property(),
                Property::required(
                    "category",
                    Schema::enumeration(category_values()).described("The category to raise to."),
                ),
            ]),
            false,
            false,
        ),
        Tool::CriterionPropose => (
            "Propose criterion",
            "Enters a criterion in the proposed state only.",
            // Phase 2 defines the arguments: `spec/API_SPEC.md` section 3 gives
            // the tool `(...)`. Until then the schema is an object with no
            // properties, and the tool is never listed or dispatched.
            Schema::object(Vec::new()),
            false,
            false,
        ),
        Tool::SpecPropose => (
            "Propose specification change",
            "Opens a pull request that proposes a specification change.",
            Schema::object(vec![
                Property::required(
                    "document_id",
                    Schema::id().described("The specification document's identifier."),
                ),
                Property::required(
                    "content",
                    Schema::string(1, 65_536).described("The proposed content."),
                ),
            ]),
            false,
            true,
        ),
        Tool::GateResult => (
            "Record gate result",
            "Records a gate run.",
            // Phase 2 or later defines the arguments; see `CriterionPropose`.
            Schema::object(Vec::new()),
            false,
            false,
        ),
    };
    ToolSpec {
        tool,
        title,
        effect,
        schema: schema.titled(title),
        read_only,
        open_world,
    }
}

/// The position of a tool in [`Tool::ALL`], by an exhaustive `match`.
const fn position(tool: Tool) -> usize {
    match tool {
        Tool::Context => 0,
        Tool::Search => 1,
        Tool::Evidence => 2,
        Tool::PlanSubmit => 3,
        Tool::Report => 4,
        Tool::Escalate => 5,
        Tool::TicketCreate => 6,
        Tool::TicketUpgrade => 7,
        Tool::CriterionPropose => 8,
        Tool::SpecPropose => 9,
        Tool::GateResult => 10,
    }
}

static CATALOGUE: OnceLock<[ToolSpec; 11]> = OnceLock::new();

/// All eleven of Ori's tools, in [`Tool::ALL`] order: AICD §17.
///
/// Built once. A tool that is not built in phase 1 is described here and is
/// never listed or dispatched while [`Tool::in_phase_1`] is false.
#[must_use]
pub fn catalogue() -> &'static [ToolSpec] {
    CATALOGUE.get_or_init(|| Tool::ALL.map(build))
}

/// The catalogue entry of one tool, or `None` if the catalogue and
/// [`Tool::ALL`] disagree on its position, which a test pins cannot happen. The
/// server treats `None` as a tool that is not available, so no path panics.
#[must_use]
pub fn spec(tool: Tool) -> Option<&'static ToolSpec> {
    catalogue().get(position(tool))
}

// ---------------------------------------------------------------------------
// The handler seam
// ---------------------------------------------------------------------------

/// Why a handler's effect failed: AICD §17.
///
/// The server records the failure as `mcp.tool_failed` and replies with an
/// error **result**, never a JSON-RPC error: a tool's business-logic failure is
/// an `isError` result (MCP's tools page). Which failure a variant is, and what
/// the reply carries of it, is the server's failure mapping
/// (`crate::server`'s module documentation): the reply and the event carry
/// codes, sections and constants, never the text a variant holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandlerError {
    /// A control inside the effect refused, under its own code and section: a
    /// memory scope refusal (AICD §25), the evidence lockout, a desk's refusal.
    Refused {
        /// A fixed code naming the refusal.
        code: &'static str,
        /// The methodology section it was refused under, which resolves.
        methodology_ref: MethodologyRef,
    },
    /// A port failed or refused; see [`crate::ports::PortError`].
    Port(PortError),
}

impl From<PortError> for HandlerError {
    fn from(error: PortError) -> Self {
        Self::Port(error)
    }
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused {
                code,
                methodology_ref,
            } => write!(f, "refused under {methodology_ref}: {code}"),
            Self::Port(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for HandlerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Refused { .. } => None,
            Self::Port(error) => Some(error),
        }
    }
}

/// What a handler knows of the call it is serving: AICD §17.
///
/// Where each field came from:
///
/// - `binding`, `called_seq` and `at` are the engine's and the server's: the
///   engine's binding, the `seq` the server's own append returned, and the
///   engine's clock;
/// - `tool` is the catalogue entry the message's name selected, never the name's
///   text;
/// - `ticket_id` is the agent's argument, validated and canonical. For a coder it
///   equals the bound ticket (check 7). For every other role the server checked
///   it against nothing, and the handler that acts on it must: slice S4 checks
///   that the ticket exists in this product, slice S3 the memory scope, slice S9
///   the escalation desk.
#[derive(Clone, Copy, Debug)]
pub struct BoundCall<'a> {
    /// The session the connection serves: the engine's binding.
    pub binding: &'a Binding,
    /// The tool called: the catalogue entry the message's name selected.
    pub tool: Tool,
    /// The `seq` of the `mcp.tool_called` event appended for this call.
    pub called_seq: u64,
    /// The call's time: the engine's clock, read once at call entry.
    pub at: Timestamp,
    /// The validated, canonical `ticket_id` argument, for a tool that has one:
    /// the agent's own claim, which the handler that acts on it checks for every
    /// role but the coder.
    pub ticket_id: Option<&'a Id>,
}

/// The effect of one tool: what slices S3 to S9 implement (AICD §17).
///
/// **When it is called.** Only after every control of the server's pipeline
/// passed (the product's database, the session's call budget and state, the
/// tool's phase and registration, the role, the arguments' size and schema and
/// the ticket rule), and after `mcp.tool_called` was appended. A handler is
/// never reached by a refused call, and it never runs without its record.
///
/// **What it gets.** The database writer of the product, through the same
/// `&mut ProductDb` the engine holds, so that it opens none of its own; the
/// [`crate::tools::BoundCall`]; and the arguments, which passed the tool's
/// schema and in which every id-typed value is already in canonical form. It
/// does not re-check the schema. It checks what the schema cannot: that a
/// record exists in this product, what a report holds per kind, what a role may
/// do with a ticket it is not bound to.
///
/// **What it returns.** `Ok` with a result, which is the reply as it is, or an
/// error or an error result, which the server records as `mcp.tool_failed`.
pub trait ToolHandler: Send {
    /// Performs the call's effect. Called only after every control passed and
    /// `mcp.tool_called` was appended.
    ///
    /// # Errors
    ///
    /// A [`crate::tools::HandlerError`]: a refusal inside the effect, or a
    /// failure of a port.
    fn call(
        &mut self,
        db: &mut ProductDb,
        call: &BoundCall<'_>,
        arguments: JsonObject,
    ) -> Result<CallToolResult, HandlerError>;
}

/// The handlers registered for Ori's tools, one per tool: AICD §17.
///
/// A second registration of a tool never replaces the first.
/// Registering a tool that is not built in phase 1 is allowed, because a later
/// phase registers it; `tools/list` and dispatch still refuse it while
/// [`Tool::in_phase_1`] is false.
pub struct Handlers {
    handlers: HashMap<Tool, Box<dyn ToolHandler>>,
}

impl Handlers {
    /// No handler registered.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Registers `handler` for `tool`, or returns `Err(tool)` and keeps the
    /// first handler when one is already registered.
    ///
    /// # Errors
    ///
    /// The tool, when a handler is already registered for it.
    pub fn register(&mut self, tool: Tool, handler: Box<dyn ToolHandler>) -> Result<(), Tool> {
        match self.handlers.entry(tool) {
            Entry::Occupied(_) => Err(tool),
            Entry::Vacant(slot) => {
                slot.insert(handler);
                Ok(())
            }
        }
    }

    /// Whether a handler is registered for `tool`.
    #[must_use]
    pub fn is_registered(&self, tool: Tool) -> bool {
        self.handlers.contains_key(&tool)
    }

    /// The handler registered for `tool`, to call it.
    pub(crate) fn handler_mut(&mut self, tool: Tool) -> Option<&mut (dyn ToolHandler + 'static)> {
        self.handlers.get_mut(&tool).map(|handler| &mut **handler)
    }
}

impl Default for Handlers {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Handlers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(Tool::ALL.iter().filter(|tool| self.is_registered(**tool)))
            .finish()
    }
}

/// The largest schema-valid argument object for `schema`: every string at its
/// maximum length of control characters (6 bytes each once serialized), every
/// array and map at its maximum size, every enum at its longest value.
///
/// For the tests of this module and of `crate::server`, which prove the byte cap
/// against it.
#[cfg(test)]
pub(crate) fn largest_valid_arguments(schema: &Schema) -> Value {
    /// Control characters that serialize as `\u00XX`, six bytes each.
    const SIX_BYTE_CONTROLS: [u32; 25] = [
        1, 2, 3, 4, 5, 6, 7, 11, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30,
    ];
    match schema.kind() {
        SchemaKind::Object { properties } => Value::Object(
            properties
                .iter()
                .map(|property| {
                    (
                        property.name.to_owned(),
                        largest_valid_arguments(&property.schema),
                    )
                })
                .collect(),
        ),
        SchemaKind::String { max_length, .. } => Value::String("\u{1}".repeat(*max_length)),
        SchemaKind::Enum { values } => {
            let longest = values.iter().max_by_key(|value| value.len());
            Value::from(longest.copied().unwrap_or_default())
        }
        SchemaKind::Id => Value::from("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
        SchemaKind::Array {
            items, max_items, ..
        } => Value::Array(
            (0..*max_items)
                .map(|_| largest_valid_arguments(items))
                .collect(),
        ),
        SchemaKind::Map {
            key_max_length,
            value,
            max_properties,
        } => Value::Object(
            (0..*max_properties)
                .map(|index| {
                    let last = char::from_u32(SIX_BYTE_CONTROLS[index % SIX_BYTE_CONTROLS.len()])
                        .unwrap_or('\u{1}');
                    let mut key = "\u{1}".repeat(key_max_length.saturating_sub(1));
                    key.push(last);
                    (key, largest_valid_arguments(value))
                })
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    /// A valid identifier, in canonical form.
    const ID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    /// The constraint keywords the validator enforces.
    const CONSTRAINT_KEYWORDS: [&str; 12] = [
        "type",
        "properties",
        "required",
        "additionalProperties",
        "minLength",
        "maxLength",
        "enum",
        "items",
        "minItems",
        "maxItems",
        "propertyNames",
        "maxProperties",
    ];

    /// The annotations a schema may carry: never constraints.
    const ANNOTATIONS: [&str; 2] = ["description", "title"];

    /// The catalogue entry of a tool, which every tool has.
    fn spec(tool: Tool) -> &'static ToolSpec {
        super::spec(tool).expect("every tool has an entry")
    }

    fn checked(schema: &Schema, value: Value) -> Result<Value, Violation> {
        let mut value = value;
        schema.validate(&mut value)?;
        Ok(value)
    }

    fn refused(schema: &Schema, value: &Value) -> Violation {
        match checked(schema, value.clone()) {
            Err(violation) => violation,
            Ok(_) => panic!("{value} was accepted"),
        }
    }

    fn violation(field: &str, why: &'static str) -> Violation {
        Violation {
            field: field.to_owned(),
            why,
        }
    }

    // -------------------------------------------------------------------
    // The catalogue
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0169_the_catalogue_covers_all_eleven_tools_in_order() {
        let specs = catalogue();
        assert_eq!(specs.len(), 11);
        for (index, tool) in Tool::ALL.iter().enumerate() {
            assert_eq!(specs[index].tool(), *tool);
            assert_eq!(position(*tool), index);
            assert_eq!(spec(*tool).tool(), *tool);
        }
    }

    #[test]
    fn ori_t_0169_every_definition_has_its_title_description_and_annotations() {
        for spec in catalogue() {
            let definition = spec.definition(524_288);
            assert_eq!(definition.name, spec.tool().wire_name());
            assert_eq!(definition.title.as_deref(), Some(spec.title()));
            let description = definition.description.expect("a description");
            assert!(
                description.starts_with(spec.effect())
                    && description.ends_with(" Arguments over 512 KiB serialized are refused."),
                "{description}"
            );
            assert_eq!(
                definition.output_schema, None,
                "no output schema in phase 1"
            );
            assert_eq!(definition.execution, None);
            let annotations = definition.annotations.expect("annotations");
            let read_only = matches!(spec.tool(), Tool::Context | Tool::Search | Tool::Evidence);
            assert_eq!(
                annotations.read_only_hint,
                Some(read_only),
                "{}",
                spec.tool()
            );
            assert_eq!(annotations.destructive_hint, Some(false), "{}", spec.tool());
            assert_eq!(
                annotations.open_world_hint,
                Some(spec.tool() == Tool::SpecPropose),
                "only aicd_spec_propose reaches an external host: {}",
                spec.tool()
            );
            assert_eq!(annotations.idempotent_hint, None);
            assert_eq!(
                definition.input_schema.get("title"),
                Some(&Value::from(spec.title())),
                "the root carries the title"
            );
            assert_eq!(spec.read_only(), read_only);
            assert_eq!(spec.open_world(), spec.tool() == Tool::SpecPropose);
        }
    }

    #[test]
    fn ori_t_0169_the_cap_sentence_states_the_cap_in_force() {
        assert_eq!(
            cap_sentence(524_288),
            "Arguments over 512 KiB serialized are refused."
        );
        assert_eq!(
            cap_sentence(1024),
            "Arguments over 1 KiB serialized are refused."
        );
        assert_eq!(
            cap_sentence(1000),
            "Arguments over 1000 bytes serialized are refused."
        );
        assert_eq!(
            cap_sentence(0),
            "Arguments over 0 bytes serialized are refused."
        );
    }

    /// The argument lists of `spec/API_SPEC.md` section 3's first table, by wire
    /// name, in the order the signatures give them. `None` for `(...)`.
    fn spec_signatures() -> Vec<(String, Option<Vec<String>>)> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for part in ["..", "..", "spec", "API_SPEC.md"] {
            path.push(part);
        }
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let mut found = Vec::new();
        for line in text
            .lines()
            .skip_while(|line| !line.starts_with("## 3."))
            .skip(1)
            .take_while(|line| !line.starts_with("### "))
        {
            let Some(rest) = line.strip_prefix("| `aicd_") else {
                continue;
            };
            let signature = rest.split('`').next().expect("a closing backtick");
            let (name, arguments) = signature
                .split_once('(')
                .unwrap_or_else(|| panic!("a signature with no argument list: {line}"));
            let arguments = arguments.trim_end_matches(')');
            let list = if arguments == "..." {
                None
            } else {
                Some(arguments.split(',').map(|a| a.trim().to_owned()).collect())
            };
            found.push((format!("aicd_{name}"), list));
        }
        found
    }

    fn property_names(schema: &Schema) -> Vec<&'static str> {
        match schema.kind() {
            SchemaKind::Object { properties } => {
                properties.iter().map(|property| property.name).collect()
            }
            other => panic!("not an object: {other:?}"),
        }
    }

    #[test]
    fn ori_t_0169_the_argument_names_are_the_signatures_of_the_specification() {
        let signatures = spec_signatures();
        assert_eq!(signatures.len(), 11, "the first table lists eleven tools");
        for (name, arguments) in &signatures {
            let tool = Tool::from_wire_name(name).unwrap_or_else(|| panic!("{name} is a tool"));
            let names = property_names(spec(tool).schema());
            match arguments {
                _ if tool == Tool::GateResult => {
                    // The specification names the four arguments of
                    // `aicd_gate_result` and gives no type or limit for any. The
                    // tool is not built in phase 1 and is never listed, so its
                    // schema stays an object with no properties until the phase
                    // that builds it defines them. This arm pins the gap: it
                    // fails when either side changes, so the change is made on
                    // purpose.
                    assert_eq!(
                        arguments.as_deref(),
                        Some(&["gate_id", "pr_id", "status", "output"].map(str::to_owned)[..])
                    );
                    assert!(names.is_empty());
                    assert!(!tool.in_phase_1());
                }
                Some(arguments) => assert_eq!(&names, arguments, "{name}"),
                None => assert!(
                    names.is_empty(),
                    "{name}: the specification defines no arguments"
                ),
            }
        }
    }

    #[test]
    fn ori_t_0169_every_property_of_every_tool_is_required() {
        for spec in catalogue() {
            let json = spec.schema().to_json();
            let required: BTreeSet<&str> = json
                .get("required")
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let names: BTreeSet<&str> = property_names(spec.schema()).into_iter().collect();
            assert_eq!(required, names, "{}", spec.tool());
        }
    }

    #[test]
    fn ori_t_0169_the_enums_hold_the_decided_values() {
        let values = |tool: Tool, property: &str| -> Vec<&'static str> {
            match spec(tool).schema().property(property).map(Schema::kind) {
                Some(SchemaKind::Enum { values }) => values.clone(),
                other => panic!("not an enum: {other:?}"),
            }
        };
        assert_eq!(
            values(Tool::Report, "kind"),
            ["closing", "blocked", "qa_run", "drift_audit"]
        );
        assert_eq!(
            values(Tool::Escalate, "trigger"),
            [
                "adr_area",
                "test_modified",
                "new_dependency",
                "spec_conflict",
                "scope_conflict",
                "contract_change",
                "security",
                "precondition_missing"
            ]
        );
        let categories = ["auto", "behavioral", "decisional", "product_signal"];
        assert_eq!(values(Tool::TicketUpgrade, "category"), categories);
        assert_eq!(values(Tool::TicketCreate, "category"), categories);
        assert_eq!(
            values(Tool::TicketCreate, "kind"),
            ["defect", "feature", "chore"]
        );
        for tool in Tool::ALL {
            for (name, schema) in enum_nodes(spec(tool).schema()) {
                let SchemaKind::Enum { values } = schema.kind() else {
                    continue;
                };
                for value in values {
                    assert!(
                        value.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
                        "{tool} {name}: {value} is not a plain word, which the worst case relies on"
                    );
                }
            }
        }
    }

    fn enum_nodes(schema: &Schema) -> Vec<(String, &Schema)> {
        let mut found = Vec::new();
        fn visit<'a>(name: &str, schema: &'a Schema, found: &mut Vec<(String, &'a Schema)>) {
            match schema.kind() {
                SchemaKind::Enum { .. } => found.push((name.to_owned(), schema)),
                SchemaKind::Object { properties } => {
                    for property in properties {
                        visit(property.name, &property.schema, found);
                    }
                }
                SchemaKind::Array { items, .. } => visit(name, items, found),
                SchemaKind::Map { value, .. } => visit(name, value, found),
                SchemaKind::String { .. } | SchemaKind::Id => {}
            }
        }
        visit("", schema, &mut found);
        found
    }

    #[test]
    fn ori_t_0169_the_string_bounds_are_the_decided_ones() {
        let bounds = |tool: Tool, path: &[&str]| -> (usize, usize) {
            let mut node = spec(tool).schema();
            for step in path {
                node = match (node.kind(), *step) {
                    (SchemaKind::Array { items, .. }, "[]") => items,
                    (SchemaKind::Map { value, .. }, "*") => value,
                    (SchemaKind::Object { .. }, name) => node.property(name).expect("a property"),
                    (other, step) => panic!("no step {step} in {other:?}"),
                };
            }
            match node.kind() {
                SchemaKind::String {
                    min_length,
                    max_length,
                } => (*min_length, *max_length),
                other => panic!("not a string: {other:?}"),
            }
        };
        assert_eq!(bounds(Tool::Search, &["query"]), (1, 256));
        assert_eq!(bounds(Tool::PlanSubmit, &["plan"]), (1, 65_536));
        assert_eq!(
            bounds(Tool::PlanSubmit, &["declared_scope", "[]"]),
            (1, 256)
        );
        assert_eq!(bounds(Tool::Report, &["content", "*"]), (0, 8192));
        assert_eq!(bounds(Tool::Escalate, &["question"]), (1, 8192));
        assert_eq!(bounds(Tool::Escalate, &["recommendation"]), (1, 8192));
        assert_eq!(
            bounds(Tool::Escalate, &["evidence", "[]", "what"]),
            (1, 2048)
        );
        assert_eq!(
            bounds(Tool::Escalate, &["evidence", "[]", "found_at"]),
            (1, 1024)
        );
        assert_eq!(
            bounds(Tool::TicketCreate, &["evidence", "[]", "what"]),
            (1, 2048)
        );
        assert_eq!(bounds(Tool::TicketCreate, &["title"]), (1, 256));
        assert_eq!(bounds(Tool::TicketCreate, &["spec_anchor"]), (1, 512));
        assert_eq!(bounds(Tool::SpecPropose, &["content"]), (1, 65_536));
        // The arrays and the map.
        let json = |tool: Tool| Value::Object(spec(tool).schema().to_json());
        let plan = json(Tool::PlanSubmit);
        assert_eq!(plan["properties"]["declared_scope"]["minItems"], 1);
        assert_eq!(plan["properties"]["declared_scope"]["maxItems"], 64);
        let escalate = json(Tool::Escalate);
        assert_eq!(escalate["properties"]["evidence"]["minItems"], 1);
        assert_eq!(escalate["properties"]["evidence"]["maxItems"], 16);
        let report = json(Tool::Report);
        assert_eq!(report["properties"]["content"]["maxProperties"], 10);
        assert_eq!(
            report["properties"]["content"]["propertyNames"],
            json!({"minLength": 1, "maxLength": 64})
        );
    }

    #[test]
    fn ori_t_0169_the_search_query_cap_is_at_most_the_indexers_byte_cap() {
        // 256 characters are at most 1 024 bytes of UTF-8, the indexer's
        // `MAX_QUERY_BYTES`.
        let schema = Schema::object(vec![Property::required("query", Schema::string(1, 256))]);
        let four_byte = "\u{1F600}".repeat(256);
        assert_eq!(four_byte.len(), 1024);
        assert!(checked(&schema, json!({"query": four_byte})).is_ok());
        assert_eq!(
            refused(&schema, &json!({"query": "x".repeat(257)})),
            violation("/query", WHY_TOO_LONG)
        );
    }

    // -------------------------------------------------------------------
    // What a schema publishes
    // -------------------------------------------------------------------

    /// Fails on any keyword that is not a constraint the validator enforces or an
    /// annotation, at any depth, and on an object with no `additionalProperties:
    /// false`.
    fn check_published(
        node: &Value,
        path: &str,
        seen: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        let Value::Object(object) = node else {
            return Err(format!("{path}: a schema node is an object"));
        };
        for key in object.keys() {
            if !CONSTRAINT_KEYWORDS.contains(&key.as_str()) && !ANNOTATIONS.contains(&key.as_str())
            {
                return Err(format!("{path}: the keyword {key} is not enforced"));
            }
            seen.insert(key.clone());
        }
        let Some(Value::String(kind)) = object.get("type") else {
            return Err(format!("{path}: no type"));
        };
        match kind.as_str() {
            "object" => {
                if let Some(Value::Object(properties)) = object.get("properties") {
                    if object.get("additionalProperties") != Some(&Value::Bool(false)) {
                        return Err(format!(
                            "{path}: an object without additionalProperties: false"
                        ));
                    }
                    for (name, inner) in properties {
                        check_published(inner, &format!("{path}/{name}"), seen)?;
                    }
                } else {
                    // A map: its value schema stands in additionalProperties.
                    let Some(inner) = object.get("additionalProperties") else {
                        return Err(format!("{path}: a map without a value schema"));
                    };
                    check_published(inner, &format!("{path}/*"), seen)?;
                    let Some(Value::Object(names)) = object.get("propertyNames") else {
                        return Err(format!("{path}: a map without propertyNames"));
                    };
                    for key in names.keys() {
                        if key != "minLength" && key != "maxLength" {
                            return Err(format!("{path}: propertyNames carries {key}"));
                        }
                        seen.insert(format!("propertyNames.{key}"));
                    }
                }
            }
            "array" => {
                let Some(items) = object.get("items") else {
                    return Err(format!("{path}: an array without items"));
                };
                check_published(items, &format!("{path}/[]"), seen)?;
            }
            "string" => {}
            other => return Err(format!("{path}: the type {other} is not one the tree has")),
        }
        Ok(())
    }

    #[test]
    fn ori_t_0169_no_schema_publishes_a_keyword_the_validator_does_not_enforce() {
        let mut seen = BTreeSet::new();
        for spec in catalogue() {
            let published = Value::Object(spec.definition(524_288).input_schema);
            if let Err(why) = check_published(&published, spec.tool().wire_name(), &mut seen) {
                panic!("{why}");
            }
        }
        // The walk is not vacuous: every constraint keyword is published by
        // some tool.
        for keyword in CONSTRAINT_KEYWORDS {
            assert!(seen.contains(keyword), "no tool publishes {keyword}");
        }
        assert!(seen.contains("propertyNames.minLength"));
        assert!(seen.contains("propertyNames.maxLength"));
    }

    #[test]
    fn ori_t_0169_the_walk_refuses_a_planted_unenforced_keyword() {
        let mut seen = BTreeSet::new();
        let planted = json!({"type": "object", "additionalProperties": false, "properties": {
            "a": {"type": "string", "minLength": 1, "maxLength": 2, "pattern": "^x$"}}});
        let error = check_published(&planted, "planted", &mut seen).expect_err("a keyword");
        assert!(error.contains("pattern"), "{error}");
        let open = json!({"type": "object", "properties": {"a": {"type": "string"}}});
        assert!(check_published(&open, "open", &mut seen).is_err());
        let nested = json!({"type": "object", "additionalProperties": false, "properties": {
            "a": {"type": "array", "items": {"type": "object", "properties": {}}}}});
        assert!(
            check_published(&nested, "nested", &mut seen).is_err(),
            "additionalProperties: false is required at every depth"
        );
    }

    #[test]
    fn ori_t_0169_an_id_is_published_as_26_characters_and_enforced_as_a_ulid() {
        let id = Schema::id();
        assert_eq!(
            Value::Object(id.to_json()),
            json!({"type": "string", "minLength": 26, "maxLength": 26})
        );
        // The published schema says less than the server enforces: 26 characters
        // of the wrong alphabet pass the first and fail the second.
        let schema = Schema::object(vec![Property::required("id", Schema::id())]);
        for bad in [
            "01ARZ3NDEKTSV4RRFFQ69G5FAU", // U is not in Crockford base 32
            "01ARZ3NDEKTSV4RRFFQ69G5FAI",
            "81ARZ3NDEKTSV4RRFFQ69G5FAV", // first symbol above 7
            "01ARZ3NDEKTSV4RRFFQ69G5FA-",
            "é1ARZ3NDEKTSV4RRFFQ69G5FAV", // 26 characters, 27 bytes
        ] {
            assert_eq!(
                bad.chars().count(),
                26,
                "{bad} is 26 characters, as published"
            );
            assert_eq!(
                refused(&schema, &json!({"id": bad})),
                violation("/id", WHY_NOT_AN_IDENTIFIER),
                "{bad}"
            );
        }
        assert_eq!(
            refused(&schema, &json!({"id": "01ARZ3NDEKTSV4RRFFQ69G5FA"})),
            violation("/id", WHY_TOO_SHORT)
        );
        assert_eq!(
            refused(&schema, &json!({"id": "01ARZ3NDEKTSV4RRFFQ69G5FAVX"})),
            violation("/id", WHY_TOO_LONG)
        );
        assert_eq!(
            refused(&schema, &json!({"id": 7})),
            violation("/id", WHY_NOT_A_STRING)
        );
    }

    #[test]
    fn ori_t_0169_an_accepted_id_is_rewritten_to_its_canonical_form() {
        let schema = Schema::object(vec![
            Property::required("id", Schema::id()),
            Property::required("ids", Schema::array(Schema::id(), 1, 3)),
        ]);
        let lower = ID_A.to_ascii_lowercase();
        let checked = checked(&schema, json!({"id": lower, "ids": [lower, ID_A]}))
            .expect("lower case is accepted");
        assert_eq!(checked, json!({"id": ID_A, "ids": [ID_A, ID_A]}));
    }

    // -------------------------------------------------------------------
    // The validator, node kind by node kind
    // -------------------------------------------------------------------

    fn object_schema() -> Schema {
        Schema::object(vec![
            Property::required("name", Schema::string(2, 4)),
            Property::optional("tag", Schema::enumeration(vec!["a", "b"])),
        ])
    }

    #[test]
    fn ori_t_0169_an_object_node_reports_its_violations_by_pointer() {
        let schema = object_schema();
        assert_eq!(
            checked(&schema, json!({"name": "ab"})),
            Ok(json!({"name": "ab"}))
        );
        assert!(checked(&schema, json!({"name": "ab", "tag": "b"})).is_ok());
        // Not an object, at the root: the pointer is "".
        for not_object in [json!([]), json!("x"), json!(null), json!(3)] {
            assert_eq!(
                refused(&schema, &not_object),
                violation("", WHY_NOT_AN_OBJECT)
            );
        }
        // A required property that is missing reports that property.
        assert_eq!(
            refused(&schema, &json!({})),
            violation("/name", WHY_MISSING_PROPERTY)
        );
        // An extra key reports the object that holds it, never the key.
        assert_eq!(
            refused(&schema, &json!({"name": "ab", "zz-marker-key": 1})),
            violation("", WHY_UNEXPECTED_PROPERTY)
        );
        // A violation inside a property reports that property.
        assert_eq!(
            refused(&schema, &json!({"name": "a"})),
            violation("/name", WHY_TOO_SHORT)
        );
        assert_eq!(
            refused(&schema, &json!({"name": "ab", "tag": "c"})),
            violation("/tag", WHY_NOT_AN_ALLOWED_VALUE)
        );
    }

    #[test]
    fn ori_t_0169_the_order_of_violations_is_fixed() {
        let schema = object_schema();
        // An unexpected key is reported before a property's own violation.
        assert_eq!(
            refused(&schema, &json!({"name": "a", "extra": 1})),
            violation("", WHY_UNEXPECTED_PROPERTY)
        );
        // Properties are checked in schema order.
        assert_eq!(
            refused(&schema, &json!({"tag": "c"})),
            violation("/name", WHY_MISSING_PROPERTY)
        );
    }

    #[test]
    fn ori_t_0169_a_string_node_counts_characters_not_bytes() {
        let schema = Schema::object(vec![Property::required("s", Schema::string(2, 3))]);
        // Two characters of two bytes each, and of four bytes each.
        assert!(checked(&schema, json!({"s": "éé"})).is_ok());
        assert!(checked(&schema, json!({"s": "\u{1F600}\u{1F600}\u{1F600}"})).is_ok());
        assert_eq!(
            refused(&schema, &json!({"s": "é"})),
            violation("/s", WHY_TOO_SHORT)
        );
        assert_eq!(
            refused(&schema, &json!({"s": "éééé"})),
            violation("/s", WHY_TOO_LONG)
        );
        for not_string in [json!(1), json!(true), json!(null), json!(["ab"]), json!({})] {
            assert_eq!(
                refused(&schema, &json!({ "s": not_string })),
                violation("/s", WHY_NOT_A_STRING)
            );
        }
    }

    #[test]
    fn ori_t_0169_an_enum_node_accepts_only_its_values() {
        let schema = Schema::object(vec![Property::required(
            "e",
            Schema::enumeration(vec!["one", "two"]),
        )]);
        assert!(checked(&schema, json!({"e": "one"})).is_ok());
        for bad in ["One", "one ", "three", ""] {
            assert_eq!(
                refused(&schema, &json!({ "e": bad })),
                violation("/e", WHY_NOT_AN_ALLOWED_VALUE)
            );
        }
        assert_eq!(
            refused(&schema, &json!({"e": 1})),
            violation("/e", WHY_NOT_A_STRING)
        );
    }

    #[test]
    fn ori_t_0169_an_array_node_bounds_its_items_and_reports_the_index() {
        let schema = Schema::object(vec![Property::required(
            "xs",
            Schema::array(Schema::string(1, 2), 1, 3),
        )]);
        assert!(checked(&schema, json!({"xs": ["a"]})).is_ok());
        assert!(checked(&schema, json!({"xs": ["a", "b", "c"]})).is_ok());
        assert_eq!(
            refused(&schema, &json!({"xs": []})),
            violation("/xs", WHY_TOO_FEW_ITEMS)
        );
        assert_eq!(
            refused(&schema, &json!({"xs": ["a", "b", "c", "d"]})),
            violation("/xs", WHY_TOO_MANY_ITEMS)
        );
        assert_eq!(
            refused(&schema, &json!({"xs": ["a", "b", "ccc"]})),
            violation("/xs/2", WHY_TOO_LONG)
        );
        assert_eq!(
            refused(&schema, &json!({"xs": "a"})),
            violation("/xs", WHY_NOT_AN_ARRAY)
        );
        assert_eq!(
            refused(&schema, &json!({"xs": ["a", 1]})),
            violation("/xs/1", WHY_NOT_A_STRING)
        );
    }

    #[test]
    fn ori_t_0169_an_object_in_an_array_reports_the_index_and_the_property() {
        let schema = Schema::object(vec![Property::required(
            "evidence",
            Schema::array(
                Schema::object(vec![
                    Property::required("what", Schema::string(1, 5)),
                    Property::required("found_at", Schema::string(1, 5)),
                ]),
                1,
                4,
            ),
        )]);
        let good = json!({"what": "w", "found_at": "f"});
        assert_eq!(
            refused(
                &schema,
                &json!({"evidence": [good, good, good, {"what": "w", "found_at": "f", "k": 1}]})
            ),
            violation("/evidence/3", WHY_UNEXPECTED_PROPERTY)
        );
        assert_eq!(
            refused(&schema, &json!({"evidence": [good, {"what": "w"}]})),
            violation("/evidence/1/found_at", WHY_MISSING_PROPERTY)
        );
        assert_eq!(
            refused(
                &schema,
                &json!({"evidence": [good, {"what": "", "found_at": "f"}]})
            ),
            violation("/evidence/1/what", WHY_TOO_SHORT)
        );
        assert_eq!(
            refused(&schema, &json!({"evidence": [good, 7]})),
            violation("/evidence/1", WHY_NOT_AN_OBJECT)
        );
    }

    fn map_schema() -> Schema {
        Schema::object(vec![
            Property::required("content", Schema::map(3, Schema::string(0, 3), 2)),
            Property::required("after", Schema::string(1, 2)),
        ])
    }

    #[test]
    fn ori_t_0169_a_map_node_never_puts_an_agent_chosen_key_in_a_pointer() {
        let schema = map_schema();
        assert!(checked(&schema, json!({"content": {}, "after": "a"})).is_ok());
        assert!(
            checked(
                &schema,
                json!({"content": {"k": "v", "kk": ""}, "after": "a"})
            )
            .is_ok()
        );
        let marker = "mk";
        // A violation inside a value reports the map, never the key.
        assert_eq!(
            refused(&schema, &json!({"content": {marker: 77}, "after": "a"})),
            violation("/content", WHY_NOT_A_STRING)
        );
        assert_eq!(
            refused(
                &schema,
                &json!({"content": {marker: "toolong"}, "after": "a"})
            ),
            violation("/content", WHY_TOO_LONG)
        );
        // Too many properties, a key too long, a key too short.
        assert_eq!(
            refused(
                &schema,
                &json!({"content": {"a": "", "b": "", "c": ""}, "after": "a"})
            ),
            violation("/content", WHY_TOO_MANY_PROPERTIES)
        );
        assert_eq!(
            refused(&schema, &json!({"content": {"abcd": ""}, "after": "a"})),
            violation("/content", WHY_PROPERTY_NAME_TOO_LONG)
        );
        assert_eq!(
            refused(&schema, &json!({"content": {"": ""}, "after": "a"})),
            violation("/content", WHY_PROPERTY_NAME_TOO_SHORT)
        );
        assert_eq!(
            refused(&schema, &json!({"content": "x", "after": "a"})),
            violation("/content", WHY_NOT_AN_OBJECT)
        );
        // The pointer is not left frozen after a map: the sibling that follows it
        // reports its own name.
        assert_eq!(
            refused(&schema, &json!({"content": {"k": "v"}, "after": "abc"})),
            violation("/after", WHY_TOO_LONG)
        );
    }

    #[test]
    fn ori_t_0169_a_map_of_arrays_still_reports_the_maps_own_pointer() {
        let schema = Schema::object(vec![Property::required(
            "m",
            Schema::map(4, Schema::array(Schema::string(1, 1), 1, 2), 3),
        )]);
        assert_eq!(
            refused(&schema, &json!({"m": {"k": ["a", "bb"]}})),
            violation("/m", WHY_TOO_LONG),
            "no key and no index inside a map value"
        );
    }

    #[test]
    fn ori_t_0169_a_property_name_is_escaped_in_a_pointer_as_rfc_6901_says() {
        let schema = Schema::object(vec![Property::required("a/b~c", Schema::string(1, 1))]);
        assert_eq!(
            refused(&schema, &json!({})),
            violation("/a~1b~0c", WHY_MISSING_PROPERTY)
        );
        assert_eq!(
            refused(&schema, &json!({"a/b~c": ""})),
            violation("/a~1b~0c", WHY_TOO_SHORT)
        );
    }

    // -------------------------------------------------------------------
    // The phase 1 schemas as a whole
    // -------------------------------------------------------------------

    /// The smallest valid argument object of a node.
    fn minimal(schema: &Schema) -> Value {
        match schema.kind() {
            SchemaKind::Object { properties } => Value::Object(
                properties
                    .iter()
                    .filter(|property| property.required)
                    .map(|property| (property.name.to_owned(), minimal(&property.schema)))
                    .collect(),
            ),
            SchemaKind::String { min_length, .. } => Value::String("x".repeat(*min_length)),
            SchemaKind::Enum { values } => Value::from(values[0]),
            SchemaKind::Id => Value::from(ID_A),
            SchemaKind::Array {
                items, min_items, ..
            } => Value::Array((0..*min_items).map(|_| minimal(items)).collect()),
            SchemaKind::Map { .. } => Value::Object(JsonObject::new()),
        }
    }

    #[test]
    fn ori_t_0169_every_catalogue_schema_accepts_a_minimal_object_and_refuses_an_empty_one() {
        for spec in catalogue() {
            let schema = spec.schema();
            let minimal = minimal(schema);
            let accepted = checked(schema, minimal.clone())
                .unwrap_or_else(|v| panic!("{}: {minimal} refused: {v:?}", spec.tool()));
            assert_eq!(accepted, minimal, "{}", spec.tool());
            let names = property_names(schema);
            if names.is_empty() {
                // The phase 2 tools publish an object with no properties: nothing
                // is required, and nothing is accepted.
                assert_eq!(
                    refused(schema, &json!({"x": 1})),
                    violation("", WHY_UNEXPECTED_PROPERTY),
                    "{}",
                    spec.tool()
                );
                assert!(!spec.tool().in_phase_1(), "{}", spec.tool());
            } else {
                assert_eq!(
                    refused(schema, &json!({})),
                    violation(&format!("/{}", names[0]), WHY_MISSING_PROPERTY),
                    "{}",
                    spec.tool()
                );
            }
            assert_eq!(
                refused(schema, &json!("x")),
                violation("", WHY_NOT_AN_OBJECT)
            );
        }
    }

    #[test]
    fn ori_t_0169_every_tool_with_a_ticket_argument_is_found_from_its_schema() {
        let with_ticket: Vec<Tool> = catalogue()
            .iter()
            .filter(|spec| spec.takes_ticket())
            .map(ToolSpec::tool)
            .collect();
        assert_eq!(
            with_ticket,
            [
                Tool::Context,
                Tool::PlanSubmit,
                Tool::Report,
                Tool::Escalate,
                Tool::TicketUpgrade
            ]
        );
    }

    // -------------------------------------------------------------------
    // The byte cap is part of the contract
    // -------------------------------------------------------------------

    #[test]
    fn ori_t_0169_the_largest_valid_object_of_every_tool_fits_under_the_default_cap() {
        let cap = crate::server::DEFAULT_MAX_ARGUMENTS_BYTES;
        assert_eq!(cap, 524_288);
        assert!(
            cap < crate::protocol::MAX_LINE_BYTES - 1024,
            "the cap leaves room for the request envelope under the line cap"
        );
        for spec in catalogue() {
            let schema = spec.schema();
            let worst = schema.worst_case_bytes();
            assert!(
                worst <= cap,
                "{}: the worst case is {worst} bytes",
                spec.tool()
            );
            // The model is exact: the largest valid object is that big, no more
            // and no less, so the figure is neither a guess nor an
            // underestimate.
            let largest = largest_valid_arguments(schema);
            let serialized = serde_json::to_vec(&largest).expect("serializes").len();
            assert_eq!(serialized, worst, "{}", spec.tool());
            assert!(
                checked(schema, largest.clone()).is_ok(),
                "{}: its own largest object is valid",
                spec.tool()
            );
        }
    }

    #[test]
    fn ori_t_0169_a_maximum_raised_past_the_cap_is_caught_by_the_worst_case() {
        // The larger tools' worst cases are at least their text alone at 6 bytes
        // per character.
        let worst = |tool: Tool| spec(tool).schema().worst_case_bytes();
        // plan 65 536 characters, 64 scope entries of 256, one id.
        assert!(worst(Tool::PlanSubmit) > 65_536 * 6 + 64 * 256 * 6);
        assert!(worst(Tool::Report) > 10 * (64 + 8192) * 6);
        assert!(worst(Tool::Escalate) > (8192 * 2 + 16 * (2048 + 1024)) * 6);
        assert!(worst(Tool::SpecPropose) > 65_536 * 6);
        // And a schema whose maxima were raised past the cap is caught.
        let too_big = Schema::object(vec![Property::required("a", Schema::string(1, 90_000))]);
        assert!(too_big.worst_case_bytes() > crate::server::DEFAULT_MAX_ARGUMENTS_BYTES);
    }

    /// The keywords the validator enforces for a node of this kind, read from
    /// `Schema::walk`: what a published node of that kind carries, exactly,
    /// besides its annotations.
    ///
    /// - A fixed-property object: `type`, `properties`, `additionalProperties`
    ///   (it refuses an unlisted key), and `required` when a property is required.
    /// - A map: `type`, `propertyNames` (its keys' length), `additionalProperties`
    ///   (its value schema) and `maxProperties`.
    /// - A string and an identifier: `type`, `minLength` and `maxLength`.
    /// - An enum: `type` and `enum`.
    /// - An array: `type`, `items`, `minItems` and `maxItems`.
    fn enforced_keywords(kind: &SchemaKind) -> BTreeSet<&'static str> {
        let words: &[&'static str] = match kind {
            SchemaKind::Object { properties } => {
                if properties.iter().any(|property| property.required) {
                    &["type", "properties", "additionalProperties", "required"]
                } else {
                    &["type", "properties", "additionalProperties"]
                }
            }
            SchemaKind::Map { .. } => &[
                "type",
                "propertyNames",
                "additionalProperties",
                "maxProperties",
            ],
            SchemaKind::String { .. } | SchemaKind::Id => &["type", "minLength", "maxLength"],
            SchemaKind::Enum { .. } => &["type", "enum"],
            SchemaKind::Array { .. } => &["type", "items", "minItems", "maxItems"],
        };
        words.iter().copied().collect()
    }

    fn kind_name(kind: &SchemaKind) -> &'static str {
        match kind {
            SchemaKind::Object { .. } => "fixed-property object",
            SchemaKind::String { .. } => "string",
            SchemaKind::Enum { .. } => "enum",
            SchemaKind::Id => "identifier",
            SchemaKind::Array { .. } => "array",
            SchemaKind::Map { .. } => "map",
        }
    }

    /// Checks one published node against its tree node, then its children, by the
    /// kind of each.
    fn check_per_kind(schema: &Schema, node: &JsonObject, path: &str) -> Result<(), String> {
        let published: BTreeSet<&str> = node
            .keys()
            .map(String::as_str)
            .filter(|key| !ANNOTATIONS.contains(key))
            .collect();
        let enforced = enforced_keywords(schema.kind());
        if published != enforced {
            return Err(format!(
                "{path}: a {} node publishes {published:?} and the validator enforces {enforced:?}",
                kind_name(schema.kind())
            ));
        }
        let child = |value: Option<&Value>, at: &str| -> Result<(), String> {
            let Some(Value::Object(inner)) = value else {
                return Err(format!("{path}: no schema object at {at}"));
            };
            match schema.kind() {
                SchemaKind::Array { items, .. } => {
                    check_per_kind(items, inner, &format!("{path}/[]"))
                }
                SchemaKind::Map { value, .. } => check_per_kind(value, inner, &format!("{path}/*")),
                _ => Ok(()),
            }
        };
        match schema.kind() {
            SchemaKind::Object { properties } => {
                let Some(Value::Object(published_properties)) = node.get("properties") else {
                    return Err(format!("{path}: no properties"));
                };
                for property in properties {
                    let Some(Value::Object(inner)) = published_properties.get(property.name) else {
                        return Err(format!("{path}: {} is not published", property.name));
                    };
                    check_per_kind(
                        &property.schema,
                        inner,
                        &format!("{path}/{}", property.name),
                    )?;
                }
                Ok(())
            }
            SchemaKind::Array { .. } => child(node.get("items"), "items"),
            SchemaKind::Map { .. } => {
                let Some(Value::Object(names)) = node.get("propertyNames") else {
                    return Err(format!("{path}: no propertyNames"));
                };
                let keys: BTreeSet<&str> = names.keys().map(String::as_str).collect();
                if keys != BTreeSet::from(["minLength", "maxLength"]) {
                    return Err(format!("{path}: propertyNames publishes {keys:?}"));
                }
                child(node.get("additionalProperties"), "additionalProperties")
            }
            SchemaKind::String { .. } | SchemaKind::Enum { .. } | SchemaKind::Id => Ok(()),
        }
    }

    #[test]
    fn ori_t_0169_each_schema_node_publishes_only_the_keywords_the_validator_enforces_for_its_kind()
    {
        for spec in catalogue() {
            let schema = spec.schema();
            let published = schema.to_json();
            if let Err(why) = check_per_kind(schema, &published, spec.tool().wire_name()) {
                panic!("{why}");
            }
        }
        // The check is not vacuous: a keyword of another kind on a node is caught,
        // at the root and below it.
        let schema = spec(Tool::Report).schema();
        let mut planted = schema.to_json();
        planted.insert("maxProperties".to_owned(), Value::from(3));
        let error = check_per_kind(schema, &planted, "report").expect_err("a stray keyword");
        assert!(error.contains("maxProperties"), "{error}");
        let mut planted = schema.to_json();
        let Some(Value::Object(properties)) = planted.get_mut("properties") else {
            panic!("properties");
        };
        let Some(Value::Object(content)) = properties.get_mut("content") else {
            panic!("content");
        };
        content.remove("maxProperties");
        assert!(
            check_per_kind(schema, &planted, "report").is_err(),
            "a keyword the validator enforces and the node does not publish"
        );
        let mut planted = schema.to_json();
        let Some(Value::Object(properties)) = planted.get_mut("properties") else {
            panic!("properties");
        };
        let Some(Value::Object(kind)) = properties.get_mut("kind") else {
            panic!("kind");
        };
        kind.insert("minLength".to_owned(), Value::from(1));
        assert!(
            check_per_kind(schema, &planted, "report").is_err(),
            "an enum node publishing a string's keyword"
        );
    }
}
