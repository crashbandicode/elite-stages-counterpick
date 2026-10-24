//! Winner-callback baseline and the post-result pop.
//!
//! The pop hook runs before `ldur x0, [x9, #-8]!` at main+0x17a97e8. It may
//! exchange two full tokens inside `[begin, end)`. The original instruction
//! then loads that last token and the next instruction stores the new end.
//! The vector header is not written here.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::{filter, pool, settings};
use elite_stages_counterpick_core::construction::{
    CounterObservation, LocalConstruction, MatchStart, NativeParticipant, ParticipantRow,
    ResultSession, Route,
};
use elite_stages_counterpick_core::{mode_vector_offset, Fighter, PopRoute, Selection};
use skyline::hooks::{getRegionAddress, InlineCtx, Region};
use skyline::nn::ro::LookupSymbol;

/// Fail-closed stack copy. `0x1792240` keeps its three mode pools in heap
/// vectors (frame `0xc0`; frees at `0x17923e4`, `0x17923f4`, `0x1792404`).
/// Those receipts contain no fixed capacity. Pop65 held 10 full tokens.
/// A longer span stays with the native pop, tokens unmoved.
const MAX_TOKENS: usize = 16;
const ROW_BASE: usize = 0x52c5730;
const ROW_STRIDE: usize = 0x260;
const WIN_OFF: usize = 0x256;
const LIST_COUNT: usize = 0x52cc5f6;
const LIST_WORDS: usize = 0x52cc5fc;
const ROW_INDEX_BASE: usize = 0x52c5180;

const POP_RETURN: usize = 0x17923a0;
const PREFERRED_ON: usize = 0x1791ea8;
const PREFERRED_OFF: usize = 0x1791e24;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static MAIN: AtomicUsize = AtomicUsize::new(0);
static CONFIG_EPOCH: AtomicU64 = AtomicU64::new(1);

struct Visit {
    ticket: u64,
    winner: u8,
    /// Construction is waiting for the post-result pop.
    open: bool,
    config_epoch: u64,
}

struct Slot {
    bridge: LocalConstruction,
    visit: Visit,
    session: ResultSession,
}

static SLOT: Mutex<Slot> = Mutex::new(Slot {
    bridge: LocalConstruction::new(),
    visit: Visit {
        ticket: 0,
        winner: 0,
        open: false,
        config_epoch: 0,
    },
    session: ResultSession::new(),
});

pub fn start() {
    if !filter::active() {
        skyline::println!("[CounterpickAdapter] legal-stage adapter unavailable");
        return;
    }
    let main = unsafe { getRegionAddress(Region::Text) as usize };
    if main == 0 || !unsafe { fingerprints(main) } {
        skyline::println!("[CounterpickAdapter] refused fingerprint");
        return;
    }
    MAIN.store(main, Ordering::Release);
    skyline::install_hooks!(
        elite_cp_winner_increment,
        elite_cp_eligible_pop,
        elite_cp_result_exit
    );
    ACTIVE.store(true, Ordering::Release);
    skyline::println!("[CounterpickAdapter] armed");
}

pub fn active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

/// Menu save publishes settings first, then invalidates an older result's intent.
/// No mutex or native memory is accessed from the menu's persistence worker.
pub fn configuration_changed() {
    CONFIG_EPOCH.fetch_add(1, Ordering::AcqRel);
}

