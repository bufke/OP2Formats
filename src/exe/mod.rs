//! Tables read from the player's own `Outpost2.exe` at runtime rather than copied into the repository
//! (docs/legal.md rule 4): a minimal PE reader that maps virtual addresses to file bytes, and [`GameExe`], every table
//! op2-rs takes from the executable, read once per game folder in a process ([`GameExe::of`]).
//!
//! The addresses are GOG's 1.2.0.7 build's (the one OPU 1.4.1 runs on; a CD copy brought up to date by OPU has the
//! same file). [`GameExe::read`] notes whether the file is that build, by its SHA-1, but doesn't refuse another: the
//! gate is each table's own check (shapes, ranges, the code patterns it reads), and a table that doesn't look right
//! is one error naming the file, the table and the builds op2-rs supports. Input is untrusted: every read is
//! bounds-checked.

pub(crate) mod code;

use std::sync::Arc;

use crate::units::UnitArt;
use crate::{Error, GameFiles};

/// GOG's `Outpost2.exe` (version 1.2.0.7, as listed in the release's `goggame-1494370738.hashdb`), the build the
/// addresses in this project were read from.
pub const GOG_EXE_SHA1: &str = "1c7d63a05beae42af441e0955d0a9420fae2f85e";

/// The Outpost2.exe of a CD install updated by OPU 1.4.1: its installer's patches for the CD builds
/// ([`CD_EXE_SHA1`]) all end in this file (1,144,320 bytes; GOG's is 1,144,832). OPU has no patch to or from GOG's.
/// Untested here (no CD copy): whether its tables sit where GOG's do; op2-rs reads it if its tables pass the checks.
pub const OPU_CD_EXE_SHA1: &str = "9ac2ef155fb5a86444eb2bbd0fadd3667ffa4ec6";

/// The Outpost2.exe builds OPU 1.4.1's installer updates to [`OPU_CD_EXE_SHA1`] (its Helper.dll's patches
/// `<before>_<after>.dat`): the CD releases and their updates. op2-rs's OPU install applies the patch to a copy
/// (`opu::PATCHED_EXE`).
/// Where op2-rs's OPU tree keeps the copy of a CD build's Outpost2.exe that it updated with OPU's patch, below the OPU
/// folder (`GameFiles` indexes `base` before the game folder, and `install::check` reads it there too).
pub const PATCHED_EXE: &str = "base/Outpost2.exe";

pub const CD_EXE_SHA1: [&str; 6] = [
    "03daaf6183f01b58414e2b737f682557cc7b2de9",
    "278c7b992013884f12a7cf55d21ce5a3b31c3c1f",
    "2f8132055513fa316abcc76db7f2783407a580bb",
    "6430e83fe7214bfa832d6474664e1b1349b292ea",
    "793874d4dab1b880b43401f49e70eb9b3c702796",
    "a28b474c90fc2d7ed21bd05575ea30cafafe75fb",
];

/// The executable's image: the file plus its section table.
pub struct Exe {
    /// Where it came from, for messages.
    path: String,
    data: Vec<u8>,
    base: u32,
    sections: Vec<Section>,
}

struct Section {
    name: [u8; 8],
    va: u32,
    vsize: u32,
    rsize: u32,
    raw: u32,
    /// IMAGE_SCN_* characteristics.
    flags: u32,
}

/// SoundManager's file names, one `char*` per SoundID (0x4E0170, read by the loader at 0x47E015).
const SOUND_FILES: u32 = 0x4E0170;
/// SoundIDs in that table (the loader stops at 0x4E0560).
pub const SOUND_COUNT: usize = 0xFC;
/// Building::DoAmbientAnimation's sequence (0x4E40E0, read at 0x483600/0x48364D): rows of 5 bytes, a delay then four
/// columns of animation slots (5 + value), between two rows starting with 0xFF.
const AMBIENT_SEQUENCE: u32 = 0x4E40E0;
/// The tube/wall graphic for each set of connections (0x4EA778, read by 0x4A1D80 and 0x4A1C40): 16 dwords indexed
/// by a mask of north 1, south 2, west 4, east 8, each the position in the terrain type's 16-graphic group.
const TUBE_GRAPHICS: u32 = 0x4EA778;
/// Rows of the ambient animation sequence, including the 0xFF rows at both ends.
pub const AMBIENT_ROWS: usize = 11;

/// The cell type table (CellTypeInfo[32] at 0x4DEBA8, TethysAPI `MapImpl::GetCellTypeInfo`): 0x24-byte entries of a
/// name pointer, five track speeds, blocks-line-of-sight, blight speed and a minimap colour. Only the first entry is
/// data in the file; a static initializer (0x40B5F0, straight-line code ending at 0x40BD5F) stores the rest.
const CELL_TYPES: u32 = 0x4DEBA8;
const CELL_TYPES_INIT: u32 = 0x40B5F0;
const CELL_TYPE_SIZE: u32 = 0x24;
/// Cell types (a 5-bit field of a tile).
pub const CELL_TYPE_COUNT: usize = 32;

