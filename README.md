# Legal Stages + Loser Counterpick

An experimental standalone Skyline plugin for Smash Ultimate 13.0.5. It has two
separate features:

- **Legal stage pool** limits the stages the game may pick to the ones you
  check. It only narrows the game's own list; it never adds a stage.
- **Optimal counter pick** (optional) affects an accepted online 1v1 rematch.
  Under *your* preferred rules, it makes the game pick the loser's
  highest-ranked eligible stage.

The counterpick requires the stage pool to be enabled. Enabling the pool alone
never turns the counterpick on. Both features start disabled.

**Status: experimental and unstable until physical-Switch review.** Read
[Validation](#validation) before relying on it. Download the emulator-tested
build from [Forgejo Releases](https://git.syntaxbyexample.com/intpa/elite-stages-counterpick/releases).

## Requirements and installation

- Smash Ultimate 13.0.5 with Skyline.
- **Remove the original Elite Legal Stages NRO.** This plugin replaces it and
  hooks the same game function. If another plugin already owns that hook, this
  plugin refuses to install its own hook.
- A compatible **imgui-smash host** NRO (ImGui `1.92.7 WIP`) for the in-game
  menu. This plugin draws through that existing host and never installs its own
  renderer. Without the host, the menu is unavailable but the files still work.

Build the release NRO (see [Building](#building)) and copy
`libelite_stages_counterpick.nro` to
`sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/`.

## In-game menu

Press **L + R + D-pad Up** to open or close **Legal Stages + Loser Counterpick**.
This is a separate window drawn by the shared ImGui host, not a page inside
another plugin's menu.
Edits are drafts until saved; closing the window discards unsaved edits.

- **Enable legal stage pool**, the eleven stage checkboxes and **Save stage
  pool**. At least one stage must stay checked.
- **Optimal counter pick** turns the counterpick on or off. The menu warns when
  the stage pool is not enabled.
- **Fighter** and **Preferred stage** with **Make first preference** move one
  stage to the top of that fighter's order. **Clear this fighter's
  preferences** opts the fighter out. **Save counterpick preferences** saves
  these settings.

The two save buttons write different files and do not overwrite each other's
draft. The menu also reports when a hook is unavailable because the game
version is incompatible or another plugin owns the site.

Settings live in `sd:/ultimate/elite-legal-stages-public/`:

- `settings.toml`: the stage pool, in the original Elite format.
- `counterpick.cfg`: the counterpick toggle and ordered fighter preferences.

Full ordered rankings can be edited in `counterpick.cfg`; see
[CONFIGURATION.md](CONFIGURATION.md).

## How the counterpick decides

After an online 1v1 result, the plugin records the winner and loser. When both
players accept the rematch, it picks the first stage in the loser's order that
is both checked in your pool and allowed by the game for that pick.

- It acts only when your **Preferred Rules** setting is used for that pick.
  If the opponent's rules are used, the game's choice is left unchanged.
- An empty, missing or unmatched profile, or an unknown fighter, leaves the
  game's choice unchanged.
- Saving new settings cancels any pending counterpick from an earlier result.

## Recommendations

The shipped order in `defaults/community-v1.cfg` is a low-confidence snapshot
of one community publisher's per-fighter stage lists (retrieved 2026-10-16).
It is not a matchup analysis, and no stage is optimal for every fighter or
opponent. Coverage:

- 88 fighters have a nine-stage order. Richter has only the three stages that
  the source page lists.
- Pokemon Trainer and Pyra/Mythra group keys have no profile. Their separate
  forms do have profiles.
- Northern Cave and Hollow Bastion are never ranked by default.

Sources, hashes and caveats are in
[defaults/provenance.json](defaults/provenance.json). Replace any order with
your own.

## Validation

Proven with the current source:

- 81 host tests, zero-warning Clippy for host and native targets, no-default
  builds and the native release build passed.
- In an emulator, the menu saved both files and a fresh process reloaded them.
  The menu and settings code is unchanged since that test.
- In an emulator LAN-online Quickplay match with the local player's preferred
  rules on, the remote Mario lost. Both players accepted the rematch, and the game
  loaded Pokemon Stadium 2, Mario's first preference in the test profile.
- With the local Mii Brawler losing instead, the native proposal and actual
  accepted rematch both selected Final Destination, Brawler's first preference.
- Turning counterpick off preserved the native Smashville pick on an accepted
  rematch, with the legal stage pool still enabled.
- With local Preferred Rules off, the native adapter retained the game's
  choice and the accepted rematch used the opponent's Small Battlefield setting.
- The final pair completed both games and returned to character select after
  the native rematch timeout. Tests used the standalone plugin with the full
  Training Modpack and no original Elite Legal Stages NRO. The opponent had
  no Skyline NRO plugins installed.

Not yet proven:

- Any physical Switch test.
- Both players running this stage picker, or compatibility with arbitrary
  opponent mods.

These are targeted gameplay checks, not a long-running stability certification.
Test-driver navigation and timed-input failures were recorded separately from
feature outcomes; they are not counted as passes.

## Building

The dependency-free policy library is in `src/`; the Skyline adapter and menu
are in `native/`. Host checks from the repository root:

```text
cargo test --all-targets --features native-construction-alpha
cargo clippy --all-targets --features native-construction-alpha -- -D warnings
cargo build --no-default-features
```

Run the native build from `native/`, using the AArch64 Skyline target:

```text
cargo skyline build --release --no-default-features --features adapt-native
```

`COUNTERPICK_IMGUI_DIR` must point to the directory with the pinned
`libimgui_smash.a` archive and its `.sha256` file, as checked by
`native/build.rs`. Without `adapt-native`, the build installs no hooks.
The original plugin binary and game assets are not included.
