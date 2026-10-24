#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod config;
#[cfg(feature = "native-construction-alpha")]
pub mod construction;
mod eligible;
#[cfg(feature = "std")]
pub mod files;
mod native_kind;
mod roster;
#[cfg(feature = "std")]
pub mod storage;

pub use config::{Config, ConfigError, ConfigErrorKind};
pub use eligible::{
    eligible_pool, hash40, live_token_count, mode_vector_offset, position_last,
    route_for_choice_byte, select_eligible, span_matches, stage_of_full_token, PopRoute,
    RetainReason, SelectQuery, Selection,
};
pub use roster::FIGHTER_KEYS;

/// Stable configuration identity; deliberately not a game fighter-kind number.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Fighter(u8);

impl Fighter {
    pub fn from_key(key: &str) -> Option<Self> {
        FIGHTER_KEYS
            .iter()
            .position(|&name| name == key)
            .map(|i| Self(i as u8))
    }

    pub fn key(self) -> &'static str {
        FIGHTER_KEYS[usize::from(self.0)]
    }
}

/// Stable policy indices; these discriminants MUST NOT be passed to Smash.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Stage {
    Battlefield,
    SmallBattlefield,
    FinalDestination,
    YoshisStory,
    LylatCruise,
    PokemonStadium2,
    Smashville,
    KalosPokemonLeague,
    TownAndCity,
    NorthernCave,
    HollowBastion,
}

impl Stage {
    pub const ALL: [Self; 11] = [
        Self::Battlefield,
        Self::SmallBattlefield,
        Self::FinalDestination,
        Self::YoshisStory,
        Self::LylatCruise,
        Self::PokemonStadium2,
        Self::Smashville,
        Self::KalosPokemonLeague,
        Self::TownAndCity,
        Self::NorthernCave,
        Self::HollowBastion,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Battlefield => "battle_field",
            Self::SmallBattlefield => "battle_field_s",
            Self::FinalDestination => "end",
            Self::YoshisStory => "yoshi_cartboard",
            Self::LylatCruise => "fox_lylatcruise",
            Self::PokemonStadium2 => "poke_stadium2",
            Self::Smashville => "animal_village",
            Self::KalosPokemonLeague => "poke_kalos",
            Self::TownAndCity => "animal_city",
            Self::NorthernCave => "ff_cave",
            Self::HollowBastion => "trail_castle",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|stage| stage.key() == key)
    }
}

/// A snapshot of the ORIGINAL plugin's enabled pool. Empty is a safe fallback.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StagePool(u16);

impl StagePool {
    pub fn from_stages(stages: &[Stage]) -> Self {
        let mut pool = Self::default();
        for &stage in stages {
            pool.0 |= 1 << stage as u8;
        }
        pool
    }