/// Per-scientist research efficiency, signed bytes indexed `team * 15 + max scientists` (read at 0x43E4CC from
/// 0x4E0CD5). The index runs past the 17 rows of 15 into the next ones and below 4 into the bytes before the table,
/// so this is the whole range the game can index: 289 bytes from 0x4E0CD5.
const RESEARCH_EFFICIENCY: u32 = 0x4E0CD5;
pub const RESEARCH_EFFICIENCY_LEN: usize = 289;

/// The morale modifier table (0x4E1F10): 67 entries of {name, value (0x80000000 until morale.txt is read), percent
/// flag}; the flagged ones are percentages the loader converts to 1/256 (0x44E4E6).
const MORALE_MODS: u32 = 0x4E1F10;
pub const MORALE_MOD_COUNT: usize = 67;

/// The Savant queue's priority bytes (0x56E82E, not in the file): SoundManager's constructor (0x47DCE0, straight-line
/// code) stores them one by one from registers holding constants. SavantQueue reads them 0x5E bytes too early
/// (`0x56E7D0[id - 0x5E]` at 0x47F180), so SoundID 0xBC + k gets byte k. op2-sim takes the first 40 (ids 0xBC..0xE3);
/// the constructor writes 134, and what ids 0xE4.. get is unmeasured (notes/uncertain-claims.md).
const SAVANT_PRIORITY: u32 = 0x56E82E;
const SOUND_MANAGER_INIT: u32 = 0x47DCE0;
pub const SAVANT_PRIORITY_LEN: usize = 40;

/// The armor value of each sheet armor class 0..=5 (a damage factor out of 256): six dwords at 0x4E1518, which the
/// sheet loaders index.
const ARMOR_VALUES: u32 = 0x4E1518;
pub const ARMOR_CLASSES: usize = 6;
/// The music tracks' names, one `char*` per SongID (0x4E2700, e.g. `EDEN11`; op2.clm holds the tracks).
const MUSIC_FILES: u32 = 0x4E2700;
pub const SONG_COUNT: usize = 26;
/// The music volume levels in hundredths of a dB (DirectSound's unit), indexed by the ini's MusicVol (0x4D4AD8).
const MUSIC_VOLUMES: u32 = 0x4D4AD8;
pub const MUSIC_VOLUME_STEPS: usize = 6;
/// The production lists of a Vehicle Factory, an Arachnid Factory and a Structure Factory, each three tables (the
/// list, the chassis table and the weapon table it refers to) read by the list builder 0x468250.
const VEHICLE_FACTORY_LIST: [u32; 3] = [0x4E3390, 0x4E33C8, 0x4E33D8];
const ARACHNID_FACTORY_LIST: [u32; 3] = [0x4E3368, 0x4E3380, 0x4E3388];
const STRUCTURE_FACTORY_LIST: [u32; 3] = [0x4E3408, 0x4E3494, 0x4E3498];
/// The table of cursor sources (0x4E9878, loaded by 0x488910 into 0x56EA08): 35 dwords, each a cursor id or a pointer
/// to the `.ani` file name. The first three are the system's (IDC_WAIT, IDC_CROSS, IDC_ARROW), the rest come from
/// out2res.dll or the game folder.
const CURSORS: u32 = 0x4E9878;
pub const CURSOR_COUNT: usize = 35;

/// `trunc(64 * cos)` of each heading, 256 signed bytes (0x4E9B30; the sine table at 0x4E9C30 is the same shifted by
/// 64). op2-sim computes it; [`Exe::direction_table`] reads it for the test that checks the two agree.
pub const COS_TABLE: u32 = 0x4E9B30;

/// Everything op2-rs reads from the player's Outpost2.exe. `Default` is no exe: empty tables, `None` options (rules
/// for tests that invent their own values).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameExe {
    /// Whether the file is GOG's 1.2.0.7 build ([`GOG_EXE_SHA1`]); another build whose tables passed their checks
    /// runs with a warning ([`GameExe::warning`]).
    pub gog_build: bool,
    /// The sound file of every SoundID (TethysAPI `SoundID`), e.g. `laser.wav`.
    pub sound_files: Vec<String>,
    /// The structures' ambient animation sequence ([`Exe::ambient_sequence`]).
    pub ambient: Option<[[u8; 5]; AMBIENT_ROWS]>,
    /// The tube and wall graphic of each set of connections ([`Exe::tube_graphics`]).
    pub tube_graphics: Option<[u8; 16]>,
    /// Which animations each unit type draws (`units`).
    pub units: UnitArt,
    /// The cell types' movement and sight properties (`CELL_TYPES`).
    pub cells: Vec<CellType>,
    /// Research efficiency bytes (`RESEARCH_EFFICIENCY`).
    pub research_efficiency: Vec<i8>,
    /// The morale modifiers' names in the game's order, each with its percent flag (`MORALE_MODS`).
    pub morale_mods: Vec<(String, bool)>,
    /// The Savant queue priority of SoundIDs 0xBC.. (`SAVANT_PRIORITY`).
    pub savant_priority: Vec<u8>,
    /// The armor value of each armor class ([`GameExe::armor_value`]).
    pub armor_values: Vec<u32>,
    /// The music track of every SongID (`MUSIC_FILES`).
    pub music_files: Vec<String>,
    /// The music volume levels in hundredths of a dB by MusicVol.
    pub music_volumes: Vec<i32>,
    /// A Vehicle Factory's, an Arachnid Factory's and a Structure Factory's production lists in the list builder's
    /// order: (MapID, MapID of the weapon it carries). What the colony may build is the caller's check.
    pub vehicle_factory: Vec<(u16, Option<u16>)>,
    pub arachnid_factory: Vec<(u16, Option<u16>)>,
    pub structure_factory: Vec<(u16, Option<u16>)>,
    /// The cursor sources by cursor index.
    pub cursors: Vec<CursorSource>,
}

