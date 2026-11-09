# Story and mission texts (`story.vol`: `.rtf`, `ch_title.txt`, `credits.txt`)

The campaign's words outside the game itself (each mission's briefing, its objectives for each difficulty, the story
chapters between missions, the list of chapter titles and the credits) are separate text files. The briefings,
objectives and story are Rich Text Format (`.rtf`); the chapter titles and the credits are plain text. All of them are
shown by the front end, `OP2Shell.dll`, never by the game screen. op2-formats has no reader for them: `GameFiles`
finds them like any other file, and a reader needs only an RTF parser and a line splitter.

Unless a section says otherwise, the numbers here were measured with op2-formats on GOG Outpost 2 (`OP2Shell.dll` and
`Outpost2.exe` 1.2.0.7) with OPU 1.4.1. Addresses are in `OP2Shell.dll` (image base 0x13000000) unless they start
with 0x4 (`Outpost2.exe`) or 0x16 (`OPUPatch.dll`).

## Where the files are

| Copy | Where | Files |
|---|---|---|
| Original | `story.vol` in the game folder, every entry stored raw (see [volumes.md](volumes.md)) | 124: 122 `.rtf`, 2 `.txt` |
| OPU 1.4.1 | `OPU/base/story/`, loose | the same 124 |

All 124 loose copies are byte-identical to the volume's entries (compared one by one with op2-formats' `VolArchive`),
and neither copy has a file the other lacks. OPU only moves the files out of the volume. Its `OPUPatch.dll` maps file
extensions to subfolders of `OPU/base` for the game's file search: `.rtf` and `.txt` go to `story` (the table at
0x1600EBDE-0x1600ED40, next to `.txt` for `sheets` and `techs`). OPU's `CHANGES.md` says a language mod gets its own
copies of the story files, copied from a localized CD; this install has no language mod.

The shell opens every file through the game's resource manager (`ResManager::OpenStream`, exported by
`Outpost2.exe`), so the usual search applies: loose files first, then the volumes ([volumes.md](volumes.md)). Names
are matched without regard to case; the shell asks for `B_e01.rtf`, the files are stored as `b_e01.rtf`.

## File families

| Pattern | Format string | Count | Sizes (bytes) | Contents | Shown in |
|---|---|---|---|---|---|
| `b_eNN.rtf`, `b_pNN.rtf` | `B_%s%02i.rtf` (0x130121E0) | 12 + 12 | 2,100-4,399 | the mission briefing | "%s Mission %i: Briefing" |
| `o_eNN_D.rtf`, `o_pNN_D.rtf` | `O_%s%02i_%c.rtf` (0x130121D0) | 36 + 36 | 1,591-2,471 | the mission objectives at one difficulty | "%s Mission %i: Objectives" |
| `s_eNN.rtf`, `s_pNN.rtf` | `S_%s%02i.rtf` (0x130121A0) | 13 + 13 | 3,519-42,040 | a story chapter | "%s Part %i" |
| `ch_title.txt` | 0x13012190 | 1 | 418 | the chapter titles, both campaigns | CHAPTERS list |
| `credits.txt` | 0x13012144 | 1 | 1,783 | the credits | ABOUT OUTPOST 2 list |

- `%s` is the campaign letter, `e` (Eden, 0x130121B4) or `p` (Plymouth, 0x130121B0).
- `NN` is the mission or chapter number, two digits: 01-12 for briefings and objectives, 01-13 for story chapters.
  Chapter 13 is each campaign's epilogue.
