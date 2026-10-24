//! Hook the game eligibility predicate, not the original plugin export.
//!
//! `A64HookFunction` reaches the replacement with `B` or `BR`, so `x30` at
//! `elite_cp_legal_filter_entry` is still the game caller. The entry copies that
//! register to `x2` before the Rust prologue. A normal wrapper that
//! called the plugin's `can_select` would replace this return address and
//! the original filter would not run. This module does not call that export.
//!
//! Qualified caller: the return address of the `BL` at main+0x3271a30, which
//! is main+0x3271a34. The other direct caller, main+0x2310ba8, keeps the
//! native result. The post-result eligible pop is not hooked here.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use skyline::hooks::{getRegionAddress, A64HookFunction, Region};

use crate::pool::{self, ENABLED_BIT, STAGE_MASK};

const PREDICATE: usize = 0x32b3400;
const QUALIFIED_CALL: usize = 0x3271a30;
const QUALIFIED_RETURN: u64 = 0x3271a34;
const OTHER_CALL: usize = 0x2310ba8;
const PREDICATE_WORD: u32 = 0xa9bd57f6;
const QUALIFIED_WORD: u32 = 0x94010674;
const OTHER_WORD: u32 = 0x943e8a16;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static MAIN: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);

core::arch::global_asm!(
    ".globl elite_cp_legal_filter_entry",
    ".type elite_cp_legal_filter_entry, %function",
    "elite_cp_legal_filter_entry:",
    "mov x2, x30",
    "b {body}",
    body = sym elite_cp_legal_filter_body,
);

pub fn start() {
    if pool::startup() == pool::Startup::Refused {
        skyline::println!("[LegalStages] refused settings");
        return;
    }
    let main = unsafe { getRegionAddress(Region::Text) as usize };
    if main == 0 || !unsafe { fingerprints(main) } {
        skyline::println!("[LegalStages] refused fingerprint");
        return;
    }
    let mut original = core::ptr::null_mut();
    unsafe {
        A64HookFunction(
            (main + PREDICATE) as *const core::ffi::c_void,
            elite_cp_legal_filter_entry as *const core::ffi::c_void,
            &mut original,
        );
    }
    if original.is_null() {
        skyline::println!("[LegalStages] refused hook");
        return;
    }
    ORIGINAL.store(original as usize, Ordering::Release);
    MAIN.store(main, Ordering::Release);
    ACTIVE.store(true, Ordering::Release);
    skyline::println!("[LegalStages] armed");
}

pub fn active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

unsafe fn fingerprints(main: usize) -> bool {
    word(main, PREDICATE) == PREDICATE_WORD
        && word(main, QUALIFIED_CALL) == QUALIFIED_WORD
        && word(main, OTHER_CALL) == OTHER_WORD
        && bl_target(QUALIFIED_CALL, QUALIFIED_WORD) == Some(PREDICATE as u64)
        && bl_target(OTHER_CALL, OTHER_WORD) == Some(PREDICATE as u64)
}

unsafe fn word(main: usize, offset: usize) -> u32 {
    core::ptr::read_volatile((main + offset) as *const u32)
}

fn bl_target(site: usize, instruction: u32) -> Option<u64> {
    if instruction & 0xfc00_0000 != 0x9400_0000 {
        return None;
    }
    let imm26 = (instruction & 0x03ff_ffff) as i32;
    let signed = (imm26 << 6) >> 6;
    Some((site as u64).wrapping_add((signed as i64 as u64).wrapping_mul(4)))
}

#[no_mangle]
pub unsafe extern "C" fn elite_cp_legal_filter_body(record: u64, token: u64, caller: u64) -> u64 {
    let trampoline = ORIGINAL.load(Ordering::Acquire);
    if trampoline == 0 {
        return 0;
    }
    let original: unsafe extern "C" fn(u64, u64) -> u64 = core::mem::transmute(trampoline);
    let native = original(record, token);
    let kept = u64::from(native as u32);
    if !ACTIVE.load(Ordering::Acquire) {
        return kept;
    }
    let main = MAIN.load(Ordering::Acquire) as u64;
    if main == 0 || caller.wrapping_sub(main) != QUALIFIED_RETURN {
        return kept;
    }
    let bits = pool::state_word();
    if bits & ENABLED_BIT == 0 || bits & STAGE_MASK == 0 {
        return kept;
    }
    if kept == 0 {
        return 0;
    }
    match pool::stage_bit(token) {
        Some(bit) if bits & (1u32 << bit) != 0 => kept,
        _ => 0,
    }
}

extern "C" {
    fn elite_cp_legal_filter_entry();
}