/// Where the original loads a cursor from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorSource {
    /// A Windows stock cursor (IDC_WAIT, IDC_CROSS or IDC_ARROW).
    Stock(u16),
    /// A cursor resource of out2res.dll by id.
    Resource(u16),
    /// An animated cursor file in the game folder.
    File(String),
}

/// One cell type's entry of the cell type table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellType {
    pub name: String,
    /// Time cost per track type (Legged, Wheeled, Miner, Tracked, Hover): 4096 = normal, 0 = impassable.
    pub speed: [i32; 5],
    pub blocks_sight: bool,
    /// How readily the Blight takes this cell type.
    pub blight_speed: i32,
}

impl GameExe {
    /// The tables of `files`' Outpost2.exe, read on the first call for that game folder and shared after (about
    /// 4 ms: the file's hash and a few thousand decoded instructions). A failed read isn't kept: the next call tries
    /// again.
    pub fn of(files: &GameFiles) -> Result<Arc<GameExe>, Error> {
        static CACHE: crate::cache::FolderCache<GameExe> = crate::cache::FolderCache::new();
        CACHE.get_or_try(files.root(), || Self::read(&Exe::open(files)?))
    }

    /// Every table of `exe`. A table that fails its check is an error naming the file and the table; the SHA-1 only
    /// sets [`GameExe::gog_build`].
    pub fn read(exe: &Exe) -> Result<GameExe, Error> {
        let sha1 = exe.sha1();
        let gog_build = sha1 == GOG_EXE_SHA1;
        if CD_EXE_SHA1.contains(&sha1.as_str()) {
            return Err(Error::Unsupported(format!(
                "{} is a CD build (SHA-1 {sha1}) that OPU updates: install OPU 1.4.1 (`op2 setup` applies its update to \
                 a copy in op2-rs's data folder; your game folder isn't changed)",
                exe.path
            )));
        }
        let refused = |e: Error| {
            let build = match sha1.as_str() {
                GOG_EXE_SHA1 => "GOG's 1.2.0.7 build".to_string(),
                OPU_CD_EXE_SHA1 => "a CD build updated by OPU 1.4.1".to_string(),
                other => format!("an unknown build, SHA-1 {other}"),
            };
            Error::Unsupported(format!(
                "{} ({build}): {e}. op2-rs reads the game's tables from GOG's Outpost2.exe 1.2.0.7, or a CD copy \
                 brought up to it by OPU 1.4.1",
                exe.path
            ))
        };
        let research = exe.research_efficiency().map_err(refused)?;
        Ok(GameExe {
            gog_build,
            sound_files: exe.sound_files().map_err(refused)?,
            ambient: Some(exe.ambient_sequence().map_err(refused)?),
            tube_graphics: Some(exe.tube_graphics().map_err(refused)?),
            units: UnitArt::read(exe).map_err(refused)?,
            cells: exe.cell_types().map_err(refused)?,
            research_efficiency: research,
            morale_mods: exe.morale_mods().map_err(refused)?,
            savant_priority: exe.savant_priority().map_err(refused)?,
            armor_values: exe.armor_values().map_err(refused)?,
            music_files: exe.music_files().map_err(refused)?,
            music_volumes: exe.music_volumes().map_err(refused)?,
            vehicle_factory: exe.build_list(VEHICLE_FACTORY_LIST).map_err(refused)?,
            arachnid_factory: exe.build_list(ARACHNID_FACTORY_LIST).map_err(refused)?,
            structure_factory: exe.build_list(STRUCTURE_FACTORY_LIST).map_err(refused)?,
            cursors: exe.cursors().map_err(refused)?,
        })
    }

    /// The armor value the game derives from a sheet armor class (0-5), e.g. a Lynx's class 1 and a Command Center's
    /// class 5 (a damage factor out of 256); 256 (no armor) for an unknown class or without an exe.
    pub fn armor_value(&self, class: u32) -> u32 {
        self.armor_values.get(class as usize).copied().unwrap_or(256)
    }

    /// What to tell the player about a build that isn't GOG's but whose tables passed their checks.
    pub fn warning(&self) -> Option<String> {
        (!self.gog_build && !self.sound_files.is_empty()).then(|| {
            "Outpost2.exe isn't GOG's 1.2.0.7 build: its tables passed op2-rs's checks, but op2-rs is tested with \
             GOG's; sounds or animations may differ"
                .to_string()
        })
    }
}

