# Tables in `Outpost2.exe`

Some of what Outpost 2 needs isn't in any data file: which sound file each SoundID plays, how fast each track type
crosses each cell type, which animation each unit type draws, what each factory can build. These live in the game's
executable, as data tables or as code that fills tables at start-up. op2-formats reads them from the player's own
`Outpost2.exe` at runtime, so no copy of the game's data is kept in source. Read by op2-formats' `exe/mod.rs` (the PE
reader, the tables and their checks) and `exe/code.rs` (a small x86 walker for tables built by code); the unit
animation tables are in `units.rs` and described in [sprites.md](sprites.md#which-animation-a-unit-draws).

Addresses are virtual addresses in GOG's build 1.2.0.7 (image base 0x400000). Unless a section says otherwise, the
numbers here were measured with op2-formats on that file.

## Builds

| Build | Bytes | SHA-1 | op2-formats |
|---|---|---|---|
| GOG 1.2.0.7 | 1,144,832 | `1c7d63a05beae42af441e0955d0a9420fae2f85e` | the build the addresses come from |
| CD copy updated by OPU 1.4.1 | 1,144,320 | `9ac2ef155fb5a86444eb2bbd0fadd3667ffa4ec6` | read if its tables pass the checks |
| CD releases and their official updates | | 6 known SHA-1s (`03daaf61...`, `278c7b99...`, `2f813205...`, `6430e83f...`, `793874d4...`, `a28b474c...`) | refused: install OPU first |

- GOG's file carries version resource 1.2.0.7 (`FileVersion` and the fixed version), product "Outpost 2", company
  "Sierra On-Line Inc.". Its PE timestamp is 0x34493B6A.
- OPU 1.4.1's installer brings each of the six CD builds to the same file (its patches are named
  `<before>_<after>.dat` by SHA-1); it has no patch to or from GOG's. op2-formats can apply that patch to a copy and
  keeps it as `OPU/base/Outpost2.exe`, which its file search finds before the game folder's.
- **Unconfirmed:** whether the OPU-updated CD build keeps its tables at GOG's addresses. Nobody has measured it here
  or in op2-rs (no CD copy); the size and SHA-1 come from OPU's installer as recorded in op2-formats' comments. Running
  op2-formats' table checks on a CD copy updated by OPU would settle it.

The SHA-1 is not the gate. A file that isn't GOG's is read anyway, and each table's own check (its shape, value
ranges and code patterns) decides: if any check fails, op2-formats returns one error naming the file, the build (by
SHA-1) and the table; if all pass, the tables are used with a warning that the build is untested. One of the
library's tests flips the last byte of GOG's file: the tables still pass and the warning appears; breaking the first
byte of the ambient table then fails with that table named.

## PE basics

Only enough of the PE format to turn a virtual address into file bytes. All little-endian.

| Where | Type | Field | GOG |
|---|---|---|---|
| file 0x3C | u32 | offset of the PE header (`e_lfanew`) | 0x80 |
| PE + 0 | 4 bytes | `PE\0\0` | |
| PE + 4 | u16 | machine | 0x14C (i386) |
| PE + 6 | u16 | section count | 8 |
| PE + 20 | u16 | optional header size | 224 |
| PE + 24 | u16 | optional header magic | 0x10B (PE32) |
| PE + 24 + 28 | u32 | image base | 0x400000 |
| PE + 24 + optional size | 40 × count | section table | |

Each section table entry:

| Offset | Type | Field |
|---|---|---|
| 0 | 8 bytes | name, NUL-padded |
| 8 | u32 | virtual size |
| 12 | u32 | virtual address, relative to the image base |
| 16 | u32 | size of raw data in the file |
| 20 | u32 | file offset of the raw data |
| 36 | u32 | characteristics (0x20 code, 0x02000000 discardable, 0x80000000 writable) |

GOG's sections:

