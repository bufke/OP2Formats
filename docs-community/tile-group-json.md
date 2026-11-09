# Tile group JSON (proposed)

A tile group JSON file holds **one tile group**: a named rectangle of tile mapping indices, the terrain pieces
(rock outcrops, craters, mesas, wall and tube sets) that a map editor offers for placing. It is a proposed format
for map tools, modelled on the [JSON maps](map-json.md); the game never reads it. Nothing in OP2Formats reads or
writes it yet. This document describes the format as drafted and as the one existing set of files has it, and
states the open issue it still has.

The measurements below were made with op2-formats on the 89 maps of GOG Outpost 2 with OPU 1.4.1, and on a set of
218 tile group files exported from those maps' groups, one file per group.

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
| `bmp` | string | reserved; meant for a picture of the group; empty in every file |
| `notes` | string | free text |

### `tiles`

`height` arrays of `width` integers. Row `y`, element `x` is the mapping index of the group's cell (x, y), counted
from the group's top-left corner. Element `tiles[y][x]` is word `y × width + x` of the group in the `.map`, so a
group converts both ways without reordering. Mapping indices are 0 to 2047 (11 bits of a tile word), though a
`.map` stores each as a full 32-bit word.

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

## Open issue: mapping indices need a mapping table

The values in `tiles` are **tile mapping indices**, not tiles. A mapping index means something only together with a
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

- `tiles` has `height` rows of `width` values, and the header's size matches.
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
- Measured: the tile groups of all 89 shipped maps of OPU 1.4.1, read with op2-formats; the 218 exported tile group
  files, compared with `cm01.map`'s groups and rebuilt byte for byte.