- `D` in the objectives is the difficulty: `e` easy, `m` normal, `h` hard.
- `%s` in the dialog titles is "Eden" or "Plymouth" (shell strings at 0x13012320 and 0x13012314, reached through the
  shell's string pointer table at 0x130123D8; OPU makes that table translatable, see [strings.md](strings.md)).

The shell builds these names only in the campaign's dialogs (mission menu, briefing, objectives, chapters); no other
part of the game opens them.

## How the shell picks a file

The shell keeps the campaign state in two places: a flags word at 0x13015A74 (bit 0x04 set: Eden; bits 0x18: the
difficulty) and the number of missions completed at 0x13015BAE (0 at the start of a campaign, up to 12).

| File | Number used | Letter(s) | Code |
|---|---|---|---|
| Briefing | missions completed + 1 | campaign | 0x1300607E-0x130060BF |
| Objectives | missions completed + 1 | campaign; difficulty bits 0 → `e`, 0x08 → `m`, 0x10 → `h` | 0x13005B8E-0x13005BEA |
| Story | the chapter picked in CHAPTERS | campaign | 0x13005421-0x13005485 |

The briefing and the objectives are therefore always those of the next mission to play. The story number comes from
the CHAPTERS dialog: each title it lists carries its chapter number as item data (0x13005006), and the selection is
stored at 0x13015E24 (0x13005188) for the story dialog. CHAPTERS lists chapters 1 to missions completed + 1, so the
epilogue (chapter 13) appears once mission 12 is won. When missions completed is 12, the shell relabels the dialogs'
MISSION MENU button "MAIN MENU" (0x1300505F, 0x13005451).

The difficulty mapping is the same one the colony game scores use ([settings.md](settings.md#colonygamescores)).
op2-rs measured it on the running game with `O_e01_m` (Normal) and `O_p01_h` (Hard).

### Loading and display

One function (0x13007880) loads every RTF:

1. It asks the resource manager for the path of `STORY.VOL` (0x13007893) and opens that file with `OpenFile` only to
   check that it exists; if it doesn't, the shell shows "Could not open story file." (0x130123B0) and the box stays
   empty. The handle is closed again unused.
2. It opens the requested file with `ResManager::OpenStream` (0x130078EC). A missing file gives the same message.
3. It sends `EM_STREAMIN` (0x449) to the text box with format `SF_RTF` (2) and a callback (0x13007950) that reads
   the stream, so the control parses the whole file.

**Unconfirmed:** whether OPU still needs `story.vol` to exist for step 1 (it redirects file lookups); removing the
volume under OPU and opening a briefing would settle it.

The text box is a Rich Edit 1.0 control: window class `RICHEDIT` from `RICHED32.DLL`, which the shell loads at start
(0x13007C30). Each dialog creates one, read-only and multi-line with scroll bars, as control 0x43F at (20, 25) in the
dialog: 230 × 295 pixels for the briefing (0x13005FB3-0x13005FE6), 565 × 295 for the objectives (0x13005B7D) and the
story (0x13005410). Right after creating it the shell sends `EM_SETBKGNDCOLOR` (0x443) with colour 0, so the
background is black; the files colour their own text green (below). The briefing dialog also shows a mission
picture to the right of the text, a bitmap from `op2shres.dll` (loaded at 0x13006042): number 400 + missions
completed for Eden, 412 + missions completed for Plymouth (measured in op2-rs).

## The RTF subset

The 122 RTF files come in two shapes. Both are plain RTF 1 that any Rich Edit version reads. There are no pictures
(`\pict`), objects (`\object`), fields or Unicode escapes (`\u`).

### Header shape 1: 116 files

All briefings and objectives, and most story chapters. A word processor's full header:

```
{\rtf1\ansi \deff4\deflang1033{\fonttbl{\f4\froman\fcharset0\fprq2 Times New Roman;}{\f5\fswiss\fcharset0\fprq2 Arial;}}
{\colortbl;\red0\green0\blue0;\red0\green0\blue255; ... \red192\green192\blue192;}
{\stylesheet{\widctlpar \f4\fs20 \snext0 Normal;}{\*\cs10 \additive Default Paragraph Font;}}
{\info{\author ...}{\operator ...}{\creatim\yr1997...}{\revtim...}{\version2}...{\*\company ...}{\vern57431}}
\widowctrl\ftnbj\aenddoc ... \sectd \linex0 ... {\*\pnseclvl1\pnucrm\pnstart1\pnindent720\pnhang{\pntxta .}} ...
```

- **Font table:** 110 files have exactly `\f4` Times New Roman and `\f5` Arial; 5 add `\f11` Courier New; 1 has
  those three and `\f1` Symbol. The default font is `\f4`, but the text itself is set in `\f5` (Arial).
- **Colour table:** the same 16 colours in all 116, after an empty entry 0: black, blue, cyan, **green (index 4)**,
  magenta, red, yellow, white, then the dark versions and the two greys. `\cf4` is the only colour these files
  select, so all their text is green, typically as `{\f5\cf4 ...}`.
- `\info` holds authoring metadata (author, operator, company, 1997 creation and revision times, word counts); the
  game ignores it.
- The `\*\pnseclvl1`-`9` destinations (outline numbering defaults) are in every one of these files; no paragraph uses
  numbering.

A typical objectives body (the text replaced):

```
\pard\plain \widctlpar \f4\fs20 {\f5\cf4 First objective.\par }\par }
```

### Header shape 2: 6 files

`b_e03`, `s_e12`, `s_e13`, `s_p02`, `s_p04` and `s_p06`. A short header with no style sheet or `\info`:

```
{\rtf1\ansi\deff0\deftab720{\fonttbl{\f0\fnil MS Sans Serif;}{\f1\fnil\fcharset2 Symbol;}{\f2\fswiss\fprq2 System;}{\f3\fnil Times New Roman;}{\f4\fswiss\fprq2 Arial;}{\f5\froman\fprq2 Times New Roman;}}
{\colortbl\red0\green0\blue0;\red0\green255\blue0;}
\deflang1033\pard\plain\f4\fs20\cf1 Commander:
\par ...
```

Two of them lack `\f5`. The colour table has two entries without the empty first one, so **green is index 1** here and
the text is `\f4\fs20\cf1`: Arial, 10 point, green, as in shape 1. These six files end in `}`, CR LF and a NUL byte;
the NUL comes after the closing brace, where the control ignores it.

### Control words in use

Counted over all 122 files (occurrences / files). Beyond the header words above:

| Group | Words |
|---|---|
| Paragraphs | `\par` 5293/122, `\pard` 326/122, `\plain` 214/122, `\widctlpar` 926/116, `\nowidctlpar` 30/6, `\keep` 240/6, `\keepn` 174/6 |
| Alignment and indents | `\qj` 531/15, `\qc` 100/26, `\li` 668/25 (360-4680 twips), `\ri` 313/24, `\fi` 175/6, `\sb` 156/6, `\sa` 129/15, `\sl` + `\slmult` 232/6 |
| Character formatting | `\f` 1864/122, `\fs` 863/122, `\cf` 800/122, `\i` 304/31, `\b` 97/26, `\caps` 6/6, `\ul` 1, `\uldashdd` 10/1, `\super` 3/1, `\up` 7/4, `\expnd`/`\expndtw` 94-113/7, `\kerning` 26/2 |
| Styles | `\s` 681/15, `\sbasedon` 544/15, `\snext` 653/116, `\cs` 125/116, `\additive` 125/116 |
| Tabs | `\tx` 222/6, `\tqr` 156/6, `\tqc` 54/6, `\tldot` 72/6 |
| Frames, borders, shading | `\absw`, `\absh`, `\posx`, `\posy`, `\phpg`, `\pvpg`, `\abslock` (6 files), `\brdrl`/`\brdrb`/`\brdrs`/`\brdrw`/`\brsp`/`\brdrcf` 6 files, `\shading` 12/6 |
| Special characters | `\emdash` 50/19, `\rquote` 63/29, `\ldblquote` 10/7, `\rdblquote` 8/7, `\'xx` 9/4 |
| Bookmarks | `\bkmkstart`/`\bkmkend` 13/9 |
| Document and section | `\sectd`, `\linex`, `\headery`, `\footery`, `\colsx`, `\endnhere`, `\ftnbj`, `\aenddoc`, `\widowctrl`, `\formshade`, `\fet`, `\hyphcaps`, `\title`, `\pgnrestart`, `\sbknone` and a few more page-layout words |

Font sizes (`\fs`, half-points): 20 (10 pt) in 684 of 863 uses; 16-28 elsewhere. The large sizes (34, 40, 60, 72) and
the frames, tab leaders, borders, Courier New and the `\keep` words are all in six story chapters (`s_e03`, `s_e08`
to `s_e11`, `s_p13`) that show in-story documents. **Unconfirmed** how those frames and page-layout words render in
the shell's 565-pixel box; a screenshot of one of those chapters would settle it.

### Encoding

- Pure 7-bit ASCII: no file has a byte of 0x80 or above. Lines end in CR LF in all 122 (the line breaks are not text;
  `\par` makes paragraphs).
- `\ansi` with `\deflang1033` (US English) and `\fcharset0`: characters above 127 are written as `\'xx` escapes in
  Windows-1252. The shipped files use four: `\'85` (an ellipsis, `b_e02`, `b_e08`), `\'b7` (a middle dot, 5 times in
  `s_p13`), `\'e0` and `\'e9` (accented letters in `s_e03`).
- Quotes and dashes are mostly control symbols (`\rquote`, `\ldblquote`, `\rdblquote`, `\emdash`) rather than
  escapes.

## `ch_title.txt`

Plain text, ASCII, CR LF, 27 lines (418 bytes):

| Lines | Contents |
|---|---|
| 1-12 | Eden chapter titles 1-12, each "Number: Title" |
| 13 | Eden chapter 13 (the epilogue) |
| 14-25 | Plymouth chapter titles 1-12 |
| 26 | Plymouth chapter 13 (the epilogue) |
| 27 | empty (the file ends with an extra CR LF) |

The CHAPTERS dialog (op2shres dialog 215) reads it at 0x13004F66:

- It reads the first 2,000 bytes of the file into a buffer (0x13004F95).
- It walks the lines with a search for LF (0x13004FD1) and writes a NUL over the byte **before** each LF, so the
  line ends must be CR LF: with LF alone the last character of each title would be cut off.
- The campaign's first line is 0 for Eden and 13 for Plymouth (0x13004FB4). Lines from there up to that line +
  missions completed are added to the list box (0x439), each with item data = its chapter number (line − first + 1).
- Nothing checks that a line was found: a file with too few lines makes the loop write through a null result.

## `credits.txt`

Plain text, ASCII, CR LF, 98 lines (1,783 bytes). Section headings start in column 0, names are indented with three
or five spaces; there are no tabs or empty lines. The ABOUT OUTPOST 2 dialog (op2shres dialog 213) shows it in a list
box (0x432): at 0x13004768 the shell clears a 5,000-byte buffer, reads up to 5,000 bytes of the file into it, and
adds each line to the list, splitting at LF and overwriting the byte before the LF (the CR) with a NUL. Leading spaces are kept. Text
after the last LF is not shown, so the file needs a final CR LF; a file of 5,000 bytes or more is cut off (and has no
terminating NUL in the buffer).

## Mission-pack briefings

OPU's mission packs carry no story files of this kind. One pack, `OPU/maps/Exiles/` (missions `mm4_x01.dll` to
`mm4_x05.dll`), has a `Briefings/` folder with `Exiles01.txt` to `Exiles05.txt`: plain ASCII text with CR LF line ends,
1,728 to 3,656 bytes, lines up to 117 characters, no markup. Nothing in the game, the shell, OPU's DLLs or the pack's
mission DLLs refers to them (searched for `Briefings` and the file names), so they are for players to read outside the
game. The pack's DLLs name only their tech tree, `cooptech.txt`.

## Writing story files

- Keep the names: `b_` or `s_` + `e`/`p` + two digits + `.rtf`, `o_` + `e`/`p` + two digits + `_` + `e`/`m`/`h` +
  `.rtf`. All 12 briefings, 36 objectives files per campaign and 13 chapters should exist; a missing one shows the
  "Could not open story file." message instead of text.
- Under OPU, put the files in `OPU/base/story/` (or a mod's own folder); no volume is needed. Without OPU, a loose
  file goes wherever the game's file search finds it before the volume ([volumes.md](volumes.md) gives the order and
  what is unconfirmed about it), and `story.vol` must still exist (the existence check above).
- Write RTF 1 with `\ansi`, a colour table and green text (`\cf` pointing at 0,255,0): the background is black, so
  the default black text would be invisible. Arial at `\fs20` matches the shipped files. Stick to the paragraph and
  character words in the table above, which the shipped files show the control accepts.
- Escape characters above 127 as `\'xx` (Windows-1252), braces and backslashes as `\{`, `\}`, `\\`.
- Keep `ch_title.txt` at 26 title lines plus the final line end, CR LF, under 2,000 bytes; `credits.txt` CR LF, under
  5,000 bytes.
- **Unconfirmed:** the longest text the box takes. The shell sends no `EM_EXLIMITTEXT`, so Rich Edit 1.0's default
  limit of about 32,000 characters may apply; the largest shipped chapter, `s_e10.rtf`, is 42,040 bytes of RTF with less
  text than that. A longer test chapter would settle it.

## Sources

- Disassembly of GOG's `OP2Shell.dll` (the addresses above): file names, the campaign state, the loader and the
  `ch_title.txt` and `credits.txt` parsers. Dialog numbers and control ids from `op2shres.dll`'s dialog resources read
  with op2-formats `pe_res`.
- Disassembly of OPU 1.4.1's `OPUPatch.dll`: the extension-to-folder table.
- op2-formats `vol.rs` (`VolArchive`) and `game_files.rs`: listing `story.vol` and comparing it with `OPU/base/story`.
- Measured: the 122 RTF and 2 text files of `story.vol` and the five `Exiles` briefings (control words, font and
  colour tables, sizes, bytes, line ends), with a probe program over op2-formats.
- Measured in op2-rs: the briefing picture numbers, the difficulty letters on the running game, green text on black.
- OPU 1.4.1's `CHANGES.md` (language mods).
