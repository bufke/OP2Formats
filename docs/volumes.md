# Volumes (`.vol`) and LZH

A **volume** is the game's archive: a flat list of named files (no folders) packed into one file, each stored raw
or compressed with an LZH scheme. The original game keeps most of its data in five volumes in the game folder.
OPU ships every one of those files loose under `OPU/base/`, and loose files win, so on an OPU install the volumes
are only a fallback. Read by op2-formats' `vol.rs` (archives), `lzh.rs` (decompression) and `game_files.rs` (how
loose files and volumes are layered).

Unless a section says otherwise, the numbers here were measured with op2-formats and small probe programs on the
five volumes of GOG Outpost 2 with OPU 1.4.1.

## The shipped volumes

| Volume | Bytes | Files | Index slots | Contents | Compression |
|---|---|---|---|---|---|
| `maps.vol` | 10,690,596 | 71 | 78 | 54 `.map`, 13 tileset wells (PBMP) and 2 other `.bmp`, `op2_art.prt`, `virmask.raw` | none |
| `sheets.vol` | 87,304 | 10 | 15 | 10 `.txt` rule sheets and tech trees | LZH, all 10 |
| `sound.vol` | 2,867,324 | 97 | 100 | 97 `.wav` sound effects | none |
| `story.vol` | 743,916 | 124 | 125 | 122 `.rtf`, 2 `.txt` | none |
| `voices.vol` | 5,374,296 | 135 | 150 | 135 `.wav` voice lines | none |

No file name appears in more than one volume. `sheets.vol` is the only volume with compressed entries; the other
four store everything raw. The formats of the files inside have their own documents: [maps.md](maps.md),
[tilesets.md](tilesets.md), [sprites.md](sprites.md), [virmask.md](virmask.md), [sheets.md](sheets.md),
[tech-trees.md](tech-trees.md), [sound.md](sound.md).

## Layout

All numbers are little-endian. A volume is built from **sections**, each an 8-byte header followed by its payload:

| Offset | Type | Field |
|---|---|---|
| 0 | char[4] | tag |
| 4 | u32 | bits 0-30: payload length in bytes (the 8-byte header not counted); bit 31: alignment |

Bit 31 is set on every section of every shipped volume (the length word reads `0x80xxxxxx`). It says how the payload
is padded: set, the next section starts at the payload length rounded up to a multiple of 4; clear, rounded up to a
multiple of 2. The game's section code does exactly this when it skips to a section's end (0x4073CF-0x4073EF, and
for a file block 0x4A0826-0x4A0843), and its writer sets the bit when asked for 4-byte alignment (0x40721B). Bits
24-30 belong to the length: the reader masks with `0x7FFFFFFF` (for example 0x4A0C9D, 0x4A0CED). op2-formats does
the same (`length & 0x7FFFFFFF`), sets the bit when writing and pads to 4.

The file is:

```
"VOL "  len = size of everything up to the end of the index (padded)
  "volh"  len = 0
  "vols"  string table
  "voli"  index
  (0-3 zero bytes to a multiple of 4)
"VBLK"  file data          one block per file, each at a multiple of 4,
(0-3 zero bytes)           in any order
"VBLK"  file data
...
```

