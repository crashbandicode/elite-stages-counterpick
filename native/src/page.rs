//! Deluxe own-page rows for the stage pool and the counterpick toggle.
//!
//! The host draws these rows. This module does not call ImGui, touch the
//! filesystem, or install a hook. A accepted edit updates the draft and asks
//! the caller to queue the existing save path.

use elite_stages_counterpick_core::{Config, Stage};
#[cfg(test)]
use elite_stages_counterpick_core::Fighter;

pub const ABI_VERSION: u32 = 4;
pub const FLAG_OWN_PAGE: u32 = 1;
pub const FLAG_NO_PERSIST: u32 = 1 << 0;
pub const FLAG_READ_ONLY: u32 = 1 << 1;
pub const FLAG_HEADER: u32 = 1 << 2;
pub const CHOICE_UNAVAILABLE: u32 = u32::MAX;
pub const REQUEST_APPLIED: u32 = 0;
pub const REQUEST_REJECTED: u32 = 2;
pub const REQUEST_UNAVAILABLE: u32 = 3;
pub const MAX_OPTIONS: usize = 64;
#[cfg(test)]
pub const MAX_CHOICES: usize = 16;
#[cfg(test)]
pub const MAX_LABEL_LEN: usize = 24;

pub const STAGE_ROW: usize = 2;
pub const COUNTER_ROW: usize = 14;
pub const NEEDS_POOL_ROW: usize = 15;
pub const POOL_HOOK_ROW: usize = 16;
pub const RESULT_HOOK_ROW: usize = 17;
pub const SAVE_ROW: usize = 18;
pub const CONTROL_COUNT: usize = SAVE_ROW + 1;

const _: () = assert!(CONTROL_COUNT <= MAX_OPTIONS);
const _: () = assert!(STAGE_ROW + 11 == 13);

pub const OFF_ON: [&str; 2] = ["Off", "On"];
pub const NEED_CHOICES: [&str; 2] = ["No", "Yes"];
pub const HOOK_CHOICES: [&str; 2] = ["Ready", "Unavailable"];
pub const SAVE_CHOICES: [&str; 4] = ["Ready", "Saving", "Not saved", "No file"];
pub const STAGE_LABELS: [&str; 11] = [
    "Battlefield",
    "Small Battlefield",
    "Final Destination",
    "Yoshi's Story",
    "Lylat Cruise",
    "Pokemon Stadium 2",
    "Smashville",
    "Kalos Pokemon League",
    "Town and City",
    "Northern Cave",
    "Hollow Bastion",
];
const STAGE_IDS: [&str; 11] = [
    Stage::Battlefield.key(),
    Stage::SmallBattlefield.key(),
    Stage::FinalDestination.key(),
    Stage::YoshisStory.key(),
    Stage::LylatCruise.key(),
    Stage::PokemonStadium2.key(),
    Stage::Smashville.key(),
    Stage::KalosPokemonLeague.key(),
    Stage::TownAndCity.key(),
    Stage::NorthernCave.key(),
    Stage::HollowBastion.key(),
];

const OFF_ON_TONES: [u8; 2] = [0, 1];
const NEED_TONES: [u8; 2] = [0, 2];
const HOOK_TONES: [u8; 2] = [1, 3];
const SAVE_TONES: [u8; 4] = [1, 2, 3, 3];
const NO_CHOICES: [&str; 0] = [];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowKind {
    Header,
    Value,
    Status,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OptionMeta {
    pub id: &'static str,
    pub label: &'static str,
    pub choices: &'static [&'static str],
    pub tones: Option<&'static [u8]>,
    pub kind: RowKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluginMeta {
    pub id: &'static str,
    pub label: &'static str,
    pub own_page: bool,
    pub config_path: Option<&'static str>,
    pub options: &'static [OptionMeta],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageId {
    Controls,
}

impl PageId {
    pub const ALL: [PageId; 1] = [PageId::Controls];

    pub const fn meta(self) -> &'static PluginMeta {
        match self {
            PageId::Controls => &CONTROLS,
        }
    }
}