    pub fn contains(self, stage: Stage) -> bool {
        self.0 & (1 << stage as u8) != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Participant {
    /// Adapter-owned continuity identity, independent of player-slot order.
    pub id: u64,
    /// Completed match's fighter. Unknown identities must stay None.
    pub fighter: Option<Fighter>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Match {
    /// Adapter-owned session generation, changed after disconnect/reconstruction.
    pub session: u64,
    /// Strictly increasing within the session, assigned by the verified adapter.
    pub epoch: u64,
    pub players: [Participant; 2],
    /// False for team/FFA/otherwise unsupported result semantics.
    pub supported_singles: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Winner(u64),
    TieOrNoContest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedRematch {
    pub session: u64,
    pub previous_epoch: u64,
    pub next_epoch: u64,
    pub participants: [u64; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RulesOwner {
    Local,
    Opponent,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProposalContext {
    pub session: u64,
    pub next_epoch: u64,
    pub participants: [u64; 2],
    pub supported_game: bool,
    pub quickplay_or_elite: bool,
    pub hazards_off: bool,
    pub original_mod_enabled: bool,
    pub rules_owner: RulesOwner,
    pub enabled_pool: StagePool,
    /// Candidates already permitted by the original selection operation. An
    /// unknown candidate set must be empty, never assumed to be all stages.
    pub original_eligible_pool: StagePool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fallback {
    NoAcceptedRematch,
    ContextChanged,
    Disabled,
    UnsupportedScope,
    NonLocalRules,
    EmptyPool,
    UnknownFighter,
    NoRankedEnabledStage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Decision {
    /// Caller must retain the original selection behavior without edits.
    Baseline(Fallback),
    /// One local proposal only; it is not a promise of the stage loaded online.
    Propose(Stage),
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    Idle,
    Playing(Match),
    Result {
        game: Match,
        loser: Participant,
    },
    Armed {
        next: AcceptedRematch,
        loser: Participant,
    },
}

/// All state belongs to one adapter session. No game pointers or persistent
/// rematch state; the adapter serializes callbacks on one owner thread.
#[derive(Debug)]
pub struct Counterpick {
    session: u64,
    last_started_epoch: Option<u64>,
    phase: Phase,
    config: Config,
}

impl Counterpick {
    pub fn new(session: u64, config: Config) -> Self {
        Self {
            session,
            last_started_epoch: None,
            phase: Phase::Idle,
            config,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Publish only a fully parsed configuration. Invalidate an old result or
    /// proposal; an in-progress match may still complete under the new config.
    pub fn replace_config(&mut self, config: Config) {
        self.config = config;
        if !matches!(self.phase, Phase::Playing(_)) {
            self.phase = Phase::Idle;
        }
    }

    /// Decline, cancel, disconnect, mode change and failed next-match entry.
    pub fn invalidate(&mut self) {
        self.phase = Phase::Idle;
    }

    pub fn begin_match(&mut self, game: Match) -> bool {
        if game.session != self.session
            || game.players[0].id == game.players[1].id
            || self
                .last_started_epoch
                .is_some_and(|last| game.epoch <= last)
        {
            return false;
        }
        self.last_started_epoch = Some(game.epoch);
        self.phase = Phase::Playing(game);
        true
    }

    pub fn complete_match(&mut self, session: u64, epoch: u64, outcome: Outcome) -> bool {
        let Phase::Playing(game) = self.phase else {
            return false;
        };
        if game.session != session || game.epoch != epoch {
            return false;
        }
        self.phase = Phase::Idle;
        if !self.config.enabled() || !game.supported_singles {
            return false;
        }
        let loser = match outcome {
            Outcome::Winner(id) if id == game.players[0].id => game.players[1],
            Outcome::Winner(id) if id == game.players[1].id => game.players[0],
            _ => return false,
        };
        self.phase = Phase::Result { game, loser };
        true
    }

    /// The native adapter must prove BOTH peers accepted. A local Yes button
    /// has no corresponding entry point and must never call this method.
    pub fn accept_rematch(&mut self, event: AcceptedRematch) -> bool {
        let Phase::Result { game, loser } = self.phase else {
            return false;
        };
        if event.session != game.session || event.previous_epoch != game.epoch {
            return false;
        }
        if event.next_epoch <= game.epoch
            || !same_participants(event.participants, [game.players[0].id, game.players[1].id])
        {
            self.phase = Phase::Idle;
            return false;
        }
        self.phase = Phase::Armed { next: event, loser };
        true
    }

    /// Call ONCE at the verified local stage-proposal boundary after rules
    /// ownership is known. Every matched attempt consumes the pending proposal,
    /// including fallback. A later begin_match starts the next lifecycle.
    pub fn take_proposal(&mut self, context: ProposalContext) -> Decision {
        let Phase::Armed { next, loser } = self.phase else {
            return Decision::Baseline(Fallback::NoAcceptedRematch);
        };
        self.phase = Phase::Idle;
        if next.session != context.session
            || next.next_epoch != context.next_epoch
            || !same_participants(next.participants, context.participants)
        {
            return Decision::Baseline(Fallback::ContextChanged);
        }
        if !self.config.enabled() || !context.original_mod_enabled {
            return Decision::Baseline(Fallback::Disabled);
        }
        if !context.supported_game || !context.quickplay_or_elite || !context.hazards_off {
            return Decision::Baseline(Fallback::UnsupportedScope);
        }
        if context.rules_owner != RulesOwner::Local {
            return Decision::Baseline(Fallback::NonLocalRules);
        }
        if context.enabled_pool.is_empty() {
            return Decision::Baseline(Fallback::EmptyPool);
        }
        let Some(fighter) = loser.fighter else {
            return Decision::Baseline(Fallback::UnknownFighter);
        };
        self.config
            .choose(
                fighter,
                context
                    .enabled_pool
                    .intersect(context.original_eligible_pool),
            )
            .map_or(
                Decision::Baseline(Fallback::NoRankedEnabledStage),
                Decision::Propose,
            )
    }
}

fn same_participants(a: [u64; 2], b: [u64; 2]) -> bool {
    a[0] != a[1] && (a == b || a == [b[1], b[0]])
}