unsafe fn fingerprints(main: usize) -> bool {
    const TRAIN: [u8; 32] = [
        0xfd, 0x7b, 0xbf, 0xa9, 0xfd, 0x03, 0x00, 0x91, 0x08, 0xe5, 0x01, 0xf0, 0x08, 0x61, 0x13,
        0x91, 0x08, 0xfd, 0xdf, 0x08, 0x48, 0x01, 0x00, 0x36, 0xa8, 0xe2, 0x01, 0x90, 0x08, 0xc1,
        0x46, 0x39,
    ];
    let mut predicate = 0usize;
    LookupSymbol(
        &mut predicate,
        c"_ZN3app9smashball16is_training_modeEv".as_ptr().cast(),
    );
    if predicate != main + 0x1671650
        || std::slice::from_raw_parts(predicate as *const u8, TRAIN.len()) != TRAIN
    {
        return false;
    }
    const POPUP: [u32; 10] = [
        0x9000fe48, 0xf9441108, 0xf9400514, 0xf9408a88, 0xaa0003f3, 0xf9404100, 0x97ae214f,
        0xb940fa88, 0x7100011f, 0x39450a89,
    ];
    words(main, 0x33678dc, &POPUP)
        && words(
            main,
            0x1791c60,
            &[
                0xa9ba6ffc, 0xa90167fa, 0xa9025ff8, 0xa90357f6, 0xa9044ff4, 0xa9057bfd, 0x910143fd,
                0xd10843ff,
            ],
        )
        && words(main, 0x2316604, &[0x97d1ed97, 0x6b15027f])
        && words(main, 0x2368f98, &[0x97d0a332, 0xd0017d08])
        && words(main, 0x236959c, &[0x97d0a1b1, 0x90017cf8])
        && words(main, 0x3349de4, &[0x97911f9f, 0x08dfff68])
        && words(main, 0x1791e20, &[0x94000108])
        && words(main, 0x1791e24, &[0x528043c1])
        && words(main, 0x1791ea4, &[0x940000e7])
        && words(main, 0x1791ea8, &[0x79400661])
        && words(main, 0x17923a0, &[0x2a1f03e1])
        && words(
            main,
            0x17a97dc,
            &[0xa940250a, 0xeb09015f, 0x54000080, 0xf85f8d20, 0xf9000509],
        )
        && words(
            main,
            0x14fac08,
            &[
                0xd001ee90, 0x91176210, 0xb9800131, 0x9b0c2e32, 0x39601a51, 0x11000620,
                0x13001c11, 0x6b0a023f, 0x39201a40,
            ],
        )
        // Exact results-exit function qualified by the replay helper. The
        // earlier word window came from inside another routine, not this entry.
        // Live 13.0.5 capture matched the same complete 0xd0-byte fingerprint.
        && fingerprint(main, 0x150e9d0, 0xd0, 0x7900bafaf1df025f)
}

unsafe fn fingerprint(main: usize, offset: usize, length: usize, expected: u64) -> bool {
    let bytes = std::slice::from_raw_parts((main + offset) as *const u8, length);
    bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    }) == expected
}

unsafe fn words(main: usize, at: usize, expect: &[u32]) -> bool {
    let site = (main + at) as *const u32;
    expect
        .iter()
        .enumerate()
        .all(|(i, word)| site.add(i).read_volatile() == *word)
}

unsafe fn rd<T: Copy>(addr: usize) -> Option<T> {
    if addr == 0 || !addr.is_multiple_of(std::mem::align_of::<T>()) {
        None
    } else {
        Some(std::ptr::read_volatile(addr as *const T))
    }
}

fn online(main: usize) -> bool {
    unsafe {
        rd::<u8>(main + 0x53144d8).is_some_and(|b| b & 1 != 0)
            && rd::<u8>(main + 0x52c51b0) == Some(0x0e)
            && rd::<u32>(main + 0x53050f0) == Some(0x0701_0101)
    }
}

fn fighter_of(kind: i8) -> Option<Fighter> {
    // Signed E+8 equals FighterEntry+0x18 in bind_saved_entry. That number is
    // the entry-descriptor kind from_entry_kind maps. A negative byte is outside it.
    Fighter::from_entry_kind(u32::try_from(kind).ok()?)
}

