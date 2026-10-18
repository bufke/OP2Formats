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
}
