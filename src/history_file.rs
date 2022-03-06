#![cfg(feature = "jenkins")]

use anyhow::Result;
#[cfg(not(test))]
use directories::ProjectDirs;
use log::*;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use time::{Duration, OffsetDateTime};

const STALENESS_THRESHOLD: Duration = Duration::days(5);
static CRATE_NAME: &str = "crabby-merge";
#[cfg(not(test))]
static DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let dir = ProjectDirs::from("", "", CRATE_NAME)
        .expect("Could not get project directory")
        .data_dir()
        .to_path_buf();
    std::fs::create_dir_all(&dir).ok();
    dir
});
#[cfg(test)]
static DATA_DIR: Lazy<PathBuf> = Lazy::new(|| {
    // Create a directory in the temp directory
    // TODO: figure out how to remove the directory afterward
    let temp_dir = Box::leak(Box::new(
        tempdir::TempDir::new(CRATE_NAME).expect("Could not get project directory"),
    ));
    temp_dir.path().to_path_buf()
});

/// Retry history for a single pull request
///
/// The state is associated with a pull request with a specific _id_, where the id can be any
/// string identifier. This state is generally stored on the filesystem and the id is encoded in
/// the filename.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct History {
    /// Number of retries already made
    n_retries: u32,
    last_update: OffsetDateTime,
}

impl History {
    /// Return the path associated with a given id. Does not guarantee that the path exists.
    fn path(id: &str) -> PathBuf {
        let mut path = DATA_DIR.clone();
        path.push(id);
        path
    }

    /// Save new history file for a given id, overwriting any existing file
    pub fn save(id: &str, n_retries: u32) -> Result<()> {
        let history = History {
            n_retries,
            last_update: OffsetDateTime::now_utc(),
        };
        let buf = serde_json::to_vec(&history)?;
        Ok(File::create(Self::path(id))?.write_all(&buf)?)
    }

    fn from_file(path: &Path) -> Result<Option<Self>> {
        let mut buf = Vec::new();
        match File::open(path) {
            Ok(mut file) => file.read_to_end(&mut buf)?,
            Err(_) => return Ok(None),
        };
        let history: Self = serde_json::from_slice(&buf)?;
        Ok(Some(history))
    }

    /// Load history for a given id
    pub fn load(id: &str) -> Result<Option<Self>> {
        Self::from_file(&Self::path(id))
    }

    /// Delete history for a given id
    pub fn delete(id: &str) -> Result<()> {
        Ok(std::fs::remove_file(Self::path(id))?)
    }

    /// Return the time since the last history update
    pub fn age(&self) -> Duration {
        OffsetDateTime::now_utc() - self.last_update
    }

    /// Return the number of previous retries
    pub fn n_retries(&self) -> u32 {
        self.n_retries
    }
}

/// Clean out history files older than `STALENESS_THRESHOLD`
pub fn decruft() -> Result<()> {
    debug!("Cleaning {}", DATA_DIR.display());
    for entry in std::fs::read_dir(&*DATA_DIR)?.flatten() {
        let delete = match History::from_file(&entry.path()) {
            Ok(Some(history)) => history.age() >= STALENESS_THRESHOLD,
            Ok(None) => false,
            Err(_) => true,
        };
        if delete {
            std::fs::remove_file(&entry.path()).ok();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback() {
        History::save("pandas", 5).unwrap();
        let history_loaded = History::load("pandas").unwrap().unwrap();
        assert_eq!(history_loaded.n_retries, 5);
        assert!(history_loaded.age() < Duration::seconds(10));
        assert!(history_loaded.age() > Duration::ZERO);
        History::delete("pandas").unwrap();
        assert!(History::delete("pandas").is_err());
    }
}
