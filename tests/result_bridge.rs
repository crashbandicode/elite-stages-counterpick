#![cfg(feature = "native-construction-alpha")]

use elite_stages_counterpick_core::construction::{
    CounterObservation, LocalConstruction, MatchStart, NativeParticipant, ParticipantRow, Route,
};
use elite_stages_counterpick_core::{
    hash40, Config, Fighter, PopRoute, RetainReason, Selection, Stage,
};

const BRAWLER_TOKEN: u64 = 0xc1004c13fdfdfb2b;
const MARIO_TOKEN: u64 = 0xc100010edaf3c863;

fn row(number: u8, token: u64, kind: u32, fighter: &str, wins: u8) -> ParticipantRow {
    ParticipantRow {
        identity: NativeParticipant {
            number,
            fighter_token: token,
            fighter_kind: kind,
            fighter: Fighter::from_key(fighter),
        },
        active: true,
        wins,
    }
}

/// O58 rows. The capture's participation byte was 0 on both in-match rows;
/// `active` is the caller's qualification, not that byte.
fn o58(wins: [u8; 2]) -> [ParticipantRow; 2] {
    [
        row(0, BRAWLER_TOKEN, 72, "mii_brawler", wins[0]),
        row(1, MARIO_TOKEN, 0, "mario", wins[1]),
    ]
}

fn start(ticket: u64, wins: [u8; 2]) -> MatchStart {
    MatchStart { ticket, online_1v1: true, rows: o58(wins) }
}

fn seen<'a>(ticket: u64, wins: [u8; 2], winners: &'a [u8]) -> CounterObservation<'a> {
    CounterObservation { ticket, online_1v1: true, rows: o58(wins), winner_numbers: winners }
}

fn config() -> Config {
    Config::parse(
        "schema=1\nenabled=true\nfighter.mii_brawler=end,battle_field\nfighter.mario=battle_field,end\n",
    )
    .unwrap()
}

fn full(stage: Stage) -> u64 {
    (0x69u64 << 56) | hash40(stage)
}

#[test]
fn o58_css_decline_drops_the_observed_one_step_delta() {
    let mut state = LocalConstruction::new();
    assert!(state.start_match(start(1, [0, 0])));
    assert!(state.observe_finish(&seen(1, [0, 1], &[1])));
    let css = seen(1, [0, 1], &[1]);
    assert_eq!(state.enter(Route::from_main_relative_lr(0x2368f9c), &css), None);
    assert_eq!(state.enter(Route::from_main_relative_lr(0x2316608), &css), None);

    let mut later = LocalConstruction::new();
    assert!(later.start_match(start(2, [0, 0])));
    assert!(later.observe_finish(&seen(2, [0, 1], &[1])));
    assert_eq!(
        later.enter(Route::from_main_relative_lr(0x23695a0), &seen(2, [0, 1], &[1])),
        None
    );
    assert_eq!(later.enter(Route::PostResult, &seen(2, [0, 1], &[1])), None);
}

#[test]
fn repeated_or_older_ticket_keeps_the_original_baseline() {
    let mut state = LocalConstruction::new();
    assert!(state.start_match(start(5, [0, 0])));
    assert!(!state.start_match(start(5, [0, 1])));
    assert!(!state.start_match(start(4, [0, 0])));
    assert!(state.observe_finish(&seen(5, [0, 1], &[1])));
    assert!(state.enter(Route::PostResult, &seen(5, [0, 1], &[1])).is_some());
}

#[test]
fn later_delta_two_refuses_the_earlier_delta_one() {
    let mut state = LocalConstruction::new();
    assert!(state.start_match(start(1, [0, 0])));
    assert!(state.observe_finish(&seen(1, [0, 1], &[1])));
    assert!(!state.observe_finish(&seen(1, [0, 2], &[1])));
    assert_eq!(state.enter(Route::PostResult, &seen(1, [0, 1], &[1])), None);
    assert_eq!(state.enter(Route::PostResult, &seen(1, [0, 2], &[1])), None);
}

