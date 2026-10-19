//! The parcel store: one tab-separated file, rewritten whole on every change.
//!
//! Each line is `id<TAB>destination<TAB>status`. The file is written to a
//! sibling path and renamed over the original, so a crash leaves either the
//! old file or the new one.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::model::{Parcel, Status};

/// Why a store operation failed.
#[derive(Debug)]
pub enum StoreError {
    /// The file could not be read or written.
    Io(io::Error),
    /// A line of the file is not `id<TAB>destination<TAB>status`.
    Corrupt {
        /// One-based line number.
        line: usize,
    },
    /// A parcel with this id is already stored.
    Duplicate(String),
    /// No parcel has this id.
    Unknown(String),
    /// The parcel is in a final status and cannot change.
    Final(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(error) => write!(f, "store file: {error}"),
            StoreError::Corrupt { line } => write!(f, "store file is corrupt at line {line}"),
            StoreError::Duplicate(id) => write!(f, "parcel {id} already exists"),
            StoreError::Unknown(id) => write!(f, "no parcel {id}"),
            StoreError::Final(id) => write!(f, "parcel {id} is already in a final status"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        StoreError::Io(error)
    }
}

/// All parcels, loaded from and saved to one file.
#[derive(Debug)]
pub struct Store {
    path: PathBuf,
    parcels: Vec<Parcel>,
}

impl Store {
    /// Load the store at `path`. A missing file is an empty store.
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(StoreError::Io(error)),
        };
        let mut parcels = Vec::new();
        for (index, line) in text.lines().enumerate() {
            if line.is_empty() {
                continue;
            }
            let mut fields = line.split('\t');
            let (Some(id), Some(destination), Some(status), None) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                return Err(StoreError::Corrupt { line: index + 1 });
            };
            let status = Status::parse(status).ok_or(StoreError::Corrupt { line: index + 1 })?;
            parcels.push(Parcel {
                id: id.to_owned(),
                destination: destination.to_owned(),
                status,
            });
        }
        Ok(Store {
            path: path.to_owned(),
            parcels,
        })
    }

    /// Every parcel, in the order they were registered.
    pub fn all(&self) -> &[Parcel] {
        &self.parcels
    }

    /// The parcel with this id, if there is one.
    pub fn get(&self, id: &str) -> Option<&Parcel> {
        self.parcels.iter().find(|parcel| parcel.id == id)
    }

    /// Store a new parcel.
    pub fn insert(&mut self, parcel: Parcel) -> Result<(), StoreError> {
        if self.get(&parcel.id).is_some() {
            return Err(StoreError::Duplicate(parcel.id));
        }
        self.parcels.push(parcel);
        self.save()
    }

    /// Move a parcel to the next status on the forward path and return it.
    pub fn advance(&mut self, id: &str) -> Result<Status, StoreError> {
        let parcel = self.find_mut(id)?;
        let next = parcel
            .status
            .next()
            .ok_or_else(|| StoreError::Final(id.to_owned()))?;
        parcel.status = next;
        self.save()?;
        Ok(next)
    }

    /// Mark a parcel as returned to its sender.
    pub fn mark_returned(&mut self, id: &str) -> Result<(), StoreError> {
        let parcel = self.find_mut(id)?;
        if parcel.status.is_final() {
            return Err(StoreError::Final(id.to_owned()));
        }
        parcel.status = Status::Returned;
        self.save()
    }

    fn find_mut(&mut self, id: &str) -> Result<&mut Parcel, StoreError> {
        self.parcels
            .iter_mut()
            .find(|parcel| parcel.id == id)
            .ok_or_else(|| StoreError::Unknown(id.to_owned()))
    }

    fn save(&self) -> Result<(), StoreError> {
        let mut text = String::new();
        for parcel in &self.parcels {
            text.push_str(&format!(
                "{}\t{}\t{}\n",
                parcel.id, parcel.destination, parcel.status
            ));
        }
        let mut sibling = self.path.clone().into_os_string();
        sibling.push(".tmp");
        let sibling = PathBuf::from(sibling);
        fs::write(&sibling, text)?;
        fs::rename(&sibling, &self.path)?;
        Ok(())
    }
}
