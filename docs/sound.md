# Music and sounds (`op2.clm`, `.wav`)

Outpost 2 plays two kinds of audio: **music**, 26 long tracks packed into one archive, `op2.clm` (a *clump* file),
and **sound effects and voices**, ordinary RIFF `.wav` files kept in `sound.vol` and `voices.vol`. The game refers
to both by number: a *SongID* for music and a *SoundID* for everything else. The numbers become file names through
two tables in `Outpost2.exe` (see [exe-tables.md](exe-tables.md)). Read by op2-formats' `sound.rs`; the name tables
by `exe/mod.rs`.

Unless a section says otherwise, the numbers here were measured with op2-formats on GOG Outpost 2 with OPU 1.4.1.

## Where the files are

| File | Where | Contents |
|---|---|---|
| `op2.clm` | game folder | the 26 music tracks, 263,127,516 bytes |
| `sound.vol` | game folder | 97 sound effects, stored uncompressed |
| `voices.vol` | game folder | 135 voice lines (the Savant, the game's computer voice), uncompressed |
| `OPU/base/sounds/`, `OPU/base/voices/` | OPU | loose copies of the same 97 and 135 files, byte for byte identical |
| `music1.wav` | game folder | one more music piece, named in `OP2Shell.dll` (see below) |

The game finds a `.wav` by name through its usual search: OPU's loose files first, then the game folder, then the
`.vol` archives (see [volumes.md](volumes.md)). So a loose file in `OPU/base/sounds/` replaces a sound.

## `op2.clm`

A clump file is a header with one shared wave format, an index of tracks, then the tracks' raw PCM samples back to
back. All numbers are little-endian.

| Offset | Size | Field | Value in `op2.clm` |
|---|---|---|---|
| 0 | 32 | version string, NUL-padded | `OP2 Clump File Version 1.0`, then 0x1A, then NULs |
| 32 | 18 | `WAVEFORMATEX` (below) | PCM, mono, 22050 Hz, 16-bit |
| 50 | 2 | padding (not read) | 0 |
| 52 | 4 | u32 block size | 65,536 |
| 56 | 4 | u32 track count | 26 |
| 60 | 16 × count | index entries | |
| 60 + 16 × count | | sample data | |

op2-formats accepts a file whose version string starts with `OP2 Clump File`.

### Wave format

The standard Windows `WAVEFORMATEX`, shared by every track:

| Offset | Type | Field | Value |
|---|---|---|---|
| 32 | u16 | format tag | 1 (PCM) |
| 34 | u16 | channels | 1 |
| 36 | u32 | samples per second | 22050 |
| 40 | u32 | average bytes per second | 44100 |
| 44 | u16 | block align | 2 |
| 46 | u16 | bits per sample | 16 |
| 48 | u16 | extra format bytes (`cbSize`) | 0 |

### Index entry

| Offset | Type | Field |
|---|---|---|
| 0 | 8 bytes | track name, ASCII, NUL-padded (a name of 8 characters has no NUL) |
| 8 | u32 | file offset of the samples |
| 12 | u32 | length of the samples in bytes |

In `op2.clm`:

- The index ends at byte 476 and the first track starts there. Each track starts where the previous one ends and
  the last one ends at the end of the file: no gaps, no padding, no per-track header.
- Every track's length is a multiple of 65,536 bytes (137 to 173 × 65,536). Tracks run about 204 to 257 seconds;
  together they hold 263,127,040 bytes of samples, about 99 minutes.
- Names are mixed case as stored (`EDEN11`, `Plymth11`, `PLYMTH31`, `Static01`). The groups are `EDEN11`-`EDEN33`
  (6 tracks), `EP41`-`EP63` (8), `PLYMTH11`-`PLYMTH33` (7) and `STATIC01`-`STATIC05` (5).

A track's samples are plain PCM in the header's format; prepending a 44-byte RIFF header gives a playable `.wav`
(op2-formats: `Clm::wav`).

### How the game reads it

From the disassembly of the music player's open routine (0x450340) and its streaming timer (0x450CF0):

- It reads the first 76 bytes (the header and the first index entry), then the rest of the index: 60 + 16 × count
  bytes in all.
- It refuses the file unless the first 32 bytes match `OP2 Clump File Version 1.0` without regard to case, up to the
  string's NUL (so the 0x1A after it isn't compared; 0x4503E6); the block size at 52 is a multiple of 32,768
  (0x4503F6); and the format has tag 1, 1 channel, 22050 samples per second and block align 2 (0x45040E-0x450436).
  The bits per sample, the average byte rate and `cbSize` are not checked.