| Section | Address | Virtual size | Raw size | File offset |
|---|---|---|---|---|
| `.text` | 0x401000 | 0xCDBBD | 0xCDC00 | 0x400 |
| `.rdata` | 0x4CF000 | 0xE754 | 0xE800 | 0xCE000 |
| `.data` | 0x4DE000 | 0xA4F94 | 0x11E00 | 0xDC800 |
| `.idata` | 0x583000 | 0x1AB4 | 0x1C00 | 0xEE600 |
| `DSEG` | 0x585000 | 0x1E0D | 0x2000 | 0xF0200 |
| `.tls` | 0x587000 | 0x10 | 0x200 | 0xF2200 |
| `.rsrc` | 0x588000 | 0x8E4 | 0xA00 | 0xF2400 |
| `.reloc` | 0x589000 | 0x11538 | 0x11600 | 0xF2E00 |

To read *n* bytes at address *a*: find the section with address ≤ *a* < address + max(virtual size, raw size); the
bytes are at file offset raw offset + (*a* − address), provided (*a* − address) + *n* ≤ raw size. Past the raw size a
section is zeros when loaded: `.data` has 0x11E00 bytes in the file but 0xA4F94 in memory, so most of its addresses
(from 0x4EFE00 on) hold nothing until the game's code writes there. op2-formats refuses a read that runs past a
section's raw data, and bounds-checks every read: the file is untrusted input.

Strings are `char*` pointers (absolute addresses) to NUL-terminated ASCII; op2-formats reads at most 255 bytes.

The version resource in `.rsrc` is read with the resource reader of [resources.md](resources.md).

## The tables

Each table below gives its address, layout and the check op2-formats applies. "Read by" addresses are where the
game's code uses the table, as cited in op2-formats.

### SoundIDs

