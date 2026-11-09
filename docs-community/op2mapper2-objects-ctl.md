# OP2Mapper2 object table (`objects.ctl`)

OP2Mapper2 is the classic Outpost 2 map editor, written in Visual Basic 6 by BlackBox and Hooman (2.0 alpha released
in 2005; the source read here is marked 2.2.4 in its project file). `objects.ctl`, in the mapper's own folder, lists
the Gaia objects it can place, those that belong to no player: walls, tubes, mining beacons, magma vents, fumaroles
and wreckage. The game never reads it.

It is read at start-up after [`units.ctl`](op2mapper2-units-ctl.md) and
[`weapons.ctl`](op2mapper2-weapons-ctl.md); its entries join the same definition list as the units, after them. A
placement in a [`.map.dat`](op2mapper2-map-dat.md) names an object by its `map_id` and its three extras.

## Line syntax

The same as the other two tables. Each line is handled on its own:

1. Leading and trailing spaces are trimmed, then every tab is removed.
2. A line starting with `;` is a comment.
3. A line starting with `INCLUDE` (any case) reads another file with the same parser. The name is the text between
   the first pair of double quotes, taken relative to **the mapper's folder**, not to the including file.
4. An empty line is skipped.
5. Anything else is a record: comma-separated fields. Field 1 (counting from 0) is a string in double quotes; it
   can't hold a double quote. Every other field is an integer made of the digits and `-`. A space outside the
   quotes, a quote outside field 1 or any other character is an error that stops reading the file; so is a field
   that doesn't convert to a number or doesn't fit its type. Fields past the seventh are ignored, whatever they hold.

The records read before an error stay in the list. The shipped file uses CRLF line ends and ends with a comment line
without a line break, which the file says works around a parser bug.

## Records

```
map_id,"name",type,art[,extra1[,extra2[,extra3]]]
```

| Field | Type | Meaning |
|---|---|---|
| map_id | 32-bit | the game's type id: 17 tube, 18 wall, 19 lava wall, 20 microbe wall, 81 mining beacon, 82 magma vent, 83 fumarole, 84 wreckage |
| name | text | the name shown in the Gaia list |
| type | 16-bit | which code template section the object uses: 0 `$$beacon`, 1 `$$wall`, 2 `$$wreck` (see [the templates](op2mapper2-tpl.md)) |
| art | 32-bit | an `op2_art.prt` animation (see [sprites.md](../docs/sprites.md)), −1 for none, or a marker |
| extra1-3 | 16-bit | the values given to the template's `$extra1` to `$extra3`; −1 when left out |

The markers draw text on the object's tile instead of a picture: −2 a white T, −3, −4 and −5 the letters NW, LW and
MW in cyan. They work only for entries that aren't structures, which objects never are; another negative value draws
nothing.

The type only chooses the template section; the mapper places and draws every object the same way, on one tile. A
type other than 0, 1 or 2 has no section (see the templates document for what the generator writes then).

The extras are the arguments of the mission SDK call the shipped templates write:

| Type | Call | extra1 | extra2 | extra3 |
|---|---|---|---|---|
| 0 | `TethysGame::CreateBeacon` | 0 common, 1 rare | yield: 0 three bars, 1 two, 2 one | variant: 0-2, −1 random |
| 1 | `TethysGame::CreateWallOrTube` | the call's unused third argument | | |
| 2 | `TethysGame::CreateWreck` | the tech id the wreck gives | 1 if visible without a scout | |

The yield and variant values follow the mission SDK's `Yield` and `Variant` enumerations (−1 is random for both).
Several objects can share a `map_id`; the mapper tells them apart by `map_id` and all three extras together.

## The shipped file

13 records: the tube (marker T) and the three walls (NW, LW, MW), each type 1 with extra1 0; six beacons that share
`map_id` 81 and differ only in their extras (common and rare, each at yield 2, 1 and 0, variant −1), named Common and
Rare Beacon 1 to 3 by their bars; the magma vent and the fumarole (type 0 with no extras, so −1, −1, −1); and one
wreck, "Tiger Speed Mod Wreckage" (tech 11999, visible).

## Writing one

One record per line, no spaces outside the quoted name, and a `;` comment as the last line to be safe. Give each
object a type its template handles, and distinct extras to objects that share a `map_id`.

## Sources

- OP2Mapper2 source (BlackBox and Hooman; OPU archive copy marked 2.2.4): `modParsers.bas` (`ParseObjectTable`,
  `ParseAndGenerateUnits`), `frmDocument.frm` (drawing the markers), and the shipped `objects.ctl`.
- The Outpost 2 mission SDK headers (OutpostUniverse): the `map_id`, `Yield` and `Variant` enumerations and the
  `CreateBeacon`, `CreateWallOrTube` and `CreateWreck` calls.
