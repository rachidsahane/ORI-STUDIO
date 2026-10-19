//! parceltrack: record parcels and follow them through a carrier network.
//!
//! A parcel is registered with a destination, moves forward through its
//! statuses one step at a time, and can be returned until it is delivered.
//! Parcels are kept in a tab-separated file. The same data is reachable from
//! the command line and, read only, over a small HTTP listener.

pub mod config;
pub mod model;
pub mod notify;
pub mod server;
pub mod store;
