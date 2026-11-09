# Cutscenes (`.avi`: Cinepak video, MS ADPCM audio)

Outpost 2's cutscenes are 18 AVI files in the game folder: 640x480 Cinepak video at 15 frames per second with
stereo Microsoft ADPCM sound. This document covers the parts of AVI the files use and both codecs in enough detail
to write a decoder. Read by op2-formats' `avi.rs` (the container), `cinepak.rs` (video) and `adpcm.rs` (audio), whose
output matches a reference decoder (ffmpeg's) bit for bit on `CEP.avi` (op2-formats' `ui_media` test).

Unless a section says otherwise, the numbers here were measured with op2-formats on the 18 files of GOG Outpost 2
with OPU 1.4.1.

## The files

| File | Bytes | Frames | Length | Keyframes |
|---|---|---|---|---|
| `CEP.avi` | 14,507,510 | 207 | 13.8 s | 2 |
| `CES.avi` | 41,161,868 | 1070 | 71.3 s | 6 |
| `CPP.avi` | 11,395,454 | 166 | 11.1 s | 2 |
| `CPS.avi` | 44,415,622 | 1091 | 72.7 s | 7 |
| `I.avi` | 88,070,256 | 2370 | 158.0 s | 14 |
| `IE.avi` | 56,993,422 | 842 | 56.1 s | 11 |
| `IP.avi` | 36,704,534 | 543 | 36.2 s | 6 |
| `MEBL.avi` | 9,761,466 | 201 | 13.4 s | 1 |
| `MEBW.avi` | 15,279,308 | 317 | 21.1 s | 2 |
| `MEF.avi` | 51,278,542 | 1148 | 76.5 s | 9 |
| `MEUL.avi` | 24,967,180 | 328 | 21.9 s | 3 |
| `MEUW.avi` | 23,346,984 | 320 | 21.3 s | 1 |
| `MPBL.avi` | 13,026,636 | 327 | 21.8 s | 1 |
| `MPBW.avi` | 15,192,104 | 328 | 21.9 s | 2 |
| `MPF.avi` | 51,428,074 | 1162 | 77.5 s | 9 |
| `MPUL.avi` | 24,925,394 | 328 | 21.9 s | 2 |
| `MPUW.avi` | 23,345,364 | 334 | 22.3 s | 1 |
| `e4.avi` | 20,471,704 | 334 | 22.3 s | 1 |
| | **566,271,422** | | | |

