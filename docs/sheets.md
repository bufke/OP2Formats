# Rule sheets (`vehicles.txt` and friends)

Outpost 2 takes most of its unit and economy numbers from six small text files, the **rule sheets**: `vehicles.txt`,
`weapons.txt`, `building.txt`, `space.txt`, `mines.txt` and `morale.txt`. They hold hit points, costs, build times,
weapon damage, mine yields and morale effects. Research topics, which change these numbers during a game, are in the
tech trees ([tech-trees.md](tech-trees.md)); the names the game shows for each row are in its string table
([strings.md](strings.md)). Read by op2-formats' `sheets.rs` (`Sheet`, `Rules`, `parse_keyed_rows`).

Unless a section says otherwise, the numbers here were measured with op2-formats on GOG Outpost 2 with OPU 1.4.1,
on both OPU's loose copies and the original copies in `sheets.vol`.

## Where the game finds them

| Copy | Where | Notes |
|---|---|---|
| Original | `sheets.vol`, LZH-compressed ([volumes.md](volumes.md)) | The same archive holds the four tech trees |
| OPU 1.4.1 | loose in `OPU/base/sheets/` | OPU makes the game prefer its `base` folder over the archives |
| OPU's backup | `OPU/backups/pre140/` | byte-identical to the `sheets.vol` copies |

Lookups are by bare file name and case-insensitive. op2-formats layers an install the way OPU does: `OPU/base`, then
`OPU/maps`, then the game folder, then the `.vol` archives (`GameFiles::read`), so the first copy found is the one the
game uses. OPU's notes (its `CHANGES.md`) add that a map's own folder under `OPU/maps/<map>` takes precedence while that
map is loaded; op2-formats doesn't model that per-map layer.

`Outpost2.exe` names the sheets in upper case (`VEHICLES.TXT` at 0x4E1548, `MINES.TXT` at 0x4E1ADC, `MORALE.TXT` at
0x4E2258, the others next to `VEHICLES.TXT`); the files on disk are lower case.

| File | In `sheets.vol` (stored / decoded bytes) | OPU copy (bytes) |
|---|---|---|
| `vehicles.txt` | 690 / 1356 | 1273 |
| `weapons.txt` | 943 / 1943 | 1662 |
| `building.txt` | 1679 / 4149 | 4126 |
| `space.txt` | 465 / 910 | 870 |
| `mines.txt` | 201 / 661 | 323 |
| `morale.txt` | 693 / 1673 | 1511 |

### What OPU changed

