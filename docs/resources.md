# Windows resources (`out2res.dll`, `op2shres.dll`; `.bmp`, `.cur`, `.ico`, `.ani`)

Outpost 2 keeps most of its interface art, its dialog layouts and its cursors as ordinary Windows resources in two
DLLs: `out2res.dll` for the game itself and `op2shres.dll` for the menu shell. The animated cursors are loose `.ani`
files in the game folder. None of this is specific to Outpost 2: it is the standard Win32 resource format, and this
document describes the parts the game's files use. Read by op2-formats' `pe_res.rs`.

Unless a section says otherwise, the numbers here were measured with op2-formats on GOG Outpost 2 with OPU 1.4.1.

## Which files

| File | Resources | By type |
|---|---|---|
| `out2res.dll` (451,584 bytes) | 79 | 36 bitmaps, 19 dialogs, 10 cursor groups with 10 cursors, 3 accelerator tables, 1 version |
| `op2shres.dll` (2,462,208 bytes) | 91 | 67 bitmaps, 16 dialogs, 2 string blocks, 1 menu, 1 icon group with 2 icons, 1 accelerator table, 1 version |
| `Outpost2.exe` | 4 | 1 icon group, 1 icon, 1 manifest, 1 version |
| `odasl.dll`, `us_them.dll`, the other 105 DLLs (the missions: `e01.dll`, `p01.dll`, `t01.dll`, `ml4_21.dll`, ...) | 1 each | a version resource |
| `OP2Shell.dll`, `helphook.dll` | 0 | |

All resources are language 1033 (US English). Both resource DLLs are resource-only: an `.rdata`, an `.rsrc` and a
`.reloc` section, the `.rsrc` taking nearly all of the file.

The version resources say "Sierra On-Line Inc." and file version 1.2.0.5 for the two resource DLLs and most other
Sierra DLLs (98 files), 1.2.0.7 for `Outpost2.exe` and 5 mission DLLs, 1.2.0.8 and 1.2.0.9 for three each. The game
folder's other DLLs and programs (GOG's, OPU's, the network and IPX helpers, the uninstaller) are not the game's and
are not covered here.

