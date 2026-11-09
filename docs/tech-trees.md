# Tech trees (`*tek.txt`)

A tech tree lists a game's **research topics**: what each costs, which topics it needs first, which lab and how many
scientists it takes, the text the research screens show, and the unit statistics it changes when finished. Each
mission names the tree it uses. The base numbers the topics change are in the rule sheets ([sheets.md](sheets.md)).
Read by op2-formats' `sheets.rs` (`parse_techs`, `Tech`, `TECH_FILES`).

Unless a section says otherwise, the numbers here were measured with op2-formats on GOG Outpost 2 with OPU 1.4.1.

## The files

| File | Topics | Ids | Used by (mission DLLs naming it) |
|---|---|---|---|
| `edentek.txt` | 135 | 1000-12599 | the Eden campaign `e01`-`e12`, and `t14` |
| `ply_tek.txt` | 137 | 1000-12599 | the Plymouth campaign `p01`-`p12`, `t13`, the demos `ademo1`-`ademo3` |
| `multitek.txt` | 137 | 2099-12599 | all 59 multiplayer missions, the 5 colony games (`CEP1`, `CES1`, `CPP1`, `CPS1`, `CPS2`), `us_them` |
| `tutortek.txt` | 95 | 2099-12599 | the tutorial `t01`-`t12` |

A mission DLL names its tree in an exported string (`TechtreeName`); the shipped DLLs spell it in upper case
(`MULTITEK.TXT`), and lookup is case-insensitive. Measured by searching the 106 mission DLLs in the game folder for
the export name and a `*TEK.TXT` string: each has exactly one. Community missions under `OPU/maps/` bring their own
trees under other names (20 files in OPU 1.4.1, e.g. `survtech.txt`, `scouttek.txt`), so a reader shouldn't assume the
four names. `Outpost2.exe` itself contains `multitek.txt`, `ply_tek.txt` and `edentek.txt` (0x4E25A0-0x4E25BC) and
`MULTITEK.TXT` (0x4E9960), but not `tutortek.txt`.

