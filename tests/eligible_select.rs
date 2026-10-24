use elite_stages_counterpick_core::{
    hash40, live_token_count, mode_vector_offset, position_last, route_for_choice_byte,
    select_eligible, span_matches, stage_of_full_token, Config, Fighter, PopRoute, RetainReason,
    SelectQuery, Selection, Stage,
};

fn config(enabled: bool) -> Config {
    Config::parse(&format!(
        "schema=1\nenabled={enabled}\nfighter.mario=yoshi_cartboard,battle_field,battle_field_s\nfighter.little_mac=end,poke_stadium2\n"
    ))
    .unwrap()
}

fn fighter(key: &str) -> Fighter {
    Fighter::from_key(key).unwrap()
}

fn full(stage: Stage, cache: u16) -> u64 {
    (0x69u64 << 56) | (u64::from(cache) << 40) | hash40(stage)
}

fn decide(
    route: PopRoute,
    armed: bool,
    preferred: bool,
    enabled: bool,
    loser: Option<Fighter>,
    tokens: &[u64],
) -> Selection {
    let parsed = config(enabled);
    select_eligible(SelectQuery {
        route,
        construction_armed: armed,
        native_preferred_rules_enabled: preferred,
        config: &parsed,
        loser,
        tokens,
    })
}

fn armed_pop(loser: Fighter, tokens: &[u64]) -> Selection {
    decide(PopRoute::LocalEligiblePop, true, true, true, Some(loser), tokens)
}

#[test]
fn stage_hashes_roundtrip_through_the_cache_field() {
    for stage in Stage::ALL {
        let raw = full(stage, 0x1234);
        assert_eq!(stage_of_full_token(raw), Some(stage));
        assert_eq!(raw >> 56, 0x69);
    }
    assert_eq!(stage_of_full_token(hash40(Stage::Battlefield)), None);
    assert_eq!(stage_of_full_token(0x69ffff0000000000), None);
}

#[test]
fn mode_offsets_match_the_pop_csel_chain() {
    assert_eq!(mode_vector_offset(0), 0x08);
    assert_eq!(mode_vector_offset(1), 0x20);
    assert_eq!(mode_vector_offset(2), 0x38);
    assert_eq!(mode_vector_offset(5), 0x08);
}

#[test]
fn live_span_requires_an_aligned_eight_byte_range() {
    assert_eq!(live_token_count(0x1000, 0x1010), Some(2));
    assert_eq!(live_token_count(0x1000, 0x1000), Some(0));
    assert_eq!(live_token_count(0x1004, 0x100c), None);
    assert_eq!(live_token_count(0x1010, 0x1000), None);
    assert_eq!(live_token_count(0x1000, 0x1004), None);
    let tokens = [full(Stage::Battlefield, 1)];
    assert!(span_matches(&tokens, 0x2000, 0x2008));
    assert!(!span_matches(&tokens, 0x2000, 0x2010));
}

#[test]
fn only_choice_byte_five_is_the_derived_route() {
    assert_eq!(route_for_choice_byte(5), PopRoute::Choice5Derived);
    for choice in [0u8, 1, 2, 3, 4, 6, 255] {
        assert_eq!(route_for_choice_byte(choice), PopRoute::LocalEligiblePop);
    }
}

#[test]
fn choice5_retains_even_when_a_preferred_token_is_present() {
    let tokens = [full(Stage::YoshisStory, 7), full(Stage::Battlefield, 1)];
    assert_eq!(
        decide(
            PopRoute::Choice5Derived,
            false,
            true,
            true,
            Some(fighter("mario")),
            &tokens,
        ),
        Selection::Retain(RetainReason::UnsupportedChoice5)
    );
}

