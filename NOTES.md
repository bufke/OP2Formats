# Working notes

State and plan for OP2Formats. Newest first; keep it short and current.

## Where it stands (2026-10-23)

- `op2-formats/` is an unmodified copy of op2-rs's `tools/crates/op2-formats` at commit
  `9798188` (commit `1705db3` here). It stays byte-identical to upstream; anything this repo
  needs lives outside that folder (root `Cargo.toml` workspace, `rust-toolchain.toml`,
  README, CI).
- Builds and tests on Windows (MSVC, Rust 1.99). With an OPU 1.4.1 install as `OP2_DIR` and
  `OP2_REQUIRE_DATA=1`, 68 of 69 tests pass with `--all-features`; the failure is
  `rules_and_tech_trees_parse`'s cross-check against op2-rs's recorded oracle runs, which
  only exist in op2-rs.
- Not yet built on Linux or macOS: CI is set up for both, but the free plan's Actions minutes
  for private repos are used up.

## Next steps

1. **C interface** (`op2formats-c`): a cdylib with a cbindgen-generated header, so C, C++, C#
   and other projects can use the library. Opaque handles, integer error codes, no panics
   crossing the boundary, strings returned into caller buffers (the shape of OP2UtilityC).
   Start with what a game or tool needs first: open a game folder, read a map (size, cell
   types, tile graphics, tileset names), read the sheets. Then tilesets, sprites
   (`op2_art.prt`) and `.vol` entries. Check it from C++ with MSVC and MinGW.
2. **CI**: run it on a self-hosted runner, or switch the trigger to `workflow_dispatch` until
   minutes are available, or make the repo public (public repos get standard runners free).
3. **Upstream**: tell bufke about this repo; send reader fixes to op2-rs as merge requests;
   to take in upstream changes, copy the folder again from a newer op2-rs commit and update
   the commit named in the README.
4. **Name**: the crate is still `op2-formats`. Decide before publishing anywhere (crates.io
   needs a unique name; `outpost2-formats` was one idea).

## License note

GPL-3.0-or-later, as upstream. Any program that links OP2Formats (including through the C
interface) must be distributed under the GPL. A permissive license would need the agreement
of op2-formats' contributors: bufke (nearly all of it), Leviathan and johnathonfox.
