# Changelog

Changes to the `op2-formats` crate and its C interface, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [semver](https://semver.org/): while
the version is 0.x, a breaking change raises the minor version.

## [Unreleased]

## [0.3.0] - 2026-11-10

op2-rs's strict string-table check, from bufke (pull request 1). One breaking change, in the install check.

### Added

- `strings::GameStrings::load_checked` and `parse_checked`: a table that lacks any id in `0..=LAST_ID` (the ids
  op2-rs reads) is refused with `strings::Error` (`Missing`, or `Incomplete` with how many are missing and the
  first), so a caller using the table as its only source of UI text never meets a hole. `GameStrings::text` looks up
  such a fixed id. `GameStrings::load` is unchanged: it still gives an empty table on a stock install.

### Changed

- Breaking: `install::Check::exe_tables_problem` is now `data_problem`, and with OPU present it also reports a
  `language.ini` that `load_checked` refuses (or the game's files failing to open), so an install whose strings
  op2-rs can't use doesn't pass the check.

## [0.2.0] - 2026-11-10

The community's `.opm` missions, JSON maps and tile group files, and the game's mission DLLs, in the one crate.
Nothing from 0.1.0 changed: the new parts are new modules, and the two that need extra dependencies are behind
features that are off by default.

### Added

- `opm` module (feature `opm`): reads and writes OP2MissionEditor's `.opm` missions. The reader accepts every known
  writer's layout (OP2MissionEditor's, the SDK tools', the older SDK layout, comments, trailing commas, `null`s,
  members in any order, unknown members kept); the writer produces the editor's layout. `Unit::cargo` decodes a
  unit's cargo: truck cargo, starship modules, wreckage, gene banks, a ConVec's structure kit, weapons. Example:
  `opm_rewrite`.
- `mapjson` module (feature `mapjson`): converts `.map` files to and from OP2MapJsonTools' JSON maps (PerRowPadded),
  byte for byte as that tool writes them.
- `mapjson::TileGroupJson`: OP2Mapper3's and OP2TileSetTools' tile group files, read and written in their layout,
  and converted to and from a `.map`'s tile groups (`map_tile_groups`, `tileset_prefix`).
- `mission` module: reads a mission DLL's description without loading it: `DescBlock` (type, players, maximum tech
  level, unit-only flag), `LevelDesc`, `MapName`, `TechtreeName`, `DescBlockEx`'s extra AI players, every export,
  and the game's file checksum. It warns where a DLL would confuse the game's own PE reader. `mission::missions`
  lists an install's missions; `multiplayer_name` reads a lobby file name's type and player count.
- `GameFiles::loose_files`: every loose file the lookups can resolve to.
- Tests: `tests/missions.rs` checks every count in `docs/mission-dlls.md` against an OPU 1.4.1 install (159
  missions); `tests/opm.rs` and `tests/mapjson.rs` with their sample files; two `.opm` missions in `tests/data/opm`.

### Compared with the earlier `op2-opm` crate

The `opm` module began as a separate crate in this repository, never published. Its model now follows the SDK and
the game:

- `MissionType` values are `DescBlock`'s: `Colony` −1, `AutoDemo` −2, `Tutorial` −3, `MultiLandRush` −4 to
  `MultiLastOneStanding` −8 (they were in the wrong order). `MissionType::number` also reads a campaign mission's
  number.
- `MoraleLevel` is numbered from `Excellent` 0, and an absent one is `Excellent`, as the SDK reads it.
- `BotType` has the SDK's ten presets (it had three).
- No colour 7 (`White`) and no `Common` or `Rare` marker: the SDK defines none, so they read as unknown names and are
  written back as they were.
- An empty truck is `Empty` (`TruckCargo` 0), not `None`.
- A numeric `CargoType` is decoded by its low 4 bits, as the game stores truck cargo, so `90` and `10` are both a gene
  bank.
- A ConVec's cargo is a structure kit, with `CargoAmount` its weapon.
- `Region` has its `ID`.

### C interface (`op2formats-c`, not on crates.io)

- Mission DLLs: `op2f_mission_*` and `op2f_game_missions`.
- Tile group files: `op2f_tilegroup_*`, `op2f_map_add_tile_group` and `op2f_map_remove_tile_group`.
- `Op2fOpmLevel.mission_type` gives the corrected values, and `Op2fOpmCargo` gains `Kit` (added last, so the other
  values keep their numbers).
- 345 functions in all; the CMake package version is 0.2.0.

## [0.1.0] - 2026-11-09

The first release: the `op2-formats` crate of [op2-rs](https://gitlab.com/bufke/op2-rs), bufke's reimplementation
of Outpost 2, as op2-rs had it at commit `69fb43ff`, with its full history. The public API is op2-rs's, unchanged.

- Readers for `.vol` archives (and LZH), `.map` files, tilesets, `op2_art.prt` sprites, the rule sheets and tech
  trees, UI strings, `op2.clm` music and `.wav` sounds, the AVI cutscenes (Cinepak, MS ADPCM), the Blight's masks,
  Windows resources in the game's DLLs, and the tables in `Outpost2.exe` (`exe::GameExe`).
- Finding and checking an install; GOG's offline installer (feature `inno`), OPU's installer (feature `opu`) and
  downloading it (feature `fetch-opu`).
- Package metadata for crates.io (repository, description, `rust-version` 1.99).

[Unreleased]: https://github.com/leviathan400/OP2Formats/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/leviathan400/OP2Formats/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/leviathan400/OP2Formats/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/leviathan400/OP2Formats/releases/tag/v0.1.0