fn pair(main: usize) -> Option<[ParticipantRow; 2]> {
    let mut found = [ParticipantRow {
        identity: NativeParticipant {
            number: 0,
            fighter_token: 0,
            fighter_kind: 0,
            fighter: None,
        },
        active: false,
        wins: 0,
    }; 2];
    let mut count = 0usize;
    for index in 0..8u8 {
        let row = main + ROW_BASE + usize::from(index) * ROW_STRIDE;
        let part = unsafe { rd::<u8>(row + 0x0a)? };
        if part == 2 {
            continue;
        }
        if count == 2 {
            return None;
        }
        let number = unsafe { rd::<u32>(row + 0x14)? };
        if number != u32::from(index) || number >= 8 {
            return None;
        }
        let kind = unsafe { rd::<i8>(row + 0x08)? };
        let kind_bits = u32::from(kind.cast_unsigned());
        found[count] = ParticipantRow {
            identity: NativeParticipant {
                number: index,
                fighter_token: unsafe { rd::<u64>(row)? },
                fighter_kind: kind_bits,
                fighter: fighter_of(kind),
            },
            active: true,
            wins: unsafe { rd::<u8>(row + WIN_OFF)? },
        };
        count += 1;
    }
    (count == 2).then_some(found)
}

/// The winner callback begins the results epoch before its UI exists. A new
/// epoch replaces any abandoned intent; a duplicate keeps its original baseline.
fn begin_visit(slot: &mut Slot) -> bool {
    let Some(ticket) = slot.session.begin() else {
        return false;
    };
    slot.visit = Visit {
        ticket,
        winner: 0,
        open: false,
        config_epoch: CONFIG_EPOCH.load(Ordering::Acquire),
    };
    true
}

// Skyline's inline dispatcher consumes x16/x17 before saving InlineCtx. Hook
// before the native ADRP/ADD and LDRSW define them, never at the later MADD
// that needs their live values. The native CMP also defines NZCV before use.
#[skyline::hook(offset = 0x14fac08, inline)]
unsafe fn elite_cp_winner_increment(ctx: &mut InlineCtx) {
    if !ACTIVE.load(Ordering::Acquire)
        || !settings::enabled()
        || !pool::view().is_some_and(|pool| pool.enabled && !pool.stage_pool().is_empty())
    {
        return;
    }
    let main = MAIN.load(Ordering::Acquire);
    let x8 = ctx.registers[8].x();
    let x9 = ctx.registers[9].x();
    let x11 = ctx.registers[11].x();
    let x12 = ctx.registers[12].x();
    // Before the list loop: x8 is still the count, x9 still the first word.
    // The following ldrb/add/strb is the native +1. This callback does not store it.
    if x12 != 0x260
        || x11 != (main + ROW_INDEX_BASE) as u64
        || x9 != (main + LIST_WORDS) as u64
        || x8 != 1
    {
        return;
    }
    // Read the same source as the upcoming native LDRSW. x17 in InlineCtx
    // belongs to Skyline's handler, not the original winner register.
    let Some(listed) = rd::<u32>(main + LIST_WORDS).filter(|p| *p < 8) else {
        return;
    };
    let count = rd::<u8>(main + LIST_COUNT);
    if count != Some(1) {
        return;
    }
    let Some(mut slot) = SLOT.try_lock().ok() else {
        return;
    };
    // One finish for this results session. A later hit keeps the first
    // pre-increment image and does not add another win.
    let fresh = begin_visit(&mut slot);
    skyline::println!(
        "[CounterpickAdapter] finish ticket={} fresh={} pending={}",
        slot.visit.ticket,
        fresh,
        slot.visit.open
    );
    if !fresh {
        return;
    }
    let Some(rows) = pair(main) else {
        skyline::println!("[CounterpickAdapter] finish refused participant rows");
        slot.bridge.invalidate();
        return;
    };
    let winner = listed as u8;
    if !rows.iter().any(|row| row.identity.number == winner) || !online(main) {
        skyline::println!("[CounterpickAdapter] finish refused winner/online scope");
        slot.bridge.invalidate();
        return;
    }
    let ticket = slot.visit.ticket;
    if !slot.bridge.start_match(MatchStart {
        ticket,
        online_1v1: true,
        rows,
    }) {
        return;
    }
    let mut now = rows;
    for row in &mut now {
        if row.identity.number == winner {
            row.wins = row.wins.wrapping_add(1);
        }
    }
    let list = [winner];
    let ok = slot.bridge.observe_finish(&CounterObservation {
        ticket,
        online_1v1: true,
        rows: now,
        winner_numbers: &list,
    });
    if ok {
        slot.visit.winner = winner;
        slot.visit.open = true;
        skyline::println!(
            "[CounterpickAdapter] result ticket={} winner={} participant_kinds={}/{}",
            ticket,
            winner,
            rows[0].identity.fighter_kind,
            rows[1].identity.fighter_kind
        );
    } else {
        slot.visit.open = false;
    }
}

