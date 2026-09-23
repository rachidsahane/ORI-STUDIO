//! The keychain: AICD §27, `spec/SECURITY_NOTES.md` "Secrets", `spec/LLD.md`
//! section 2 (`ori-broker` owns "`Keychain` (keyring)").
//!
//! `spec/SECURITY_NOTES.md` "Secrets" states three rules this module answers
//! directly: "Only the broker touches the keychain. Secrets are never logged,
//! never serialized into events, never returned by any RPC" and "Credential
//! issuance is recorded without the secret." `spec/DATA_MODEL.md` section 2's
//! `ProviderBinding` row, "id, product_id (nullable = application default),
//! provider, key_ref (keychain), roles (json) | Per-project override when
//! product_id set", is the mechanism criterion ORI-P1-037 tests: "Provider key
//! set at application level; project B has an override | Spawn a coder in A
//! and in B | A receives the application key, B receives its override;
//! neither key appears in any event or log."
//!
//! # What is here
//!
//! - [`Secret`], a newtype that makes leaking the value it holds structurally
//!   hard rather than merely discouraged. See its own doc comment.
//! - [`KeyRef`], the reference `spec/DATA_MODEL.md` section 2 calls `key_ref`:
//!   a name for an entry in the keychain, never the entry's value.
//! - [`Keychain`], the trait, and its two implementations:
//!   [`KeyringBackedKeychain`] (the real one, over the `keyring` crate) and
//!   [`InMemoryKeychain`] (the fake, over a `Mutex<BTreeMap>`). Every test in
//!   this module runs against the fake. See "Why no test touches the real
//!   backend" below.
//! - [`ProviderBinding`] and [`resolve_provider_binding`], the
//!   application-default-with-per-project-override mechanism ORI-P1-037
//!   tests.
//! - [`record_binding_resolved`], which writes the event
//!   `spec/SECURITY_NOTES.md` "Secrets" asks for, "recorded without the
//!   secret", through `ori_store::event_log::EventLog::append`. Full
//!   `CredentialIssuance` (scope, expiry, revocation on session end) is
//!   ORI-T-0027's `issuance.rs`; this is the resolution-event half ORI-P1-037
//!   actually exercises, not the whole entity.
//!
//! # Why no test touches the real backend
//!
//! CLAUDE.md's absolute rule 4 binds the coder implementing this ticket, not
//! only the product: "You never read `.env*` files, the OS keychain, or any
//! file named in `spec/ENV_SETUP.md` as a secret location." That forbids
//! running anything, test included, that reads or writes the real OS
//! keychain on the machine this was implemented on. [`Keychain`] is the trait
//! that makes that possible without leaving [`KeyringBackedKeychain`] an
//! untested stub: every function in this module that needs a keychain takes
//! `&dyn Keychain` or a generic `K: Keychain`, so a test hands it
//! [`InMemoryKeychain`] and production code hands it
//! [`KeyringBackedKeychain`], and neither this module nor a caller can tell
//! the difference from the trait alone.
//!
//! This is not only a rule-compliance move. `keyring`'s own doc comment says
//! "There are no default features in this crate: you must specify explicitly
//! which platform-specific credential stores you intend to use. If no
//! specified credential store features apply to a given platform, this crate
//! will use the (platform-independent) mock credential store on that
//! platform", and on Linux specifically (`ori-broker`'s `Cargo.toml`, see its
//! own comment for why `linux-native` rather than the D-Bus Secret Service)
//! the real backend is the kernel's `keyutils` facility, which
//! `ubuntu-latest`, the Linux CI runner, has, but which a *test* run has no
//! business writing real secret-shaped bytes into regardless of whether the
//! call would succeed. A test written against the real backend on any
//! platform is why the trait exists rather than only a lone
//! `#[cfg(not(test))]` guard around the real calls: a guard can be forgotten
//! on one call site, a type that is never constructed as `dyn Keychain` in
//! test code cannot be exercised by accident.
//!
//! `tests::ori_t_0026_the_real_backend_constructs_without_touching_the_os_keychain`
//! is the one place this module's own test suite even names
//! [`KeyringBackedKeychain`]: it constructs one (which touches nothing; see
//! that type's doc comment) and calls no method on it. No other test in this
//! crate names the type at all.
//!
//! # The vacuity trap ORI-P1-037 sits on top of
//!
//! "Neither key appears in any event or log" is vacuously true of an empty
//! event set: a scan over nothing finds nothing, every time, including the
//! run where the code that should have produced the event was deleted. AICD
//! §39 names this "present but reporting nothing".
//! `tests::ori_p1_037_a_receives_the_application_key_and_b_receives_its_override_and_neither_key_appears_in_any_event_or_log`
//! is built against exactly that trap, in the order that closes it:
//!
//! 1. It drives the real flow: two real `ori_store::db::ProductDb`s, on disk
//!    in a scratch directory, and [`record_binding_resolved`] calling the
//!    real `ori_store::event_log::EventLog::append`, not a mock of either.
//! 2. It asks the store itself, not a counter this test kept, how many events
//!    exist (`ori_store::event_log::EventLog::verify`'s `events_checked`) and
//!    asserts it is exactly one per product, so a change that stopped
//!    producing the event fails this assertion loudly rather than leaving the
//!    scan below with nothing to find and nothing to say about it.
//! 3. Only then does it scan: every event payload read back through
//!    `ori_store::event_log::EventLog::read_range`, and, past that, the raw
//!    bytes of both products' `product.sqlite` files (and any `-wal`
//!    companion) read straight off disk, for the literal fake secret strings.
//!    "Any event or log" is read literally: not only the typed `Event` this
//!    process just built, but the durable file a human or a forensic tool
//!    would actually open.
//!
//! # Trust boundary
//!
//! ```mermaid
//! flowchart LR
//!   subgraph trusted["ori-broker (trusted, per spec/SECURITY_NOTES.md \"Auto mode\")"]
//!     R["resolve_provider_binding"] --> K["Keychain::get"]
//!     R --> E["record_binding_resolved\n(key_ref only)"]
//!   end
//!   K --> OS["KeyringBackedKeychain -> OS keychain\n(production)"]
//!   K -. test .-> FAKE["InMemoryKeychain\n(every test in this crate)"]
//!   E --> LOG["ori-store EventLog::append\n(product.sqlite, this product's log)"]
//!   OS -. "the secret itself" .-> SESSION["a session (ori-runtime, ORI-T-0027)"]
//!   LOG -. "key_ref, identity, time\nnever the secret" .-> AUDIT["anyone reading the log"]
//! ```
//!
//! Must not: persist secrets anywhere but the keychain (`spec/LLD.md`
//! section 2, inherited from the crate). [`InMemoryKeychain`] is the one
//! deliberate exception, and it exists only under `#[cfg(test)]`... except
//! that it does not: this crate's whole test suite needs it, so it is a
//! plain, always-compiled type, documented as being for tests, never
//! constructed by any non-test code in this crate. See its own doc comment
//! for why an actual `#[cfg(test)]` gate was rejected.