- It doesn't play with the header's format: the DirectSound buffer it creates has its own fixed format, PCM, mono,
  22050 Hz, 16-bit (0x45069F), and is 65,536 bytes long (0x45070A). So a file in any other format is either refused
  or played wrongly.
- Every one of the 26 SongID names (below) must be in the index, matched in its first 8 characters without regard to
  case (0x4504CC); one missing name and the file is refused.
- A track is streamed in chunks of 32,768 bytes, each read from the track's offset plus the position (0x450EF1,
  0x450F22). The position steps by 32,768 and the track ends when it reaches the track's length (0x450DC0). A length
  that isn't a multiple of 32,768 would play the bytes after the track to fill its last chunk, which is why the
  shipped lengths are whole blocks. Nothing else reads the block size.

A replacement `op2.clm` should keep the shipped format, the 26 names and lengths in whole multiples of 32,768 bytes.

### SongIDs

The game names its music in a table of 26 string pointers in `Outpost2.exe` (0x4E2700), one per SongID, in the same
order as the clump's index: SongID 0 is `EDEN11`, SongID 25 is `STATIC05`. The names in the exe are upper case while
the index stores `Plymth11` and `Static01`, so names are matched without regard to case (op2-formats:
`ClmIndex::entry`). The music volume setting (`MusicVol` in `outpost2.ini`) picks one
of six levels from another exe table (0x4D4AD8): 0, −500, −900, −1200, −1400 and −1500 hundredths of a decibel
(DirectSound's attenuation unit), loudest first.

### `music1.wav`

The game folder also has `music1.wav`, a standard RIFF file in the clump's format (PCM, mono, 22050 Hz, 16-bit),
10,588,142 bytes of samples (about 240 seconds). It is the menu music: `OP2Shell.dll`, the menu shell, opens it
through MCI's `waveaudio` device when `Music` in the `[Game]` section of `outpost2.ini` is 1 (0x13007C60) and reopens
it each time it ends, so it loops. Measured in op2-rs: it starts when the main menu or a mission menu opens and keeps
playing between menus, and stops for a movie and when a game starts. `Outpost2.exe` never names it; in a game the
music comes from `op2.clm`.

## `.wav` files

Standard RIFF WAVE files. The parts the game's files use:

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | `RIFF` |
| 4 | 4 | u32 size of the rest of the file (see below) |
| 8 | 4 | `WAVE` |
| 12 | 4 | `fmt ` |
| 16 | 4 | u32 chunk length: 16 |
| 20 | 16 | the first 16 bytes of `WAVEFORMATEX` (no `cbSize`) |
| 36 | 4 | `data` |
| 40 | 4 | u32 sample bytes |
| 44 | | samples, then a pad byte if the length is odd |

Chunks after the first are found by walking: each is a 4-byte tag and a u32 length, and an odd length is followed by
one pad byte. op2-formats' `wav_info` walks the chunks, takes the format from `fmt ` and stops at `data`; other
chunks are skipped.

Measured on all 232 files (the same set in the `.vol`s and in OPU's folders):

- **Every one is PCM, mono, 22050 Hz, 8-bit** (block align 1, 22050 bytes per second), with a 16-byte `fmt ` chunk
  directly followed by `data` at offset 36. 8-bit samples are unsigned (128 is silence), as in any 8-bit WAV.
- Sample data runs from 1,216 bytes (`beep2.WAV`, 0.06 seconds) to 164,288 bytes (`savnt101.wav`, 7.5 seconds).
- 25 files have an odd data length. Their pad byte is present, but their RIFF size counts only up to the last
  sample, so it is one less than the file's length minus 8. A reader should trust the chunk lengths and the file's
  end, not the RIFF size.
- One file, `sulv.wav`, has a `LIST` `INFO` chunk after the data.
- Names are lower case except for a few upper-case extensions (`acid2.WAV`, `beep2.WAV`); the exe's table names
  every file with `.wav`, so lookups ignore case.

### SoundIDs

`Outpost2.exe` has a table of 252 string pointers (0x4E0170, read by the sound loader at 0x47E015), one file name per
SoundID; TethysAPI's `SoundID` enumeration names the same numbers. Some examples: SoundID 0 is `bld_fal3.wav`,
0x10 `guardsel.wav`, 0xE4 `mis_fire.wav`, and 0xFB, the last, `medexp1.wav`.

- The 252 entries name 230 distinct files; a few files serve two ids (`bigexp1.wav` is 0x48 and 0xEE).
- Every name resolves: 118 ids to sound effects and 134 to voice lines. The voice lines are the contiguous range
  0x5E-0xE3 (`savnt212.wav` to `savnt277.wav` and the `savantNN.wav` lines in between).
- Two files in the archives have no SoundID: `temp.WAV` in `sound.vol` and `savnt217.wav` in `voices.vol`.

A unit type's selection sound is a SoundID too, stored by the type's `Init` code (see
[exe-tables.md](exe-tables.md#unit-art-and-selection-sounds)).

### The Savant's timing

The game queues the Savant's voice lines so they don't talk over each other. It waits for a line by a length it
computes once, at load, from the file's data chunk (SoundManager's length table at 0x56DC00, filled by 0x47F300):

```
milliseconds = data bytes × 1000 / 22050      (integer division)
```

which is the playing time of 22 kHz 8-bit mono, whatever the file's real format (op2-formats: `exe::savant_ms`; e.g.
SoundID 0xBC, `savnt238.wav`, waits 1950 ms). A replacement voice line in another format would be waited for by its
byte count, not its playing time.

Queued lines also have a priority: the 40 SoundIDs 0xBC-0xE3 get one byte each, 0, 1 or 2 (7, 28 and 5 of them),
which the exe builds in code at start-up (see [exe-tables.md](exe-tables.md#savant-queue-priority)).

## Making a clump or a sound

- **A clump:** the 60-byte header (version string, the wave format, the six bytes as above, the count), the index,
  then the samples in index order. Offsets are absolute; the first is 60 + 16 × count. Keep the track names the exe
  asks for, or SongIDs that name a missing track will find nothing. Keep 22050 Hz 16-bit mono to be safe.
- **A sound:** 22050 Hz, 8-bit, mono PCM WAV with the 44-byte header above, named as the SoundID table names it,
  placed loose in `OPU/base/sounds/` or `OPU/base/voices/`. Write the RIFF size correctly (file length − 8, pad byte
  included).
- Voice lines keep their Savant timing only if they stay 22 kHz 8-bit mono.

## Sources

- op2-formats `sound.rs` (the clump header, `wav_info`, `Clm::wav`), `exe/mod.rs` (the SoundID and SongID tables,
  music volumes, `savant_ms`, the Savant priorities).
- OP2Utility (OutpostUniverse): `ClmFile`, the clump layout op2-formats follows.
- TethysAPI (OutpostUniverse): the `SoundID` numbering.
- Disassembly of GOG's `Outpost2.exe` 1.2.0.7 at the addresses given, as cited in op2-formats' comments; the clump
  loader (0x450340), buffer set-up (0x450680) and streaming timer (0x450CF0) read for this document with an iced-x86
  probe, and `OP2Shell.dll`'s music routine (0x13007C60).
- op2-rs (bufke's reimplementation): its measurements of the menu music in the running game.
- Measured: the header and index of `op2.clm`; every `.wav` in `sound.vol`, `voices.vol`, OPU's `sounds` and `voices`
  and the game folder; the exe's SoundID, SongID and volume tables, all with op2-formats.
