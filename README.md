# Legal Stages + Loser Counterpick

An experimental standalone Skyline plugin for Smash Ultimate 13.0.5. It provides
a configurable legal-stage pool and an optional loser counterpick for accepted
rematches. It does not require the original Elite Legal Stages NRO; remove that
NRO before using this replacement so both plugins do not own the same hook.

The counterpick proposes the loser's highest-ranked eligible stage through the
game's preferred-rules path. It cannot guarantee the stage when the opponent's
rules are chosen. Rankings are editable recommendations, not a claim that one
stage is objectively optimal in every matchup.

**Acceptance is incomplete.** The native plugin builds, its menu saves, and a
fresh emulator process reloads those settings. The paired gameplay test of the
loser → accepted rematch → preferred stage path and physical-console acceptance
remain pending. This is development source, not a stable release.

## Configuration

With a compatible ImGui host installed, press **L + R + D-pad Up** to open
**Legal Stages + Loser Counterpick**. Use **Save stage pool** for the pool and
**Save counterpick preferences** for the separate counterpick settings. Both
features start disabled. The menu uses the host's existing drawing callbacks;
this plugin does not install another renderer.

The files live in `sd:/ultimate/elite-legal-stages-public/`:

- `settings.toml`: stage-pool settings.
- `counterpick.cfg`: versioned counterpick toggle and ordered fighter preferences.

Only stages allowed by both the pool and the current native selection operation
are eligible. Full rankings can also be edited in `counterpick.cfg`. See
[CONFIGURATION.md](CONFIGURATION.md) for the format. The recommendation snapshot
records sources, retrieval dates, coverage and uncertainty in
[defaults/provenance.json](defaults/provenance.json).

## Source and validation

The dependency-free policy library is in `src/`; the Skyline adapter and menu
are in `native/`. Native hooks check the supported instruction fingerprints
before installation and refuse incompatible or already-owned sites. The original
plugin binary and game assets are not included.

The core's 79 tests, strict all-target Clippy, no-default build and native release
build passed. Native menu persistence and fresh-process loading passed in Eden.
These checks do not substitute for the outstanding gameplay and console tests.

Host checks run from this repository:

```text
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --no-default-features
```

The native build uses `native/Cargo.toml`, the `adapt-native` feature and the
AArch64 Skyline target. `COUNTERPICK_IMGUI_DIR` must point to the pinned
`libimgui_smash.a` archive and checksum sidecar expected by `native/build.rs`.
The default native feature set deliberately installs no hooks.
