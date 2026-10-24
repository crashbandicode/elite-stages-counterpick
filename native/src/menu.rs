//! Registers the stage pool and counterpick preferences as Online Deluxe pages.
//!
//! Callbacks run on Deluxe's overlay draw thread. They update an in-memory
//! draft and queue the existing settings worker; they do not write files or
//! call back into the host. The host draws the rows with its own ImGui renderer.
use std::ffi::{c_char, CString};
use std::ops::Range;
use std::sync::{
    mpsc::{sync_channel, Receiver, SyncSender},
    Mutex, OnceLock,
};

use crate::abi::{self, CurrentFn, OptionDesc, PluginDesc, RequestFn};
use crate::{filter, hooks, imgui_host, page, pool, settings};

static STATE: Mutex<Option<page::MenuDraft>> = Mutex::new(None);
static SENDER: OnceLock<SyncSender<page::Save>> = OnceLock::new();

pub fn start() {
    let Some(host) = (unsafe { imgui_host::resolve() }) else {
        skyline::println!(
            "[CounterpickMenu] Deluxe plugin menu unavailable; file configuration remains active"
        );
        return;
    };
    let initial = page::MenuDraft {
        pool: pool::view().map(|view| page::PoolDraft {
            enabled: view.enabled,
            selected: view.selected,
        }),
        config: settings::draft(),
        busy: false,
        failed: false,
    };
    match STATE.lock() {
        Ok(mut state) => *state = Some(initial),
        Err(_) => return,
    }
    let (send, receive) = sync_channel(1);
    if SENDER.set(send).is_err() {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("counterpick-settings".into())
        .stack_size(128 * 1024)
        .spawn(move || worker(receive));
    if let Err(error) = spawned {
        skyline::println!("[CounterpickMenu] worker unavailable: {error}");
        return;
    }
    for id in page::PageId::ALL {
        let code = register(&host, id);
        skyline::println!("[CounterpickMenu] {} registration {code}", id.meta().id);
    }
}

fn worker(receive: Receiver<page::Save>) {
    while let Ok(save) = receive.recv() {
        match save {
            page::Save::Pool(draft) => match pool::replace_pool(draft.enabled, draft.selected) {
                Ok(()) => {
                    hooks::configuration_changed();
                    with_draft(page::finish_save);
                }
                Err(_) => {
                    skyline::println!("[CounterpickMenu] pool save refused");
                    let published = pool::view().map(|view| page::PoolDraft {
                        enabled: view.enabled,
                        selected: view.selected,
                    });
                    with_draft(|menu| page::fail_pool(menu, published));
                }
            },
            page::Save::Config(config) => match settings::save(config) {
                Ok(()) => {
                    hooks::configuration_changed();
                    with_draft(page::finish_save);
                }
                Err(error) => {
                    skyline::println!("[CounterpickMenu] counterpick save refused: {error}");
                    let published = settings::draft();
                    with_draft(|menu| page::fail_config(menu, published));
                }
            },
        }
    }
}

fn with_draft(update: impl FnOnce(&mut page::MenuDraft)) {
    let Ok(mut state) = STATE.lock() else {
        return;
    };
    if let Some(draft) = state.as_mut() {
        update(draft);
    }
}

fn enqueue(save: &page::Save) -> bool {
    SENDER
        .get()
        .is_some_and(|sender| sender.try_send(save.clone()).is_ok())
}

fn answer(id: page::PageId, option: u32) -> u32 {
    let Ok(state) = STATE.try_lock() else {
        return page::CHOICE_UNAVAILABLE;
    };
    let Some(draft) = state.as_ref() else {
        return page::CHOICE_UNAVAILABLE;
    };
    let hooks = page::Hooks {
        pool: filter::active(),
        result: hooks::active(),
    };
    match page::current(id, draft, hooks, option as usize) {
        page::Answer::Choice(choice) => choice as u32,
        page::Answer::Unavailable => page::CHOICE_UNAVAILABLE,
    }
}

fn request(id: page::PageId, option: u32, choice: u32) -> u32 {
    let Ok(mut state) = STATE.try_lock() else {
        return page::REQUEST_REJECTED;
    };
    let Some(draft) = state.as_mut() else {
        return page::REQUEST_UNAVAILABLE;
    };
    let decision = page::edit(draft, id, option as usize, choice as usize, enqueue);
    page::code(&decision)
}

unsafe extern "C" fn current_controls(_: *mut core::ffi::c_void, option: u32) -> u32 {
    answer(page::PageId::Controls, option)
}
unsafe extern "C" fn request_controls(_: *mut core::ffi::c_void, option: u32, choice: u32) -> u32 {
    request(page::PageId::Controls, option, choice)
}

fn callbacks(id: page::PageId) -> (CurrentFn, RequestFn) {
    match id {
        page::PageId::Controls => (current_controls, request_controls),
    }
}

struct Built {
    _text: Vec<CString>,
    _choices: Vec<Vec<*const c_char>>,
    _tones: Vec<Vec<u8>>,
    options: Vec<OptionDesc>,
    id: CString,
    label: CString,
}

fn build(id: page::PageId) -> Option<Built> {
    let meta = id.meta();
    // These pages own their file writes; host INI persistence is not supported.
    if !meta.own_page || meta.config_path.is_some() {
        return None;
    }
    let mut text = Vec::new();
    let mut rows = Vec::with_capacity(meta.options.len());
    for option in meta.options {
        let id_at = text.len();
        text.push(CString::new(option.id).ok()?);
        let label_at = text.len();
        text.push(CString::new(option.label).ok()?);
        let choice_at = text.len();
        for choice in option.choices {
            text.push(CString::new(*choice).ok()?);
        }
        rows.push((
            id_at,
            label_at,
            choice_at..text.len(),
            option.tones,
            page::option_flags(option.kind),
        ));
    }
    let mut choices = Vec::with_capacity(rows.len());
    for (_, _, range, _, _) in &rows {
        choices.push(pointers(&text, range));
    }
    let mut tones = Vec::with_capacity(rows.len());
    let mut options = Vec::with_capacity(rows.len());
    for (index, (id_at, label_at, _, tone, flags)) in rows.iter().enumerate() {
        tones.push(tone.map(|bytes| bytes.to_vec()).unwrap_or_default());
        let tone_ptr = if tones[index].is_empty() {
            core::ptr::null()
        } else {
            tones[index].as_ptr()
        };
        let choice_ptr = if choices[index].is_empty() {
            core::ptr::null()
        } else {
            choices[index].as_ptr()
        };
        options.push(OptionDesc {
            id: text[*id_at].as_ptr(),
            label: text[*label_at].as_ptr(),
            choices: choice_ptr,
            choice_count: choices[index].len() as u32,
            flags: *flags,
            config_key: core::ptr::null(),
            choice_values: core::ptr::null(),
            choice_tones: tone_ptr,
        });
    }
    Some(Built {
        _text: text,
        _choices: choices,
        _tones: tones,
        options,
        id: CString::new(meta.id).ok()?,
        label: CString::new(meta.label).ok()?,
    })
}

fn pointers(text: &[CString], range: &Range<usize>) -> Vec<*const c_char> {
    text[range.clone()].iter().map(|text| text.as_ptr()).collect()
}

fn register(host: &imgui_host::Host, id: page::PageId) -> i32 {
    let Some(built) = build(id) else {
        return abi::REGISTER_ERR_INVALID;
    };
    let (current, request) = callbacks(id);
    let desc = PluginDesc {
        struct_size: core::mem::size_of::<PluginDesc>() as u32,
        abi_version: page::ABI_VERSION,
        id: built.id.as_ptr(),
        label: built.label.as_ptr(),
        options: built.options.as_ptr(),
        option_count: built.options.len() as u32,
        flags: page::FLAG_OWN_PAGE,
        user: core::ptr::null_mut(),
        current: Some(current),
        request: Some(request),
        config_path: core::ptr::null(),
    };
    // `desc` holds raw pointers into `built`. Keep that storage alive until
    // the host copies it and returns.
    let _alive = (
        &built._text,
        &built._choices,
        &built._tones,
        &built.id,
        &built.label,
    );
    unsafe { host.register(&desc) }
}