0x4E0170, 252 (0xFC) `char*`, one sound file name per SoundID; the loader at 0x47E015 stops at 0x4E0560. Check: every
entry points at a string longer than 4 characters ending in `.wav` (any case). SoundID 0 is `bld_fal3.wav`. See
[sound.md](sound.md#soundids).

### SongIDs and music volume

- **Music tracks:** 0x4E2700, 26 `char*`, one track name in `op2.clm` per SongID (SongID 0 is `EDEN11`). Check: 1 to
  8 characters, all upper-case letters or digits.
- **Music volumes:** 0x4D4AD8 in `.rdata`, 6 i32, indexed by `MusicVol` in the ini, in hundredths of a decibel.
  Check: the first is 0, each is lower than the one before, the last is above −10000. Measured 0 down to −1500.

### Ambient animation sequence

0x4E40E0, 11 rows of 5 bytes, read by Building::DoAmbientAnimation (0x483600, 0x48364D). Each row is a delay and four
columns; a structure type names its column, and a column's value *v* picks the type's animation slot 5 + *v*. Rows
0 and 10 are end markers starting with 0xFF. Check: both markers present; each inner row's delay is below 16 and
each column value below 5. (How structures use it: [sprites.md](sprites.md).)

### Tube and wall graphics

0x4EA778, 16 u32, read by 0x4A1D80 and 0x4A1C40. Indexed by a neighbour mask (north 1, south 2, west 4, east 8), each
the position 0-15 of the tile within a terrain type's group of 16 wall or tube tiles. Check: every value below 16 and
each position used exactly once. Four tables at 0x4EA578 (gaining a neighbour) and four at 0x4EA678 (losing one),
0x40 bytes apart, give the new graphic of each neighbour of a tube built or removed; op2-formats doesn't read them,
and its tests check that all eight follow from this one (mask with the bit set or cleared). See
[tilesets.md](tilesets.md#special-tiles).

### Cell types

32 entries of 0x24 bytes at 0x4DEBA8 (TethysAPI `MapImpl::GetCellTypeInfo`), one per cell type (see
[maps.md](maps.md#cell-types)):

| Offset | Type | Field |
|---|---|---|
| 0x00 | `char*` | name |
| 0x04 | 5 × i32 | time cost per track type: Legged, Wheeled, Miner, Tracked, Hover; 4096 = normal, 0 = impassable |
| 0x18 | u32 | blocks line of sight: 0 or 1 |
| 0x1C | i32 | how readily the Blight spreads into it |
| 0x20 | u32 | minimap colour |

**Only entry 0 is in the file**; entries 1-31 are zeros there. A static initializer at 0x40B5F0 (straight-line code,
ending at 0x40BD5F) stores the rest when the game starts, so op2-formats walks that code (see
[Tables built by code](#tables-built-by-code)) and lays its stores over the file's bytes. Check: the initializer
stores no value the walk can't follow; every entry has a non-empty name; speeds are at most 0x10000; the sight flag is
0 or 1; the Blight value is at most 0x100. The names are the game's own, spelling included: `Fast Passible 1` ...
`Vents and Fumaroles` for 0-11, `zPad 12` to `zPad 20` for the unused 12-20, then `Dozed Area`, `Rubble`, the three
walls and `Tube0` to `Tube5`.

### Research efficiency

289 signed bytes from 0x4E0CD5, read at 0x43E4CC as `team × 15 + max scientists`. The table proper (team sizes 1 to
17) starts at index 19; the 19 bytes before it are the end of an unrelated format string. The game's index can reach
those bytes and run past the table's last row, so op2-formats takes the whole range it can address. Check: every byte
from index 19 is 1 to 32. Measured 7 to 13.

### Morale modifiers

0x4E1F10, 67 entries of 12 bytes: a `char*` name (`KID_DIES`, `ADULT_DIES`, ...), a value that is 0x80000000 until
the game reads `morale.txt`, and a percent flag (0 or 1). The loader converts the flagged values from percent to
1/256 (0x44E4E6). Check: every name non-empty, every value 0x80000000, every flag 0 or 1. 7 entries are flagged. See
[sheets.md](sheets.md).

### Savant queue priority

The priorities of SoundIDs 0xBC-0xE3 for the Savant's voice queue, one byte each at 0x56E82E. That address is in
`.data` past its raw size: **not in the file**. SoundManager's constructor (0x47DCE0, straight-line code) stores the
bytes one by one from registers it has loaded with constants, and op2-formats walks it like the cell type
initializer. The queue reads `0x56E7D0[id − 0x5E]` (0x47F180), 0x5E bytes before the stored range, so SoundID
0xBC + *k* gets byte *k*. Check: each of the 40 bytes is stored as a single byte and is 0, 1 or 2. Measured 7 zeros,
28 ones, 5 twos.

The constructor writes 134 bytes, 0x56E82E to 0x56E8B3: one per Savant SoundID 0x5E-0xE3, the table the queue was
meant to index. With the shifted read only the first 40 are ever reached: every id that gets to the queue is 0xE3 or
less. Status messages, the mission DLLs' `TethysGame::AddMessage` among them (0x478735), pass on to it only ids
0x5E-0xE3 (MessageLog::AddMessage, 0x439084; other ids play as plain sounds), and each of the game's other calls to AddSavantSound (0x47F010) passes a constant from 0x7B to 0xE3
(0x40341A ... 0x49C6AA). So ids 0xBC-0xE3 get bytes 0-39 and the other 94 bytes are never read; op2-formats takes
the 40.

### Armor values

0x4E1518, 6 u32: the damage factor out of 256 for each sheet armor class 0-5. Check: class 0 is 256, each is lower
than the one before, class 5 is above 0. Class 0 is 256 and class 5 is 40; an unknown class counts as 256 (no
armor).

### Production lists

What a Vehicle Factory, an Arachnid Factory and a Structure Factory offer, built by the list builder at 0x468250 from
three tables each:

| Factory | List | Chassis | Weapons |
|---|---|---|---|
| Vehicle Factory | 0x4E3390 | 0x4E33C8 | 0x4E33D8 |
| Arachnid Factory | 0x4E3368 | 0x4E3380 | 0x4E3388 |
| Structure Factory | 0x4E3408 | 0x4E3494 | 0x4E3498 |

All are i32 tables. In the list, a positive value is a MapID (a unit type); −*k* stands for the *k*-th chassis of
the chassis table, listed once for each weapon of the weapon table, in that order. The list and the weapon table end
with −100. The chassis table has no end marker (it runs into the weapon table), so it is read by index up to the
largest *k* the list uses. op2-formats reads up to 64 entries per table. Check: an end marker within 64 entries;
every MapID 1-114; every chassis index in range; a non-empty result.

Measured: the Vehicle Factory offers 43 entries (10 plain vehicles, then 3 tank chassis with 11 weapons each), the
Arachnid Factory 4, the Structure Factory 44 (the Guard Post once per weapon, 11 times). The builder checks each
entry against what the player has researched, and lists the Trade Center (MapID 0x20) only if a global flag is set;
op2-formats leaves both to the caller.

### Cursors

0x4E9878, 35 u32, loaded by 0x488910 into 0x56EA08. Each entry is one of:

| Entries | Value | Source |
|---|---|---|
| 0-2 | 0x7F00-0x7FFF | a Windows stock cursor (`IDC_WAIT`, `IDC_CROSS`, `IDC_ARROW`) |
| 3-34 | 1-0xFFFF | a cursor resource of `out2res.dll` by id |
| 3-34 | 0x10000 or more | a `char*` naming an animated cursor file (`.ani`) in the game folder |

Check: each entry fits its row and every file name ends in `.ani`. Measured: 3 stock cursors, 15 resource entries
(10 distinct ids, all present in `out2res.dll`) and 17 files. See [resources.md](resources.md).

### Direction table

0x4E9B30, 256 signed bytes: `trunc(64 × cos)` of each of the 256 headings (0 = east, clockwise). The sine table at
0x4E9C30 is the same shifted by 64. op2-formats reads it (`Exe::direction_table`) only for tests that compare it with
a computed table.

### Unit art and selection sounds

Which animations each unit type draws, by colony, state and direction, comes from code: each unit type's `Init`
stores animation ids into its type object. The method and the layouts are in
[sprites.md](sprites.md#which-animation-a-unit-draws); in outline:

- **Finding the types:** scan `.rdata` for vtables whose entries 2 and 3 are MapObjectType's shared Save and Load
  (0x444FE0, 0x445030). Entry 1 is GetCodeName, which must be exactly `mov eax, imm32; ret` (bytes `B8`, the string
  address, `C3`): the string is the type's sheet code name (`LIGHT_TANK`). Entry 4 is `Init`. An `Init` shared by
  several types stores no art and is skipped.
- **Init stores:** the [code walk](#tables-built-by-code) collects each `Init`'s constant stores relative to `this`.
  Vehicles store `int[4][16]` tables at +0x24C (Eden) and +0x34C (Plymouth); structures `int[10]` at +0x250 and
  +0x278 plus their ambient columns as bytes at +0x2A6 and +0x2A7. Every type stores its **selection sound**, a
  SoundID, at +0x1F0.
- **Turrets:** a weapon type's vtable entries 7-10 are its turret graphic selectors (Guard Post, Tiger, Panther,
  Lynx); the base class's own (0x447180-0x4471B0) mean none. Each selector is decoded up to its `ret`; the 16-entry
  tables it addresses in `.data` (displacements 0x4Exxxx), or the immediates it stores in the turret graphic global
  0x4EA7B8, give the turret animations.
- **Plain tables:** the Cargo Truck's chassis per cargo, through the jump table at 0x4065FC (each case's last
  `mov edx, imm32` is the Eden table and `mov esi, imm32` the Plymouth one); its dock and undock animations, four
  tables of 11 u16 from 0x4DEAE0, 0x18 apart; structure explosions by footprint, u32 at 0x4DEB34 indexed
  `width × 4 + height`.

Check: every `Init` decodes as code, no two weapons draw a turret on the same chassis, every selector is one the
reader understands, the Cargo Truck's tables are found, explosion ids are below 0x1000, and vehicles, structures and
turrets are all non-empty. Measured: 15 vehicle types, 38 structure types, 12 weapons with turrets and 53 selection
sounds. The Small Capacity Air Transport (`SCAT`) is skipped.

## Tables built by code

The cell types, the Savant priorities and the unit art exist only once the game's start-up code has run. op2-formats
doesn't execute anything; it decodes the function's x86 instructions (with the iced-x86 decoder) and follows which
registers hold known constants, collecting the stores whose value it knows. The rules (`exe::code::stores`):

- **Known values:** a register becomes known after `mov reg, imm`, after `xor reg, reg` (zero), and after `inc`,
  `dec`, `add reg, imm` or `sub reg, imm` on a known value. Any other instruction that writes a register, or part of
  it (implicit writes included: `cdq`, `mul`, string instructions), makes it unknown. A `call` makes every register
  unknown.
- **`this`:** ECX holds the object on entry (the `thiscall` convention). A register copied from ECX also counts as
  `this`; a `call` loses ECX, EAX and EDX, but a copy in a callee-saved register (EBX, ESI, EDI, EBP) survives.
- **Stores:** `mov [address], value` with no base or index register is recorded as an absolute store of its size.
  `mov [this + offset], value` is recorded as a dword or byte store at that offset (offsets 1 to 0x7FFFFFFF; byte
  values only from an immediate or AL, BL, CL or DL). A store whose value isn't known is counted as unknown; the cell
  type reader refuses the table if there is any. Stores through an index register are ignored.
- **Branches:** a forward branch inside the function carries what is known to its target; where paths meet, a
  register stays known only if every path agrees on its value. Code after an unconditional jump or a `ret` is
  resumed only at a branch target; code that nothing reaches is skipped. Backward branches are not followed.
- **The end:** the function ends at a `ret` or a jump out of it (a tail call) that no branch seen so far jumps past,
  or at an `int3` after a `ret` (the padding between the compiler's functions), or after a byte limit (0x1000 for the
  cell types, 0x400 for SoundManager, 0x1400 for an `Init`), or at an invalid instruction.

The functions these tables come from are straight-line or nearly so, which is why so simple a walk suffices; a
build compiled differently would most likely fail a check rather than give wrong values. Reading every table,
SHA-1 included, takes about 3 ms.

## Checking a reader

op2-formats' tests keep the SHA-1 of each table's debug text as read from GOG's build (11 digests: units, cells,
research, morale, Savant, sounds, the ambient and tube tables, armor, music, production lists, cursors), so a change
to the readers can't change a value unnoticed, while the source holds no game data. A reimplementation can compare
its own tables the same way.

## Sources

- op2-formats `exe/mod.rs` (PE reader, every table, its address, check and reading), `exe/code.rs` (the code walk),
  `units.rs` (unit art, selection sounds); the addresses and the functions that use them are cited there from
  disassembly of GOG's `Outpost2.exe` 1.2.0.7.
- Disassembly read for this document with an iced-x86 probe: the Savant queue (0x47F050), its callers and
  MessageLog::AddMessage (0x439070).
- TethysAPI (OutpostUniverse): the names `SoundID` and `MapImpl::GetCellTypeInfo`.
- OPU 1.4.1's installer (OutpostUniverse): the CD builds' SHA-1s and their patch, as recorded in op2-formats.
- Measured: GOG's `Outpost2.exe` with op2-formats: the PE and section headers, the version resource, every table's
  count and checks, the timing.
