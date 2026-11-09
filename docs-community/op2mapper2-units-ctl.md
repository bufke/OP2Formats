# OP2Mapper2 unit table (`units.ctl`)

OP2Mapper2 is the classic Outpost 2 map editor, written in Visual Basic 6 by BlackBox and Hooman (2.0 alpha released
in 2005; the source read here is marked 2.2.4 in its project file). `units.ctl`, in the mapper's own folder, lists the
player units it can place: structures and vehicles, with their footprints, tube connections and the picture it draws
for each. The game never reads it.

The mapper reads three tables at start-up, in this order: `units.ctl`, then
[`weapons.ctl`](op2mapper2-weapons-ctl.md), then [`objects.ctl`](op2mapper2-objects-ctl.md). Units and objects go
into one definition list, units first; placements saved in a [`.map.dat`](op2mapper2-map-dat.md) refer to its
entries by `map_id`.

## Line syntax

The three tables share one syntax. Each line is handled on its own:

1. Leading and trailing spaces are trimmed, then every tab is removed.
2. A line starting with `;` is a comment.
3. A line starting with `INCLUDE` (any case) reads another file with the same parser. The name is the text between
   the first pair of double quotes, taken relative to **the mapper's folder**, not to the including file.
4. An empty line is skipped.
5. Anything else is a record: comma-separated fields. Field 1 (counting from 0) is a string in double quotes; it
   can't hold a double quote. Every other field is an integer made of the digits and `-`. A space outside the
   quotes, a quote outside field 1 or any other character is an error that stops reading the file; so is a field
   that doesn't convert to a number (an empty field, `1-2`) or doesn't fit its type. Fields past the last one the
   table knows are ignored, whatever they hold.

The records read before an error stay in the list. The shipped files use CRLF line ends and end with a comment line
without a line break, which the files say works around a parser bug.

## Records

```
map_id,"name",art                                   a vehicle
map_id,"name",width,height,art                      a structure without tubes
map_id,"name",width,height,tube_x,tube_y,art        a structure with tubes
```

| Field | Type | Meaning |
|---|---|---|
| map_id | 32-bit | the game's unit type id (the mission SDK's `map_id`), e.g. 1 Cargo Truck, 28 Command Center |
| name | text | the name shown in the unit list |
| width, height | 16-bit | footprint in tiles |
| tube_x, tube_y | 16-bit | which column and which row of the footprint (counting from 1) the tubes leave from |
| art | 32-bit | the animation the mapper draws, an index into `op2_art.prt` (see [sprites.md](../docs/sprites.md)) |

The parser decides the kind by the number of fields: 3 is a vehicle, 5 a structure, 7 a structure with tubes. With
4 or 6 fields the entry is a structure with no art (−1), which isn't drawn.

**Placing.** A vehicle sits on the clicked tile. A structure without tubes has its top-left tile on the clicked
tile. A structure with tubes is placed by its tube connection: its top-left tile is tube_x − 1 columns left and
tube_y − 1 rows up from the clicked tile, so the clicked tile is the footprint's tube column and tube row. The mapper
draws the tubes as two short white lines: one leaving the east side on the tube row, one leaving the south side on
the tube column. The file's own comment puts it as "an X, Y offset from the edge of the bulldozed border": the
bulldozed border is one tile wider on each side, so tube_x is also the tube column's distance from the border's west
edge.

## The shipped file

53 records: 38 structures (30 with tubes) and 15 vehicles. Measured against the game:

- Every `map_id` is the one the mission SDK's `map_id` enumeration gives that name.
- All 38 footprints equal the X-size and Y-size of the structure's row in the game's `building.txt` (OPU 1.4.1's
  `OPU/base/sheets`; the Command Center is 3 × 2). The tube offsets were not checked against the game.
- Art, against the animation tables in `Outpost2.exe` (op2-formats `UnitArt`): each vehicle's art is in its Eden
  animation table (the first state, facing east, for 14 of the 15), and 35 of the 38 structures' art is the first
  animation slot of the Eden version of that building (its construction animation; the mapper draws its last frame,
  the finished building). The other three, Guard Post (441), Common Ore Mine (104) and Rare Ore Mine (405), aren't
  in those tables; the file itself says the Guard Post has no turret-less art and borrows another picture.

Which units take a turret isn't in this file: [`weapons.ctl`](op2mapper2-weapons-ctl.md) marks them with
`HASTURRET`, which is why `units.ctl` must be read first.

## Writing one

One record per line in the syntax above, no spaces outside the quoted name, and a `;` comment as the last line to be
safe. Structures need a footprint; give the tube offsets for any structure that connects to tubes.

## Sources

- OP2Mapper2 source (BlackBox and Hooman; OPU archive copy marked 2.2.4): `modParsers.bas` (`ParseUnitTable`),
  `modMain.bas` (the `UnitType` record and the order the tables are read), `frmDocument.frm` (placing and drawing
  structures), and the shipped `units.ctl`.
- The Outpost 2 mission SDK headers (OutpostUniverse): the `map_id` enumeration.
- Measured: footprints against `building.txt` (OPU 1.4.1); art against `Outpost2.exe`'s unit animation tables and
  `op2_art.prt` (2079 animations) with op2-formats `UnitArt` and `ArtFile`.
