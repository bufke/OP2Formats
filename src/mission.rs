//! Mission DLLs: the description the game reads out of each level's DLL without loading it (the `DescBlock`,
//! `LevelDesc`, `MapName` and `TechtreeName` exports), the other exports, and the game's file checksum. No DLL code
//! runs: the PE file is read as data. The format is described in `docs/mission-dlls.md`.
//!
//! ```no_run
//! # fn main() -> Result<(), op2_formats::Error> {
//! let files = op2_formats::GameFiles::open("C:/Games/Outpost 2")?;
//! for (path, m) in op2_formats::mission::missions(&files) {
//!     println!("{}: {} ({:?}, {} players, map {})", path.display(), m.level_desc, m.mission_type, m.players, m.map_name);
//! }
//! # Ok(())
//! # }
//! ```

use std::path::{Path, PathBuf};

use crate::{Error, GameFiles};

/// `DescBlock`'s first field: what kind of level this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MissionType {
    /// A campaign mission, numbered 1 to 12 (`e01.dll`..`e12.dll`, `p01.dll`..`p12.dll`).
    Campaign(u8),
    /// −1: a colony game (`c*.dll`).
    Colony,
    /// −2: an auto demo (`ademo1.dll`..`ademo3.dll`).
    AutoDemo,
    /// −3: a tutorial (`t*.dll`).
    Tutorial,
    /// −4: multiplayer Land Rush (`mu<N>*.dll`).
    LandRush,
    /// −5: multiplayer Space Race (`mf<N>*.dll`).
    SpaceRace,
    /// −6: multiplayer Resource Race (`mr<N>*.dll`).
    ResourceRace,
    /// −7: multiplayer Midas (`mm<N>*.dll`).
    Midas,
    /// −8: multiplayer Last One Standing (`ml<N>*.dll`).
    LastOneStanding,
    /// Any other value; the stock game lists such a level nowhere.
    Other(i32),
}

impl MissionType {
    pub fn from_value(value: i32) -> Self {
        match value {
            1..=12 => Self::Campaign(value as u8),
            -1 => Self::Colony,
            -2 => Self::AutoDemo,
            -3 => Self::Tutorial,
            -4 => Self::LandRush,
            -5 => Self::SpaceRace,
            -6 => Self::ResourceRace,
            -7 => Self::Midas,
            -8 => Self::LastOneStanding,
            _ => Self::Other(value),
        }
    }

    /// The value as `DescBlock` stores it.
    pub fn value(self) -> i32 {
        match self {
            Self::Campaign(n) => n as i32,
            Self::Colony => -1,
            Self::AutoDemo => -2,
            Self::Tutorial => -3,
            Self::LandRush => -4,
            Self::SpaceRace => -5,
            Self::ResourceRace => -6,
            Self::Midas => -7,
            Self::LastOneStanding => -8,
            Self::Other(v) => v,
        }
    }

    /// A multiplayer type, −8 to −4, as the game tests it (0x4028E0).
    pub fn is_multiplayer(self) -> bool {
        (-8..=-4).contains(&self.value())
    }
}

/// One exported name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    /// The export's address, relative to the image base.
    pub rva: u32,
    /// The name of the section that holds it, empty when no section does.
    pub section: String,
    /// In a code section: a function rather than data.
    pub is_code: bool,
}

/// What a mission DLL says about itself.
#[derive(Debug, Clone)]
pub struct MissionDll {
    pub mission_type: MissionType,
    /// Player slots the mission uses, AI included; for a multiplayer game the most human players.
    pub players: i32,
    /// The highest tech level the tech tree is loaded with.
    pub max_tech_level: i32,
    /// The colony report and economy update are skipped for human players.
    pub unit_only: bool,
    /// The text the game's menus show.
    pub level_desc: String,
    /// The `.map` file, a bare file name.
    pub map_name: String,
    /// The tech tree file, a bare file name.
    pub techtree_name: String,
    /// `DescBlockEx`'s first field, the extra AI players of a multiplayer game, when the DLL exports it.
    pub extra_ai_players: Option<i32>,
    /// Every export, in the export table's name order.
    pub exports: Vec<Export>,
    /// The game's checksum of the whole file ([`checksum`]).
    pub checksum: u32,
    /// Ways the file differs from what the game's own PE reader (0x419480) expects; empty for every shipped DLL.
    /// Windows loads such a DLL, but the game may read its description wrongly or not list it.
    pub warnings: Vec<String>,
}

