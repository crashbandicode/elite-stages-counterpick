//! Client of the installed Online Deluxe plugin menu.
//!
//! Resolves `ssbu_online_deluxe_plugin_api_v1` from one mapped NRO. The whole
//! table must be readable in one mapped region before it is dereferenced, and
//! `register` must lie in that same NRO text segment. This plugin does not link a
//! second ImGui client, create a context, or install a renderer or NVN hook.
use core::mem::{align_of, size_of};

use crate::abi::{self, PluginApiV1, PluginDesc};

type Entry = unsafe extern "C" fn() -> *const PluginApiV1;

pub struct Host {
    register: unsafe extern "C" fn(desc: *const PluginDesc) -> i32,
}

impl Host {
    pub unsafe fn register(&self, desc: *const PluginDesc) -> i32 {
        (self.register)(desc)
    }
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
    core::arch::asm!(
        "svc 0x6",
        inout("x0") &mut info as *mut MemoryInfo as u64 => result,
        lateout("x1") _,
        in("x2") address as u64,
        clobber_abi("C"),
        options(nostack)
    );
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

unsafe fn image(symbol: usize) -> Option<(usize, usize)> {
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
    if word(0x20)? != 0 || total < 0x80 || text > total || symbol.checked_sub(base)? >= text {
        return None;
    }
    Some((base, text))
}

unsafe fn lookup(name: &[u8]) -> Option<usize> {
    let mut address = 0;
    skyline::nn::ro::LookupSymbol(&mut address, name.as_ptr());
    (address != 0).then_some(address)
}

pub unsafe fn resolve() -> Option<Host> {
    let Some(address) = lookup(abi::API_SYMBOL) else {
        skyline::println!("[CounterpickMenu] Deluxe plugin API symbol missing");
        return None;
    };
    let Some(expected) = image(address) else {
        skyline::println!("[CounterpickMenu] Deluxe plugin API image rejected");
        return None;
    };
    let entry: Entry = core::mem::transmute(address);
    let table = entry();
    let table_address = table as usize;
    if table.is_null()
        || !table_address.is_multiple_of(align_of::<PluginApiV1>())
        || bytes(table_address, size_of::<PluginApiV1>()).is_none()
    {
        skyline::println!("[CounterpickMenu] Deluxe plugin API table rejected");
        return None;
    }
    let table = &*table;
    if table.struct_size as usize != size_of::<PluginApiV1>()
        || table.abi_version != abi::TABLE_VERSION
    {
        skyline::println!(
            "[CounterpickMenu] Deluxe plugin API version refused size={} version={}",
            table.struct_size,
            table.abi_version
        );
        return None;
    }
    let register = table.register;
    if image(register as usize) != Some(expected) {
        skyline::println!("[CounterpickMenu] Deluxe plugin API register rejected");
        return None;
    }
    Some(Host { register })
}
