# OP2Formats

Multiplatform Outpost 2 file formats library.

OP2Formats reads the data files of Outpost 2 (Dynamix/Sierra, 1997) in pure Rust, so the
same code runs on Windows, Linux and macOS: `.vol` archives (including LZH-compressed
entries), `.map` files, tilesets, `op2_art.prt` sprites, the stat sheets, sounds, the AVI
cutscenes, tables in `Outpost2.exe`, and the GOG and OPU installers.

## Layout

| Folder | What |
|---|---|
| `op2-formats/` | The Rust library: the readers themselves |
| `op2-mapjson/` | Converts `.map` files to and from OP2MapJsonTools' JSON maps, PerRowPadded (see its [README](op2-mapjson/README.md)) |
| `op2-opm/` | Reads and writes OP2MissionEditor's `.opm` mission files (see its [README](op2-opm/README.md)) |
| `op2formats-c/` | A C interface for C, C++ and other languages: `op2formats.dll` / `.so` / `.dylib` and a static library, with the header `include/op2formats.h` (see its [README](op2formats-c/README.md)) |

How the parts fit together: [ARCHITECTURE.md](ARCHITECTURE.md).

## Where it comes from

This is the `op2-formats` crate of [op2-rs](https://gitlab.com/bufke/op2-rs), bufke's
reimplementation of Outpost 2, written by bufke and op2-rs's contributors. It was
imported unmodified so that other Outpost 2 projects can use it on its own; the copy here is
op2-rs commit `f7e9a15` (see the git history). All credit for the format work goes to them.

The code in `op2-formats/` is kept identical to upstream so updates can be brought over
cleanly; changes needed to build it here live outside that folder (the root `Cargo.toml`
declares the workspace values it inherits, such as the `iced-x86` dependency). Fixes to the
readers themselves should go to op2-rs too.

## License

GPL-3.0-or-later, the same as op2-rs (see [LICENSE](LICENSE)). A program that links this
library must be distributed under the GPL as well.

## Building

```
cargo build
cargo test
```

`rust-toolchain.toml` pins Rust 1.99, the version op2-rs builds with; rustup installs it on
first use. Optional features of `op2-formats`: `inno` (GOG's offline installer), `opu` (the
OPU installer) and `fetch-opu` (downloading it).

C and C++ projects that use CMake can take the library with `add_subdirectory` or, once
installed, `find_package(OP2Formats)`; the top-level `CMakeLists.txt` runs Cargo. See
[op2formats-c/README.md](op2formats-c/README.md).

## Game data

No game data is included or extracted into this repository: you need your own copy of
Outpost 2. The tables the game keeps in `Outpost2.exe` (which animations each unit draws,
armor values, sound and music names) are read from the player's own copy when used.

Tests that need the game find an install through `$OP2_DIR` (an Outpost 2 folder with OPU
1.4.1) and skip their real-data checks without one. `OP2_REQUIRE_DATA=1` turns a skip into a
failure.

## Leftovers from op2-rs

A few parts still assume op2-rs's repository layout. None affects normal use:

- `source_tree_game_dir` looks for op2-rs's Wine prefix next to its build folder; here it
  finds nothing and the normal install search is used.
- A cross-check in `tests/game_data.rs` (`rules_and_tech_trees_parse`) reads op2-rs's
  recorded oracle runs and skips when they aren't there; only `OP2_REQUIRE_RUNS=1` (op2-rs's
  switch for them) makes that a failure, so leave it unset here. With `OP2_REQUIRE_DATA=1`
  every test passes against an OPU 1.4.1 install.
- The installer tests (`inno_extract`, `opu_package`) need test installers named by their
  own variables (see `tests/inno_test.iss`) and pass without checking anything otherwise.
