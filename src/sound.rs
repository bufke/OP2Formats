//! Sound: `.clm` music archives (raw PCM clips sharing one wave format) and the RIFF `.wav`
//! files in `sound.vol` / `voices.vol`. Layout follows OP2Utility's ClmFile:
//!
//! ```text
//! header  32-byte version string, WAVEFORMATEX (18 bytes), 6 unknown bytes, u32 count
//! index   count x (8-byte name, u32 data offset, u32 data length)
//! ```

use crate::Error;
use crate::reader::Reader;

/// The PCM fields of a WAVEFORMATEX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveFormat {
    pub format_tag: u16,
    pub channels: u16,
    pub samples_per_sec: u32,
    pub avg_bytes_per_sec: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
}

impl WaveFormat {
    fn read(r: &mut Reader) -> Result<Self, Error> {
        Ok(Self {
            format_tag: r.u16()?,
            channels: r.u16()?,
            samples_per_sec: r.u32()?,
            avg_bytes_per_sec: r.u32()?,
            block_align: r.u16()?,
            bits_per_sample: r.u16()?,
        })
    }

    /// Playing time of `bytes` of sample data.
    pub fn seconds(&self, bytes: usize) -> f64 {
        bytes as f64 / self.avg_bytes_per_sec.max(1) as f64
    }
}

#[derive(Debug, Clone)]
pub struct ClmEntry {
    pub name: String,
    pub offset: u32,
    pub length: u32,
}

pub struct Clm {
    pub format: WaveFormat,
    pub entries: Vec<ClmEntry>,
    data: Vec<u8>,
}

impl Clm {
    pub fn parse(data: Vec<u8>) -> Result<Self, Error> {
        let mut r = Reader::new(&data);
        let version = r.bytes(32)?;
        if !version.starts_with(b"OP2 Clump File") {
            return Err(Error::Invalid("not a .clm file (missing 'OP2 Clump File' header)".into()));
        }
        let format = WaveFormat::read(&mut r)?;
        r.u16()?; // cbSize
        r.skip(6)?;
        let count = r.u32()?;
        let mut entries = Vec::new();
        for _ in 0..count {
            let name = r.bytes(8)?;
            let name = String::from_utf8_lossy(name.split(|&b| b == 0).next().unwrap_or(&[])).into_owned();
            let entry = ClmEntry { name, offset: r.u32()?, length: r.u32()? };
            if entry.offset as usize + entry.length as usize > data.len() {
                return Err(Error::Invalid(format!("clm entry {} runs past the end of the file", entry.name)));
            }
            entries.push(entry);
        }
        Ok(Self { format, entries, data })
    }

    /// Raw PCM samples of an entry.
    pub fn samples(&self, entry: &ClmEntry) -> &[u8] {
        &self.data[entry.offset as usize..(entry.offset + entry.length) as usize]
    }

    /// An entry as a standalone `.wav` file.
    pub fn wav(&self, entry: &ClmEntry) -> Vec<u8> {
        let samples = self.samples(entry);
        let f = &self.format;
        let mut out = Vec::with_capacity(samples.len() + 44);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        for v in [f.format_tag, f.channels] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in [f.samples_per_sec, f.avg_bytes_per_sec] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in [f.block_align, f.bits_per_sample] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        out.extend_from_slice(samples);
        out
    }
}

/// Format and sample-data length of a RIFF WAVE file.
pub fn wav_info(data: &[u8]) -> Result<(WaveFormat, usize), Error> {
    let mut r = Reader::new(data);
    r.expect_tag(b"RIFF")?;
    r.u32()?;
    r.expect_tag(b"WAVE")?;
    let mut format = None;
    while r.pos() + 8 <= data.len() {
        let tag = r.tag()?;
        let len = r.u32()? as usize;
        let start = r.pos();
        match &tag {
            b"fmt " => format = Some(WaveFormat::read(&mut r)?),
            b"data" => {
                let format = format.ok_or_else(|| Error::Invalid("wav data chunk before fmt".into()))?;
                return Ok((format, len.min(data.len() - start)));
            }
            _ => {}
        }
        r = Reader::at(data, start + len + (len & 1));
    }
    Err(Error::Invalid("wav file has no data chunk".into()))
}