#[test]
fn changed_participant_rejects_the_ticket() {
    let mut state = LocalConstruction::new();
    assert!(state.start_match(start(1, [0, 0])));
    let mut changed = o58([0, 1]);
    changed[1].identity.fighter_token ^= 1;
    let obs = CounterObservation {
        ticket: 1,
        online_1v1: true,
        rows: changed,
        winner_numbers: &[1],
    };
    assert!(!state.observe_finish(&obs));
    assert_eq!(state.enter(Route::PostResult, &seen(1, [0, 1], &[1])), None);
}

#[test]
fn post_result_proposal_uses_the_prior_loser_without_a_local_player_check() {
    let mut state = LocalConstruction::new();
    assert!(state.start_match(start(1, [0, 0])));
    let plus = seen(1, [0, 1], &[1]);
    assert!(state.observe_finish(&plus));
    let token = state
        .enter(Route::from_main_relative_lr(0x2316608), &plus)
        .unwrap();
    let parsed = config();
    let tokens = [full(Stage::Battlefield), full(Stage::FinalDestination)];
    assert_eq!(
        state.propose(token, &parsed, true, PopRoute::LocalEligiblePop, &tokens),
        Selection::PositionLast { index: 1, stage: Stage::FinalDestination }
    );
    assert_eq!(
        state.propose(token, &parsed, false, PopRoute::LocalEligiblePop, &tokens),
        Selection::Retain(RetainReason::PreferredRulesDisabled)
    );
    assert_eq!(
        state.propose(token, &parsed, true, PopRoute::Choice5Derived, &tokens),
        Selection::Retain(RetainReason::UnsupportedChoice5)
    );
    assert_eq!(
        state.propose(token, &parsed, true, PopRoute::LocalEligiblePop, &[full(Stage::YoshisStory)]),
        Selection::Retain(RetainReason::NoConfiguredStageInPool)
    );
    state.leave(token);
    assert_eq!(
        state.propose(token, &parsed, true, PopRoute::LocalEligiblePop, &tokens),
        Selection::Retain(RetainReason::ConstructionNotArmed)
    );
}

#[test]
fn unsupported_scope_list_and_departure_reject() {
    let mut departed = LocalConstruction::new();
    assert!(departed.start_match(start(1, [0, 0])));
    assert!(departed.observe_finish(&seen(1, [0, 1], &[1])));
    let mut unsupported_next = start(2, [0, 0]);
    unsupported_next.online_1v1 = false;
    assert!(!departed.start_match(unsupported_next));
    assert_eq!(departed.enter(Route::PostResult, &seen(1, [0, 1], &[1])), None);

    let mut state = LocalConstruction::new();
    let mut offline = start(1, [0, 0]);
    offline.online_1v1 = false;
    assert!(!state.start_match(offline));

    let mut inactive = start(1, [0, 0]);
    inactive.rows[0].active = false;
    assert!(!state.start_match(inactive));

    let mut same_token = start(1, [0, 0]);
    same_token.rows[1].identity.fighter_token = BRAWLER_TOKEN;
    assert!(!state.start_match(same_token));

    let mut high_number = start(1, [0, 0]);
    high_number.rows[0].identity.number = 8;
    assert!(!state.start_match(high_number));

    assert!(state.start_match(start(1, [0, 0])));
    assert!(!state.observe_finish(&seen(1, [0, 1], &[0, 1])));
    assert_eq!(state.enter(Route::PostResult, &seen(1, [0, 1], &[1])), None);

    let mut fresh = LocalConstruction::new();
    assert!(fresh.start_match(start(1, [0, 0])));
    assert!(fresh.observe_finish(&seen(1, [0, 1], &[1])));
    fresh.invalidate();
    assert_eq!(fresh.enter(Route::PostResult, &seen(1, [0, 1], &[1])), None);

    let mut listed = LocalConstruction::new();
    assert!(listed.start_match(start(1, [0, 0])));
    assert!(!listed.observe_finish(&seen(1, [0, 1], &[0])));
    assert_eq!(listed.enter(Route::PostResult, &seen(1, [0, 1], &[1])), None);
}