How the game uses them: the cursor table in `Outpost2.exe` names 10 cursor groups of `out2res.dll` by id, alongside
the stock Windows cursors and the `.ani` files (see [exe-tables.md](exe-tables.md#cursors)); all 10 exist. The two
DLLs' bitmaps are the window chrome of the game (`out2res.dll`) and the menus' art (`op2shres.dll`); the command
pane's buttons are not resources but images in `op2_art.prt` (see [sprites.md](sprites.md)). Where the ones below
appear was measured in op2-rs (screenshots of the real game matched against the resources, and the shell's code at
the addresses given):

| Resource | Where |
|---|---|
| `out2res.dll` bitmap 184 | the 8-pixel divider left of the command pane |
| `out2res.dll` bitmap 186 | the 192x8 bar under the minimap |
| `out2res.dll` dialogs 241, 259, 262 | LOAD GAME (ten slots), PICK CONNECTION METHOD (multiplayer), the "Success" box after a won mission |
| `op2shres.dll` dialogs 137, 140, 142, 213, 215, 216, 219, 222, 269 | the main menu, new campaign options, preferences, about, story chapters, briefing / objectives / story text, a campaign's mission menu, colony games, tutorials |
| `op2shres.dll` bitmap 190 | the main menu's title art |
| `op2shres.dll` bitmap 225 | the 640x480 background behind the shell's dialogs |
| `op2shres.dll` bitmaps 400-411, 412-423 | the briefing pictures: 400 + the Eden missions completed, 412 + the Plymouth ones (`OP2Shell.dll` 0x13005DF6) |
| `op2shres.dll` strings 701-711 | the shell's text styles (colour, face, weight, size, alignment) |

**Unconfirmed:** where the rest appear. By their look, op2shres bitmaps 201-258 are the edges and fills of the
shell's owner-drawn buttons and 235 and 236 title-bar strips, and out2res bitmaps 187-208 the window buttons; that was
judged by eye only. A screenshot match of each would settle it.

## Finding the resources in a PE file

All numbers are little-endian.

1. The u32 at file offset 0x3C is the offset of `PE\0\0`. After the 4-byte signature comes the 20-byte COFF header
   (section count at +2, optional header size at +16), then the optional header.
2. The optional header's magic is 0x10B (PE32) or 0x20B (PE32+). Its data directories start 96 bytes into it for PE32
   or 112 for PE32+, preceded by their count (u32). Directory 2 is the resources: a u32 RVA and a u32 size, 8 bytes
   per directory.
3. Section headers (40 bytes each) follow the optional header: virtual size at +8, RVA at +12, raw size at +16, raw
   file offset at +20. An RVA inside a section's raw data is at file offset raw offset + (RVA − section RVA).

`out2res.dll` and `op2shres.dll` both have their resource directory at RVA 0x2000, the start of `.rsrc`.

### The resource tree

Three levels of directories: **type**, then **name** (or id), then **language**. Each directory is a 16-byte header
followed by its entries:

| Offset | Type | Field |
|---|---|---|
| 0 | u32 | characteristics (0) |
| 4 | u32 | timestamp |
| 8 | u16, u16 | major, minor version |
| 12 | u16 | number of named entries |
| 14 | u16 | number of id entries |
| 16 | 8 × entries | named entries first, then id entries |

Each entry is two u32:

| Field | High bit clear | High bit set |
|---|---|---|
| name | a numeric id (low 16 bits) | offset of a name: u16 length in UTF-16 units, then the text, no terminator |
| target | offset of a data entry (a leaf) | offset of the next level's directory (low 31 bits) |

Offsets in the tree are relative to the start of the resource directory. A leaf is 16 bytes: the data's **RVA**
(relative to the image, not to the tree), its size, a code page and a reserved u32. op2-formats reads exactly three
levels, refuses a subdirectory offset that doesn't point past the current one (so a malformed file can't loop) and
any data that runs past the end of the file. It treats a named language as language 0.

In the two DLLs every type and almost every name is a numeric id; the only string name is `op2shres.dll`'s
accelerator table `IDR_ACCELSHELL`. Code pages in the leaves are 0.

### Resource types

| Id | Type | Decoded by op2-formats |
|---|---|---|
| 1 | `CURSOR` | yes, through its group |
| 2 | `BITMAP` | yes |
| 3 | `ICON` | yes, through its group |
| 4 | `MENU` | yes (standard templates) |
| 5 | `DIALOG` | yes |
| 6 | `STRING` | yes |
| 9 | `ACCELERATOR` | no (raw bytes) |
| 12 | `GROUP_CURSOR` | yes |
| 14 | `GROUP_ICON` | yes |
| 16 | `VERSION` | yes (the string values) |
| 24 | `MANIFEST` | no (raw bytes) |

Other standard ids (7 `FONTDIR`, 8 `FONT`, 10 `RCDATA`, 11 `MESSAGETABLE`, 17, 19-23) are named but not decoded.
Any resource's raw bytes are available.

## Bitmaps

A `BITMAP` resource is a device-independent bitmap without the 14-byte file header of a `.bmp`: a
`BITMAPINFOHEADER`, the palette, then the pixels.

| Offset | Type | Field |
|---|---|---|
| 0 | u32 | header size: 40 (op2-formats refuses less, i.e. OS/2 bitmaps; a longer V4/V5 header is accepted) |
| 4 | i32 | width |
| 8 | i32 | height: positive = rows stored bottom-up, negative = top-down |
| 12 | u16 | planes (1) |
| 14 | u16 | bits per pixel: 1, 4, 8, 16, 24 or 32 |
| 16 | u32 | compression: 0 none, 1 RLE8, 2 RLE4, 3 bit fields |
| 20 | u32 | image size (may be 0 when uncompressed) |
| 24 | 2 × i32 | resolution (unused) |
| 32 | u32 | colours used: 0 means 2^bits for 1-, 4- and 8-bit images |
| 36 | u32 | colours important (unused) |

- **Palette:** right after the header, 4 bytes per colour in the order blue, green, red, unused.
- **Bit fields** (compression 3, 16 or 32 bits): with a 40-byte header, three u32 masks for red, green and blue follow
  the header and come before any palette; with a longer header the masks are its fields at offset 40. Without them,
  16-bit pixels are 5-5-5 (masks 0x7C00, 0x03E0, 0x001F) and 24- and 32-bit pixels are blue, green, red bytes.
  op2-formats scales each field to 0-255 with rounding.
- **Uncompressed pixels:** each row is padded to a multiple of 4 bytes. Within a byte, 1- and 4-bit pixels run from
  the most significant bits.
- **RLE8 / RLE4:** pairs of bytes, rows in file order (bottom-up unless the height is negative):

  | Bytes | Meaning |
  |---|---|
  | *n* > 0, *v* | *n* pixels of *v* (RLE4: *v*'s two nibbles alternating, high first) |
  | 0, 0 | end of row |
  | 0, 1 | end of bitmap |
  | 0, 2, *dx*, *dy* | move right *dx* and down *dy* (in file order) |
  | 0, *n* ≥ 3, *n* values | *n* literal pixels (RLE4: *n* nibbles), padded to an even number of bytes |

  Pixels a run skips keep index 0 in op2-formats; pixels past the row's end are dropped.

Measured:

| | Bitmaps | Formats | Sizes |
|---|---|---|---|
| `out2res.dll` | 36 | all 8-bit, uncompressed, 256-colour palette | 8 to 597 pixels wide (bitmap 184, 8x378, is the narrowest) |
| `op2shres.dll` | 67 | 31 8-bit, 22 8-bit RLE8, 13 24-bit, 1 4-bit | 1 to 730 pixels wide; the RLE8 ones include the 640x480 main menu background (190) and several 320x240 |

All 103 are bottom-up with 40-byte headers; colours used is 0 in 77 and 256 in the rest. The 36 bitmaps of
`out2res.dll` use 5 distinct palettes, one of them shared by 23 bitmaps and another by 10. The 24-bit ones in
`op2shres.dll` include eight of 1x26 pixels.

## Cursors and icons

A cursor or icon is two resources: a **group** (`GROUP_CURSOR` 12, `GROUP_ICON` 14) listing the available images,
and the images themselves (`CURSOR` 1, `ICON` 3), referred to by id.

The group starts with three u16: reserved (0), type (1 = icon, 2 = cursor) and the image count. Then one 14-byte
entry per image:

| Offset | Icon group | Cursor group |
|---|---|---|
| 0 | u8 width, u8 height (0 means 256) | u16 width |
| 2 | u8 colour count, u8 reserved | u16 height, doubled (it covers the image and its mask) |
| 4 | u16 planes | u16 planes |
| 6 | u16 bits per pixel | u16 bits per pixel |
| 8 | u32 size of the image resource | u32 size |
| 12 | u16 id of the `ICON` / `CURSOR` resource | u16 id |

An `ICON` resource is a DIB (as for bitmaps) whose height counts twice the image: the colour (XOR) image, then a
1-bit AND mask of the same size, rows padded to 4 bytes. A `CURSOR` resource is the same, preceded by two u16: the
hotspot x and y. op2-formats picks the group's largest image (by the width byte), decodes the colour image with its
palette and makes pixels whose mask bit is 1 transparent. It doesn't decode PNG-compressed images (newer icons) and
says so.

On a monochrome cursor, Windows draws a pixel whose mask bit is 1 and colour bit is 1 by inverting the screen; a
reader that only knows "transparent or colour" shows such pixels as transparent.

Measured: `out2res.dll`'s 10 cursor groups each hold one 32x32 1-bit image of 308 bytes (the 4-byte hotspot, a
40-byte header, a 2-colour palette, 128 bytes of image, 128 of mask); the group entries give width 32 and height 64.
`op2shres.dll`'s icon group holds a 32x32 and a 16x16 image, both 4-bit with 16 colours. `Outpost2.exe` has one icon.

## String tables

`STRING` resources come in blocks of 16. Block *b* (its resource id) holds strings (*b* − 1) × 16 to
(*b* − 1) × 16 + 15. Each string is a u16 length in UTF-16 units and that many units, no terminator; an empty slot is
a length of 0. Measured: only `op2shres.dll` has strings, 8 in blocks 44 and 45 (ids 701 to 711; id 701 names the
shell's font).

## Dialogs

A `DIALOG` resource is a `DLGTEMPLATE`, or a `DLGTEMPLATEEX` if it starts with the u16s 1 and 0xFFFF. Coordinates
are dialog units (a quarter of the font's average character width, an eighth of its height).

| `DLGTEMPLATE` | `DLGTEMPLATEEX` |
|---|---|
| u32 style | u16 version 1, u16 0xFFFF, u32 help id |
| u32 extended style | u32 extended style |
| | u32 style |
| u16 control count | u16 control count |
| i16 x, y, width, height | i16 x, y, width, height |
| menu: sz_Or_Ord | menu: sz_Or_Ord |
| window class: sz_Or_Ord | window class: sz_Or_Ord |
| title: UTF-16, NUL-terminated | title |
| if style has `DS_SETFONT` (0x40): u16 point size, font name | the same, with u16 weight, u8 italic, u8 charset between |

A **sz_Or_Ord** is a u16: 0 = none; 0xFFFF = an ordinal follows (u16); anything else is the first unit of a
NUL-terminated UTF-16 string.

Each control starts on a 4-byte boundary (from the start of the resource):

| Control | Control (ex) |
|---|---|
| u32 style | u32 help id |
| u32 extended style | u32 extended style |
| | u32 style |
| i16 x, y, width, height | i16 x, y, width, height |
| u16 id | u32 id |
| class: sz_Or_Ord | class: sz_Or_Ord |
| text: sz_Or_Ord | text: sz_Or_Ord |
| u16 count of creation data bytes, then the bytes | the same |

Class ordinals are the predefined window classes: 0x80 button, 0x81 edit, 0x82 static, 0x83 list box, 0x84 scroll
bar, 0x85 combo box. Other classes are named.

Measured: `out2res.dll` has 19 dialogs with 160 controls (88 buttons, 41 statics, 17 combo boxes, 12 edits, a list
box and one `SysListView32`, the common-controls list view, by name); `op2shres.dll` has 16 with 130 controls
(67 buttons, 51 statics, 6 list boxes, 4 combo boxes, 2 edits). All are plain `DLGTEMPLATE`s except one empty
`DLGTEMPLATEEX` in `op2shres.dll` (dialog 146, no title, no controls). All 35 set the font MS Sans Serif, 8 points.

## Menus

A standard `MENU` template starts with two u16: version 0 and the offset from there to the first item (0 here).
Each item is a u16 flags word, then, unless it is a popup (flag 0x10), a u16 command id, then its text as a
NUL-terminated UTF-16 string; an empty text with id 0 is a separator. A popup's items follow it. Flag 0x80 marks the
last item of its level. op2-formats flattens the menu to (depth, id, text) and refuses extended (`MENUEX`, version 1)
templates. Measured: `op2shres.dll`'s one menu (135) is a single popup with four commands and two separators.

## Accelerators

An `ACCELERATOR` resource is a list of 8-byte entries:

| Offset | Type | Field |
|---|---|---|
| 0 | u16 | flags: 0x01 the key is a virtual-key code (else a character), 0x02 don't flash the menu, 0x04 Shift, 0x08 Ctrl, 0x10 Alt, 0x80 last entry |
| 2 | u16 | key |
| 4 | u16 | command id (sent as `WM_COMMAND`) |
| 6 | u16 | padding (0) |

op2-formats doesn't decode them. Measured with a probe over the raw bytes:

| Table | Bytes | Entries | Use (measured in op2-rs) |
|---|---|---|---|
| `out2res.dll` 130 | 264 | 33 | the view's keys: Alt with ten letters and Alt+Shift+N, Ctrl+L, Ctrl+P, Ctrl+S, F1, F5-F12 with and without Ctrl, Home, Pause |
| `out2res.dll` 206 | 376 | 47 | unit keys: 0-9 alone, with Ctrl and with Shift (groups), H, Tab, Backspace, Space, the keypad's + and −, and the characters `+ - = _`; off while the chat line is open |
| `out2res.dll` 212 | 16 | 2 | while paused: Ctrl+P and Pause |
| `op2shres.dll` `IDR_ACCELSHELL` | 8 | 1 | F1 |

In every table the sizes are whole entries, the padding words are 0 and flag 0x80 is set on the last entry only.
Every key is a virtual key (flag 0x01, often with 0x02) except four plain-character entries in table 206. The game
runs table 212 while paused, else 130 and then 206, before any other key handling (op2-rs).

## Version information

A `VERSION` resource is a tree of records, each:

| Type | Field |
|---|---|
| u16 | length of the record, children included |
| u16 | length of the value (in bytes for binary, UTF-16 units for text) |
| u16 | type: 0 binary, 1 text |
| UTF-16, NUL-terminated | key |
| | padding to a 4-byte boundary |
| | value, padded to 4 bytes |
| | children |

The root, `VS_VERSION_INFO`, has a 52-byte binary value (`VS_FIXEDFILEINFO`, starting with the signature
0xFEEF04BD, then the structure version and the file and product versions as pairs of u32: major × 65536 + minor,
build × 65536 + revision). Its children are `StringFileInfo` (a table per language and code page, `040904b0` in
these files: US English, Unicode, holding text records such as `FileVersion`) and `VarFileInfo` (`Translation`).
op2-formats doesn't walk the tree: it scans for text records with a plain alphanumeric key and returns their key and value. The Sierra files all have the
same eight keys: `CompanyName`, `LegalCopyright`, `ProductName`, `ProductVersion`, `FileVersion`,
`FileDescription`, `InternalName`, `OriginalFilename`. op2-formats reads the exe's `FileVersion` this way when it
checks an install.

## Standalone files

### `.bmp`

A 14-byte file header, then a DIB as above:

| Offset | Type | Field |
|---|---|---|
| 0 | 2 bytes | `BM` |
| 2 | u32 | file size |
| 6 | 2 × u16 | reserved |
| 10 | u32 | offset of the pixels from the start of the file |

Pass the bytes from offset 14 to the DIB reader. The pixels start at the stated offset (for a 256-colour bitmap with
a 40-byte header, 14 + 40 + 1024 = 1078). Loose `.bmp` files in the install: OPU's `sprites/color.bmp` (a 1x1 8-bit
image), the tileset wells (see [tilesets.md](tilesets.md)) and `OP2_ART.BMP`, whose header doesn't describe its
contents (see [sprites.md](sprites.md#op2_artbmp)).

### `.ico` and `.cur`

The same group layout as a resource, but self-contained: three u16 (reserved, type 1 icon or 2 cursor, count), then
16-byte entries:

| Offset | Type | Field |
|---|---|---|
| 0 | u8, u8 | width, height (0 means 256) |
| 2 | u8, u8 | colour count, reserved |
| 4 | u16 | icon: planes; cursor: hotspot x |
| 6 | u16 | icon: bits per pixel; cursor: hotspot y |
| 8 | u32 | size of the image |
| 12 | u32 | file offset of the image |

The image is a DIB with its AND mask, as for resources, or a PNG. op2-formats' `parse_cur_file` decodes the first
entry. The game ships no `.cur` files and no `.ico` of its own; the three `.ico` files in the GOG folder are GOG's
(one is PNG only, which op2-formats refuses).

### `.ani` (animated cursors)

A RIFF file of type `ACON`:

| Chunk | Contents |
|---|---|
| `anih` | 36 bytes, nine u32: header size (36), frame count, step count, width, height, bits, planes (all 0 here), default display rate in jiffies (1/60 s), flags (bit 0: frames are icon/cursor files; bit 1: a `seq ` chunk gives the order) |
| `rate` | optional: one u32 per step, the display rate of that step in jiffies |
| `seq ` | optional: one u32 per step, the frame shown at that step |
| `LIST` `INFO` | optional: `INAM` title, `IART` author |
| `LIST` `fram` | the frames: one `icon` chunk each, holding a whole `.cur` (or `.ico`) file |

Without `seq `, step *k* shows frame *k*; without `rate`, every step lasts the default rate. op2-formats walks the
chunks recursively and decodes each frame with `parse_cur_file`.

Measured on the game folder's 17 files (`attack.ani` ... `waypoint.ani`, all named in the exe's cursor table):

- 2 to 8 frames, every frame a 32x32 1-bit cursor (a 326-byte `icon` chunk) with its own hotspot; within a file the
  hotspot is the same for every frame, except `repair.ani` and `norepair.ani`, whose hotspot moves.
- Default rate 10 jiffies in all. 8 files have a `rate` chunk (4 to 20 jiffies per step).
- 5 have a `seq ` chunk and flags 3 (the four 3-frame cursors play frames 0, 1, 2, 1; `waypoint.ani` plays 0, 1, 2,
  3, 2, 1); the other 12 have flags 1 and as many steps as frames.
- None has an `INFO` list.

## Making resources

Any resource compiler or editor writes these formats; the game loads its DLLs' resources by id, so a replacement
`out2res.dll` or `op2shres.dll` must keep every id and type the game asks for. A replacement `.ani` cursor is any
standard animated cursor with the same name in the game folder. Keep bitmaps at their original size and colour
depth to be safe: **Unconfirmed** whether the game copes with others; a run of the game with one bitmap replaced by
one of another size would settle it.

## Sources

- op2-formats `pe_res.rs` (the resource tree, DIB, RLE, cursor, icon, `.ani`, dialog, menu, string and version
  readers) and its `ui_media` test.
- The PE, resource, DIB, cursor, icon, animated cursor, dialog, menu, string table, accelerator and version layouts
  are Microsoft's published Win32 formats.
- `exe/mod.rs`: the cursor table that names the cursor resources and `.ani` files.
- op2-rs (bufke's reimplementation): its measurements of where the bitmaps and dialogs appear and of which
  accelerator table the game uses when.
- Measured: every DLL and the exe in the GOG game folder, the 17 `.ani` files, the `.ico` files and OPU's
  `color.bmp`, with op2-formats and a probe that tallied resource types, bitmap formats, palettes, dialog controls
  and cursor headers; the accelerator tables' entries with a probe over their raw bytes.