const fn row(
    id: &'static str,
    label: &'static str,
    choices: &'static [&'static str],
    tones: Option<&'static [u8]>,
    kind: RowKind,
) -> OptionMeta {
    OptionMeta {
        id,
        label,
        choices,
        tones,
        kind,
    }
}

const fn control_rows() -> [OptionMeta; CONTROL_COUNT] {
    let mut rows = [row(
        "pool_header",
        "Stage pool",
        &NO_CHOICES,
        None,
        RowKind::Header,
    ); CONTROL_COUNT];
    rows[1] = row(
        "pool",
        "Legal stages",
        &OFF_ON,
        Some(&OFF_ON_TONES),
        RowKind::Value,
    );
    let mut index = 0;
    while index < STAGE_IDS.len() {
        rows[STAGE_ROW + index] = row(
            STAGE_IDS[index],
            STAGE_LABELS[index],
            &OFF_ON,
            Some(&OFF_ON_TONES),
            RowKind::Value,
        );
        index += 1;
    }
    rows[13] = row(
        "cp_header",
        "Counterpick",
        &NO_CHOICES,
        None,
        RowKind::Header,
    );
    rows[COUNTER_ROW] = row(
        "enabled",
        "Counterpick",
        &OFF_ON,
        Some(&OFF_ON_TONES),
        RowKind::Value,
    );
    rows[NEEDS_POOL_ROW] = row(
        "needs_pool",
        "Needs pool",
        &NEED_CHOICES,
        Some(&NEED_TONES),
        RowKind::Status,
    );
    rows[POOL_HOOK_ROW] = row(
        "pool_hook",
        "Pool hook",
        &HOOK_CHOICES,
        Some(&HOOK_TONES),
        RowKind::Status,
    );
    rows[RESULT_HOOK_ROW] = row(
        "result_hook",
        "Result hook",
        &HOOK_CHOICES,
        Some(&HOOK_TONES),
        RowKind::Status,
    );
    rows[SAVE_ROW] = row(
        "save_state",
        "Save",
        &SAVE_CHOICES,
        Some(&SAVE_TONES),
        RowKind::Status,
    );
    rows
}

static CONTROL_ROWS: [OptionMeta; CONTROL_COUNT] = control_rows();

