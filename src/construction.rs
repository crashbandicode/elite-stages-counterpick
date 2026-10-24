//! Value-only bridge from one match-start snapshot to one local construction.
//!
//! The native adapter must read the live return address and pass it here.
//! Compiling this module does not prove that `0x2316608` is the return address
//! in the running module. No hook is installed.
//!
//! A win-list entry is only a cross-check against the row whose counter moved.
//! It is not freshness. Callback uniqueness is not proved: any later delta
//! other than wrapping +1 poisons the ticket, so an earlier +1 image cannot
//! be published after that.

use crate::{select_eligible, Config, Fighter, PopRoute, SelectQuery, Selection};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeParticipant {
    /// Native E+14 player number, never a HUD P1/P2 label.
    pub number: u8,
    pub fighter_token: u64,
    pub fighter_kind: u32,
    pub fighter: Option<Fighter>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParticipantRow {
    pub identity: NativeParticipant,
    /// Caller qualification that this row is one of the two active participants.
    /// The observed in-match E+a byte was 0; this flag is not that byte.
    pub active: bool,
    pub wins: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatchStart {
    /// Caller-supplied monotonic match ticket. Not a native generation.
    pub ticket: u64,
    pub online_1v1: bool,
    pub rows: [ParticipantRow; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CounterObservation<'a> {
    pub ticket: u64,
    pub online_1v1: bool,
    pub rows: [ParticipantRow; 2],
    /// Native winner-list numbers. Exactly one entry must name the advanced row.
    pub winner_numbers: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Route {
    PostResult,
    BeforeCharacterSelect,
    BeforeFreshSearch,
    Other,
}

impl Route {
    pub fn from_main_relative_lr(lr: usize) -> Self {
        match lr {
            0x2316608 => Self::PostResult,
            0x2368f9c => Self::BeforeCharacterSelect,
            0x23695a0 => Self::BeforeFreshSearch,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Construction(u64);

#[derive(Clone, Copy, Debug)]
struct Snapshot {
    ticket: u64,
    rows: [ParticipantRow; 2],
}

#[derive(Clone, Copy, Debug)]
struct Active {
    token: Construction,
    loser: NativeParticipant,
}

#[derive(Debug)]
pub struct LocalConstruction {
    last_ticket: Option<u64>,
    snapshot: Option<Snapshot>,
    pending: Option<NativeParticipant>,
    active: Option<Active>,
    poisoned: bool,
    consumed: bool,
    next_token: u64,
}

impl Default for LocalConstruction {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalConstruction {
    pub const fn new() -> Self {
        Self {
            last_ticket: None,
            snapshot: None,
            pending: None,
            active: None,
            poisoned: false,
            consumed: false,
            next_token: 0,
        }
    }

    /// Arm one baseline. An equal or older ticket does not move it.
    pub fn start_match(&mut self, start: MatchStart) -> bool {
        if self.last_ticket.is_some_and(|last| start.ticket <= last) {
            return false;
        }
        if !start.online_1v1 || !pair_ok(&start.rows) {
            // A newly entered unsupported context cannot retain the previous match's result.
            // Ignore stale start callbacks above so they cannot retire the current baseline.
            self.invalidate();
            return false;
        }
        self.last_ticket = Some(start.ticket);
        self.snapshot = Some(Snapshot {
            ticket: start.ticket,
            rows: start.rows,
        });
        self.pending = None;
        self.active = None;
        self.poisoned = false;
        self.consumed = false;
        true
    }

    /// Record a later counter observation. A non-+1 result poisons the ticket.
    pub fn observe_finish(&mut self, obs: &CounterObservation<'_>) -> bool {
        if self.active.is_some() {
            return false;
        }
        let Some(loser) = self.accept_delta(obs) else {
            return false;
        };
        self.pending = Some(loser);
        true
    }

    /// Post-result revalidates the snapshot against the recorded finish.
    /// Every other route drops the ticket. A constructor sample alone does not publish.
    pub fn enter(&mut self, route: Route, obs: &CounterObservation<'_>) -> Option<Construction> {
        if route != Route::PostResult || self.active.is_some() {
            self.invalidate();
            return None;
        }
        let loser = self.accept_delta(obs)?;
        if self.pending != Some(loser) {
            self.invalidate();
            return None;
        }
        let next = self.next_token.checked_add(1)?;
        self.next_token = next;
        self.pending = None;
        self.consumed = true;
        let token = Construction(next);
        self.active = Some(Active { token, loser });
        Some(token)
    }

    /// Loser's ranked stage among the supplied full tokens. No local-player test.
    pub fn propose(
        &self,
        token: Construction,
        config: &Config,
        native_preferred_rules_enabled: bool,
        pop: PopRoute,
        tokens: &[u64],
    ) -> Selection {
        let active = self.active.filter(|active| active.token == token);
        select_eligible(SelectQuery {
            route: pop,
            construction_armed: active.is_some(),
            native_preferred_rules_enabled,
            config,
            loser: active.and_then(|active| active.loser.fighter),
            tokens,
        })
    }

    pub fn leave(&mut self, token: Construction) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.token == token)
        {
            self.active = None;
        }
    }

    /// Scope loss, departure, or disconnect for the open ticket.
    pub fn invalidate(&mut self) {
        self.pending = None;
        self.active = None;
        self.poisoned = true;
    }

    fn accept_delta(&mut self, obs: &CounterObservation<'_>) -> Option<NativeParticipant> {
        let snapshot = self.snapshot?;
        if obs.ticket != snapshot.ticket || self.poisoned || self.consumed {
            return None;
        }
        if !obs.online_1v1 {
            self.invalidate();
            return None;
        }
        match one_win(&snapshot.rows, &obs.rows, obs.winner_numbers) {
            Some(loser) => Some(loser),
            None => {
                self.invalidate();
                None
            }
        }
    }
}

fn pair_ok(rows: &[ParticipantRow; 2]) -> bool {
    let [a, b] = rows;
    a.active
        && b.active
        && a.identity.number < 8
        && b.identity.number < 8
        && a.identity.number != b.identity.number
        && a.identity.fighter_token != b.identity.fighter_token
}

fn same_identity(a: NativeParticipant, b: NativeParticipant) -> bool {
    a.number == b.number
        && a.fighter_token == b.fighter_token
        && a.fighter_kind == b.fighter_kind
        && a.fighter == b.fighter
}

fn one_win(
    start: &[ParticipantRow; 2],
    now: &[ParticipantRow; 2],
    winner_numbers: &[u8],
) -> Option<NativeParticipant> {
    if !pair_ok(now) {
        return None;
    }
    let mut winner = None;
    let mut loser = None;
    for baseline in start {
        let current = now
            .iter()
            .find(|row| row.identity.number == baseline.identity.number)?;
        if !same_identity(baseline.identity, current.identity) {
            return None;
        }
        match current.wins.wrapping_sub(baseline.wins) {
            0 => {
                if loser.is_some() {
                    return None;
                }
                loser = Some(current.identity);
            }
            1 => {
                if winner.is_some() {
                    return None;
                }
                winner = Some(current.identity.number);
            }
            _ => return None,
        }
    }
    let winner = winner?;
    if winner_numbers.len() != 1 || winner_numbers[0] != winner {
        return None;
    }
    loser
}
