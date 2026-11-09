# Sprites (`op2_art.prt` and `OP2_ART.BMP`)

Every unit, structure, turret, explosion, weather effect and in-game marker is drawn from two files that work as a
pair:

- **`op2_art.prt`** holds the 256-colour palettes, a table describing each image (size, palette, where its pixels
  are), and the animations: which images make up each frame, where they sit, the selection box and the clip
  rectangles.
- **`OP2_ART.BMP`** holds the pixels of every image, packed one after another behind a BMP header.

The game decides which animation to show for a unit from tables in `Outpost2.exe`, not from these files (see
[Which animation a unit draws](#which-animation-a-unit-draws)). Read by op2-formats' `art.rs`; the unit tables by
`units.rs` (through `exe::GameExe`), click tests by `pick.rs`.

Unless a section says otherwise, the numbers here were measured with op2-formats on GOG Outpost 2 1.2.0.7 with OPU
1.4.1.

## Where the files are

| File | Size (bytes) | Where |
|---|---|---|
| `op2_art.prt` | 1,555,938 | `maps.vol` (stored uncompressed); OPU also ships it loose in `OPU/base/sprites/` |
| `color.bmp` | 1,082 | `maps.vol` (uncompressed); loose in `OPU/base/sprites/` |
| `OP2_ART.BMP` | 7,494,386 | loose in the game folder only |

The loose copies of `op2_art.prt` and `color.bmp` are byte for byte the ones in `maps.vol`. The game finds the files
by the usual search (OPU's loose files first, then the `.vol` archives; see [volumes.md](volumes.md)).

All numbers in both files are little-endian.

## `op2_art.prt`

### Overview

| Section | Size | Contents |
|---|---|---|
| Palette container | 8 + count × 1052 | `CPAL` tag, palette count, then the palettes |
| Image table | 4 + count × 20 | image count, then one record per image |
| Animation header | 16 | four totals |
| Animations | varies | one variable-length record per animation |
| Trailing bytes | varies | not part of any record (see [Trailing bytes](#trailing-bytes)) |

There is no padding between sections and no offset table: a reader walks the file from the start. In the shipped
file the image table starts at 0x2504, the animation header at 0x1CA20 and the animations end at 0x178E56, 12,172
bytes before the end of the file (0x17BDE2).

### Palette container

| Offset | Type | Field | Value in the shipped file |
|---|---|---|---|
| 0 | char[4] | tag | `CPAL` |
| 4 | u32 | palette count | 9 |
| 8 | | palettes | count × 1052 bytes |

The `CPAL` tag looks like the game's tagged sections (`PPAL`, `head`, `data`, also used by its PBMP images, see
[tilesets.md](tilesets.md#file-encodings)), but its second field is a count of palettes, not a byte length.

Each palette is 1052 bytes:

| Offset | Type | Field | Value in the shipped file |
|---|---|---|---|
| 0 | char[4] | tag | `PPAL` |
| 4 | u32 | section size | 1048 in all 9 |
| 8 | char[4] | tag | `head` |
| 12 | u32 | head size | 4 |
| 16 | u32 | version | 1 |
| 20 | char[4] | tag | `data` |
| 24 | u32 | data size | 1024 (op2-formats refuses other sizes) |
| 28 | u8[4] × 256 | entries | red, green, blue, flags |

Entries are stored **red, green, blue, flags**: the reverse of a BMP's blue, green, red order. The flags byte is 4
in all 2,304 entries of the shipped file (Windows' `PC_NOCOLLAPSE`); the game's loader ignores it: it converts each
entry's red, green and blue to a 15-bit colour and steps over the fourth byte (0x4047FA). op2-formats checks the
four tags and the data size, and skips the other three numbers. The `PPAL` size (1048) is 4 more than the 1044 bytes
that follow it, so step from one palette to the next by 1052 bytes, not by the size.

### Image table

A u32 count (5,390), then 20 bytes per image. Images are numbered from 0 in table order; animations refer to them by
that number.

| Offset | Type | Field | Meaning |
|---|---|---|---|
| 0 | u32 | scanline width | bytes per row as recorded: `width` rounded up to a multiple of 4 |
| 4 | u32 | pixel offset | where the image's first row starts, counted from `OP2_ART.BMP`'s pixel data offset |
| 8 | u32 | height | pixels |
| 12 | u32 | width | pixels |
| 16 | u16 | type | flags, below |
| 18 | u16 | palette | index into the palettes (op2-formats refuses one out of range) |

Height comes before width.

The type is a bit field:

| Bits | Meaning | Images with it set |
|---|---|---|
| 0 | drawn in the player's colour (below) | 5,221 |
| 1 | never set | 0 |
| 2 | 1-bit shadow image (else 8-bit indexed) | 408 |
| 3-4 | never set | 0 |
| 5-9 | clip index *k*: drawn only inside the animation's *k*-th clip rectangle (0 = no clipping) | 28 |
| 10-15 | never set | 0 |

The type values that occur, with counts: 0x0 (83), 0x1 (4,871), 0x4 (86), 0x5 (322), 0x21 (3, *k* = 1), 0x41 (20,
*k* = 2), 0x61 (3, *k* = 3) and 0x81 (2, *k* = 4). See [Clip rectangles](#clip-rectangles).

Bit 0 decides player tinting: the sprite draw (0x403F20) hands the blitter the caller's player number only when the
image has bit 0 set, and 0xFF (no tint) otherwise (0x404148-0x404150). OP2Utility calls the bit "in-game graphic".
Of the 4,982 8-bit images, 83 have it clear: 50 of palette 0's 51, and 33 in palettes 3, 5, 6 and 8.

Measured on the shipped table:

- Widths 1 to 196, heights 1 to 205.
- Palette use: palette 0 by 51 images, 1 by 531, 2 by 638, 3 by 1,359, 4 by 14, 5 by 1,169, 6 by 784, 7 by 469,
  8 by 375. Shadows have a palette too (1, 2, 3, 5, 6, 7 or 8), which drawing ignores.
- The scanline width of every image, shadows included, is `width` rounded up to 4. For a shadow this is **not** its
  real row size (see [`OP2_ART.BMP`](#op2_artbmp)).
- 426 images share their pixel offset with another image: they are the same pixels described twice. Every image is
  used by at least one animation.

### Animation header

| Offset | Type | Field | Shipped value |
|---|---|---|---|
| 0 | u32 | animation count | 2,079 |
| 4 | u32 | total frames over all animations | 24,185 |
| 8 | u32 | total layers over all frames | 160,922 |
| 12 | u32 | optional count | 6,012 |

op2-formats checks the frame and layer totals against what it reads and refuses a file where they differ. The
optional count is kept but not checked.

The game uses all three counts as allocation sizes (loader 0x404480, called with `op2_art` at 0x485D91): 14 bytes per
frame, 6 per layer and 4 per optional slot (0x40496F-0x4049D4). Optional slots are handed out per animation: the
first frame that carries a pair reserves one 4-byte slot for **every** frame of its animation (0x404B58-0x404B6D), and
each frame's pairs are stored in its slot as pair 1 x, y, pair 2 x, y (zeros where a pair is absent). So the
optional count must be the total frame count of the animations that have a pair on at least one frame: 6,012 in the
shipped file, not the number of pairs (1,663 first pairs and 647 second pairs). A smaller count overruns the
allocation.

The loader's arrays also bound the file: the sprite manager has room for 5,608 image records and 2,176 animations
(fixed arrays before the image count at manager +0x1B628 and the animation count at +0x56E2C); the shipped file uses
5,390 and 2,079.

### Animation record

36 bytes of header, then the frames, then the clip rectangles:

| Offset | Type | Field | Meaning |
|---|---|---|---|
| 0 | u32 | unknown | 1 in 2,071 animations, 0 in 8 (indices 1535, 1611, 1617, 1865, 1872, 1876, 1897, 1904) |
| 4 | i32 | selection left | selection box, frame-local pixels |
| 8 | i32 | selection top | |
| 12 | i32 | selection right | exclusive |
| 16 | i32 | selection bottom | exclusive |
| 20 | i32 | reference x | the frame-local pixel that sits on the object's map position |
| 24 | i32 | reference y | |
| 28 | u32 | unknown 2 | 0 in 2,060 animations, 60 in 18, 13 in 1 |
| 32 | u32 | frame count | 1 to 263 |
| 36 | | frames | frame count × variable length, below |
| | u32 | clip rectangle count | 0 to 4 |
| | u32[4] × count | clip rectangles | left, top, right, bottom, frame-local pixels, edges included |

The selection box is right and bottom, not width and height: `pick.rs` takes the width as right − left, and that
reproduces the original's click masks. In the shipped file right ≥ left and bottom ≥ top everywhere; 29 animations
have an all-zero box (effects and markers that can't be clicked). Measured ranges: left 0-226, top 0-71, right
0-226, bottom 0-207; reference x 0-148, reference y 0-280.

The animations with unknown 2 = 60 include the construction animations of several structures (for example
animation 70, the Eden Command Center's); the one with 13 is 1645 (a Spider chassis direction). What either field does
is not known.

Frame counts: 1,001 animations have one frame, 252 have three, 166 have fifteen; the longest has 263.

### Frame

| Offset | Type | Field |
|---|---|---|
| 0 | u8 | bits 0-6: layer count (0-127). Bit 7: optional pair 1 follows |
| 1 | u8 | bits 0-6: unknown. Bit 7: optional pair 2 follows |
| 2 | i8, i8 | optional pair 1 (x, y), only if byte 0 bit 7 is set |
| | i8, i8 | optional pair 2 (x, y), only if byte 1 bit 7 is set |
| | 8 bytes × layer count | layers |

So a frame is 2, 4 or 6 bytes plus 8 per layer; pair 1 always comes before pair 2.

- **Layer counts** run 0 to 60. 25 frames have none (in 10 animations).
- **The unknown 7 bits** take 55 values from 0 to 58, plus 127. 127 occurs only in 9 animations that also have
  empty frames. In 845 animations the value changes from frame to frame. The game ignores them: the loader keeps
  only bit 7 of the second byte (0x404B09-0x404B11) and stores a constant 0x7FFE where a frame field might have
  held them (0x404AE7). They are not a frame delay; frame timing comes from the code that draws each kind of
  object.
- **Optional pairs** are signed pixel offsets from the reference point (TethysAPI calls them `FrameOptionalInfo`).
  1,663 frames have pair 1 and 647 have pair 2 (641 both, 6 only pair 2). Values run −126 to 124. On a vehicle
  chassis, pair 1 is where its turret attaches (the Lynx facing east: (0, −5)); on a Tiger turret, pair 2 is the
  second barrel (see [Turrets](#turrets)). In 156 animations only some frames carry pair 1.

### Layer

| Offset | Type | Field | Meaning |
|---|---|---|---|
| 0 | u16 | image | index into the image table |
| 2 | u8 | unknown | 255 in 159,122 layers; 0-42 in the other 1,800 (67 animations) |
| 3 | u8 | layer index | the layer's position in its frame: 0 for the first, 1 for the next, ... in all 160,922 |
| 4 | i16 | x | top-left of the image, frame-local pixels (−23 to 170 measured) |
| 6 | i16 | y | (−40 to 276 measured) |

Later layers are drawn on top. The game places each layer at the position its index byte names, not at its place in
the file (it stores the layer at `layers[index]`, 0x404C45-0x404CA7), so the draw order is index order; in the shipped
file the two are the same.

The values other than 255 in byte 2 cluster in a few animation families (the Cargo Truck dock animations 488-499 use
1 and 2, for example), but the game reads the byte and discards it (0x404C2B-0x404C36: it lands in a scratch variable
that nothing reads), so it is not a draw priority. op2-formats ignores it too.

### Clip rectangles

An image whose type has a clip index *k* (bits 5-9) is drawn only inside rectangle *k* (counting from 1) of the
animation it is drawn in, edges included, in frame-local pixels. The game narrows the drawing surface's clip to the
rectangle (sprite draw at 0x403F20, from op2-formats' comments). This is how a Cargo Truck sinks into a Smelter's
dock pit and a ConVec into a Structure Factory's: animation 497 (a truck unloading common ore at an Eden Smelter) has
rectangles (12, 9, 41, 38) and (12, 5, 41, 38).

90 animations have rectangles: 2 have one, 84 have two, 2 have three and 2 have four. Values run 0 to 134. Every
layer with a clip index (10,680 of them) names a rectangle its animation has. An image with no clip index ignores the
rectangles.

The count of clip rectangles is not in the animation header: it
follows the frames, and op2-formats, reading it there, accounts for every byte of the 2,079 records against the
header's totals.

### Trailing bytes

The 12,172 bytes after the last animation belong to no record. They look like leftover frame and layer records (a
frame header `06 05`, 8-byte layers naming images near 5,357). The game never reads them: its loader reads the
header's number of animations and closes the file (0x404D5C). op2-formats ignores them too, as OP2ArtUtils does (it
keeps them as `extra.dat`).

## `OP2_ART.BMP`

A Windows BMP header in front of a block of packed image pixels. It is not a usable picture: the header describes an
image the data doesn't form.

| Offset | Type | Field | Shipped value |
|---|---|---|---|
| 0 | char[2] | `BM` | checked by op2-formats |
| 2 | u32 | file size | 7,459,842 (the file is 7,494,386) |
| 6 | u16, u16 | reserved | 0, 0 |
| 10 | u32 | pixel data offset | 1078: the base of every image's pixel offset |
| 14 | u32 | header size | 40 (`BITMAPINFOHEADER`) |
| 18 | i32 | width | 2732 |
| 22 | i32 | height | −2731 (top-down) |
| 26 | u16 | planes | 1 |
| 28 | u16 | bits per pixel | 8 |
| 30 | u32 | compression | 0 |
| 34-53 | | image size, resolution, colours | 0, 0, 0, 256, 0 |
| 54 | u8[4] × 256 | palette | all zero |

op2-formats reads only the `BM` tag and the pixel data offset. The image at table entry *i* starts at byte
`pixel_data_offset + image.pixel_offset` and is stored top-down, row after row:

- **8-bit images**: one palette index per pixel; each row is `scanline width` bytes (width rounded up to 4), the
  padding after the last pixel unused.
- **1-bit shadow images** (type bit 2): one bit per pixel, most significant bit first; each row is
  `ceil(width / 32) × 4` bytes, i.e. the bits rounded up to whole 32-bit words. The recorded scanline width (width
  rounded to 4) is not used for drawing.

Measured: sorted by offset, every image starts exactly where the previous one ends (by these row sizes), except
the 426 that repeat an earlier image's offset, from 0 to 7,233,648. That leaves 259,660 bytes of pixel data after the
last image, unused.

## Drawing an image

### Palettes and transparency

An 8-bit image's pixel is an index into its own palette. **Index 0 is transparent**, whatever colour the palette holds
there (black in palettes 1-4 and 7, magenta in 5, 6 and 8, dark grey in 0). In a shadow image a set bit is shadow and
a clear bit is transparent.

The shadow's colour isn't in the files. The game darkens each pixel under a set bit to three quarters: on its 15-bit
screen it takes `(p >> 1) & 0x3DEF` plus that value's own half (`>> 1`, `& 0x3DEF`), i.e. each 5-bit channel `c`
becomes `c / 2 + c / 4`, rounded down (shadow blitter 0x5867FA, in the `DSEG` section; chosen at 0x403FCF). That is
a 25% darkening. Shadow layers are skipped while a game-wide flag (0x576270) is clear (0x403FC2).
op2-formats draws set bits as black at alpha 112 out of 255 (`SHADOW_RGBA`, about 44%), darker than the game.

### Player colours

Palette slots 1-23 of palettes 1-8 are the player colour ramp. As stored they hold player 0's colours (blue): they
are identical in all eight palettes and to the first ramp of `color.bmp`. Palette 0 (51 images) has ordinary colours
there; slot 24 and above are the image's own colours in every palette.

`color.bmp` (1,082 bytes) is a 1×1, 8-bit BMP whose palette carries the ramps: 6 players × 24 entries, from palette
entry 0, each entry blue, green, red, reserved as in any BMP. The six ramps are blue, red, green, yellow, cyan and
magenta (players 0-5). Entry 0 of each ramp is black and unused (slot 0 is the transparent index). Entries 144-255
aren't part of any ramp.

To draw an image in player *p*'s colour, replace slots 1-23 of its palette with entries 1-23 of ramp *p*
(`color.bmp` palette entries `p × 24 + 1` to `p × 24 + 23`). The game does this for images whose type has bit 0 set,
whatever their palette (see [Image table](#image-table)); it copies 24 entries, slots 0-23, from its ramp table
(0x4A8F5A), and slot 0 is transparent anyway. op2-formats' `Sprites::palette` with `Tint::Player` tints every image
except those of palette 0; the two rules differ on the 33 untinted images in palettes 3, 5, 6 and 8 and on the one
palette 0 image with bit 0 set. Only slots 1-23 match between the palettes and `color.bmp`.

### Composing a frame

1. Take the frame's layers in file order.
2. For each, decode the image (8-bit or 1-bit), pick the palette (tinted if needed), and put its top-left at the
   layer's (x, y) in frame-local pixels. Skip transparent pixels; with a clip index *k*, also skip pixels outside
   rectangle *k*.
3. Later layers draw over earlier ones; a shadow layer darkens what is already drawn.
4. Place the frame on the map so that the frame-local reference point (x, y) lands on the object's pixel position:
   the frame's origin goes at `object_pixel − reference`.

op2-formats' `Sprites::frame_rgba` crops to the frame's layers; `frame_rgba_aligned` uses the union of all frames'
layers, so frames of one animation line up. Both return the reference point within the image as `hotspot`.

## Which animation a unit draws

### Game ids

The game numbers animations from **101**: animation record *i* is game id *i* + 101 (`units::ANIM_ID_BASE`). The
sprite manager indexes its array with `id − 101` (GetNumFrames 0x404D80, GetFrameOptionalInfo 0x404F00), and the
smallest id stored anywhere is 101 (the Robo-Dozer facing east). op2-formats' chassis, structure and turret tables
hold record indices (id − 101); the Cargo Truck, docking and explosion tables keep game ids.

### Where the tables come from

No data file says which animation a unit uses. Each unit type's `Init` in `Outpost2.exe` writes animation ids into its
type object. op2-formats' `UnitArt::read` finds them in the player's exe at load time:

1. It finds the unit type vtables in `.rdata` by their shared Save and Load entries (vtable entries 2 and 3: 0x444FE0,
   0x445030). Entry 1 returns the type's sheet code name (`mov eax, string; ret`), entry 4 is its `Init`. An `Init`
   shared by several types stores no art and is skipped, as is the unused SCAT.
2. It decodes each `Init` and collects its stores of constants into the type object (`exe::code::stores`).
3. It reads the turret routines and the Cargo Truck tables described below.

On GOG's exe this gives 15 vehicle types, 38 structure types and 12 weapons with turrets.

### Vehicles: state and direction

A vehicle type stores two tables of `int[4][16]`: Eden at type offset 0x24C, Plymouth at 0x34C, indexed
`[state][direction column]` (Vehicle::GetChassisAnimationIndex 0x44C600; the unit is Eden when MapObject flag 0x1000
is set). An entry left at zero falls back to state 0.

**Direction column.** A unit's rotation is 0-255, 0 = east, increasing clockwise on screen (south = 64). The column is

```
column = ((rotation + 8) & 0xF0) >> 4        (0x4857C3)
```

so each column covers 16 rotation steps centred on `column × 16`. Within a chassis the art runs counterclockwise:
for the 16-sprite types, column *d* is `east + (16 − d) mod 16` (the Lynx: east is record 1202, column 1 is 1217,
column 15 is 1203). The Panther, Tiger and Robo-Dozer have 8 sprites, each filling two adjacent columns. The
Robo-Surveyor's and Earthworker's state 0 rows alternate between two sets of 8.

**States.** Row 0 is the normal chassis. Rows 1-3 exist for some types (ConVec 4 rows, Robo-Dozer and Earthworker 3,
Robo-Surveyor 2, the Eden Repair Vehicle 3, the Plymouth Spider 3). Vehicle::GetChassisAnimationIndex (0x44C600)
picks the row, first match wins:

| Row | When |
|---|---|
| 1 | the unit's action (MapObject `+0x22`) is Move (1) and its path context (`+0x34`) has a step pending (`+0x58`, `+0x5C` or `+0x60` non-zero) |
| 2 | the unit's command (`+0x21`) equals the type's `+0x44C` and the action isn't Move |
| 3 | the command equals the type's `+0x450` and the action isn't Move |
| 0 | otherwise |

The types' `Init`s set `+0x44C` and `+0x450` (both default to 0x35, which no command uses, 0x44B52F): Robo-Dozer
`+0x44C` = 1, its doze command (0x409DAE); Earthworker 7, build wall (0x4487A0); Repair Vehicle and Spider 0x1C,
repair (0x448A21, 0x4478AD); ConVec 6, build (0x410DD0), and `+0x450` = 0x1C, repair (0x410DEE). So row 1 is the
driving sprite (only the Robo-Surveyor has one), row 2 the working sprite, row 3 the ConVec's repairing sprite.
Measured in op2-rs: the Earthworker's and Robo-Dozer's working rows, the ConVec's building row and the Surveyor's
driving row appear exactly when this rule says, on every tick of its recorded runs. Many rows have entries only in
some columns (the ConVec's state 2 only in column 6; the Earthworker's state 2 only in even columns); an entry of 0
falls back to row 0.

**Cargo Truck.** Its chassis depends on the cargo: CargoTruck::GetChassisAnimationIndex jumps through a table by
cargo (0x4065FC); each case loads an Eden and a Plymouth table of 16 u16 game ids. op2-formats reads 10 cargo kinds
(food, common and rare ore, common and rare metal, rubble, spacecraft, wreckage, gene bank and a tenth). The dock and
undock animations by cargo (0 = empty) are four tables of 11 u16 game ids, 0x18 bytes apart from 0x4DEAE0: Eden
dock, Eden undock, Plymouth dock, Plymouth undock. A 0 entry means the plain chassis.

### Structures: slots

A structure type stores `int[10]` for Eden at type offset 0x250 and for Plymouth at 0x278. The structure's current
slot is MapObject `+0x36` (Building::GetSpecialAnimationIndex 0x4836E0):

| Slot | Use |
|---|---|
| 0 | construction: scaffold frames ending in the finished building; a built structure stands on its last frame |
| 1 | damaged (30% of its hit points lost; Building 0x408A20) |
| 2 | heavily damaged (60% lost) |
| 3 | destroyed: the rubble sequence |
| 4 | the structure's 45×45 picture for the command pane, shared by structures of a kind (below) |
| 5-8 | ambient animations played now and then while the structure works (Building::DoAmbientAnimation 0x483580) |
| 9 | unused by every shipped type |

Unused slots are zero. Bytes `+0x2A6` (Eden) and `+0x2A7` (Plymouth) give the type's column in the ambient animation
sequence (0 = none; `Exe::ambient_sequence`). Measured: the Eden Command Center uses records 70 (41 frames), 61, 62,
194, 1111 and 1845 (87 frames, ambient).

Slot 4 is read in two places, both in the command pane: the selection's picture, drawn from the selected
structure's slot 4 (frame 0, in the local player's colour; 0x459727-0x459751, taken when MapObject flag 0x4, set by
every structure's constructor at 0x482F20, is on), and the Structure Factory's picture of the kit it holds, the slot 4
of the structure type in the factory's `+0x60` byte (0x415BC0, called at 0x45BB16 next to a "name, n/m" caption).
Measured: every slot 4 is one of 13 animations of 5 to 51 frames whose first frame is one 45×45 image: records
1100-1112 for 37 structure types (for example 1109 for both mines, both smelters, both storages, the Magma Well and
the GORF; 1107 for the residences, the Forum, Nursery, University, Recreation Facility and Consumer Factory) and 1840
for the Arachnid Factory. Eden and Plymouth share them except the Command Center, Trade Center and Light Tower (1111
Eden, 1112 Plymouth).

A structure's death explosion depends on its footprint: Building::ProcessDeath (0x408B6E) reads a u32 game id at
`0x4DEB34 + 4 × (width × 4 + height)` for widths 1-5 and heights 1-4.

### Turrets

Tanks and Guard Posts draw their weapon as a second sprite. Each weapon type's vtable entries 7-10 are
`Select{GuardPost,Tiger,Panther,Lynx}Graphic`; the base class's versions (0x447180-0x4471B0) draw nothing. Each one
reads a 16-entry table of game ids in `.data` indexed by the direction column (or, for Starflare and Supernova,
stores one id in the turret graphic global 0x4EA7B8). EMP and Starflare choose between two tables by an owner-player
flag: the first is the Eden art (checked visually for EMP). The Guard Post versions of Starflare and Supernova are
types of their own (TURRET_DESTRUCT, TURRET_DESTRUCT_ADV) whose turret op2-formats files under the weapon.

To draw a tank, op2-formats (`units::tank_sprite`):

1. picks the chassis animation for the column;
2. picks the turret animation for the same column from the weapon's table for that chassis;
3. draws the turret with its reference point at the chassis's reference point plus the chassis frame's optional pair
   1 (the Lynx east: 0, −5).

The game uses the pair of the chassis frame it is drawing: the tank draw (0x484A50) picks the chassis frame (frame 0
when standing, a running frame while driving or docking), fetches that frame's optional slot (GetFrameOptionalInfo,
0x484B31) and adds pair 1 to the unit's position for the turret (0x484B48-0x484B5B). op2-formats uses frame 0's pair 1
(checked visually in all 16 directions), which is the same for a standing tank; a driving tank whose chassis frames
carry different pairs moves its turret with them in the game. A Tiger turret's optional pair 2 is its second barrel,
which a shot with flag 0x4000 starts from (Tiger stickyfoam at 0x4A6348).

## What a click hits

The original tests a click against the objects near it (MapObjectManager's point query, 0x437940/0x437990), walking
the object list sorted by pixel y and only objects within 96 rows of the click. Units that are docking are skipped.
For each object (`pick::hits`):

1. **Selection box.** The current animation's selection rectangle, moved so the reference point sits on the object:
   `left − ref_x ≤ dx < right − ref_x` and `top − ref_y ≤ dy < bottom − ref_y`, where (dx, dy) is the click relative
   to the object's pixel. A vehicle's box comes from its chassis animation for its column (0x44CDA0); for a vehicle,
   that is the whole test. The Lynx facing east: a 35×32 box from (−17, −13).
2. **Structure pixels.** A structure must also have a non-zero byte under the click in one of the current frame's
   images (Building MouseOver 0x4093D0, pixel test 0x404F40). The test, as op2-formats reproduces it
   (`Sprites::hit`):
   - The click is turned into frame-local pixels (offset into the box plus the selection's left and top).
   - For each layer, the position inside the image is tested against 0 to width and 0 to height **inclusive**, so the
     byte just past each row and the row just past the bottom count.
   - The byte is read at `pixel_data_offset + pixel_offset + y × scanline_width + x`, for shadows too: a 1-bit
     shadow is read as if it were 8-bit at its recorded stride, so it is clickable in a pattern of stripes and dots,
     and past its end in whatever bytes follow it.
   - A layer with a clip index counts only where the position inside the image falls within its clip rectangle
     minus the reference point (edges included). Unlike drawing, this compares image-local, not frame-local,
     pixels.

   A Guard Post skips this test (MapObject flag 0x80, 0x437A7C), so its whole box counts. The Agridome, Common and
   Rare Storage, Garage, mines and Magma Well also accept the first frame of a second animation when their own
   frame misses (`pick::overlay`, chosen by the structure's slot and colony).

A left click selects the **last** of the player's own units hit in list order (the one drawn on top, 0x41B00D); a
command such as Attack or Repair targets the **first** unit of any player hit (the one underneath, e.g. 0x4533D0).
These rules come from op2-rs's harness measurements of the original's click masks, as recorded in op2-formats'
comments.

## Making one

To add or change art the game will load:

1. Write the palettes (each 1052 bytes as above, entries R, G, B, 4). Keep slots 1-23 of unit palettes as the blue
   ramp so player tinting works, and slot 0 for transparency.
2. Pack the pixels after a BMP header with a correct pixel data offset: 8-bit rows padded to 4 bytes, 1-bit rows to
   4-byte words, images back to back. Record each image's offset from the pixel data start, its width, height,
   scanline width (width rounded up to 4, for shadows as well), type and palette.
3. Write the animations with their frames, layers and clip rectangles, numbering each layer's index byte by its
   position (the game files each layer under that number; at most 4 clip rectangles per animation, the room the
   game's animation record has). The game ignores layer byte 2 and the frame's unknown bits; 255 and 0 match the
   shipped file. Set bit 0 of the type on images that should take the player's colour.
4. Write the header totals: frames and layers must match the records (the game sizes its arrays by them, and
   op2-formats refuses a file whose totals don't), and set the optional count to the total frame count of the
   animations that have an optional pair on any frame (the game sizes the pair storage by it).
5. Keep the shipped animations' indices: the exe's tables address them by number, so new animations go at the end,
   up to 2,176 animations and 5,608 images in all.

Nothing here has been tested by loading a changed file into the game.

## Sources

- op2-formats `art.rs` (the reader, palettes, tinting, compositing, the pixel test), `units.rs` (animation selection
  from `Outpost2.exe`), `pick.rs` (click tests), `exe` (the exe reader).
- OP2Utility (OutpostUniverse): `ArtReader`, the layout of `op2_art.prt` and the image type bits.
- OP2ArtUtils (OutpostUniverse): `prt_file.py`, the layout and the trailing bytes.
- TethysAPI (OutpostUniverse): `FrameOptionalInfo`, `GlobalUnitStats`.
- Disassembly of GOG's `Outpost2.exe` 1.2.0.7 at the addresses given: some as cited in op2-formats' comments, and,
  read for this document, the `.prt` loader (0x404480), the sprite draw (0x403F20) and its blitters (0x4A8EE0, the
  `DSEG` routines from 0x586CEF), GetChassisAnimationIndex (0x44C600), the pane's uses of slot 4 and the tank draw
  (0x484A50).
- op2-rs's measurements of the original game (vehicle animation rows, from its recorded runs).
- Measured: `op2_art.prt`, `OP2_ART.BMP` and `color.bmp` of GOG Outpost 2 with OPU 1.4.1, and the unit tables of GOG's
  `Outpost2.exe`, with op2-formats (counts, value ranges, flags, packing and offsets above).
