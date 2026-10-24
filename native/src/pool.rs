//! Original Elite `settings.toml` schema, owned by this module.
//!
//! The core `ConfigStore` remains the counterpick.cfg store. This file is a
//! separate compatibility store: `enabled` plus the eleven stage keys. The
//! hook reads one atomic word. File I/O happens at startup and when the menu
//! API saves; one lock serializes those writers.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use elite_stages_counterpick_core::{hash40, Stage, StagePool};

pub(crate) const DIR: &str = "sd:/ultimate/elite-legal-stages-public";
pub(crate) const FILE_NAME: &str = "settings.toml";
const LIMIT: usize = 32 * 1024;
pub(crate) const ENABLED_BIT: u32 = 1 << 16;
pub(crate) const STAGE_MASK: u32 = 0x7ff;
/// Set only inside `STATE`: a published word exists. `state_word` hides it, so
/// the hook still sees the documented word (`0x107fb` for the imported file).
const LOADED_BIT: u32 = 1 << 31;
const HASH40_MASK: u64 = 0x00ff_ffff_ffff;

/// The one publication word. Zero means nothing is published: fail closed.
static STATE: AtomicU32 = AtomicU32::new(0);
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
/// Serializes startup and saves so disk and the published word cannot be
/// interleaved by two writers. Nothing is invoked while it is held.
static WRITER: Mutex<()> = Mutex::new(());

const HEADER: &str = "\
# elite-legal-stages-public settings, changed from the in-game menu (L+R+Up).\n\
# safe to delete: the defaults come back (mod off, all stages on).\n\
# only these stages can be listed: battle_field, battle_field_s, end, yoshi_cartboard, fox_lylatcruise, poke_stadium2, animal_village, poke_kalos, animal_city, ff_cave, trail_castle\n\
\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolView {
    pub enabled: bool,
    /// `Stage::ALL` order. That order is the original plugin's pool-bit order.
    pub selected: [bool; 11],
}