Comparing the two copies cell by cell: OPU's only data change is the **owner** column, where every `E` (Eden only) and
`P` (Plymouth only) became `B` (both colonies): 4 rows in `vehicles.txt`, 11 in `weapons.txt`, 11 in `building.txt`,
2 in `space.txt`. `G` (Gaia) rows kept `G`. All other cells are identical. The trailing bytes differ (see
[The end of the file](#the-end-of-the-file)).

## Text encoding and line syntax

- 8-bit text, one byte per character. Every byte before the trailer is printable ASCII, tab, CR or LF; non-ASCII bytes
  appear only in the trailing garbage. op2-formats reads each byte as the character of the same code point.
- Lines end in **CR LF**, and the game depends on it: its line reader (0x411310) ends the file at a line feed without a
  carriage return before it (see [The end of the file](#the-end-of-the-file)). A line, CR LF included, must fit in
  512 bytes, the chunk the reader takes (0x411337); a longer one also ends the file.
- A line whose first byte is `;` is a comment; the line reader skips it (0x4113B0), so comments may stand anywhere,
  including before the header rows.
- Cells are separated by **one tab** (the reader's separator, set at 0x411477). op2-formats splits on every tab and
  trims spaces from each cell. The game doesn't trim: it drops one `"` at the start of a cell and turns one `"` at its
  end into a space (0x411404, 0x41142B), and reads every number with the C library's `atoi` (0x41143C), which skips
  leading blanks and stops at the first non-digit. A few cells carry a trailing space in the shipped files (`c ` in
  `weapons.txt`, `d ` in `building.txt`, `i ` and `s ` in `space.txt`); all of them are name keys, which the game
  ignores (below).
- An empty cell (two tabs in a row) is legal: `building.txt` leaves the docking-location cells empty for buildings
  nobody docks at. The game reads an empty number cell as 0.
- Columns are fixed by **position**. The game reads a row's cells one after another, left to right, each into a fixed
  field, and never reads the header text of the four two-header sheets (`building.txt` from 0x44522B, `vehicles.txt`
  from 0x4457BF, `weapons.txt` from 0x445BA4, `space.txt` from 0x445E62). `mines.txt` and `morale.txt` check their
  header names but still read the data by position (below). op2-formats likewise reads each field from its column
  index. Keep the shipped column order.
- Integers are decimal and may carry leading zeros (research topics are written `05115`). Negative numbers appear only
  in `morale.txt`. op2-formats turns an unparsable or missing number into 0, as `atoi` does.
- Flags are `Y` or `N`. The game takes `Y`, `y` and `1` as true and everything else as false (jump table at 0x446420,
  used for every flag column); op2-formats takes `Y` (any case) as true and everything else, `1` included, as false.

### Header rows

`vehicles.txt`, `weapons.txt`, `building.txt` and `space.txt` start with **two header rows**. Each column's title is split
across them, e.g. `Hit` above `Points`; op2-formats joins the two parts with a space (`Hit Points`). The first cell of the
second row is empty. The game skips the first two lines that aren't comments without reading them (two calls of the
line reader before the first row, e.g. 0x445775-0x445799), so their text is free. `mines.txt` has **one** header row.
`morale.txt` has one header row whose first cell is empty.

### Rows and order

After the headers comes one row per object, keyed by its **code name** in the first cell (`LIGHT_TANK`, `TOKAMAK`). The
game compares the cell, byte for byte and so **case-sensitively**, with the code name of each of its object types
(the type table at 0x4E134C, the names around 0x4E1568) and writes the row's values into the type it matches. A row
whose first cell names no type (an empty line, a typo, garbage) is skipped without an error (0x445810 in
`vehicles.txt`'s loader; the other three loaders do the same). op2-formats looks a row up by code name, ignoring case.

Row order is still significant, for the names: the game gives each matched row the next string of a fixed range of its
string table, in row order ([strings.md](strings.md#names-of-sheet-rows)). Swapping two rows swaps the two objects'
names but not their numbers.

The second cell (and in `building.txt` the third) is a short key such as `a`, `b`, ... `z`, `0`-`9`, then `a1`, `b1`,
`c2`, `d1`, `e1`, `f1`. The game reads the cell and does nothing with it (0x44581D for vehicles, 0x445C0F for weapons,
0x44529A and 0x4452CC for buildings); the name comes from the string range instead. op2-formats keeps it as `name_key`
and doesn't use it either.

### The end of the file

The game has no end marker. Its line reader (0x411310) reads the file in 512-byte chunks and reports the end of the
file when one of these holds for the next line:

- nothing is left (end of file);
- the line starts with byte 0x1A (Ctrl-Z, 0x41134C);
- no line feed comes before the first NUL byte or the end of the chunk (0x411353-0x411360);
- the line feed has no carriage return before it (0x411362-0x411368).

Everything after that point is never read. Within the data, a row that names no object type is skipped
([Rows and order](#rows-and-order)), so empty CR LF lines and stray rows cost nothing.

op2-formats stops reading rows at the first empty line (or a line starting with `;`). That is how OPU's copies end: the
last row, an empty line, then a line `; Garbage so checksum matches original:` followed by a few binary bytes, with no
final newline. For the game the empty line is a skipped row, the `;` line a comment, and the binary bytes hold no line
feed before their end or a NUL, so reading ends there (measured on all six OPU copies).

The original copies in `sheets.vol` end differently, and the first-blank-line rule does not hold for them. Running the
game's line rules over them (a probe that follows 0x411310) shows where each one ends:

| File | What follows the last real row | Where the game stops |
|---|---|---|
| `vehicles.txt` | a garbage line `3lg6` ending in a bare LF, then more LF-only lines and binary garbage | at `3lg6`: bare LF |
| `weapons.txt` | a row whose first cell is a quoted comment about `;` rows, then rows of tabs (CR LF), then garbage | at the garbage: bare LF; the comment row and the tab rows name no object and are skipped |
| `building.txt`, `space.txt`, `mines.txt` | binary garbage directly, no empty line | at the garbage: no line feed before a NUL |
| `morale.txt` | binary garbage directly, after the last empty-line-separated group | at the garbage: bare LF |

Read with op2-formats' rules, the original copies give one bogus extra row each (18 vehicles, 30 weapons, 43
buildings, 24 space objects, 10 mine rows) and one bogus `morale.txt` key. A reader for the original files should
follow the game's rules above, or stop at the shipped row counts below, or skip rows whose code name isn't an object
type's.

### The checksum the trailer keeps

The game takes a checksum of the six sheets and the three built-in tech trees, whose names it lists at 0x4E25C8
(`BUILDING.TXT`, `MINES.TXT`, `MORALE.TXT`, `SPACE.TXT`, `VEHICLES.TXT`, `WEAPONS.TXT`, `edentek.txt`, `ply_tek.txt`,
`multitek.txt`); 0x44FFE0 checksums each file in turn (through 0x4712A0). OPU's notes describe this as the check made
when a multiplayer game starts. The checksum (0x40C050) works on the file's bytes:

1. Start from 0. If the length isn't a multiple of 4, the first `n % 4` bytes are taken first: for each byte, the low
   byte of the value becomes that byte and the value shifts left by 8; then the value rotates right by 1.
2. Then for each following 4-byte little-endian word: XOR it in, rotate right by 1.
3. A result of 0 becomes 1.

Measured with a probe that does the same on every copy: for `mines.txt` and `morale.txt` (and the four tech trees,
[tech-trees.md](tech-trees.md#the-end-of-the-file)) the checksum of OPU's loose file equals the checksum of the
**stored, LZH-compressed** entry in `sheets.vol`, not that of the decoded original text. So the trailer is there to make
that number come out the same. For the four sheets whose owner column OPU changed (`vehicles.txt`, `weapons.txt`,
`building.txt`, `space.txt`) the two checksums differ, e.g. 0x081715DD against 0x081493FD for `vehicles.txt`. The
copies in `OPU/backups/pre140/` checksum the same as the decoded originals, as byte-identical files must.

## Row counts and value ranges

| Sheet | Rows | op2-formats type |
|---|---|---|
| `vehicles.txt` | 17 | `VehicleStats` |
| `weapons.txt` | 29 (14 turret weapons and charges, then the EMP missile's blast, the shuttle, map features, disasters and explosions) | `WeaponStats` |
| `building.txt` | 42 | `BuildingStats` |
| `space.txt` | 23 | `SpaceStats` |
| `mines.txt` | 9 | `MineYield` |
| `morale.txt` | 5 morale levels + 67 modifiers | `BTreeMap<String, Vec<String>>` |

### `vehicles.txt`

19 columns.

| # | Header | Field | Type | Measured | Meaning |
|---|---|---|---|---|---|
| 0 | Code_Name | `code` | text | | e.g. `LIGHT_TANK` |
| 1 | Unit Name | `name_key` | text | `a`-`q` | see [Rows and order](#rows-and-order) |
| 2 | Track Type W/T/L/M | `track` | `Track` | W, T, L, M | movement class (below) |
| 3 | Owner P/E/B | `owner` | `Owner` | B (OPU); E, P, B (original) | who may build it |
| 4 | Hit Points | `hit_points` | i32 | 1-800 | the packs have 1 |
| 5 | Armor Class | `armor_class` | u32 | 0-3 | see [Armor classes](#armor-classes) |
| 6 | Move Points | `move_speed` | i32 | 2-5 | lower is faster; op2-formats notes step time = value × the cell's track speed / 512 |
| 7 | Turn Rate | `turn_rate` | i32 | 4-32 | rotation per tick, in 1/256 of a full turn (op2-formats) |
| 8 | Common Ore Cost | `common_cost` | i32 | 250-1500 | |
| 9 | Rare Ore Cost | `rare_cost` | i32 | 0-250 | |
| 10 | Build Time | `build_time` | i32 | 200-1000 | |
| 11 | VF | `vehicle_factory` | Y/N | | built at a Vehicle Factory |
| 12 | AF | `arachnid_factory` | Y/N | | built at an Arachnid Factory |
| 13 | Cargo Capacity | `cargo_capacity` | i32 | 0-1000 | |
| 14 | Sight range | `sight_range` | i32 | 2-4 | |
| 15 | Weapon enabled | `has_weapon` | Y/N | | carries a turret |
| 16 | Research Topic | `research_topic` | u32 | 2099-8201 | topic id that enables it ([tech-trees.md](tech-trees.md)) |
| 17 | Production Rate | `production_rate` | i32 | 0-128 | |
| 18 | Repair Amt | `repair_amount` | i32 | 0-32 | |

Track letters (`Track`, whose discriminants are the game's track type, the index into each cell type's speed table in
[maps.md](maps.md)): `L` legged 0, `W` wheeled 1, `M` miner 2, `T` tracked 3, `H` hover 4. The shipped files use only
W, T, L and M. The game also takes the lower-case letters, and leaves the type's track as it was for any other letter
(jump table at 0x4463A4, from 0x445870); op2-formats reads any other letter as wheeled.

Owner letters (`Owner`): `E` Eden, `P` Plymouth, `B` both, `G` Gaia (the world: disasters and the like). The game sets
an Eden flag (0x1000) for `E`, a Plymouth flag (0x800) for `P` and both for `B`, takes the lower-case letters alike,
and sets neither for `G` or anything else (jump tables at 0x4463D4 for vehicles, 0x446054 for buildings).
op2-formats reads any value other than the four letters as both.

The values agree with what op2-rs's harness read from the running game (hit points, move speed, turn rate, armor),
according to `sheets.rs`.

### `weapons.txt`

14 columns. The sheet covers turret weapons and also things that do damage without a turret.

| # | Header | Field | Type | Measured |
|---|---|---|---|---|
| 0 | Code_Name | `code` | text | |
| 1 | Weapon Name | `name_key` | text | `a`-`z`, `0`-`2` |
| 2 | Damage Radius | `damage_radius` | i32 | 0-320 |
| 3 | Pixels Skipped When Firing | `pixels_skipped` | i32 | 0-96 |
| 4 | Weapon Range in tiles | `range` | i32 | 0-7 |
| 5 | Turret Turn Rate | `turret_turn_rate` | i32 | 0-16 |
| 6 | Owner P/E/B/G | `owner` | `Owner` | 17 B and 12 G (OPU) |
| 7 | Concussion Damage | `concussion_damage` | i32 | 0-1500 |
| 8 | Penetration Damage | `penetration_damage` | i32 | 0-1200 |
| 9 | Common Ore Cost | `common_cost` | i32 | 0-1200 |
| 10 | Rare Ore Cost | `rare_cost` | i32 | 0-300 |
| 11 | Build Time | `build_time` | i32 | 0-20 |
| 12 | Reload Time | `reload_time` | i32 | 0-255 |
| 13 | Research Topic | `research_topic` | u32 | 0-10102; 0 for rows no topic enables |

Both the damage radius and "pixels skipped" are in **pixels** (32 to a tile). Measured in op2-rs: a plain weapon
hurts what stands within its damage radius of the impact point, a vehicle taking `(radius - d) * base / radius` with `d`
the distance in pixels (the four turret weapons have radius 16), and a shot whose scattered aim point lands nearer
than "pixels skipped" is moved out along its bearing. The weapons loader stores the radius and pixels skipped as read
(0x445C57, 0x445C70), and clamps the turret turn rate to 1-16 (0x445C98), so the explosion rows' 0 becomes 1.

### `building.txt`

32 columns.

| # | Header | Field | Type | Measured |
|---|---|---|---|---|
| 0 | Code_Name | `code` | text | |
| 1 | Produce List Name | `produce_list_key` | text | same key as column 2 in every row |
| 2 | Building Name | `name_key` | text | |
| 3 | X-size | `width` | i32 | 1-5 tiles |
| 4 | Y-size | `height` | i32 | 1-4 tiles |
| 5 | Owner P/E/B | `owner` | `Owner` | all B (OPU) |
| 6 | Hit Points | `hit_points` | i32 | 300-3000 |
| 7 | Armor Class | `armor_class` | u32 | 0-5 |
| 8 | Border Y/N | `border` | Y/N | |
| 9 | Common Ore Cost | `common_cost` | i32 | 50-3000 |
| 10 | Rare Ore Cost | `rare_cost` | i32 | 0-600 |
| 11 | Structure Kit | `structure_kit` | Y/N | built from a kit at a Structure Factory |
| 12 | Building Points | `build_points` | i32 | 50-3000 |
| 13 | Normal Rubble | `common_rubble` | i32 | 0-7 |
| 14 | Rare Ore Rubble | `rare_rubble` | i32 | 0-1 |
| 15 | Power required | `power_required` | i32 | 0-100 |
| 16 | Workers required | `workers_required` | i32 | 0-4 |
| 17 | Scientists required | `scientists_required` | i32 | 0-3 |
| 18 | Sight range | `sight_range` | i32 | 0-8 |
| 19 | Docking: All | `dock_all` | Y/N | any vehicle docks |
| 20 | Docking: Truck | `dock_truck` | Y/N | cargo trucks dock |
| 21 | Docking: CV | `dock_convec` | Y/N | ConVecs dock |
| 22 | Docking: Evac | `dock_evac` | Y/N | evacuation transports dock |
| 23 | Docking Loc E | `dock_location_eden` | `Option<i32>` | empty, or 1-4 |
| 24 | Docking Loc P | `dock_location_plymouth` | `Option<i32>` | empty, or 1-5 |
| 25 | Storage Capacity | `storage_capacity` | i32 | 0-10000 |
| 26 | Production Capacity | `production_capacity` | i32 | 0-500 |
| 27 | Storage Bays | `storage_bays` | i32 | 0-6 |
| 28 | Can be AutoTargeted | `auto_targeted` | Y/N | |
| 29 | Explosion Size | `explosion_size` | i32 | 0-3 |
| 30 | Resource priority | `resource_priority` | i32 | 1-34, 99, 999 (mines, tubes, walls) |
| 31 | Research Topic | `research_topic` | u32 | 2101-10204 |

The docking locations are filled in exactly for the 12 buildings that have a docking flag set. A docking location is
the dock tile's **column on the footprint's bottom row**, counted from 1 at the west edge, one for an Eden structure
(`Docking Loc E`) and one for a Plymouth one (`Docking Loc P`), since the two colonies' buildings can have the dock
in different places. The game's dock lookup (0x482F40) returns no dock unless the type has `Docking: All`, `Truck` or
`CV` set (the `Evac` flag isn't part of that test), picks the column by the structure's Eden flag, and returns the
tile `(left + location - 1, bottom row)`, the x masked by the map's width mask. The loader keeps each location as a
byte (0x445658, 0x44566E).

Resource priority runs 1-34 without gaps (one building each), then 99 for two and 999 for six. It orders the
colony's building list: measured in op2-rs, the list is kept sorted by this number (the newest first among equals),
and power, workers and scientists are handed out in that order, so a lower number is served first. The loader keeps
it as a byte (0x445733), so 999 becomes 231. Explosion size goes into two flag bits (0x445710), so only 0-3 fit.
Production capacity means different things per
building: the tech trees raise it for residences under the description "Increased Colonist capacity", for example
([tech-trees.md](tech-trees.md#unit_prop)).

### `space.txt`

7 columns: `Code_Name`, `Space Object Name` (key), `Owner (P/E/B)`, `Common Ore Cost`, `Rare Ore Cost`, `Build Time`,
`Research Topic`, decoded as `SpaceStats` (`code`, `name_key`, `owner`, `common_cost`, `rare_cost`, `build_time`,
`research_topic`). Rows are satellites, the starship modules `SPACE_1` to `SPACE_15`, the launch vehicles `SULV` and
`RLV`, `EMP_MISSILE`, and the luxury goods `LUX_1` to `LUX_3`. Measured: costs 0-10000 of either ore, build times
500-1800, research topic 0 (none) on 6 rows.

### `mines.txt`

One header row, then 9 rows: three yield classes (`High`, `Med`, `Low`) with three variants each (1-3). 7 columns,
decoded as `MineYield`:

| # | Header | Field | Meaning (from the headers and op2-formats' docs) |
|---|---|---|---|
| 0 | YIELD | `yield_class` | `High`, `Med` or `Low` |
| 1 | VARIANT | `variant` | 1-3 |
| 2 | INITIAL_YIELD_% | `initial_yield_percent` | yield when the mine opens |
| 3 | PEAK_TRUCK | `peak_truck` | truckload count at which yield peaks |
| 4 | PEAK_YIELD_% | `peak_yield_percent` | the peak yield |
| 5 | MIN_TRUCK | `min_truck` | truckload count at which yield bottoms out |
| 6 | MIN_YIELD_% | `min_yield_percent` | the lowest yield |

op2-formats reads the rows itself (not through `Sheet`, which assumes two header rows).

The game's loader (0x44B010) is stricter than the other sheets':

- The header must have exactly 7 cells (6 tabs, checked at 0x44B056), named as above; each name is compared
  ignoring case (`_stricmp`, 0x4CBEB0) with the list at 0x4E1AC0. Any mismatch fails the load.
- Then it reads exactly 9 lines (two nested loops of 3, 0x44B0A5-0x44B17C) and ignores their `YIELD` and `VARIANT`
  cells: line *r* is yield class *r* / 3 (High, Med, Low: the 3-, 2- and 1-bar mines) and variant *r* % 3. Keep the
  shipped row order.
- The three yields are stored ×10 (`x * 1000 / 100`, 0x44B0E6), turning the percentages into ore per truckload; the
  two truck counts are stored as written.

How a mine uses a row, measured in op2-rs: the yield of each load climbs linearly from the initial value at load 0 to
the peak at load `PEAK_TRUCK`, falls linearly to the minimum at load `MIN_TRUCK`, and stays at the minimum from then on
(mines never run dry); an improved mine yields 120%, and a rare-ore mine half. Two trucks on one mine share its load
count. The row is chosen by the mine's beacon: its yield class (bars) and variant (0-2) are the beacon's, set by the
mission when it places the beacon or drawn at random when the mission leaves them open.

### `morale.txt`

A keyed list rather than a table. The first line is a header whose first cell is empty, then 8 column names
(`M_PTS_TOT`, `M_POW_BONUS`, `M_RSCH_BONUS`, `M_FACT_BONUS`, `M_FOOD_BONUS`, `M_DEFECT_RATE`, `M_FERT_RATE`,
`M_DIE_RATE`). Then groups of rows separated by empty lines:

- **Morale levels**: 5 rows (`UTOPIA`, `GROOVY`, `OKAYFINE`, `UPSET`, `WAYBAD`), each with 8 numbers for the 8 columns.
- **Modifiers**: 67 rows of a name and one number. The number sits in the third cell, after an empty one, i.e. under
  `M_POW_BONUS`; some rows carry further empty cells.

op2-formats' `parse_keyed_rows` returns every row whose first cell is a printable-ASCII name, keyed by that name, with
its non-empty cells; it stops at a line starting with `;`. That drops the cell positions, which only matters for
the modifiers' empty first value cell.

The game's loader (0x44E290) reads it by position and by name:

- The header must have 9 cells (8 tabs, 0x44E2DA); cells 2-9 are compared ignoring case with the 8 column names
  (list at 0x4E2238). A mismatch fails the load.
- The next 5 lines are the morale levels, in the order UTOPIA to WAYBAD; their names aren't checked (the first 15
  characters are kept, 0x44E350), and their 8 numbers fill the columns in order.
- Every further line is a modifier: a line with an empty first cell is skipped; otherwise the name is looked up,
  ignoring case, in the 67-entry modifier table, and the value is taken from the **third** cell (0x44E4CB-0x44E4E0).
  A name not in the table, or one given twice, fails the load (0x44E613, 0x44E629).

The 67 modifier names are exactly the names of the executable's morale modifier table (0x4E1F10: 67 entries of name,
value and a percent flag; values are 0x80000000 until the file is read). Seven are flagged as percentages, which the
loader converts to 1/256 (0x44E4E6): `EVENT_DEC_RATE` and the six `DIFF_*` difficulty rows. Modifier values in the
shipped file range from -800 to 110. The level names are not in that table.

## Armor classes

A sheet stores an **armor class**, not an armor value. The game turns the class into an armor value, a damage factor
out of 256, from six dwords in `Outpost2.exe` at 0x4E1518 (op2-formats: `Exe::armor_values`,
`GameExe::armor_value`):

| Class | 0 | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|---|
| Armor value | 256 | 180 | 130 | 90 | 60 | 40 |

256 means no armor. op2-formats checks the table's shape (first entry 256, strictly falling, last above 0) and returns
256 for a class outside 0-5. Measured: vehicles use classes 0-3, buildings 0-5. The unit panel's armor word comes from
the armor value and differs between vehicles and structures ([strings.md](strings.md#armor-words)).

## Writing a sheet

- Keep the columns, their order and both header rows (one for `mines.txt` and `morale.txt`, whose header names the
  game checks), tab-separated, CR LF on every line: a bare LF ends the file for the game.
- Keep the rows and their order: the names follow row order, and `mines.txt` and the morale levels are read by
  position. Code names must match the game's exactly, in upper case.
- End the data with an empty line, which op2-formats needs and the game skips. A trailer after it is optional; OPU's
  copies keep one so the multiplayer checksum stays the original's.
- Put the file loose in `OPU/base/sheets/` (or a map's folder, per OPU's notes) rather than editing `sheets.vol`.

## Sources

- op2-formats `sheets.rs` (`Sheet::parse`, `parse_keyed_rows`, `Rules::load`, the stats types), `exe/mod.rs`
  (`ARMOR_VALUES`, `MORALE_MODS`), `game_files.rs` (lookup order).
- OPU 1.4.1's `CHANGES.md` (the `base` folder and per-map folders, the game-start checksum).
- Measured: OPU 1.4.1's loose sheets and the `sheets.vol` copies, decoded with op2-formats, compared cell by cell; the
  strings and tables in GOG's `Outpost2.exe` 1.2.0.7; the game's line rules and its checksum (0x40C050) run over
  every copy by a probe.
- Raw disassembly of GOG's `Outpost2.exe` 1.2.0.7 (iced-x86), at the addresses given: the line and cell readers
  (0x411290-0x411450), the four table loaders (0x4451E8-0x445FD9), the `mines.txt` and `morale.txt` loaders
  (0x44B010, 0x44E290), the dock lookup (0x482F40), the checksum (0x40C050, 0x44FFE0).
- op2-rs (bufke's reimplementation), measurements of the running game: weapon damage radius and pixels skipped, mine
  yields over time, the building list's priority order.
