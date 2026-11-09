# Settings (`outpost2.ini`)

Outpost 2 keeps its preferences, its window placement and the colony game scores in an INI file, `outpost2.ini`. The
game, its front end (`OP2Shell.dll`) and OPU's DLLs read and write it with the Windows profile functions
(`GetPrivateProfileIntA`, `GetPrivateProfileStringA`, `WritePrivateProfileStringA`). op2-formats has no reader for
it; any INI parser that follows the Windows rules below will do.

Unless a section says otherwise, the facts here come from the disassembly of GOG Outpost 2 (`Outpost2.exe` 1.2.0.7,
`OP2Shell.dll`) and OPU 1.4.1 (`op2ext.dll`, `OPUPatch.dll`, `NetHelper.dll`, `OPULauncher.exe`), with the addresses
given, and from the two files on this install. Addresses starting 0x4 or 0x5 are in `Outpost2.exe`, 0x13 in
`OP2Shell.dll`, 0x16 in `OPUPatch.dll`, 0x10 in `op2ext.dll`, 0x25 in `NetHelper.dll`.

## Which file

| Run | File | How |
|---|---|---|
| Original game | `<game folder>/Outpost2.ini` | the exe's own path with the extension replaced |
| OPU (`OPULauncher.exe`, or `Outpost2.exe /OPU`) | `<game folder>/OPU/outpost2.ini` | op2ext replaces the path |

**Original game.** All access goes through one object, the exported `gConfigFile` (a `CConfig`, 0x547090), whose
first 260 bytes are the file's path. Its initialiser (0x410680) takes the running exe's path from
`GetModuleFileNameA`, splits it (0x4C1180), replaces the extension with `INI` and joins it again (0x4C18A0). The file
is therefore named after the exe and sits beside it: `Outpost2.exe` uses `Outpost2.INI` in the game folder (the
profile functions ignore case, so `outpost2.ini` is the same file). A renamed copy of the exe would use its own INI.

**OPU.** OPU's loader shim (`winmm.dll` in the game folder, which imports `op2ext.dll`) loads op2ext, and op2ext
copies its own INI path over `gConfigFile`'s path buffer, at most 260 bytes (0x10002D41). That path is built at
0x1000D683 from a folder made at 0x1000D5EA (the game folder joined with `OPU`) and the name `outpost2.ini`. From then
on the game, the shell and OPUPatch all read and write `OPU/outpost2.ini`; op2ext reads its own sections from the
same path. OPU's `CHANGES.md` gives the reason: the modded game keeps its settings apart from the unmodded one's. The
two files here agree: only the OPU one has OPU's keys and op2ext's sections.
**Unconfirmed:** where op2ext decides to do this only for an OPU run (OPU's `CHANGES.md` says plain `Outpost2.exe`
runs the unmodded game); tracing op2ext's start-up for its command-line check would settle it.

Two OPU programs name the file relative to the current folder instead: `OPULauncher.exe` reads `.\outpost2.ini`
(0x483524) and `NetHelper.dll` reads `.\Outpost2.ini` (0x2506237C). **Unconfirmed** which folder is current for
NetHelper in a game run; the launcher, started from `OPU/`, reads `OPU/outpost2.ini`.

## Syntax and access

- A Windows INI file: `[Section]` lines, then `key=value` lines. Section and key names are matched ignoring case.
  The OPU folder's file has CR LF line ends.
- Integers are read with `GetPrivateProfileIntA`: decimal digits with an optional minus sign; a missing key gives the
  caller's default. They are written as decimal text (`_itoa` base 10 at 0x410843).
- Strings are read with `GetPrivateProfileStringA` into a fixed buffer (sizes below; longer values are cut off).
- A key is written only by the code paths listed below; reading a missing key doesn't add it. Most keys appear in the
  file only once their dialog has been accepted or their setting toggled.
- `CConfig::Flush` (0x410C00) calls `WritePrivateProfileStringA(NULL, NULL, NULL, file)` to flush Windows' cache; the
  shell calls it after the Preferences dialog's ACCEPT (0x1300383B), the game at 0x48679D.

`CConfig`'s methods, exported by `Outpost2.exe` (the shell imports them):

| Method | Address | Does |
|---|---|---|
| `GetInt(section, key, default)` | 0x410870 | `GetPrivateProfileIntA` |
| `SetInt(section, key, value)` | 0x410830 | decimal text, `WritePrivateProfileStringA` |
| `GetString(section, key, buffer, size, default)` | 0x4108B0 | `GetPrivateProfileStringA` |
| `SetString(section, key, value)` | 0x410890 | `WritePrivateProfileStringA` |
| two-argument forms of all four | 0x4108E0, 0x4109B0, 0x410A70, 0x410B30 | take `"Section\Key"` (or `/`) in one string and split it |
| `Flush()` | 0x410C00 | as above |

