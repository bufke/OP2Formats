//! Archives, sound and video.

use crate::*;
use std::ffi::c_char;

/// A `.vol` archive.
pub struct Op2fVol(pub(crate) VolArchive);

// ---- .vol archives ----

/// Opens a `.vol` archive by path. Entries are read from the file when asked for.
///
/// # Safety
/// `path` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_open(path: *const c_char, out: *mut *mut Op2fVol) -> Op2fStatus {
    guard(|| {
        let path = unsafe { str_arg(path, "path") }?;
        let vol = VolArchive::open(path.as_ref()).map_err(lib_err)?;
        unsafe { put(out, Op2fVol(vol)) }
    })
}

/// Parses a `.vol` archive from memory (the bytes are copied).
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_parse(data: *const u8, len: usize, out: *mut *mut Op2fVol) -> Op2fStatus {
    guard(|| {
        let vol = VolArchive::parse(unsafe { bytes_arg(data, len) }?.to_vec()).map_err(lib_err)?;
        unsafe { put(out, Op2fVol(vol)) }
    })
}

/// # Safety
/// `vol` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_free(vol: *mut Op2fVol) {
    unsafe { free(vol) }
}

/// Number of entries.
///
/// # Safety
/// `vol` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_entry_count(vol: *const Op2fVol) -> usize {
    unsafe { vol.as_ref() }.map_or(0, |v| v.0.entries().len())
}

/// Entry `index`'s file name.
///
/// # Safety
/// `vol` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_entry_name(vol: *const Op2fVol, index: usize, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let vol = &unsafe { obj(vol, "vol") }?.0;
        unsafe { copy_str(&self::index(vol.entries(), index, "entry")?.name, buf, cap, len) }
    })
}

/// Finds an entry by file name, ignoring case.
///
/// # Safety
/// `vol` is a live handle; `name` a string; `index` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_find(vol: *const Op2fVol, name: *const c_char, index: *mut usize) -> Op2fStatus {
    guard(|| {
        let vol = &unsafe { obj(vol, "vol") }?.0;
        let name = unsafe { str_arg(name, "name") }?;
        let found = vol.entries().iter().position(|e| e.name.eq_ignore_ascii_case(name));
        let i = found.ok_or_else(|| fail(Op2fStatus::NotFound, format!("{name} not in the archive")))?;
        unsafe { put_value(index, i) }
    })
}

/// Entry `index`'s contents, decompressed.
///
/// # Safety
/// `vol` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_read(vol: *const Op2fVol, index: usize, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let vol = &unsafe { obj(vol, "vol") }?.0;
        let entry = self::index(vol.entries(), index, "entry")?;
        let data = vol.data(entry).map_err(lib_err)?.into_owned();
        unsafe { put(out, Op2fBytes(data)) }
    })
}

/// How a `.vol` entry is stored. Only `NONE` and `LZH` are read (the game's archives use nothing else).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fCompression {
    None,
    Rle,
    Lz,
    Lzh,
    Unknown,
}

/// A `.vol` entry's storage.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fVolEntryInfo {
    pub compression: Op2fCompression,
    /// The compression code as stored (0x100 none, 0x101 RLE, 0x102 LZ, 0x103 LZH).
    pub compression_code: u16,
    /// The size in the index: for compressed entries the stored size, not the decoded one.
    pub size: u32,
}

/// # Safety
/// `vol` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_entry_info(vol: *const Op2fVol, index: usize, out: *mut Op2fVolEntryInfo) -> Op2fStatus {
    guard(|| {
        use op2_formats::vol::Compression;
        let entry = self::index(unsafe { obj(vol, "vol") }?.0.entries(), index, "entry")?;
        let (compression, compression_code) = match entry.compression {
            Compression::None => (Op2fCompression::None, 0x100),
            Compression::Rle => (Op2fCompression::Rle, 0x101),
            Compression::Lz => (Op2fCompression::Lz, 0x102),
            Compression::Lzh => (Op2fCompression::Lzh, 0x103),
            Compression::Unknown(code) => (Op2fCompression::Unknown, code),
        };
        unsafe { put_value(out, Op2fVolEntryInfo { compression, compression_code, size: entry.size }) }
    })
}

/// Entry `index`'s bytes as stored (still compressed if it is).
///
/// # Safety
/// `vol` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vol_read_stored(vol: *const Op2fVol, index: usize, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let vol = &unsafe { obj(vol, "vol") }?.0;
        let entry = self::index(vol.entries(), index, "entry")?;
        unsafe { put(out, Op2fBytes(vol.stored(entry).map_err(lib_err)?.into_owned())) }
    })
}

/// Decompresses LZH data (a `.vol` entry with compression 0x103).
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_lzh_decompress(data: *const u8, len: usize, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let data = unsafe { bytes_arg(data, len) }?;
        unsafe { put(out, Op2fBytes(op2_formats::lzh::decompress(data))) }
    })
}
