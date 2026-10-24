//! Use the installed ImGui host's public C API. This plugin installs no drawing
//! hook and owns no GPU memory. Disk work runs on a separate settings worker.
use crate::{filter, hooks, imgui_host, pool, settings};
use core::ffi::c_char;
use elite_stages_counterpick_core::{Config, Fighter, Stage, FIGHTER_KEYS};
use std::ffi::{CStr, CString};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{sync_channel, SyncSender},
    Mutex, OnceLock,
};

#[repr(C)]
#[derive(Clone, Copy)]
struct Vec2 {
    x: f32,
    y: f32,
}
const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
// Pinned cimgui 1.92.7 WIP enum values; version checked before registration.
const DPAD_UP: i32 = 640;
const L1: i32 = 642;
const R1: i32 = 643;

struct Api {
    context: unsafe extern "C" fn() -> *mut core::ffi::c_void,
    begin: unsafe extern "C" fn(*const c_char, *mut bool, i32) -> bool,
    end: unsafe extern "C" fn(),
    text: unsafe extern "C" fn(*const c_char, *const c_char),
    checkbox: unsafe extern "C" fn(*const c_char, *mut bool) -> bool,
    button: unsafe extern "C" fn(*const c_char, Vec2) -> bool,
    combo: unsafe extern "C" fn(*const c_char, *const c_char, i32) -> bool,
    end_combo: unsafe extern "C" fn(),
    selectable: unsafe extern "C" fn(*const c_char, bool, i32, Vec2) -> bool,
    separator: unsafe extern "C" fn(),
    size: unsafe extern "C" fn(Vec2, i32),
    focus: unsafe extern "C" fn(),
    down: unsafe extern "C" fn(i32) -> bool,
    pressed: unsafe extern "C" fn(i32, bool) -> bool,
}
static API: OnceLock<Api> = OnceLock::new();
static SENDER: OnceLock<SyncSender<Command>> = OnceLock::new();
static NAMES: OnceLock<Vec<CString>> = OnceLock::new();
static OPEN: AtomicBool = AtomicBool::new(false);
static NEED_FOCUS: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<State> = Mutex::new(State {
    pool: None,
    config: None,
    fighter: 0,
    preferred: 0,
    busy: false,
    message: c"Open with L + R + D-pad Up.",
});

const STAGES: [&CStr; 11] = [
    c"Battlefield",
    c"Small Battlefield",
    c"Final Destination",
    c"Yoshi's Story",
    c"Lylat Cruise",
    c"Pokemon Stadium 2",
    c"Smashville",
    c"Kalos Pokemon League",
    c"Town and City",
    c"Northern Cave",
    c"Hollow Bastion",
];

struct State {
    pool: Option<pool::PoolView>,
    config: Option<Config>,
    fighter: usize,
    preferred: usize,
    busy: bool,
    message: &'static CStr,
}
enum Command {
    Refresh,
    SavePool(pool::PoolView),
    SaveConfig(Config),
}

// Direct links into the pinned client library; the host exports only callbacks.
extern "C" {
    fn igGetCurrentContext() -> *mut core::ffi::c_void;
    fn igBegin(name: *const c_char, open: *mut bool, flags: i32) -> bool;
    fn igEnd();
    fn igTextUnformatted(begin: *const c_char, end: *const c_char);
    fn igCheckbox(label: *const c_char, value: *mut bool) -> bool;
    fn igButton(label: *const c_char, size: Vec2) -> bool;
    fn igBeginCombo(label: *const c_char, preview: *const c_char, flags: i32) -> bool;
    fn igEndCombo();
    fn igSelectable_Bool(label: *const c_char, selected: bool, flags: i32, size: Vec2) -> bool;
    fn igSeparator();
    fn igSetNextWindowSize(size: Vec2, condition: i32);
    fn igSetNextWindowFocus();
    fn igIsKeyDown(key: i32) -> bool;
    fn igIsKeyPressed(key: i32, repeat: bool) -> bool;
}

fn linked_api() -> Api {
    Api {
        context: igGetCurrentContext,
        begin: igBegin,
        end: igEnd,
        text: igTextUnformatted,
        checkbox: igCheckbox,
        button: igButton,
        combo: igBeginCombo,
        end_combo: igEndCombo,
        selectable: igSelectable_Bool,
        separator: igSeparator,
        size: igSetNextWindowSize,
        focus: igSetNextWindowFocus,
        down: igIsKeyDown,
        pressed: igIsKeyPressed,
    }
}

