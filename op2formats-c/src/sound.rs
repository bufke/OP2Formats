//! Sound: `.clm` music archives (raw PCM tracks sharing one format) and RIFF `.wav` files.

use crate::*;
use op2_formats::sound::{Clm, ClmEntry, ClmIndex, WaveFormat};
use std::ffi::c_char;
use std::fs::File;
use std::path::PathBuf;

/// A WAVEFORMATEX's fields.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fWaveFormat {
    /// 1 for PCM.
    pub format_tag: u16,
    pub channels: u16,
    pub samples_per_sec: u32,
    pub avg_bytes_per_sec: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
}

pub(crate) fn wave_format(f: &WaveFormat) -> Op2fWaveFormat {
    Op2fWaveFormat {
        format_tag: f.format_tag,
        channels: f.channels,
        samples_per_sec: f.samples_per_sec,
        avg_bytes_per_sec: f.avg_bytes_per_sec,
        block_align: f.block_align,
        bits_per_sample: f.bits_per_sample,
    }
}

/// A `.clm` music archive (`op2.clm`). Opened from a file, only its index is read; tracks are read when asked for.
pub struct Op2fClm(pub(crate) ClmSource);

pub(crate) enum ClmSource {
    File { path: PathBuf, index: ClmIndex },
    Memory(Clm),
}

impl ClmSource {
    fn format(&self) -> &WaveFormat {
        match self {
            ClmSource::File { index, .. } => &index.format,
            ClmSource::Memory(clm) => &clm.format,
        }
    }

    fn entries(&self) -> &[ClmEntry] {
        match self {
            ClmSource::File { index, .. } => &index.entries,
            ClmSource::Memory(clm) => &clm.entries,
        }
    }

    fn samples(&self, entry: &ClmEntry) -> Result<Vec<u8>, Op2fStatus> {
        match self {
            ClmSource::File { path, index } => {
                let mut f = File::open(path).map_err(|e| fail(Op2fStatus::Io, format!("{}: {e}", path.display())))?;
                index.read_samples(&mut f, entry).map_err(lib_err)
            }
            ClmSource::Memory(clm) => Ok(clm.samples(entry).to_vec()),
        }
    }
}

fn open_clm_file(path: PathBuf) -> Result<Op2fClm, Op2fStatus> {
    let mut f = File::open(&path).map_err(|e| fail(Op2fStatus::NotFound, format!("{}: {e}", path.display())))?;
    let index = ClmIndex::read(&mut f).map_err(lib_err)?;
    Ok(Op2fClm(ClmSource::File { path, index }))
}

/// Opens the game's `op2.clm` (OPU's copy first).
///
/// # Safety
/// `game` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_clm_open(game: *const Op2fGame, out: *mut *mut Op2fClm) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let path = game.files.loose_path("op2.clm").ok_or_else(|| fail(Op2fStatus::NotFound, "no op2.clm in the game folder"))?;
        unsafe { put(out, open_clm_file(path.to_path_buf())?) }
    })
}

/// Opens a `.clm` file by path.
///
/// # Safety
/// `path` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_clm_open_file(path: *const c_char, out: *mut *mut Op2fClm) -> Op2fStatus {
    guard(|| unsafe { put(out, open_clm_file(path_arg(path, "path")?)?) })
}

/// Parses a whole `.clm` from memory (the bytes are copied).
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_clm_parse(data: *const u8, len: usize, out: *mut *mut Op2fClm) -> Op2fStatus {
    guard(|| {
        let clm = Clm::parse(unsafe { bytes_arg(data, len) }?.to_vec()).map_err(lib_err)?;
        unsafe { put(out, Op2fClm(ClmSource::Memory(clm))) }
    })
}

/// # Safety
/// `clm` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_clm_free(clm: *mut Op2fClm) {
    unsafe { free(clm) }
}

