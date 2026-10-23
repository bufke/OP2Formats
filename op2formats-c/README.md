# op2formats-c

A C interface to OP2Formats, so C, C++ and anything else that can call a C library can read Outpost 2's files.
The header is [`include/op2formats.h`](include/op2formats.h); its top comment describes the conventions
(opaque handles, status codes, `op2f_last_error`, strings copied into caller buffers).

It covers all of op2-formats, one source file per area:

| Area | Functions | File |
|---|---|---|
| Game folder | `op2f_game_open` (a folder, or `$OP2_DIR` / an install it finds), read any file by name, where it came from, loose paths, map names, `.vol` list, the default game folder | `game.rs` |
| Installs | check a folder (release, OPU version, problems), search the usual places, the data folders, the OPU folder | `install.rs` |
| Maps | open, parse or write; size, wrapping and x padding, clip rect, tiles (read and edit), tileset slots, tile mappings, tile groups, terrain types, cell speeds | `map.rs`, `render.rs` |
| Drawing maps | a map's tilesets loaded; the whole map at 1-32 px per tile, tile groups, one tile, a mapping's minimap colour; the GPU tile palette (texture-array layers and an overview image); tileset strips; PBMP to BMP | `render.rs` |
| Sheets | any sheet (`vehicles.txt`, ...) or keyed sheet (`morale.txt`) by row and column | `rules.rs`, `stats.rs` |
| Rules | the sheets decoded as the game loads them: vehicle, weapon, structure, space and mine stats; armor values; where each sheet came from | `stats.rs` |
| Tech trees | topics with costs, requirements, texts and the stat changes they make | `stats.rs` |
| UI strings | OPU's `language.ini`, by string id; the ids of unit, structure and weapon names and armor words | `stats.rs` |
| Sprites | images and animation frames drawn as RGBA in any player colour, with hotspots; palettes, raw pixels, frame layers, bounds, clip rectangles; the game's click test on a frame | `sprites.rs`, `art.rs` |
| Unit art | which animation the game draws for a vehicle (state, facing), a structure (slot) or a turret; turret and muzzle offsets; vehicles and tanks drawn whole | `sprites.rs`, `unit_art.rs` |
| Picking | what a click selects or targets among units, as the game decides | `unit_art.rs` |
| Images | made from pixels, shrunk, mip chains, one drawn over another | `common.rs` |
| `.vol` archives | entries, their storage, reading them stored or decoded; LZH | `media.rs` |
| Sound | `op2.clm` tracks as PCM or `.wav`; `.wav` formats | `sound.rs` |
| Cutscenes | AVI frames decoded (Cinepak) in any order, the audio decoded (MS ADPCM), raw streams | `video.rs` |
| `Outpost2.exe` | the tables read from it: sound files, Savant lengths, the ambient animation sequence, tube graphics; any value or string | `exe.rs` |
| Windows resources | the game's DLLs: bitmaps (with palettes), cursors and icons, string tables, dialogs, menus, version strings; `.bmp`, `.cur`, `.ico` and `.ani` files | `resources.rs` |
| Installers | GOG's offline installer (feature `inno`), OPU's installer and its install (`opu`), downloading OPU (`fetch-opu`) | `installers.rs` |
| Lists | string lists, tables and pairs the other calls return | `common.rs` |

The Blight's tile masks (`virmask.raw`) are in `render.rs`. Long work (extracting, installing, downloading) reports
through an `Op2fProgress` callback.

## Features

`inno` and `opu` are on by default; `fetch-opu` (an HTTPS client to download OPU) is off. A function whose feature a
build leaves out stays in the library and returns `OP2F_STATUS_UNSUPPORTED`, so every build has the same header.

```
cargo build -p op2formats-c --release --features fetch-opu
```

## Using it from CMake

The repository's top-level `CMakeLists.txt` runs Cargo for you and defines imported targets. Either build it as part
of your project:

```cmake
add_subdirectory(path/to/OP2Formats)
target_link_libraries(app PRIVATE OP2Formats::OP2Formats)
op2formats_copy_runtime(app)   # copies op2formats.dll next to app.exe after each build
```

or install it once and find it:

```
cmake -S OP2Formats -B build -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build build
cmake --install build --prefix C:/libs/OP2Formats
```

```cmake
find_package(OP2Formats 0.1 REQUIRED)   # with CMAKE_PREFIX_PATH including C:/libs/OP2Formats
target_link_libraries(app PRIVATE OP2Formats::OP2Formats)
op2formats_copy_runtime(app)
```

`OP2Formats::OP2Formats` is the DLL / shared library; set `OP2FORMATS_LINK_STATIC=ON` to link the static library
instead (`OP2Formats::shared` and `OP2Formats::static` name each directly). Other options: `OP2FORMATS_FEATURES`
(extra Cargo features, e.g. `fetch-opu`), `OP2FORMATS_RUST_TARGET` (a MinGW compiler gets `x86_64-pc-windows-gnu`
by default), `OP2FORMATS_BUILD_EXAMPLES`. Cargo has to be installed (https://rustup.rs); `rust-toolchain.toml` picks
the Rust version. The Rust library is always an optimised build, for Debug and Release programs alike.

Checked on Windows with MSVC 2022 (CMake 3.31): `add_subdirectory` with Ninja (Debug and Release, shared and
static) and with the Visual Studio generator, and `cmake --install` then `find_package`. The MinGW, Linux and macOS
branches are written but untried.

## Building by hand

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
native-static-libs` lists them): on Windows `op2formats.lib kernel32.lib ntdll.lib userenv.lib ws2_32.lib
dbghelp.lib`.

Checked so far on Windows with MSVC (Visual Studio 2022 Build Tools), as a DLL and statically, from C++ and C.
MinGW (`--target x86_64-pc-windows-gnu`), Linux and macOS are expected to work but haven't been tried.

## Example

[`examples/cpp/main.cpp`](examples/cpp/main.cpp) opens the game, prints a map's size and tilesets, decodes the
top-left tile's colour, prints the Scout's row from `vehicles.txt` and saves two sprites as TGA files: a red Lynx
with a laser and a Plymouth Command Center:

```
op2formats_example ["C:\Games\Outpost 2"] [cm01.map]
```

Building OP2Formats with CMake builds it too (`op2formats-c/examples/cpp/op2formats_example.exe` in the build
folder); its own `CMakeLists.txt` shows a project using an installed copy.

## Changing the interface

Edit `src/lib.rs`, then regenerate the header and commit both:

```
cargo install cbindgen
cd op2formats-c
cbindgen --config cbindgen.toml --output include/op2formats.h
```

`tests/c_api.rs` and `tests/all_areas.rs` call the functions the way C does, every area against real game files
where it can, comparing with another route to the same data (a map written back is the file it came from, a frame
decoded straight away is the one decoded in order, a hand-built tank is the drawn one). They need `OP2_DIR` (an
Outpost 2 install with OPU 1.4.1) and skip without it; `OP2_REQUIRE_DATA=1` makes the skip a failure. The installer
functions have no real-data test: no installer files are at hand.

## License

GPL-3.0-or-later, like the rest of OP2Formats: a program linking this library, dynamically or statically, must be
distributed under the GPL.
