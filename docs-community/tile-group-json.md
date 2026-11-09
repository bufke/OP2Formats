# Tile group JSON

A tile group JSON file holds **one tile group**: a named rectangle of tile indices, the terrain pieces (rock
outcrops, craters, mesas, wall and tube sets) that a map editor offers for placing. It is a community format for map
and tileset tools, laid out like the [JSON maps](map-json.md); the game never reads it. Two tools use it:

- **OP2Mapper3** (the map editor) lists, previews and places tile groups, and exports a selected area of a map as
  one.
- **OP2TileSetTools** (the tileset editor) writes a tile group when it imports a graphic into a tileset, and imports
  a tile group's picture back into a tileset's tiles.

Nothing in OP2Formats reads or writes it yet. This document describes the format as those two tools write and read
it, and the open issue it still has.

The measurements below were made with op2-formats on the 89 maps of GOG Outpost 2 with OPU 1.4.1, and on a set of
218 tile group files exported from those maps' groups, one file per group. What the tools do comes from their
source (OP2Mapper3's `fTileGroups.vb`, `fTileSelect.vb` and `Commands.vb`; OP2TileSetTools' `fTileGroup.vb`,
`fImportGraphic.vb` and `fMain.vb`).

## Where tile groups come from

A `.map` ends with its tile groups ([maps.md](../docs/maps.md#tile-groups)): a group count, an unknown word, then for each
group its width, height, `width × height` mapping indices as 32-bit words row by row, and a length-prefixed name
(op2-formats `map.rs`, `TileGroup`). The game isn't known to use them; they are there for editors.

In the shipped maps:

- 87 maps carry the same 218 groups (same sizes, same indices, same order); `CK9_2v2ai` and `Swarm` have none.
- In 76 of those maps every group's name is text (`BLUE`, `mudbas1`, `rocvolc1`, `snowroc7`, ...). In the other 11
  (`exiles_03`, `on4_01`, `on4_02`, `on6_01` and seven `opu_` maps) all 218 names are stale bytes, not text; the
  names have to be taken from one of the 76.
- Names are 4 to 10 characters, all different.
- Sizes run from 1×1 (62 groups) to 16×6 and 10×10; no group has more than 100 cells, and none is wider than 16 or
  taller than 10.
- Every index is a mapping index of the default table, at most 2011. Seven groups are all zeros (tile 0, the blue
  marker): `BLUE` and six placeholders (`muddoz`, `mudtubes`, `rocdoz`, `roctubes`, `sandoz`, `santube`).

## The file

UTF-8 JSON, one group per file, named after the group (`snowroc7.json`). Two members:

| Member | Type | Contents |
|---|---|---|
| `header` | object | size, tileset, name and notes |
| `tiles` | array of rows | the group's mapping indices |

### `header`

| Member | Type | Contents |
|---|---|---|
| `width` | integer | width in tiles, at least 1 |
| `height` | integer | height in tiles, at least 1 |
| `tileset` | string | the tileset the group is drawn from, as its 6-character prefix: `well00` for the game's (see [tilesets.md](../docs/tilesets.md)) |
| `name` | string | the group's name, as stored in the `.map` |
| `bmp` | string | the file name of a picture of the group, `<name>.bmp` next to the `.json`; empty when there is none |
| `notes` | string | free text |

All six members are always written. OP2Mapper3 refuses a file that lacks any of them; OP2TileSetTools reads only
`tileset`, `notes`, `width` and `height`.

The tools fill them in this way:

| Member | OP2Mapper3 export | OP2TileSetTools import of a graphic | The 218 files from the shipped maps |
|---|---|---|---|
| `tileset` | the first 6 characters of the well file name of the first cell, `well00` if none | the target tileset's name | `well00` |
| `name` | the name typed at export | the name typed at import | the `.map`'s group name |
| `bmp` | `<name>.bmp` | `<name>.bmp` | empty |
| `notes` | `Exported from OP2Mapper` | `Imported from graphic using OP2TileSetTools` | empty |

The picture is a BMP of the group as drawn, 32 × 32 pixels per tile, written next to the `.json` by both tools.

### `tiles`

`height` arrays of `width` integers. Row `y`, element `x` is the mapping index of the group's cell (x, y), counted
from the group's top-left corner. Element `tiles[y][x]` is word `y × width + x` of the group in the `.map`, so a
group converts both ways without reordering. Mapping indices are 0 to 2047 (11 bits of a tile word), though a
`.map` stores each as a full 32-bit word.

An older form stored `tiles` as one flat array of `width × height` values, row by row. OP2TileSetTools still reads
it, filling missing values with 0; OP2Mapper3 refuses it. Write the per-row form.

## How the tools use the files

### OP2Mapper3

- **Where the files are.** OP2Mapper3 looks in a `Tilegroups` folder: by default next to the program, or the folder
  set in its settings. Each subfolder is a category (`Tilegroups\Greenworld\snowroc7.json`). Loose files in the
  folder itself are listed under a "(Default)" category. Exports go to `Tilegroups\Exported\`.
- **Listing.** Groups are listed by file name, not by `name`. The file name is also the name shown in the undo
  history.
- **Reading.** The size comes from `tiles`: the number of rows, and the length of the first row. When the header's
  `width` and `height` disagree, `tiles` wins. Every row must be as long as the first. A file that is unreadable,
  lacks a member, or has a ragged or non-numeric row is refused with a message.
- **Preview and placing.** The values are read as mapping indices of the open map. Each cell is drawn through that
  map's mapping table, and a value outside the table is left blank. Placing a group writes the values into the map's
  cells as they are, and is refused when the group would run past the map's edge.
- **Export.** The tool writes the copied area's mapping indices, the header shown above, and the BMP picture.

### OP2TileSetTools

- **Import of a graphic.** The graphic is cut into tiles, which take the first free run of tile numbers in the
  tileset. The tool writes `<working folder>\<tileset>\tilegroups\<name>.json` and `<name>.bmp`. Its `tiles` are
  those tile numbers in order: a group placed at tile number 500 that is 3 wide holds 500, 501, 502 in its first
  row.
- **Import of a tile group.** The tool opens a `.json` with its picture. It looks for `<file name>.bmp` in the same
  folder, or else the first `.bmp` there; it doesn't read the `bmp` member. From an area selected on the picture,
  each 32 × 32 piece replaces the tileset tile whose number is in that cell of `tiles`.
- **Tile numbers.** OP2TileSetTools reads a value as a **tile number** counted through the tileset's wells in order:
  `well0000`'s tiles first, then `well0001`'s, and so on. It doesn't read it as a mapping index.

## Text layout

The same layout as a PerRowPadded JSON map ([map-json.md](map-json.md#text-layout)):

- Two spaces per indent level, `"name": value` with one space after the colon, CRLF line ends, no line break after
  the final `}`.
- `header`'s members one per line, in the order above.
- Each row of `tiles` on one line, `[ `, the numbers joined by `, `, ` ]`, each number right-aligned to the widest
  in its row and at least 4 characters.

For a made-up 3×2 group:

```
{
  "header": {
    "width": 3,
    "height": 2,
    "tileset": "well00",
    "name": "example",
    "bmp": "",
    "notes": ""
  },
  "tiles": [
    [  100,  101,  102 ],
    [  103,  104,  105 ]
  ]
}
```

Measured: all 218 exported files follow these rules byte for byte (rebuilt from the groups of `cm01.map`), their
tiles equal the map's groups, every `tileset` is `well00`, and every `bmp` and `notes` is empty.

## Open issue: what a value means

The two tools read a value differently. OP2Mapper3 reads it as a mapping index of the open map. OP2TileSetTools
reads it as a tile number counted through the wells. The two readings agree for the game's tileset with the
default mapping table: there, mapping index *i* is tile number *i* for every *i* below 2012, the tileset's tile count.
Mappings 2012 to 2016 are extras that point back at earlier tiles; no group uses them. This was measured on
`eden03.map`'s 2017 mappings and 13 wells, which hold 1, 269, 163, 6, 359, 147, 54, 207, 347, 141, 96, 150 and 72
tiles. For a map whose mapping table isn't in that order, a group from OP2TileSetTools places the wrong tiles in
OP2Mapper3.

The rest of this section reads the values as OP2Mapper3 does, as **tile mapping indices**, not tiles. A mapping index means something only together with a
map's tile mapping table, which says for each index which well and which tile in it to draw, and whether it is
animated ([tilesets.md](../docs/tilesets.md#how-a-map-uses-the-tiles)). The same index draws a different tile in a map
whose mappings differ.

The `tileset` member doesn't capture this. It names a tileset (the wells), not a mapping table, and two maps with
the same wells can number their tiles differently. In practice every exported group comes from maps with the
default table of 2017 mappings, so the files are correct for any map that keeps the default table (87 of the 89
shipped maps, and every map made from a shipped one). For a map with other mappings (`CK9_2v2ai`, `Swarm`, or a
custom tileset with its own numbering) the indices would place the wrong tiles, and nothing in the file says so.

Until the format settles this, a tool should treat the indices as default-table indices, and check them against
the target map before placing a group: each must be below the map's mapping count, and the map's mappings for them
should match the default table. Ways the format could carry the meaning, not yet decided: name the mapping table
(e.g. "default"), store the well and tile of each cell (`tilesetIndex`, `tileGraphicIndex`, as a JSON map's
`tileMappings` do) instead of an index, or embed the mappings the group uses.

## Reading and writing one

Checks a reader should make:

- `header` has all six members, as OP2Mapper3 requires.
- `tiles` is per-row, with `height` rows of `width` values, and the header's size matches. When they don't,
  OP2Mapper3 goes by `tiles`.
- Every value is 0 to 2047, and below the mapping count of the map it will be placed on (see above).

To a `.map` tile group: width, height, the indices row by row as 32-bit words, then the name's length and bytes.
`tileset`, `bmp` and `notes` have no place in the `.map` and are dropped. From a `.map`: take the name from a map
whose names are text, and write `well00` as the tileset only if the map names the `well00` wells.

The `.map`'s unknown word before the groups (0 or 217 in the shipped maps) belongs to the map, not to a group, and
isn't carried.

## Sources

- op2-formats `map.rs` (`TileGroup`, the tile group section of a `.map`).
- Leviathan's draft of the tile group JSON format (members, file naming, the per-row layout).
- op2-mapjson `write.rs` (the PerRowPadded layout the files share).
- OP2Mapper3's source: `fTileGroups.vb` (folders, reading, preview), `fTileSelect.vb` (export), `Commands.vb`
  (placing).
- OP2TileSetTools' source: `fImportGraphic.vb` (a group from an imported graphic), `fTileGroup.vb` (importing a
  group's picture), `fMain.vb` (`ApplyTileReplacements`, values read as tile numbers through the wells).
- Measured: `eden03.map`'s mapping table against its wells' tile counts (mapping *i* is tile *i* below 2012).
- Measured: the tile groups of all 89 shipped maps of OPU 1.4.1, read with op2-formats; the 218 exported tile group
  files, compared with `cm01.map`'s groups and rebuilt byte for byte.