impl Exe {
    pub fn parse(data: Vec<u8>) -> Result<Self, Error> {
        let u16_at = |o: usize| data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let u32_at = |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let bad = || Error::Invalid("not a PE executable".into());
        let pe = u32_at(0x3C).ok_or_else(bad)? as usize;
        if data.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err(bad());
        }
        let count = u16_at(pe + 6).ok_or_else(bad)? as usize;
        let opt = u16_at(pe + 20).ok_or_else(bad)? as usize;
        let base = u32_at(pe + 24 + 28).ok_or_else(bad)?;
        let mut sections = Vec::with_capacity(count);
        for i in 0..count {
            let o = pe + 24 + opt + 40 * i;
            let f = |k: usize| u32_at(o + k).ok_or_else(bad);
            let name = data.get(o..o + 8).and_then(|n| n.try_into().ok()).ok_or_else(bad)?;
            sections.push(Section { name, va: base.wrapping_add(f(12)?), vsize: f(8)?, rsize: f(16)?, raw: f(20)?, flags: f(36)? });
        }
        Ok(Self { path: "Outpost2.exe".into(), data, base, sections })
    }

    /// `Outpost2.exe` from the game folder.
    pub fn open(files: &GameFiles) -> Result<Self, Error> {
        let path = files.loose_path("Outpost2.exe").ok_or_else(|| Error::NotFound("Outpost2.exe".into()))?;
        let exe = Self::parse(std::fs::read(&path).map_err(|e| Error::Io { path: path.display().to_string(), source: e })?)?;
        Ok(Self { path: path.display().to_string(), ..exe })
    }

    pub fn image_base(&self) -> u32 {
        self.base
    }

    /// Whether this is GOG's 1.2.0.7 build ([`GOG_EXE_SHA1`]), the one whose addresses op2-rs was written from.
    pub fn is_gog_build(&self) -> bool {
        self.sha1() == GOG_EXE_SHA1
    }

    pub fn sha1(&self) -> String {
        sha1_smol::Sha1::from(&self.data).digest().to_string()
    }

    fn offset(&self, va: u32, len: usize) -> Option<usize> {
        self.sections.iter().find_map(|s| {
            let rel = va.checked_sub(s.va)?;
            (rel < s.vsize.max(s.rsize) && rel as usize + len <= s.rsize as usize).then(|| s.raw as usize + rel as usize)
        })
    }

    pub fn u32(&self, va: u32) -> Option<u32> {
        self.bytes(va, 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn u8(&self, va: u32) -> Option<u8> {
        self.bytes(va, 1).map(|b| b[0])
    }

    pub fn u16(&self, va: u32) -> Option<u16> {
        self.bytes(va, 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }

    /// A NUL-terminated string (at most 255 bytes) at `va`.
    pub fn cstr(&self, va: u32) -> Option<String> {
        let o = self.offset(va, 1)?;
        let tail = &self.data[o..self.data.len().min(o + 256)];
        let end = tail.iter().position(|&b| b == 0)?;
        Some(tail[..end].iter().map(|&b| b as char).collect())
    }

    pub(crate) fn bytes(&self, va: u32, len: usize) -> Option<&[u8]> {
        let o = self.offset(va, len)?;
        self.data.get(o..o + len)
    }

    /// The loaded image's constant bytes from `va` into `out`: the code and read-only sections as the file holds them
    /// (what the game's code and constant tables are before anything runs), zeros elsewhere (writable data, whose
    /// values the running game owns; discarded sections; past a section's raw data). False if no byte of `out` came
    /// from such a section.
    pub fn image_bytes(&self, va: u32, out: &mut [u8]) -> bool {
        const CODE: u32 = 0x20;
        const DISCARDABLE: u32 = 0x0200_0000;
        const WRITE: u32 = 0x8000_0000;
        out.fill(0);
        let mut any = false;
        for s in self.sections.iter().filter(|s| s.flags & DISCARDABLE == 0 && (s.flags & CODE != 0 || s.flags & WRITE == 0)) {
            let (lo, hi) = (va.max(s.va) as u64, (va as u64 + out.len() as u64).min(s.va as u64 + s.rsize.min(s.vsize.max(s.rsize)) as u64));
            if lo >= hi {
                continue;
            }
            let src = s.raw as u64 + (lo - s.va as u64);
            let Some(bytes) = self.data.get(src as usize..(src + hi - lo) as usize) else { continue };
            out[(lo - va as u64) as usize..(hi - va as u64) as usize].copy_from_slice(bytes);
            any = true;
        }
        any
    }

    /// Up to `max_len` bytes from `va`, fewer at the end of its section.
    pub(crate) fn code(&self, va: u32, max_len: usize) -> Option<&[u8]> {
        let s = self.sections.iter().find(|s| va >= s.va && va - s.va < s.rsize)?;
        let rel = (va - s.va) as usize;
        let len = max_len.min(s.rsize as usize - rel);
        self.data.get(s.raw as usize + rel..s.raw as usize + rel + len)
    }

    /// The dword-aligned addresses of the section named `name` (e.g. `.rdata`).
    pub(crate) fn section_dwords(&self, name: &str) -> impl Iterator<Item = u32> + '_ {
        let s = self.sections.iter().find(|s| s.name.split(|&b| b == 0).next() == Some(name.as_bytes()));
        s.into_iter().flat_map(|s| (s.va..s.va + s.rsize.min(s.vsize.max(s.rsize)).saturating_sub(3)).step_by(4))
    }

    /// The research efficiency bytes (`RESEARCH_EFFICIENCY`). The table proper (team sizes 1..=17, from index 19) holds
    /// small positive efficiencies; the bytes before it are a format string's tail, which the game indexes too.
    pub fn research_efficiency(&self) -> Result<Vec<i8>, Error> {
        let t = self.signed_bytes(RESEARCH_EFFICIENCY, RESEARCH_EFFICIENCY_LEN, "research efficiency")?;
        if t[19..].iter().all(|&v| (1..=32).contains(&v)) {
            Ok(t)
        } else {
            Err(Error::Invalid(format!("research efficiency table at {RESEARCH_EFFICIENCY:#x}: values out of range")))
        }
    }

    /// The direction table ([`COS_TABLE`]): `trunc(64 * cos)` of the 256 headings.
    pub fn direction_table(&self) -> Result<Vec<i8>, Error> {
        self.signed_bytes(COS_TABLE, 256, "direction")
    }

    fn signed_bytes(&self, va: u32, len: usize, what: &str) -> Result<Vec<i8>, Error> {
        let raw = self.bytes(va, len).ok_or_else(|| Error::Invalid(format!("Outpost2.exe: no {what} table at {va:#x}")))?;
        Ok(raw.iter().map(|&b| b as i8).collect())
    }

    /// The sound file of every SoundID (TethysAPI `SoundID`), e.g. `laser.wav`. An entry that isn't a `.wav` name
    /// means a different executable: then the whole table is refused.
    pub fn sound_files(&self) -> Result<Vec<String>, Error> {
        (0..SOUND_COUNT as u32)
            .map(|i| {
                let name = self.u32(SOUND_FILES + 4 * i).and_then(|p| self.cstr(p));
                name.filter(|n| n.len() > 4 && n.to_ascii_lowercase().ends_with(".wav"))
                    .ok_or_else(|| Error::Invalid(format!("Outpost2.exe: no sound file name for id {i:#x}")))
            })
            .collect()
    }

    /// The structures' ambient animation sequence (`AMBIENT_SEQUENCE`): `[delay, slot offset per column 1..=4]` rows,
    /// 0xFF-headed at both ends.
    pub fn ambient_sequence(&self) -> Result<[[u8; 5]; AMBIENT_ROWS], Error> {
        let bad = || Error::Invalid("Outpost2.exe: no ambient animation table at 0x4E40E0".into());
        let raw = self.bytes(AMBIENT_SEQUENCE, 5 * AMBIENT_ROWS).ok_or_else(bad)?;
        let mut rows = [[0u8; 5]; AMBIENT_ROWS];
        for (row, chunk) in rows.iter_mut().zip(raw.chunks(5)) {
            row.copy_from_slice(chunk);
        }
        let ends = rows[0][0] == 0xFF && rows[AMBIENT_ROWS - 1][0] == 0xFF;
        let inner = rows[1..AMBIENT_ROWS - 1].iter().all(|r| r[0] < 0x10 && r[1..].iter().all(|&s| s < 5));
        if ends && inner { Ok(rows) } else { Err(bad()) }
    }

    /// The tube and wall graphic for each set of connections (`TUBE_GRAPHICS`): position 0..16 in a terrain type's
    /// group of 16 graphics, by neighbour mask (north 1, south 2, west 4, east 8). Each position appears once.
    pub fn tube_graphics(&self) -> Result<[u8; 16], Error> {
        let bad = || Error::Invalid("Outpost2.exe: no tube graphics table at 0x4EA778".into());
        let mut out = [0u8; 16];
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.u32(TUBE_GRAPHICS + 4 * i as u32).filter(|&v| v < 16).ok_or_else(bad)? as u8;
        }
        let mut seen = [false; 16];
        out.iter().for_each(|&v| seen[v as usize] = true);
        if seen.iter().all(|&s| s) { Ok(out) } else { Err(bad()) }
    }

    /// The cell type table (`CELL_TYPES`) as the game has it once its static initializer has run: the file's bytes
    /// with the initializer's stores laid over them.
    pub fn cell_types(&self) -> Result<Vec<CellType>, Error> {
        let bad = |why: &str| Error::Invalid(format!("Outpost2.exe: cell type table at {CELL_TYPES:#x}: {why}"));
        let stores = code::stores(self, CELL_TYPES_INIT, 0x1000).ok_or_else(|| bad("no initializer"))?;
        if stores.unknown != 0 {
            return Err(bad("the initializer stores values it doesn't load as constants"));
        }
        let mut overlay = std::collections::BTreeMap::new();
        for (&at, &(v, size)) in &stores.absolute {
            for k in 0..size as u32 {
                overlay.insert(at + k, (v >> (8 * k)) as u8);
            }
        }
        let byte = |va: u32| overlay.get(&va).copied().or_else(|| self.bytes(va, 1).map(|b| b[0]));
        let dword = |va: u32| -> Option<u32> { Some(u32::from_le_bytes([byte(va)?, byte(va + 1)?, byte(va + 2)?, byte(va + 3)?])) };
        (0..CELL_TYPE_COUNT as u32)
            .map(|i| {
                let e = CELL_TYPES + CELL_TYPE_SIZE * i;
                let name = dword(e).and_then(|p| self.cstr(p)).filter(|n| !n.is_empty()).ok_or_else(|| bad("an entry has no name"))?;
                let mut speed = [0; 5];
                for (k, s) in speed.iter_mut().enumerate() {
                    *s = dword(e + 4 + 4 * k as u32).filter(|&v| v <= 0x10000).ok_or_else(|| bad("a speed out of range"))? as i32;
                }
                let blocks = dword(e + 0x18).filter(|&v| v <= 1).ok_or_else(|| bad("a sight flag that isn't 0 or 1"))?;
                let blight = dword(e + 0x1C).filter(|&v| v <= 0x100).ok_or_else(|| bad("a blight speed out of range"))?;
                Ok(CellType { name, speed, blocks_sight: blocks == 1, blight_speed: blight as i32 })
            })
            .collect()
    }

    /// The Savant queue priorities (`SAVANT_PRIORITY`), as SoundManager's constructor stores them.
    pub fn savant_priority(&self) -> Result<Vec<u8>, Error> {
        let bad = || Error::Invalid(format!("Outpost2.exe: no Savant priority stores in {SOUND_MANAGER_INIT:#x}"));
        let stores = code::stores(self, SOUND_MANAGER_INIT, 0x400).ok_or_else(bad)?;
        (0..SAVANT_PRIORITY_LEN as u32)
            .map(|k| match stores.absolute.get(&(SAVANT_PRIORITY + k)) {
                Some(&(v, 1)) if v <= 2 => Ok(v as u8),
                _ => Err(bad()),
            })
            .collect()
    }

    /// The armor value of each sheet armor class (`ARMOR_VALUES`): 256 for class 0, then falling.
    pub fn armor_values(&self) -> Result<Vec<u32>, Error> {
        let bad = || Error::Invalid(format!("Outpost2.exe: no armor values at {ARMOR_VALUES:#x}"));
        let v: Vec<u32> = (0..ARMOR_CLASSES as u32).map(|i| self.u32(ARMOR_VALUES + 4 * i).ok_or_else(bad)).collect::<Result<_, _>>()?;
        if v[0] == 256 && v.windows(2).all(|w| w[1] < w[0]) && v[ARMOR_CLASSES - 1] > 0 { Ok(v) } else { Err(bad()) }
    }

    /// The music track names (`MUSIC_FILES`): uppercase names of up to 8 characters.
    pub fn music_files(&self) -> Result<Vec<String>, Error> {
        (0..SONG_COUNT as u32)
            .map(|i| {
                let name = self.u32(MUSIC_FILES + 4 * i).and_then(|p| self.cstr(p));
                name.filter(|n| (1..=8).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()))
                    .ok_or_else(|| Error::Invalid(format!("Outpost2.exe: no music track name for song {i} at {MUSIC_FILES:#x}")))
            })
            .collect()
    }

    /// The music volume levels (`MUSIC_VOLUMES`): 0 for the loudest, then falling steps (negative hundredths of a dB).
    pub fn music_volumes(&self) -> Result<Vec<i32>, Error> {
        let bad = || Error::Invalid(format!("Outpost2.exe: no music volume levels at {MUSIC_VOLUMES:#x}"));
        let v: Vec<i32> =
            (0..MUSIC_VOLUME_STEPS as u32).map(|i| self.u32(MUSIC_VOLUMES + 4 * i).map(|v| v as i32).ok_or_else(bad)).collect::<Result<_, _>>()?;
        if v[0] == 0 && v.windows(2).all(|w| w[1] < w[0]) && v[MUSIC_VOLUME_STEPS - 1] > -10000 { Ok(v) } else { Err(bad()) }
    }

    /// A production list as the list builder 0x468250 expands it: `list` is the list table, `chassis` and `weapons`
    /// the tables it refers to. In the list a positive entry is one MapID; -k stands for the k-th chassis, listed once
    /// per weapon of the weapon table. The list and the weapon table end with -100; the chassis table has no end
    /// marker (it runs into the weapon table, padded by a 0 in the Vehicle Factory's), so it is read by index up to the
    /// largest k the list uses. Nothing is checked against what the player has built or researched (the builder asks
    /// for that per entry), and the builder's one special case, MapID 0x20 (the Trade Center, listed only if a global
    /// is set), is left to the caller.
    pub fn build_list(&self, [list, chassis, weapons]: [u32; 3]) -> Result<Vec<(u16, Option<u16>)>, Error> {
        let bad = |why: &str| Error::Invalid(format!("Outpost2.exe: production list at {list:#x}: {why}"));
        let table = |va: u32| -> Result<Vec<i32>, Error> {
            let mut out = Vec::new();
            for i in 0..64 {
                let v = self.u32(va + 4 * i).ok_or_else(|| bad("past the file"))? as i32;
                if v == -100 {
                    return Ok(out);
                }
                out.push(v);
            }
            Err(bad("no end marker"))
        };
        let id = |v: i32| u16::try_from(v).ok().filter(|&v| (1..115).contains(&v)).ok_or_else(|| bad("a MapID out of range"));
        let (entries, weapons) = (table(list)?, table(weapons)?);
        let chassis_count = entries.iter().filter(|&&v| v <= 0).map(|&v| -v as usize).max().unwrap_or(0);
        let chassis: Vec<u16> = (0..chassis_count as u32)
            .map(|i| id(self.u32(chassis + 4 * i).ok_or_else(|| bad("past the file"))? as i32))
            .collect::<Result<_, _>>()?;
        let weapons: Vec<u16> = weapons.into_iter().map(id).collect::<Result<_, _>>()?;
        let mut out = Vec::new();
        for v in entries {
            if v > 0 {
                out.push((id(v)?, None));
            } else {
                let c = chassis.get((-v - 1) as usize).copied().filter(|_| v < 0).ok_or_else(|| bad("a chassis index out of range"))?;
                out.extend(weapons.iter().map(|&w| (c, Some(w))));
            }
        }
        if out.is_empty() { Err(bad("empty")) } else { Ok(out) }
    }

    /// The cursor sources (`CURSORS`).
    pub fn cursors(&self) -> Result<Vec<CursorSource>, Error> {
        let bad = |i: usize| Error::Invalid(format!("Outpost2.exe: no cursor source {i} at {CURSORS:#x}"));
        (0..CURSOR_COUNT)
            .map(|i| {
                let v = self.u32(CURSORS + 4 * i as u32).ok_or_else(|| bad(i))?;
                match v {
                    0x7F00..=0x7FFF if i < 3 => Ok(CursorSource::Stock(v as u16)),
                    1..=0xFFFF if i >= 3 => Ok(CursorSource::Resource(v as u16)),
                    0x10000.. => self.cstr(v).filter(|n| n.to_ascii_lowercase().ends_with(".ani")).map(CursorSource::File).ok_or_else(|| bad(i)),
                    _ => Err(bad(i)),
                }
            })
            .collect()
    }

    /// The morale modifiers' names and percent flags (`MORALE_MODS`), in the table's order.
    pub fn morale_mods(&self) -> Result<Vec<(String, bool)>, Error> {
        let bad = || Error::Invalid(format!("Outpost2.exe: no morale modifier table at {MORALE_MODS:#x}"));
        (0..MORALE_MOD_COUNT as u32)
            .map(|i| {
                let e = MORALE_MODS + 12 * i;
                let name = self.u32(e).and_then(|p| self.cstr(p)).filter(|n| !n.is_empty()).ok_or_else(bad)?;
                let unset = self.u32(e + 4) == Some(0x8000_0000);
                match self.u32(e + 8) {
                    Some(flag @ 0..=1) if unset => Ok((name, flag == 1)),
                    _ => Err(bad()),
                }
            })
            .collect()
    }
}

