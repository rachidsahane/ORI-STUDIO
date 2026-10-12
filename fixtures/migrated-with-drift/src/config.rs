//! Runtime configuration, read from the process environment.

use std::path::PathBuf;

/// The port the listener binds when `PARCEL_PORT` is not set.
pub const DEFAULT_PORT: u16 = 7070;

/// The file parcels are kept in when `PARCEL_DATA_FILE` is not set.
pub const DEFAULT_DATA_FILE: &str = "parcels.tsv";

/// Everything the program takes from its environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Port the listener binds on the loopback interface.
    pub port: u16,
    /// Path of the tab-separated file that holds every parcel.
    pub data_file: PathBuf,
}

impl Config {
    /// Read the configuration from the real process environment.
    pub fn from_env() -> Config {
        Config::from_lookup(|name| std::env::var(name).ok())
    }

    /// Read the configuration through `lookup`, so a test can supply values
    /// without touching the process environment.
    pub fn from_lookup<F: Fn(&str) -> Option<String>>(lookup: F) -> Config {
        let port = lookup("PARCEL_PORT")
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(DEFAULT_PORT);
        let data_file = lookup("PARCEL_DATA_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_FILE));
        Config { port, data_file }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_set_gives_the_defaults() {
        let config = Config::from_lookup(|_| None);
        assert_eq!(config.port, DEFAULT_PORT);
        assert_eq!(config.data_file, PathBuf::from(DEFAULT_DATA_FILE));
    }

    #[test]
    fn values_that_are_set_win_and_a_bad_port_falls_back() {
        let config = Config::from_lookup(|name| match name {
            "PARCEL_PORT" => Some("9191".to_owned()),
            "PARCEL_DATA_FILE" => Some("/var/lib/parceltrack/data.tsv".to_owned()),
            _ => None,
        });
        assert_eq!(config.port, 9191);
        assert_eq!(
            config.data_file,
            PathBuf::from("/var/lib/parceltrack/data.tsv")
        );

        let bad = Config::from_lookup(|_| Some("not-a-port".to_owned()));
        assert_eq!(bad.port, DEFAULT_PORT);
    }
}
