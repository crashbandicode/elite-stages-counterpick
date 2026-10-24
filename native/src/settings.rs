//! Single persistent owner of the standalone counterpick `counterpick.cfg`.
//!
//! The file lives beside the legal-stage pool, in `pool::DIR`, never in the
//! Training Modpack folder. The cached `ConfigStore` is the only copy of the
//! published configuration. Hooks read it through `with_config`, which takes
//! the lock with `try_lock` and so never waits for a save. The menu clones a
//! `draft` on open, edits that, and calls `save` outside any callback.
//!
//! Locking: one mutex guards the store. `save` and `startup` hold it across
//! their disk I/O, so writers serialize and the file always equals the
//! published value. While they hold it a hook read returns `None` and the
//! native behavior stands. The `with_config` closure runs under the lock: it
//! must not call `draft`, `save` or `startup` (they block on the same lock and
//! would deadlock the thread). A nested `with_config` just returns `None`.

use std::fmt;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use elite_stages_counterpick_core::storage::{ConfigStore, StorageError};
use elite_stages_counterpick_core::{Config, ConfigError};

use crate::pool::DIR;

/// `None` until `startup` succeeds, and again after a refusal.
static STORE: Mutex<Option<ConfigStore>> = Mutex::new(None);
/// Lock-free copy of `current().enabled()`, updated while the lock is held.
static ENABLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Failure {
    /// Startup never ran or refused `counterpick.cfg`. Nothing was written.
    Unavailable,
    Io {
        kind: io::ErrorKind,
        os: Option<i32>,
    },
    InvalidUtf8,
    Config(ConfigError),
}

impl From<StorageError> for Failure {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::Io(error) => Self::Io {
                kind: error.kind(),
                os: error.raw_os_error(),
            },
            StorageError::InvalidUtf8 => Self::InvalidUtf8,
            StorageError::InvalidConfig(error) => Self::Config(error),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => write!(out, "counterpick.cfg unavailable"),
            Self::Io { kind, os } => write!(out, "io {kind:?} os={os:?}"),
            Self::InvalidUtf8 => write!(out, "invalid utf-8"),
            Self::Config(error) => write!(out, "config line {} {:?}", error.line, error.kind),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Startup {
    Ready,
    /// An existing file is unreadable or invalid. It is left byte-for-byte as
    /// found and counterpick stays off; `save` refuses until a restart.
    Refused(Failure),
}

impl fmt::Display for Startup {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready => write!(out, "ready"),
            Self::Refused(failure) => write!(out, "refused: {failure}"),
        }
    }
}

pub(crate) fn startup() -> Startup {
    startup_in(Path::new(DIR))
}

/// A missing file publishes the shipped disabled defaults in memory only.
pub(crate) fn startup_in(directory: &Path) -> Startup {
    let mut slot = lock();
    match ConfigStore::load(directory) {
        Ok(store) => {
            ENABLED.store(store.current().enabled(), Ordering::Release);
            *slot = Some(store);
            Startup::Ready
        }
        Err(error) => {
            ENABLED.store(false, Ordering::Release);
            *slot = None;
            Startup::Refused(error.into())
        }
    }
}

/// Lock-free. False when unloaded, refused or disabled.
pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

/// Hook-side read. No clone, allocation or file I/O. `None` means keep the
/// native behavior: unloaded, refused, or a save/startup currently owns the lock.
pub(crate) fn with_config<R>(read: impl FnOnce(&Config) -> R) -> Option<R> {
    let slot = STORE.try_lock().ok()?;
    let store = slot.as_ref()?;
    Some(read(store.current()))
}

/// Clone of the published configuration, for opening or refreshing the menu.
/// `None` when unavailable. Blocks only for a save in progress.
pub(crate) fn draft() -> Option<Config> {
    lock().as_ref().map(|store| store.current().clone())
}

/// Persist, then publish. Any failure leaves the previous file, the published
/// configuration and `enabled()` unchanged. Never call from a hook callback.
pub(crate) fn save(candidate: Config) -> Result<(), Failure> {
    let mut slot = lock();
    let store = slot.as_mut().ok_or(Failure::Unavailable)?;
    let enabled = candidate.enabled();
    store.save(candidate)?;
    ENABLED.store(enabled, Ordering::Release);
    Ok(())
}

fn lock() -> MutexGuard<'static, Option<ConfigStore>> {
    STORE.lock().unwrap_or_else(PoisonError::into_inner)
}