The `VOL ` section's payload is the three header sections: `volh` is empty (length 0 in all five volumes), and
`vols` and `voli` follow it as siblings, not inside it. So the `VOL ` length is `8 + (8 + vols length) + (8 + voli
length)`, rounded up to a multiple of 4. For `maps.vol` that is 8 + 1512 + 1100 = 2620; for `sheets.vol` 8 + 764 +
218 = 990, stored as 992 with 2 zero bytes after the index. A reader can load just the first `8 + VOL length` bytes
to get the whole index (op2-formats' `VolArchive::open` does this and reads blocks on demand).

### `vols`: the string table

| Offset | Type | Field |
|---|---|---|
| 0 | u32 | `names_len`: bytes of names that follow |
| 4 | char[names_len] | the file names, each ending in a NUL |
| 4 + names_len | | zero padding to the section's length |

`names_len` counts every name and its NUL, so the table's last byte is a NUL. The section is longer than the names
need, and its length is a multiple of 4 in all five volumes; the rest is zeros:

| Volume | `names_len` | `vols` length | Padding |
|---|---|---|---|
| `maps.vol` | 792 | 1504 | 708 |
| `sheets.vol` | 119 | 756 | 633 |
| `sound.vol` | 1131 | 1604 | 469 |
| `story.vol` | 1389 | 1704 | 311 |
| `voices.vol` | 1748 | 3004 | 1252 |

The padding is spare room. `Outpost2.exe` holds a volume builder (0x49F930) that writes the header sections first,
with a string table length chosen by its caller (rounded up to a multiple of 4, plus the 4 bytes of `names_len`) and
a chosen number of index slots, all spare; files are filled in afterwards. The shipped tables fit that: less the 4
bytes, the `vols` payloads are 1500, 752, 1600, 1700 and 3000 bytes, round numbers chosen in advance rather than
fitted to the names. Whether the shipped volumes were built with this code or with a separate tool of the same
design is not known. The game's reader reads the whole payload (0x4A0C95-0x4A0CCE) but uses only `names_len` bytes
of it, so a writer can pad to the next multiple of 4 only; op2-formats' reader accepts that too.

Names are 8.3 file names, at most 12 characters in the shipped volumes. They are plain ASCII. Case varies:
`maps.vol`, `sheets.vol`, `story.vol` and `voices.vol` use lower case only, `sound.vol` mixes (`acid2.WAV`,
`ag_sel.wav`). Lookups ignore case (op2-formats `VolArchive::find`, as Windows does).

### `voli`: the index

An array of 14-byte entries, one per slot; the section length is `14 × slots`:

| Offset | Type | Field |
|---|---|---|
| 0 | u32 | name offset: where the file's name starts, counted from the first name (byte 4 of the `vols` payload, so the first name is at 0) |
| 4 | u32 | block offset: file offset of the file's `VBLK` section (its tag) |
| 8 | u32 | size: the file's bytes as stored (for an LZH entry, the compressed size) |
| 12 | u8 | compression method (below) |
| 13 | u8 | 1 = slot in use, 0 = spare |

Read as one u16, bytes 12-13 give the codes other tools use: `0x0100` raw, `0x0101`, `0x0102`, `0x0103` LZH. The game
treats them as two bytes: it dispatches on byte 12 (0x4A0F54-0x4A0F5C) and tests byte 13 to tell used slots from
spare ones (0x4A0DC8, 0x4A1872).

The entries are in the same order as the names in the string table, and both are sorted by name, ignoring case, in
all five volumes. The order matters: the game finds a file by a **binary search** over all the slots (0x4A1830),
comparing names with a case-insensitive compare that lowers A-Z (`_stricmp`, 0x4CBEB0) and treating a spare slot as
greater than any name. A volume whose names aren't sorted that way, or with a spare slot before a used one, loses
files. (Lowering matters for `_` and the other characters between `Z` and `a`; in the shipped names the lower- and
upper-case orders agree.) The blocks are not in name order: `maps.vol` starts with the tileset wells' blocks,
`voices.vol` with a handful of files from the middle of the list.

The index has more slots than files (7, 5, 3, 1 and 15 spare in the order of the table at the top). A spare slot
has name offset `0xFFFFFFFF` and every other field 0 (byte 13 included); spare slots come after the used ones. The
game's reader (volume open 0x4A0AE0) takes the slot count from the section length (0x4A0CE4-0x4A0CF6), turns each
used slot's name offset into a pointer and stops at the first `0xFFFFFFFF` (0x4A0DBC-0x4A0DDB). op2-formats stops at
the first `0xFFFFFFFF` or after as many entries as there are names. A writer may leave no spare slots: nothing in the
reader needs them.

Compression methods (byte 12):

| Method | Code | What the game does with it | Used |
|---|---|---|---|
| 0 | `0x0100` | reads the block as it is | `maps.vol`, `sound.vol`, `story.vol`, `voices.vol`: all 427 entries |
| 1 | `0x0101` | run-length coding (below) | never |
| 2 | `0x0102` | LZSS without Huffman coding (below) | never |
| 3 | `0x0103` | LZH, see [below](#lzh-compression) | `sheets.vol`: all 10 entries |

The game's volume reader can read all four (0x4A0F5C dispatches to the readers 0x4AFB30, 0x434560 and 0x434080).
No shipped file uses methods 1 and 2, so what follows is from the disassembly only:

- **Method 1** (read 0x4AFCB0): a control byte; with bit 7 set, the next byte is repeated `control & 0x7F` times;
  with bit 7 clear, the next `control` bytes are copied as they are. The reader has no write side (0x4AFCA0 fails).
- **Method 2** (read 0x434800): a bit stream, most significant bit first. A 1 bit is followed by an 8-bit literal. A 0
  bit is followed by a 12-bit position in a 4096-byte ring buffer and a 4-bit length `n`: copy `n + 2` bytes from that
  position on. Position 0 ends the stream. Every output byte also goes into the ring at the write position, which
  starts at 1 (0x43458E). The ring is not cleared when the reader starts (0x43441A), so an encoder must only refer to
  bytes it has written.

op2-formats reports methods 1 and 2 as unsupported. OP2Utility calls them RLE and LZ.

### `VBLK`: a file's data

| Offset | Type | Field |
|---|---|---|
| 0 | char[4] | `VBLK` |
| 4 | u32 | stored size, bit 31 set (the flag above) |
| 8 | u8[size] | the file's bytes, raw or LZH |

The block's length always equals the index entry's size. Each block starts at a multiple of 4; the gap after a
block is 0-3 zero bytes, and the file ends padded to a multiple of 4 the same way (`maps.vol` ends with 2 zero
bytes, `sheets.vol` and `story.vol` with 1).

## LZH compression

Code 0x103 is LZSS (back-references into the last 4096 bytes of output) whose symbols are coded with an **adaptive
Huffman tree**: the scheme of the classic LZHUF program. There is no header inside the stream, no stored output size
and no end code. Everything here is what op2-formats' `lzh.rs` does; it decodes all ten shipped sheets into clean
text.

### Bits

The stream is read one bit at a time, most significant bit of each byte first. Reading past the last byte yields
0 bits (a code may run past the end; see [End of stream](#end-of-stream)).

### Symbols

There are 314 symbols:

- 0-255: a literal byte.
- 256-313: a copy of `symbol - 253` bytes (3 to 60) from earlier output.

### The window

A ring buffer of 4096 bytes and a write position `w`. The game fills positions 0-4035 (0xFC4 bytes) with spaces (0x20)
and starts writing at `w = 4036`, as LZHUF does; it leaves the last 60 positions as they are (0x4340B1-0x4340CB).
Every output byte is also written to `window[w]`, then `w = (w + 1) mod 4096`.

The 60 unset positions are read only by a copy near the start of a stream whose distance reaches behind the 4036
spaces (a distance of 4036 + bytes output so far, or more). No shipped stream does that (their largest distance is
4,035). op2-formats fills all 4096 positions with spaces and starts at 0, which gives the same output for every
stream that doesn't; an encoder should not produce such copies.

The initial spaces are real data: seven of the ten shipped sheets copy from them, reaching back past the start of
the output (`edentek.txt`, `ply_tek.txt` and `tutortek.txt` begin with `;` and a three-byte copy of spaces).

### The adaptive Huffman tree

The tree has 314 leaves and 313 internal nodes, 627 nodes in all, kept in an array where position 626 is the root.
Each position has:

- `freq[n]`: a 16-bit count.
- `child[n]`: for an internal node, the position of its first child (the second is `child[n] + 1`); for a leaf,
  `627 + symbol`. So `child[n] >= 627` means a leaf.
- `parent[n]`: the position of the node's parent. A second table, `leaf[s]`, gives the position of symbol `s`'s
  leaf. (op2-formats keeps both in one array of 941 entries: `parent[627 + s]` is `leaf[s]`.)

Parents are by position: positions `2k` and `2k + 1` always have parent `314 + k`. What moves is the contents of
positions (frequency and child), not the shape.

**Start:**

- Positions 0-313 are leaves: position `i` holds symbol `i` (`child[i] = 627 + i`, `leaf[i] = i`), `freq[i] = 1`.
- Positions 314-626 are internal: position `314 + k` has `child = 2k` and `freq = freq[2k] + freq[2k + 1]`.
- `parent[n] = 314 + n / 2` (integer division) for every position below 626.

**Decoding a symbol:** start at the root, `n = 626`. While `child[n] < 627`, read a bit and go to
`child[n] + bit`. The leaf reached holds symbol `child[n] - 627`.

**Updating** after every decoded symbol `s` (the encoder makes the same update, which keeps both trees equal):

1. `n = leaf[s]`.
2. `freq[n] += 1`.
3. If `n` is the root, stop.
4. Find `m`: start at `m = n` and step `m += 1` while `freq[n] > freq[m + 1]`. The array is ordered by frequency,
   so `m` is the last position whose count is now below `n`'s.
5. Swap the contents of `n` and `m`: exchange `freq[n]` and `freq[m]` and exchange `child[n]` and `child[m]`. Then
   repoint what the two children values point at: for each of the two positions `p` and its new `child[p] = c`,
   if `c >= 627` set `leaf[c - 627] = p`, else set `parent[c] = parent[c + 1] = p`. When `m = n` nothing changes.
6. `n = parent[m]` and go to step 2.

The counts are 16-bit. Like LZHUF, the game rebuilds the tree when the root's count reaches 0x8000: before updating
for a symbol it checks the root (0x4AC2C6) and, at 0x8000, rebuilds (0x4AC170): it gathers the leaves in position
order into positions 0-313, halving each count as `(freq + 1) / 2`, then rebuilds the internal nodes and the
`leaf` and `parent` tables in the manner of LZHUF's `reconst` (each new node, the sum of the next two positions, goes
where it keeps the array ordered by count). op2-formats has no rebuild. In the shipped streams the root's count (314
plus the number of symbols decoded) reaches at most 19,656, so it never comes into play; a stream of more than
32,454 symbols needs it, and a decoder without it goes wrong from that symbol on.

### Copies

After a copy symbol, read the distance. Its upper 6 bits come from a fixed prefix code of 3 to 8 bits, then the lower
6 bits follow raw. Read 8 bits as a byte `b`; its range gives how many more bits to read and the upper 6 bits `u`:

| `b` | More bits | `u` | Prefix length |
|---|---|---|---|
| 0x00-0x1F | 1 | 0 | 3 |
| 0x20-0x4F | 2 | `(b - 0x20) / 16 + 1` (1-3) | 4 |
| 0x50-0x8F | 3 | `(b - 0x50) / 8 + 4` (4-11) | 5 |
| 0x90-0xBF | 4 | `(b - 0x90) / 4 + 12` (12-23) | 6 |
| 0xC0-0xEF | 5 | `(b - 0xC0) / 2 + 24` (24-47) | 7 |
| 0xF0-0xFF | 6 | `b - 0xC0` (48-63) | 8 |

Append the extra bits below `b` (`v = (b << more) | bits`, most significant first). The distance is
`d = u × 64 + (v & 0x3F)`, 0 to 4095. Copy `length = symbol - 253` bytes, one at a time, from position
`(w - d - 1) mod 4096` onwards (the source advances with the copy, so a copy may overlap what it is writing: `d = 0`
repeats the last byte).

Written out as codes for `u`: 0 is `000`; 1-3 are the 4-bit values 2-4 (`0010`-`0100`); 4-11 the 5-bit values
10-17; 12-23 the 6-bit values 36-47; 24-47 the 7-bit values 96-119; 48-63 the 8-bit values 240-255. Each is followed
by the 6 low bits of `d`, so a distance takes 9 to 14 bits.

In the 10 shipped sheets: 27,000 literals and 30,559 copies; copies of every length from 3 to 60 occur (3 is the most
common, 10,534; 60 occurs 555 times); the largest distance is 4,035.

### End of stream

Decoding runs until every input bit has been read; the last symbol may need bits past the end, which read as 0.
That is all op2-formats does (`lzh::decompress`), and it returns everything decoded.

In practice that output is longer than the file. Every shipped LZH entry ends with 35 to 62 zero bytes after the
real stream, and those zeros decode to junk: runs of one repeated byte, or a few random-looking ones. Dropping the
trailing zero bytes before decoding leaves only a little junk from the last partial byte (1 byte after the text
of `building.txt`, 3 after `edentek.txt`; the others weren't checked byte by byte):

| Entry | Stored | Trailing zero bytes | Decoded whole | Decoded without the zeros |
|---|---|---|---|---|
| `building.txt` | 1,679 | 52 | 4,149 | 4,082 |
| `edentek.txt` | 24,942 | 50 | 83,464 | 83,356 |
| `mines.txt` | 201 | 35 | 661 | 279 |
| `morale.txt` | 693 | 46 | 1,673 | 1,466 |
| `multitek.txt` | 29,542 | 43 | 100,920 | 100,824 |
| `ply_tek.txt` | 24,240 | 48 | 81,766 | 81,661 |
| `space.txt` | 465 | 40 | 910 | 826 |
| `tutortek.txt` | 2,811 | 37 | 14,716 | 14,635 |
| `vehicles.txt` | 690 | 43 | 1,356 | 1,229 |
| `weapons.txt` | 943 | 62 | 1,943 | 1,787 |

The volume stores no original size, so the real end has to come from the content. The game's decoder doesn't find it
either: its read entry (0x434280) produces exactly as many bytes as asked, and its bit reader turns a failed read
past the end of the block into a 0 byte (0x4AC023), so it never runs out. The end is found by the reader of the
text. The six rule sheets go through a line reader (0x411290 opens, called for them at 0x4451ED, 0x445775, 0x445B5D,
0x445E18, 0x44B02E and 0x44E2AE; 0x411310 reads a line) that reads 512 bytes at a time and stops at the first line
that

- starts with 0x1A (the old DOS end-of-file mark), or
- has no line feed within the 512 bytes, or
- ends in a line feed without a carriage return before it.

Lines starting with `;` are skipped. Measured with a model of that reader on the decoded sheets (with 4 KB of extra
zero bytes after each stored stream, standing in for the game's endless zero bits): `building.txt`, `mines.txt`,
`space.txt`, `morale.txt`, `vehicles.txt` and `weapons.txt` all stop exactly after their last text line, the first
three because no line feed follows, the last three on a lone line feed in the junk. The four tech trees (`edentek`,
`multitek`, `ply_tek`, `tutortek`) would stop early under that reader (each has a line longer than 512 bytes well
before its end), so they are read by other code, not followed here (see [tech-trees.md](tech-trees.md)). The
trailing zeros themselves are unexplained; they look like an encoder flushing a buffer larger than the data.

### Encoding

An encoder mirrors the decoder: the same window, the same tree and the same update after every symbol. To emit a
symbol, walk from `leaf[s]` to the root, taking bit 1 when the position is odd (a second child: first children are
always at even positions) and 0 when even, then write those bits in reverse (root first). A copy writes its
distance's prefix code and 6 low bits as above. Lengths are 3-60, distances 0-4095 (counting back from the byte
before the write position); a match is worth coding only from 3 bytes. Any match-finding strategy gives a stream
the decoder reads; the original's choice of matches is unknown. A writer should not count on a trailing-zero
convention and should flush the last partial byte with zero bits.

## How the game and op2-formats find files

The game opens its data by file name (for example `VIRMASK.RAW`, see [virmask.md](virmask.md)) through a resource
manager that searches loose files and the volumes. op2-formats' `GameFiles` models an OPU install this way, first
match wins:

1. A name with a folder in it that exists on disk is read directly.
2. Loose files under `OPU/base/` (all subfolders).
3. Loose files under `OPU/maps/` (all subfolders).
4. Loose files in the game folder itself (not its subfolders).
5. The game folder's `*.vol` files, in sorted file name order; the first archive with an entry of that name.

Names match ignoring case, at every step, and only the file name counts: `OPU/base/maps/campaign/eden01.map` answers
`eden01.map`. The OPU folder is the game folder's `OPU` subfolder, or the copy of OPU that op2-rs installs in its
own data folder (`install::opu_dir`). LZH entries are decompressed whole, junk included.

The game's own search, in `Outpost2.exe` (open by name, 0x471170), is:

1. A loose file, looked for by full path (0x471590): first in the folder named by an `ART_PATH` setting in a `DEBUG`
   section, if there is one; then in the game folder; then in a second folder that 0x4714A0 locates (it asks again
   with a retry prompt while it can't; presumably the game CD).
2. The five volumes, always in this order (table at 0x4E38B0): `maps.vol`, `sheets.vol`, `sound.vol`, `voices.vol`,
   `story.vol`, each found by the same path search and opened on first use. The first volume with the name wins.

So loose files beat volumes, and the exe looks in the game folder only, not its subfolders, and in no other volume.
The `OPU/` folders, and any further volumes, are added by OPU's loader, which hooks this search; their place
relative to the game folder is **unconfirmed** here (OPU's loader source, or a test with conflicting loose copies,
would settle it). op2-formats' `.vol` order (sorted by file name: `story` before `voices`) differs from the exe's,
which matters only for a name in both, and no shipped name is.

On this install every file in the five volumes has a loose OPU copy, so none is read from a volume:

| Volume | Loose copies in | Identical to the volume's |
|---|---|---|
| `maps.vol` | `OPU/base/maps/<mission>/`, `OPU/base/tilesets/`, `OPU/base/sprites/` | 33 of 71. The 13 wells differ (OPU converted PBMP to BMP, see [tilesets.md](tilesets.md)); so do 25 maps OPU revised (`cm01`-`cm04`, most `eden` and `ply` campaign maps) |
| `sheets.vol` | `OPU/base/sheets/` (6), `OPU/base/techs/` (4) | 0 of 10: OPU edited every sheet |
| `sound.vol` | `OPU/base/sounds/` | 97 of 97 |
| `story.vol` | `OPU/base/story/` | 124 of 124 |
| `voices.vol` | `OPU/base/voices/` | 135 of 135 |

A mod or a new map therefore needs no volume: a loose file under `OPU/base/` replaces the volume's copy.

## Making a volume

1. List the files, sorted by name compared as lower case (the game's binary search depends on this order).
2. Build the string table: `names_len`, then each name and a NUL; pad the section with zeros to a multiple of 4.
3. Build the index: one 14-byte entry per file, in the same order (name offset counted from the first name, block
   offset, size, method 0 and in-use byte 1, i.e. code 0x100, for raw). Spare slots are optional; if present they
   are `0xFFFFFFFF` then zeros, after the used ones.
4. Write `VOL ` (length as above), `volh` (length 0), `vols`, `voli`, zeros to a multiple of 4, then one `VBLK` per
   file at a multiple of 4. Set bit 31 on every section length, as the shipped files do.
5. Store files raw unless there is a reason to compress; op2-formats reads raw and LZH entries.

The game's reader needs neither the shipped files' spare slots nor their large string padding: it sizes the index
by the `voli` length and the names by `names_len` (0x4A0C95-0x4A0D23), and its search covers whatever slots there
are. op2-formats accepts such a volume too (its tests write one). Nothing here has been tested by loading a new
volume into the game.

## Sources

- op2-formats `vol.rs` (layout, flag bit, codes), `lzh.rs` (the decoder), `game_files.rs` (layering), `install.rs`
  (the OPU folder).
- OP2Utility (OutpostUniverse): `Volume` reader and the HuffLZ / AdaptiveHuffmanTree / BitStreamReader classes that
  `lzh.rs` was ported from; the compression code names.
- LZHUF (Haruyasu Yoshizaki's adaptive-Huffman LZSS program), the scheme the LZH entries follow.
- Disassembly of GOG's `Outpost2.exe` 1.2.0.7, read for this document: the section code (0x4071E0-0x407720), the
  volume reader (open 0x4A0AE0, search 0x4A1830, entry open 0x4A0F20), the volume builder (0x49F930), the method 1,
  2 and 3 readers (0x4AFCB0, 0x434800, 0x434280 with the tree at 0x4ABDA0-0x4AC380), the open-by-name search
  (0x471170, 0x471590) and the sheets' line reader (0x411310).
- Measured: every header field, index slot, block offset and gap of the five volumes of GOG Outpost 2; every LZH
  stream decoded with an instrumented copy of the decoder (symbol counts, lengths, distances, tree counts, end
  behaviour); each volume entry compared with the file `GameFiles` resolves on OPU 1.4.1.