/// The four exports the game needs to list or start a mission (`GetModuleDesc`, 0x4025B0).
pub const REQUIRED_EXPORTS: [&str; 4] = ["DescBlock", "MapName", "LevelDesc", "TechtreeName"];

impl MissionDll {
    /// Reads a mission DLL's description. A DLL without `DescBlock` isn't a mission: that is an error, as is one
    /// that lacks any of [`REQUIRED_EXPORTS`].
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let mut pe = Pe::parse(data)?;
        let exports = pe.exports()?;
        let find = |name: &str| exports.iter().find(|e| e.name == name);
        let desc = find("DescBlock").ok_or_else(|| Error::Invalid("not a mission DLL: no DescBlock export".into()))?;
        for name in REQUIRED_EXPORTS {
            if find(name).is_none() {
                return Err(Error::Invalid(format!("mission DLL without the {name} export")));
            }
        }
        let block = pe.bytes_at(desc.rva, 16).ok_or_else(|| Error::Invalid("DescBlock lies outside the file".into()))?;
        let field = |i: usize| i32::from_le_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        let text = |name: &str| -> Result<String, Error> {
            let rva = find(name).unwrap().rva;
            pe.cstr_at(rva).ok_or_else(|| Error::Invalid(format!("{name} lies outside the file")))
        };
        let extra_ai_players =
            find("DescBlockEx").and_then(|e| pe.bytes_at(e.rva, 4)).map(|b| i32::from_le_bytes(b.try_into().unwrap()));
        Ok(Self {
            mission_type: MissionType::from_value(field(0)),
            players: field(1),
            max_tech_level: field(2),
            unit_only: field(3) != 0,
            level_desc: text("LevelDesc")?,
            map_name: text("MapName")?,
            techtree_name: text("TechtreeName")?,
            extra_ai_players,
            checksum: checksum(data),
            warnings: pe.game_reader_warnings(),
            exports,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let data = std::fs::read(path).map_err(|source| Error::Io { path: path.display().to_string(), source })?;
        Self::parse(&data)
    }

    pub fn export(&self, name: &str) -> Option<&Export> {
        self.exports.iter().find(|e| e.name == name)
    }

    pub fn has_export(&self, name: &str) -> bool {
        self.export(name).is_some()
    }
}

/// Every mission DLL among the install's loose files: the game folder's own and those under OPU's `base` and `maps`
/// folders, one per file name (OPU's copies of the stock missions count once), sorted by file name. DLLs that aren't
/// missions, or don't read, are left out.
pub fn missions(files: &GameFiles) -> Vec<(PathBuf, MissionDll)> {
    let mut found: Vec<(PathBuf, MissionDll)> = files
        .loose_files()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dll")))
        .filter_map(|p| MissionDll::open(&p).ok().map(|m| (p, m)))
        .collect();
    found.sort_by_key(|(p, _)| p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()));
    found
}

/// The type and player count a multiplayer mission's file name gives, the way the game's lobby reads it (0x4029C0):
/// `m`, then `u`, `f`, `r`, `m` or `l` for the type, then a digit from 2 to 6; any case. `None` for other names.
pub fn multiplayer_name(file_name: &str) -> Option<(MissionType, u8)> {
    let b = file_name.as_bytes();
    if b.len() < 3 || !b[0].eq_ignore_ascii_case(&b'm') {
        return None;
    }
    let kind = match b[1].to_ascii_lowercase() {
        b'u' => MissionType::LandRush,
        b'f' => MissionType::SpaceRace,
        b'r' => MissionType::ResourceRace,
        b'm' => MissionType::Midas,
        b'l' => MissionType::LastOneStanding,
        _ => return None,
    };
    matches!(b[2], b'2'..=b'6').then(|| (kind, b[2] - b'0'))
}