Length is frames / 15. Keyframes are the frames that decode without the ones before them (see
[Keyframes](#keyframes)); every file's first frame is one.

`OP2Shell.dll`, the menu shell, plays them all and names all 18 (in upper case, in a table it builds at
0x13008429). `Outpost2.exe` names only `I.AVI`, and only to search for it and split its path into folder and name
(0x4977DF); it plays no movie itself. Which event plays which file, measured in op2-rs (the shell's code at the
addresses given, and runs of the real game):

| File | Played |
|---|---|
| `I.avi` | at start-up, when `IntroAVI` in `outpost2.ini` is 1 (0x13008AD7) |
| `IE.avi`, `IP.avi` | on starting a new Eden or Plymouth campaign, before mission 1 (0x13008384) |
| `MEBW.avi`, `MEUW.avi` | an Eden mission won: `MEBW` after missions 1, 2, 3, 5, 8 and 10, `MEUW` after 6, 7, 9 and 11 |
| `e4.avi` | Eden mission 4 won |
| `MEBL.avi`, `MEUL.avi` | an Eden mission lost: `MEBL` for missions 1, 2, 3, 5, 8, 10 and 12, `MEUL` for 4, 6, 7, 9 and 11 |
| `MPBW.avi`, `MPUW.avi` | a Plymouth mission won: `MPBW` after 1, 2, 3, 5, 7, 8 and 11, `MPUW` after 4, 6, 9 and 10 |
| `MPBL.avi`, `MPUL.avi` | a Plymouth mission lost: `MPBL` for 1, 2, 3, 5, 7, 8, 11 and 12, `MPUL` for 4, 6, 9 and 10 |
| `MEF.avi`, `MPF.avi` | the Eden or Plymouth mission 12 won: the campaign's ending (0x13008B73) |
| `CEP.avi`, `CES.avi`, `CPP.avi`, `CPS.avi` | a colony game won, chosen by the letters in the mission DLL's name (0x13008BD4) |

The win and loss movies come from four tables built at 0x13008963 and indexed by the campaign's progress. The `B`
(base) and `U` (unit) split follows the mission DLL's units-only flag, except Eden mission 7, which gets the unit
movies. `ShowMovies=0` in `outpost2.ini` skips all of them (0x130084F1).

All 18 files have the same structure, the same stream formats and the same codec settings; only the frame data and
counts differ. 17 of them start with a small first frame of the same size and structure (20,286 bytes, codebooks of
3 entries; the bytes differ between files); `e4.avi`'s first frame is an ordinary keyframe.

## AVI container

AVI is a RIFF file: chunks of a 4-byte tag, a u32 little-endian length, the data, and one pad byte when the length is
odd (not counted in the length). A `LIST` chunk's data is a 4-byte list type followed by more chunks. The files'
layout:

```
RIFF 'AVI '
  LIST 'hdrl'                       8850 bytes in every file
    avih                            main header, 56 bytes
    LIST 'strl'                     stream 0: video
      strh                          stream header, 56 bytes
      strf                          BITMAPINFOHEADER, 40 bytes
      JUNK                          4120 bytes of padding
    LIST 'strl'                     stream 1: audio
      strh                          56 bytes
      strf                          WAVEFORMATEX + MS ADPCM extra, 50 bytes
      JUNK                          4120 bytes
    LIST 'odml'
      dmlh                          248 bytes; its first u32 is the total frame count
  JUNK                              1362 bytes
  LIST 'movi'                       the stream data
    01wb 00dc 01wb 00dc ...
  idx1                              the index
```

The RIFF size is exact in all 18 (file length − 8). Despite the `odml` list, none is an OpenDML (over 1 GiB) file:
there is one `RIFF` chunk and no `AVIX`. No file uses `rec ` lists inside `movi`.

### `avih` (main header)

All u32, little-endian:

| Offset | Field | Value |
|---|---|---|
| 0 | microseconds per frame | 66666 (15 fps) |
| 4 | max bytes per second | varies (557,407 to 1,141,792) |
| 8 | padding granularity | 0 |
| 12 | flags | 0x110: has index (0x10), interleaved (0x100) |
| 16 | total frames | the video frame count |
| 20 | initial frames | 0 |
| 24 | streams | 2 |
| 28 | suggested buffer size | 0 |
| 32 | width | 640 |
| 36 | height | 480 |
| 40 | reserved (4 × u32) | 0 |

### `strh` (stream header)

| Offset | Type | Field | Video | Audio |
|---|---|---|---|---|
| 0 | 4 bytes | stream type | `vids` | `auds` |
| 4 | 4 bytes | handler | `cvid` | four zero bytes |
| 8 | u32 | flags | 0 | 0 |
| 12 | u16, u16 | priority, language | 0 | 0 |
| 16 | u32 | initial frames | 0 | 0 |
| 20 | u32 | scale | 1 | 1024 |
| 24 | u32 | rate | 15 | 22311 |
| 28 | u32 | start | 0 | 0 |
| 32 | u32 | length | frames | ADPCM blocks |
| 36 | u32 | suggested buffer size | largest frame | 11264 |
| 40 | u32 | quality | 10000 | 10000 |
| 44 | u32 | sample size | 0 | 1024 |
| 48 | 4 × i16 | frame rectangle | 0, 0, 640, 480 | 0 |

A stream runs at rate / scale units per second: 15 video frames, or 22311 / 1024 = 21.79 audio blocks.

### `strf` for the video: `BITMAPINFOHEADER`

| Offset | Type | Field | Value |
|---|---|---|---|
| 0 | u32 | header size | 40 |
| 4 | i32 | width | 640 |
| 8 | i32 | height | 480 |
| 12 | u16 | planes | 1 |
| 14 | u16 | bits per pixel | 24 |
| 16 | 4 bytes | compression | `cvid` (Cinepak) |
| 20 | u32 | image size | 90,900 |
| 24 | 4 × u32 | resolution, colours used, colours important | 0 |

### `strf` for the audio: `WAVEFORMATEX` with the ADPCM extra

| Offset | Type | Field | Value |
|---|---|---|---|
| 0 | u16 | format tag | 2 (`WAVE_FORMAT_ADPCM`) |
| 2 | u16 | channels | 2 |
| 4 | u32 | samples per second | 22050 |
| 8 | u32 | average bytes per second | 22311 |
| 12 | u16 | block align | 1024 |
| 14 | u16 | bits per sample | 4 |
| 16 | u16 | extra bytes (`cbSize`) | 32 |
| 18 | u16 | samples per block (per channel) | 1012 |
| 20 | u16 | coefficient pairs | 7 |
| 22 | 7 × (i16, i16) | coefficients | the standard set (see [MS ADPCM](#ms-adpcm-audio)) |

### `movi`: the stream data

Each chunk's tag is the stream number as two decimal digits plus a type: `00dc` is a compressed video frame of
stream 0, `01wb` audio data of stream 1. (AVI also defines `db`, uncompressed video, and `pc`, a palette change;
these files have neither.) op2-formats collects each stream's chunks by walking `movi` directly, in file order,
without using the index.

The chunks interleave audio ahead of video:

- The first chunk is `01wb` with 11 ADPCM blocks (11,264 bytes, about half a second of sound).
- Then `00dc` and `01wb` strictly alternate, each audio chunk holding 1 or 2 whole blocks (1,024 or 2,048 bytes),
  which keeps the delivered sound up to 0.55 s ahead of the frame being shown.
- The file ends with 7 to 9 `00dc` chunks in a row, played out on the sound already delivered.

Every audio chunk is a whole number of blocks. Every video chunk holds exactly one Cinepak frame (2,674 to 87,720
bytes); no file has an empty (repeat-the-last-frame) video chunk, though a reader should treat one as such.

### `idx1`: the index

16 bytes per chunk of `movi`, in file order:

| Offset | Type | Field |
|---|---|---|
| 0 | 4 bytes | chunk tag (`00dc`, `01wb`) |
| 4 | u32 | flags: 0x10 = keyframe |
| 8 | u32 | offset of the chunk's tag, counted from the `movi` list type (the first chunk is at 4) |
| 12 | u32 | data length |

Every audio entry has the keyframe flag; video entries have it exactly on the frames that are
[keyframes](#keyframes) by the definition below (all 18 files agree). A player seeking to time *t* can start from the
last flagged frame before *t*; op2-formats instead tests the frame data itself (`cinepak::is_keyframe`), which needs
no index.

## Cinepak video

Cinepak codes each frame as a few horizontal **strips**. Each strip has two **codebooks** of 256 entries: V1 and V4.
An entry is a 2x2 block of pixels (four luma values and one shared chroma pair). The picture is coded in 4x4-pixel
**blocks**, and each block is either:

- **V1**: one V1 entry, each of its four pixels scaled up to a 2x2 square; or
- **V4**: four V4 entries, one per 2x2 quarter of the block.

Inter frames may also **skip** a block, keeping the previous frame's pixels there. Codebooks persist from frame to
frame, and chunks can replace them entirely or update selected entries.

All multi-byte numbers inside Cinepak data are **big-endian**.

### Frame header (10 bytes)

| Offset | Type | Field | In the game's files |
|---|---|---|---|
| 0 | u8 | flags | 0 on keyframes, 1 on all other frames (see below) |
| 1 | u24 | frame size, header included | always the chunk length |
| 4 | u16 | width | 640 |
| 6 | u16 | height | 480 |
| 8 | u16 | strip count | 3 in every frame |

Flag bit 0 decides where a strip's codebooks start from (see [Codebook persistence](#codebook-persistence)).

### Strip header (12 bytes)

| Offset | Type | Field | In the game's files |
|---|---|---|---|
| 0 | u8 | strip id | 0x10 (intra) or 0x11 (inter) |
| 1 | u24 | strip size, header included | |
| 4 | u16 | y1 | 0 |
| 6 | u16 | x1 | 0 |
| 8 | u16 | y2 | 160 |
| 10 | u16 | x2 | 640 |

The files give every strip y1 = 0 and y2 = 160: y2 is the strip's height, not an absolute row, and the strips stack
top to bottom (rows 0-159, 160-319, 320-479). op2-formats reads y2 as the height and ignores y1, x1 and x2. The strip
id doesn't change how a strip is decoded: an inter (0x11) strip may code every block. In the files the first strip of
a keyframe is 0x10 and every other strip is 0x11.

After the header, a strip is a sequence of **chunks**, each with a 4-byte header: a u8 chunk id and a u24 size
(header included). A decoder skips chunk ids it doesn't know.

### Chunk ids

| Id | Contents |
|---|---|
| 0x20 | V4 codebook, full, colour |
| 0x21 | V4 codebook, selective update, colour |
| 0x22 | V1 codebook, full, colour |
| 0x23 | V1 codebook, selective update, colour |
| 0x24-0x27 | the same four, greyscale |
| 0x30 | vectors: every block coded, V1 or V4 |
| 0x31 | vectors: blocks may be skipped |
| 0x32 | vectors: every block V1 |

So for a codebook chunk, bit 0 of the id means selective, bit 1 means V1 (clear: V4) and bit 2 means greyscale. The
vectors chunk ends the strip: op2-formats stops at it.

Counts over all 18 files (3 strips per frame): 0x20 and 0x22 appear once per keyframe (in its first strip), 0x21 and
0x23 in every other strip, 0x30 in the strips of keyframes, 0x31 in all other frames, and 0x32 only in the small
first frame (its second and third strips, in place of 0x30). No file uses greyscale books.

### Codebook entries

A colour entry is 6 bytes, a greyscale entry 4:

| Byte | Field |
|---|---|
| 0-3 | Y0 Y1 Y2 Y3: luma of the top-left, top-right, bottom-left and bottom-right pixel, unsigned |
| 4 | U, signed (i8) |
| 5 | V, signed (i8) |

Each pixel is converted to RGB with the entry's shared U and V:

```
R = clamp(Y + 2V)
G = clamp(Y − U/2 − V)        U/2 rounds toward zero
B = clamp(Y + 2U)
clamp(x) = min(max(x, 0), 255)
```

A greyscale entry has R = G = B = Y. This integer conversion is the one that matches the reference decoder's RGB24
output exactly; convert once when the codebook is loaded, not per pixel.

A **full** codebook chunk (0x20, 0x22, 0x24, 0x26) holds entries for indices 0, 1, 2, ... in order until the chunk
ends; it may hold fewer than 256 (the 17 small first frames' books hold 3 entries), and entries past the end keep
their old values. A **selective** chunk (0x21, 0x23, 0x25, 0x27) has, before each run of 32 indices, a u32 bit mask:
bit 31 for the first index of the run down to bit 0 for the last. A set bit is followed by that index's entry; a
clear bit leaves the entry as it was. The mask for the next 32 indices comes right after the last entry of the
current run. Parsing stops at the chunk's end. A full update of all 256 colour entries is 1,540 bytes with its header;
a selective update of all 256 is 1,572.

### Vectors

The strip's blocks are visited in raster order: rows of blocks 4 pixels high from the strip's top, and in each row,
blocks 4 pixels wide from the left (160 blocks per row, 40 rows per strip here). Each block reads, in this order:

1. **0x31 only:** one flag bit. 0 = skip the block (its pixels stay as they were); 1 = code it.
2. **0x30 and 0x31:** one flag bit. 0 = V1, 1 = V4. (0x32 reads no bits: every block is V1.)
3. **V1:** one byte, an index into the strip's V1 book. **V4:** four bytes, indices into the V4 book for the
   top-left, top-right, bottom-left and bottom-right quarter.

Flag bits come from u32 words, most significant bit first. A new word is read from the chunk at the moment a bit is
needed and the previous word is used up, so flag words sit between the index bytes, wherever the 33rd, 65th, ... bit
is first needed. A skipped block uses one bit; a coded block in 0x31 uses two.

Drawing a block at (bx, by):

- **V1**, entry e: pixel e.Y0 fills (bx..bx+1, by..by+1), Y1 fills (bx+2..bx+3, by..by+1), Y2 fills (bx..bx+1,
  by+2..by+3) and Y3 fills (bx+2..bx+3, by+2..by+3).
- **V4**, entries a, b, c, d: a's four pixels are the 2x2 at (bx, by), b's at (bx+2, by), c's at (bx, by+2) and d's
  at (bx+2, by+2), each entry's pixels in its Y0-Y3 order.

Pixels outside the picture are not drawn (not reached at 640x480).

### Codebook persistence

The decoder keeps a V1 and a V4 book for each strip index (op2-formats allows 32 strips). Before decoding strip *i*
(*i* > 0) of a frame whose flag bit 0 is **clear**, it copies strip *i* − 1's books (as they are after that strip's
own updates) into strip *i*; the strip's chunks then update them. When the bit is **set**, each strip starts from its
own books as the previous frame left them. Strip 0 always starts from its own books. Books start out all zeros.

In the game's files keyframes have the bit clear: their first strip loads full books and the next two strips start
from those and apply selective updates. All other frames have the bit set.

### Keyframes

A frame decodes without the frames before it when every strip codes all of its blocks (vectors 0x30 or 0x32, never
0x31), the strips cover the full height, and every strip's V1 and V4 books are fully defined within the frame: by a
full codebook chunk, or by inheriting them from the strip above (flag bit 0 clear) when that strip's were. Selective
updates on top don't matter then. op2-formats' `cinepak::is_keyframe` tests exactly this. In the 18 files it marks 80
keyframes, the same frames `idx1` flags. Strip ids are no guide: keyframes after the first frame are one 0x10 strip
and two 0x11 strips.

## MS ADPCM audio

Microsoft ADPCM codes 16-bit samples as 4-bit differences from a linear prediction, in fixed-size blocks that each
start afresh. The cutscenes use 1,024-byte blocks of 1,012 stereo sample frames.

### Block layout

For *c* channels (here 2), a block starts with a header of 7 × *c* bytes, each field given for channel 0, then
channel 1:

| Bytes | Field |
|---|---|
| *c* × u8 | predictor index into the coefficient table (0-6) |
| *c* × i16 | initial step size (delta) |
| *c* × i16 | sample 1, the more recent of the two starting samples |
| *c* × i16 | sample 2, the older one |

Then 4-bit codes, two per byte, **high nibble first**; with two channels the nibbles alternate left, right, left,
right. A 1,024-byte stereo block is a 14-byte header and 1,010 bytes = 2,020 nibbles = 1,010 coded samples per
channel, plus the 2 header samples: 1,012 sample frames per block. In general

```
samples per block = (block align − 7 × channels) × 2 / channels + 2
```

which the format's extra field states too.

### Decoding

Output, per channel: sample 2, then sample 1, then one sample per nibble. For each nibble *n* (0-15) of a channel:

```
signed     = n − 16 if n ≥ 8, else n                       (−8..7)
predicted  = (sample1 × coef1 + sample2 × coef2) / 256      integer division, rounding toward zero
new        = clamp16(predicted + signed × delta)
sample2    = sample1
sample1    = new
delta      = max(16, (ADAPT[n] × delta) >> 8)
```

with `clamp16` to −32768..32767, (coef1, coef2) the pair the block's predictor index picks, and the step adaptation
table indexed by the unsigned nibble:

```
ADAPT = 230 230 230 230 307 409 512 614 768 614 512 409 307 230 230 230
```

The coefficient table is the one in the format's extra bytes; the cutscenes carry the standard seven pairs:

| Index | coef1 | coef2 |
|---|---|---|
| 0 | 256 | 0 |
| 1 | 512 | −256 |
| 2 | 0 | 0 |
| 3 | 192 | 64 |
| 4 | 240 | 0 |
| 5 | 460 | −208 |
| 6 | 392 | −232 |

(op2-formats falls back to this set when a format lists fewer than 7 pairs.) Every predictor index 0-6 occurs in the
cutscenes' blocks.

A block's 1,012 sample frames last 1,012 / 22050 = 45.9 ms, which is where the stream rate comes from: 22050 × 1024 /
1012 = 22311 bytes per second. The audio runs within 0.1 s of the video's length in every file. `CEP.avi` decodes to
302,588 sample frames (299 blocks).

## Making a cutscene

The shell doesn't decode the files itself: it opens each with MCI's `avivideo` device (0x130014B0), gives it a child
window of its own over the 640x480 shell window, and plays it with notification. Decoding is Windows' (Video for
Windows), so a movie plays if the system has its video and audio codecs. The shell asks the device for the movie's
source rectangle and centres a window of that size in the shell window (0x13001410), so a movie of another size is
shown unscaled in the middle. Measured in op2-rs: the shipped movies are shown 1:1. Other codecs and sizes were not
tried; Wine and each Windows version decide which codecs are present. Matching the shipped files is the safe choice: 640x480, 15 fps, Cinepak (`cvid`) with
3 strips of 160 rows, stereo MS ADPCM at 22050 Hz with 1,024-byte blocks, audio interleaved about half a second
ahead, and an `idx1` index with keyframes flagged.

## Sources

- op2-formats `avi.rs` (container walk, stream headers and formats), `cinepak.rs` (decoder, keyframe test),
  `adpcm.rs` (decoder); its `ui_media` test checks the decoded frames and samples of `CEP.avi` against hashes of a
  reference decoder's output.
- The AVI RIFF layout, `BITMAPINFOHEADER`, `WAVEFORMATEX` and MS ADPCM's block format and tables are Microsoft's
  published multimedia formats; Cinepak's bitstream as implemented by op2-formats follows the long-standing open
  decoders (libavcodec's among them).
- Disassembly of GOG's `OP2Shell.dll` (the movie table at 0x13008429, the MCI player at 0x130013B0-0x130016FF)
  and `Outpost2.exe` (0x4977DF), read with an iced-x86 probe.
- op2-rs (bufke's reimplementation): its measurements of which movie the shell plays after which event, from runs of
  the real game and the shell's code.
- Measured: all 18 `.avi` files of the GOG install, with op2-formats and a probe that walked their RIFF chunks,
  strip and chunk headers, ADPCM block headers and `idx1` entries.
