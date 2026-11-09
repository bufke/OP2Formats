# Game strings (`language.ini`)

The text of Outpost 2's own interface (panel labels, unit names, armor words, error messages, the save and load
dialogs) is a table of strings indexed by a **string id**. The original game keeps the table inside `Outpost2.exe`;
OPU 1.4.1 moves it out to `OPU/base/language.ini`, where it can be read and translated. Read by op2-formats'
`strings.rs` (`GameStrings`, `armor_name_id`), which reads the `[Game]` section of `language.ini`.

Mission texts (briefings, objectives) are not here; nor are the research texts, which are in the tech trees
([tech-trees.md](tech-trees.md)).

Unless a section says otherwise, the numbers here were measured with op2-formats on GOG Outpost 2 (`Outpost2.exe`
1.2.0.7) with OPU 1.4.1.

## Where the strings are

| Copy | Where | Strings |
|---|---|---|
| Original | `Outpost2.exe`: an array of string pointers at 0x4E88F8, one per id | 871 (ids 0-870) |
| Original, front end | `OP2Shell.dll` (menus and briefings; it contains `[Shell]` string 0 verbatim) | not counted here |
| OPU 1.4.1 | `OPU/base/language.ini`: `[Game]` for the exe's table, `[Shell]` for `OP2Shell.dll`'s | 870 + 18 |

OPU's `OPUPatch.dll` reads the file (it contains the name `language.ini`). OPU's `CHANGES.md` describes the purpose:
the built-in string tables of `Outpost2.exe` and `OP2Shell.dll` become configurable, with a `language.ini` per language
mod under `OPU/lang`. This install has no `OPU/lang` folder; the English table is the one in `OPU/base`.

The exe's pointer array: entry *n* is the address of a NUL-terminated string for id *n*. Entry 871 is not a pointer
into the image any more, which ends the table. op2-formats doesn't read the array; without `language.ini` (an install
without OPU) every `GameStrings` lookup misses and callers fall back to code names. A reader that wants names on a
stock install can follow the array instead, on this exe build.

### `[Game]` against the exe

Comparing all 871 entries with the unescaped values of `[Game]`: 868 are identical. The differences:

| Id | Exe | `language.ini` |
|---|---|---|
| 455 | colour code `<C2>` | `<C1>` (same text otherwise) |
| 536 | `Version` with a trailing space | without it |
| 832 | empty string | no entry; a comment `; 832 (non-str)` in its place |

## File syntax

- 8-bit text, all ASCII in this file (24,128 bytes, 895 lines), CR LF line ends.
- INI style: `[Section]` lines, then `key=value` lines. op2-formats matches the section name `[Game]` ignoring case and
  reads until the next `[`.
- Lines starting with `;` are comments. op2-formats trims each line first.
- In `[Game]` and `[Shell]` the key is the decimal string id. op2-formats skips lines whose key isn't a number.
- The value is in double quotes. op2-formats removes one quote at each end if both are there, else keeps the value
  as written. Quotes inside the value are not escaped: `"Replace "%s"?"` is a value with two inner quotes.
- Values are on one line; line breaks and tabs are written as escapes.

The file has three sections:

| Section | Contents |
|---|---|
| `[Language]` | `Code=en` |
| `[Game]` | ids 0-870 except 832: 870 strings |
| `[Shell]` | ids 0-17: 18 strings |

### Escapes

op2-formats undoes them the way OPUPatch does: for each escape in the order `\a`, `\b`, `\f`, `\n`, `\r`, `\t`, `\v`,
`\\`, `\'`, `\"`, `\?`, every occurrence is replaced, working from the end of the string back to the start. Because
`\n` is handled before `\\`, the text `\\n` becomes a backslash followed by a newline, not `\n`. A writer that needs a
literal backslash before one of the letters above can't express it.

In the shipped file only three escapes appear: `\n` 35 times, `\t` 4 times and `\r` twice (in network messages that
end in `\r\n`).

### Markup inside the strings

The strings carry the game's own markup, which a reader should keep as text:

- `&` before a letter marks the keyboard accelerator of a button or tab (`&DETAILS`); 151 `[Game]` lines contain `&`.
- `<C0>`, `<C1>`, `<C2>` switch the text colour (string 455, the space program's status line, is `<C1>`, a newline, a tab
  and the text; strings 308 and 309 are a bare `<C0>` and `<C2>`, prefixes the game puts before other text). Measured
  in op2-rs on screenshots of the game: `<C0>` is green, `<C1>` yellow and `<C2>` red, the colours of fine, needs
  attention and lacking on the status pages; a fourth, `<C3>`, is blue, but no string uses it. The exact shade can
  depend on the page: the Space Program list draws `<C1>` as (222, 255, 33), the status text as (255, 255, 0). The
  message log puts `<C2>` (red) before each entry, and the objectives list `<C2>` or `<C0>`.
- `<R>` switches the rest of a line to right alignment (and `<L>` back, measured in op2-rs); 5 `[Game]` lines use it,
  the option labels 416-420 (`Game Speed:<R>%i`).