use core::fmt;
use std::collections::BTreeMap;
use std::sync::Mutex;

use ori_core::types::Actor;
use ori_core::types::Id;
use ori_core::types::Role;
use ori_core::types::Timestamp;
use ori_store::db::ProductDb;
use ori_store::event_log::Event;
use ori_store::event_log::EventLog;
use ori_store::event_log::EventLogError;

use crate::identity::AgentIdentity;

// ---------------------------------------------------------------------------
// Secret
// ---------------------------------------------------------------------------

/// A secret value, held so that leaking it by accident is structurally hard.
///
/// `spec/SECURITY_NOTES.md` "Secrets": "Secrets are never logged, never
/// serialized into events, never returned by any RPC." A type cannot stop a
/// caller from choosing to leak a value it holds; what it can do is remove
/// every path that leaks the value *without* the caller choosing to. This
/// type removes four:
///
/// 1. **No derived, and no hand-written leaking, `Debug`.** CLAUDE.md's own
///    instructions for this ticket name this explicitly: "Do not derive
///    `Debug`." [`Debug`] is implemented, by hand, and always writes
///    `"Secret(REDACTED)"`, so `format!("{secret:?}")`,
///    `assert_eq!(a, b)`'s failure message (were `PartialEq` implemented,
///    which it deliberately is not, below), and any `#[derive(Debug)]`
///    container that holds a `Secret` all redact it too, by construction:
///    Rust's derived `Debug` for a struct calls each field's own `Debug`, so
///    a struct holding a `Secret` inherits this redaction rather than needing
///    its own.
/// 2. **`Display` redacts as well.** Not asked for by the type system (only
///    `Debug` is ever derived by accident), but a `println!("{secret}")` or a
///    `format!` in a log line is exactly the leak `spec/SECURITY_NOTES.md`
///    names, and `Display` is the trait code reaches for to write a value
///    into a message meant for a human. Always writes `"REDACTED"`.
/// 3. **No `serde::Serialize`, and not because nobody wrote one.** This
///    crate's `Cargo.toml` depends on no serialization framework at all: no
///    `serde` line exists anywhere in this workspace's dependency graph for
///    `ori-broker`. There is therefore no trait named `serde::Serialize` in
///    scope for this type, or any type, in this crate to implement; writing
///    `impl serde::Serialize for Secret` would first need `serde` added as a
///    dependency, which is itself CLAUDE.md rule 6's `new_dependency`
///    escalation, not a decision this ticket makes. So the absence is not
///    "nobody got around to it" but "the trait this type would need to
///    implement is not nameable from this crate's source at all": structural,
///    not a missing `impl` block a future edit could add without first
///    crossing an escalation.
/// 4. **No path out into an owned `String` except through
///    [`Secret::expose`].** No `Deref<Target = str>`, no `AsRef<str>`, no
///    `Into<String>` or `From<Secret> for String`, no public field. The only
///    way to read the bytes is [`Secret::expose`], which hands a `&str` (not
///    an owned value) to a caller-supplied closure and returns only what the
///    closure returns: `let leaked: String = secret.expose(|s| s.to_owned());`
///    still compiles, because a closure that chooses to copy the bytes out
///    can always do so, but it is a visible, explicit act at the call site
///    rather than something `let x = secret;`, a struct update, or a `?`
///    could do by accident. `secret.0` does not compile from outside this
///    module: the field is private.
///
/// What this type does not attempt: zeroing its buffer on [`Drop`]. That
/// would need either `unsafe` code this module has no other reason to carry,
/// or the `zeroize` crate, a dependency this ticket does not escalate for
/// (CLAUDE.md rule 6) when nothing in ORI-P1-037 or `spec/SECURITY_NOTES.md`
/// "Secrets" asks for it. Noted here rather than left for a reader to
/// discover, and in this ticket's report as a possible follow-up.
pub struct Secret(Box<str>);

impl Secret {
    /// Holds `value` as a secret.
    #[must_use]
    pub fn new(value: impl Into<Box<str>>) -> Self {
        Self(value.into())
    }

    /// Hands the secret's bytes, as `&str`, to `f`, and returns only what `f`
    /// returns. The one way to read this value; see the type's own doc
    /// comment, point 4.
    pub fn expose<R>(&self, f: impl FnOnce(&str) -> R) -> R {
        f(&self.0)
    }

