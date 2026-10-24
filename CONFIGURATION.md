# Configuration

Both files live in `sd:/ultimate/elite-legal-stages-public/`. The in-game menu
(**L + R + D-pad Up**, requires the imgui-smash host) edits both; each file has
its own save button and neither save rewrites the other file.

| File | Feature | Saved by |
| --- | --- | --- |
| `settings.toml` | Legal stage pool | **Save stage pool** |
| `counterpick.cfg` | Optimal counter pick | **Save counterpick preferences** |

The two features are separate. The counterpick only acts while the stage pool
is enabled with at least one stage. Enabling the pool never enables the
counterpick.

## Stage pool: `settings.toml`

This is the original Elite Legal Stages format, so an existing file is reused:

```toml
enabled = true
stages = [
    "battle_field",
    "battle_field_s",
    "end",
]
```

`enabled` comes first, then `stages`. `#` comments are allowed; other keys,
unknown or duplicate stages and files over 32 KiB are refused. A refused file is
left untouched, the pool stays inactive and the menu does not overwrite it. A
missing file means pool off with all eleven stages checked; nothing is written
until you save.

When enabled, the pool removes unchecked stages from the game's own legal list.
It never adds a stage the game would not allow. An empty `stages` list has the
same effect as a disabled pool and keeps the counterpick inactive. The menu
refuses to save an empty pool.

## Counterpick: `counterpick.cfg`

A separate versioned `key=value` format, not TOML. UTF-8, up to 32 KiB, one
record per line:

```text
schema=1
enabled=false
fighter.mario=yoshi_cartboard,battle_field,battle_field_s
fighter.little_mac=end,poke_stadium2
fighter.richter=battle_field_s,fox_lylatcruise,animal_city
```

`schema` and `enabled` are required. `enabled` is the counterpick toggle shown
as **Optimal counter pick** in the menu. Full-line `#` comments and blank lines
are accepted, and spaces around keys, values and commas are ignored. Inline
comments, quotes and aliases are not supported. Unknown keys, fighters or
stages, duplicate records or stages, unsupported schemas and malformed values
produce a line-numbered error.

Each fighter record lists stages from first to last preference. On an accepted
online 1v1 rematch, the plugin uses the loser's record and picks the first stage
that is both checked in the pool and allowed by the game for that pick. Disabled
stages are skipped, never re-enabled, and no default stage is appended. A
missing, empty or fully unavailable record leaves the game's pick unchanged.
`fighter.mario=` explicitly opts a fighter out. To change an order, edit or
replace its whole line; do not add a duplicate line.

The counterpick changes only picks made under the **local player's Preferred
Rules**. When the opponent's rules are used, the game's pick is unchanged.
Saving either file cancels any pending counterpick from an earlier result.

Fighter keys are listed in `src/roster.rs`. They are configuration names, not
native fighter IDs. Separate forms have separate profiles; for example, `pyra`
and `mythra`. The group keys `pokemon_trainer` and `pyra_and_mythra` are
accepted but have no default. An unidentified fighter gets no counterpick.

Stage keys:

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

## Shipped defaults

With no `counterpick.cfg`, the plugin uses `defaults/community-v1.cfg` in memory
with the counterpick off and writes nothing. That snapshot is low confidence:
one community publisher's ordered per-fighter stage lists, with no votes,
sample sizes or matchup data. It does not identify a stage that is best for
every fighter or opponent.

- 88 fighters have a nine-stage order covering every pool stage except Northern
  Cave and Hollow Bastion.
- Richter has only the three stages listed on the source page.
- `pokemon_trainer` and `pyra_and_mythra` have no record.

Sources, hashes and caveats are in
[defaults/provenance.json](defaults/provenance.json). You may add Northern Cave
or Hollow Bastion to your own profiles; the snapshot makes no claim about them.

## Saving and recovery

Loading and saving happen at boot and on an explicit menu save, never during
gameplay frames. A save writes and flushes a temporary file, then replaces the
destination; on failure, the previous file and active settings remain. An
invalid file is reported and preserved, not overwritten. Power-loss durability
depends on the platform filesystem.

For library users: `Config::parse` builds a complete candidate before anything
changes, `Config::set_enabled` and `set_ranking` edit a draft, and
`Config::serialize` returns canonical text. With the `std` feature,
`storage::ConfigStore` loads and saves `counterpick.cfg` only.

## Test status

Host storage tests pass. In an emulator, the menu saved both files and a fresh
process reloaded them. Physical-Switch filesystem behavior is not yet tested.
See the README's Validation section for gameplay status.