/// The game's checksum of a file (0x40C050), kept for the DLL and the map when a multiplayer game starts. The
/// `size mod 4` leading bytes are shifted in a byte at a time and the result rotated right by 1; then each following
/// little-endian u32 is XORed in, rotating right by 1 after each. A result of 0 becomes 1. The game tests the counts
/// with 16-bit `jcxz`, so a size whose low 16 bits are 0, or a dword count whose low 16 bits are 0 after leading
/// bytes, skips that part; this does the same.
pub fn checksum(data: &[u8]) -> u32 {
    let size = data.len() as u32;
    let mut sum: u32 = 0;
    if size & 0xFFFF != 0 {
        let lead = (size & 3) as usize;
        let dwords = size >> 2;
        let mut skip_dwords = false;
        if lead != 0 {
            for &b in &data[..lead] {
                sum = (sum & !0xFF | b as u32) << 8;
            }
            sum = sum.rotate_right(1);
            skip_dwords = dwords & 0xFFFF == 0;
        }
        if !skip_dwords {
            for chunk in data[lead..].as_chunks::<4>().0.iter().take(dwords as usize) {
                sum = (sum ^ u32::from_le_bytes(*chunk)).rotate_right(1);
            }
        }
    }
    if sum == 0 { 1 } else { sum }
}

struct Section {
    name: String,
    virtual_address: u32,
    virtual_size: u32,
    raw_size: u32,
    raw_pointer: u32,
    characteristics: u32,
}

/// The parts of a 32-bit PE file a mission description needs, read the way Windows does.
struct Pe<'a> {
    data: &'a [u8],
    optional_header_size: u16,
    sections: Vec<Section>,
    export_dir: Option<(u32, u32)>,
    /// Name index -> ordinal index, from the export table, to compare with the game's reader.
    name_ordinals: Vec<u16>,
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