    /// The length of the secret in bytes, which is not itself the secret and
    /// is occasionally useful for a caller checking "something was stored"
    /// without reading what.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the secret is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(REDACTED)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("REDACTED")
    }
}

// ---------------------------------------------------------------------------
// KeyRef
// ---------------------------------------------------------------------------

/// A reference to an entry in the keychain: `spec/DATA_MODEL.md` section 2's
/// `key_ref`, on `ProviderBinding` and (as `credential_ref`) on `Integration`
/// and `McpServer`.
///
/// Not secret-shaped: `spec/SECURITY_NOTES.md` "Secrets" is a rule about the
/// value an entry holds, never about the name used to look the entry up, so
/// this derives an ordinary [`fmt::Debug`] and [`fmt::Display`], unlike
/// [`Secret`].
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KeyRef(String);

impl KeyRef {
    /// Reads a key reference, refusing one that is empty or only whitespace.
    pub fn parse(text: impl Into<String>) -> Result<Self, KeychainError> {
        let text = text.into();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(KeychainError::Malformed {
                what: "KeyRef",
                value: text,
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The reference as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for KeyRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// KeychainError
// ---------------------------------------------------------------------------

/// A failure from [`Keychain`] or from parsing one of this module's values.
///
/// None of these is a refusal in `ori_core::error::Error`'s sense (a control
/// refusing a governed action): a keychain that has no entry for a key, or
/// that failed for a platform reason, is an operational failure the same way
/// `ori_store::event_log::EventLogError::Sql` is, not a policy decision, so
/// nothing here carries a `MethodologyRef`.
#[derive(Debug)]
#[non_exhaustive]
pub enum KeychainError {
    /// A value did not parse into the type or shape named by `what`.
    Malformed {
        /// The field or type the value was read as.
        what: &'static str,
        /// The value as it was given.
        value: String,
    },
    /// No entry exists for this key reference.
    NotFound {
        /// The key reference that was looked up.
        key_ref: KeyRef,
    },
    /// The backend (the OS keychain, or the fake standing in for it) refused
    /// or failed the call for a reason of its own.
    ///
    /// `message` is built with [`ToString`]/[`fmt::Display`] on the
    /// underlying error, never with [`fmt::Debug`]: the `keyring` crate's own
    /// `Error::BadEncoding(Vec<u8>)` variant carries the raw bytes that failed
    /// to decode as UTF-8 (which, for this module's use, could be a secret's
    /// bytes), and its `Debug` implementation prints them, while its
    /// `Display` implementation deliberately does not (`"Data is not UTF-8
    /// encoded"`, checked directly against `keyring`'s own source before this
    /// choice was made, not assumed). Formatting only ever goes through
    /// `Display` here for exactly that reason: it is the one keyring itself
    /// already redacts.
    Backend {
        /// The key reference the call was against.
        key_ref: KeyRef,
        /// The underlying failure, via `Display`, never `Debug`.
        message: String,
    },
}

impl fmt::Display for KeychainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { what, value } => write!(f, "not a valid {what}: {value:?}"),
            Self::NotFound { key_ref } => write!(f, "no keychain entry for {key_ref}"),
            Self::Backend { key_ref, message } => {
                write!(f, "keychain backend failure for {key_ref}: {message}")
            }
        }
    }
}

impl std::error::Error for KeychainError {}

// ---------------------------------------------------------------------------
// Keychain
// ---------------------------------------------------------------------------

/// The keychain: `spec/LLD.md` section 2 ("`Keychain` (keyring)"),
/// `spec/SECURITY_NOTES.md` "Secrets" ("Only the broker touches the
/// keychain").
///
/// One trait, two implementations: [`KeyringBackedKeychain`] over the real OS
/// store, [`InMemoryKeychain`] over nothing but process memory. See this
/// module's own doc comment, "Why no test touches the real backend".
pub trait Keychain {
    /// Stores `secret` under `key_ref`, overwriting any value already there.
    fn set(&self, key_ref: &KeyRef, secret: Secret) -> Result<(), KeychainError>;

    /// Reads the secret stored under `key_ref`,
    /// [`KeychainError::NotFound`] if none is.
    fn get(&self, key_ref: &KeyRef) -> Result<Secret, KeychainError>;

    /// Removes the entry at `key_ref`, [`KeychainError::NotFound`] if none is
    /// there to remove.
    fn delete(&self, key_ref: &KeyRef) -> Result<(), KeychainError>;
}

// ---------------------------------------------------------------------------
// KeyringBackedKeychain
// ---------------------------------------------------------------------------

/// The real keychain, over the OS-level secure store, through the `keyring`
/// crate: AICD §27, `ops/escalations/E-0004-external-crates.md`.
///
/// # Never exercised in this crate's test suite, by construction
///
/// Constructing a [`KeyringBackedKeychain`] touches nothing: it only stores
/// the fixed `service` name every entry it opens will share, a plain `String`
/// held in memory. The OS keychain is touched only inside
/// [`Keychain::set`], [`Keychain::get`] and [`Keychain::delete`], each of
/// which builds one `keyring::Entry` and calls exactly one of
/// `set_password`/`get_password`/`delete_credential` on it. No other function
/// in this module, and no test in this crate, calls any of the three. The one
/// test that names this type at all,
/// `tests::ori_t_0026_the_real_backend_constructs_without_touching_the_os_keychain`,
/// constructs one and calls no method on it, which is exactly what
/// CLAUDE.md's absolute rule 4 and this module's own doc comment, "Why no
/// test touches the real backend", require and no more.
#[derive(Debug, Default)]
pub struct KeyringBackedKeychain {
    /// The `service` half of `keyring::Entry::new(service, user)`, constant
    /// across every entry this keychain opens; `user` is the call's
    /// [`KeyRef`], so two different key references never collide within one
    /// service namespace, and every product this engine runs shares one
    /// keychain service the way `spec/SECURITY_NOTES.md` "Configuration
    /// scopes and trust" describes ("held once in the OS keychain and used by
    /// every project's engine through the broker; project-level overrides are
    /// separate keychain entries", and the separateness comes from `user`,
    /// this module's `KeyRef`, not from a second service name).
    service: String,
}

impl KeyringBackedKeychain {
    /// The `service` name every entry this keychain opens is stored under.
    const SERVICE: &'static str = "ori-studio";