Where they live: the originals are LZH-compressed in `sheets.vol` ([volumes.md](volumes.md)), next to the rule sheets;
OPU 1.4.1 ships loose copies in `OPU/base/techs/`, which the game prefers (the lookup order is in
[sheets.md](sheets.md#where-the-game-finds-them)). OPU's copies parse to the same topics as the originals; only the
end of the file differs (below).

## Text and line syntax

- 8-bit text; every byte outside the trailer is printable ASCII, tab, CR or LF. Lines end in CR LF.
- op2-formats reads the file line by line (the game doesn't, [below](#how-the-game-reads-it)). It trims each line; a
  line is a **key**, then whitespace, then the rest. Indentation is four spaces in the shipped files and values are
  aligned with spaces; `BEGIN_TECH` is followed by two spaces on 10 lines. Tabs appear only in comments.
- A line whose first non-blank character is `;` is a comment. For op2-formats a `;` elsewhere is ordinary text:
  descriptions contain semicolons inside their quotes. The shipped files have no comment after a value on the same
  line, which the game would allow.
- Empty lines are ignored.
- Anything outside a `BEGIN_TECH` ... `END_TECH` block that isn't a comment is ignored by op2-formats; that is how
  it skips the trailer.

### How the game reads it

The game's parser (one topic per call of 0x473A80, called in a loop by the tree loader 0x472940) doesn't work line by
line. It reads three kinds of token from the byte stream, each skipping whatever comes before it:

| Token | Reader | Skips | Takes |
|---|---|---|---|
| word | 0x48F210 | every byte that isn't a letter | a letter, then letters and `_` (up to 100) |
| number | 0x48F0E0 | every byte that isn't a digit or `-`; meeting a letter first is a failure | digits and `-` (up to 32), read with `atoi` |
| quoted text | 0x48F330 | everything up to a `"` | the bytes up to the next `"` (up to 100 for a name, 800 for the texts), leaving out CR, LF and tab |

While skipping, each reader treats a `;` as the start of a comment and skips to the end of the line, so a comment may
also follow a value on the same line. A `;` inside quoted text is ordinary text. Consequences:

- Line breaks and indentation mean nothing to the game; a quoted text may run over several lines (the line breaks are
  dropped, not turned into spaces). op2-formats needs one key per line and a text on one line.
- A word ends at the first byte that is neither a letter nor `_`, so a code name with a digit in it (`SPACE_1`) can't be
  named in a `UNIT_PROP` line. No shipped or community tree tries.
- The longest text in the shipped and community trees is 796 characters (measured), under the 800-character buffer.

A topic starts with the word `BEGIN_TECH` (compared ignoring case). When the word read where a topic should start is
anything else, or there is no word left, the parser returns without a message and the tree ends there (0x473ABD):
everything after it is never read.

### The end of the file

OPU's copies end with an empty line and `; Garbage so checksum matches original:` followed by a few binary bytes, no
final newline. The originals end with a run of LF-only empty lines; `tutortek.txt`'s has a short garbage line among
them. Both are outside any block. For the game the tree ends at the first word after the last `END_TECH` that isn't
`BEGIN_TECH`, or at the end of the file.

The checksum is the game's file checksum, the one it takes of the built-in trees and the sheets
([sheets.md](sheets.md#the-checksum-the-trailer-keeps)). Measured: for all four trees, the checksum of OPU's loose copy
equals that of the stored, LZH-compressed entry in `sheets.vol` (e.g. 0x88E002CF for `multitek.txt`). OPU's notes
add that its own game-start check now checksums the tree a map actually uses and leaves out text that doesn't change
how the game plays.

## Topic blocks

```
BEGIN_TECH "Name" 05506
    CATEGORY        3
    REQUIRES        05201
    COST            1200
    ...
END_TECH
```

`BEGIN_TECH` takes the topic's name in double quotes, then its **id**: the last whitespace-separated word of the line.
Ids are written as 5 digits with leading zeros (`01000`) and are integers (op2-formats: `u32`). Names may contain
spaces and digits (`"06000 Tech Level 6"`) and repeat: two `"Efficiency Engineering"` topics with different ids, many
`"NOT AVAILABLE"`. Ids are unique within each shipped file and every `REQUIRES` names an id in the same file.

The game reads the quoted name and then a number (0x473ACF, 0x473AF6). It keeps `id / 1000` as the topic's **tech
level** (0x473B0D), which a mission's maximum tech level is compared with; an id below 1000 gives level 0 and is an
error. After the file is read the topics are sorted by id (0x472A2F), so their order in the file doesn't matter, and
each `REQUIRES` id is looked up among them; an id that isn't in the tree makes the whole tree fail to load (0x472BA8).

### Keys

All keys in the shipped files are upper case. The game compares keys **ignoring case** (`_stricmp`, 0x4CBEB0, for
every key from 0x473B6E to 0x4740A5), so the community tree's `PlYMOUTH_COST` works. op2-formats matches them
case-sensitively; any key it doesn't know goes to `Tech::other` verbatim. The game has no such slot: a word inside a
block that is none of its keys and not `END_TECH` is an error.

| Key | Value | op2-formats | In the shipped files |
|---|---|---|---|
| `CATEGORY` | integer | `category` (0 if missing) | every topic |
| `REQUIRES` | topic id; one line per prerequisite | `requires` | 0-6 per topic |
| `COST` | integer | `cost` | every topic in the campaign and tutorial trees; 40 in `multitek.txt` |
| `EDEN_COST` | integer | `eden_cost` | 97 topics in `multitek.txt`, always with `PLYMOUTH_COST` |
| `PLYMOUTH_COST` | integer | `plymouth_cost` | the same 97 |
| `MAX_SCIENTISTS` | integer | `max_scientists` | topics with a positive cost; 1-18 |
| `LAB` | integer | `lab` | topics with a positive cost; 1-3 |
| `DESCRIPTION` | quoted text | `description` | topics with a positive cost |
| `TEASER` | quoted text | `teaser` | topics with a positive cost |
| `IMPROVE_DESC` | quoted text | `improve_desc` | 30, 32, 42 and 2 topics |
| `UNIT_PROP` | `CODE Property value` | `unit_props` | see [UNIT_PROP](#unit_prop) |

`Outpost2.exe` holds the key names together (0x4E3ABC-0x4E3B60; `LAB` is the building code name at 0x4E1868) with
two more that no shipped tree uses, `PLAYER_PROP` and `FUNCTION_RESULT` (op2-formats keeps them in `other`):

| Key | Value | What the game does |
|---|---|---|
| `PLAYER_PROP` | `Name value` | `Name` must be a player property of the property table ([UNIT_PROP](#unit_prop)); the only one is `Ore`. When the topic is researched, the value is written into a dword at offset 0x20 of the player's record (records of 0xC24 bytes from 0x56EF1C; 0x4739ED). That dword is the colony's common ore stock (op2-rs measured a repair checking it for common ore, 0x43CB00), so the line sets the stock, it doesn't add to it. |
| `FUNCTION_RESULT` | `Name value` | `Name` must be a function entry of the table; the only one is `Test_Function`. When the topic is researched, the game calls the entry's function with the player and the value (0x473A0C); `Test_Function`'s function (0x472910) is a bare `ret`, so the line does nothing. |

A bad block shows a message box, "Outpost 2 Parsing Error" with "Error reading tech-entry at line %i." (0x4E3A7C,
0x473B8D), naming the line the parser had reached. It's raised by an unknown key, a value that can't be read, an
unknown code or property name, or a property of the wrong kind. Loading then stops at that topic: the topics before it
are kept (0x4729E2), the rest of the file is not read.

op2-formats takes the rest of the line and strips the quotes at both ends; the shipped texts contain no inner quotes. A long text is one line.
Descriptions use a row of underscores as a visual divider between the effect summary and the story text.

### Costs

A topic either has `COST` (the same for both colonies) or both `EDEN_COST` and `PLYMOUTH_COST`; no shipped topic
mixes them. Measured values: -1, 0 and positive numbers up to 6000.

| Value | Count (eden / ply / multi / tutor) | What the files show |
|---|---|---|
| positive | 76 / 78 / 96 / 12 topics with a positive cost for at least one colony | these and only these have `LAB`, `MAX_SCIENTISTS`, `DESCRIPTION` and `TEASER` |
| 0 | 45 / 45 / 44 / 7 cost cells | topics without research text: buildings and units that come with the tree, the "Tech Level" markers |
| -1 | 14 / 14 / 48 / 76 cost cells | `"NOT AVAILABLE"` topics, and in `multitek.txt` the topics one colony may not have |

What the values do, measured in op2-rs on the running game:

- **0**: the topic is granted free, without research, as soon as its prerequisites are researched and its id is within
  the mission's tech level. A player's starting topics are this cascade from the root, and each researched topic
  grants its free follow-ups the same way (0x472CF0, 0x472E40).
- **-1**: the colony can never research it; a lab never lists it (the topic check 0x4733C0 requires a colony cost
  other than -1), and the free cascade skips it. The check is the list's only: a research order sent without it (a
  command op2-rs's harness sent) was carried out and finished within one 192-tick step.
- **positive**: research points. A lab's progress counter starts at cost × 256 and drops every 192 ticks by an amount
  that grows with the team size (a table indexed by the team and the topic's `MAX_SCIENTISTS`).

One exception to keep in mind: the Plymouth tree gives "Evacuation Module" (12599) a cost of 3 with a comment saying it
isn't researchable and a mission script uses it; the tutorial tree gives it -1.

In `multitek.txt` 46 topics have positive costs for both colonies, 39 are positive for one colony and -1 for the
other, 7 are positive for one and 0 for the other, and 5 are 0 for one and -1 for the other.

The game keeps one cost per colony. `COST` writes the value to both (0x473D03-0x473D18), `EDEN_COST` and
`PLYMOUTH_COST` each to their own (0x473D36, 0x473D64), so the last line of a topic wins for each colony: a colony
cost after `COST` overrides that colony's. A topic is zeroed before it's read (0x4729BC), so a missing cost is 0, a
free topic.

### Requirements

Each `REQUIRES` line adds one prerequisite id. Measured: 134, 136, 144 and 1 prerequisites in the four trees; in
`multitek.txt` 32 topics have none, 75 one, 24 two, 5 three and one topic six. All listed topics must be researched:
measured in op2-rs, a lab offers a topic only when every prerequisite is researched (0x4733C0), and a free topic is
granted on the same condition. A topic without `REQUIRES` is given one on topic 0, the tree's root, which every player
has (0x4740BC).

In the campaign trees each mission's section starts with a `"Tech Level N"` topic (id `N000`) that the section's
topics require. The first digits of an id usually match the section: 106 of the 121 sectioned topics in `edentek.txt`
and 103 of 123 in `ply_tek.txt`. It is a naming habit, not a rule the files keep; `multitek.txt` has no levels at all
and groups topics by how many predecessors they have.

### Categories

| Category | Topics (eden / ply / multi / tutor) | What the topics in it are about (from their names) |
|---|---|---|
| 0 | 61 / 61 / 41 / 94 | enablers without research text; in the campaign trees also the two script-driven starship modules |
| 1 | 5 / 5 / 0 / 0 | the campaign's database recovery topics (Astronomy, Chemistry, ...) |
| 2 | 3 / 3 / 4 / 0 | efficiency and fortifications |
| 3 | 4 / 3 / 5 / 0 | power generation |
| 4 | 13 / 15 / 18 / 1 | vehicles and robotics |
| 5 | 1 / 1 / 1 / 0 | food |
| 6 | 9 / 7 / 9 / 0 | mining and metals |
| 7 | 10 / 15 / 23 / 0 | weapons |
| 8 | 4 / 2 / 4 / 0 | space program |
| 9 | 6 / 7 / 11 / 0 | housing, morale, education |
| 10 | 6 / 5 / 7 / 0 | disaster warning |
| 11 | 4 / 4 / 5 / 0 | health and emergency services |
| 12 | 9 / 9 / 9 / 0 | starship modules |

The community trees under `OPU/maps/` use the same 0-12. The category picks the topic's picture: measured in op2-rs,
a lab's status page shows the researched topic with its category's picture (categories 1-12 each have one, chosen at
0x4321F0). **Unconfirmed** whether the game uses the category anywhere else; the parser just stores it (0x473B7A,
offset 4 of the topic). A search of the code for every read of that field would settle it.

`LAB` takes 1-3 and `MAX_SCIENTISTS` 1-18. Measured in op2-rs: `LAB` is the lab type, 1 Basic, 2 Standard, 3
Advanced; a lab lists only the topics of its own type (a Standard Lab never lists Basic or Advanced topics).
`MAX_SCIENTISTS` is the "Maximum Project Staff" of the assign page: the team starts at the smaller of it and the
scientists available, and MORE is disabled at the limit. It also indexes the research rate table, which rewards a full
team. The shipped topic with `MAX_SCIENTISTS 1` ("Tiger Speed Modification") reads that table out of its bounds.

## UNIT_PROP

```
UNIT_PROP LIGHT_TANK Armor 2
```

Three words: a code name from the rule sheets, a property name and an integer. op2-formats requires exactly three
words and an integer value, and refuses the file otherwise. Every code used in the shipped trees is a row of
`vehicles.txt`, `weapons.txt` or `building.txt`, except `EMP_MISSILE` (a `space.txt` row, in `multitek.txt`). The
values are new settings in the sheet's terms: the Lynx's armor class goes from 1 to 2, a move-points value from 3 to 2
(faster), a laser's reload from 35 to 22. `Armor` is a class (0-5, see [sheets.md](sheets.md#armor-classes)): the
parser turns it into the armor value through the same table as the sheets (0x474043-0x474059). `Rate_Of_Fire` is the
weapon's reload time, lower being faster.

The game parses the line as a word (the code name), a word (the property) and a number (0x473F72-0x47402A). The code
name is looked up byte for byte, so **case-sensitively**, among the object types' code names (0x445120); an unknown one
is an error. The property name is looked up **ignoring case** (0x473FCD) in a table of 21 entries at 0x4D5C48, each a
kind, a name and a value. For the 19 unit properties (kind 0) the value is an offset into the per-player block of
the unit type's statistics, the block the sheet values are loaded into, which starts 8 bytes into the type's own
fields. Measured in op2-rs: a researched topic writes each value into that block for the researching player only,
setting rather than adding, and the change applies at once to existing units.

The offsets fix the pairing with the sheet columns. Matching each against the field that the sheet loaders fill
(0x4451E8-0x445DAF):

| Property | Offset | Vehicle sheet column | Weapon sheet column | Building sheet column | Shipped trees |
|---|---|---|---|---|---|
| `Hit_Points` | 0x00 | Hit Points | | Hit Points | yes |
| `Armor` | 0x08 | Armor Class (as a value) | | Armor Class (as a value) | yes |
| `Common_Required` | 0x0C | Common Ore Cost | Common Ore Cost | Common Ore Cost | yes |
| `Rare_Required` | 0x10 | Rare Ore Cost | Rare Ore Cost | Rare Ore Cost | yes |
| `Build_Time`, `Build_Points` | 0x14 | Build Time | Build Time | Building Points | no |
| `Sight_Range` | 0x18 | Sight range | Weapon Range in tiles | Sight range | yes |
| `Improved` | 0x1D | | | | yes, for mines, smelters, GORF, Observatory |
| `Move_Speed`, `Power_Required` | 0x24 | Move Points | Turret Turn Rate | Power required | yes |
| `Turn_Rate`, `Concussion_Damage`, `Workers_Required` | 0x28 | Turn Rate | Concussion Damage | Workers required | some |
| `Production_Rate`, `Penetration_Damage` | 0x2C | Production Rate | Penetration Damage | Scientists required | yes |
| `Rate_Of_Fire`, `Storage_Capacity` | 0x30 | | Reload Time | Storage Capacity | yes |
| `Production_Capacity` | 0x34 | | | Production Capacity | yes |
| `Storage_Bays` | 0x38 | Cargo Capacity | | Storage Bays | no |

The block is a union: names that share an offset set the same field, and what the field means depends on the type.
`Turn_Rate` on a building would set its workers required, `Rate_Of_Fire` on a vehicle a field no vehicle column fills.
`Improved` writes a whole dword at 0x1D; the shipped trees always give it 1, an upgrade flag with no sheet column.
"Some" for 0x28: `Concussion_Damage` and `Workers_Required` are used, `Turn_Rate` isn't. The other two table entries
are `Ore` (kind 1, used by `PLAYER_PROP`) and `Test_Function` (kind 2, used by `FUNCTION_RESULT`); `UNIT_PROP` accepts
only kind 0, so `UNIT_PROP X Ore 5` is an error.

The shipped trees use 15 properties, all in the case shown. The community trees also use the four unused ones and case
variants (`Hit_points`, `TURN_RATE`, `Rate_of_fire`), which the game accepts since it ignores case. op2-formats keeps the
property name as written.

## Writing a tree

- One block per topic, unique ids of 1000 or more, every `REQUIRES` pointing at an id in the file (the game refuses
  the whole tree otherwise).
- Only the keys above: the game stops at the first key it doesn't know, with a parsing error. Code names in
  `UNIT_PROP` exactly as the sheets spell them, upper case.
- Nothing but comments between blocks: the first other word ends the tree for the game.
- Every topic the sheets name as a research topic must exist: measured, every non-zero research topic in the four
  rule sheets is a topic of each of the four shipped trees, even where the tree marks it -1.
- Give researchable topics (positive cost) a `LAB`, `MAX_SCIENTISTS`, `DESCRIPTION` and `TEASER`, as every shipped one
  has.
- `COST` or both colony costs, not a mix.
- Save it under a new name in the mission's folder and name it in the mission's `TechtreeName`; or replace a stock
  tree loose in `OPU/base/techs/`.

## Sources

- op2-formats `sheets.rs` (`parse_techs`, `Tech`, `TECH_FILES`).
- OPU 1.4.1's `CHANGES.md` (the `base` folder and per-map folders, the game-start checksum).
- Raw disassembly of GOG's `Outpost2.exe` 1.2.0.7 (iced-x86), at the addresses given: the tree loader (0x472940), the
  topic parser (0x473A80), the token readers (0x48F0E0-0x48F430), the property table (0x4D5C48), the research
  completion (0x473920), the checksum (0x40C050).
- op2-rs (bufke's reimplementation), measurements of the running game: free and unavailable topics, research points
  and team sizes, the lab lists by `LAB`, the assign page, category pictures, research upgrades applying per player.
- Measured: the four trees from OPU 1.4.1 and from `sheets.vol`, and the 20 community trees under `OPU/maps/`,
  parsed with op2-formats; key, category, cost and property counts; the tech file names in the 106 mission DLLs; the
  key and property strings in GOG's `Outpost2.exe` 1.2.0.7; the game's checksum of each tree copy (loose, decoded and
  stored in `sheets.vol`) by a probe; the longest quoted text.