#[skyline::hook(offset = 0x17a97e8, inline)]
unsafe fn elite_cp_eligible_pop(ctx: &mut InlineCtx) {
    if !ACTIVE.load(Ordering::Acquire) {
        return;
    }
    let main = MAIN.load(Ordering::Acquire);
    // Before `ldur x0, [x9, #-8]!`. x9 is still the vector end. x29 is unchanged
    // by that load, so the saved-LR walk is the same one sampled at the next instruction.
    let Some(callers) = frame_chain(main, ctx.registers[29].x(), ctx.sp.x()) else {
        return;
    };
    if callers[0] != POP_RETURN || (callers[1] != PREFERRED_ON && callers[1] != PREFERRED_OFF) {
        return;
    }
    let Some(mut slot) = SLOT.try_lock().ok() else {
        return;
    };
    let preferred = callers[1] == PREFERRED_ON;
    let route = Route::from_main_relative_lr(callers[2]);
    if route != Route::PostResult
        || !slot.visit.open
        || !online(main)
        || slot.visit.config_epoch != CONFIG_EPOCH.load(Ordering::Acquire)
        || !settings::enabled()
        || !pool::view().is_some_and(|pool| pool.enabled && !pool.stage_pool().is_empty())
    {
        if route != Route::PostResult {
            // Snapshot first: the receiver borrow of `slot.bridge` must not
            // overlap the read of `slot.visit`.
            let ticket = slot.visit.ticket;
            slot.bridge.enter(route, &empty_obs(ticket));
        } else {
            slot.bridge.invalidate();
        }
        drop_visit(&mut slot, "scope-or-settings");
        return;
    }
    let Some(rows) = pair(main) else {
        slot.bridge.invalidate();
        drop_visit(&mut slot, "participant-rows");
        return;
    };
    let mode_reg = ctx.registers[19].x();
    let header = ctx.registers[8].x();
    let pool = ctx.registers[20].x();
    let mode = u32::try_from(mode_reg).unwrap_or(u32::MAX);
    if mode > 2 || header != pool + mode_vector_offset(mode) as u64 {
        slot.bridge.invalidate();
        drop_visit(&mut slot, "pool-header");
        return;
    }
    let list = [slot.visit.winner];
    let ticket = slot.visit.ticket;
    let obs = CounterObservation {
        ticket,
        online_1v1: true,
        rows,
        winner_numbers: &list,
    };
    let Some(token) = slot.bridge.enter(Route::PostResult, &obs) else {
        drop_visit(&mut slot, "result-delta");
        return;
    };
    if let Some((copied, count)) =
        copy_span(header as usize, ctx.registers[10].x(), ctx.registers[9].x())
    {
        let choice = settings::with_config(|config| {
            slot.bridge.propose(
                token,
                config,
                preferred,
                PopRoute::LocalEligiblePop,
                &copied[..count],
            )
        });
        skyline::println!(
            "[CounterpickAdapter] proposal ticket={} preferred={} choice={:?}",
            ticket,
            preferred,
            choice
        );
        if let Some(Selection::PositionLast { index, .. }) = choice {
            place_last(ctx.registers[10].x() as usize, count, index);
        }
    }
    slot.bridge.leave(token);
    drop_visit(&mut slot, "consumed");
}

