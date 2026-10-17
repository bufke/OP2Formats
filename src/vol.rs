//! `.vol` archives: a flat list of named files, stored raw or LZH-compressed (see `lzh`).
//! Layout (all sections are `tag, u32 length`):
//!
//! ```text
//! "VOL " header_len
//!   "volh" 0
//!   "vols" len   u32 names_len, NUL-separated names, padding
//!   "voli" len   14-byte index entries
//! per file: "VBLK" len, data, pad to 4 bytes
//! ```
//!
//! The top bit of each section length is a padding flag, not part of the length.

use std::borrow::Cow;

use crate::Error;
use crate::reader::Reader;

const LENGTH_MASK: u32 = 0x7FFF_FFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None,
    Rle,
    Lz,
    Lzh,
    Unknown(u16),
}

impl Compression {
    fn from_code(code: u16) -> Self {
        match code {
            0x100 => Self::None,
            0x101 => Self::Rle,
            0x102 => Self::Lz,
            0x103 => Self::Lzh,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VolEntry {
    pub name: String,
    pub compression: Compression,
    /// Size from the index entry (for LZH entries, the stored size, not the decoded one).
    pub size: u32,
    block_offset: usize,
}

pub struct VolArchive {
    data: Vec<u8>,
    entries: Vec<VolEntry>,
}

impl VolArchive {
    pub fn parse(data: Vec<u8>) -> Result<Self, Error> {
        let mut r = Reader::new(&data);
        r.expect_tag(b"VOL ")?;
        r.u32()?;
        r.expect_tag(b"volh")?;
        r.u32()?;

        r.expect_tag(b"vols")?;
        let vols_len = (r.u32()? & LENGTH_MASK) as usize;
        let vols_start = r.pos();
        let names_len = r.u32()? as usize;
        let names: Vec<String> = r
            .bytes(names_len)?
            .split(|&b| b == 0)
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect();

        let mut r = Reader::at(&data, vols_start + vols_len);
        r.expect_tag(b"voli")?;
        let index_len = (r.u32()? & LENGTH_MASK) as usize;

        let mut entries = Vec::new();
        for name in names.into_iter().take(index_len / 14) {
            let filename_offset = r.u32()?;
            let block_offset = r.u32()? as usize;
            let size = r.u32()?;
            let compression = Compression::from_code(r.u16()?);
            if filename_offset == u32::MAX {
                break;
            }
            entries.push(VolEntry { name, compression, size, block_offset });
        }

        Ok(Self { data, entries })
    }

    pub fn entries(&self) -> &[VolEntry] {
        &self.entries
    }

    /// Raw bytes of an entry. Only uncompressed entries are supported; see `data` for all.
    pub fn read(&self, entry: &VolEntry) -> Result<&[u8], Error> {
        if entry.compression != Compression::None {
            return Err(Error::Unsupported(format!(
                "{} is {:?}-compressed; use VolArchive::data to decompress",
                entry.name, entry.compression
            )));
        }
        self.stored(entry)
    }

    /// The entry's bytes as stored in its VBLK block (still compressed if it is).
    pub fn stored(&self, entry: &VolEntry) -> Result<&[u8], Error> {
        let mut r = Reader::at(&self.data, entry.block_offset);
        r.expect_tag(b"VBLK")?;
        let len = (r.u32()? & LENGTH_MASK) as usize;
        r.bytes(len)
    }

    /// The entry's contents, decompressing LZH entries.
    pub fn data(&self, entry: &VolEntry) -> Result<Cow<'_, [u8]>, Error> {
        match entry.compression {
            Compression::None => self.stored(entry).map(Cow::Borrowed),
            Compression::Lzh => Ok(Cow::Owned(crate::lzh::decompress(self.stored(entry)?))),
            other => Err(Error::Unsupported(format!("{} uses unsupported compression {other:?}", entry.name))),
        }
    }

    pub fn find(&self, name: &str) -> Option<&VolEntry> {
        self.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name))
    }
}
