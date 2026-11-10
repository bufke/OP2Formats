# Working notes

State and plan for OP2Formats. Keep it short and current.

## Where it stands (2026-11-10)

- **Home.** op2-formats moved out of op2-rs into this repository (op2-rs issue 33). The repo was rebuilt from
  op2-rs's history of `tools/crates/op2-formats` (to op2-rs `69fb43ff`, extracted with `git filter-repo`, the crate
  at the root) merged with this repo's own history; the old repo stays as OP2Formats-Old. Edit op2-formats here.
- **Releases** (crates.io, tags on GitHub): 0.1.0 (2026-11-09, `v0.1.0`), the crate as op2-rs had it; 0.2.0
  (2026-11-10, `v0.2.0`), with what 0.2 adds below. The SVN copy follows each release.
- **Layout.** The root `Cargo.toml` is the `op2-formats` crate and the workspace for `op2formats-c` (not published).
  The crate takes `edition`, `license` and `iced-x86` from the workspace, as it did in op2-rs; `cargo publish` fills
  them in. Features: `inno`, `opu`, `fetch-opu` (installers), `opm` and `mapjson` (the community's JSON formats).
- **What 0.2 adds.**
  - `opm`: OP2MissionEditor's `.opm` missions, read and written (was the `op2-opm` crate; history kept with
    `git mv`). The model follows the SDK and the game: MissionType is the `DescBlock` value, MoraleLevel numbered
    from Excellent, the SDK's ten BotTypes, no colour 7 or Common/Rare markers, `Empty` trucks, numeric cargo by its
    low 4 bits, ConVec kits, Region `ID`.
  - `mapjson`: OP2MapJsonTools' JSON maps, PerRowPadded only (was the `op2-mapjson` crate). Every shipped map
    round-trips to the same `.map` bytes; `map1.json` (`OP2_MAPJSON`, not committed) writes back byte for byte.
  - `mapjson::TileGroupJson`: OP2Mapper3's and OP2TileSetTools' tile group files, read, written and converted to and
    from a `.map`'s groups; `cm01.map`'s `snowroc9` comes out byte for byte as OP2Mapper3 wrote it.
  - `mission`: mission DLLs read as data (`DescBlock`, the texts, exports, the game's file checksum 0x40C050), every
    count in `docs/mission-dlls.md` checked on OPU 1.4.1's 159 missions.
- **Game tables.** op2-formats reads the unit art, armor values and other tables from the player's `Outpost2.exe` at
  runtime (`exe::GameExe`); nothing from the game is embedded.
- **C interface** (`op2formats-c/`): 345 functions, 35 structs, one source file per area, header by cbindgen 0.29.4.
  Checked on Windows with MSVC: the header compiles as C and C++ with `/W4 /WX`, every struct has the same size and
  alignment in C, C++ and Rust, the DLL exports every declared function. The OPU installer functions pass against
  OPU 1.4.1's official installer (`OP2_OPU_INSTALLER`); GOG's installer functions are untested against a real one.
- **CMake.** The top-level `CMakeLists.txt` runs Cargo and defines `OP2Formats::OP2Formats` / `::shared` /
  `::static` and `op2formats_copy_runtime`, and installs a `find_package` config (`cmake/`). Checked with MSVC 2022
  and CMake 3.31 (Ninja and Visual Studio generators, Debug and Release, shared and static, installed and found).
- **Tests and CI.** With an OPU 1.4.1 install as `OP2_DIR` and `OP2_REQUIRE_DATA=1`, every test passes with
  `--all-features` (op2-rs's recorded-run check follows its own `OP2_REQUIRE_RUNS`, left unset). CI (GitHub Actions)
  runs clippy and the tests on Ubuntu, Windows and macOS with no, default and all features, without game data.

## Next steps

1. **After 0.2.0**: a note on op2-rs issue 33. Adding bufke as a crates.io owner is the user's call. A release sets
   the version in both `Cargo.toml`s, `CMakeLists.txt` and the two `find_package` examples.
2. **C interface**: test the GOG installer functions with GOG's offline installer. Try MinGW
   (`x86_64-pc-windows-gnu`, what op2remake's Qt build uses; the CMake branch for it is written but untried) and the
   C/CMake side on Linux and macOS.
3. **The rest of `GameExe` in the C interface**: build lists, cell types, music names and volumes, cursors, morale
   modifiers, research efficiency, select sounds.
4. **CI**: add the C side (cbindgen's output matches the committed header, the CMake example builds) and a MinGW job.
5. **Tile groups in JSON maps** (IDEAS.md): a JSON map has no `tileGroups` member yet; agree a layout first.
6. **`opm`, next phase**: effective mission (a variant and difficulty merged onto the master), `.opm` to map and
   in-game coordinates, unit names to unit art, validation against the map and tech tree, setters in the C interface.
7. **Releases**: op2-rs needs reader fixes released within days. The repo stays on leviathan400 for now
   (OutpostUniverse maybe later).

Not planned for now: saved games (`.op2`) and OP2Mapper2's files (the user's call: OP2Mapper2 is old and unused).

## Checks before a commit

- `cargo clippy --workspace --all-targets -- -D warnings`, also with `--no-default-features` and `--all-features`.
- `cargo test --workspace` and `cargo test --workspace --all-features` without game data (as CI runs them), and with
  `OP2_DIR` + `OP2_REQUIRE_DATA=1` (all pass; leave `OP2_REQUIRE_RUNS` unset). Sample files outside the repo:
  `OP2_OPM_DIR`, `OP2_MAPJSON`, `OP2_OPU_INSTALLER` (see the module READMEs).
- After changing the C interface: regenerate the header with cbindgen, check it compiles as C and C++ with warnings
  as errors, that every struct has the same size and alignment in C, C++ and Rust, and that the DLL exports every
  function the header declares.

## License note

GPL-3.0-or-later, as op2-rs. Any program that links OP2Formats (including through the C interface) must be
distributed under the GPL. A permissive license would need the agreement of op2-formats' contributors: bufke (nearly
all of it), Leviathan and johnathonfox.
