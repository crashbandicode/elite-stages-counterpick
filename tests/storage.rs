#![cfg(feature = "std")]
use elite_stages_counterpick_core::{
    storage::{ConfigStore, StorageError, FILE_NAME},
    *,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "counterpick-storage-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
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
fn missing_file_uses_disabled_defaults_without_writes() {
    let dir = Fixture::new();
    fs::write(dir.0.join("settings.toml"), b"original=true\n").unwrap();
    let store = ConfigStore::load(&dir.0).unwrap();
    assert!(!store.current().enabled());
    assert!(store.current().profile_count() > 0);
    assert!(!dir.0.join(FILE_NAME).exists());
    assert_eq!(
        fs::read(dir.0.join("settings.toml")).unwrap(),
        b"original=true\n"
    );
}

#[test]
fn edited_toggle_and_order_replace_only_own_file() {
    let dir = Fixture::new();
    fs::write(dir.0.join("settings.toml"), b"baseline=unchanged\n").unwrap();
    let mut store = ConfigStore::load(&dir.0).unwrap();
    let mut draft = store.current().clone();
    let mario = Fighter::from_key("mario").unwrap();
    draft.set_enabled(true);
    draft
        .set_ranking(mario, &[Stage::HollowBastion, Stage::FinalDestination])
        .unwrap();
    assert!(!store.current().enabled()); // A menu draft is not published early.
    store.save(draft.clone()).unwrap();
    assert_eq!(ConfigStore::load(&dir.0).unwrap().current(), &draft);
    draft.set_enabled(false);
    draft.set_ranking(mario, &[]).unwrap();
    store.save(draft.clone()).unwrap();
    assert_eq!(ConfigStore::load(&dir.0).unwrap().current(), &draft);
    assert_eq!(
        fs::read(dir.0.join("settings.toml")).unwrap(),
        b"baseline=unchanged\n"
    );
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 2);
}

#[test]
fn duplicate_draft_order_preserves_previous_ranking() {
    let mut draft = Config::parse("schema=1\nenabled=true\nfighter.mario=end\n").unwrap();
    let before = draft.clone();
    assert_eq!(
        draft
            .set_ranking(
                Fighter::from_key("mario").unwrap(),
                &[Stage::Battlefield, Stage::Battlefield]
            )
            .unwrap_err()
            .kind,
        ConfigErrorKind::DuplicateStage
    );
    assert_eq!(draft, before);
}

#[test]
fn malformed_and_oversized_files_are_preserved() {
    let dir = Fixture::new();
    let path = dir.0.join(FILE_NAME);
    for bytes in [
        b"schema=1\nenabled=maybe\n".to_vec(),
        vec![b' '; 32769],
        vec![0xff],
    ] {
        fs::write(&path, &bytes).unwrap();
        let error = ConfigStore::load(&dir.0).unwrap_err();
        assert!(matches!(
            error,
            StorageError::InvalidConfig(_) | StorageError::InvalidUtf8
        ));
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn unreadable_path_is_an_error_and_not_a_missing_file() {
    let dir = Fixture::new();
    // A directory where the file belongs exists, so load must not fall back to defaults.
    fs::create_dir(dir.0.join(FILE_NAME)).unwrap();
    fs::write(dir.0.join(FILE_NAME).join("keep"), b"external").unwrap();
    assert!(matches!(
        ConfigStore::load(&dir.0),
        Err(StorageError::Io(_))
    ));
    assert_eq!(
        fs::read(dir.0.join(FILE_NAME).join("keep")).unwrap(),
        b"external"
    );
}

#[test]
fn first_save_creates_the_missing_folder_and_a_fresh_load_sees_it() {
    let dir = Fixture::new();
    let folder = dir.0.join("not").join("yet");
    let mut store = ConfigStore::load(&folder).unwrap();
    assert!(!folder.exists()); // Loading never creates anything.
    let mut draft = store.current().clone();
    draft.set_enabled(true);
    store.save(draft.clone()).unwrap();
    assert_eq!(ConfigStore::load(&folder).unwrap().current(), &draft);
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
}

#[test]
fn failed_replace_preserves_active_config_and_removes_only_own_temporary() {
    let dir = Fixture::new();
    let mut store = ConfigStore::load(&dir.0).unwrap();
    let before = store.current().clone();
    // An unreplaceable destination makes rename fail even as root.
    fs::create_dir(dir.0.join(FILE_NAME)).unwrap();
    fs::write(dir.0.join(FILE_NAME).join("keep"), b"external").unwrap();
    let mut draft = before.clone();
    draft.set_enabled(true);
    assert!(matches!(store.save(draft), Err(StorageError::Io(_))));
    assert_eq!(store.current(), &before);
    assert_eq!(
        fs::read(dir.0.join(FILE_NAME).join("keep")).unwrap(),
        b"external"
    );
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[test]
fn interrupted_save_recovers_only_a_valid_backup() {
    let dir = Fixture::new();
    let path = dir.0.join(FILE_NAME);
    let backup = dir.0.join(format!("{FILE_NAME}.bak"));
    let valid = b"schema=1\nenabled=true\nfighter.mario=end\n";
    fs::write(&backup, valid).unwrap();
    assert!(ConfigStore::load(&dir.0).unwrap().current().enabled());
    assert_eq!(fs::read(&path).unwrap(), valid);
    assert!(!backup.exists());
    fs::remove_file(&path).unwrap();
    fs::write(&backup, b"broken").unwrap();
    assert!(ConfigStore::load(&dir.0).is_err());
    assert!(!path.exists());
    assert_eq!(fs::read(&backup).unwrap(), b"broken");
}
