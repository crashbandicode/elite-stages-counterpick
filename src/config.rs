use crate::{Fighter, Stage, StagePool};
use alloc::{collections::BTreeMap, format, string::String};

pub(crate) const MAX_CONFIG_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Ranking {
    stages: [Stage; 11],
    length: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Config {
    enabled: bool,
    profiles: BTreeMap<Fighter, Ranking>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigErrorKind {
    TooLarge,
    Syntax,
    UnknownKey,
    UnknownFighter,
    UnknownStage,
    DuplicateKey,
    DuplicateStage,
    UnsupportedSchema,
    InvalidBoolean,
    MissingSchema,
    MissingEnabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigError {
    /// One-based line; zero denotes a whole-file error.
    pub line: usize,
    pub kind: ConfigErrorKind,
}

impl Config {
    /// Edit a draft. The native menu publishes it only after storage succeeds.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Replace one fighter's entire preference order. An empty order opts out;
    /// validation errors leave the previous order untouched.
    pub fn set_ranking(&mut self, fighter: Fighter, stages: &[Stage]) -> Result<(), ConfigError> {
        let mut ranking = Ranking {
            stages: [Stage::Battlefield; 11],
            length: 0,
        };
        for &stage in stages {
            if ranking.stages[..ranking.length].contains(&stage) {
                return Err(ConfigError {
                    line: 0,
                    kind: ConfigErrorKind::DuplicateStage,
                });
            }
            // Eleven variants and no duplicates prove the next index fits.
            ranking.stages[ranking.length] = stage;
            ranking.length += 1;
        }
        self.profiles.insert(fighter, ranking);
        Ok(())
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    pub fn ranking(&self, fighter: Fighter) -> Option<&[Stage]> {
        self.profiles
            .get(&fighter)
            .map(|ranking| &ranking.stages[..ranking.length])
    }

    pub fn choose(&self, fighter: Fighter, pool: StagePool) -> Option<Stage> {
        self.ranking(fighter)?
            .iter()
            .copied()
            .find(|&stage| pool.contains(stage))
    }

    /// Deliberately small versioned format, NOT a TOML parser. Full-line #
    /// comments, key=value records, comma-separated stage keys; blank rank lists
    /// explicitly opt a fighter out. No quoting/inline comments/implicit aliases.
    /// Parsing is transactional: the caller receives either a complete candidate
    /// or an error, never a partially updated active configuration.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        if text.len() > MAX_CONFIG_BYTES {
            return Err(ConfigError {
                line: 0,
                kind: ConfigErrorKind::TooLarge,
            });
        }
        let mut config = Self::default();
        let mut schema_seen = false;
        let mut enabled_seen = false;
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let fail = |kind| ConfigError { line, kind };
            let raw = raw.trim();
            if raw.is_empty() || raw.starts_with('#') {
                continue;
            }
            let (key, value) = raw
                .split_once('=')
                .ok_or_else(|| fail(ConfigErrorKind::Syntax))?;
            let (key, value) = (key.trim(), value.trim());
            if key == "schema" {
                if schema_seen {
                    return Err(fail(ConfigErrorKind::DuplicateKey));
                }
                if value != "1" {
                    return Err(fail(ConfigErrorKind::UnsupportedSchema));
                }
                schema_seen = true;
            } else if key == "enabled" {
                if enabled_seen {
                    return Err(fail(ConfigErrorKind::DuplicateKey));
                }
                config.enabled = match value {
                    "true" => true,
                    "false" => false,
                    _ => return Err(fail(ConfigErrorKind::InvalidBoolean)),
                };
                enabled_seen = true;
            } else if let Some(key) = key.strip_prefix("fighter.") {
                let fighter =
                    Fighter::from_key(key).ok_or_else(|| fail(ConfigErrorKind::UnknownFighter))?;
                if config.profiles.contains_key(&fighter) {
                    return Err(fail(ConfigErrorKind::DuplicateKey));
                }
                let mut ranking = Ranking {
                    stages: [Stage::Battlefield; 11],
                    length: 0,
                };
                if !value.is_empty() {
                    for token in value.split(',') {
                        let stage = Stage::from_key(token.trim())
                            .ok_or_else(|| fail(ConfigErrorKind::UnknownStage))?;
                        if ranking.stages[..ranking.length].contains(&stage) {
                            return Err(fail(ConfigErrorKind::DuplicateStage));
                        }
                        // There are only eleven valid Stage keys; uniqueness
                        // proves length < 11 before each insertion.
                        ranking.stages[ranking.length] = stage;
                        ranking.length += 1;
                    }
                }
                config.profiles.insert(fighter, ranking);
            } else {
                return Err(fail(ConfigErrorKind::UnknownKey));
            }
        }
        if !schema_seen {
            return Err(ConfigError {
                line: 0,
                kind: ConfigErrorKind::MissingSchema,
            });
        }
        if !enabled_seen {
            return Err(ConfigError {
                line: 0,
                kind: ConfigErrorKind::MissingEnabled,
            });
        }
        Ok(config)
    }

    /// Canonical, deterministic representation; filesystem persistence belongs
    /// to the future adapter and must use replace-after-success semantics.
    pub fn serialize(&self) -> String {
        let mut text = format!("schema=1\nenabled={}\n", self.enabled);
        for (&fighter, ranking) in &self.profiles {
            text.push_str("fighter.");
            text.push_str(fighter.key());
            text.push('=');
            for (index, stage) in ranking.stages[..ranking.length].iter().enumerate() {
                if index != 0 {
                    text.push(',');
                }
                text.push_str(stage.key());
            }
            text.push('\n');
        }
        text
    }
}
