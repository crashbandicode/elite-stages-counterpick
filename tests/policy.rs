use elite_stages_counterpick_core::*;

fn fighter(key: &str) -> Fighter {
    Fighter::from_key(key).unwrap()
}

fn config(enabled: bool) -> Config {
    Config::parse(&format!("schema=1\nenabled={enabled}\nfighter.mario=yoshi_cartboard,battle_field,battle_field_s\nfighter.little_mac=end,poke_stadium2\n")).unwrap()
}

fn game(epoch: u64) -> Match {
    Match {
        session: 7,
        epoch,
        supported_singles: true,
        players: [
            Participant {
                id: 100,
                fighter: Some(fighter("mario")),
            },
            Participant {
                id: 200,
                fighter: Some(fighter("little_mac")),
            },
        ],
    }
}

fn rematch() -> AcceptedRematch {
    AcceptedRematch {
        session: 7,
        previous_epoch: 10,
        next_epoch: 11,
        participants: [100, 200],
    }
}

fn context() -> ProposalContext {
    ProposalContext {
        session: 7,
        next_epoch: 11,
        participants: [100, 200],
        supported_game: true,
        quickplay_or_elite: true,
        hazards_off: true,
        original_mod_enabled: true,
        rules_owner: RulesOwner::Local,
        enabled_pool: StagePool::from_stages(&Stage::ALL),
        original_eligible_pool: StagePool::from_stages(&Stage::ALL),
    }
}

fn completed(winner: u64) -> Counterpick {
    let mut engine = Counterpick::new(7, config(true));
    assert!(engine.begin_match(game(10)));
    assert!(engine.complete_match(7, 10, Outcome::Winner(winner)));
    engine
}

fn armed(winner: u64) -> Counterpick {
    let mut engine = completed(winner);
    assert!(engine.accept_rematch(rematch()));
    engine
}

#[test]
fn selects_local_loser_profile() {
    assert_eq!(
        armed(200).take_proposal(context()),
        Decision::Propose(Stage::YoshisStory)
    );
}

#[test]
fn selects_remote_loser_profile_when_local_player_won() {
    assert_eq!(
        armed(100).take_proposal(context()),
        Decision::Propose(Stage::FinalDestination)
    );
}

#[test]
fn disabled_best_stage_uses_next_rank_without_mutating_pool() {
    let mut ctx = context();
    ctx.enabled_pool = StagePool::from_stages(&[Stage::Battlefield, Stage::FinalDestination]);
    let before = ctx.enabled_pool;
    assert_eq!(
        armed(200).take_proposal(ctx),
        Decision::Propose(Stage::Battlefield)
    );
    assert_eq!(ctx.enabled_pool, before);
}

#[test]
fn singleton_pool_selects_only_ranked_enabled_stage() {
    let mut ctx = context();
    ctx.enabled_pool = StagePool::from_stages(&[Stage::SmallBattlefield]);
    assert_eq!(
        armed(200).take_proposal(ctx),
        Decision::Propose(Stage::SmallBattlefield)
    );
}

#[test]
fn original_ineligible_stages_are_never_reenabled() {
    let mut ctx = context();
    ctx.original_eligible_pool = StagePool::from_stages(&[Stage::Battlefield]);
    assert_eq!(
        armed(200).take_proposal(ctx),
        Decision::Propose(Stage::Battlefield)
    );
    ctx.original_eligible_pool = StagePool::default();
    assert_eq!(
        armed(200).take_proposal(ctx),
        Decision::Baseline(Fallback::NoRankedEnabledStage)
    );
}

#[test]
fn unranked_pool_preserves_baseline() {
    let mut ctx = context();
    ctx.enabled_pool = StagePool::from_stages(&[Stage::NorthernCave, Stage::HollowBastion]);
    assert_eq!(
        armed(200).take_proposal(ctx),
        Decision::Baseline(Fallback::NoRankedEnabledStage)
    );
}

#[test]
fn empty_pool_does_not_invent_a_stage() {
    let mut ctx = context();
    ctx.enabled_pool = StagePool::default();
    assert_eq!(
        armed(200).take_proposal(ctx),
        Decision::Baseline(Fallback::EmptyPool)
    );
}

#[test]
fn opponent_and_unknown_rules_are_never_overridden() {
    for owner in [RulesOwner::Opponent, RulesOwner::Unknown] {
        let mut engine = armed(200);
        let mut ctx = context();
        ctx.rules_owner = owner;
        assert_eq!(
            engine.take_proposal(ctx),
            Decision::Baseline(Fallback::NonLocalRules)
        );
        assert_eq!(
            engine.take_proposal(context()),
            Decision::Baseline(Fallback::NoAcceptedRematch)
        );
    }
}

