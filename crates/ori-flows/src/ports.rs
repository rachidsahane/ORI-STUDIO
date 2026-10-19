//! Shared flow ports used by later slices: AICD §17.
//!
//! [`IdSource`], [`Clock`] and [`PortError`] match the shapes in
//! `crates/ori-mcp/src/ports.rs` so the engine can implement one pattern for
//! every crate that appends product events.

use core::fmt;

use ori_core::error::{Error, MethodologyRef};
use ori_core::types::{Id, Timestamp};

/// Why a port could not do what it was asked: AICD §17.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PortError {
    /// The component behind the port refused with an `ori-core` error.
    Refused(Error),
    /// The component refused under a rule `ori-core` has no variant for.
    Denied {
        /// Fixed refusal code (MCP-style reason, not an `E_` code).
        code: &'static str,
        /// Methodology section the refusal cites.
        methodology_ref: MethodologyRef,
    },
    /// An argument was not acceptable.
    Invalid {
        /// Field name.
        field: &'static str,
        /// Why it was refused, without echoing secrets.
        why: String,
    },
    /// The component could not be reached.
    Unavailable {
        /// What was unavailable.
        what: String,
    },
}

impl PortError {
    /// The methodology section of a control refusal, if any: AICD §17.
    #[must_use]
    pub fn methodology_ref(&self) -> Option<MethodologyRef> {
        match self {
            Self::Refused(error) => error.methodology_ref(),
            Self::Denied {
                methodology_ref, ..
            } => Some(methodology_ref.clone()),
            Self::Invalid { .. } | Self::Unavailable { .. } => None,
        }
    }
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(error) => write!(f, "{error}"),
            Self::Denied {
                code,
                methodology_ref,
            } => write!(f, "refused under {methodology_ref}: {code}"),
            Self::Invalid { field, why } => write!(f, "{field} is not acceptable: {why}"),
            Self::Unavailable { what } => write!(f, "{what} is unavailable"),
        }
    }
}

impl std::error::Error for PortError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Refused(error) => Some(error),
            Self::Denied { .. } | Self::Invalid { .. } | Self::Unavailable { .. } => None,
        }
    }
}

/// A source of identifiers: AICD §13.
pub trait IdSource {
    /// The next identifier.
    ///
    /// # Errors
    ///
    /// [`PortError::Unavailable`] when the engine cannot produce one.
    fn next_id(&mut self) -> Result<Id, PortError>;
}

/// Wall-clock time for event timestamps: AICD §13.
pub trait Clock: Send {
    /// Current UTC time in milliseconds since the Unix epoch.
    fn now(&mut self) -> Timestamp;
}
