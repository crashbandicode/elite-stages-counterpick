//! Configuration continuation across two separately launched processes. Pending
//! match/rematch state is intentionally never written to a configuration file.
use elite_stages_counterpick_core::storage::ConfigStore;
use elite_stages_counterpick_core::*;
use std::{
    env, fs,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

const CUSTOM: &str = "schema=1\nenabled=true\nfighter.mario=trail_castle,end,battle_field\n";

#[test]
fn fresh_process_configuration_continuation() {
    if let Ok(mode) = env::var("ELITE_COUNTERPICK_CONFIG_TEST_PHASE") {
        let path = env::var_os("ELITE_COUNTERPICK_CONFIG_TEST_PATH").unwrap();
        if mode == "save" {
            let config = Config::parse(CUSTOM).unwrap();
            assert_eq!(
                config.ranking(Fighter::from_key("mario").unwrap()).unwrap()[0],
                Stage::HollowBastion
            );
            let mut store =
                ConfigStore::load(std::path::Path::new(&path).parent().unwrap()).unwrap();
            store.save(config).unwrap();
        } else {
            assert_eq!(mode, "load");
            let config = ConfigStore::load(std::path::Path::new(&path).parent().unwrap())
                .unwrap()
                .current()
                .clone();
            assert_eq!(config, Config::parse(CUSTOM).unwrap());
            let mut engine = Counterpick::new(77, config);
            let ctx = ProposalContext {
                session: 77,
                next_epoch: 2,
                participants: [1, 2],
                supported_game: true,
                quickplay_or_elite: true,
                hazards_off: true,
                original_mod_enabled: true,
                rules_owner: RulesOwner::Local,
                enabled_pool: StagePool::from_stages(&Stage::ALL),
                original_eligible_pool: StagePool::from_stages(&Stage::ALL),
            };
            assert_eq!(
                engine.take_proposal(ctx),
                Decision::Baseline(Fallback::NoAcceptedRematch)
            );
            assert!(engine.begin_match(Match {
                session: 77,
                epoch: 1,
                supported_singles: true,
                players: [
                    Participant {
                        id: 1,
                        fighter: Fighter::from_key("mario")
                    },
                    Participant {
                        id: 2,
                        fighter: Fighter::from_key("little_mac")
                    }
                ],
            }));
            assert!(engine.complete_match(77, 1, Outcome::Winner(2)));
            assert!(engine.accept_rematch(AcceptedRematch {
                session: 77,
                previous_epoch: 1,
                next_epoch: 2,
                participants: [1, 2],
            }));
            assert_eq!(
                engine.take_proposal(ctx),
                Decision::Propose(Stage::HollowBastion)
            );
        }
        return;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = env::temp_dir().join(format!(
        "elite-counterpick-config-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("counterpick.cfg");
    for phase in ["save", "load"] {
        let launched = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let child = Command::new(env::current_exe().unwrap())
            .args(["--exact", "fresh_process_configuration_continuation"])
            .env("ELITE_COUNTERPICK_CONFIG_TEST_PHASE", phase)
            .env("ELITE_COUNTERPICK_CONFIG_TEST_PATH", &path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id();
        let output = child.wait_with_output().unwrap();
        if !output.status.success() {
            fs::remove_dir_all(&dir).unwrap();
            panic!(
                "{phase} process failed: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        eprintln!(
            "continuation phase={phase} child_pid={pid} launched_unix_ns={launched} exit={}",
            output.status.code().unwrap()
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            Config::parse(CUSTOM).unwrap().serialize()
        );
        eprintln!("continuation phase={phase} persisted_nondefault_config_equal=true");
    }
    fs::remove_dir_all(&dir).unwrap();
}
