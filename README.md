# OP2Formats

Multiplatform Outpost 2 file formats library.

OP2Formats reads the data files of Outpost 2 (Dynamix/Sierra, 1997) in pure Rust, so the
same code runs on Windows, Linux and macOS: `.vol` archives (including LZH-compressed
entries), `.map` files, tilesets, `op2_art.prt` sprites, the stat sheets, sounds, the AVI
cutscenes, tables in `Outpost2.exe`, mission DLLs' descriptions, the GOG and OPU installers,
and the community tools' `.opm` missions, JSON maps and tile group files. It writes maps and
the JSON formats too.

The Rust library is [`op2-formats` on crates.io](https://crates.io/crates/op2-formats):

```
cargo add op2-formats --features opm,mapjson
```

C and C++ programs use it through a C interface (below). What changed in each release:
[CHANGELOG.md](CHANGELOG.md).

## Layout

| Folder | What |
|---|---|
| `src/`, `tests/` (the root crate, `op2-formats`) | The Rust library: every reader and writer. OP2MissionEditor's `.opm` missions ([`src/opm/`](src/opm/README.md)) and OP2MapJsonTools' JSON maps ([`src/mapjson/`](src/mapjson/README.md)) are modules behind the `opm` and `mapjson` features |
| `op2formats-c/` | A C interface for C, C++ and other languages: `op2formats.dll` / `.so` / `.dylib` and a static library, with the header `include/op2formats.h` (see its [README](op2formats-c/README.md)) |

The root `Cargo.toml` is both the `op2-formats` crate and the workspace for the C interface.

How the parts fit together: [ARCHITECTURE.md](ARCHITECTURE.md). The file formats themselves are described in
[docs/](docs/README.md) (the original game's) and [docs-community/](docs-community/README.md) (the community's tools').

## Where it comes from

The `op2-formats` crate was written by bufke and op2-rs's contributors as part of
[op2-rs](https://gitlab.com/bufke/op2-rs), bufke's reimplementation of Outpost 2. It moved
here, with its full git history, so that it can be maintained for every Outpost 2 project and
published on its own (op2-rs issue 33). All credit for the format work goes to them. The
history up to op2-rs commit `69fb43ff` is op2-rs's; op2-rs plans to depend on the published
crate.

The C interface, the `.opm` and JSON map modules and the format documents were developed in this
repository's own history, which is merged in beside op2-rs's.

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
OPU installer), `fetch-opu` (downloading it), `opm` (`.opm` missions) and `mapjson` (JSON maps).

C and C++ projects that use CMake can take the library with `add_subdirectory` or, once
installed, `find_package(OP2Formats)`; the top-level `CMakeLists.txt` runs Cargo. See
[op2formats-c/README.md](op2formats-c/README.md).

## Game data

No game data is included or extracted into this repository: you need your own copy of
Outpost 2. The tables the game keeps in `Outpost2.exe` (which animations each unit draws,
armor values, sound and music names) are read from the player's own copy when used.

Tests that need the game find an install through `$OP2_DIR` (an Outpost 2 folder with OPU
1.4.1) and skip their real-data checks without one. `OP2_REQUIRE_DATA=1` turns a skip into a
failure. Optional sample files outside the repository: `$OP2_OPM_DIR` (a folder of `.opm`
missions), `$OP2_MAPJSON` (OP2MapJsonTools' JSON maps) and `$OP2_OPU_INSTALLER` (OPU's
official installer).

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
