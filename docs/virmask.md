# The Blight's masks (`virmask.raw`)

`virmask.raw` holds 16 one-bit masks of 32x32 pixels, one for each way the Blight (the microbe) can cover the four
cells round a tile corner. The game picks a mask per tile from which of those cells are blighted and draws the
Blight through it, so the Blight's edge is a rounded shape instead of whole tiles. Read by op2-formats'
`virmask.rs`.

Unless a section says otherwise, the numbers here were measured with op2-formats on GOG Outpost 2 with OPU 1.4.1,
and the code addresses are in GOG's 1.2.0.7 `Outpost2.exe`.

## Where the file is

- `maps.vol`, entry `virmask.raw`, stored raw (code 0x100; see [volumes.md](volumes.md)).
- OPU ships it loose as `OPU/base/sprites/virmask.raw`, byte for byte the same as the `maps.vol` copy, and the loose
  file wins.

The game asks for it by the name `VIRMASK.RAW` (the exe's only mention of it, at 0x4EA568), through the same
search as every other data file, so a loose `virmask.raw` replaces it.

## Layout

2048 bytes, no header:

| Offset | Size | Contents |
|---|---|---|
| `k × 128` | 128 | mask `k`, for `k` = 0 to 15 |
| `k × 128 + y × 4` | 4 | row `y` (0 = top) of mask `k`: a **big-endian** 32-bit word |

In a row word, bit 31 is the leftmost pixel (x = 0) and bit 0 the rightmost (x = 31). So pixel (x, y) of mask `k`
is bit `7 - (x mod 8)` of byte `k × 128 + y × 4 + x / 8`.

The bits are **inverted**: a 0 bit in the file is a Blight pixel, a 1 bit is clear ground.

The game reads it at 0x49E250: it asks for exactly 0x800 bytes (and gives up, returning failure, if the read
fails), then for each of the 512 words swaps it to big-endian order and inverts it in place (`bswap`, `not`).
op2-formats' `virmask::parse` does the same and returns `[[u32; 32]; 16]` with a set bit meaning Blight; it needs
at least 2048 bytes and ignores any more.

## Choosing a mask

The mask index is four corner bits, each the Blight bit (bit 30 of the cell's tile word, see [maps.md](maps.md)) of
one cell:

| Bit | Value | Cell |
|---|---|---|
| 0 | 1 | the tile's own cell |
| 1 | 2 | its east neighbour (x + 1) |
| 2 | 4 | the cell below (y + 1) |
| 3 | 8 | below-right (x + 1, y + 1) |

Measured in the detail view's drawing code: 0x470204-0x470277 reads bit 30 of those four cells and combines them
into 1, 2, 4 and 8. The neighbours' x and y wrap with the map's size masks. That code builds an index only when a
game-wide flag (the dword at 0x57626C) is non-zero, and then only for cells with bit 29 set (the "the Blight may
spread from here" bit in maps.md); every other cell gets 0.

The masks follow that corner layout: the Blight of each mask sits in the quadrant of the corner bits it has set
(bit 1 the top left, 2 the top right, 4 the bottom left, 8 the bottom right).

## What the masks hold

Blight pixels per mask, and how they fall in the four 16x16 quadrants:

| Mask | Corners | Blight pixels | Share | Top left | Top right | Bottom left | Bottom right |
|---|---|---|---|---|---|---|---|
| 0 | none | 0 | 0% | 0 | 0 | 0 | 0 |
| 1 | own | 227 | 22% | 190 | 19 | 17 | 1 |
| 2 | east | 209 | 20% | 13 | 180 | 0 | 16 |
| 3 | own, east | 471 | 46% | 221 | 214 | 17 | 19 |
| 4 | below | 215 | 21% | 12 | 0 | 188 | 15 |
| 5 | own, below | 472 | 46% | 214 | 20 | 221 | 17 |
| 6 | east, below | 414 | 40% | 26 | 188 | 178 | 22 |
| 7 | all but below-right | 825 | 81% | 256 | 246 | 245 | 78 |
| 8 | below-right | 199 | 19% | 0 | 10 | 11 | 178 |
| 9 | own, below-right | 434 | 42% | 190 | 31 | 32 | 181 |
| 10 | east, below-right | 469 | 46% | 17 | 219 | 19 | 214 |
| 11 | all but below | 809 | 79% | 244 | 256 | 68 | 241 |
| 12 | below, below-right | 471 | 46% | 20 | 16 | 214 | 221 |
| 13 | all but east | 815 | 80% | 243 | 76 | 256 | 240 |
| 14 | all but own | 797 | 78% | 66 | 237 | 239 | 255 |
| 15 | all four | 913 | 89% | 256 | 256 | 199 | 202 |

Other facts measured on the file:

- Mask 0 is empty, as expected. **Mask 15 is not full**: rows 0-27 are full, rows 28-30 are empty, and row 31 has 17
  of 32 pixels set. The game never uses it: the tile blit draws a Blight index of 15 without a mask, as a fully
  blighted tile (0x4A8B7D), and a light index of 15 as a fully lit one (0x4A8AB6; see below). A reader should do the
  same.
- Each one-corner mask is the exact complement of the three-corner mask missing that corner: 1 and 14, 2 and 13, 4
  and 11, 8 and 7. The two-corner masks (3, 5, 6, 9, 10, 12) are not complements of each other.
- The masks are drawn separately, not made by flipping: no mask is the mirror image of the mask with mirrored
  corners (left-right or top-bottom), and they don't nest (in 43 of the pairs where one mask's corners include
  another's, the bigger mask doesn't contain all of the smaller one's pixels).
- In the file, 621 bytes are 0x00 (8 Blight pixels), 714 are 0xFF (8 clear pixels), the rest are edges.

## How the game uses it

The loader runs on the Blight manager (0x57B7D8, called at 0x485D23) and keeps the masks at 0x57B7EC. That address
appears in the code only in the tile blit (0x4A89A0, which picks mask `k` at `0x57B7EC + k × 128`,
0x4A8AEC-0x4A8C27). The blit takes two corner indices per tile, and the masks serve both:

- **The Blight.** Inside the Blight mask, every other pixel is drawn in RGB555 0x3018, the Blight's purple (99, 0,
  198 on screen; blitter 0x58665B, the colour at 0x5866B3), the rest show the terrain. The alternation follows the
  pixel's screen position, so the dither is fixed to the screen. Index 15 draws the dither over the whole tile.
  Measured in op2-rs: its purple pixels match the original's, all 18,643 of them, in a recorded run (tick 2000).
- **Night.** The second index is the tile's four lit corners (bit 0 the tile, 1 east, 2 south, 3 south-east, from the
  view's lit-corner bitmap, 0x4702A0-0x4702F6). 15 draws the tile with the day palette, 0 with the night one, and
  the other values pixel by pixel through the same masks: a set pixel lit, a clear one dark. Measured in op2-rs: the
  lit area round a light source matches the original's to a pixel or two.

## Making one

1. Draw 16 masks of 32x32, mask `k` for the corner set `k` in the table above (Blight towards the set corners).
2. Write each row as a 32-bit big-endian word with the leftmost pixel in the top bit, a **0** bit for Blight and a 1
   for clear ground; masks in order 0-15, rows top to bottom: 2048 bytes.
3. Put it at `OPU/base/sprites/virmask.raw` (or inside a volume as `virmask.raw`).

Keep mask 0 all clear (all bytes 0xFF in the file). Making mask 15 fully blighted (all 0x00) is the safe choice.

## Sources

- op2-formats `virmask.rs` (layout, inversion, corner bits).
- Outpost2.exe 1.2.0.7, disassembled for this document: the loader at 0x49E250 (reads 0x800 bytes of `VIRMASK.RAW`,
  byte-swaps and inverts each word), the corner bits at 0x470204-0x470277 and 0x4702A0-0x4702F6, and the tile blit
  0x4A89A0 with its `DSEG` blitters.
- op2-rs's measurements of the original game: the Blight's dither pixels and the night shading.
- Measured: OPU 1.4.1's `virmask.raw` and the `maps.vol` copy, compared, and every mask counted.