- printf-style placeholders (`%s`, `%i`) appear on 70 `[Game]` lines; the game fills them in.
- A few strings end in spaces (2 in `[Game]`, more in `[Shell]`).

## Names of sheet rows

Unit, building, weapon and space-object names are strings whose ids follow the **row order** of the rule sheets
([sheets.md](sheets.md#rows-and-order)): the row's name key in the sheet isn't used. Row *r* of a sheet has the name
*base + r*:

| Sheet | Rows | Base | Ids | op2-formats |
|---|---|---|---|---|
| `vehicles.txt` | 17 | 718 | 718-734 | `VEHICLE_NAMES` |
| `building.txt` | 42 | 735 | 735-776 | `BUILDING_NAMES` |
| `building.txt` again | 42 | 777 | 777-818 | none |
| `weapons.txt` | 29 | 819 | 819-847 | `WEAPON_NAMES` |
| `space.txt` | 23 | 848 | 848-870 | none |

Measured by matching every row to its id: each gives the expected name (the Lynx, `LIGHT_TANK`, row 5, is 723). The
second building range holds list-style forms of the same names, noun first (e.g. a "Vehicle Factory" is "Factory,
Vehicle" there); 17 of the 42 differ from the first range. The game's sheet loaders take the names from the string
table, not from the key cells: each loader starts a pointer at its range's first entry of the array at 0x4E88F8 (0x4E9430
for id 718, 0x4E95C4 for 819, 0x4E9638 for 848, 0x4E951C for 777) and moves it on by one for each row that names an
object type ([sheets.md](sheets.md#rows-and-order)). For a building row it copies the first range's name (735 + r) into
the type's name, the same field the vehicles and weapons fill, and the second range's (777 + r) into a second name
field (0x4452A3-0x4452FC). Neither of the sheet's two building key columns is used; by the order the loader reads them,
the second range sits with column 1 (`Produce List Name`) and the first with column 2 (`Building Name`). Measured in
op2-rs: the Structure Factory's production list shows the second range's names, sorted alphabetically.

Weapon row 13 (`SCORPION_WEAPON`) has id 832, the empty string. Ids 716 and 717 just before the vehicles are names of
the Spider and Scorpion packs; **unconfirmed** where the game shows them, since the pack rows themselves (732, 734)
have their own names. Neither array entry is referenced as a fixed address in the code, and the two ids don't appear
as immediate values used as string ids, so the game must compute them; finding the code that turns an object type
into these ids, or a screenshot showing either text, would settle it.

## Armor words

The unit panel describes armor in words. The word depends on the **armor value** (from the sheet's armor class,
[sheets.md](sheets.md#armor-classes)) and on whether the unit is a vehicle or a structure; the choice is made at
0x45984B (op2-formats: `armor_name_id`). The words are strings 234-241:

| Armor value | Vehicle: id, word | Structure: id, word |
|---|---|---|
| 256 or more | 241, No Armor | 241, No Armor |
| 180-255 | 240, Light | 237, Very Light |
| 130-179 | 239, Medium | 240, Light |
| 90-129 | 238, Heavy | 236, Medium-Light |
| 60-89 | 235, Very Heavy | 239, Medium |
| below 60 | 234, Adamantium | 238, Heavy |

(Each string ends in "Armor", e.g. "Light Armor".) With the shipped armor values a class lands on one row each: class 1
(180) reads "Light" on a vehicle and "Very Light" on a structure. op2-formats' tests check three of these against
panels captured from the running game (vehicle 180, structures 60 and 40).

## Writing a language file

- Keep `[Game]` ids 0-870 and `[Shell]` ids 0-17 with the same meanings; the game looks strings up by id.
- Quote every value, write line breaks as `\n` and tabs as `\t`, keep `%s`/`%i` placeholders in their order and the
  `&` accelerators and `<C...>` codes.
- Keep the row-order name ranges in step with the sheets: adding or reordering sheet rows shifts every name after it.
- Save as plain 8-bit text with CR LF line ends. **Unconfirmed** which code page OPU uses for characters above 127;
  the English file has none. `OPUPatch.dll` imports `GetPrivateProfileStringA`, the Windows ANSI reader for INI
  files, so the bytes most likely reach the game unchanged and are drawn in the system's ANSI code page; a
  `language.ini` with an accented letter, looked at in the game, would settle it.

## Sources

- op2-formats `strings.rs` (`GameStrings::parse`, `unescape`, the name bases, `armor_name_id`), whose escape order
  follows OPUPatch (OutpostUniverse), `UserInterface.cpp`.
- OPU 1.4.1's `CHANGES.md` (localization through `language.ini`).
- Measured: OPU 1.4.1's `language.ini`; the string pointer array of GOG's `Outpost2.exe` 1.2.0.7 read with op2-formats'
  `Exe` and compared entry by entry; the sheet rows matched to their name ids; the imports named in `OPUPatch.dll`.
- Raw disassembly of GOG's `Outpost2.exe` 1.2.0.7 (iced-x86): the sheet loaders' name pointers (0x4451E8-0x445FD9).
- op2-rs (bufke's reimplementation), measurements of the running game: the `<Cn>`, `<R>` and `<L>` markup on
  screenshots, the production list's names.