    /// Builds a keychain over the OS-level secure store. Touches nothing:
    /// see this type's own doc comment.
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: Self::SERVICE.to_owned(),
        }
    }

    fn entry(&self, key_ref: &KeyRef) -> Result<keyring::Entry, KeychainError> {
        keyring::Entry::new(&self.service, key_ref.as_str()).map_err(|source| {
            KeychainError::Backend {
                key_ref: key_ref.clone(),
                message: source.to_string(),
            }
        })
    }
}

impl Keychain for KeyringBackedKeychain {
    fn set(&self, key_ref: &KeyRef, secret: Secret) -> Result<(), KeychainError> {
        let entry = self.entry(key_ref)?;
        secret
            .expose(|plain| entry.set_password(plain))
            .map_err(|source| KeychainError::Backend {
                key_ref: key_ref.clone(),
                message: source.to_string(),
            })
    }

    fn get(&self, key_ref: &KeyRef) -> Result<Secret, KeychainError> {
        let entry = self.entry(key_ref)?;
        match entry.get_password() {
            Ok(plain) => Ok(Secret::new(plain)),
            Err(keyring::Error::NoEntry) => Err(KeychainError::NotFound {
                key_ref: key_ref.clone(),
            }),
            // Display, never Debug: see KeychainError::Backend's own doc
            // comment for why that is load-bearing here specifically.
            Err(source) => Err(KeychainError::Backend {
                key_ref: key_ref.clone(),
                message: source.to_string(),
            }),
        }
    }