#[test]
fn unarmed_or_disabled_routes_do_not_select() {
    let tokens = [full(Stage::FinalDestination, 1), full(Stage::YoshisStory, 2)];
    let mario = fighter("mario");
    assert_eq!(
        decide(PopRoute::LocalEligiblePop, false, true, true, Some(mario), &tokens),
        Selection::Retain(RetainReason::ConstructionNotArmed)
    );
    assert_eq!(
        decide(PopRoute::LocalEligiblePop, true, false, true, Some(mario), &tokens),
        Selection::Retain(RetainReason::PreferredRulesDisabled)
    );
    assert_eq!(
        decide(PopRoute::LocalEligiblePop, true, true, false, Some(mario), &tokens),
        Selection::Retain(RetainReason::CounterpickDisabled)
    );
    assert_eq!(
        decide(PopRoute::LocalEligiblePop, true, true, true, None, &tokens),
        Selection::Retain(RetainReason::UnknownFighter)
    );
}

#[test]
fn empty_or_untagged_vectors_retain_the_native_pop() {
    let mario = fighter("mario");
    assert_eq!(armed_pop(mario, &[]), Selection::Retain(RetainReason::EmptyVector));
    assert_eq!(
        armed_pop(mario, &[0x69ffff0000000000]),
        Selection::Retain(RetainReason::NoFullKnownToken)
    );
    assert_eq!(
        armed_pop(mario, &[hash40(Stage::YoshisStory)]),
        Selection::Retain(RetainReason::NoFullKnownToken)
    );
}

#[test]
fn first_configured_full_token_moves_to_the_end_exactly() {
    let yoshi = full(Stage::YoshisStory, 0x0abc);
    let battlefield = full(Stage::Battlefield, 0x0003);
    let mut tokens = [battlefield, yoshi, 0x69ffff0000000000];
    assert_eq!(
        armed_pop(fighter("mario"), &tokens),
        Selection::PositionLast { index: 1, stage: Stage::YoshisStory }
    );
    assert!(position_last(&mut tokens, 1));
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0], battlefield);
    assert_eq!(tokens[1], 0x69ffff0000000000);
    assert_eq!(tokens[2], yoshi);
}

#[test]
fn absent_higher_rank_uses_the_next_present_full_token() {
    let battlefield = full(Stage::Battlefield, 9);
    let small = full(Stage::SmallBattlefield, 4);
    let mut tokens = [small, battlefield];
    assert_eq!(
        armed_pop(fighter("mario"), &tokens),
        Selection::PositionLast { index: 1, stage: Stage::Battlefield }
    );
    assert!(position_last(&mut tokens, 1));
    assert_eq!(tokens, [small, battlefield]);
}

#[test]
fn untagged_copy_of_the_preferred_stage_is_not_promoted() {
    let untagged = hash40(Stage::YoshisStory);
    let battlefield = full(Stage::Battlefield, 2);
    let mut tokens = [untagged, battlefield];
    assert_eq!(
        armed_pop(fighter("mario"), &tokens),
        Selection::PositionLast { index: 1, stage: Stage::Battlefield }
    );
    assert!(position_last(&mut tokens, 1));
    assert_eq!(tokens, [untagged, battlefield]);
}

#[test]
fn no_ranked_token_in_the_live_vector_retains_baseline() {
    let tokens = [full(Stage::TownAndCity, 1), full(Stage::HollowBastion, 2)];
    assert_eq!(
        armed_pop(fighter("little_mac"), &tokens),
        Selection::Retain(RetainReason::NoConfiguredStageInPool)
    );
}

#[test]
fn position_last_rejects_an_empty_or_out_of_range_index() {
    let mut tokens = [full(Stage::FinalDestination, 1), full(Stage::PokemonStadium2, 2)];
    let original = tokens;
    assert!(!position_last(&mut tokens, 2));
    assert_eq!(tokens, original);
    let mut empty = [];
    assert!(!position_last(&mut empty, 0));
}

#[test]
fn duplicate_full_tokens_keep_the_lowest_index_value() {
    let first = full(Stage::FinalDestination, 1);
    let second = full(Stage::FinalDestination, 2);
    let other = full(Stage::PokemonStadium2, 3);
    let mut tokens = [other, first, second];
    assert_eq!(
        armed_pop(fighter("little_mac"), &tokens),
        Selection::PositionLast { index: 1, stage: Stage::FinalDestination }
    );
    assert!(position_last(&mut tokens, 1));
    assert_eq!(tokens, [other, second, first]);
}
