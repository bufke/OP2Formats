# op2formats-c

A C interface to OP2Formats, so C, C++ and anything else that can call a C library can read Outpost 2's files.
The header is [`include/op2formats.h`](include/op2formats.h); its top comment describes the conventions
(opaque handles, status codes, `op2f_last_error`, strings copied into caller buffers).

What it covers so far:

| Area | Functions |
|---|---|
| Game folder | `op2f_game_open` (a folder, or `$OP2_DIR` / an install it finds), `op2f_game_read` any file by name, `op2f_game_source`, map names |
| Maps | open or parse, size, wrapping and x padding, clip rect, single tiles or all tile words, tileset slots, tile mappings |
| Tilesets | open by name or parse a BMP/PBMP, tile count, one tile as RGBA |
| Sheets | open (`vehicles.txt`, `weapons.txt`, ...) or parse, column names, rows, cells, find a row by code name |
| `.vol` archives | open or parse, entry names, find, read an entry (LZH entries decompressed) |
| Sprites | load `op2_art.prt`; image and animation info; draw an image or an animation frame as RGBA in any player colour, with its hotspot; the game's click test on a frame |
| Unit art | which animation the game draws for a vehicle (by state and facing) or a structure (by slot); a tank drawn with its turret |

Sounds, strings, the cutscenes and the rest of op2-formats aren't exposed yet.

## Building

```
cargo build -p op2formats-c --release
```

This produces, in `target/release`:

- Windows (MSVC): `op2formats.dll` with its import library `op2formats.dll.lib`, and the static library
  `op2formats.lib`.
- Linux: `libop2formats.so` and `libop2formats.a`; macOS: `libop2formats.dylib` and `libop2formats.a`.

Add `op2formats-c/include` to your include path and link one of them. With MSVC, for example:

```
cl /std:c++20 /EHsc /MD /I op2formats-c\include main.cpp /link target\release\op2formats.dll.lib
```

and ship `op2formats.dll` next to the program. Linking the static library instead needs the system libraries Rust's
standard library uses (`cargo rustc -p op2formats-c --release --lib --crate-type staticlib -- --print
native-static-libs` lists them): on Windows `op2formats.lib kernel32.lib ntdll.lib userenv.lib ws2_32.lib dbghelp.lib
advapi32.lib`. Build your program with the same C runtime (`/MD`).

Checked so far on Windows with MSVC (Visual Studio 2022 Build Tools), as a DLL and statically, from C++ and C.
MinGW (`--target x86_64-pc-windows-gnu`), Linux and macOS are expected to work but haven't been tried.

## Example

[`examples/cpp/main.cpp`](examples/cpp/main.cpp) opens the game, prints a map's size and tilesets, decodes the
top-left tile's colour, prints the Scout's row from `vehicles.txt` and saves two sprites as TGA files: a red Lynx
with a laser and a Plymouth Command Center:

```
op2formats_example ["C:\Games\Outpost 2"] [cm01.map]
```

## Changing the interface

Edit `src/lib.rs`, then regenerate the header and commit both:

```
cargo install cbindgen
cd op2formats-c
cbindgen --config cbindgen.toml --output include/op2formats.h
```

`tests/c_api.rs` calls the functions the way C does. Its game-data test needs `OP2_DIR` (an Outpost 2 install with
OPU 1.4.1) and skips without it; `OP2_REQUIRE_DATA=1` makes the skip a failure.

## License

GPL-3.0-or-later, like the rest of OP2Formats: a program linking this library, dynamically or statically, must be
distributed under the GPL.
