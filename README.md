# Legal Stages + Loser Counterpick

An experimental standalone Skyline plugin for Smash Ultimate 13.0.5. It has two
separate features:

- **Legal stage pool** limits the stages the game may pick to the ones you
  check. It only narrows the game's own list; it never adds a stage.
- **Optimal counter pick** (optional) affects an accepted online 1v1 rematch.
  Under *your* preferred rules, it makes the game pick the loser's
  highest-ranked eligible stage.

The counterpick requires the stage pool to be enabled. Enabling the pool alone
never turns the counterpick on. With no settings files yet, both features
start disabled.

**Status: experimental and unstable until physical-Switch review.** Read
[Validation](#validation) before relying on it. Download the emulator-tested
build from [Forgejo Releases](https://git.syntaxbyexample.com/intpa/elite-stages-counterpick/releases).
The release ZIP also holds the older Online Deluxe build it was tested with. If
you already have Online Deluxe 0.8.0 or later, copy only
`libelite_stages_counterpick.nro`.

## Requirements and installation

- Smash Ultimate 13.0.5 with Skyline.
- **Remove the original Elite Legal Stages NRO.** This plugin replaces it and
  hooks the same game function. If another plugin already owns that hook, this
  plugin refuses to install its own hook.
- Online Deluxe 0.5.3 or later for the settings page; 0.8.0 or later shows the
  status table described below. Deluxe draws the page; this plugin draws
  nothing itself. Without Deluxe there is no page, but the stage pool and the
  counterpick still work from their files. Details are in
  [CONFIGURATION.md](CONFIGURATION.md#the-deluxe-page).

Copy `libelite_stages_counterpick.nro` from the release (or build it, see
[Building](#building)) to
`sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/`.

## In-game menu

The settings are one own page in the Online Deluxe window, in the same cycle as
the other plugins' pages. Hold **L + R** and press **D-pad Down** to step
through Deluxe's pages (Settings, the shared Plugins page when a plugin uses
it, each plugin's own page, Performance, Window Options) until
**Stages + Counterpick** appears. If it never appears, check that
**Window Options** doesn't hide it. Hold **ZR** and
press **Left** or **Right** to change the selected row. Up and Down move
between rows.

Each accepted change is written by the settings worker. The draw thread does
not write files. A refused or failed write leaves the previous file in place
and puts the row back. Unchecking the last stage is refused.

**Stages + Counterpick** has **Legal stages**, the eleven stages, and
**Counterpick**. The rest of the page is passive status, not settings. The
cursor skips it and Left/Right do nothing there:

- **Needs pool** reads Yes while counterpick is on and the pool is off.
- A small table, **Pool | Results | Save**, with a check mark under a column
  that is ready and a cross under one that is not:
  - **Pool**: the stage-list hook is installed.
  - **Results**: the result and rematch hooks are installed.
  - **Save**: a cross while a save is running (yellow), after a failed save or
    when both settings files were refused at startup (red); a check only
    once a save succeeded or nothing is pending.

The table needs Online Deluxe 0.8.0 or later. With an older Deluxe the same
information appears as four plain lines instead: **Needs pool** (No or Yes),
**Stage filter** and **Rematch pick** (Ready or Unavailable) and **Save**
(Ready, Saving, Not saved or No file). Those older builds can still rest the
cursor on those lines. Nothing is written either way.
The page does not edit fighter orders. **Counterpick** only turns the feature
on or off and writes the existing rankings back with that toggle. Edit orders
in `counterpick.cfg`.

The pool rows and the counterpick toggle write different files. A pool write
does not replace the counterpick file, and a counterpick write does not replace
the pool file.

Settings live in `sd:/ultimate/elite-legal-stages-public/`:

- `settings.toml`: the stage pool, in the original Elite format.
- `counterpick.cfg`: the counterpick toggle and ordered fighter preferences.

The legal stage list is an ordinary text file you can edit without the menu:
`sd:/ultimate/elite-legal-stages-public/settings.toml`. It is the same file the
original Elite plugin used, so an existing one keeps working. Edit it with the
game closed; it is read once at boot. Full ordered rankings are edited in
`counterpick.cfg`. Both formats are in [CONFIGURATION.md](CONFIGURATION.md).

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

Validation retained for the unchanged selection policy, plus checks of the new
Deluxe page:

- The preceding standalone build passed 81 host tests. The first page build
  passed eight page/ABI tests and reran sixteen settings tests; the current
  build, with the status table, passed ten page/ABI tests. Each had
  zero-warning Clippy for host and native targets, no-default builds and the
  release build. These counts overlap; they are not a combined unique-test
  total.
- With the first page build, in Training, the Deluxe page appeared alongside
  the other plugin pages. Both toggles saved, existing fighter rankings stayed
  intact, and hook/save statuses read Ready. Earlier fresh-process persistence
  evidence remains applicable to the unchanged file format and storage
  functions.
- With the current build, in an emulator, the status table drew centered, the
  cursor skipped it in both directions, the pool saved off and on and read
  back, **Save** showed a check once saving finished, and the emulator exited
  normally.
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
- With the first page build installed on **both** peers and Preferred Rules enabled,
  the local Brawler lost and both accepted the rematch. The native proposal
  selected Final Destination and both games loaded that stage. The rematch
  ran and both eventually returned to character select. Both emulators then
  exited normally. The second match's automation assertion stopped because its
  hit counter missed projectile damage; the planned final No/No sequence was
  not executed, so that sequence is not claimed as a pass.

Not yet proven:

- Any feature on a physical Switch. The current build is installed on a Switch,
  and the plugin has loaded there next to other plugins (crash reports from
  unrelated crashes, 2026-11-04 to 2026-11-08, list it), but its page, stage
  pool and counterpick have not been checked there.
- Online play through Nintendo's servers (Quickplay, including Elite Smash),
  offline modes and local wireless. Every match above was emulator LAN-online
  Quickplay.
- The stage pool on its own: no recorded match checked that an unchecked
  stage is never picked. The matches above checked the counterpick.
- Compatibility with arbitrary opponent mods.

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
cargo test --manifest-path native/Cargo.toml --features adapt-native --lib
```

Run the native build from `native/`, using the AArch64 Skyline target:

```text
cargo skyline build --release --no-default-features --features adapt-native
```

Without `adapt-native`, the build installs no hooks. The native plugin
resolves Deluxe at runtime and does not link an ImGui client. The original
plugin binary and game assets are not included.