pub static CONTROLS: PluginMeta = PluginMeta {
    id: "stages_counterpick",
    label: "Stages + Counterpick",
    own_page: true,
    config_path: None,
    options: &CONTROL_ROWS,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PoolDraft {
    pub enabled: bool,
    pub selected: [bool; 11],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuDraft {
    pub pool: Option<PoolDraft>,
    pub config: Option<Config>,
    pub busy: bool,
    pub failed: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Hooks {
    pub pool: bool,
    pub result: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Save {
    Pool(PoolDraft),
    Config(Config),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Decision {
    Applied,
    Pending(Save),
    Rejected,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Answer {
    Choice(usize),
    Unavailable,
}

pub fn option_flags(kind: RowKind) -> u32 {
    match kind {
        RowKind::Header => FLAG_HEADER,
        RowKind::Value => FLAG_NO_PERSIST,
        RowKind::Status => FLAG_NO_PERSIST | FLAG_READ_ONLY,
    }
}

pub fn code(decision: &Decision) -> u32 {
    match decision {
        Decision::Applied => REQUEST_APPLIED,
        // The visible draft is already applied. Save status is our separate row;
        // host Pending would retain a stale requested value after a failed save.
        Decision::Pending(_) => REQUEST_APPLIED,
        Decision::Rejected => REQUEST_REJECTED,
        Decision::Unavailable => REQUEST_UNAVAILABLE,
    }
}

pub fn current(page: PageId, draft: &MenuDraft, hooks: Hooks, option: usize) -> Answer {
    let meta = page.meta();
    let Some(spec) = meta.options.get(option) else {
        return Answer::Unavailable;
    };
    if spec.kind == RowKind::Header {
        return Answer::Unavailable;
    }
    match page {
        PageId::Controls => control_current(draft, hooks, option),
    }
}

pub fn edit(
    draft: &mut MenuDraft,
    page: PageId,
    option: usize,
    choice: usize,
    accept: impl FnOnce(&Save) -> bool,
) -> Decision {
    let snapshot = draft.clone();
    let decision = apply(draft, page, option, choice);
    if let Decision::Pending(save) = &decision {
        if !accept(save) {
            *draft = snapshot;
            return Decision::Rejected;
        }
    }
    decision
}

pub fn finish_save(draft: &mut MenuDraft) {
    draft.busy = false;
    draft.failed = false;
}

pub fn fail_pool(draft: &mut MenuDraft, published: Option<PoolDraft>) {
    draft.pool = published;
    draft.busy = false;
    draft.failed = true;
}

pub fn fail_config(draft: &mut MenuDraft, published: Option<Config>) {
    draft.config = published;
    draft.busy = false;
    draft.failed = true;
}

fn control_current(draft: &MenuDraft, hooks: Hooks, option: usize) -> Answer {
    match option {
        1 => bool_answer(draft.pool.as_ref().map(|pool| pool.enabled)),
        index if (STAGE_ROW..STAGE_ROW + 11).contains(&index) => bool_answer(
            draft
                .pool
                .as_ref()
                .map(|pool| pool.selected[index - STAGE_ROW]),
        ),
        COUNTER_ROW => bool_answer(draft.config.as_ref().map(Config::enabled)),
        NEEDS_POOL_ROW => match (draft.pool.as_ref(), draft.config.as_ref()) {
            (Some(pool), Some(config)) => {
                Answer::Choice(usize::from(config.enabled() && !pool.enabled))
            }
            _ => Answer::Unavailable,
        },
        POOL_HOOK_ROW => Answer::Choice(usize::from(!hooks.pool)),
        RESULT_HOOK_ROW => Answer::Choice(usize::from(!hooks.result)),
        SAVE_ROW => Answer::Choice(save_choice(draft)),
        _ => Answer::Unavailable,
    }
}

fn bool_answer(value: Option<bool>) -> Answer {
    value.map_or(Answer::Unavailable, |on| Answer::Choice(usize::from(on)))
}

fn save_choice(draft: &MenuDraft) -> usize {
    if draft.busy {
        1
    } else if draft.failed {
        2
    } else if draft.pool.is_none() && draft.config.is_none() {
        3
    } else {
        0
    }
}

fn apply(draft: &mut MenuDraft, page: PageId, option: usize, choice: usize) -> Decision {
    if draft.busy {
        return Decision::Rejected;
    }
    let Some(spec) = page.meta().options.get(option) else {
        return Decision::Rejected;
    };
    if spec.kind != RowKind::Value || choice >= spec.choices.len() {
        return Decision::Rejected;
    }
    match page {
        PageId::Controls => apply_control(draft, option, choice),
    }
}

fn apply_control(draft: &mut MenuDraft, option: usize, choice: usize) -> Decision {
    if option == COUNTER_ROW {
        return apply_enabled(draft, choice);
    }
    let Some(mut pool) = draft.pool else {
        return Decision::Unavailable;
    };
    let on = choice == 1;
    if option == 1 {
        if pool.enabled == on {
            return Decision::Applied;
        }
        pool.enabled = on;
    } else if (STAGE_ROW..STAGE_ROW + 11).contains(&option) {
        let index = option - STAGE_ROW;
        if pool.selected[index] == on {
            return Decision::Applied;
        }
        pool.selected[index] = on;
        if !pool.selected.iter().any(|stage| *stage) {
            return Decision::Rejected;
        }
    } else {
        return Decision::Rejected;
    }
    draft.pool = Some(pool);
    draft.busy = true;
    draft.failed = false;
    Decision::Pending(Save::Pool(pool))
}

fn apply_enabled(draft: &mut MenuDraft, choice: usize) -> Decision {
    let Some(config) = draft.config.clone() else {
        return Decision::Unavailable;
    };
    let on = choice == 1;
    if config.enabled() == on {
        return Decision::Applied;
    }
    let mut next = config;
    next.set_enabled(on);
    draft.config = Some(next.clone());
    draft.busy = true;
    draft.failed = false;
    Decision::Pending(Save::Config(next))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(pool: Option<PoolDraft>, config: Option<Config>) -> MenuDraft {
        MenuDraft {
            pool,
            config,
            busy: false,
            failed: false,
        }
    }

    fn all_stages() -> PoolDraft {
        PoolDraft {
            enabled: true,
            selected: [true; 11],
        }
    }

    fn hooks() -> Hooks {
        Hooks {
            pool: true,
            result: true,
        }
    }

    fn id_ok(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 32
            && id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    }

    fn label_ok(label: &str) -> bool {
        !label.is_empty() && label.len() <= MAX_LABEL_LEN && !label.chars().any(char::is_control)
    }

    #[test]
    fn descriptors_fit_the_deluxe_own_page_registry() {
        assert_eq!(CHOICE_UNAVAILABLE, u32::MAX);
        let mut seen_plugins = Vec::new();
        for page in PageId::ALL {
            let meta = page.meta();
            assert!(meta.own_page);
            assert_eq!(meta.config_path, None);
            assert!(id_ok(meta.id));
            assert!(label_ok(meta.label));
            assert!(!seen_plugins.contains(&meta.id));
            seen_plugins.push(meta.id);
            assert!(!meta.options.is_empty() && meta.options.len() <= MAX_OPTIONS);
            let mut seen_ids = Vec::new();
            for option in meta.options {
                assert!(id_ok(option.id), "{}", option.id);
                assert!(label_ok(option.label), "{}", option.label);
                assert!(!seen_ids.contains(&option.id));
                seen_ids.push(option.id);
                assert!(option.choices.len() <= MAX_CHOICES);
                assert!(option.choices.iter().all(|choice| label_ok(choice)));
                assert!(!option.label.contains('_'), "{}", option.label);
                assert!(option.choices.iter().all(|choice| !choice.contains('_')));
                if let Some(tones) = option.tones {
                    assert_eq!(tones.len(), option.choices.len());
                    assert!(tones.iter().all(|tone| *tone <= 3));
                }
                match option.kind {
                    RowKind::Header => {
                        assert!(option.choices.is_empty());
                        assert_eq!(option_flags(option.kind), FLAG_HEADER);
                    }
                    RowKind::Value => {
                        assert!(option.choices.len() > 1);
                        assert_eq!(option_flags(option.kind), FLAG_NO_PERSIST);
                    }
                    RowKind::Status => {
                        assert!(option.choices.len() > 1);
                        assert_eq!(option_flags(option.kind), FLAG_NO_PERSIST | FLAG_READ_ONLY);
                    }
                }
            }
        }
        assert_eq!(seen_plugins, ["stages_counterpick"]);
        assert_eq!(CONTROLS.options.len(), CONTROL_COUNT);
        assert_eq!(CONTROLS.options[1].label, "Legal stages");
        assert_eq!(CONTROLS.options[COUNTER_ROW].label, "Counterpick");
        for (index, stage) in Stage::ALL.iter().enumerate() {
            assert_eq!(CONTROLS.options[STAGE_ROW + index].id, stage.key());
            assert_eq!(
                CONTROLS.options[STAGE_ROW + index].label,
                STAGE_LABELS[index]
            );
        }
        assert_eq!(FLAG_OWN_PAGE, 1);
        assert_eq!(ABI_VERSION, 4);
    }

    #[test]
    fn stage_edits_keep_one_stage_and_queue_only_the_pool() {
        let mut draft = ready(Some(all_stages()), Some(Config::default()));
        let config = draft.config.clone();
        let decision = edit(&mut draft, PageId::Controls, STAGE_ROW + 3, 0, |_| true);
        match decision {
            Decision::Pending(Save::Pool(pool)) => {
                assert!(!pool.selected[3]);
                assert_eq!(pool.selected.iter().filter(|on| **on).count(), 10);
                assert!(pool.enabled);
            }
            other => panic!("stage edit queues the pool, got {other:?}"),
        }
        assert!(draft.busy);
        assert_eq!(draft.config, config);
        assert_eq!(
            current(PageId::Controls, &draft, hooks(), STAGE_ROW + 3),
            Answer::Choice(0)
        );
        finish_save(&mut draft);

        let mut last = ready(Some(all_stages()), Some(Config::default()));
        for index in 0..10 {
            let decision = edit(&mut last, PageId::Controls, STAGE_ROW + index, 0, |_| true);
            assert!(
                matches!(decision, Decision::Pending(Save::Pool(_))),
                "stage {index}"
            );
            finish_save(&mut last);
        }
        let rejected = edit(&mut last, PageId::Controls, STAGE_ROW + 10, 0, |_| true);
        assert_eq!(rejected, Decision::Rejected);
        assert!(last.pool.unwrap().selected[10]);
        assert!(!last.busy);
        assert_eq!(last.config, config);
    }

    #[test]
    fn disabling_the_pool_does_not_turn_counterpick_off() {
        let mut config = Config::default();
        config.set_enabled(true);
        let mut pool = all_stages();
        pool.enabled = true;
        let mut draft = ready(Some(pool), Some(config));
        let decision = edit(&mut draft, PageId::Controls, 1, 0, |_| true);
        match decision {
            Decision::Pending(Save::Pool(saved)) => {
                assert!(!saved.enabled);
                assert_eq!(saved.selected, [true; 11]);
            }
            other => panic!("pool toggle pending, got {other:?}"),
        }
        assert!(draft.config.as_ref().unwrap().enabled());
        assert_eq!(
            current(PageId::Controls, &draft, hooks(), NEEDS_POOL_ROW),
            Answer::Choice(1)
        );
        finish_save(&mut draft);
        let same = edit(&mut draft, PageId::Controls, 1, 0, |_| true);
        assert_eq!(same, Decision::Applied);
        assert!(!draft.busy);
    }

    #[test]
    fn counterpick_toggle_queues_only_the_config_and_can_precede_the_pool() {
        let mut draft = ready(
            Some(PoolDraft {
                enabled: false,
                selected: [true; 11],
            }),
            Some(Config::default()),
        );
        let decision = edit(&mut draft, PageId::Controls, COUNTER_ROW, 1, |_| true);
        match &decision {
            Decision::Pending(Save::Config(saved)) => {
                assert!(saved.enabled());
                assert_eq!(code(&decision), REQUEST_APPLIED);
            }
            other => panic!("counterpick toggle pending, got {other:?}"),
        }
        assert!(!draft.pool.unwrap().enabled);
        assert_eq!(code(&Decision::Rejected), REQUEST_REJECTED);
        assert_eq!(code(&Decision::Unavailable), REQUEST_UNAVAILABLE);
    }

    #[test]
    fn counterpick_toggle_keeps_the_file_ranking() {
        let mario = Fighter::from_key("mario").unwrap();
        let order = [
            Stage::Smashville,
            Stage::FinalDestination,
            Stage::Battlefield,
        ];
        let mut config = Config::default();
        config.set_ranking(mario, &order).unwrap();
        let mut draft = ready(Some(all_stages()), Some(config));
        let decision = edit(&mut draft, PageId::Controls, COUNTER_ROW, 1, |_| true);
        match decision {
            Decision::Pending(Save::Config(saved)) => {
                assert!(saved.enabled());
                assert_eq!(saved.ranking(mario), Some(&order[..]));
                assert_eq!(
                    saved.serialize(),
                    "schema=1\nenabled=true\nfighter.mario=animal_village,end,battle_field\n"
                );
            }
            other => panic!("counterpick toggle pending, got {other:?}"),
        }
        let pool = draft.pool;
        finish_save(&mut draft);
        assert_eq!(draft.pool, pool);
        let stage = edit(&mut draft, PageId::Controls, STAGE_ROW, 0, |_| true);
        assert!(matches!(stage, Decision::Pending(Save::Pool(_))));
        assert_eq!(
            draft.config.as_ref().unwrap().ranking(mario),
            Some(&order[..])
        );
    }

    #[test]
    fn rejected_and_unloaded_rows_do_not_publish_a_change() {
        let mut draft = ready(Some(all_stages()), Some(Config::default()));
        let before = draft.clone();
        assert_eq!(
            edit(&mut draft, PageId::Controls, 0, 0, |_| true),
            Decision::Rejected
        );
        assert_eq!(
            edit(&mut draft, PageId::Controls, STAGE_ROW, 2, |_| true),
            Decision::Rejected
        );
        assert_eq!(
            edit(&mut draft, PageId::Controls, CONTROL_COUNT, 1, |_| true),
            Decision::Rejected
        );
        assert_eq!(
            edit(&mut draft, PageId::Controls, NEEDS_POOL_ROW, 1, |_| true),
            Decision::Rejected
        );
        assert_eq!(draft, before);

        draft.busy = true;
        let busy = edit(&mut draft, PageId::Controls, 1, 0, |_| true);
        assert_eq!(busy, Decision::Rejected);
        assert!(draft.pool.unwrap().enabled);
        draft.busy = false;

        let mut unloaded = ready(None, None);
        assert_eq!(
            edit(&mut unloaded, PageId::Controls, 1, 1, |_| true),
            Decision::Unavailable
        );
        assert_eq!(
            edit(&mut unloaded, PageId::Controls, COUNTER_ROW, 1, |_| true),
            Decision::Unavailable
        );
        assert_eq!(
            current(PageId::Controls, &unloaded, hooks(), SAVE_ROW),
            Answer::Choice(3)
        );
        assert_eq!(unloaded, ready(None, None));
    }

    #[test]
    fn a_refused_queue_restores_the_row_and_a_failed_save_reverts_one_side() {
        let mut draft = ready(Some(all_stages()), Some(Config::default()));
        let before = draft.clone();
        let decision = edit(&mut draft, PageId::Controls, STAGE_ROW, 0, |_| false);
        assert_eq!(decision, Decision::Rejected);
        assert_eq!(draft, before);

        let accepted = edit(&mut draft, PageId::Controls, STAGE_ROW, 0, |_| true);
        assert!(matches!(accepted, Decision::Pending(_)));
        let published = Some(all_stages());
        fail_pool(&mut draft, published);
        assert_eq!(draft.pool, published);
        assert!(draft.failed);
        assert!(!draft.busy);
        assert_eq!(
            current(PageId::Controls, &draft, hooks(), SAVE_ROW),
            Answer::Choice(2)
        );
        assert_eq!(
            current(PageId::Controls, &draft, hooks(), STAGE_ROW),
            Answer::Choice(1)
        );

        let mut config = Config::default();
        config.set_enabled(true);
        draft.config = Some(config);
        draft.failed = false;
        let toggle = edit(&mut draft, PageId::Controls, COUNTER_ROW, 0, |_| true);
        assert!(matches!(toggle, Decision::Pending(Save::Config(_))));
        let previous = Some(Config::default());
        let pool = draft.pool;
        fail_config(&mut draft, previous.clone());
        assert_eq!(draft.config, previous);
        assert_eq!(draft.pool, pool);
        finish_save(&mut draft);
        assert_eq!(
            current(PageId::Controls, &draft, Hooks::default(), POOL_HOOK_ROW),
            Answer::Choice(1)
        );
        assert_eq!(
            current(
                PageId::Controls,
                &draft,
                Hooks {
                    pool: true,
                    result: false
                },
                RESULT_HOOK_ROW
            ),
            Answer::Choice(1)
        );
    }
}
