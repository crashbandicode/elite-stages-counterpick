//! Client of the existing imgui-smash host; never installs a renderer or NVN hook.
//! Binding follows the validated Deluxe external-host design: every callback
//! must belong to one mapped NRO with the pinned ImGui ABI tag in its RO segment.
use core::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
type Setup = unsafe extern "C" fn(unsafe extern "C" fn(*mut u64));
type Draw = unsafe extern "C" fn(*const c_void);
type Get = unsafe extern "C" fn() -> *mut u64;
static CONTEXT: AtomicUsize = AtomicUsize::new(0);
static GET: AtomicUsize = AtomicUsize::new(0);
extern "C" {
    fn igSetCurrentContext(context: *mut c_void);
}

pub struct Host {
    setup: Setup,
    draw: Draw,
    get: Option<Get>,
}
impl Host {
    pub unsafe fn install(self, callback: unsafe extern "C" fn()) {
        if let Some(get) = self.get {
            GET.store(get as usize, Ordering::Release);
        }
        (self.setup)(accept_context);
        (self.draw)(callback as *const c_void);
    }
}
unsafe extern "C" fn accept_context(context: *mut u64) {
    if !context.is_null() && (context as usize).is_multiple_of(8) {
        CONTEXT.store(context as usize, Ordering::Release);
    }
}
pub unsafe fn bind_context() -> bool {
    let mut context = CONTEXT.load(Ordering::Acquire);
    if context == 0 {
        let get = GET.load(Ordering::Acquire);
        if get == 0 {
            return false;
        }
        let get: Get = core::mem::transmute(get);
        accept_context(get());
        context = CONTEXT.load(Ordering::Acquire);
    }
    if context == 0 {
        return false;
    }
    // Host owns this process-lifetime context, supplied on setup. No fighter,
    // scene, menu draft, or resource pointer is retained here.
    igSetCurrentContext(context as *mut c_void);
    true
}

#[repr(C)]
#[derive(Default)]
struct MemoryInfo {
    address: u64,
    size: u64,
    kind: u32,
    attributes: u32,
    permission: u32,
    ipc_references: u32,
    device_references: u32,
    padding: u32,
}
unsafe fn region(address: usize) -> Option<(usize, usize)> {
    if address < 0x10000 {
        return None;
    }
    let mut info = MemoryInfo::default();
    let result: u64;
    core::arch::asm!("svc 0x6", inout("x0") &mut info as *mut MemoryInfo as u64 => result,
        lateout("x1") _, in("x2") address as u64, clobber_abi("C"), options(nostack));
    if result as u32 != 0 || info.size == 0 || info.permission & 1 == 0 {
        return None;
    }
    Some((info.address as usize, info.size as usize))
}
unsafe fn bytes(address: usize, size: usize) -> Option<&'static [u8]> {
    let (base, length) = region(address)?;
    if address < base || address.checked_add(size)? > base.checked_add(length)? {
        return None;
    }
    Some(core::slice::from_raw_parts(address as *const u8, size))
}
unsafe fn image(symbol: usize) -> Option<(usize, usize, usize)> {
    let (base, _) = region(symbol)?;
    let header = bytes(base, 0x80)?;
    if header.get(0x10..0x14)? != b"NRO0" {
        return None;
    }
    let word = |offset| {
        Some(u32::from_le_bytes(header.get(offset..offset + 4)?.try_into().ok()?) as usize)
    };
    let total = word(0x18)?;
    let text = word(0x24)?;
    let ro = word(0x28)?;
    let size = word(0x2c)?;
    if word(0x20)? != 0
        || total < 0x80
        || text > total
        || symbol.checked_sub(base)? >= text
        || ro.checked_add(size)? > total
        || size == 0
    {
        return None;
    }
    Some((base, base.checked_add(ro)?, size))
}
unsafe fn lookup(name: &[u8]) -> Option<usize> {
    let mut address = 0;
    skyline::nn::ro::LookupSymbol(&mut address, name.as_ptr());
    (address != 0).then_some(address)
}
pub unsafe fn resolve() -> Option<Host> {
    let setup = lookup(b"imgui_setup_context_export\0")?;
    let draw = lookup(b"imgui_smash_add_on_draw_frame_wrapper\0")?;
    let (base, ro, size) = image(setup)?;
    if image(draw)?.0 != base {
        return None;
    }
    let tag = b"1.92.7 WIP (19263)";
    if !bytes(ro, size)?.windows(tag.len()).any(|part| part == tag) {
        return None;
    }
    let get = match lookup(b"imgui_get_context_export\0") {
        Some(address) => {
            if image(address)?.0 != base {
                return None;
            }
            Some(core::mem::transmute::<usize, Get>(address))
        }
        None => None,
    };
    Some(Host {
        setup: core::mem::transmute::<usize, Setup>(setup),
        draw: core::mem::transmute::<usize, Draw>(draw),
        get,
    })
}