impl<'a> Pe<'a> {
    fn parse(data: &'a [u8]) -> Result<Self, Error> {
        let bad = |what: &str| Error::Invalid(format!("not a PE file: {what}"));
        if data.get(..2) != Some(b"MZ") {
            return Err(bad("no MZ header"));
        }
        let pe = u32_at(data, 0x3C).ok_or_else(|| bad("truncated"))? as usize;
        if data.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err(bad("no PE signature"));
        }
        let section_count = u16_at(data, pe + 6).ok_or_else(|| bad("truncated"))? as usize;
        let optional_header_size = u16_at(data, pe + 20).ok_or_else(|| bad("truncated"))?;
        let opt = pe + 24;
        if u16_at(data, opt) != Some(0x10B) {
            return Err(Error::Unsupported("not a 32-bit PE file (mission DLLs are 32-bit)".into()));
        }
        let dir_count = u32_at(data, opt + 92).unwrap_or(0);
        let export_dir = (dir_count > 0 && optional_header_size >= 104)
            .then(|| Some((u32_at(data, opt + 96)?, u32_at(data, opt + 100)?)))
            .flatten()
            .filter(|&(rva, size)| rva != 0 && size != 0);
        let table = opt + optional_header_size as usize;
        let sections = (0..section_count)
            .map(|i| {
                let s = table + i * 40;
                let raw = data.get(s..s + 40).ok_or_else(|| bad("section table truncated"))?;
                let name = String::from_utf8_lossy(&raw[..8]).trim_end_matches('\0').to_string();
                let w = |o: usize| u32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
                Ok(Section {
                    name,
                    virtual_size: w(8),
                    virtual_address: w(12),
                    raw_size: w(16),
                    raw_pointer: w(20),
                    characteristics: w(36),
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Self { data, optional_header_size, sections, export_dir, name_ordinals: Vec::new() })
    }

    fn section_of(&self, rva: u32) -> Option<&Section> {
        self.sections.iter().find(|s| rva >= s.virtual_address && rva - s.virtual_address < s.virtual_size.max(s.raw_size))
    }

    /// The file offset of `rva`, when the file holds its byte.
    fn offset(&self, rva: u32) -> Option<usize> {
        let s = self.section_of(rva)?;
        let within = rva - s.virtual_address;
        (within < s.raw_size).then(|| (s.raw_pointer + within) as usize).filter(|&o| o < self.data.len())
    }

    fn bytes_at(&self, rva: u32, len: usize) -> Option<&'a [u8]> {
        let o = self.offset(rva)?;
        self.data.get(o..o + len)
    }

    /// A NUL-terminated string (Windows-1252, read as Latin-1 like the game's other text).
    fn cstr_at(&self, rva: u32) -> Option<String> {
        let o = self.offset(rva)?;
        let rest = &self.data[o..];
        let end = rest.iter().position(|&b| b == 0)?;
        Some(rest[..end].iter().map(|&b| b as char).collect())
    }

    fn exports(&mut self) -> Result<Vec<Export>, Error> {
        let Some((dir_rva, _)) = self.export_dir else { return Ok(Vec::new()) };
        let bad = || Error::Invalid("export table lies outside the file".into());
        let dir = self.bytes_at(dir_rva, 40).ok_or_else(bad)?;
        let w = |o: usize| u32::from_le_bytes(dir[o..o + 4].try_into().unwrap());
        let (address_count, name_count) = (w(20) as usize, w(24) as usize);
        let (addresses, names, ordinals) = (w(28), w(32), w(36));
        let mut out = Vec::with_capacity(name_count);
        let mut name_ordinals = Vec::with_capacity(name_count);
        for i in 0..name_count {
            let name_rva = self.bytes_at(names + 4 * i as u32, 4).ok_or_else(bad)?;
            let name = self.cstr_at(u32::from_le_bytes(name_rva.try_into().unwrap())).ok_or_else(bad)?;
            let ordinal = u16::from_le_bytes(self.bytes_at(ordinals + 2 * i as u32, 2).ok_or_else(bad)?.try_into().unwrap());
            if ordinal as usize >= address_count {
                return Err(Error::Invalid(format!("export {name} has ordinal index {ordinal} past the address table")));
            }
            let rva = u32::from_le_bytes(self.bytes_at(addresses + 4 * ordinal as u32, 4).ok_or_else(bad)?.try_into().unwrap());
            let section = self.section_of(rva);
            out.push(Export {
                name,
                rva,
                section: section.map(|s| s.name.clone()).unwrap_or_default(),
                is_code: section.is_some_and(|s| s.characteristics & (0x20 | 0x2000_0000) != 0),
            });
            name_ordinals.push(ordinal);
        }
        self.name_ordinals = name_ordinals;
        Ok(out)
    }

    fn game_reader_warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if self.optional_header_size != 224 {
            warnings.push(format!(
                "optional header is {} bytes; the game's reader takes the section table to follow a 224-byte one",
                self.optional_header_size
            ));
        }
        if self.sections.len() > 128 {
            warnings.push(format!("{} sections; the game's reader takes at most 128", self.sections.len()));
        }
        if self.name_ordinals.len() > 0x8000 {
            warnings.push("more than 0x8000 export names; the game's reader searches only 0x8000".into());
        }
        if self.name_ordinals.iter().enumerate().any(|(i, &o)| o as usize != i) {
            warnings.push(
                "export ordinals aren't in name order; the game's reader pairs name i with address i and gets the \
                 wrong export"
                    .into(),
            );
        }
        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal 32-bit DLL: one `.data` section holding the export table, the exports' data and their names.
    fn dll(exports: &[(&str, Vec<u8>)], optional_header_size: u16) -> Vec<u8> {
        const SECTION_RVA: u32 = 0x1000;
        const FILE_ALIGN: usize = 0x200;
        let pe = 0x40usize;
        let headers = pe + 24 + optional_header_size as usize + 40;
        let raw_start = headers.div_ceil(FILE_ALIGN) * FILE_ALIGN;

        // The section's contents: export directory, tables, then each export's data and name.
        let n = exports.len();
        let mut body = vec![0u8; 40 + n * 10];
        let addresses = 40;
        let names = addresses + 4 * n;
        let ordinals = names + 4 * n;
        let mut sorted: Vec<usize> = (0..n).collect();
        sorted.sort_by_key(|&i| exports[i].0);
        for (k, &i) in sorted.iter().enumerate() {
            let data_rva = SECTION_RVA + body.len() as u32;
            body.extend_from_slice(&exports[i].1);
            let name_rva = SECTION_RVA + body.len() as u32;
            body.extend_from_slice(exports[i].0.as_bytes());
            body.push(0);
            body[addresses + 4 * k..addresses + 4 * k + 4].copy_from_slice(&data_rva.to_le_bytes());
            body[names + 4 * k..names + 4 * k + 4].copy_from_slice(&name_rva.to_le_bytes());
            body[ordinals + 2 * k..ordinals + 2 * k + 2].copy_from_slice(&(k as u16).to_le_bytes());
        }
        let put = |b: &mut Vec<u8>, o: usize, v: u32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut body, 20, n as u32);
        put(&mut body, 24, n as u32);
        put(&mut body, 28, SECTION_RVA + addresses as u32);
        put(&mut body, 32, SECTION_RVA + names as u32);
        put(&mut body, 36, SECTION_RVA + ordinals as u32);
        body.resize(body.len().div_ceil(FILE_ALIGN) * FILE_ALIGN, 0);

        let mut f = vec![0u8; raw_start];
        f[..2].copy_from_slice(b"MZ");
        put(&mut f, 0x3C, pe as u32);
        f[pe..pe + 4].copy_from_slice(b"PE\0\0");
        f[pe + 4..pe + 6].copy_from_slice(&0x14Cu16.to_le_bytes());
        f[pe + 6..pe + 8].copy_from_slice(&1u16.to_le_bytes());
        f[pe + 20..pe + 22].copy_from_slice(&optional_header_size.to_le_bytes());
        let opt = pe + 24;
        f[opt..opt + 2].copy_from_slice(&0x10Bu16.to_le_bytes());
        put(&mut f, opt + 92, 16);
        put(&mut f, opt + 96, SECTION_RVA);
        put(&mut f, opt + 100, 40);
        let s = opt + optional_header_size as usize;
        f[s..s + 5].copy_from_slice(b".data");
        put(&mut f, s + 8, body.len() as u32);
        put(&mut f, s + 12, SECTION_RVA);
        put(&mut f, s + 16, body.len() as u32);
        put(&mut f, s + 20, raw_start as u32);
        put(&mut f, s + 36, 0xC000_0040);
        f.extend_from_slice(&body);
        f
    }

