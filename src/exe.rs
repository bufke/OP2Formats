//! Tables read from the player's own `Outpost2.exe` (1.2 with OPU, the version the addresses in this project are
//! for) at runtime, rather than copied into the repository: a minimal PE reader that maps virtual addresses to file
//! bytes. Input is untrusted: every read is bounds-checked and a table that doesn't look right is an error.

use crate::{Error, GameFiles};

/// The executable's image: the file plus its section table.
pub struct Exe {
    data: Vec<u8>,
    base: u32,
    /// (virtual address, virtual size, raw size, raw offset)
    sections: Vec<(u32, u32, u32, u32)>,
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
            sections.push((base.wrapping_add(f(12)?), f(8)?, f(16)?, f(20)?));
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

    fn offset(&self, va: u32, len: usize) -> Option<usize> {
        self.sections.iter().find_map(|&(start, vsize, rsize, raw)| {
            let rel = va.checked_sub(start)?;
            (rel < vsize.max(rsize) && rel as usize + len <= rsize as usize).then(|| raw as usize + rel as usize)
        })
    }

    pub fn u32(&self, va: u32) -> Option<u32> {
        let o = self.offset(va, 4)?;
        self.data.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// A NUL-terminated string (at most 255 bytes) at `va`.
    pub fn cstr(&self, va: u32) -> Option<String> {
        let o = self.offset(va, 1)?;
        let tail = &self.data[o..self.data.len().min(o + 256)];
        let end = tail.iter().position(|&b| b == 0)?;
        Some(tail[..end].iter().map(|&b| b as char).collect())
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

    fn bytes(&self, va: u32, len: usize) -> Option<&[u8]> {
        let o = self.offset(va, len)?;
        self.data.get(o..o + len)
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

    #[test]
    fn reads_the_sound_table() {
        let Some(files) = crate::testdata::game_files() else { return };
        let exe = Exe::open(&files).expect("Outpost2.exe");
        let names = exe.sound_files().expect("sound table");
        assert_eq!(names.len(), SOUND_COUNT);
        assert!(names.iter().all(|n| n.ends_with(".wav")));
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