#[test]
fn result_and_local_yes_without_acceptance_cannot_propose() {
    let mut engine = completed(200);
    // Local Yes is deliberately not a core event: leaving Result untouched is
    // the only valid translation until the adapter proves mutual acceptance.
    assert_eq!(
        engine.take_proposal(context()),
        Decision::Baseline(Fallback::NoAcceptedRematch)
    );
    assert!(engine.accept_rematch(rematch()));
    assert_eq!(
        engine.take_proposal(context()),
        Decision::Propose(Stage::YoshisStory)
    );
}

#[test]
fn proposal_is_consumed_once_and_duplicates_do_not_rearm() {
    let mut engine = armed(200);
    assert!(!engine.accept_rematch(rematch()));
    assert_eq!(
        engine.take_proposal(context()),
        Decision::Propose(Stage::YoshisStory)
    );
    assert!(!engine.complete_match(7, 10, Outcome::Winner(200)));
    assert!(!engine.accept_rematch(rematch()));
    assert_eq!(
        engine.take_proposal(context()),
        Decision::Baseline(Fallback::NoAcceptedRematch)
    );
}

#[test]
fn swapped_slots_keep_stable_loser_identity() {
    let mut engine = completed(200);
    let mut event = rematch();
    event.participants.swap(0, 1);
    assert!(engine.accept_rematch(event));
    assert_eq!(
        engine.take_proposal(context()),
        Decision::Propose(Stage::YoshisStory)
    );
}

#[test]
fn replacement_opponent_cancels_rematch() {
    let mut engine = completed(200);
    let mut event = rematch();
    event.participants[1] = 300;
    assert!(!engine.accept_rematch(event));
    assert!(!engine.accept_rematch(rematch()));
    assert_eq!(
        engine.take_proposal(context()),
        Decision::Baseline(Fallback::NoAcceptedRematch)
    );
}

#[test]
fn stale_acceptance_does_not_erase_current_result() {
    let mut engine = completed(200);
    let mut event = rematch();
    event.previous_epoch = 9;
    assert!(!engine.accept_rematch(event));
    event = rematch();
    event.session = 6;
    assert!(!engine.accept_rematch(event));
    assert!(engine.accept_rematch(rematch()));
}

#[test]
fn invalid_next_epoch_and_duplicate_participants_are_rejected() {
    let mut event = rematch();
    event.next_epoch = 10;
    assert!(!completed(200).accept_rematch(event));
    event = rematch();
    event.participants = [100, 100];
    assert!(!completed(200).accept_rematch(event));
}

#[test]
fn unrelated_commit_clears_pending_without_proposing() {
    let mut engine = armed(200);
    let mut ctx = context();
    ctx.next_epoch = 12;
    assert_eq!(
        engine.take_proposal(ctx),
        Decision::Baseline(Fallback::ContextChanged)
    );
    assert_eq!(
        engine.take_proposal(context()),
        Decision::Baseline(Fallback::NoAcceptedRematch)
    );
}

#[test]
fn changed_participants_or_session_at_commit_cannot_propose() {
    let mut ctx = context();
    ctx.participants = [100, 300];
    assert_eq!(
        armed(200).take_proposal(ctx),
        Decision::Baseline(Fallback::ContextChanged)
    );
    ctx = context();
    ctx.session = 8;
    assert_eq!(
        armed(200).take_proposal(ctx),
        Decision::Baseline(Fallback::ContextChanged)
    );
}

#[test]
fn invalidation_covers_decline_cancel_disconnect_and_mode_exit() {
    for before_acceptance in [true, false] {
        let mut engine = if before_acceptance {
            completed(200)
        } else {
            armed(200)
        };
        engine.invalidate();
        assert!(!engine.accept_rematch(rematch()));
        assert!(!engine.begin_match(game(10)));
        assert_eq!(
            engine.take_proposal(context()),
            Decision::Baseline(Fallback::NoAcceptedRematch)
        );
    }
}

#[test]
fn disabled_toggle_never_retains_a_result() {
    let mut engine = Counterpick::new(7, config(false));
    assert!(engine.begin_match(game(10)));
    assert!(!engine.complete_match(7, 10, Outcome::Winner(200)));
    assert!(!engine.accept_rematch(rematch()));
    assert_eq!(
        engine.take_proposal(context()),
        Decision::Baseline(Fallback::NoAcceptedRematch)
    );
}