    fn desc_block(kind: i32, players: i32, tech: i32, unit_only: i32) -> Vec<u8> {
        [kind, players, tech, unit_only, 0, 0, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn text(s: &str) -> Vec<u8> {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        v
    }

    #[test]
    fn reads_a_mission_description() {
        let data = dll(
            &[
                ("DescBlock", desc_block(-8, 4, 12, 0)),
                ("LevelDesc", text("4 Player, LastOne, 'Test' map.")),
                ("MapName", text("TEST.map")),
                ("TechtreeName", text("MULTITEK.TXT")),
                ("DescBlockEx", vec![1, 0, 0, 0]),
            ],
            224,
        );
        let m = MissionDll::parse(&data).unwrap();
        assert_eq!(m.mission_type, MissionType::LastOneStanding);
        assert!(m.mission_type.is_multiplayer());
        assert_eq!((m.players, m.max_tech_level, m.unit_only), (4, 12, false));
        assert_eq!(m.level_desc, "4 Player, LastOne, 'Test' map.");
        assert_eq!((m.map_name.as_str(), m.techtree_name.as_str()), ("TEST.map", "MULTITEK.TXT"));
        assert_eq!(m.extra_ai_players, Some(1));
        assert_eq!(m.exports.len(), 5);
        assert!(m.has_export("MapName") && !m.export("DescBlock").unwrap().is_code);
        assert!(m.warnings.is_empty(), "{:?}", m.warnings);
        assert_eq!(m.checksum, checksum(&data));
    }

    #[test]
    fn campaign_and_unit_only() {
        let data = dll(
            &[
                ("DescBlock", desc_block(6, 2, 6, 1)),
                ("LevelDesc", text("Eden Mission #6")),
                ("MapName", text("EDEN06.map")),
                ("TechtreeName", text("EDENTEK.TXT")),
            ],
            224,
        );
        let m = MissionDll::parse(&data).unwrap();
        assert_eq!(m.mission_type, MissionType::Campaign(6));
        assert!(m.unit_only && m.extra_ai_players.is_none());
    }

    #[test]
    fn refuses_what_isnt_a_mission() {
        let not_mission = dll(&[("SomethingElse", vec![0; 4])], 224);
        assert!(MissionDll::parse(&not_mission).unwrap_err().to_string().contains("no DescBlock"));
        let incomplete = dll(&[("DescBlock", desc_block(-1, 2, 12, 0)), ("MapName", text("a.map"))], 224);
        assert!(MissionDll::parse(&incomplete).unwrap_err().to_string().contains("LevelDesc"));
        assert!(MissionDll::parse(b"not a PE file").is_err());
    }

    #[test]
    fn warns_about_what_the_game_reads_differently() {
        let data = dll(
            &[
                ("DescBlock", desc_block(-1, 2, 12, 0)),
                ("LevelDesc", text("Colony")),
                ("MapName", text("c.map")),
                ("TechtreeName", text("MULTITEK.TXT")),
            ],
            240,
        );
        let m = MissionDll::parse(&data).unwrap();
        assert_eq!(m.level_desc, "Colony", "Windows' rules still read it");
        assert!(m.warnings.iter().any(|w| w.contains("224")), "{:?}", m.warnings);
    }

    #[test]
    fn mission_types_round_trip() {
        for v in -9..=13 {
            assert_eq!(MissionType::from_value(v).value(), v);
        }
        assert_eq!(MissionType::from_value(0), MissionType::Other(0));
        assert!(!MissionType::Colony.is_multiplayer() && MissionType::LandRush.is_multiplayer());
    }

    #[test]
    fn multiplayer_names() {
        assert_eq!(multiplayer_name("ml4_21.dll"), Some((MissionType::LastOneStanding, 4)));
        assert_eq!(multiplayer_name("MU2_10.DLL"), Some((MissionType::LandRush, 2)));
        assert_eq!(multiplayer_name("mf6x.dll"), Some((MissionType::SpaceRace, 6)));
        assert_eq!(multiplayer_name("ml_demo.dll"), None, "no digit");
        assert_eq!(multiplayer_name("ml7a.dll"), None, "2 to 6 players");
        assert_eq!(multiplayer_name("mx4.dll"), None);
        assert_eq!(multiplayer_name("c_euf.dll"), None);
    }

    #[test]
    fn checksum_follows_the_game() {
        // Dwords only: XOR in, rotate right 1.
        let d = [1u8, 0, 0, 0, 2, 0, 0, 0];
        assert_eq!(checksum(&d), (1u32.rotate_right(1) ^ 2).rotate_right(1));
        // One leading byte, shifted in and rotated, then the dword.
        let d = [0xAB, 4, 3, 2, 1];
        assert_eq!(checksum(&d), ((0xABu32 << 8).rotate_right(1) ^ 0x0102_0304).rotate_right(1));
        // Leading bytes shift each other up.
        let d = [1u8, 2, 3];
        assert_eq!(checksum(&d), (((1u32 << 8 | 2) << 8 | 3) << 8).rotate_right(1));
        // 0 becomes 1, and so does a size whose low 16 bits are 0.
        assert_eq!(checksum(&[0; 8]), 1);
        assert_eq!(checksum(&[0x55; 0x10000]), 1);
        assert_eq!(checksum(&[]), 1);
    }
}
