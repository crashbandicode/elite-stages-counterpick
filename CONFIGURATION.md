# Configuration contract

`Config::parse` accepts UTF-8 text up to 32 KiB, with one `key=value` record per
line. This is a separate versioned format, not TOML. The standalone plugin keeps
it in `counterpick.cfg` beside its stage-pool `settings.toml`, under
`sd:/ultimate/elite-legal-stages-public/`; neither file replaces the other.

```text
schema=1
enabled=false
fighter.mario=yoshi_cartboard,battle_field,battle_field_s
fighter.little_mac=end,poke_stadium2
fighter.richter=battle_field_s,fox_lylatcruise,animal_city
```

Both `schema` and `enabled` are required. `enabled` is the independent requested
counterpick toggle. The standalone plugin's stage-pool enable switch also applies.
Full-line `#` comments and blank lines are accepted. Spaces around keys, values
and commas are ignored. Inline comments, quotes and implicit aliases are not
supported. Unknown keys, fighters or stages, duplicate records, duplicate stages,
unsupported schemas and malformed values produce a line-numbered error.

Each fighter record orders preferences from first to last. A stage is eligible
only if it is currently enabled in this plugin's pool and already
permitted by the original selection operation. Missing, empty or
disjoint profiles leave selection to the original behavior. No default stage is
appended and disabled stages are never re-enabled. An empty record such as
`fighter.mario=` explicitly opts that fighter out. To change a recommendation,
edit or replace its entire line; do not add a duplicate line.

Stable fighter keys are listed in `src/roster.rs`. They are configuration names,
not native fighter-kind values. Separate fighter forms have separate profiles.
The additional `pokemon_trainer` and `pyra_and_mythra` group keys have no inferred
aggregate defaults. The native adapter uses its qualified captured fighter
identity; an unknown identity remains unknown and does not invent a recommendation.

Supported stage keys are:

| Stage | Key |
| --- | --- |
| Battlefield | `battle_field` |
| Small Battlefield | `battle_field_s` |
| Final Destination | `end` |
| Yoshi's Story | `yoshi_cartboard` |
| Lylat Cruise | `fox_lylatcruise` |
| Pokémon Stadium 2 | `poke_stadium2` |
| Smashville | `animal_village` |
| Kalos Pokémon League | `poke_kalos` |
| Town and City | `animal_city` |
| Northern Cave | `ff_cave` |
| Hollow Bastion | `trail_castle` |

These are the supported pool keys; tournament legality varies by ruleset.
The default snapshot covers nine of the eleven stages. Users may add Northern
Cave or Hollow Bastion to their own profiles; the research snapshot makes no
claim about their rank. Defaults start with the counterpick toggle off.

Parsing creates a complete candidate before the active configuration changes.
`Counterpick::replace_config` clears pending result/rematch proposals while
retaining an in-progress match. `Config::set_enabled` and `set_ranking` edit a
draft; `Config::serialize` returns canonical text.

With the `std` feature, `storage::ConfigStore` loads `counterpick.cfg` from a
supplied directory. A missing file uses the shipped disabled defaults without
writing; an invalid file is reported and preserved. Saving writes and flushes a
separate temporary file, replaces `counterpick.cfg`, then publishes the draft in
memory. A failed replacement leaves the active configuration unchanged and never
deletes the previous destination. This config store never writes `settings.toml`;
the stage-pool settings have their own save operation.
Power-loss durability depends on the platform filesystem. Loading and saving are
for boot and explicit menu actions, never per-frame callbacks.

Host storage, native menu saves and fresh-process continuation are tested. Open
the native menu with **L + R + D-pad Up** and use **Save counterpick preferences**
to publish its draft. **Save stage pool** is separate. The menu reuses an existing
compatible ImGui host. The accepted-rematch gameplay check and physical-Switch
filesystem acceptance remain pending.
