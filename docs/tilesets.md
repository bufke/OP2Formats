# Tilesets

The ground of an Outpost 2 map is drawn from 32x32 tiles kept in a **tileset**: 13 image files called *wells*,
each a vertical strip of tiles. A map doesn't point at tiles directly; it points at *tile mappings*, which the
`.map` file carries (see [How a map uses the tiles](#how-a-map-uses-the-tiles)). Read by op2-formats' `tileset.rs`;
the map side is in `map.rs`.

Unless a section says otherwise, the numbers here were measured with op2-formats on GOG Outpost 2 with OPU 1.4.1.

## The well files

The game's tileset is named `well00`; its wells are `well0000` to `well0012`. A map names each well in full (8
characters), so a custom tileset is a 6-character prefix plus the well number, e.g. `grnwld00` to `grnwld12`.

| Well | File | Tiles | Global indices (default mappings) |
|---|---|---|---|
| 0 | `well0000.bmp` | 1 | 0 |
| 1 | `well0001.bmp` | 269 | 1-269 |
| 2 | `well0002.bmp` | 163 | 270-432 |
| 3 | `well0003.bmp` | 6 | 433-438 |
| 4 | `well0004.bmp` | 359 | 439-797 |
| 5 | `well0005.bmp` | 147 | 798-944 |
| 6 | `well0006.bmp` | 54 | 945-998 |
| 7 | `well0007.bmp` | 207 | 999-1205 |
| 8 | `well0008.bmp` | 347 | 1206-1552 |
| 9 | `well0009.bmp` | 141 | 1553-1693 |
| 10 | `well0010.bmp` | 96 | 1694-1789 |
| 11 | `well0011.bmp` | 150 | 1790-1939 |
| 12 | `well0012.bmp` | 72 | 1940-2011 |
| | | **2012** | |

Each map also stores every well's tile count, in its tileset slots (`numTiles`). A well is 32 pixels wide and `tiles × 32` tall: `well0001` is 32x8608.

## File encodings

A well is 8-bit indexed colour with its own 256-colour palette, in one of two containers:

- **Windows BMP**, as OPU ships them loose in `OPU/base/tilesets/`: a `BITMAPINFOHEADER` (40 bytes), 256 palette
  entries in BGRx order, then one byte per pixel. OPU's wells are **top-down** (negative height, e.g. `well0001` has
  height -8608). BMP also allows bottom-up (positive height); a reader must follow the sign. Rows of 32 pixels need
  no padding.
- **PBMP**, the game's own container, inside the original `.vol` archives: tagged sections (`PBMP`, `head`, ...),
  the palette with red and blue swapped relative to BMP, pixels top-down. OPU's installer converts these to the BMPs
  above (op2-formats: `tileset::pbmp_to_bmp`).