/// The Savant queue's length of every SoundID's file ([`savant_ms`]; 0 for a file that can't be read).
pub fn sound_lengths(files: &GameFiles, names: &[String]) -> Vec<u32> {
    names
        .iter()
        .map(|n| files.read(n).ok().and_then(|(d, _)| crate::sound::wav_info(&d).ok()).map_or(0, |(_, len)| savant_ms(len)))
        .collect()
}

/// How long the Savant queue waits for a line (SoundManager's length table 0x56DC00, filled at load by 0x47F300):
/// the WAV's data bytes / 22.05, in ms, as for 22 kHz 8-bit mono, which the game's sounds are.
pub fn savant_ms(data_len: usize) -> u32 {
    (data_len as u64 * 1000 / 22050) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage() {
        assert!(Exe::parse(vec![0; 64]).is_err());
    }

    /// Another build runs when its tables pass their checks (with a warning), and is refused, naming the table and the
    /// builds op2-rs supports, when one doesn't.
    #[test]
    fn another_build_runs_unless_a_table_fails() {
        let Some(files) = crate::testdata::game_files() else { return };
        let path = files.loose_path("Outpost2.exe").expect("Outpost2.exe");
        let mut data = std::fs::read(path).expect("read");
        let last = data.len() - 1;
        data[last] ^= 1;
        let other = GameExe::read(&Exe::parse(data.clone()).expect("still a PE")).expect("its tables still pass");
        assert!(!other.gog_build && other.warning().is_some());
        assert_eq!(other.units, GameExe::of(&files).unwrap().units);
        let at = Exe::parse(data.clone()).unwrap().offset(AMBIENT_SEQUENCE, 5).unwrap();
        data[at] = 0;
        let err = GameExe::read(&Exe::parse(data).unwrap()).expect_err("a broken ambient table").to_string();
        assert!(err.contains("ambient animation table") && err.contains("an unknown build"), "{err}");
        assert!(err.contains("a CD copy brought up to it by OPU"), "{err}");
    }

    /// SHA-1 digests of the tables read from GOG's build (of their `Debug` text: no game data in the source), so a
    /// change to the readers can't shift a value unseen. The values equal the tables op2-rs embedded before
    /// (units_table.rs, the cell speeds, ...).
    #[test]
    fn gog_tables_digests() {
        let Some(files) = crate::testdata::game_files() else { return };
        let exe = GameExe::of(&files).expect("Outpost2.exe tables");
        let digest = |v: &dyn std::fmt::Debug| sha1_smol::Sha1::from(format!("{v:?}").as_bytes()).digest().to_string();
        let got = [
            ("units", digest(&exe.units)),
            ("cells", digest(&exe.cells)),
            ("research", digest(&exe.research_efficiency)),
            ("morale", digest(&exe.morale_mods)),
            ("savant", digest(&exe.savant_priority)),
            ("sounds", digest(&exe.sound_files)),
            ("looks", digest(&(exe.ambient, exe.tube_graphics))),
            ("armor", digest(&exe.armor_values)),
            ("music", digest(&(&exe.music_files, &exe.music_volumes))),
            ("lists", digest(&(&exe.vehicle_factory, &exe.arachnid_factory, &exe.structure_factory))),
            ("cursors", digest(&exe.cursors)),
        ];
        let want = GOG_DIGESTS;
        for ((name, d), (wname, w)) in got.iter().zip(want) {
            assert_eq!((*name, d.as_str()), (wname, w), "the {name} table read from GOG's build changed");
        }
    }

    const GOG_DIGESTS: [(&str, &str); 11] = [
        ("units", "04db49f3eda5addb654a4fe5e8f239282ec16420"),
        ("cells", "37a96cea973bc03eca893fd284e488647274bcfa"),
        ("research", "aaa77e9939f50f525e359c572f94554e75e93e9e"),
        ("morale", "434ec25e710c35adddc4ded81a178103bed7b008"),
        ("savant", "d2b5bc7946f131db2e269b02deeb7c97d0a78749"),
        ("sounds", "32bf5a3acc062ebe78f7be22c8cd15be0d829871"),
        ("looks", "232b7d34b1f71a69cbaafcc698b7238104951525"),
        ("armor", "f67eb3ee965934d822ab5a673303e6c9ab451be9"),
        ("music", "4ad762445831287a6c4f5c673d0e2632aa17df7e"),
        ("lists", "7ad2430a689798e08869c8afa2c193c633102227"),
        ("cursors", "b1fc13af0753be2f0f0d865e76dcf1aef7c300ed"),
    ];

    #[test]
    fn reads_every_table() {
        let Some(files) = crate::testdata::game_files() else { return };
        let exe = GameExe::of(&files).expect("Outpost2.exe tables");
        assert_eq!(exe.sound_files.len(), SOUND_COUNT);
        assert!(exe.sound_files.iter().all(|n| n.ends_with(".wav")));
        assert_eq!(exe.cells.len(), CELL_TYPE_COUNT);
        assert_eq!(exe.cells[0].name, "Fast Passible 1");
        assert_eq!(exe.research_efficiency.len(), RESEARCH_EFFICIENCY_LEN);
        assert_eq!(exe.morale_mods.len(), MORALE_MOD_COUNT);
        assert_eq!(exe.morale_mods.iter().filter(|m| m.1).count(), 7, "the percentage modifiers");
        assert_eq!(exe.savant_priority.len(), SAVANT_PRIORITY_LEN);
        assert_eq!(exe.units.select_sounds.len(), 53, "every vehicle and structure type");
        assert!(exe.units.select_sounds.iter().all(|&(_, id)| (id as usize) < SOUND_COUNT));
        assert_eq!((exe.armor_value(0), exe.armor_value(5), exe.armor_value(9)), (256, 40, 256));
        assert_eq!((exe.music_files.len(), exe.music_files[0].as_str(), exe.music_volumes.len()), (SONG_COUNT, "EDEN11", MUSIC_VOLUME_STEPS));
        // 10 vehicles then 3 tank chassis with 11 turrets each; the Scorpion with its weapon, its 3-Pack, the Spider and
        // its 3-Pack; the Guard Post (MapID 23) once per turret among the other structures.
        assert_eq!((exe.vehicle_factory.len(), exe.vehicle_factory[0], exe.vehicle_factory[10]), (43, (1, None), (5, Some(59))));
        assert_eq!(exe.arachnid_factory, [(4, Some(73)), (113, Some(73)), (3, None), (112, None)]);
        assert_eq!(exe.structure_factory.iter().filter(|e| e.0 == 23).count(), 11);
        assert_eq!(exe.structure_factory.len(), 33 + 11);
        assert_eq!(exe.cursors.len(), CURSOR_COUNT);
        assert!(exe.gog_build && exe.warning().is_none());
        assert!(Arc::ptr_eq(&exe, &GameExe::of(&files).unwrap()), "read once per folder");
    }

    /// The game's per-neighbour tables (0x4EA578.., the neighbour north of a new tube gains a south connection, and
    /// so on; 0x4EA678.. lose one when a tube is bulldozed) are all the connection table applied to the mask with
    /// that bit set or cleared, so only `tube_graphics` is read.
    #[test]
    fn neighbour_tables_follow_the_tube_graphics() {
        let Some(files) = crate::testdata::game_files() else { return };
        let exe = Exe::open(&files).expect("Outpost2.exe");
        let graphic = exe.tube_graphics().expect("tube table");
        let mask_of = |pos: u32| graphic.iter().position(|&g| g as u32 == pos).expect("each position once");
        for (k, bit) in [2usize, 1, 8, 4].into_iter().enumerate() {
            for pos in 0..16u32 {
                let add = exe.u32(0x4EA578 + 0x40 * k as u32 + 4 * pos).unwrap();
                let del = exe.u32(0x4EA678 + 0x40 * k as u32 + 4 * pos).unwrap();
                assert_eq!(add, graphic[mask_of(pos) | bit] as u32);
                assert_eq!(del, graphic[mask_of(pos) & !bit] as u32);
            }
        }
        assert!(exe.ambient_sequence().is_ok());
    }
}
