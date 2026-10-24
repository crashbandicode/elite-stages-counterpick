//! Configuration I/O for boot and explicit menu saves, never per-frame hooks.
//! Match results, rematch tickets and game pointers are intentionally absent.
use crate::{config::MAX_CONFIG_BYTES, Config, ConfigError, ConfigErrorKind};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const FILE_NAME: &str = "counterpick.cfg";
const DEFAULTS: &str = include_str!("../defaults/community-v1.cfg");
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    InvalidUtf8,
    InvalidConfig(ConfigError),
}

impl From<io::Error> for StorageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Single menu owner's last successfully loaded or saved configuration.
/// The caller supplies a directory; this never writes `settings.toml`.
#[derive(Debug)]
pub struct ConfigStore {
    directory: PathBuf,
    current: Config,
}

impl ConfigStore {
    /// Missing file uses the shipped, disabled recommendation snapshot in
    /// memory. Invalid existing files are reported and left byte-for-byte intact.
    /// A validated backup from an interrupted save is restored when the main
    /// file is absent. Other loading never creates a file or modifies the pool.
    pub fn load(directory: impl AsRef<Path>) -> Result<Self, StorageError> {
        let directory = directory.as_ref().to_path_buf();
        let destination = directory.join(FILE_NAME);
        let (path, recovering) = crate::files::source_path(&destination)?;
        // Pinned Skyline's `File::open` loses the typed NotFound, but
        // `fs::metadata` keeps it. Decide missing versus present here, before
        // any open, so no other failure is ever mistaken for an absent file.
        let current = match fs::metadata(&path) {
            Ok(metadata) => {
                if metadata.len() > MAX_CONFIG_BYTES as u64 {
                    return Err(too_large());
                }
                let mut bytes = Vec::new();
                // Bound the read itself too: the file may grow after metadata.
                File::open(&path)?
                    .take(MAX_CONFIG_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > MAX_CONFIG_BYTES {
                    return Err(too_large());
                }
                let text = std::str::from_utf8(&bytes).map_err(|_| StorageError::InvalidUtf8)?;
                Config::parse(text).map_err(StorageError::InvalidConfig)?
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Config::parse(DEFAULTS).map_err(StorageError::InvalidConfig)?
            }
            Err(error) => return Err(error.into()),
        };
        if recovering {
            crate::files::restore_validated(&path, &destination)?;
        }
        Ok(Self { directory, current })
    }

    pub fn current(&self) -> &Config {
        &self.current
    }

    /// Save a complete edited draft through a sibling file, then publish it in
    /// memory. A write/flush/rename failure keeps the active config unchanged.
    /// Horizon uses a backup rename; an interrupted backup is validated on load.
    /// This does not promise power-loss durability of the platform's directory.
    pub fn save(&mut self, candidate: Config) -> Result<(), StorageError> {
        let text = candidate.serialize();
        // Bound serialized edits by the same contract as loaded files.
        Config::parse(&text).map_err(StorageError::InvalidConfig)?;
        fs::create_dir_all(&self.directory)?;
        let (temporary, mut file) = create_temporary(&self.directory)?;
        let written = (|| -> io::Result<()> {
            file.write_all(text.as_bytes())?;
            file.sync_all()
        })();
        drop(file);
        let replaced = written
            .and_then(|()| crate::files::replace(&temporary, &self.directory.join(FILE_NAME)));
        if let Err(error) = replaced {
            // Only this successful create_new belongs to us. Never remove an
            // earlier interrupted writer's file or the destination on failure.
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        self.current = candidate;
        Ok(())
    }
}

fn too_large() -> StorageError {
    StorageError::InvalidConfig(ConfigError {
        line: 0,
        kind: ConfigErrorKind::TooLarge,
    })
}

fn create_temporary(directory: &Path) -> io::Result<(PathBuf, File)> {
    // No truncate of a stale or concurrent writer. A bounded collision retry
    // also lets a fresh process save after an interrupted earlier save.
    // The pinned Skyline filesystem needs the explicit write/create seed that
    // Training's replay store proved; `create_new` still makes it exclusive.
    for _ in 0..64 {
        let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!("{FILE_NAME}.pending-{id}"));
        match OpenOptions::new()
            .write(true)
            .create(true)
            .create_new(true)
            .truncate(false)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "counterpick temporary files occupy the available save slots",
    ))
}