impl PoolView {
    pub fn stage_pool(self) -> StagePool {
        let mut stages = [Stage::Battlefield; 11];
        let mut count = 0;
        for (stage, on) in Stage::ALL.into_iter().zip(self.selected) {
            if on {
                stages[count] = stage;
                count += 1;
            }
        }
        StagePool::from_stages(&stages[..count])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolError {
    Empty,
    Io,
    /// Startup refused `settings.toml` or never ran. The file is left as found.
    NotLoaded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Startup {
    Ready,
    Refused,
}

pub fn view() -> Option<PoolView> {
    let word = STATE.load(Ordering::Acquire);
    if word & LOADED_BIT == 0 {
        return None;
    }
    Some(decode(word))
}

/// Persist, then publish. A failed write publishes nothing and keeps the
/// previous file. A refused startup keeps its (malformed) file untouched.
pub fn replace_pool(enabled: bool, selected: [bool; 11]) -> Result<(), PoolError> {
    replace_pool_in(Path::new(DIR), enabled, selected)
}

pub(crate) fn replace_pool_in(
    directory: &Path,
    enabled: bool,
    selected: [bool; 11],
) -> Result<(), PoolError> {
    if !selected.iter().any(|on| *on) {
        return Err(PoolError::Empty);
    }
    let _writer = writer();
    if STATE.load(Ordering::Acquire) & LOADED_BIT == 0 {
        return Err(PoolError::NotLoaded);
    }
    let pool = PoolView { enabled, selected };
    write_settings(directory, &pool).map_err(|_| PoolError::Io)?;
    publish(pool);
    Ok(())
}

pub(crate) fn startup() -> Startup {
    startup_in(Path::new(DIR))
}

/// Missing file: publish the default (mod off, all eleven) without creating a
/// file. Any other failure publishes nothing and leaves the file as found.
pub(crate) fn startup_in(directory: &Path) -> Startup {
    let _writer = writer();
    match read_settings(directory) {
        Ok(Some(pool)) => {
            publish(pool);
            Startup::Ready
        }
        Ok(None) => {
            publish(defaults());
            Startup::Ready
        }
        Err(()) => {
            STATE.store(0, Ordering::Release);
            Startup::Refused
        }
    }
}

pub(crate) fn state_word() -> u32 {
    STATE.load(Ordering::Acquire) & !LOADED_BIT
}

fn writer() -> MutexGuard<'static, ()> {
    WRITER.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn stage_bit(token: u64) -> Option<u32> {
    let hash = token & HASH40_MASK;
    Stage::ALL
        .into_iter()
        .position(|stage| hash40(stage) == hash)
        .and_then(|index| u32::try_from(index).ok())
}

fn defaults() -> PoolView {
    PoolView {
        enabled: false,
        selected: [true; 11],
    }
}

fn shift(index: usize) -> Option<u32> {
    u32::try_from(index).ok().map(|count| 1u32 << count)
}

fn publish(pool: PoolView) {
    let mut bits = 0u32;
    for (index, on) in pool.selected.iter().enumerate() {
        if *on {
            if let Some(flag) = shift(index) {
                bits |= flag;
            }
        }
    }
    if pool.enabled {
        bits |= ENABLED_BIT;
    }
    STATE.store(bits | LOADED_BIT, Ordering::Release);
}

fn decode(bits: u32) -> PoolView {
    let mut selected = [false; 11];
    for (index, slot) in selected.iter_mut().enumerate() {
        if let Some(flag) = shift(index) {
            *slot = bits & flag != 0;
        }
    }
    PoolView {
        enabled: bits & ENABLED_BIT != 0,
        selected,
    }
}

fn read_settings(directory: &Path) -> Result<Option<PoolView>, ()> {
    let destination = directory.join(FILE_NAME);
    let (path, recovering) =
        elite_stages_counterpick_core::files::source_path(&destination).map_err(|_| ())?;
    // Pinned Skyline's `File::open` loses the typed NotFound, but
    // `fs::metadata` keeps it. Missing is decided here, before any open, so an
    // unreadable or malformed file is never mistaken for an absent one.
    let length = match fs::metadata(&path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    if length > LIMIT as u64 {
        return Err(());
    }
    let mut bytes = Vec::new();
    // Bound the read itself too: the file may grow after metadata.
    File::open(&path)
        .map_err(|_| ())?
        .take(LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if bytes.len() > LIMIT {
        return Err(());
    }
    let text = core::str::from_utf8(&bytes).map_err(|_| ())?;
    let pool = parse_settings(text)?;
    if recovering {
        elite_stages_counterpick_core::files::restore_validated(&path, &destination)
            .map_err(|_| ())?;
    }
    Ok(Some(pool))
}

fn write_settings(directory: &Path, pool: &PoolView) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let text = render(pool);
    let (temporary, mut file) = create_temporary(directory)?;
    let written = (|| -> io::Result<()> {
        file.write_all(text.as_bytes())?;
        file.sync_all()
    })();
    drop(file);
    let replaced = written.and_then(|()| {
        elite_stages_counterpick_core::files::replace(&temporary, &directory.join(FILE_NAME))
    });
    if let Err(error) = replaced {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

fn create_temporary(directory: &Path) -> io::Result<(std::path::PathBuf, File)> {
    for _ in 0..64 {
        let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!("{FILE_NAME}.pending-{id}"));
        // Explicit write/create seed proven by Training's replay store on the
        // pinned Skyline filesystem; `create_new` keeps it exclusive.
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
        "legal-stage temporary files occupy the available save slots",
    ))
}

fn render(pool: &PoolView) -> String {
    let mut text = String::from(HEADER);
    text.push_str(if pool.enabled {
        "enabled = true\n"
    } else {
        "enabled = false\n"
    });
    text.push_str("stages = [\n");
    for (stage, on) in Stage::ALL.into_iter().zip(pool.selected) {
        if on {
            text.push_str("    \"");
            text.push_str(stage.key());
            text.push_str("\",\n");
        }
    }
    text.push_str("]\n");
    text
}

fn parse_settings(text: &str) -> Result<PoolView, ()> {
    let mut scan = Scan {
        bytes: text.as_bytes(),
        index: 0,
    };
    scan.skip();
    scan.word(b"enabled")?;
    scan.skip();
    scan.punct(b'=')?;
    scan.skip();
    let enabled = scan.boolean()?;
    scan.skip();
    scan.word(b"stages")?;
    scan.skip();
    scan.punct(b'=')?;
    scan.skip();
    scan.punct(b'[')?;
    let mut selected = [false; 11];
    let mut count = 0usize;
    loop {
        scan.skip();
        if scan.eat(b']') {
            break;
        }
        let key = scan.quoted()?;
        let stage = Stage::from_key(key).ok_or(())?;
        let index = Stage::ALL
            .iter()
            .position(|item| *item == stage)
            .ok_or(())?;
        if selected[index] || count >= Stage::ALL.len() {
            return Err(());
        }
        selected[index] = true;
        count += 1;
        scan.skip();
        if scan.eat(b',') {
            continue;
        }
        scan.punct(b']')?;
        break;
    }
    scan.skip();
    if scan.index != scan.bytes.len() {
        return Err(());
    }
    Ok(PoolView { enabled, selected })
}

struct Scan<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl Scan<'_> {
    fn skip(&mut self) {
        loop {
            while self.index < self.bytes.len() && self.bytes[self.index].is_ascii_whitespace() {
                self.index += 1;
            }
            if self.index < self.bytes.len() && self.bytes[self.index] == b'#' {
                while self.index < self.bytes.len() && self.bytes[self.index] != b'\n' {
                    self.index += 1;
                }
                continue;
            }
            break;
        }
    }

    fn eat(&mut self, expected: u8) -> bool {
        if self.index < self.bytes.len() && self.bytes[self.index] == expected {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn punct(&mut self, expected: u8) -> Result<(), ()> {
        if self.eat(expected) {
            Ok(())
        } else {
            Err(())
        }
    }

    fn word(&mut self, expected: &[u8]) -> Result<(), ()> {
        let end = self.index + expected.len();
        if self.bytes.get(self.index..end) == Some(expected) {
            let boundary = self.bytes.get(end).copied().unwrap_or(b' ');
            if boundary.is_ascii_alphanumeric() || boundary == b'_' {
                return Err(());
            }
            self.index = end;
            Ok(())
        } else {
            Err(())
        }
    }

    fn boolean(&mut self) -> Result<bool, ()> {
        if self.word(b"true").is_ok() {
            Ok(true)
        } else if self.word(b"false").is_ok() {
            Ok(false)
        } else {
            Err(())
        }
    }

    fn quoted(&mut self) -> Result<&str, ()> {
        self.punct(b'"')?;
        let start = self.index;
        while self.index < self.bytes.len() && self.bytes[self.index] != b'"' {
            if self.bytes[self.index] == b'\\' || !self.bytes[self.index].is_ascii() {
                return Err(());
            }
            self.index += 1;
        }
        let key = core::str::from_utf8(&self.bytes[start..self.index]).map_err(|_| ())?;
        self.punct(b'"')?;
        Ok(key)
    }
}