pub fn start() {
    // Skyline loads all NROs before plugin mains. Register before NVN creates
    // the host context; no polling or renderer initialization is needed here.
    let Some(host) = (unsafe { imgui_host::resolve() }) else {
        skyline::println!("[CounterpickMenu] compatible ImGui host unavailable; file configuration remains active");
        return;
    };
    if API.set(linked_api()).is_err() {
        return;
    }
    let names = FIGHTER_KEYS
        .iter()
        .map(|name| CString::new(*name).expect("static fighter key"))
        .collect();
    if NAMES.set(names).is_err() {
        return;
    }
    let (send, receive) = sync_channel(1);
    if SENDER.set(send).is_err() {
        return;
    }
    let result = std::thread::Builder::new()
        .name("counterpick-settings".into())
        .stack_size(128 * 1024)
        .spawn(move || {
            while let Ok(command) = receive.recv() {
                let (message, refresh_pool, refresh_config) = match command {
                    Command::Refresh => (
                        c"Changes are drafts until saved. Closing discards unsaved edits.",
                        true,
                        true,
                    ),
                    Command::SavePool(pool) => {
                        match pool::replace_pool(pool.enabled, pool.selected) {
                            Ok(()) => {
                                hooks::configuration_changed();
                                (c"Stage pool saved.", true, false)
                            }
                            Err(pool::PoolError::Empty) => (
                                c"Keep at least one stage enabled. Nothing saved.",
                                false,
                                false,
                            ),
                            Err(_) => (
                                c"Stage pool could not be saved. Previous settings retained.",
                                false,
                                false,
                            ),
                        }
                    }
                    Command::SaveConfig(config) => match settings::save(config) {
                        Ok(()) => {
                            hooks::configuration_changed();
                            (c"Counterpick preferences saved.", false, true)
                        }
                        Err(_) => (
                            c"Counterpick settings could not be saved. Previous settings retained.",
                            false,
                            false,
                        ),
                    },
                };
                // Preserve the other section's unsaved draft. Disk work and store
                // locks finish before taking the UI lock, never on the draw thread.
                let pool = refresh_pool.then(pool::view);
                let config = refresh_config.then(settings::draft);
                if let Ok(mut state) = STATE.lock() {
                    if let Some(pool) = pool {
                        state.pool = pool;
                    }
                    if let Some(config) = config {
                        state.config = config;
                        refresh_preferred(&mut state);
                    }
                    state.busy = false;
                    state.message = message;
                }
            }
        });
    if let Err(error) = result {
        skyline::println!("[CounterpickMenu] worker unavailable: {}", error);
        return;
    }
    unsafe {
        host.install(draw);
    }
    skyline::println!("[CounterpickMenu] registered existing ImGui host");
}

fn request(state: &mut State, command: Command) {
    if state.busy {
        return;
    }
    match SENDER
        .get()
        .and_then(|sender| sender.try_send(command).ok())
    {
        Some(()) => {
            state.busy = true;
            state.message = c"Saving/loading...";
        }
        None => state.message = c"Settings worker unavailable. Nothing saved.",
    }
}

fn refresh_preferred(state: &mut State) {
    state.preferred = state
        .config
        .as_ref()
        .and_then(|cfg| cfg.ranking(Fighter::from_key(FIGHTER_KEYS[state.fighter])?))
        .and_then(|rank| rank.first())
        .and_then(|first| Stage::ALL.iter().position(|stage| stage == first))
        .unwrap_or(0);
}

unsafe extern "C" fn draw() {
    let Some(api) = API.get() else { return };
    if !imgui_host::bind_context() || (api.context)().is_null() {
        return;
    }
    let chord = (api.down)(L1)
        && (api.down)(R1)
        && (api.down)(DPAD_UP)
        && ((api.pressed)(L1, false) || (api.pressed)(R1, false) || (api.pressed)(DPAD_UP, false));
    if !chord && !OPEN.load(Ordering::Relaxed) {
        return;
    }
    let Some(mut state) = STATE.try_lock().ok() else {
        return;
    };
    if chord {
        let open = !OPEN.load(Ordering::Relaxed);
        OPEN.store(open, Ordering::Relaxed);
        if open {
            NEED_FOCUS.store(true, Ordering::Relaxed);
            request(&mut state, Command::Refresh);
        }
    }
    let mut open = OPEN.load(Ordering::Relaxed);
    if !open {
        return;
    }
    (api.size)(Vec2 { x: 610.0, y: 650.0 }, 4); // FirstUseEver
    if NEED_FOCUS.swap(false, Ordering::Relaxed) {
        (api.focus)();
    }
    let visible = (api.begin)(c"Legal Stages + Loser Counterpick".as_ptr(), &mut open, 0);
    if visible {
        render(api, &mut state);
    }
    (api.end)();
    OPEN.store(open, Ordering::Relaxed);
}