The game looks up a well by its name through the usual search (OPU's loose files first, then the `.vol`s), so a
custom tileset can be loose BMPs in `OPU/base/tilesets/`.

## How a map uses the tiles

Each map cell holds a **tile mapping index** (11 bits of the cell's 32-bit word; 5 more bits are its cell type).
The `.map` file then lists:

- **Tileset slots**: well names and their tile counts.
- **Tile mappings**: for each mapping index, a slot, a tile within that well, and an animation (how many following
  tiles to cycle through, and the delay between them).
- **Terrain types**: which mapping-index ranges form a terrain, and which mappings the game switches a cell to when
  it is bulldozed, walled, tubed, turned to rubble or scorched (264 bytes each, see below).

So the *meaning* of an index belongs to the map, not the tileset. The "global indices" in this document are mapping
indices under the **default mapping table**, which 87 of the 89 shipped maps use (the two that differ are
`CK9_2v2ai.map` and `Swarm.map`):

- 2017 mappings. Mappings 0-2011 are the identity: mapping *k* is the *k*-th tile counting through the wells in
  order (the table above). Mappings 2012-2016 all point at tile 0 of `well0000`.
- 25 mappings are animated, all in `well0007` (see [Lava](#lava-and-other-animated-tiles)).

A custom tileset that keeps the default mappings can reuse every shipped map's layout and the special tiles below.

## Terrain types

The default table has four terrain types. Their ranges, from the maps' terrain type records:

| Terrain | In `well00` | Mapping range |
|---|---|---|
| 0 | sand | 1-438 |
| 1 | rock | 439-1205 |
| 2 | orange (mud) | 1206-1789 |
| 3 | none in use | 1790-2011 |

Terrain 3 has no special tiles: its record holds zeros and small placeholder values.

### Special tiles

When something happens to a cell, the game replaces its mapping with one taken from the terrain type the cell's
mapping falls in. With the default table:

| Purpose | Terrain 0 (sand) | Terrain 1 (rock) | Terrain 2 (orange) | Count |
|---|---|---|---|---|
| Scorch marks (explosions, meteors) | 264-269 | 789-797 | 1547-1552 | 6 / 9 / 6 |
| Lava wall | 270-285 | 798-813 | 1553-1568 | 16 |
| Microbe wall | 286-301 | 814-829 | 1569-1584 | 16 |
| Rubble: 4 common, then 4 rare | 323-330 | 830-837 | 1585-1592 | 8 |
| Normal wall | 331-346 | 838-853 | 1593-1608 | 16 |
| Damaged wall | 347-362 | 854-869 | 1609-1624 | 16 |
| Heavily damaged wall | 363-378 | 870-885 | 1625-1640 | 16 |
| Bulldozed | 409 | 921 | 1670 | 1 |
| Tubes | 417-432 | 929-944 | 1678-1693 | 16 |
| Lava | none | 999 | none | |

The record gives the first scorch mark; the counts are the scorch-mark tiles drawn after it. The game uses six of
them (0x48B7C0): a scorched cell gets `scorch + random(0-2)` if its mapping is a bulldozed mapping (or 10), and
`scorch + 3 + random(0-2)` otherwise, so terrain 1's last three (795-797) are never chosen. Only cells whose mapping
lies in one of the terrain records' scorchable ranges are scorched (0x48BBB0, called at 0x43DAC5 and 0x44A992). Each
set of 16 wall or tube tiles is ordered by which neighbours connect (Outpost2.exe's tube graphic table, op2-formats
`Exe::tube_graphics`).

The 7 tiles after each bulldozed tile (410-416, 922-928, 1671-1677) look like more bulldozed ground, but the game
never picks them. Every place that bulldozes a cell (8 calls of 0x48B770, among them 0x436D81, 0x43B46E, 0x483206
and 0x4A271A) writes the record's one bulldozed mapping and nothing after it, and the scorch code treats only that
mapping as bulldozed. The ranges 409-416, 921-928 and 1670-1677 in the records are scorchable ranges, so a map that
paints one of the 7 tiles gets it scorched like natural ground. None of the 118 `.map` files under `OPU/` uses them.
They are free for other uses only as far as the game is concerned; a map editor may still offer them.

### The terrain type record

264 bytes, all 16-bit values; names follow OP2Utility's `TerrainType.h`:

| Bytes | Field |
|---|---|
| 4 | tile mapping range: start, end |
| 2 | first bulldozed mapping |
| 2 | first rubble mapping (4 common, then 4 rare) |
| 12 | tube mappings (6) |
| 160 | wall mappings: 5 sets of 16 (lava, microbe, normal, damaged, heavily damaged) |
| 2 | lava mapping |
| 6 | flat1, flat2, flat3: how lava mappings are laid out (below); terrain 1 has 6, 6, 1 |
| 32 | tube mappings (16) |
| 2 | scorched mapping |
| 12 | scorchable ranges: 3 pairs of start and end, a start of 0 meaning unused |
| 30 | unknown (15 values, all 0 in the default table) |

The flat fields describe the lava tiles. When the game sets a cell's lava tile (0x43286B-0x4328E1), it takes the lava
mapping of the cell's terrain and the four-neighbour pattern of lava around the cell (0-15). With the pattern below
15 the mapping is `lava + flat1 × pattern`; with lava on all four sides it is `lava + 15 × flat1 + random(0 to
flat2 − 2) × flat3`. The flat values come from the first terrain record with a lava mapping (0x48BA50, 0x48BA90,
0x48BAD0). With the defaults this gives the animated edge mappings 999, 1005, ... 1083 (six apart) and, inside a lava
field, one of the still tiles 1089-1093. No code was found that reads the 15 trailing words: the terrain-type
routines (0x48B770-0x48BC50) read nothing past the scorchable ranges. What they were for is not known.

## Lava and other animated tiles

All of `well0007` (999-1205) is lava: still lava and the frames of flowing-lava animations. Terrain 1's lava
mapping is 999. The 25 animated mappings are 999, 1005, ... 1083 (5 following tiles each), then 1095, 1107, 1118,
1129, 1140, 1150, 1161, 1172, 1184 and 1195 (9 or 10 each); each cycles through the tiles after it.

## Free tiles for a custom tileset

A custom tileset that keeps the default mappings may redraw any tile, but the special tiles above must keep their
roles, tile 0 (a blue marker) stays, and `well0007` must stay lava. Counting only the special tiles in the table above
as taken:

| Well | Tiles | Special | Free | Free global indices |
|---|---|---|---|---|
| 0 | 1 | 1 (tile 0) | 0 | |
| 1 | 269 | 6 | 263 | 1-263 |
| 2 | 163 | 105 | 58 | 302-322, 379-408, 410-416* |
| 3 | 6 | 0 | 6 | 433-438 |
| 4 | 359 | 9 | 350 | 439-788 |
| 5 | 147 | 105 | 42 | 886-920, 922-928* |
| 6 | 54 | 0 | 54 | 945-998 |
| 7 | 207 | 207 (lava) | 0 | |
| 8 | 347 | 6 | 341 | 1206-1546 |
| 9 | 141 | 105 | 36 | 1641-1669, 1671-1677* |
| 10 | 96 | 0 | 96 | 1694-1789 |
| 11 | 150 | 0 | 150 | 1790-1939 |
| 12 | 72 | 0 | 72 | 1940-2011 |
| | **2012** | **544** | **1468** | |

\* The 21 tiles after the bulldozed ones: the game never places them (see above), but they are scorchable ground.

Other things to keep: tiles of the same terrain belong to the same mapping range, so a tile moved into another
terrain's range gets that terrain's walls, tubes and rubble when built on. Tile groups (the shapes a map editor
places) refer to mapping indices too.

## Making a tileset

1. 13 wells with the tile counts above, named with a 6-character prefix plus `00` to `12`.
2. Each well 32 pixels wide, 8-bit indexed, at most 256 colours (each well has its own palette, so a tileset can use
   up to 13 × 256 colours).
3. Save as uncompressed BMP like OPU's (top-down is what the game's own files use).
4. Keep the special tiles and the lava well in place if the maps should keep using the default mappings.
5. A map uses the tileset by naming its wells in its tileset slots.

## Sources

- op2-formats `tileset.rs`, `map.rs`; op2-mapjson `terrain.rs`.
- OP2Utility (OutpostUniverse): `.map` and `TerrainType` layouts.
- Leviathan's OP2TileSetTools notes (tile allocation by well).
- Disassembly of GOG's `Outpost2.exe` 1.2.0.7: the terrain-type routines (0x48B770-0x48BC50), their callers for
  bulldozing and scorching, and the lava placement (0x43286B).
- Measured: wells, mappings and terrain types of OPU 1.4.1's files with op2-formats, and the tiles drawn.