#[test]
fn original_mod_disable_preserves_baseline() {
    let mut ctx = context();
    ctx.original_mod_enabled = false;
    assert_eq!(
        armed(200).take_proposal(ctx),
        Decision::Baseline(Fallback::Disabled)
    );
}

#[test]
fn unsupported_version_offline_arena_or_hazards_on_preserve_baseline() {
    for field in 0..3 {
        let mut ctx = context();
        match field {
            0 => ctx.supported_game = false,
            1 => ctx.quickplay_or_elite = false,
            _ => ctx.hazards_off = false,
        }
        assert_eq!(
            armed(200).take_proposal(ctx),
            Decision::Baseline(Fallback::UnsupportedScope)
        );
    }
}

#[test]
fn tie_unknown_winner_and_unsupported_singles_cannot_arm() {
    for (supported, outcome) in [
        (true, Outcome::TieOrNoContest),
        (true, Outcome::Winner(300)),
        (false, Outcome::Winner(200)),
    ] {
        let mut engine = Counterpick::new(7, config(true));
        let mut game = game(10);
        game.supported_singles = supported;
        assert!(engine.begin_match(game));
        assert!(!engine.complete_match(7, 10, outcome));
        assert!(!engine.accept_rematch(rematch()));
    }
}

#[test]
fn unknown_fighter_uses_baseline() {
    let mut engine = Counterpick::new(7, config(true));
    let mut game = game(10);
    game.players[0].fighter = None;
    assert!(engine.begin_match(game));
    assert!(engine.complete_match(7, 10, Outcome::Winner(200)));
    assert!(engine.accept_rematch(rematch()));
    assert_eq!(
        engine.take_proposal(context()),
        Decision::Baseline(Fallback::UnknownFighter)
    );
}

#[test]
fn invalid_match_and_stale_result_do_not_replace_live_match() {
    let mut engine = Counterpick::new(7, config(true));
    assert!(engine.begin_match(game(10)));
    let mut invalid = game(11);
    invalid.players[1].id = 100;
    assert!(!engine.begin_match(invalid));
    invalid = game(11);
    invalid.session = 8;
    assert!(!engine.begin_match(invalid));
    assert!(!engine.begin_match(game(10)));
    assert!(!engine.complete_match(7, 9, Outcome::Winner(200)));
    assert!(engine.complete_match(7, 10, Outcome::Winner(200)));
}

#[test]
fn new_match_and_changed_fighter_do_not_reuse_previous_snapshot() {
    let mut engine = armed(200);
    let mut next = game(11);
    next.players[0].fighter = Some(fighter("little_mac"));
    next.players[1].fighter = Some(fighter("mario"));
    assert!(engine.begin_match(next));
    assert_eq!(
        engine.take_proposal(context()),
        Decision::Baseline(Fallback::NoAcceptedRematch)
    );
    assert!(engine.complete_match(7, 11, Outcome::Winner(200)));
    let mut event = rematch();
    event.previous_epoch = 11;
    event.next_epoch = 12;
    assert!(engine.accept_rematch(event));
    let mut ctx = context();
    ctx.next_epoch = 12;
    assert_eq!(
        engine.take_proposal(ctx),
        Decision::Propose(Stage::FinalDestination)
    );
}

#[test]
fn successful_config_reload_invalidates_pending_but_failed_parse_does_not() {
    let mut engine = armed(200);
    let before = engine.config().clone();
    assert!(Config::parse("schema=1\nenabled=true\nfighter.mario=typo").is_err());
    assert_eq!(engine.config(), &before);
    engine.replace_config(config(false));
    engine.replace_config(config(true));
    assert_eq!(
        engine.take_proposal(context()),
        Decision::Baseline(Fallback::NoAcceptedRematch)
    );
}

#[test]
fn config_reload_during_play_keeps_only_that_match() {
    let mut engine = Counterpick::new(7, config(false));
    assert!(engine.begin_match(game(10)));
    engine.replace_config(config(true));
    assert!(engine.complete_match(7, 10, Outcome::Winner(200)));
}

