//! Shared settings replacement for Horizon, whose rename cannot overwrite a file.
//! Callers serialize writers and validate a recovery file before restoring it.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

fn backup(destination: &Path) -> PathBuf {
    let mut name = destination.as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}

/// Pick a backup only when the destination is absent. A malformed destination
/// must never be hidden by an older valid file.
pub fn source_path(destination: &Path) -> io::Result<(PathBuf, bool)> {
    match fs::metadata(destination) {
        Ok(_) => Ok((destination.to_path_buf(), false)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let old = backup(destination);
            match fs::metadata(&old) {
                Ok(metadata) if metadata.is_file() => Ok((old, true)),
                Ok(_) => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "settings backup is not a file",
                )),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    Ok((destination.to_path_buf(), false))
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

/// Restore only after the caller's bounded read and format validation succeeded.
pub fn restore_validated(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

/// The old file is retained until the new file has taken its place. A failed
/// second rename rolls back; a failed rollback leaves .bak for validated recovery.
/// The caller owns and removes its temporary file on failure.
pub fn replace(temporary: &Path, destination: &Path) -> io::Result<()> {
    match fs::rename(temporary, destination) {
        Ok(()) => Ok(()),
        Err(first) => {
            if !fs::metadata(destination).is_ok_and(|metadata| metadata.is_file()) {
                return Err(first);
            }
            replace_via_backup(temporary, destination)
        }
    }
}

fn replace_via_backup(temporary: &Path, destination: &Path) -> io::Result<()> {
    let old = backup(destination);
    match fs::metadata(&old) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "unresolved settings backup",
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::rename(destination, &old)?;
    if let Err(error) = fs::rename(temporary, destination) {
        let _ = fs::rename(&old, destination);
        return Err(error);
    }
    let _ = fs::remove_file(old);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "counterpick-replace-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn horizon_replacement_preserves_previous_until_new_file_is_ready() {
        let dir = Fixture::new();
        let dst = dir.0.join("settings");
        let tmp = dir.0.join("pending");
        fs::write(&dst, b"old").unwrap();
        fs::write(&tmp, b"new").unwrap();
        replace_via_backup(&tmp, &dst).unwrap();
        assert_eq!(fs::read(&dst).unwrap(), b"new");
        assert!(!backup(&dst).exists());
        assert!(replace_via_backup(&tmp, &dst).is_err());
        assert_eq!(fs::read(&dst).unwrap(), b"new");
        assert!(!backup(&dst).exists());
    }

    #[test]
    fn interrupted_backup_is_selected_only_if_destination_is_absent() {
        let dir = Fixture::new();
        let dst = dir.0.join("settings");
        let old = backup(&dst);
        fs::write(&old, b"old").unwrap();
        assert_eq!(source_path(&dst).unwrap(), (old.clone(), true));
        fs::write(&dst, b"current").unwrap();
        assert_eq!(source_path(&dst).unwrap(), (dst.clone(), false));
        let tmp = dir.0.join("pending");
        fs::write(&tmp, b"new").unwrap();
        assert!(replace_via_backup(&tmp, &dst).is_err());
        assert_eq!(fs::read(&dst).unwrap(), b"current");
        assert_eq!(fs::read(old).unwrap(), b"old");
    }
}
