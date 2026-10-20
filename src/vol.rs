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
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

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
    storage: Storage,
    entries: Vec<VolEntry>,
}

/// Where the blocks are: in memory, or in the file, read when asked for (the game's archives are ~20 MB together, and
/// a game reads a few files from them once).
enum Storage {
    Memory(Vec<u8>),
    File(PathBuf),
}

impl VolArchive {
    pub fn parse(data: Vec<u8>) -> Result<Self, Error> {
        let entries = Self::parse_index(&data)?;
        Ok(Self { storage: Storage::Memory(data), entries })
    }

    /// Reads only the archive's index from `path`; entries are read from the file when asked for.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let io = |source| Error::Io { path: path.display().to_string(), source };
        let mut f = std::fs::File::open(path).map_err(io)?;
        let mut head = [0u8; 8];
        f.read_exact(&mut head).map_err(io)?;
        // "VOL " and the length of the header sections (volh, vols, voli) that follow.
        let header_len = (u32::from_le_bytes(head[4..8].try_into().expect("4 bytes")) & LENGTH_MASK) as usize;
        let mut index = head.to_vec();
        f.take(header_len as u64).read_to_end(&mut index).map_err(io)?;
        let entries = Self::parse_index(&index)?;
        Ok(Self { storage: Storage::File(path.to_path_buf()), entries })
    }

    fn parse_index(data: &[u8]) -> Result<Vec<VolEntry>, Error> {
        let mut r = Reader::new(data);
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

        let mut r = Reader::at(data, vols_start + vols_len);
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
        Ok(entries)
    }

    pub fn entries(&self) -> &[VolEntry] {
        &self.entries
    }

    /// Raw bytes of an entry. Only uncompressed entries are supported; see `data` for all.
    pub fn read(&self, entry: &VolEntry) -> Result<Cow<'_, [u8]>, Error> {
        if entry.compression != Compression::None {
            return Err(Error::Unsupported(format!(
                "{} is {:?}-compressed; use VolArchive::data to decompress",
                entry.name, entry.compression
            )));
        }
        self.stored(entry)
    }

    /// The entry's bytes as stored in its VBLK block (still compressed if it is).
    pub fn stored(&self, entry: &VolEntry) -> Result<Cow<'_, [u8]>, Error> {
        match &self.storage {
            Storage::Memory(data) => {
                let mut r = Reader::at(data, entry.block_offset);
                r.expect_tag(b"VBLK")?;
                let len = (r.u32()? & LENGTH_MASK) as usize;
                r.bytes(len).map(Cow::Borrowed)
            }
            Storage::File(path) => {
                let io = |source| Error::Io { path: path.display().to_string(), source };
                let mut f = std::fs::File::open(path).map_err(io)?;
                f.seek(SeekFrom::Start(entry.block_offset as u64)).map_err(io)?;
                let mut head = [0u8; 8];
                f.read_exact(&mut head).map_err(io)?;
                Reader::new(&head).expect_tag(b"VBLK").map_err(|e| match e {
                    Error::BadTag { expected, found, .. } => Error::BadTag { offset: entry.block_offset, expected, found },
                    e => e,
                })?;
                let len = (u32::from_le_bytes(head[4..8].try_into().expect("4 bytes")) & LENGTH_MASK) as usize;
                let mut data = Vec::with_capacity(len);
                f.take(len as u64).read_to_end(&mut data).map_err(io)?;
                if data.len() < len {
                    return Err(Error::Truncated { offset: entry.block_offset + 8 + data.len(), wanted: len - data.len() });
                }
                Ok(Cow::Owned(data))
            }
        }
    }

    /// The entry's contents, decompressing LZH entries.
    pub fn data(&self, entry: &VolEntry) -> Result<Cow<'_, [u8]>, Error> {
        match entry.compression {
            Compression::None => self.stored(entry),
            Compression::Lzh => Ok(Cow::Owned(crate::lzh::decompress(&self.stored(entry)?))),
            other => Err(Error::Unsupported(format!("{} uses unsupported compression {other:?}", entry.name))),
        }
    }

    pub fn find(&self, name: &str) -> Option<&VolEntry> {
        self.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-file archive, uncompressed.
    fn sample() -> Vec<u8> {
        let files: [(&str, &[u8]); 2] = [("a.txt", b"hello"), ("B.MAP", b"map!")];
        let names: Vec<u8> = files.iter().flat_map(|(n, _)| n.bytes().chain([0])).collect();
        let mut vols = (names.len() as u32).to_le_bytes().to_vec();
        vols.extend(&names);
        while !vols.len().is_multiple_of(4) {
            vols.push(0);
        }
        let header_len = 8 + 8 + vols.len() + 8 + 14 * files.len();
        let (mut blocks, mut index, mut name_at) = (Vec::new(), Vec::new(), 0);
        for (n, data) in files {
            index.extend((name_at as u32).to_le_bytes());
            index.extend(((8 + header_len + blocks.len()) as u32).to_le_bytes());
            index.extend((data.len() as u32).to_le_bytes());
            index.extend(0x100u16.to_le_bytes());
            name_at += n.len() + 1;
            blocks.extend(b"VBLK");
            blocks.extend((data.len() as u32 | 0x8000_0000).to_le_bytes());
            blocks.extend(data);
            while !blocks.len().is_multiple_of(4) {
                blocks.push(0);
            }
        }
        let mut out = b"VOL ".to_vec();
        out.extend((header_len as u32 | 0x8000_0000).to_le_bytes());
        out.extend(b"volh");
        out.extend(0x8000_0000u32.to_le_bytes());
        out.extend(b"vols");
        out.extend((vols.len() as u32 | 0x8000_0000).to_le_bytes());
        out.extend(&vols);
        out.extend(b"voli");
        out.extend(((14 * files.len()) as u32 | 0x8000_0000).to_le_bytes());
        out.extend(&index);
        out.extend(&blocks);
        out
    }

    #[test]
    fn reads_from_memory_and_from_the_file_alike() {
        let bytes = sample();
        let path = std::env::temp_dir().join(format!("op2-formats-vol-{}.vol", std::process::id()));
        std::fs::write(&path, &bytes).unwrap();
        let (mem, file) = (VolArchive::parse(bytes).unwrap(), VolArchive::open(&path).unwrap());
        for vol in [&mem, &file] {
            assert_eq!(vol.entries().iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["a.txt", "B.MAP"]);
            assert_eq!(&*vol.data(vol.find("A.TXT").unwrap()).unwrap(), b"hello");
            assert_eq!(&*vol.data(vol.find("b.map").unwrap()).unwrap(), b"map!");
        }
        let _ = std::fs::remove_file(&path);
    }
}