#[test]
fn parser_rejects_unknown_duplicate_and_malformed_records() {
    for (tail, kind) in [
        ("fighter.mariio=end", ConfigErrorKind::UnknownFighter),
        ("fighter.mario=unknown", ConfigErrorKind::UnknownStage),
        ("fighter.mario=end,end", ConfigErrorKind::DuplicateStage),
        ("fighter.mario=end,", ConfigErrorKind::UnknownStage),
        (
            "fighter.mario=end\nfighter.mario=battle_field",
            ConfigErrorKind::DuplicateKey,
        ),
        ("stages=end", ConfigErrorKind::UnknownKey),
        ("fighter.mario", ConfigErrorKind::Syntax),
        ("enabled=false", ConfigErrorKind::DuplicateKey),
        ("schema=1", ConfigErrorKind::DuplicateKey),
    ] {
        assert_eq!(
            Config::parse(&format!("schema=1\nenabled=true\n{tail}"))
                .unwrap_err()
                .kind,
            kind
        );
    }
    assert_eq!(
        Config::parse("enabled=true").unwrap_err().kind,
        ConfigErrorKind::MissingSchema
    );
    assert_eq!(
        Config::parse("schema=1").unwrap_err().kind,
        ConfigErrorKind::MissingEnabled
    );
    assert_eq!(
        Config::parse("schema=2\nenabled=true").unwrap_err().kind,
        ConfigErrorKind::UnsupportedSchema
    );
    assert_eq!(
        Config::parse("schema=1\nenabled=yes").unwrap_err().kind,
        ConfigErrorKind::InvalidBoolean
    );
    assert_eq!(
        Config::parse(&"#".repeat(32769)).unwrap_err().kind,
        ConfigErrorKind::TooLarge
    );
}

#[test]
fn parser_reports_line_and_preserves_empty_opt_out() {
    let config =
        Config::parse("# comment\r\n schema = 1\r\nenabled=false\r\nfighter.mario=\r\n").unwrap();
    assert_eq!(config.ranking(fighter("mario")), Some(&[][..]));
    assert_eq!(
        config.choose(fighter("mario"), StagePool::from_stages(&Stage::ALL)),
        None
    );
    assert_eq!(
        Config::parse("schema=1\nenabled=true\nfighter.mario=typo")
            .unwrap_err()
            .line,
        3
    );
}

#[test]
fn missing_profile_and_explicit_empty_profile_preserve_baseline() {
    for record in ["", "fighter.mario=\n"] {
        let config = Config::parse(&format!("schema=1\nenabled=true\n{record}")).unwrap();
        let mut engine = Counterpick::new(7, config);
        assert!(engine.begin_match(game(10)));
        assert!(engine.complete_match(7, 10, Outcome::Winner(200)));
        assert!(engine.accept_rematch(rematch()));
        assert_eq!(
            engine.take_proposal(context()),
            Decision::Baseline(Fallback::NoRankedEnabledStage)
        );
    }
}

#[test]
fn all_stage_and_fighter_keys_round_trip_without_native_id_assumptions() {
    assert_eq!(Stage::ALL.len(), 11);
    for stage in Stage::ALL {
        assert_eq!(Stage::from_key(stage.key()), Some(stage));
    }
    for &key in FIGHTER_KEYS {
        assert_eq!(fighter(key).key(), key);
    }
    assert_eq!(Stage::from_key("11"), None);
    assert_eq!(Fighter::from_key("0"), None);
}

#[test]
fn shipped_defaults_parse_and_serialize_without_loss() {
    let defaults = Config::parse(include_str!("../defaults/community-v1.cfg")).unwrap();
    assert!(!defaults.enabled());
    assert_eq!(defaults.profile_count(), 89);
    assert_eq!(
        defaults.choose(fighter("mario"), StagePool::from_stages(&Stage::ALL)),
        Some(Stage::YoshisStory)
    );
    assert_eq!(Config::parse(&defaults.serialize()).unwrap(), defaults);
    assert_eq!(defaults.ranking(fighter("pokemon_trainer")), None);
    assert_eq!(defaults.ranking(fighter("pyra_and_mythra")), None);
    assert_eq!(
        defaults.ranking(fighter("richter")),
        Some(
            &[
                Stage::SmallBattlefield,
                Stage::LylatCruise,
                Stage::TownAndCity
            ][..]
        )
    );
}

#[test]
fn every_pool_subset_obeys_enabled_membership_and_rank_precedence() {
    let config = config(true);
    let expected_order = [
        Stage::YoshisStory,
        Stage::Battlefield,
        Stage::SmallBattlefield,
    ];
    for bits in 0_u16..(1 << 11) {
        let stages: Vec<_> = Stage::ALL
            .into_iter()
            .enumerate()
            .filter_map(|(index, stage)| ((bits & (1 << index)) != 0).then_some(stage))
            .collect();
        let pool = StagePool::from_stages(&stages);
        let selected = config.choose(fighter("mario"), pool);
        let expected = expected_order
            .iter()
            .copied()
            .find(|stage| stages.contains(stage));
        assert_eq!(selected, expected, "pool bits {bits}");
        if let Some(stage) = selected {
            assert!(pool.contains(stage));
        }
    }
}