unsafe fn text(api: &Api, value: &CStr) {
    (api.text)(value.as_ptr(), core::ptr::null());
}

unsafe fn render(api: &Api, state: &mut State) {
    text(
        api,
        c"Local preferred rules only. The opponent's rules remain unchanged.",
    );
    if !filter::active() {
        text(
            api,
            c"Stage hook unavailable: incompatible game or another stage plugin owns it.",
        );
    }
    if !hooks::active() {
        text(
            api,
            c"Counterpick hook unavailable. Check diagnostics for compatibility.",
        );
    }
    text(api, state.message);
    if state.busy {
        return;
    }
    if let Some(pool) = state.pool.as_mut() {
        (api.checkbox)(c"Enable legal stage pool".as_ptr(), &mut pool.enabled);
        for (label, enabled) in STAGES.iter().zip(pool.selected.iter_mut()) {
            (api.checkbox)(label.as_ptr(), enabled);
        }
        if (api.button)(c"Save stage pool".as_ptr(), ZERO) {
            let copy = *pool;
            request(state, Command::SavePool(copy));
            return;
        }
    } else {
        text(
            api,
            c"Stage settings unavailable. Existing file was not overwritten.",
        );
    }
    (api.separator)();
    let Some(config) = state.config.as_mut() else {
        text(
            api,
            c"Counterpick settings unavailable. Existing file was not overwritten.",
        );
        return;
    };
    let mut enabled = config.enabled();
    if (api.checkbox)(c"Optimal counter pick".as_ptr(), &mut enabled) {
        config.set_enabled(enabled);
    }
    if enabled && !state.pool.is_some_and(|pool| pool.enabled) {
        text(api, c"Counterpick needs the legal stage pool enabled.");
    }

    let Some(names) = NAMES.get() else { return };
    let mut changed = false;
    if (api.combo)(c"Fighter".as_ptr(), names[state.fighter].as_ptr(), 0) {
        for (index, label) in names.iter().enumerate() {
            if (api.selectable)(label.as_ptr(), index == state.fighter, 0, ZERO) {
                state.fighter = index;
                changed = true;
            }
        }
        (api.end_combo)();
    }
    if changed {
        refresh_preferred(state);
    }
    if (api.combo)(
        c"Preferred stage".as_ptr(),
        STAGES[state.preferred].as_ptr(),
        0,
    ) {
        for (index, label) in STAGES.iter().enumerate() {
            if (api.selectable)(label.as_ptr(), index == state.preferred, 0, ZERO) {
                state.preferred = index;
            }
        }
        (api.end_combo)();
    }
    let fighter = Fighter::from_key(FIGHTER_KEYS[state.fighter]).expect("static fighter key");
    let Some(config) = state.config.as_mut() else {
        return;
    };
    if (api.button)(c"Make first preference".as_ptr(), ZERO) {
        let first = Stage::ALL[state.preferred];
        let mut order = [first; 11];
        let mut count = 1;
        if let Some(old) = config.ranking(fighter) {
            for &stage in old {
                if stage != first {
                    order[count] = stage;
                    count += 1;
                }
            }
        }
        let _ = config.set_ranking(fighter, &order[..count]);
    }
    if (api.button)(c"Clear this fighter's preferences".as_ptr(), ZERO) {
        let _ = config.set_ranking(fighter, &[]);
    }
    text(api, c"Preference order (disabled stages are skipped):");
    if let Some(ranking) = config.ranking(fighter) {
        for &stage in ranking {
            text(api, STAGES[stage as usize]);
        }
    }
    text(
        api,
        c"Full ordered preferences can also be edited in counterpick.cfg.",
    );
    if (api.button)(c"Save counterpick preferences".as_ptr(), ZERO) {
        let copy = config.clone();
        request(state, Command::SaveConfig(copy));
    }
}
