//! Choose one already-eligible full stage token for the local pop.
//!
//! The native pop returns the last element of one mode
//! vector. This module only decides which existing element should occupy that
//! position. It does not read native memory, write a vector header, or call
//! the original eligibility predicate.

use crate::{Config, Fighter, Stage, StagePool};

const FULL_TAG: u64 = 0x69;
const HASH40_MASK: u64 = 0x00ff_ffff_ffff;

/// Hash40(`ui_stage_<Stage::key>`), in `Stage::ALL` order.
const STAGE_HASH40: [u64; 11] = [
    0x154b5c4392,
    0x171954c00c,
    0x0c40bf5050,
    0x188ad4c5ee,
    0x18b3a39e1f,
    0x16c133cba8,
    0x17d35a2eab,
    0x1397e2df75,
    0x144e6d100c,
    0x10bc05aed3,
    0x159ba9e93e,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PopRoute {
    /// The qualified local constructor calls the native pop.
    LocalEligiblePop,
    /// Choice byte 5 looks up one fixed row and uses `0x3381340 -> 0x33800f0 -> 0x3271490`.
    Choice5Derived,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainReason {
    UnsupportedChoice5,
    ConstructionNotArmed,
    PreferredRulesDisabled,
    CounterpickDisabled,
    UnknownFighter,
    EmptyVector,
    NoFullKnownToken,
    NoConfiguredStageInPool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Selection {
    Retain(RetainReason),
    /// `index` is the live element that must be last before the native pop.
    /// The native end pointer is left for that pop to decrement.
    PositionLast {
        index: usize,
        stage: Stage,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct SelectQuery<'a> {
    pub route: PopRoute,
    /// True only for the armed post-result local construction.
    pub construction_armed: bool,
    /// True only when the constructor's preferred-rules enable byte is set.
    pub native_preferred_rules_enabled: bool,
    pub config: &'a Config,
    /// Loser from a qualified pending outcome. This module does not infer one.
    pub loser: Option<Fighter>,
    /// Live full-token elements, copied from the selected mode vector.
    pub tokens: &'a [u64],
}

pub fn hash40(stage: Stage) -> u64 {
    STAGE_HASH40[usize::from(stage as u8)]
}

pub fn route_for_choice_byte(choice: u8) -> PopRoute {
    if choice == 5 {
        PopRoute::Choice5Derived
    } else {
        PopRoute::LocalEligiblePop
    }
}

/// Mode 1 -> pool+0x20, mode 2 -> pool+0x38, every other mode -> pool+0x08.
pub fn mode_vector_offset(mode: u32) -> usize {
    match mode {
        1 => 0x20,
        2 => 0x38,
        _ => 0x08,
    }
}

pub fn stage_of_full_token(raw: u64) -> Option<Stage> {
    if raw >> 56 != FULL_TAG {
        return None;
    }
    let hash = raw & HASH40_MASK;
    Stage::ALL.into_iter().find(|stage| hash40(*stage) == hash)
}

pub fn eligible_pool(tokens: &[u64]) -> StagePool {
    let mut found = [Stage::Battlefield; 11];
    let mut count = 0;
    for &raw in tokens {
        if let Some(stage) = stage_of_full_token(raw) {
            if !found[..count].contains(&stage) {
                found[count] = stage;
                count += 1;
            }
        }
    }
    StagePool::from_stages(&found[..count])
}

/// Element count of a `begin`/`end` pair of 8-byte tokens.
pub fn live_token_count(begin: u64, end: u64) -> Option<u64> {
    if !begin.is_multiple_of(8) {
        return None;
    }
    let bytes = end.checked_sub(begin)?;
    if !bytes.is_multiple_of(8) {
        return None;
    }
    Some(bytes / 8)
}

pub fn span_matches(tokens: &[u64], begin: u64, end: u64) -> bool {
    live_token_count(begin, end) == Some(tokens.len() as u64)
}

pub fn select_eligible(query: SelectQuery<'_>) -> Selection {
    if let PopRoute::Choice5Derived = query.route {
        return Selection::Retain(RetainReason::UnsupportedChoice5);
    }
    if !query.construction_armed {
        return Selection::Retain(RetainReason::ConstructionNotArmed);
    }
    if !query.native_preferred_rules_enabled {
        return Selection::Retain(RetainReason::PreferredRulesDisabled);
    }
    if !query.config.enabled() {
        return Selection::Retain(RetainReason::CounterpickDisabled);
    }
    let Some(loser) = query.loser else {
        return Selection::Retain(RetainReason::UnknownFighter);
    };
    if query.tokens.is_empty() {
        return Selection::Retain(RetainReason::EmptyVector);
    }
    let pool = eligible_pool(query.tokens);
    if pool.is_empty() {
        return Selection::Retain(RetainReason::NoFullKnownToken);
    }
    let Some(stage) = query.config.choose(loser, pool) else {
        return Selection::Retain(RetainReason::NoConfiguredStageInPool);
    };
    let Some(index) = query
        .tokens
        .iter()
        .position(|raw| stage_of_full_token(*raw) == Some(stage))
    else {
        return Selection::Retain(RetainReason::NoConfiguredStageInPool);
    };
    Selection::PositionLast { index, stage }
}

/// Exchange `tokens[index]` with the last element. Length is unchanged.
/// An index already at the end is a successful no-op.
pub fn position_last(tokens: &mut [u64], index: usize) -> bool {
    let Some(last) = tokens.len().checked_sub(1) else {
        return false;
    };
    if index > last {
        return false;
    }
    tokens.swap(index, last);
    true
}
