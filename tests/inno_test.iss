; Inno Setup script for dummy installers that exercise op2-formats' installer reader (tests/inno_installer.rs).
; Build with the Inno Setup compiler (it runs under Wine), from a source folder of arbitrary files that contains
; sub\deeper\Deep.TXT and notepad.exe (any Windows .exe, so the x86 branch filter is used), e.g.:
;
;   wine ISCC.exe /DSrc=Z:\path\to\src /DOut=Z:\path\to\out /DOutName=lzma2solid inno_test.iss
;   wine ISCC.exe /DSrc=... /DOut=... /DOutName=lzma /DComp=lzma /DSolid=no inno_test.iss
;   wine ISCC.exe /DSrc=... /DOut=... /DOutName=zip /DComp=zip /DSolid=no inno_test.iss
;   wine ISCC.exe /DSrc=... /DOut=... /DOutName=span /DSpan=1 inno_test.iss
;
; Then: OP2_INNO_TEST_INSTALLERS=out OP2_INNO_TEST_SOURCE=src cargo test -p op2-formats --features inno --test inno_installer
; The installer has what GOG's have around the files (languages, types, components, tasks, a registry key, a run
; entry, a {tmp} file) so the reader has to step over them.
#ifndef Comp
  #define Comp "lzma2/ultra64"
#endif
#ifndef Solid
  #define Solid "yes"
#endif

[Setup]
AppName=Outpost 2
AppVersion=1.2
DefaultDirName={sd}\GOG Games\Outpost 2
OutputDir={#Out}
OutputBaseFilename={#OutName}
Compression={#Comp}
SolidCompression={#Solid}
#ifdef Span
DiskSpanning=yes
DiskSliceSize=1500000
SlicesPerDisk=1
#endif
DisableProgramGroupPage=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"

[Types]
Name: "full"; Description: "Full installation"
Name: "custom"; Description: "Custom installation"; Flags: iscustom

[Components]
Name: "main"; Description: "Main files"; Types: full custom; Flags: fixed
Name: "extra"; Description: "Extras"; Types: full

[Tasks]
Name: "desktopicon"; Description: "Create a desktop icon"

[Dirs]
Name: "{app}\saves"

[Files]
Source: "{#Src}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs; Components: main
; The same destination per language: the reader keeps the English one.
Source: "{#Src}\notepad.exe"; DestDir: "{app}\lang"; DestName: "readme.txt"; Languages: german
Source: "{#Src}\sub\deeper\Deep.TXT"; DestDir: "{app}\lang"; DestName: "readme.txt"; Languages: english
Source: "{#Src}\notepad.exe"; DestDir: "{tmp}"; Flags: deleteafterinstall

[Icons]
Name: "{commondesktop}\Outpost 2"; Filename: "{app}\notepad.exe"; Tasks: desktopicon

[Registry]
Root: HKLM; Subkey: "Software\GOG.com\Games\1494370738"; ValueType: string; ValueName: "path"; ValueData: "{app}"

[Run]
Filename: "{app}\notepad.exe"; Description: "Launch"; Flags: postinstall nowait skipifsilent
