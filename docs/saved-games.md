# Saved games (`.op2`)

A saved game holds the complete running state of a game: the per-player unit statistics, the map with every unit
on it, the mission DLL's state and triggers, each player's economy, research, and the random number generator. The
game writes and reads it as one flat stream of sections with no length headers, so a reader has to walk every
section in order. op2-formats doesn't read saved games: `Map::parse` refuses a header whose saved-game flag is set.

There are no saved games in the GOG + OPU 1.4.1 install measured here. The measurements below come from one file,
`DEMO300.OP2` (317,441 bytes), which the GOG `Outpost2.exe` (1.2.0.7) wrote itself at tick 300 of the shell's attract
demo, saved by a debugger script with no save dialog, so the file's preview block is empty. Everything stated as
measured was checked on that file with a small op2-formats probe that walks the sections and ends exactly at the
file's last byte. The layout of each section, and what the game does with it, was read in `Outpost2.exe`'s raw
disassembly (GOG build); the addresses are given in the text. The save is `GameImpl::SaveGame` (0x48A1F0), the load
`GameImpl::LoadGame` (0x48A3E0), and they handle the sections in the same order. Fields that neither the file nor the
code settles are marked **Unconfirmed**.

## Where the game keeps them

The save and load dialogs have ten slots, `SGAME0.OP2` to `SGAME9.OP2` (format string `SGAME%d.OP2` at 0x4E0614,
filled with the slot number 0-9), and `SGAME10.OP2` (0x4E0628) is a small index of the slots' descriptions (see
[The slot index](#the-slot-index-sgame10op2)). `Outpost2.exe` contains no other saved-game names. The stock game puts
all eleven files in the folder that holds `Outpost2.exe`: the dialog builds the paths from the exe's drive and
directory (`GetModuleFileNameA` split at 0x470F70, copied at 0x416F78). OPU 1.4.1 moves them to `OPU\saves\` in the
game folder (measured in op2-rs: the dialogs pass `...\OPU\saves\SGAMEn.OP2` to the save and load code).

## Layout

All numbers are little-endian. "u32" is 4 bytes, "i32" signed. No padding anywhere.

| Section | Size | In `DEMO300.OP2` |
|---|---|---|
| [Signature](#signature) | 25 | 0x00000 |
| [Preview block](#preview-block) | 0x1086C | 0x00019 |
| [Unit type table](#unit-type-table) | 115 × 0x1E0 = 0xD7A0 | 0x10885 |
| [Map block](#map-block) | varies | 0x1E025 |
| [Message log](#message-log) | 12 + 0x4C × count | 0x49C13 |
| [Game block](#game-block) | 0x484 | 0x49C1F |
| [Mission block](#mission-block) | varies | 0x4A0A3 |
| [Player blocks](#player-blocks), seven | varies | 0x4ADDB |
| [Research](#research) | 40 + 2 × techs | 0x4D60F |
| [Day, night and minimap](#day-night-and-minimap) | 24 | 0x4D749 |
| [Hot-key groups](#hot-key-groups) | varies | 0x4D761 |
| [View positions](#view-positions) | 0x48 | 0x4D791 |
| [Random number state](#random-number-state) | 8 | 0x4D7D9 |
| [Lava and Blight spreaders](#lava-and-blight-spreaders) | 2 × 16 | 0x4D7E1 |
| end of file | | 0x4D801 |

The first three sections have fixed sizes, so the map block always starts at 0x1E025.

### Signature

25 bytes: the ASCII text `OUTPOST 2.00 SAVED GAME`, then 0x1A and 0x00. The same 25 bytes are in `Outpost2.exe`
(GOG build, file offset 0xD5380, address 0x4D6380). The load reads 25 bytes and compares them with that text as a C
string (0x48A413), so all 25 must match; on a mismatch it returns failure before it touches the running game (the
game is torn down only after the check, at 0x48A453). The dialogs make the same check (0x48A190) before they show a
slot's preview.

Saves written under OPU 1.4.1 start with a different text of the same length, `OUTPOST2 1.4.0-OPU SAVE` (measured in
op2-rs), so the stock exe refuses them by this check.

### Preview block

0x1086C bytes that the Load and Save dialogs show as the slot's description and picture. A save from the dialog
writes them with 0x419090; a save with no dialog writes 0x1086C zero bytes (0x4192E0, chosen at 0x48A262), which is
what `DEMO300.OP2` holds.

| Offset | Size | Field |
|---|---|---|
| 0x00 | 4 | checksum of the picture, also the key that encodes it |
| 0x04 | 4 | save time: the C library's `time()` (0x4C2690), seconds since 1970 |
| 0x08 | 4 | 0 |
| 0x0C | 80 | the description typed in the dialog, NUL-terminated (read from the edit box with `EM_GETLINE`, 0x418B28) |
| 0x5C | 0x10810 | picture: 260 × 130 pixels, 16 bits per pixel, top row first |

The picture is the minimap. The save dialog points a 260 × 130 drawing surface at the block's picture bytes, draws
the map into it scaled to fit and centred, then the units (0x418BA0 to 0x418D19). The dialog shows it through a DIB
header it builds at 0x416D51: width 260, height −130 (top row first), 16 bits, `BI_RGB`, which Windows reads as
5-5-5.

The first 0x5C bytes are also the slot's entry in the slot index. The checksum is the 32-bit sum of the picture's
0x4204 u32 words, each with one bit cleared: the bit numbered by the slot (0-9) (0x4190A3). Each picture word is then
XORed with the checksum (0x419139); the description and the first three words aren't encoded. When the dialog
reads a slot (0x4191B0) it decodes the picture with the file's own checksum, recomputes the sum with its slot's bit,
and reports a failure if the sums differ or the third word isn't 0. Both callers ignore the result (0x41721B,
0x418061), so a file copied to another slot fails the check and still shows its picture. The dialog draws a black
picture for a slot whose index entry has checksum 0 (0x4177D1). Loading a game skips the block: 0x4192B0 reads it
into a scratch buffer.

### Unit type table

115 records of 0x1E0 bytes, one per unit type id (map id) 0 to 114, in id order. The save walks the game's table of
115 type objects (0x4E1348 to 0x4E1514, 0x446780); each type writes a u32 from its +0x1E4, then 0x1DC bytes from its
+0x08 (0x444FE0; the load, 0x445030, reads them back to the same places):

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | the type's research topic: an index into the tech tree, see below |
| 4 | 7 × 0x44 | the type's statistics for each player, player 0 first |

These are the live values, after research upgrades. Each player's 0x44 bytes, with the offsets the tech file parser
writes its property names to (table at 0x4D5C4C):

| Offset | Size | Field |
|---|---|---|
| 0x00 | 4 | `Hit_Points` |
| 0x08 | 4 | `Armor`, stored as the armor value (the parser turns the class into a value) |
| 0x0C | 4 | `Common_Required` |
| 0x10 | 4 | `Rare_Required` |
| 0x14 | 4 | `Build_Time` |
| 0x18 | 4 | `Sight_Range` |
| 0x1D | 1 | `Improved` (the parser writes a u32 here) |
| 0x24 | 4 | `Move_Speed` (vehicles) or `Power_Required` (structures) |
| 0x28 | 4 | `Turn_Rate`, `Concussion_Damage` or `Workers_Required` |
| 0x2C | 4 | `Penetration_Damage` or `Production_Rate` |
| 0x30 | 4 | `Rate_Of_Fire` or `Storage_Capacity` |
| 0x34 | 4 | `Production_Capacity` |
| 0x38 | 4 | `Storage_Bays`; a vehicle's cargo capacity |
| 0x3C | 8 | the topics that upgraded this block, two u32 (measured in op2-rs) |

The same offset serves several names because each type uses only the ones that apply to it. The bytes at 0x04, 0x1C
and 0x1E to 0x23 are **Unconfirmed** (no tech file property writes them; reading the code that uses them would
settle it). Measured in `DEMO300.OP2`: in record 1 (Cargo Truck) player 0's block reads, as 17 u32 values, 400, 0,
180, 800, 0, 750, 4, 0, 0, 4, 8, 0, 0, 0, 1000, 0, 0: the `BIG_TRUCK` row of `vehicles.txt` (hit points 400, armor
class 1 as the value 180, common and rare ore 800 and 0, build time 750, sight 4, move 4, turn rate 8, cargo 1000). Eight player blocks differ from their player 0 block: record 1 player 1, record 3 player 2,
record 5 players 1 and 2, records 60 to 62 for player 1 or 2, and record 114 player 6.

The first word is 0 for 21 records (type 0 among them) and otherwise 3 to 137. It is the 1-based position of the
type's research topic in the tech tree sorted by tech id, which is the order the game keeps its techs in (the
[research](#research) section saves them in that order). Measured: for all 15 vehicle types of `sheets.vol`'s
`vehicles.txt` the word is the rank of the row's research topic among the 137 tech ids of `sheets.vol`'s
`ply_tek.txt` (Cargo Truck 41 = 03604, ConVec 42 = 03605, Spider 3 = 02099, Tiger 103 = 08201). The game reads it as
an index into its tech list (0x472D2B to 0x472D58). What index 0 means is **Unconfirmed**: the research section
doesn't save entry 0, which is probably "no topic"; reading how the tech loader fills entry 0 (0x472940) would settle
it.

**Correction to the community notes:** they put 116 records of 0x1E0 right after the signature, with the `.map`
layout following at 0xD999. The file and the code show 115 records after the preview block, and the map at 0x1E025.

### Map block

Written by the same routine as a `.map` file (0x4357F0), called with the saved-game flag set (0x48A297). The terrain
part is the [`.map` layout](maps.md) up to and including the version tag after the terrain types, with these
differences:

- The saved-game flag (offset 4 of the block) is 1.
- The tile words carry the running state: unit index (bits 16-26), lava, the Blight, walls and buildings.
- **No tile groups.** With the flag set the routine goes on with unit data, then the version tag once more
  (0x4359AC to 0x435ADE).

Measured: the demo's mission is `ademo2.dll`, which names `eden04.map`. The save's map block is byte for byte
`eden04.map` (from OPU's `maps/campaign/`) from the start to the first trailing tag, except the flag and 17 tile
words: 16 cells hold a unit index (exactly the 16 slots of the live vehicles), and two cells show other mappings (13
became 268 and 8 became 267). Width (log 2, 6), height (64), the 512 tileset slots, the clip rect (32, 0, 95, 62), the
2017 mappings and the 4 terrain types are the map's own. Copying the block to the first trailing tag, clearing the
flag, adding the tag again and an empty tile group list (two u32 zeros) gives a `.map` that op2-formats'
`Map::parse` reads.

After the first trailing tag:

| Size | Field | In `DEMO300.OP2` |
|---|---|---|
| 4 | live unit count | 20 |
| 4 | highest unit slot used | 46 |
| 4 | free-slot ring: write position | 42 |
| 4 | free-slot ring: read position | 16 |
| 4 | unit record size, 0x78 | 0x78 |
| 4 | waypoint block count *b* | 32 |
| 4 | free waypoint block count *f* | 16 |
| *b* × 512 | waypoint blocks | |
| *f* × 4 | free waypoint block numbers | 2, 0, 1, 5, 18, 3, 22, ... 31 |
| 4 | first unit's slot | 9 |
| 4 | last unit's slot | 17 |
| 1023 × 0x78 | unit slots 1 to 1023 | |
| 1024 × 4 | free-slot ring, only when its two positions differ | present here |
| 4 | version tag again | 0x1011 |

The unit header is four fields of the map object (+0x3C to +0x48), then the constant 0x78. The game keeps 1025
records of 0x78 bytes: slot 0 is the head of the unit list and slot 1024 its tail; slots 1 to 1023 hold units. A new
map fills the whole array with 0xEAEAEAEA and sets each slot's second u32 to −1 (0x435688). The first and last
unit's slots are the head's next and the tail's previous links.

A destroyed unit's slot goes on the free-slot ring at the write position, which then advances modulo 1024
(0x439C20). A new unit takes the oldest ring entry that was freed more than 128 ticks ago (0x439A60 to 0x439AF2),
or else the slot after the highest used one, up to 1023. The ring holds slot numbers in the file (pointers in
memory), and the save writes it only when its read and write positions differ (0x435A76).

The waypoint blocks are a pool of 512-byte blocks that the game commits 16 at a time (0x446B60), so *b* is a multiple
of 16. The free list is saved head first as block numbers, and the blocks as raw bytes (0x446C20); a free block's
first u32 is a raw pointer to the next free one. What a used block holds is **Unconfirmed** (a save with vehicles on
long routes, read against their paths, would settle it).

The save turns each live record's pointers into slot numbers, writes the 1023 records in one piece and turns them
back (0x435B00, 0x435CD0). A unit record:

| Offset | Size | Field |
|---|---|---|
| 0x00 | 4 | live: the type id (the save puts it in place of the record's class pointer, 0x435B9F); freed: 0x004D0498 |
| 0x04 | 4 | the next unit's slot; −1 marks a free slot |
| 0x08 | 4 | the previous unit's slot; in a freed slot, the tick it was freed (0x439C7B) |
| 0x0C | 4 | the next slot in the owner's unit list (see [player blocks](#player-blocks)), −1 for none |
| 0x10 | 4 | the slot's own number (0x439B01, 0x439B43) |
| 0x14 | 4 | x in pixels, internal coordinates |
| 0x18 | 4 | y in pixels; the unit list is kept sorted by it (0x439B53) |
| 0x1D | 1 | the owner, low 4 bits |
| 0x34 | 4 | vehicles: waypoint block number, −1 for none |
| 0x38 | 4 | vehicles: waypoint block number, −1 for none |
| 0x44 | 4 | flags: bit 1 (0x2) puts the object on its owner's vehicle list, bit 2 (0x4) and bit 6 (0x40) on the other two (0x43A0E0) |

The rest of the 0x78 bytes is **Unconfirmed**. A freed record keeps 0x004D0498, the base class pointer its
destructor leaves (0x439C21). Two class functions (0x44B5E0 and 0x474390, in 19 class tables) turn +0x34 and +0x38
into block numbers before the save.

Measured in `DEMO300.OP2`: following the next-unit links from the first unit's slot visits all 20 live slots in
ascending y and ends with 1024 after the last unit's slot. The 20 live units are 16 vehicles (types 1, 3 and 5) and
four objects of types 61 and 62. In 12 of the 16 vehicles the cell at (x / 32 − 32, y / 32) holds the vehicle's slot
as its unit index (the others are between cells). The 16 vehicles' +0x34 words are 16 different block numbers, exactly
the 16 blocks not on the free list; their +0x38 words are all −1. Of the other 1003 slots, 977 are never-used slots
(every u32 0xEAEAEAEA except the −1) and 26 are freed units, whose third u32 holds a game tick (250, 178, ...). The
ring's read and write positions (16 and 42) bound exactly 26 entries, the number of freed slots.

The community notes list the unit header, the waypoint blocks, the two slot numbers and the 1023 records, but miss the
free-slot ring.

### Message log

| Size | Field |
|---|---|
| 4 | time of the last message (Windows `timeGetTime`, set at 0x4390DA; −1 for none) |
| 4 | ring position of the oldest message |
| 4 | message count *n* |
| *n* × 0x4C | messages, oldest first |

The log is a ring of 64 messages of 0x4C bytes; the save writes the *n* that are in use, from the oldest (0x4392E0).
`DEMO300.OP2` has −1, 0, 0. The layout of a 0x4C-byte message is **Unconfirmed** (a save with messages in the log
would settle it).

### Game block

0x484 raw bytes of the game's global state (`GameImpl`, at 0x56EA98), written and read whole (0x48A2C2, 0x48A4A5).
Known fields:

| Offset | Field | In `DEMO300.OP2` |
|---|---|---|
| 0x48 | daylight moves (game option 0x12; measured in op2-rs) | 0 |
| 0x4C | daylight everywhere (game option 0x13; measured in op2-rs) | 0 |
| 0x50 | game speed × 4: the speed setting 1-10 times 4 (0x4891B2) | 20 |
| 0x68 | number of players (`TethysGame::NoPlayers`, 0x477F00) | 3 |
| 0x84 | the tick (`TethysGame::Tick`, 0x477EF0) | 300 |
| 0x90 | the local player (`TethysGame::LocalPlayer`, 0x477F10) | 0 |
| 0xB4 | the mission DLL's name, NUL-terminated | `ademo2.dll` |

A load sets the game's frame rate from the saved speed (0x48A5DB), so a loaded game runs at the speed it was saved
at. The other bytes are **Unconfirmed**.

### Mission block

| Size | Field |
|---|---|
| 4 | magic 0xAEDE0034 |
| 4 + *n* | mission DLL name, u32 length then the bytes, no NUL (`ademo2.dll`) |
| 4 + *s* | the DLL's save region, u32 size then the bytes: only when the DLL exports `GetSaveRegions` and it gives a buffer |
| 0x30 | six pairs of i32, one per player 0-5 |
| varies | script objects, below |
| 4 | magic 0xAEDE0034 again |

The save (0x402C60) writes the DLL's file name and extension. When the game loads a DLL it calls its
`GetSaveRegions` export, if there is one, with a {buffer, size} pair to fill (0x402BCC); with no export, or no
buffer, the size word is left out. The load (0x402DC0) finds and loads the named DLL first, then refuses the save if
the stored size differs from the size the DLL gives now. `ademo2.dll` has no region.

The first word of each pair is the id of a `CombatBase` script object that the game makes for each computer player
(0x402F80), or −1 for a human player; the game sets the second to 0. `DEMO300.OP2` has (−1, −1) for player 0 and
(0, 0), (1, 0), (2, 0), (3, 0), (4, 0) for players 1 to 5. The second word's use is **Unconfirmed** (a save of a
mission with an AI base would show whether it changes).

Script objects (the mission's triggers, groups and other script stubs, 0x47AB60):

| Size | Field |
|---|---|
| 4 | class count, 29 |
| varies | the class names, each a u32 length then the bytes (`AttackedTrigger`, `BuildGroup`, ... `_Trigger`) |
| 4 | last object id given out, 14 |
| varies | one record per object: u32 id, u32 class number, then the class's own data |
| 4 | 0xFF, the end |

Object ids are 0 to 254: the game gives out the next free id after the last one (0x47B439), so 0xFF in the id's place
ends the list. The game keeps its objects newest first (0x47B4A0) and the save walks the list backwards, so the
records come oldest first. The load refuses the save unless the class count and every name, in order, match the
running game's (0x47AD1D, 0x47AD74). Each class writes its own data through its save function; those payloads are
**Unconfirmed**. Measured: the magic, name, class list, last id and the 0xFF end marker right before the closing
magic. The probe found the end of this block by its two magics rather than by decoding every record.

### Player blocks

Seven, player 0 first (0x490300; the load is 0x4904D0):

| Size | Field |
|---|---|
| 4 | magic 0xE3D4C5B4 |
| 0x4C8 | the player's state (`PlayerImpl`), raw |
| 16 × (14 + *len*) | 16 queued commands: a 14-byte header, then *len* bytes, where *len* is the i16 at header offset 4 |
| 12 | three unit slot numbers, −1 for none |
| 4 | magic 0xEDA34CEF |

The queued commands are the player's ring of 16 command packets of 0x71 bytes each (op2-rs measured how the game
fills it); the save writes only each packet's used part. The three slots are the heads of the player's three unit
lists, which run on through the records' +0x0C: objects with flag bit 2 (`PlayerImpl` +0xC18), the vehicles (flag
bit 1, +0xC1C) and objects with flag bit 6 (+0xC20) (0x43A0E0). What kinds the first and third lists hold is
**Unconfirmed** (a save with structures would settle it).

Known fields of the 0x4C8 bytes (the ones op2-rs's mission host reads and writes, which agree with the file):

| Offset | Field | In `DEMO300.OP2`, players 0 to 3 |
|---|---|---|
| 0x00 | player number | 0, 1, 2, 3 |
| 0x10 | food stored | 0 in all |
| 0x20 | common ore | 0 in all |
| 0x24 | rare ore | 0 in all |
| 0x28 | human player | 1, 0, 0, 0 |
| 0x2C | Eden | 0, 1, 0, 1 |
| 0x30 | colour | 0, 1, 2, 3 |

The save and the load both set +0xA0 (a flag that makes the game recompute the player's totals, measured in op2-rs)
for each player in the game (0x48A212, 0x48A5BC). The other bytes are **Unconfirmed**.

Measured: all seven blocks parse with both magics in place; player 0's is 0x5CC bytes (16 more bytes of commands),
the others 0x5BC. Player 1's vehicle list starts at slot 6 and runs 6, 5, 4, 2, 1 through the records' +0x0C; player
2's starts at slot 18.

### Research

| Size | Field | In `DEMO300.OP2` |
|---|---|---|
| 4 | tech count *t* | 137 |
| 4 | highest tech id | 8999 |
| 32 | tech file name, NUL-padded | `PLY_TEK.TXT` |
| *t* × 2 | one i16 per tech, in the order of tech ids | |

The highest tech id is the mission's tech level × 1000 + 999 (0x47348D), the limit the game passes to its tech file
loader (0x472940); 8999 is tech level 8. 137 is the
number of techs in `ply_tek.txt`. The game keeps its techs in a list whose entry 0 isn't saved; the save writes entries
1 to *t* (0x473580), and that order is the tech id order (see the [unit type table](#unit-type-table)). Each i16 is
the tech's mask of players who have it (bit *n* for player *n*; tested at 0x472D58).

The load (0x473630) loads the named tech file again with the saved highest id, refuses the save unless the file
gives exactly *t* techs, and then sets each tech's mask from its i16.

### Day, night and minimap

Six u32 (0x4206A0; the load is 0x420760): 0, 0, 2856, 1, 0, 0 in `DEMO300.OP2`.

| Word | Field |
|---|---|
| 0 | globe view: while it is set, dragging on the minimap turns word 2 (0x41FB7B) |
| 1 | daylight position, 16 bits; it gains 4 each game cycle while daylight moves (0x489ED3; measured in op2-rs) |
| 2 | globe rotation, 16 bits (0x41FBAB) |
| 3 | set to 1 when a game starts (0x41F8F7) |
| 4 | a minimap setting the load applies (0x4207FE) |
| 5 | a minimap setting the load applies when it changes (0x420827) |

The meanings of words 3 to 5 are **Unconfirmed** (perhaps day and night on, the minimap zoom and inverted colours;
saves made with each minimap toggle changed would settle them).

### Hot-key groups

Ten groups, each a u32 count *n* then *n* u16 unit numbers, then two u32 (0x47B670). With no groups set the section
is 48 bytes (measured). In memory each group is a count byte and up to 32 unit numbers (0x41 bytes per group); setting
a group takes the units out of the other groups (0x47BC70).

The two closing words are the state of the keys that cycle through the units of one type (0x47B990): first a sort
key of the unit cycled to last (its type id × 65536 + its byte +0x2C × 256 + the high four bits of +0x1D), then −1
if that was a unit of the list for flag bit 2, else 0. `DEMO300.OP2` has 0 and 0.

### View positions

0x48 bytes (0x4084A0): eight stored view positions, then the current one, each an i32 x and y in pixels. An unused
position has x = −1 (0x4084F8). The load clamps each one to the map (0x407990). `DEMO300.OP2` has −1 in all eight
and the view at (1232, 0).

### Random number state

Two u32: 0 and 0x04822412 in `DEMO300.OP2`. They are the game's simulation random number generator (0x56BE20, the
one `TethysGame::GetRand` draws from), saved as they are (0x46EF90) and restored as they are by the load (0x46EFF0).
Each draw shifts both words with feedback (0x46F000) and returns their XOR. Word 0 stays 0 once it is 0, and
`TethysGame::SetSeed` (0x46EFB0) makes it 0 for any seed below 0x10000, so the attract demo's seed is below 0x10000;
the seed itself is **Unconfirmed** (the demo mission's `SetSeed` call would settle it).

### Lava and Blight spreaders

Two records of four u32, lava first (0x54EFE0, the object `TethysGame::SetLavaSpeed` sets; the Blight is 0x57B7D8,
`SetMicrobeSpreadSpeed`), written by 0x49E8F0. `DEMO300.OP2` has {0x515, 0, 0, 0} and {0x00710071, 0, 0, 0}, where
neither spreads.

| Word | Field |
|---|---|
| 0 | the spreader's own random state; each try picks a cell from it (0x49DD0B) |
| 1 | set to 0 when the speed is set (0x49E31D) |
| 2 | **Unconfirmed** |
| 3 | cells tried per spread step: speed × the map's cell count / 10000 (0x49E324) |

Setting a speed also resets word 0 to 0x00710071, the Blight's value in the demo.

## The slot index (`SGAME10.OP2`)

No such file here; the layout is from the dialog's code (0x416F9C to 0x417099, 0x418260).

| Size | Field |
|---|---|
| 4 | the slot last saved or loaded, 0-9 |
| 10 × 0x5C | each slot's entry: the first 0x5C bytes of its save's preview block (checksum, time, 0, description) |

The dialog opens the file for update and reads 4 + 10 × 0x5C bytes. If the file is missing, or shorter, or the slot
number isn't 0-9, it creates the file again and writes the slot number and then 10 × 0x1086C bytes from its entry
array: a fresh index is 676,084 bytes, and everything after byte 924 is unrelated dialog memory. When the dialog
closes with a change it rewinds and writes 4 + 10 × 0x5C bytes over the start, leaving the rest. Reading a slot's
save also refreshes its entry from the file when the checksums differ or the entry has no description (0x4191EB).

## Edge cases

- An overwritten slot keeps stale bytes after the last section when the new save is shorter: the stock game opens the
  file with `OPEN_ALWAYS` (0x4ABB44) and closes it without truncating (0x4ABAF0). A reader should stop after the
  spreaders, not at the end of the file. Whether OPU's file handling does the same is **Unconfirmed** (overwrite a
  save with a shorter one under OPU and compare sizes).
- The save stores raw memory in many places: pointers into the mission DLL, the 0xEAEAEAEA fill of unused unit
  slots, freed records, free waypoint blocks. A save made by a debug build or under a debugger can hold different
  garbage from one made normally.
- The file depends on the mission DLL (its name, class list and save region) and on the tech file: the load refuses
  it when they don't match.

## Making one

Only the game can write a valid saved game; the state it holds is the game's whole memory layout, not a description
of a level. A tool can read the map block out of one, as above, or read the header fields (tick, mission name, tech
file) after walking the sections in order. To confirm the **Unconfirmed** fields, save games from the real game in
situations that exercise them (messages, script objects with saved data, a mission with a save region, active lava
and Blight, the minimap settings) and walk them the same way.

## Sources

- Measured: `DEMO300.OP2`, written by the GOG `Outpost2.exe` (1.2.0.7), walked section by section with a probe built
  on op2-formats (`Map::parse` for the map block); `eden04.map`, `vehicles.txt` and `ply_tek.txt` from OPU 1.4.1 and
  `sheets.vol`; the signature and file names in `Outpost2.exe`.
- `Outpost2.exe` (GOG 1.2.0.7), raw disassembly of the save and load code and the dialogs, at the addresses given
  above, and its export table for the `TethysGame` functions.
- op2-rs measurements of the original game: the save folder under OPU, OPU's signature, the daylight options and
  position, the command packets, the per-type upgrade list and the player record fields.
- An early disassembly-based description of the `.map` and `.op2` layouts (OP2 community notes): the signature and
  the unit data after the terrain.
- op2-formats `map.rs` (the `.map` part).