/// The format every track shares.
///
/// # Safety
/// `clm` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_clm_format(clm: *const Op2fClm, out: *mut Op2fWaveFormat) -> Op2fStatus {
    guard(|| unsafe { put_value(out, wave_format(obj(clm, "clm")?.0.format())) })
}

/// Number of tracks.
///
/// # Safety
/// `clm` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_clm_track_count(clm: *const Op2fClm) -> usize {
    unsafe { clm.as_ref() }.map_or(0, |c| c.0.entries().len())
}

/// Track `index`'s name (up to 8 characters, e.g. "op2_01") and, when `bytes` isn't NULL, its size in bytes.
///
/// # Safety
/// `clm` is a live handle; `bytes` NULL or writable; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_clm_track(
    clm: *const Op2fClm,
    index: usize,
    bytes: *mut u32,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let entry = self::index(unsafe { obj(clm, "clm") }?.0.entries(), index, "track")?;
        if !bytes.is_null() {
            unsafe { bytes.write(entry.length) };
        }
        unsafe { copy_str(&entry.name, buf, cap, len) }
    })
}

/// Finds a track by name, ignoring case.
///
/// # Safety
/// `clm` is a live handle; `name` a string; `index` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_clm_find(clm: *const Op2fClm, name: *const c_char, index: *mut usize) -> Op2fStatus {
    guard(|| {
        let entries = unsafe { obj(clm, "clm") }?.0.entries();
        let name = unsafe { str_arg(name, "name") }?;
        let i = entries.iter().position(|e| e.name.eq_ignore_ascii_case(name));
        let i = i.ok_or_else(|| fail(Op2fStatus::NotFound, format!("no track {name}")))?;
        unsafe { put_value(index, i) }
    })
}

/// Track `index`'s raw PCM samples, in `op2f_clm_format`'s format.
///
/// # Safety
/// `clm` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_clm_samples(clm: *const Op2fClm, index: usize, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let clm = &unsafe { obj(clm, "clm") }?.0;
        let entry = self::index(clm.entries(), index, "track")?;
        unsafe { put(out, Op2fBytes(clm.samples(entry)?)) }
    })
}

/// Track `index` as a standalone `.wav` file.
///
/// # Safety
/// `clm` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_clm_wav(clm: *const Op2fClm, index: usize, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let clm = &unsafe { obj(clm, "clm") }?.0;
        let entry = self::index(clm.entries(), index, "track")?;
        let wav = match clm {
            ClmSource::Memory(c) => c.wav(entry),
            ClmSource::File { index, .. } => {
                // Clm::wav needs the whole archive in memory; this writes the same header around one track.
                wav_file(&index.format, entry, clm.samples(entry)?)
            }
        };
        unsafe { put(out, Op2fBytes(wav)) }
    })
}

fn wav_file(format: &WaveFormat, entry: &ClmEntry, samples: Vec<u8>) -> Vec<u8> {
    let f = format;
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
    out.extend_from_slice(&entry.length.to_le_bytes());
    out.extend_from_slice(&samples);
    out
}

/// A `.wav` file's format and, into `*data_len`, how many bytes of samples it has.
///
/// # Safety
/// `data` is `len` readable bytes; `format` and `data_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_wav_info(data: *const u8, len: usize, format: *mut Op2fWaveFormat, data_len: *mut usize) -> Op2fStatus {
    guard(|| {
        let (f, n) = op2_formats::sound::wav_info(unsafe { bytes_arg(data, len) }?).map_err(lib_err)?;
        unsafe { put_value(format, wave_format(&f)) }?;
        unsafe { put_value(data_len, n) }
    })
}

/// How long `bytes` of sample data play in `format`, in seconds.
///
/// # Safety
/// `format` is readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_wave_seconds(format: *const Op2fWaveFormat, bytes: usize) -> f64 {
    unsafe { format.as_ref() }.map_or(0.0, |f| bytes as f64 / f.avg_bytes_per_sec.max(1) as f64)
}
