# OP2Formats

Multiplatform Outpost 2 file formats library.

OP2Formats reads the data files of Outpost 2 (Dynamix/Sierra, 1997) in pure Rust, so the
same code runs on Windows, Linux and macOS: `.vol` archives (including LZH-compressed
entries), `.map` files, tilesets, `op2_art.prt` sprites, the stat sheets, sounds, the AVI
cutscenes, tables in `Outpost2.exe`, and the GOG and OPU installers.

## Where it comes from

This is the `op2-formats` crate of [op2-rs](https://gitlab.com/bufke/op2-rs), David Burke's
reimplementation of Outpost 2, written by David Burke and op2-rs's contributors. It was
imported unmodified from op2-rs commit `9798188` (see the git history) so that other Outpost 2
projects can use it on its own. All credit for the format work goes to them.

The code in `op2-formats/` is kept identical to upstream so updates can be brought over
cleanly; changes needed to build it here live outside that folder. Fixes to the readers
themselves should go to op2-rs too.

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

## Game data

No game data is included or extracted into this repository: you need your own copy of
Outpost 2. The one generated file, `op2-formats/src/units_table.rs`, is a table of sprite
animation numbers read from `Outpost2.exe`, not game art.

Tests that need the game find an install through `$OP2_DIR` (an Outpost 2 folder with OPU
1.4.1) and skip their real-data checks without one. `OP2_REQUIRE_DATA=1` turns a skip into a
failure.

## Leftovers from op2-rs

A few parts still assume op2-rs's repository layout. None affects normal use:

- `source_tree_game_dir` looks for op2-rs's Wine prefix next to its build folder; here it
  finds nothing and the normal install search is used.
- A cross-check in `tests/game_data.rs` (`rules_and_tech_trees_parse`) reads op2-rs's
  recorded oracle runs and skips when they aren't there. With `OP2_REQUIRE_DATA=1` that
  skip becomes a failure, so expect exactly this one test to fail in a real-data run; the
  rest (68 of 69 with `--all-features`) pass against an OPU 1.4.1 install.
- The installer tests (`inno_installer`, `opu_package`) need test installers named by their
  own variables (see `tests/inno_test.iss`) and pass without checking anything otherwise.
- `scripts/unit_anims/extract.sh`, which regenerates `units_table.rs`, expects op2-rs's
  `prefix/` and `vendor/TethysAPI` folders.