    fn delete(&self, key_ref: &KeyRef) -> Result<(), KeychainError> {
        let entry = self.entry(key_ref)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Err(KeychainError::NotFound {
                key_ref: key_ref.clone(),
            }),
            Err(source) => Err(KeychainError::Backend {
                key_ref: key_ref.clone(),
                message: source.to_string(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// InMemoryKeychain
// ---------------------------------------------------------------------------

/// The fake keychain: a `Mutex<BTreeMap<String, Box<str>>>`, nothing else.
///
/// For tests, in this crate and (once `issuance.rs`, `family.rs` and
/// `forbidden.rs` exist) any other in this workspace that needs a
/// [`Keychain`] without touching the OS keychain. Not gated behind
/// `#[cfg(test)]`: this crate's own `#[cfg(test)] mod tests` blocks are what
/// need it, and a `#[cfg(test)]` item is visible only within the crate that
/// declares it under that configuration, at the configuration the *user* of
/// the item was compiled under: an external crate's own `#[cfg(test)]`
/// integration tests cannot see an item another crate marked `#[cfg(test)]`
/// even when both are compiled as tests, so gating this would leave a future
/// sibling crate's tests with no fake to depend on and one more reason to
/// invent their own. It stays a plain, always-compiled type instead, and
/// nothing in this module's non-test code ever constructs one; grep for
/// `InMemoryKeychain::new` outside `#[cfg(test)]` in this crate to check.
#[derive(Debug, Default)]
pub struct InMemoryKeychain {
    entries: Mutex<BTreeMap<String, Box<str>>>,
}

impl InMemoryKeychain {
    /// An empty fake keychain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Box<str>>> {
        match self.entries.lock() {
            Ok(guard) => guard,
            // This crate never panics while the lock is held (every critical
            // section below is a plain BTreeMap operation that cannot
            // itself panic on the keys and values this module builds), so a
            // poisoned lock never legitimately arises; recovering the guard
            // rather than propagating a second, unrelated panic is standard
            // practice for a lock this module alone touches.
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl Keychain for InMemoryKeychain {
    fn set(&self, key_ref: &KeyRef, secret: Secret) -> Result<(), KeychainError> {
        let mut entries = self.lock();
        secret.expose(|plain| {
            entries.insert(key_ref.as_str().to_owned(), Box::from(plain));
        });
        Ok(())
    }

    fn get(&self, key_ref: &KeyRef) -> Result<Secret, KeychainError> {
        let entries = self.lock();
        match entries.get(key_ref.as_str()) {
            Some(value) => Ok(Secret::new(value.clone())),
            None => Err(KeychainError::NotFound {
                key_ref: key_ref.clone(),
            }),
        }
    }

    fn delete(&self, key_ref: &KeyRef) -> Result<(), KeychainError> {
        let mut entries = self.lock();
        match entries.remove(key_ref.as_str()) {
            Some(_) => Ok(()),
            None => Err(KeychainError::NotFound {
                key_ref: key_ref.clone(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// ProviderBinding
// ---------------------------------------------------------------------------

/// A provider key binding: `spec/DATA_MODEL.md` section 2, "id, product_id
/// (nullable = application default), provider, key_ref (keychain), roles
/// (json) | Per-project override when product_id set."
///
/// `product_id: None` is the application-level default
/// (`spec/ENV_SETUP.md` section 3, "Application (all windows, all projects) |
/// ... provider keys (`PROVIDER_<NAME>_API_KEY`)"); `product_id: Some(_)` is
/// the same section's per-project override. `roles` is carried because the
/// row names it, but [`resolve_provider_binding`] does not filter by it: no
/// accepted criterion exercises a role-scoped provider binding today, and
/// this module would rather leave that filter unwritten than guess at a rule
/// `spec/ENV_SETUP.md` and `spec/DATA_MODEL.md` do not state, which is the
/// same restraint `ori_store::event_log`'s module doc comment names for
/// `payload`'s shape: "leaves which actor may carry which kind to whichever
/// crate decides".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderBinding {
    id: Id,
    product_id: Option<Id>,
    provider: String,
    key_ref: KeyRef,
    roles: Vec<Role>,
}

impl ProviderBinding {
    /// Reads a binding, refusing an empty or whitespace-only `provider`.
    pub fn new(
        id: Id,
        product_id: Option<Id>,
        provider: impl Into<String>,
        key_ref: KeyRef,
        roles: Vec<Role>,
    ) -> Result<Self, KeychainError> {
        let provider = provider.into();
        let trimmed = provider.trim();
        if trimmed.is_empty() {
            return Err(KeychainError::Malformed {
                what: "provider",
                value: provider,
            });
        }
        Ok(Self {
            id,
            product_id,
            provider: trimmed.to_owned(),
            key_ref,
            roles,
        })
    }

    /// The binding identifier.
    #[must_use]
    pub const fn id(&self) -> &Id {
        &self.id
    }

    /// The product this binding overrides for, absent for the
    /// application-level default.
    #[must_use]
    pub const fn product_id(&self) -> Option<&Id> {
        self.product_id.as_ref()
    }

    /// The provider name this binding is for (`"openai"`, `"anthropic"`, and
    /// so on; `spec/ENV_SETUP.md` section 4's `PROVIDER_<NAME>_API_KEY`
    /// naming, lower cased, is the convention, not a rule this type checks).
    #[must_use]
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// The keychain reference this binding resolves to. Never the secret
    /// itself.
    #[must_use]
    pub const fn key_ref(&self) -> &KeyRef {
        &self.key_ref
    }

    /// The roles this binding names, carried but not yet enforced; see this
    /// type's own doc comment.
    #[must_use]
    pub fn roles(&self) -> &[Role] {
        &self.roles
    }
}

/// Resolves which binding a session for `product_id` and `provider` receives:
/// the product-scoped override when one exists, the application-level
/// default otherwise. `spec/DATA_MODEL.md` section 2's `ProviderBinding` row,
/// "Per-project override when product_id set", and the mechanism criterion
/// ORI-P1-037 tests directly: "Provider key set at application level; project
/// B has an override ... A receives the application key, B receives its
/// override."
///
/// Pure: reads `bindings`, decides nothing about a keychain or an event. The
/// first binding in iteration order that matches wins for each of the two
/// passes (override, then default); `bindings` is expected to hold at most
/// one of each per `(product_id, provider)` pair, and nothing here enforces
/// that uniqueness, the same way `ori_store::event_log` does not validate
/// `payload` as JSON: a store for configuration is not this function's job.
#[must_use]
pub fn resolve_provider_binding<'a>(
    bindings: &'a [ProviderBinding],
    product_id: &Id,
    provider: &str,
) -> Option<&'a ProviderBinding> {
    bindings
        .iter()
        .find(|binding| {
            binding.product_id.as_ref() == Some(product_id) && binding.provider == provider
        })
        .or_else(|| {
            bindings
                .iter()
                .find(|binding| binding.product_id.is_none() && binding.provider == provider)
        })
}

// ---------------------------------------------------------------------------
// record_binding_resolved
// ---------------------------------------------------------------------------

/// Records that `binding` was resolved for `identity`, without the secret:
/// `spec/SECURITY_NOTES.md` "Secrets", "Credential issuance is recorded
/// without the secret."
///
/// Writes one event, through `ori_store::event_log::EventLog::append`, into
/// `identity`'s own product's log (`identity.product_id()`; `EventLog::append`
/// itself refuses a second `product_id` appearing in one product's log, so
/// this could not target the wrong file even if given the wrong identity).
/// The payload holds `identity_id`, `role`, `provider`, `key_ref` and
/// `resolved_at`; `key_ref` is [`KeyRef::as_str`], a reference, never
/// [`Secret::expose`]'d, so there is no local binding in this function's body
/// that ever holds the secret's bytes at all, let alone one that could reach
/// the payload.
///
/// `actor` is the caller's to supply rather than fixed to [`Actor::System`]
/// here: `spec/DATA_MODEL.md` section 4 restricts `system` to "scheduled
/// triggers and watchers", and a binding resolved as part of spawning a
/// session is neither on its own; the caller that actually knows why this
/// resolution happened (a human assigning a ticket, an agent's own session
/// requesting a respawn, ORI-T-0027's issuance path) is the one positioned to
/// say who or what that was.
///
/// This is the resolution-event half of `CredentialIssuance`
/// (`spec/DATA_MODEL.md` section 2) ORI-P1-037 exercises. The full entity,
/// `scope`, `expires_at`, `revoked_at`, one row bound to one session, is
/// ORI-T-0027's `issuance.rs`, outside this ticket's declared scope
/// (`crates/ori-broker/src/identity.rs`, `keychain.rs`).
///
/// # Errors
///
/// Whatever `ori_store::event_log::EventLog::append` returns; see that
/// function's own documentation.
pub fn record_binding_resolved(
    db: &mut ProductDb,
    at: Timestamp,
    actor: Actor,
    identity: &AgentIdentity,
    binding: &ProviderBinding,
) -> Result<Event, EventLogError> {
    let payload = binding_resolved_payload(at, identity, binding);
    EventLog::append(
        db.connection(),
        identity.product_id().clone(),
        at,
        actor,
        "credential.bound",
        None,
        payload,
    )
}

/// The payload [`record_binding_resolved`] writes: `identity_id`, `role`,
/// `provider`, `key_ref`, `resolved_at`, and nothing else. No field here is
/// ever a [`Secret`]; the type system already makes that hard to get wrong
/// (see [`Secret`]'s own doc comment, point 4), and this function additionally
/// never binds a `Secret` or the output of [`Secret::expose`] to any local
/// variable, so there is no value in this function's body a reviewer would
/// need to trace to confirm it was excluded.
fn binding_resolved_payload(
    at: Timestamp,
    identity: &AgentIdentity,
    binding: &ProviderBinding,
) -> String {
    format!(
        "{{\"identity_id\":\"{}\",\"role\":\"{}\",\"provider\":\"{}\",\"key_ref\":\"{}\",\"resolved_at\":{}}}",
        json_escape(identity.id().as_str()),
        identity.role(),
        json_escape(binding.provider()),
        json_escape(binding.key_ref().as_str()),
        at.millis(),
    )
}

/// Escapes `"` and `\` for the small hand-written JSON payload above. This
/// module has no JSON parser or serializer as a dependency (adding one is a
/// `new_dependency` escalation this ticket does not raise), the same
/// deliberate choice `ori_store::event_log` documents for `payload`: stored
/// and built as text, never interpreted as a data structure by this crate.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;
    use std::sync::atomic::Ordering;

    use ori_core::types::Actor;
    use ori_core::types::ModelFamily;

    use super::*;
    use crate::identity::AgentIdentity;
    use crate::identity::IdentityRuntime;
    use crate::identity::MemoryScopes;

    /// A declared model family for a test that does not care which, added by
    /// ORI-T-0108 alongside `AgentIdentity::create`'s new required
    /// parameter.
    fn family(text: &str) -> ModelFamily {
        ModelFamily::parse(text).expect("a non-empty family parses")
    }

    // -----------------------------------------------------------------------
    // Scratch layout, the same shape crates/ori-store/src/db.rs's own tests
    // use for a fresh, unique, self-cleaning temporary products root.
    // -----------------------------------------------------------------------

    struct Scratch {
        path: PathBuf,
    }

    impl Scratch {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
            let root = std::env::temp_dir();
            let path = root.join(format!(
                "ori-t-0026-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("a fresh scratch directory can be created");
            Self { path }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// A valid ULID, distinguished by `label`, the same sanitizing scheme
    /// `crates/ori-store/src/event_log.rs`'s own tests use so a readable word
    /// can stand in for a ULID.
    fn id(label: &str) -> Id {
        let mut safe = String::with_capacity(25);
        for ch in label.chars().take(25) {
            let mapped = match ch.to_ascii_uppercase() {
                upper @ ('0'..='9' | 'A'..='H' | 'J' | 'K' | 'M' | 'N' | 'P'..='T' | 'V'..='Z') => {
                    upper
                }
                'I' | 'L' => '1',
                'O' => '0',
                'U' => 'V',
                _ => '0',
            };
            safe.push(mapped);
        }
        Id::parse(&format!("0{safe:0>25}"))
            .expect("a sanitized, zero-padded, '0'-prefixed 26-character string always parses")
    }

    fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    // -----------------------------------------------------------------------
    // Secret
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0026_secret_debug_is_redacted() {
        let secret = Secret::new("fake-token-for-tests");
        assert_eq!(format!("{secret:?}"), "Secret(REDACTED)");
        assert!(!format!("{secret:?}").contains("fake-token-for-tests"));
    }

    #[test]
    fn ori_t_0026_secret_display_is_redacted() {
        let secret = Secret::new("fake-token-for-tests");
        assert_eq!(secret.to_string(), "REDACTED");
        assert!(!secret.to_string().contains("fake-token-for-tests"));
    }

    #[test]
    fn ori_t_0026_secret_is_read_only_through_expose() {
        let secret = Secret::new("fake-token-for-tests");
        let read_back = secret.expose(|s| s.to_owned());
        assert_eq!(read_back, "fake-token-for-tests");
        assert_eq!(secret.len(), "fake-token-for-tests".len());
        assert!(!secret.is_empty());
    }

    // -----------------------------------------------------------------------
    // InMemoryKeychain
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0026_in_memory_keychain_round_trips_set_and_get() {
        let keychain = InMemoryKeychain::new();
        let key_ref = KeyRef::parse("ori/application/provider/openai").expect("a key ref");
        keychain
            .set(&key_ref, Secret::new("fake-app-key-for-tests"))
            .expect("set succeeds");
        let secret = keychain.get(&key_ref).expect("get succeeds");
        secret.expose(|s| assert_eq!(s, "fake-app-key-for-tests"));
    }

    #[test]
    fn ori_t_0026_in_memory_keychain_get_of_a_missing_key_is_not_found() {
        let keychain = InMemoryKeychain::new();
        let key_ref = KeyRef::parse("ori/application/provider/never-set").expect("a key ref");
        let err = keychain.get(&key_ref).expect_err("nothing was ever set");
        assert!(matches!(err, KeychainError::NotFound { .. }));
    }

    #[test]
    fn ori_t_0026_in_memory_keychain_delete_removes_the_entry() {
        let keychain = InMemoryKeychain::new();
        let key_ref = KeyRef::parse("ori/application/provider/openai").expect("a key ref");
        keychain
            .set(&key_ref, Secret::new("fake-app-key-for-tests"))
            .expect("set succeeds");
        keychain.delete(&key_ref).expect("delete succeeds");
        let err = keychain.get(&key_ref).expect_err("deleted, so not found");
        assert!(matches!(err, KeychainError::NotFound { .. }));
    }

    #[test]
    fn ori_t_0026_in_memory_keychain_set_overwrites_a_previous_value() {
        let keychain = InMemoryKeychain::new();
        let key_ref = KeyRef::parse("ori/application/provider/openai").expect("a key ref");
        keychain
            .set(&key_ref, Secret::new("fake-first-value"))
            .expect("first set");
        keychain
            .set(&key_ref, Secret::new("fake-second-value"))
            .expect("second set overwrites");
        let secret = keychain.get(&key_ref).expect("get succeeds");
        secret.expose(|s| assert_eq!(s, "fake-second-value"));
    }

    // -----------------------------------------------------------------------
    // KeyringBackedKeychain: constructed, never exercised. See the module
    // doc comment, "Why no test touches the real backend".
    // -----------------------------------------------------------------------

    #[test]
    fn ori_t_0026_the_real_backend_constructs_without_touching_the_os_keychain() {
        let real = KeyringBackedKeychain::new();
        // Proving this is real code, not a stub, without calling set, get or
        // delete: it names the fixed service string this module documents,
        // and nothing about reading that touches the OS keychain.
        assert_eq!(real.service, KeyringBackedKeychain::SERVICE);
        // Constructing an Entry from it does not touch the OS keychain
        // either (keyring's own design: an Entry is an identifier, not a
        // connection; the syscall happens inside set_password/get_password/
        // delete_credential, none of which this test calls). This is the
        // furthest this suite goes with the real type.
        let _entry_only_never_called = real.entry(&KeyRef::parse("unused").expect("a key ref"));
    }

    // -----------------------------------------------------------------------
    // resolve_provider_binding
    // -----------------------------------------------------------------------

    fn app_binding(provider: &str, key_ref: &str) -> ProviderBinding {
        ProviderBinding::new(
            id("BINDING-APP"),
            None,
            provider,
            KeyRef::parse(key_ref).expect("a key ref"),
            Vec::new(),
        )
        .expect("a valid application-level binding")
    }

    fn product_binding(product_id: Id, provider: &str, key_ref: &str) -> ProviderBinding {
        ProviderBinding::new(
            id("BINDING-PROD"),
            Some(product_id),
            provider,
            KeyRef::parse(key_ref).expect("a key ref"),
            Vec::new(),
        )
        .expect("a valid per-project binding")
    }

    #[test]
    fn ori_t_0026_resolve_provider_binding_prefers_the_product_override() {
        let product_b = id("PRODUCT-B");
        let bindings = vec![
            app_binding("openai", "ori/application/provider/openai"),
            product_binding(product_b.clone(), "openai", "ori/product/b/provider/openai"),
        ];
        let resolved =
            resolve_provider_binding(&bindings, &product_b, "openai").expect("a binding resolves");
        assert_eq!(resolved.key_ref().as_str(), "ori/product/b/provider/openai");
        assert_eq!(resolved.product_id(), Some(&product_b));
    }

    #[test]
    fn ori_t_0026_resolve_provider_binding_falls_back_to_the_application_default() {
        let product_a = id("PRODUCT-A");
        let product_b = id("PRODUCT-B");
        let bindings = vec![
            app_binding("openai", "ori/application/provider/openai"),
            product_binding(product_b, "openai", "ori/product/b/provider/openai"),
        ];
        // Product A has no override, so it must receive the application key.
        let resolved =
            resolve_provider_binding(&bindings, &product_a, "openai").expect("a binding resolves");
        assert_eq!(
            resolved.key_ref().as_str(),
            "ori/application/provider/openai"
        );
        assert_eq!(resolved.product_id(), None);
    }

    #[test]
    fn ori_t_0026_resolve_provider_binding_returns_none_when_neither_exists() {
        let product_a = id("PRODUCT-A");
        let bindings = vec![app_binding(
            "anthropic",
            "ori/application/provider/anthropic",
        )];
        assert!(resolve_provider_binding(&bindings, &product_a, "openai").is_none());
    }

    // -----------------------------------------------------------------------
    // ORI-P1-037. Read this test's own steps against the module doc
    // comment's "The vacuity trap ORI-P1-037 sits on top of" before changing
    // it.
    // -----------------------------------------------------------------------

    /// Obviously fake: never a value that resembles a real provider key.
    const APP_SECRET: &str = "fake-application-openai-key-for-tests-AAA111";
    /// Obviously fake, and distinct from [`APP_SECRET`] so a test that
    /// accidentally compared the wrong pair could not pass by coincidence.
    const PROJECT_B_SECRET: &str = "fake-project-b-override-openai-key-for-tests-BBB222";

    #[test]
    fn ori_p1_037_a_receives_the_application_key_and_b_receives_its_override_and_neither_key_appears_in_any_event_or_log()
     {
        let scratch = Scratch::new("provider-binding");
        let product_a = id("PRODUCT-A");
        let product_b = id("PRODUCT-B");

        // --- set up: one keychain (the fake), two secrets, two bindings ---
        let keychain = InMemoryKeychain::new();
        let app_key_ref = KeyRef::parse("ori/application/provider/openai").expect("a key ref");
        let b_key_ref = KeyRef::parse("ori/product/b/provider/openai").expect("a key ref");
        keychain
            .set(&app_key_ref, Secret::new(APP_SECRET))
            .expect("the application key is stored");
        keychain
            .set(&b_key_ref, Secret::new(PROJECT_B_SECRET))
            .expect("project B's override is stored");

        let bindings = vec![
            app_binding("openai", app_key_ref.as_str()),
            product_binding(product_b.clone(), "openai", b_key_ref.as_str()),
        ];

        // --- "spawn a coder in A and in B" ---
        let identity_a = AgentIdentity::create(
            id("IDENTITY-A"),
            product_a.clone(),
            Role::Coder,
            "fake-test-model",
            IdentityRuntime::Headless,
            MemoryScopes::default(),
            family("fake-test-family"),
        )
        .expect("identity A is created");
        let identity_b = AgentIdentity::create(
            id("IDENTITY-B"),
            product_b.clone(),
            Role::Coder,
            "fake-test-model",
            IdentityRuntime::Headless,
            MemoryScopes::default(),
            family("fake-test-family"),
        )
        .expect("identity B is created");

        let resolved_a = resolve_provider_binding(&bindings, &product_a, "openai")
            .expect("A resolves a binding");
        let resolved_b = resolve_provider_binding(&bindings, &product_b, "openai")
            .expect("B resolves a binding");

        // --- "A receives the application key, B receives its override" ---
        assert_eq!(
            resolved_a.key_ref(),
            &app_key_ref,
            "A must not receive B's override"
        );
        assert_eq!(
            resolved_b.key_ref(),
            &b_key_ref,
            "B must receive its own override"
        );
        assert_ne!(resolved_a.key_ref(), resolved_b.key_ref());

        keychain
            .get(resolved_a.key_ref())
            .expect("A's key ref resolves in the keychain")
            .expose(|s| assert_eq!(s, APP_SECRET, "A must receive the application key"));
        keychain
            .get(resolved_b.key_ref())
            .expect("B's key ref resolves in the keychain")
            .expose(|s| {
                assert_eq!(
                    s, PROJECT_B_SECRET,
                    "B must receive its override, not A's key"
                )
            });

        // --- step 1: drive the real flow. Two real ProductDbs, two real
        // EventLog::append calls, through record_binding_resolved. ---
        let mut db_a = ProductDb::open(&scratch.path, product_a.as_str(), at(1_000))
            .expect("product A's database opens");
        let mut db_b = ProductDb::open(&scratch.path, product_b.as_str(), at(1_000))
            .expect("product B's database opens");

        let actor_a = Actor::Agent(identity_a.id().clone());
        let actor_b = Actor::Agent(identity_b.id().clone());
        record_binding_resolved(&mut db_a, at(2_000), actor_a, &identity_a, resolved_a)
            .expect("A's resolution is recorded");
        record_binding_resolved(&mut db_b, at(2_000), actor_b, &identity_b, resolved_b)
            .expect("B's resolution is recorded");

        // --- step 2: assert the produced set is non-empty, and its exact
        // size, asked of the store itself rather than a counter this test
        // kept. If event production were disabled (planted defect 6), these
        // two assertions are what catch it: events_checked would be 0, not
        // 1, and the scan below would never even run over the vacuous case
        // undetected. ---
        let report_a = EventLog::verify(db_a.connection()).expect("A's log verifies");
        let report_b = EventLog::verify(db_b.connection()).expect("B's log verifies");
        assert_eq!(
            report_a.events_checked, 1,
            "product A's log must hold exactly the one event just recorded"
        );
        assert_eq!(
            report_b.events_checked, 1,
            "product B's log must hold exactly the one event just recorded"
        );

        // --- step 3: scan every produced byte. First the typed events read
        // back through the store's own reader... ---
        let events_a =
            EventLog::read_range(db_a.connection(), 1, report_a.tip_seq.expect("A has a tip"))
                .expect("A's events read back");
        let events_b =
            EventLog::read_range(db_b.connection(), 1, report_b.tip_seq.expect("B has a tip"))
                .expect("B's events read back");
        assert_eq!(events_a.len(), 1);
        assert_eq!(events_b.len(), 1);
        for event in events_a.iter().chain(events_b.iter()) {
            let payload = event.payload();
            assert!(
                !payload.contains(APP_SECRET),
                "the application key leaked into an event payload: {payload}"
            );
            assert!(
                !payload.contains(PROJECT_B_SECRET),
                "project B's key leaked into an event payload: {payload}"
            );
            // It should, however, actually say something: a payload that
            // leaked nothing because it said nothing would make the two
            // assertions above pass for the wrong reason.
            assert!(payload.contains(app_key_ref.as_str()) || payload.contains(b_key_ref.as_str()));
        }

        // ...then the raw bytes of every file this flow could possibly have
        // touched: "any event or log" read literally, not only the Event
        // this process just built and not only the two product.sqlite paths
        // this test happens to know about. drive_the_test_scan_whole_tree
        // walks scratch.path recursively, so a defect that wrote the secret
        // to some other file under the product directory (a debug log, a
        // stray temp file) is caught the same way a leak into product.sqlite
        // itself is, rather than only the file this test predicted.
        drop(db_a);
        drop(db_b);
        let on_disk = read_every_file_under(&scratch.path);
        assert!(
            !on_disk.is_empty(),
            "the durable log files must have actually been written to"
        );
        let haystack = String::from_utf8_lossy(&on_disk);
        assert!(
            !haystack.contains(APP_SECRET),
            "the application key leaked into a file on disk under the products root"
        );
        assert!(
            !haystack.contains(PROJECT_B_SECRET),
            "project B's key leaked into a file on disk under the products root"
        );
    }

    /// Every byte of every regular file under `root`, concatenated,
    /// recursing into subdirectories. Used only by the leak test above, to
    /// scan the whole products root rather than only the specific
    /// `product.sqlite` paths this test happens to already know about: a
    /// defect that wrote a secret to any other file under `root` (not only
    /// the event log) is what this makes catchable. A directory entry this
    /// function cannot read (a permissions error, a symlink cycle) is
    /// skipped rather than panicking the test on an unrelated filesystem
    /// condition; the assertion that `root` produced *some* bytes at all is
    /// what stands in for "the walk actually visited real files".
    fn read_every_file_under(root: &std::path::Path) -> Vec<u8> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    stack.push(path);
                } else if file_type.is_file()
                    && let Ok(bytes) = fs::read(&path)
                {
                    out.extend(bytes);
                }
            }
        }
        out
    }
}
