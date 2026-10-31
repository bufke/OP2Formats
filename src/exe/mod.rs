//! Tables read from the player's own `Outpost2.exe` at runtime rather than copied into the repository
//! (docs/legal.md rule 4): a minimal PE reader that maps virtual addresses to file bytes, and [`GameExe`], every table
//! op2-rs takes from the executable, read once per game folder in a process ([`GameExe::of`]).
//!
//! The addresses are GOG's 1.2.0.7 build's (the one OPU 1.4.1 runs on), so [`GameExe::read`] first checks that the
//! file is that build, by its SHA-1, and refuses any other: a different build would put other bytes at these
//! addresses. Input is untrusted: every read is bounds-checked and a table that doesn't look right is an error.

pub(crate) mod code;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::units::UnitArt;
use crate::{Error, GameFiles};

/// GOG's `Outpost2.exe` (version 1.2.0.7, as listed in the release's `goggame-1494370738.hashdb`), the build the
/// addresses in this project were read from.
pub const GOG_EXE_SHA1: &str = "1c7d63a05beae42af441e0955d0a9420fae2f85e";

/// The executable's image: the file plus its section table.
pub struct Exe {
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

/// `trunc(64 * cos)` of each heading, 256 signed bytes (0x4E9B30; the sine table at 0x4E9C30 is the same shifted by
/// 64). op2-sim computes it; [`Exe::direction_table`] reads it for the test that checks the two agree.
pub const COS_TABLE: u32 = 0x4E9B30;

/// Everything op2-rs reads from the player's Outpost2.exe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameExe {
    /// The sound file of every SoundID (TethysAPI `SoundID`), e.g. `laser.wav`.
    pub sound_files: Vec<String>,
    /// The structures' ambient animation sequence ([`Exe::ambient_sequence`]).
    pub ambient: [[u8; 5]; AMBIENT_ROWS],
    /// The tube and wall graphic of each set of connections ([`Exe::tube_graphics`]).
    pub tube_graphics: [u8; 16],
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
    /// 10 ms: the file's hash and a few thousand decoded instructions). A failed read isn't kept: the next call tries
    /// again.
    pub fn of(files: &GameFiles) -> Result<Arc<GameExe>, Error> {
        static CACHE: Mutex<Vec<(PathBuf, Arc<GameExe>)>> = Mutex::new(Vec::new());
        // Held while reading, so threads starting together read a folder once.
        let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, t)) = cache.iter().find(|(r, _)| r == files.root()) {
            return Ok(t.clone());
        }
        let tables = Arc::new(Self::read(&Exe::open(files)?)?);
        cache.push((files.root().to_path_buf(), tables.clone()));
        Ok(tables)
    }

    /// Every table of `exe`, after checking it is GOG's 1.2.0.7 build ([`GOG_EXE_SHA1`]).
    pub fn read(exe: &Exe) -> Result<GameExe, Error> {
        exe.check_identity()?;
        Ok(GameExe {
            sound_files: exe.sound_files()?,
            ambient: exe.ambient_sequence()?,
            tube_graphics: exe.tube_graphics()?,
            units: UnitArt::read(exe)?,
            cells: exe.cell_types()?,
            research_efficiency: exe.signed_bytes(RESEARCH_EFFICIENCY, RESEARCH_EFFICIENCY_LEN, "research efficiency")?,
            morale_mods: exe.morale_mods()?,
            savant_priority: exe.savant_priority()?,
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
            sections.push(Section { name, va: base.wrapping_add(f(12)?), vsize: f(8)?, rsize: f(16)?, raw: f(20)? });
        }
        Ok(Self { data, base, sections })
    }

    /// `Outpost2.exe` from the game folder.
    pub fn open(files: &GameFiles) -> Result<Self, Error> {
        let path = files.loose_path("Outpost2.exe").ok_or_else(|| Error::NotFound("Outpost2.exe".into()))?;
        Self::parse(std::fs::read(path).map_err(|e| Error::Io { path: path.display().to_string(), source: e })?)
    }

    pub fn image_base(&self) -> u32 {
        self.base
    }

    /// Whether this is GOG's 1.2.0.7 build, the only one whose addresses op2-rs knows.
    pub fn check_identity(&self) -> Result<(), Error> {
        let sha1 = sha1_smol::Sha1::from(&self.data).digest().to_string();
        if sha1 == GOG_EXE_SHA1 {
            return Ok(());
        }
        Err(Error::Unsupported(format!(
            "Outpost2.exe isn't GOG's version 1.2.0.7 (its SHA-1 is {sha1}, GOG's {GOG_EXE_SHA1}): op2-rs reads the \
             game's tables from that build, so it needs the GOG release's Outpost2.exe (with OPU 1.4.1)"
        )))
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
                    .ok_or_else(|| Error::Invalid(format!("Outpost2.exe: no sound file name for id {i:#x}; not version 1.2?")))
            })
            .collect()
    }

    /// The structures' ambient animation sequence (`AMBIENT_SEQUENCE`): `[delay, slot offset per column 1..=4]` rows,
    /// 0xFF-headed at both ends.
    pub fn ambient_sequence(&self) -> Result<[[u8; 5]; AMBIENT_ROWS], Error> {
        let bad = || Error::Invalid("Outpost2.exe: no ambient animation table at 0x4E40E0; not version 1.2?".into());
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
        let bad = || Error::Invalid("Outpost2.exe: no tube graphics table at 0x4EA778; not version 1.2?".into());
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

    /// A PE whose bytes differ from GOG's build is refused before any table is read.
    #[test]
    fn refuses_another_build() {
        let Some(files) = crate::testdata::game_files() else { return };
        let path = files.loose_path("Outpost2.exe").expect("Outpost2.exe");
        let mut data = std::fs::read(path).expect("read");
        let last = data.len() - 1;
        data[last] ^= 1;
        let err = GameExe::read(&Exe::parse(data).expect("still a PE")).expect_err("a changed byte");
        assert!(err.to_string().contains("isn't GOG's version 1.2.0.7"), "{err}");
    }

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