No code in the exe calls the two-argument forms. OPUPatch calls `GetInt` and `SetInt` directly at their addresses
(0x16001B3F, 0x16001BEF, always with section `Game`) and `GetString` for `[DEBUG]` (0x1600E935).

## `[Game]`

| Key | Type, default | Values | Read / written | What it does |
|---|---|---|---|---|
| `Music` | int, 1 | 0 off, else on | exe 0x45023D; shell 0x130033FA, ACCEPT 0x13003624-0x130036AE | music in the game; the shell also plays `music1.wav` on the menus when on (0x13007C7D). Preferences: Music |
| `MusicVol` | int, 0 | 0-5, 0 loudest | exe 0x450224, written 0x451188; shell 0x13003334, ACCEPT 0x130037FD | index into the exe's 6-entry music volume table (0x4D4AD8, see [exe-tables.md](exe-tables.md)); the exe doesn't range-check it. The Preferences dialog has no control for it; it clamps the value to 0-5 and writes it back |
| `Sound` | int, 1 | 0 off, else on | exe 0x47E00A, written 0x47E4B1; shell ACCEPT 0x130036E6 | sound effects. Preferences: Sound Effects |
| `SoundVol` | int, 0 | 0, 8, ..., 40 | exe 0x47DFE9, written 0x47E213; shell ACCEPT 0x1300381D | effects attenuation: DirectSound volume −50 × SoundVol hundredths of a dB (0x47E32D). The dialog shows Volume = 6 − SoundVol / 8 (1-6) and writes (6 − Volume) × 8, so 4 dB a step, 0 to −20 dB |
| `GameSpeed` | int, 5 (10 under OPU) | 1-10 | exe 0x45F229 and four more reads; in-game faster/slower 0x46632F/0x466424; shell ACCEPT 0x13003833 | game speed: the game asks for 4 × speed ticks a second (measured in op2-rs). The exe clamps changes to 1-10, the shell clamps what it reads to 1-10 |
| `ScrollRate` | int, 4 | 1-6 | exe 0x49AD89 (clamped 1-6), in-game 0x46637A/0x466468; shell ACCEPT 0x130037E0 | scrolling speed. Preferences: Scroll Speed |
| `Zoom` | int, 1 | none | exe 0x49ADB9 | read, but the clamp after it (0x49ADC1-0x49ADD0) always stores 1: the key has no effect |
| `FrameSkip` | int, 1 | 1 or 2 | exe 0x49ADE9 (clamped 1-2 unsigned); toggle 0x457DBE | Frame Skipping in the game's own preferences page: the toggle writes 2 when the old value was 1, else 1. **Unconfirmed:** that 2 redraws the detail pane only every other tick (a gate at 0x407E62 reads a global set from it; not measured) |
| `Overlay` | int, 1 | 0/1 | exe 0x49AE1C; shell ACCEPT 0x13003732 | Preferences: Computer Overlay |
| `Comp_Obj` | int, 1 | 0/1 | exe 0x49AE44; shell ACCEPT 0x130037CA | Preferences: Show Completed Objectives |
| `Ambient` | int, 1 | 0/1 | exe 0x49AE5D; shell ACCEPT 0x1300370C | Preferences: Ambient Animations |
| `StationaryShadows` | int, 0 in the exe, 1 in the shell | 0/1 | exe 0x49AE85, toggle 0x457D2D; shell ACCEPT 0x1300377E | Preferences: Structure Shadows |
| `MobileShadows` | int, 0 in the exe, 1 in the shell | 0/1 | exe 0x49AE9E, toggle 0x457D62; shell ACCEPT 0x13003758 | Preferences: Unit Shadows |
| `IntroAVI` | int, 1 | 0/1 | shell 0x13008AE9, ACCEPT 0x130037A4 | Preferences: Play Intro AVI at Startup |
| `ShowMovies` | int, 1 | 0/1 | shell 0x13008501 | cutscenes on or off; read by the code that holds the shell's list of cutscene names (0x13008461). No dialog control |
| `SpiffyDraw` | int, 1 | 0/1 | shell 0x13007BBD; OPUPatch 0x160200CE | on: the shell draws its dialogs through `odasl.dll` with the art of `op2shres.dll` (ODASL's init at 0x13007C11). No dialog control |
| `Name` | string, empty | up to 12 characters | exe 0x47C3A2 (13-byte buffer); written 0x47C6A6, 0x47C968 | the player's name, taken from two dialogs' name fields |
| `Demo` | int, 0 | any | shell 0x13008EB7, written + 1 at 0x13008EF5 | a counter: each run of the shell's demo plays `ademo1.dll`, `ademo2.dll` or `ademo3.dll` by `Demo` mod 3 (table at 0x13012448) and increments it |

The Preferences dialog (op2shres dialog 142) reads all its keys when it opens and writes them all on ACCEPT (Music,
Sound, Ambient, Overlay, MobileShadows, StationaryShadows, IntroAVI, Comp_Obj, ScrollRate, MusicVol, SoundVol,
GameSpeed; 0x13003624-0x13003833); CANCEL writes nothing. It shows Volume and Scroll Speed as 1-6 and Game Speed as
1-10 and disables LESS and MORE at the ends (0x130039E0). op2-rs measured all of this on the running game.

### Keys OPU adds to `[Game]`

| Key | Type, default | Read / written by | What it does |
|---|---|---|---|
| `CheckForUpdates` | int, −1 | `OPULauncher.exe` 0x401648 | −1 (missing): the launcher asks whether to check for updates at start and writes `1` (yes) or `0` (no). Above 0: it checks OPU's update page before starting the game |
| `UpdateCheckTimeoutMs` | int, 2500 | `OPULauncher.exe` 0x4016FE | the update check's connection timeout in milliseconds (`InternetSetOption` option 2) |
| `ForceFullRedraw` | int, −1 | OPUPatch 0x16002048, written 0x16002180 | the fix for stuttering under desktop composition. −1 (missing): OPUPatch decides from the Windows version and a hypervisor check (`cpuid` leaf 1, ECX bit 31) and writes the result; 0 off; else on |
| `DPIAwareness` | int, missing → −5 | OPUPatch 0x160023D7, written 0x160023F2 | missing: −5 is written and used. 0: the game's built-in behaviour. Anything else is passed to `SetThreadDpiAwarenessContext` (0x160023FE); −5 is `DPI_AWARENESS_CONTEXT_UNAWARE_GDISCALED` |
| `ShowUnitHPBars` | int, −1 | OPUPatch 0x160243A2, written 0x160243D1 | −1 (missing): becomes 1 and is written. Non-zero: OPU's health bars over damaged units; 0: the original's (bars only on selected units). Measured in op2-rs |

**Unconfirmed:** the exact Windows versions behind `ForceFullRedraw`'s automatic choice. OPU's `CHANGES.md` says
Windows 7 with composition on, Windows 8 and later, or a virtual machine; the version checks before 0x16002136 weren't
traced.

OPU also changes the exe's and the shell's **defaults**: OPUPatch writes 10 over the `push 5` of every `GameSpeed`
read (exe 0x45F208, 0x45F58A, 0x48760E, 0x489189, 0x49B6B6; shell 0x13003378, 0x1300825B, 0x130086DF, 0x13008F10;
the lists at 0x16075240 and 0x16075270, patched at 0x160241FD and 0x16024291). OPU's `CHANGES.md` also names shadows,
completed objectives and a maximized window as new defaults. **Unconfirmed** how those are patched; a fresh OPU
install without the keys would show it.

## `[WindowInfo]`

| Key | Type, default | Values | Read / written | What it does |
|---|---|---|---|---|
| `MiniMapButtons` | int, the pane's current value | 0-4 | exe 0x4584A9; written 0x458DEC | where the minimap's three buttons sit. A right click on the minimap moves them to the next place, `(value + 1) mod 5`: along the top, along the bottom, down the right edge, down the left edge, hidden (measured in op2-rs) |

## `[GAME_WINDOW]` and `[DESKTOP]`

The game window's last placement and the desktop size it was saved on. The exe reads them when it creates its window
(0x49B47D-0x49B52F) and writes them when the window moves or changes size (0x49B95C-0x49BAFE).

| Section, key | Type, default | Meaning |
|---|---|---|
| `[GAME_WINDOW] H_SIZE`, `V_SIZE` | int, 640, 480 | window width and height in pixels |
| `[GAME_WINDOW] H_POS`, `V_POS` | int, 0, 0 | window position |
| `[GAME_WINDOW] MAXIMIZED` | int, 0 | 1 when the window was maximized (written from the size change's type at 0x49BA11: 1 for a maximize, else 0) |
| `[DESKTOP] H_SIZE`, `V_SIZE` | int, −1, 480 | the desktop size when the placement was saved; `H_SIZE` −1 means never saved |

The saved placement is used only if all of these hold (0x49B53C-0x49B57F): `[DESKTOP] H_SIZE` isn't −1, the current
desktop is at least as wide and as tall as the saved one, the window is at least 640 × 480, and `H_POS` and `V_POS`
are 0 or more and no larger than the current desktop's width and height. Otherwise the window is 640 × 480, centred on
the screen, and maximized if the screen is exactly 640 × 480. `MAXIMIZED=1` then maximizes the window. The shell reads
`MAXIMIZED`, `H_SIZE` and `V_SIZE` (defaults 0, 640, 480) for its own window (0x13007DE7-0x13007E4D).

GOG's file has `MAXIMIZED=1` with all sizes 0 and `[DESKTOP]` 1024 × 768: the size check fails, so the game starts
from 640 × 480 and maximizes.

## `[ColonyGameScores]`

The shell's colony game score list (op2shres dialogs 223 COLONY GAME SCORES and 224 ENTER YOUR NAME).

| Key | Type | Written | Value |
|---|---|---|---|
| `Count` | int, default 0 | 0x130074DF | number of entries |
| `Name<i>` | string | 0x1300752F | the name typed in ENTER YOUR NAME (up to 20 characters) |
| `Time<i>` | int | 0x13007568 | a shell counter divided by 100; the list shows it as seven digits (`%07i`, 0x13007066) |
| `Difficulty<i>` | string | 0x130075DA | `Easy`, `Normal` or `Hard` (flags bits 0x18: 0, 0x08, else) |
| `Date<i>` | string | 0x13007643 | local date as month-day-year, `%i-%i-%i` without leading zeros |
| `Game<i>` | string | 0x1300769B | the mission's description from its DLL's module descriptor |

Entries are numbered from 1: a new score gets `<i>` = `Count` + 1 and `Count` is raised to it. The list reads entries
1 to `Count` (0x13006EE3), strings into 60-byte buffers. CLEAR SCORES only sets `Count=0` (0x130072C4); the old
entries stay in the file and are overwritten by the next scores. **Unconfirmed:** that `Time` is the game time in
marks (the counter at 0x13015BCF wasn't traced); a recorded score with a known end mark would settle it.

## Debug and network sections

No shipped file has these; the exe reads them.

| Section, key | Type, default | Read at | Meaning |
|---|---|---|---|
| `[DEBUG] ART_PATH` | string, empty (260 and 520-byte buffers) | 0x4715C4, 0x471B86; OPUPatch 0x1600E975 | a folder searched for game files before the others (OPU's `CHANGES.md`: highest precedence); a trailing backslash is checked at 0x4715EF |
| `[DEBUG] SCRIPT` | string, empty (31 bytes) | 0x4876D4 | when set, starting a campaign mission loads this DLL instead of `E%02d.DLL` / `P%02d.DLL` (0x4876EC-0x48773B) |
| `[DEBUG] MAP` | string, `NEWWORLD.MAP` (256 bytes) | 0x489320 | the map of a debug start path (0x489302). **Unconfirmed** when that path runs |
| `[NETWORK] ShortWaitLimit` | int, −1 | 0x487D30 | used only if 1-99 |
| `[NETWORK] Retries` | int, −1 | 0x487D58 | used only if 1-99 |
| `[NETWORK] TimeOut` | int, −1 | 0x487D80 | used only if 21-9999. **Unconfirmed** unit |
| `[SIGS] HIDEUI` | int, 2 | 0x47CF28 | settings of the SIGS online service object (0x47CEFF). **Unconfirmed** meaning |
| `[SIGS] SERVER_LEVEL` | int, 100 | 0x47CF41 | as above |
| `[SIGS] ROOM_LEVEL` | int, 25 | 0x47CF5A | as above |

The exe also contains two messages about a `server.ini` (0x4E2A9C, 0x4E2B18), but no code refers to them.

## Sections of OPU's modules

op2ext reads these from its INI path (`OPU/outpost2.ini`) with `GetPrivateProfileStringA`:

| Section | Keys | Meaning |
|---|---|---|
| `[BuiltInModules]` | `IPDropDown=yes` | op2ext's built-in modules, each `yes` or `no` (any other value is reported as an error) |
| `[ExternalModules]` | `<Name>=yes` | DLL modules to load (`OpuPatch`, `NetHelper` here) |
| `[<Name>]` | `Dll=<file>` (also `Path`) | the module's DLL. op2ext passes the section name to the DLL's `InitMod` |
| `[IPHistory]` | `0=<address>`, `1=...` | addresses typed in the multiplayer join dialog, kept by the IPDropDown module |

`NetHelper.dll`'s `InitMod` (0x25001170) reads its own section, `[NetHelper]` here, from `.\Outpost2.ini`:
`ForwardMode` (default 1; 0 turns its port forwarding off), `BindAll` (1), `AllowPMPReset` (0), `LeaseSec` (86400),
`StartPort` (47776) and `EndPort` (47807). None is in this install's file, so the defaults apply. **Unconfirmed**
what each does beyond its name, apart from `ForwardMode` 0 skipping the rest (0x250011B2).

## The two files on this install

- **Game folder** (`outpost2.ini`, 373 bytes, LF line ends): `[Game]` with the stock keys (`GameSpeed=5`, `Name=Player`,
  `Demo=5`), `[WindowInfo]`, `[GAME_WINDOW]`, `[ColonyGameScores] Count=0` and `[DESKTOP] 1024 × 768`. Other
  programs may add their own keys (for example `Fullscreen` or `MenuSkin` in `[Game]`); neither the game, the shell
  nor any OPU DLL reads those two.
- **OPU folder** (`OPU/outpost2.ini`, CR LF): the same stock keys with this player's values, four of OPU's `[Game]`
  keys (all but `UpdateCheckTimeoutMs`), and op2ext's sections.

## OPU's interface files (`OPU/base/ui`)

Not settings, but read alongside them by `OPUPatch.dll`. The folder holds one PNG and eight TrueType fonts:

| File | What loads it |
|---|---|
| `MainMenuBackground00.png` (3840 × 2160, 8-bit RGB, 9.9 MB) | OPUPatch (name at 0x160746C8, used at 0x1601F0ED), decoded through WIC (`CoCreateInstance` of the WIC imaging factory, 0x1601E7FB) and drawn behind the shell's dialogs, scaled to the window height and centred (measured in op2-rs) |
| `OpenSans-Regular/Bold/Italic/BoldItalic.ttf`, `Roboto-Regular/Bold/Italic/BoldItalic.ttf` | OPUPatch, `AddFontResourceExA` with `FR_PRIVATE` (0x1601FED8): the fonts of OPU's shell text styles, kept as strings in OPUPatch's resources ("R,G,B,font,weight,size,alignment", e.g. `240,255,220,Open Sans,Bold,11,C`) |

OPUPatch's extension table sends `.ttf`, `.ttc`, `.otf`, `.png` and `.bmp` lookups to `ui` (0x1600ED51-0x1600EF0A).
**Unconfirmed:** whether further backgrounds (`MainMenuBackground01.png` and up) are picked up; op2-rs's notes say
OPU's source allows numbers up to 99, and OPU 1.4.1 ships only `00`.

## Writing the file

- Plain 8-bit text, CR LF, `[Section]` and `key=value` lines; integers in decimal. Keys can be in any order and any
  case.
- Missing keys take the defaults above; there is no need to write them. OPU writes `ForceFullRedraw`,
  `DPIAwareness`, `ShowUnitHPBars` and `CheckForUpdates` itself on first run.
- Keep values in the ranges given: the exe uses `MusicVol` as a table index without a check.
- Under OPU, edit `OPU/outpost2.ini`; the game folder's file is the unmodded game's.

## Sources

- Disassembly of GOG's `Outpost2.exe` 1.2.0.7 and `OP2Shell.dll` (every key, default, range and address above),
  with the exe's exports (`CConfig`, `gConfigFile`) and imports read by a probe over op2-formats' `Exe`.
- Disassembly of OPU 1.4.1's `op2ext.dll`, `OPUPatch.dll`, `NetHelper.dll`, `OPULauncher.exe` and `winmm.dll`.
- Dialog captions from `op2shres.dll`'s resources, read with op2-formats `pe_res`.
- Measured in op2-rs: the Preferences dialog's values and limits, `SoundVol`'s attenuation, the game speed's tick
  rate, the minimap button places, `ShowUnitHPBars`, the background's scaling.
- OPU 1.4.1's `CHANGES.md` (the OPU folder's INI, `ForceFullRedraw`, `DPIAwareness`, `DEBUG.art_path`, new defaults).
- Measured: this install's two `outpost2.ini` files and the files of `OPU/base/ui`.