#[skyline::hook(offset = 0x150e9d0, inline)]
unsafe fn elite_cp_result_exit(_ctx: &mut InlineCtx) {
    if !ACTIVE.load(Ordering::Acquire) {
        return;
    }
    let Some(mut slot) = SLOT.try_lock().ok() else {
        return;
    };
    // Same edge the results helper uses to end an epoch. An open construction
    // stays open for the post-result pop and revalidates the native row delta.
    if !slot.session.close() {
        return;
    }
    skyline::println!(
        "[CounterpickAdapter] result exit ticket={} pending={}",
        slot.visit.ticket,
        slot.visit.open
    );
}

fn drop_visit(slot: &mut Slot, reason: &str) {
    if slot.visit.open {
        skyline::println!(
            "[CounterpickAdapter] close ticket={} reason={}",
            slot.visit.ticket,
            reason
        );
    }
    slot.visit.open = false;
}

fn empty_obs(ticket: u64) -> CounterObservation<'static> {
    static NONE: [u8; 0] = [];
    CounterObservation {
        ticket,
        online_1v1: false,
        rows: [inactive(), inactive()],
        winner_numbers: &NONE,
    }
}

fn inactive() -> ParticipantRow {
    ParticipantRow {
        identity: NativeParticipant {
            number: 0,
            fighter_token: 0,
            fighter_kind: 0,
            fighter: None,
        },
        active: false,
        wins: 0,
    }
}

unsafe fn frame_chain(main: usize, mut frame: u64, stack: u64) -> Option<[usize; 3]> {
    // The frame records are borrowed only during this inline callback. Reject
    // backwards/out-of-window links before following them; never retain them.
    let limit = stack.checked_add(64 * 1024)?;
    let mut floor = stack;
    let mut out = [0usize; 3];
    for slot in &mut out {
        if frame < floor || frame.checked_add(16)? > limit || !frame.is_multiple_of(16) {
            return None;
        }
        let lr = rd::<u64>(frame as usize + 8)?;
        let rel = (lr as usize).checked_sub(main)?;
        if rel >= 0x0400_0000 {
            return None;
        }
        *slot = rel;
        floor = frame.checked_add(16)?;
        frame = rd::<u64>(frame as usize)?;
    }
    Some(out)
}

unsafe fn copy_span(
    header: usize,
    begin_reg: u64,
    end_reg: u64,
) -> Option<([u64; MAX_TOKENS], usize)> {
    let begin = rd::<u64>(header)?;
    let end = rd::<u64>(header + 8)?;
    let cap = rd::<u64>(header + 16)?;
    if begin != begin_reg || end != end_reg || begin >= end || end > cap {
        return None;
    }
    if !begin.is_multiple_of(8) || !end.is_multiple_of(8) || !cap.is_multiple_of(8) {
        return None;
    }
    let count = ((end - begin) / 8) as usize;
    if count == 0 || count > MAX_TOKENS {
        return None;
    }
    let mut tokens = [0u64; MAX_TOKENS];
    for (i, slot) in tokens.iter_mut().take(count).enumerate() {
        *slot = rd::<u64>(begin as usize + i * 8)?;
    }
    Some((tokens, count))
}

unsafe fn place_last(begin: usize, count: usize, index: usize) {
    let Some(last) = count.checked_sub(1) else {
        return;
    };
    if index >= last {
        return;
    }
    let left = begin + index * 8;
    let right = begin + last * 8;
    let (Some(a), Some(b)) = (rd::<u64>(left), rd::<u64>(right)) else {
        return;
    };
    std::ptr::write_volatile(left as *mut u64, b);
    std::ptr::write_volatile(right as *mut u64, a);
}
