//! The parcel and the statuses it moves through.

use std::fmt;

/// Where a parcel is in the carrier network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Accepted from the sender and not yet handed to a carrier.
    Registered,
    /// With a carrier between two depots.
    InTransit,
    /// On the last vehicle, due at the destination today.
    OutForDelivery,
    /// Handed over at the destination. Final.
    Delivered,
    /// Sent back to the sender. Final.
    Returned,
}

impl Status {
    /// The text a status is stored and printed as.
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Registered => "registered",
            Status::InTransit => "in_transit",
            Status::OutForDelivery => "out_for_delivery",
            Status::Delivered => "delivered",
            Status::Returned => "returned",
        }
    }

    /// The status named by `text`, if it is one of the five.
    pub fn parse(text: &str) -> Option<Status> {
        match text {
            "registered" => Some(Status::Registered),
            "in_transit" => Some(Status::InTransit),
            "out_for_delivery" => Some(Status::OutForDelivery),
            "delivered" => Some(Status::Delivered),
            "returned" => Some(Status::Returned),
            _ => None,
        }
    }

    /// The next status on the forward path. `Delivered` and `Returned` are
    /// final, so they have none.
    pub fn next(self) -> Option<Status> {
        match self {
            Status::Registered => Some(Status::InTransit),
            Status::InTransit => Some(Status::OutForDelivery),
            Status::OutForDelivery => Some(Status::Delivered),
            Status::Delivered | Status::Returned => None,
        }
    }

    /// Whether no further status can follow this one.
    pub fn is_final(self) -> bool {
        matches!(self, Status::Delivered | Status::Returned)
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a parcel could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// The identifier was empty or held whitespace.
    BadId(String),
    /// The destination was empty or held a tab or a line break.
    BadDestination(String),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::BadId(id) => write!(f, "invalid parcel id {id:?}"),
            ModelError::BadDestination(text) => write!(f, "invalid destination {text:?}"),
        }
    }
}

impl std::error::Error for ModelError {}

/// One parcel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parcel {
    /// Identifier chosen by the sender. No whitespace.
    pub id: String,
    /// Free-text delivery address. No tabs, no line breaks.
    pub destination: String,
    /// Current status.
    pub status: Status,
}

impl Parcel {
    /// A newly registered parcel.
    pub fn new(id: &str, destination: &str) -> Result<Parcel, ModelError> {
        if id.is_empty() || id.chars().any(char::is_whitespace) {
            return Err(ModelError::BadId(id.to_owned()));
        }
        if destination.trim().is_empty() || destination.contains(['\t', '\n', '\r']) {
            return Err(ModelError::BadDestination(destination.to_owned()));
        }
        Ok(Parcel {
            id: id.to_owned(),
            destination: destination.trim().to_owned(),
            status: Status::Registered,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_forward_path_ends_at_delivered() {
        let mut status = Status::Registered;
        let mut steps = 0;
        while let Some(next) = status.next() {
            status = next;
            steps += 1;
        }
        assert_eq!(status, Status::Delivered);
        assert_eq!(steps, 3);
        assert!(Status::Returned.next().is_none());
    }

    #[test]
    fn every_status_round_trips_through_its_text() {
        for status in [
            Status::Registered,
            Status::InTransit,
            Status::OutForDelivery,
            Status::Delivered,
            Status::Returned,
        ] {
            assert_eq!(Status::parse(status.as_str()), Some(status));
        }
        assert_eq!(Status::parse("lost"), None);
    }

    #[test]
    fn a_parcel_needs_a_clean_id_and_destination() {
        assert!(Parcel::new("PK-1", "12 Quay Road").is_ok());
        assert!(Parcel::new("", "12 Quay Road").is_err());
        assert!(Parcel::new("PK 1", "12 Quay Road").is_err());
        assert!(Parcel::new("PK-1", "  ").is_err());
        assert!(Parcel::new("PK-1", "a\tb").is_err());
    }
}
