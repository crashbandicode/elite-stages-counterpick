//! Host coverage for the standalone native settings owners. The two native
//! modules use only std and this crate, so they are compiled here from their
//! real source files instead of being copied. Their directory-taking seams
//! (`startup_in`, `replace_pool_in`) keep every test inside a temp directory.
//! Both modules publish process-wide state, so each test runs under one gate.
#![cfg(feature = "std")]

use elite_stages_counterpick_core::{
    hash40, storage::FILE_NAME as CONFIG_FILE, Config, Fighter, Stage,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Mutex, MutexGuard, PoisonError,
    },
};

#[allow(dead_code)]
#[path = "../native/src/pool.rs"]
mod pool;
#[allow(dead_code)]
#[path = "../native/src/settings.rs"]
mod settings;

static GATE: Mutex<()> = Mutex::new(());
static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

fn serial() -> MutexGuard<'static, ()> {
    GATE.lock().unwrap_or_else(PoisonError::into_inner)
}

#[test]
fn pool_recovers_a_valid_interrupted_backup_but_keeps_invalid_bytes() {
    let _gate = serial();
    let dir = Fixture::new();
    let backup = dir.0.join("settings.toml.bak");
    let valid = b"enabled = true\nstages = [\"end\"]\n";
    fs::write(&backup, valid).unwrap();
    assert_eq!(pool::startup_in(&dir.0), pool::Startup::Ready);
    assert!(pool::view().unwrap().enabled);
    assert_eq!(fs::read(dir.pool_file()).unwrap(), valid);
    assert!(!backup.exists());
    fs::remove_file(dir.pool_file()).unwrap();
    fs::write(&backup, b"broken").unwrap();
    assert_eq!(pool::startup_in(&dir.0), pool::Startup::Refused);
    assert!(!dir.pool_file().exists());
    assert_eq!(fs::read(&backup).unwrap(), b"broken");
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "standalone-settings-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn pool_file(&self) -> PathBuf {
        self.0.join(pool::FILE_NAME)
    }

    fn config_file(&self) -> PathBuf {
        self.0.join(CONFIG_FILE)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn entries(dir: &Path) -> usize {
    fs::read_dir(dir).unwrap().count()
}

/// Byte-for-byte copy of the installed original plugin file (559 bytes).
const INSTALLED: &str = concat!(
    "# elite-legal-stages-public settings, changed from the in-game menu (L+R+Up).\n",
    "# safe to delete: the defaults come back (mod off, all stages on).\n",
    "# only these stages can be listed: battle_field, battle_field_s, end, yoshi_cartboard, fox_lylatcruise, poke_stadium2, animal_village, poke_kalos, animal_city, ff_cave, trail_castle\n",
    "\n",
    "enabled = true\n",
    "stages = [\n",
    "    \"battle_field\",\n",
    "    \"battle_field_s\",\n",
    "    \"yoshi_cartboard\",\n",
    "    \"fox_lylatcruise\",\n",
    "    \"poke_stadium2\",\n",
    "    \"animal_village\",\n",
    "    \"poke_kalos\",\n",
    "    \"animal_city\",\n",
    "    \"ff_cave\",\n",
    "    \"trail_castle\",\n",
    "]\n",
);

/// Unusable files, each with the failure class the loaders must report.
fn unusable_pool_files() -> Vec<Vec<u8>> {
    vec![
        b"enabled = true\nstages = [\n    \"end\",\n".to_vec(),
        b"enabled = true\nstages = [\"nope\"]\n".to_vec(),
        b"enabled = true\nstages = [\"end\", \"end\"]\n".to_vec(),
        b"enabled = maybe\nstages = []\n".to_vec(),
        b"enabled = true\nstages = [\"end\"]\ngarbage\n".to_vec(),
        vec![b' '; 32 * 1024 + 1],
        vec![0xff],
    ]
}

#[test]
fn pool_missing_file_is_off_with_every_stage_and_writes_nothing() {
    let _gate = serial();
    let dir = Fixture::new();
    assert_eq!(pool::startup_in(&dir.0), pool::Startup::Ready);
    assert_eq!(pool::state_word(), 0x7ff);
    let view = pool::view().unwrap();
    assert!(!view.enabled);
    assert_eq!(view.selected, [true; 11]);
    assert_eq!(entries(&dir.0), 0);
}

#[test]
fn pool_reads_the_installed_original_file_and_preserves_its_selection() {
    let _gate = serial();
    assert_eq!(INSTALLED.len(), 559);
    let dir = Fixture::new();
    fs::write(dir.pool_file(), INSTALLED).unwrap();
    assert_eq!(pool::startup_in(&dir.0), pool::Startup::Ready);
    // Every stage but `end`, in Stage::ALL bit order, plus the enable bit.
    assert_eq!(pool::state_word(), 0x107fb);
    let view = pool::view().unwrap();
    assert!(view.enabled);
    assert!(!view.stage_pool().contains(Stage::FinalDestination));
    assert!(view.stage_pool().contains(Stage::HollowBastion));
    // Preserve the original selection; only the menu-help comment changes.
    pool::replace_pool_in(&dir.0, view.enabled, view.selected).unwrap();
    let expected = INSTALLED.replacen(
        "changed from the in-game menu (L+R+Up).",
        "changed from the Deluxe menu pages.",
        1,
    );
    assert_eq!(fs::read_to_string(dir.pool_file()).unwrap(), expected);
    assert_eq!(pool::startup_in(&dir.0), pool::Startup::Ready);
    assert_eq!(pool::state_word(), 0x107fb);
    assert_eq!(entries(&dir.0), 1);
}

#[test]
fn pool_save_persists_then_publishes_and_survives_a_fresh_load() {
    let _gate = serial();
    let dir = Fixture::new();
    assert_eq!(pool::startup_in(&dir.0), pool::Startup::Ready);
    let mut selected = [false; 11];
    selected[0] = true;
    selected[2] = true;
    selected[10] = true;
    pool::replace_pool_in(&dir.0, true, selected).unwrap();
    assert_eq!(pool::state_word(), 0x10405);
    let published = pool::view().unwrap();
    assert!(published.enabled);
    assert_eq!(published.selected, selected);
    assert_eq!(entries(&dir.0), 1);
    // A fresh load of the file, not the in-memory word, must agree.
    assert_eq!(pool::startup_in(&dir.0), pool::Startup::Ready);
    assert_eq!(pool::view().unwrap(), published);
    assert_eq!(pool::state_word(), 0x10405);
}

#[test]
fn pool_empty_selection_is_refused_but_an_empty_file_is_accepted_as_inert() {
    let _gate = serial();
    let dir = Fixture::new();
    fs::write(dir.pool_file(), "enabled = true\nstages = []\n").unwrap();
    assert_eq!(pool::startup_in(&dir.0), pool::Startup::Ready);
    // Enabled but empty: the hook sees no stage bits and keeps the native result.
    assert_eq!(pool::state_word(), pool::ENABLED_BIT);
    assert_eq!(
        pool::replace_pool_in(&dir.0, true, [false; 11]),
        Err(pool::PoolError::Empty)
    );
    assert_eq!(
        fs::read(dir.pool_file()).unwrap(),
        b"enabled = true\nstages = []\n"
    );
    assert_eq!(pool::state_word(), pool::ENABLED_BIT);
}

#[test]
fn pool_unusable_files_fail_closed_and_are_never_overwritten() {
    let _gate = serial();
    let good = Fixture::new();
    for bytes in unusable_pool_files() {
        // Start from a published state so a refusal must actively clear it.
        assert_eq!(pool::startup_in(&good.0), pool::Startup::Ready);
        assert!(pool::view().is_some());
        let dir = Fixture::new();
        fs::write(dir.pool_file(), &bytes).unwrap();
        assert_eq!(pool::startup_in(&dir.0), pool::Startup::Refused);
        assert_eq!(pool::state_word(), 0);
        assert_eq!(pool::view(), None);
        assert_eq!(
            pool::replace_pool_in(&dir.0, true, [true; 11]),
            Err(pool::PoolError::NotLoaded)
        );
        assert_eq!(fs::read(dir.pool_file()).unwrap(), bytes);
        assert_eq!(entries(&dir.0), 1);
    }
}

#[test]
fn pool_unreadable_path_is_not_mistaken_for_a_missing_file() {
    let _gate = serial();
    let dir = Fixture::new();
    // A directory where the file belongs exists, so it must not read as absent.
    fs::create_dir(dir.pool_file()).unwrap();
    fs::write(dir.pool_file().join("keep"), b"external").unwrap();
    assert_eq!(pool::startup_in(&dir.0), pool::Startup::Refused);
    assert_eq!(pool::view(), None);
    assert_eq!(fs::read(dir.pool_file().join("keep")).unwrap(), b"external");
}

#[test]
fn pool_failed_write_publishes_nothing_and_keeps_the_directory() {
    let _gate = serial();
    let dir = Fixture::new();
    assert_eq!(pool::startup_in(&dir.0), pool::Startup::Ready);
    let mut selected = [false; 11];
    selected[2] = true;
    pool::replace_pool_in(&dir.0, true, selected).unwrap();
    let before = pool::state_word();
    // An unreplaceable destination makes rename fail even as root.
    fs::remove_file(dir.pool_file()).unwrap();
    fs::create_dir(dir.pool_file()).unwrap();
    fs::write(dir.pool_file().join("keep"), b"external").unwrap();
    assert_eq!(
        pool::replace_pool_in(&dir.0, false, [true; 11]),
        Err(pool::PoolError::Io)
    );
    assert_eq!(pool::state_word(), before);
    assert_eq!(fs::read(dir.pool_file().join("keep")).unwrap(), b"external");
    assert_eq!(entries(&dir.0), 1);
}

#[test]
fn pool_concurrent_saves_leave_the_file_and_published_word_equal() {
    let _gate = serial();
    let dir = Fixture::new();
    assert_eq!(pool::startup_in(&dir.0), pool::Startup::Ready);
    std::thread::scope(|scope| {
        for writer in 0..8usize {
            let directory = &dir.0;
            scope.spawn(move || {
                for round in 0..10usize {
                    let mut selected = [false; 11];
                    selected[writer] = true;
                    selected[(writer + round) % 11] = true;
                    pool::replace_pool_in(directory, round % 2 == 0, selected).unwrap();
                }
            });
        }
    });
    let published = (pool::view().unwrap(), pool::state_word());
    assert_eq!(pool::startup_in(&dir.0), pool::Startup::Ready);
    assert_eq!((pool::view().unwrap(), pool::state_word()), published);
    assert_eq!(entries(&dir.0), 1);
}

#[test]
fn pool_stage_bit_follows_stage_order_and_ignores_high_token_bits() {
    for (index, stage) in Stage::ALL.into_iter().enumerate() {
        let bit = u32::try_from(index).ok();
        assert_eq!(pool::stage_bit(hash40(stage)), bit);
        assert_eq!(pool::stage_bit(hash40(stage) | (0xabcd << 40)), bit);
    }
    assert_eq!(pool::stage_bit(0x1234), None);
}

#[test]
fn settings_missing_file_is_disabled_defaults_and_writes_nothing() {
    let _gate = serial();
    let dir = Fixture::new();
    fs::write(dir.pool_file(), b"baseline=unchanged\n").unwrap();
    assert_eq!(settings::startup_in(&dir.0), settings::Startup::Ready);
    assert!(!settings::enabled());
    assert_eq!(settings::with_config(Config::enabled), Some(false));
    assert!(settings::with_config(Config::profile_count).unwrap() > 0);
    assert!(settings::draft().is_some());
    assert_eq!(entries(&dir.0), 1);
}

#[test]
fn settings_save_persists_then_publishes_and_never_touches_the_pool_file() {
    let _gate = serial();
    let dir = Fixture::new();
    fs::write(dir.pool_file(), INSTALLED).unwrap();
    assert_eq!(settings::startup_in(&dir.0), settings::Startup::Ready);
    let mario = Fighter::from_key("mario").unwrap();
    let mut candidate = settings::draft().unwrap();
    candidate.set_enabled(true);
    candidate
        .set_ranking(mario, &[Stage::HollowBastion, Stage::FinalDestination])
        .unwrap();
    // An open menu draft is not published early.
    assert!(!settings::enabled());
    assert_ne!(settings::draft().unwrap(), candidate);
    settings::save(candidate.clone()).unwrap();
    assert!(settings::enabled());
    assert_eq!(settings::draft().unwrap(), candidate);
    assert_eq!(
        settings::with_config(|config| config.ranking(mario).map(<[Stage]>::to_vec)),
        Some(Some(vec![Stage::HollowBastion, Stage::FinalDestination]))
    );
    assert_eq!(
        fs::read_to_string(dir.config_file()).unwrap(),
        candidate.serialize()
    );
    assert_eq!(fs::read_to_string(dir.pool_file()).unwrap(), INSTALLED);
    assert_eq!(entries(&dir.0), 2);
    // A fresh load of the file, not the cached store, must agree.
    assert_eq!(settings::startup_in(&dir.0), settings::Startup::Ready);
    assert_eq!(settings::draft().unwrap(), candidate);
    assert!(settings::enabled());
}

#[test]
fn settings_failed_save_changes_neither_disk_nor_published_state() {
    let _gate = serial();
    let dir = Fixture::new();
    assert_eq!(settings::startup_in(&dir.0), settings::Startup::Ready);
    let mut first = settings::draft().unwrap();
    first.set_enabled(true);
    settings::save(first.clone()).unwrap();
    fs::remove_file(dir.config_file()).unwrap();
    fs::create_dir(dir.config_file()).unwrap();
    fs::write(dir.config_file().join("keep"), b"external").unwrap();
    let mut second = first.clone();
    second.set_enabled(false);
    assert!(matches!(
        settings::save(second),
        Err(settings::Failure::Io { .. })
    ));
    assert!(settings::enabled());
    assert_eq!(settings::draft().unwrap(), first);
    assert_eq!(settings::with_config(Config::enabled), Some(true));
    assert_eq!(
        fs::read(dir.config_file().join("keep")).unwrap(),
        b"external"
    );
    assert_eq!(entries(&dir.0), 1);
}

#[test]
fn settings_refused_file_stays_untouched_and_nothing_can_save() {
    let _gate = serial();
    let good = Fixture::new();
    let cases: [(&[u8], &str); 3] = [
        (b"schema=1\nenabled=maybe\n", "config line 2 InvalidBoolean"),
        (&[b' '; 32 * 1024 + 1], "config line 0 TooLarge"),
        (&[0xff], "invalid utf-8"),
    ];
    for (bytes, message) in cases {
        // Start from a loaded, enabled store so a refusal must actively clear it.
        assert_eq!(settings::startup_in(&good.0), settings::Startup::Ready);
        let mut loaded = settings::draft().unwrap();
        loaded.set_enabled(true);
        settings::save(loaded).unwrap();
        assert!(settings::enabled());
        let dir = Fixture::new();
        fs::write(dir.config_file(), bytes).unwrap();
        let started = settings::startup_in(&dir.0);
        assert_eq!(started.to_string(), format!("refused: {message}"));
        let settings::Startup::Refused(failure) = started else {
            panic!("an invalid counterpick.cfg must be refused");
        };
        assert_eq!(failure.to_string(), message);
        assert!(!settings::enabled());
        assert_eq!(settings::with_config(Config::enabled), None);
        assert_eq!(settings::draft(), None);
        assert_eq!(
            settings::save(Config::default()),
            Err(settings::Failure::Unavailable)
        );
        assert_eq!(fs::read(dir.config_file()).unwrap(), bytes);
        assert_eq!(entries(&dir.0), 1);
    }
}

#[test]
fn settings_hook_read_never_waits_for_the_lock_owner() {
    let _gate = serial();
    let dir = Fixture::new();
    assert_eq!(settings::startup_in(&dir.0), settings::Startup::Ready);
    // A nested read cannot deadlock: it reports busy instead.
    assert_eq!(
        settings::with_config(|_| settings::with_config(Config::enabled)),
        Some(None)
    );
    // While another thread owns the lock, as a save does, the read returns at once.
    let (held, owner_has_lock) = mpsc::channel();
    let (release, owner_may_leave) = mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            settings::with_config(|_| {
                held.send(()).unwrap();
                owner_may_leave.recv().unwrap();
            })
            .unwrap();
        });
        owner_has_lock.recv().unwrap();
        let during = settings::with_config(Config::enabled);
        let flag = settings::enabled();
        // Release before asserting so a failure cannot leave the owner blocked.
        release.send(()).unwrap();
        assert_eq!(during, None);
        assert!(!flag);
    });
    assert_eq!(settings::with_config(Config::enabled), Some(false));
}

#[test]
fn settings_concurrent_saves_leave_the_file_and_published_config_equal() {
    let _gate = serial();
    let dir = Fixture::new();
    assert_eq!(settings::startup_in(&dir.0), settings::Startup::Ready);
    let mario = Fighter::from_key("mario").unwrap();
    let base = settings::draft().unwrap();
    std::thread::scope(|scope| {
        for writer in 0..8usize {
            let base = base.clone();
            scope.spawn(move || {
                for round in 0..10usize {
                    let mut candidate = base.clone();
                    candidate.set_enabled((writer + round) % 2 == 0);
                    let mut order = Stage::ALL;
                    let count = order.len();
                    order.rotate_left((writer + round) % count);
                    candidate
                        .set_ranking(mario, &order[..1 + round % 5])
                        .unwrap();
                    settings::save(candidate).unwrap();
                }
            });
        }
    });
    let published = settings::draft().unwrap();
    assert_eq!(
        fs::read_to_string(dir.config_file()).unwrap(),
        published.serialize()
    );
    assert_eq!(settings::enabled(), published.enabled());
    assert_eq!(entries(&dir.0), 1);
}
