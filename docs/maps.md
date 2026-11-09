# Maps (`.map`)

A `.map` file holds a map's **terrain only**: which tile each cell shows, how fast units cross it, which tilesets
it draws from and how terrain changes when built on. Units, resources, starting positions and triggers come from
the mission (a mission DLL, or an `.opm` for tools that use OP2MissionEditor). Read and written by op2-formats'
`map.rs`, which writes every shipped map back byte for byte.

Unless a section says otherwise, the numbers here were measured with op2-formats on the 89 maps of GOG Outpost 2
with OPU 1.4.1 (OPU ships them loose in `OPU/base/maps/`).

## Layout

All numbers are little-endian.

| Section | Size | Contents |
|---|---|---|
| Header | 20 | version tag, saved-game flag, width (log 2), height, tileset slot count |
| Tiles | width × height × 4 | one 32-bit word per cell, in 32-column strips (see [Tile order](#tile-order)) |
| Clip rect | 16 | x1, y1, x2, y2 (signed) |
| Tileset slots | varies | one entry per slot |
| `TILE SET` marker | 10 | `TILE SET`, 0x1A, 0x00 |
| Tile mappings | 4 + count × 8 | count, then the mappings |
| Terrain types | 4 + count × 264 | count, then the records |
| Version tag, twice | 8 | the header's version tag again, two times |
| Tile groups | varies | count, an unknown word, then the groups |

### Header

| Offset | Type | Field | Notes |
|---|---|---|---|
| 0 | u32 | version tag | 0x1011 in every shipped map. Below 0x1010 is refused (the game, 0x435E0B; op2-formats; OP2Utility) |
| 4 | i32 | saved game | 0 in a `.map`. Saved games (`.op2`) start with their own prefix and set this |
| 8 | u32 | width, log 2 | the width is `1 << value`: 64 to 512 in the shipped maps (6 to 9) |
| 12 | u32 | height | 64, 128 or 256 in the shipped maps |
| 16 | u32 | tileset slots | 512 in every shipped map |

Shipped sizes: 64×64 (11 maps), 64×128 (5), 64×256 (2), 128×64 (16), 128×128 (27), 128×256 (2), 256×64 (3),
256×128 (6), 256×256 (12) and 512×256 (5). Maps 512 tiles wide wrap around east to west ("around the world").

The game rounds the height up to a power of two, at least 2 (map setup 0x4354F0, at 0x435586-0x4355A1), and sizes
everything by the rounded height: it reads `width × rounded height` tile words from the file (0x435EA8-0x435ED9) and
lays out the strips with it (see [Tile order](#tile-order)). Every shipped height is already a power of two. A map
with another height must store its tiles for the rounded height; op2-formats reads `width × height` words, so it
would misread such a file.

### Tile words

Each cell is a 32-bit word:

| Bits | Field |
|---|---|
| 0-4 | cell type (see below) |
| 5-15 | tile mapping index (0-2047) into the map's tile mappings |
| 16-26 | unit index: the unit standing on the cell (game state) |
| 27 | lava |
| 28 | lava possible: lava may flow here |
| 29 | the Blight may spread from here |
| 30 | the Blight (microbe) |
| 31 | a wall or building stands here |

Bits 16-31 are the game's running state, saved in saved games. No shipped map sets any of them: every map's cells
are a cell type and a mapping index, with mapping indices up to 2011.

### Cell types

The 5-bit cell type decides movement and sight. The shipped maps use only 0-11; the game sets 21-31 as things are
built and destroyed. The names are the game's own, from its cell type table (`Outpost2.exe` 0x4DEBA8, read by
op2-formats' `Exe::cell_types`), spelling included. The descriptions come from the community notes; they are
**unconfirmed** as a whole. Rendering the 12 most used tiles of each type in the 63 maps under `OPU/base/maps` bears
out 0 (rock with green growth), 2 (dark grey rock), 5 (dark speckled rock), 6 (bumps and mounds), 7 (plain rock)
and 8-10 (cliff edges); 3 and 4 both show orange sand; type 1's most used tiles are plain rock, not craters, and no
map there uses type 11. A tile-by-tile survey of each type would settle them.

| Value | Name in the game | What it is |
|---|---|---|
| 0 | Fast Passible 1 | rock vegetation |
| 1 | Impassible 2 | meteor craters, cracks |
| 2 | Slow Passible 1 | dark lava rock |
| 3 | Slow Passible 2 | rippled dirt, lava rock bumps |
| 4 | Medium Passible 1 | dirt |
| 5 | Medium Passible 2 | lava rock |
| 6 | Impassible 1 | mounds, ice caps, volcanoes |
| 7 | Fast Passible 2 | rock |
| 8 | North Cliffs | |
| 9 | Cliffs - High side | |
| 10 | Cliffs - Low Side | |
| 11 | Vents and Fumaroles | |
| 12-20 | zPad 12 to zPad 20 | placeholders; the table still gives them speeds |
| 21 | Dozed Area | bulldozed ground |
| 22 | Rubble | |
| 23 | Normal Wall | |
| 24 | Microbe Wall | |
| 25 | Lava Wall | |
| 26-31 | Tube0 to Tube5 | tubes, and the ground under buildings |

How fast each track type crosses each cell type, and whether it blocks sight, is a table in `Outpost2.exe`
(op2-formats: `Exe::cell_types`), not in the map.

### Tile order

The tile words are not stored row by row. The map is cut into strips 32 columns wide; each strip is stored whole,
top row to bottom, 32 words per row, then the next strip. The word of cell (x, y) is at index

```
((x / 32) × height + y) × 32 + (x mod 32)
```

counting words from the start of the tile section. For a 32-wide map this is plain row order.

### Clip rect

Four signed 32-bit values: the part of the map that the view and units may reach, in the game's own coordinates
(see [Coordinates](#coordinates)).

- **Maps that don't wrap** (84): x1 = 32, y1 = 0, x2 = 32 + width − 1, y2 = height − 2 in all but three. `cm03`
  (32, 1, 287, 253), `cm04` (32, 1, 287, 254) and `eden08` (32, 3, 95, 126) narrow the top or bottom.
- **Wrapping maps** (all five: `fl01`, `mp6_01`, `newworld`, `survatw`, `survaxen`): x1 = −1 and x2 = 2147483647, so
  no east or west edge, with y1 of 1 or 2 and y2 = 254.

op2-mapjson leaves the clip rect out of a JSON map when it has the usual values above; for a wrapping map it treats
(0, 0, 511, height − 2) as usual, which no shipped map uses, so their JSON always carries the clip rect.

### Tileset slots

Each slot is a u32 name length, then the name (ASCII, no terminator) and a u32 tile count, or only the 0 length for
an empty slot. Every shipped map has 512 slots, the first 13 holding `well0000` to `well0012` and the rest empty. See
[tilesets.md](tilesets.md) for the wells.

### Tile mappings

A u32 count, then 8 bytes per mapping, each a u16:

| Field | Meaning |
|---|---|
| tileset | slot number |
| graphic | tile within that well |
| animation count | how many following tiles the mapping cycles through (0 = still) |
| animation delay | time between frames |

87 maps have the default table of 2017 mappings; the other two have 2012. The default table is described in
[tilesets.md](tilesets.md#how-a-map-uses-the-tiles).

### Terrain types

A u32 count (4 in every shipped map), then 264-byte records saying which mapping ranges form each terrain and which
mappings the game switches cells to for walls, tubes, rubble, bulldozing, scorch marks and lava. The record's layout
and the default values are in [tilesets.md](tilesets.md#terrain-types).

### Version tags

After the terrain types the header's version tag appears twice. op2-formats refuses a map whose tags don't match the
header's. In a saved game, unit data comes between the two tags (see below).

### Tile groups

At the end: a u32 group count, a u32 whose meaning is unknown, then each group:

| Type | Field |
|---|---|
| u32 | width in tiles |
| u32 | height in tiles |
| u32 × width × height | mapping indices, row by row |
| u32 + bytes | name length and name |

Tile groups are the terrain pieces a map editor offers (`BLUE`, `mudbas1`, ...); the game isn't known to use them.
87 maps carry the same 218 groups (sizes, mapping indices and order); two have none. The unknown word is 0 in 54 maps
and 217 in 35. In 11 of the 87 (`exiles_03`, `on4_01`, `on4_02`, `on6_01` and `opu_01`, `02`, `09`, `10`, `11`, `12`,
`15`) every group's name is stale bytes rather than text; op2-formats keeps the bytes as stored.

## Coordinates

The game addresses cells in its own coordinates, which differ from the file's on maps that don't wrap: the game
pads those maps with 32 columns on the west, so **game x = file x + 32**, game y = file y. On 512-wide maps game and
file coordinates are the same. Mission code and the clip rect use game coordinates (op2-formats: `Map::x_padding`;
measured against the running game on cm01 and newworld in op2-rs).

## Saved games

A saved game (`.op2`) embeds this layout after its own prefix (signature, preview, unit type table), with the
saved-game flag set, the full tile words (unit indices, lava, the Blight and walls included), no tile groups, and the
units and the rest of the game state after it. See [saved-games.md](saved-games.md).

## Making a map

- Width a power of two (the header can only store one); 64 to 512 is what the game ships. Make the height a power of
  two as well: the game rounds it up to one and expects that many rows of tiles (see [Header](#header)).
  op2-formats reads widths up to 1024 and heights from 1 to 1024; sizes no shipped map uses are untested in the game.
- Cell types 0-11 for the ground; the game derives the rest.
- Mapping indices that exist in the map's own mapping table, usually the default one.
- 512 tileset slots with the wells first, the default mappings and terrain types, and the version tag 0x1011 copied
  from a shipped map are the safe choice (op2-mapjson does this when converting JSON to a `.map` without a base map).

## Sources

- op2-formats `map.rs` (layout, tile order, cell types, coordinates); op2-mapjson `terrain.rs`.
- OP2Utility (OutpostUniverse): `MapReader`, `MapWriter`, `TerrainType.h`.
- An early disassembly-based description of the `.map` and `.op2` layouts (OP2 community notes): the tile word
  bits, the cell type list and the saved-game prefix.
- Disassembly of GOG's `Outpost2.exe` 1.2.0.7: the map loader (0x435DC0) and map setup (0x4354F0); the cell type
  table (0x4DEBA8).
- Measured: all 89 shipped maps of OPU 1.4.1, read with op2-formats; the most used tiles of each cell type, rendered.
