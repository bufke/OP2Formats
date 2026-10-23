//! C interface to op2-formats, for C, C++ and any language that can call a C library.
//!
//! `include/op2formats.h` is generated from these files by cbindgen; the calling conventions (handles, status codes,
//! string buffers, panics) are written at its top, in `cbindgen.toml`. Keep them true when changing the interface.
//! This file has the conventions' helpers; each area of op2-formats has its own module.

use std::cell::RefCell;
use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};

use op2_formats::Error;
pub(crate) use op2_formats::sheets::{self, Sheet};
pub(crate) use op2_formats::units;
pub(crate) use op2_formats::{GameFiles, Map, Sprite, Sprites, TILE_RGBA_BYTES, Tileset, Tint, VolArchive};

mod art;
mod common;
mod exe;
mod game;
mod install;
mod installers;
mod map;
mod media;
mod render;
mod resources;
mod rules;
mod sound;
mod sprites;
mod stats;
mod unit_art;
mod video;

pub use art::*;
pub use common::*;
pub use exe::*;
pub use game::*;
pub use install::*;
pub use installers::*;
pub use map::*;
pub use media::*;
pub use render::*;
pub use resources::*;
pub use rules::*;
pub use sound::*;
pub use sprites::*;
pub use stats::*;
pub use unit_art::*;
pub use video::*;

/// Result of a call that can fail.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fStatus {
    Ok = 0,
    /// A required pointer argument was NULL.
    NullArgument,
    /// A string argument isn't valid UTF-8.
    InvalidUtf8,
    /// An index or coordinate is out of range.
    OutOfRange,
    /// The output buffer was too small: it holds a truncated copy, and `*len` the full length.
    BufferTooSmall,
    /// The file or folder doesn't exist.
    NotFound,
    /// Reading a file failed.
    Io,
    /// The data is malformed, or not the expected format.
    InvalidData,
    /// The data uses something the library doesn't read (e.g. an unknown compression).
    Unsupported,
    /// A bug in the library; the message says where.
    Panic,
}

/// A block of bytes the library allocated.
pub struct Op2fBytes(Vec<u8>);

/// An RGBA image the library drew: a sprite image or a composited animation frame.
pub struct Op2fImage(pub(crate) Sprite);

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Records `message` as this thread's last error and returns `status`.
pub(crate) fn fail(status: Op2fStatus, message: impl Into<String>) -> Op2fStatus {
    LAST_ERROR.with(|e| *e.borrow_mut() = message.into());
    status
}

pub(crate) fn lib_err(e: Error) -> Op2fStatus {
    let status = match e {
        Error::NotFound(_) => Op2fStatus::NotFound,
        Error::Io { .. } => Op2fStatus::Io,
        Error::Unsupported(_) => Op2fStatus::Unsupported,
        Error::Truncated { .. } | Error::BadTag { .. } | Error::Invalid(_) => Op2fStatus::InvalidData,
    };
    fail(status, e.to_string())
}

/// Runs `f`, turning a panic into [`Op2fStatus::Panic`].
pub(crate) fn guard(f: impl FnOnce() -> Result<(), Op2fStatus>) -> Op2fStatus {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => Op2fStatus::Ok,
        Ok(Err(status)) => status,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            fail(Op2fStatus::Panic, format!("internal error in op2formats: {msg}"))
        }
    }
}

pub(crate) fn null(what: &str) -> Op2fStatus {
    fail(Op2fStatus::NullArgument, format!("{what} is NULL"))
}

/// # Safety
/// `p` is NULL or a NUL-terminated string that stays valid for `'a`.
pub(crate) unsafe fn str_arg<'a>(p: *const c_char, what: &str) -> Result<&'a str, Op2fStatus> {
    if p.is_null() {
        return Err(null(what));
    }
    unsafe { CStr::from_ptr(p) }.to_str().map_err(|_| fail(Op2fStatus::InvalidUtf8, format!("{what} isn't valid UTF-8")))
}

/// # Safety
/// `p` is NULL or a live handle that stays valid for `'a`.
pub(crate) unsafe fn obj<'a, T>(p: *const T, what: &str) -> Result<&'a T, Op2fStatus> {
    unsafe { p.as_ref() }.ok_or_else(|| null(what))
}

/// Moves `value` into a new handle written to `*out`.
///
/// # Safety
/// `out` is NULL or writable.
pub(crate) unsafe fn put<T>(out: *mut *mut T, value: T) -> Result<(), Op2fStatus> {
    if out.is_null() {
        return Err(null("out"));
    }
    unsafe { out.write(Box::into_raw(Box::new(value))) };
    Ok(())
}

/// # Safety
/// `out` is NULL or writable.
pub(crate) unsafe fn put_value<T>(out: *mut T, value: T) -> Result<(), Op2fStatus> {
    if out.is_null() {
        return Err(null("out"));
    }
    unsafe { out.write(value) };
    Ok(())
}

/// Copies `s` into the caller's buffer as described in the module docs.
///
/// # Safety
/// `buf` is NULL or `cap` writable bytes; `len` is NULL or writable.
pub(crate) unsafe fn copy_str(s: &str, buf: *mut c_char, cap: usize, len: *mut usize) -> Result<(), Op2fStatus> {
    if !len.is_null() {
        unsafe { len.write(s.len()) };
    }
    if buf.is_null() {
        return if cap == 0 { Ok(()) } else { Err(null("buf")) };
    }
    if cap == 0 {
        return Err(fail(Op2fStatus::BufferTooSmall, "buffer has no room"));
    }
    let n = s.len().min(cap - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr(), buf.cast::<u8>(), n);
        buf.add(n).write(0);
    }
    if n < s.len() {
        return Err(fail(Op2fStatus::BufferTooSmall, format!("string needs {} bytes plus the NUL, buffer holds {cap}", s.len())));
    }
    Ok(())
}

/// # Safety
/// `data` is NULL or `len` readable bytes.
pub(crate) unsafe fn bytes_arg<'a>(data: *const u8, len: usize) -> Result<&'a [u8], Op2fStatus> {
    match (data.is_null(), len) {
        (true, 0) => Ok(&[]),
        (true, _) => Err(null("data")),
        (false, _) => Ok(unsafe { std::slice::from_raw_parts(data, len) }),
    }
}

pub(crate) fn index<'a, T>(items: &'a [T], i: usize, what: &str) -> Result<&'a T, Op2fStatus> {
    items.get(i).ok_or_else(|| fail(Op2fStatus::OutOfRange, format!("{what} {i} out of range (count {})", items.len())))
}

/// # Safety
/// `p` is NULL or a pointer from `Box::into_raw`, not used again.
pub(crate) unsafe fn free<T>(p: *mut T) {
    if !p.is_null() {
        // Dropping parsed data can't panic, so no guard.
        drop(unsafe { Box::from_raw(p) });
    }
}

// ---- Library ----

/// The library's version, a static NUL-terminated string such as "0.1.0".
#[unsafe(no_mangle)]
pub extern "C" fn op2f_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// Copies the message of this thread's most recent failure (empty if none) into `buf`; returns its full length.
///
/// # Safety
/// `buf` is NULL or `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_last_error(buf: *mut c_char, cap: usize) -> usize {
    let msg = LAST_ERROR.with(|e| e.borrow().clone());
    // A failure here would overwrite the message being read; the length is what's returned anyway.
    let _ = unsafe { copy_str(&msg, buf, cap, std::ptr::null_mut()) };
    LAST_ERROR.with(|e| *e.borrow_mut() = msg.clone());
    msg.len()
}

/// Frees a byte block. NULL does nothing.
///
/// # Safety
/// `bytes` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_bytes_free(bytes: *mut Op2fBytes) {
    unsafe { free(bytes) }
}

/// The bytes of a block (valid until it is freed).
///
/// # Safety
/// `bytes` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_bytes_data(bytes: *const Op2fBytes) -> *const u8 {
    unsafe { bytes.as_ref() }.map_or(std::ptr::null(), |b| b.0.as_ptr())
}

/// # Safety
/// `bytes` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_bytes_len(bytes: *const Op2fBytes) -> usize {
    unsafe { bytes.as_ref() }.map_or(0, |b| b.0.len())
}

/// # Safety
/// `image` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_free(image: *mut Op2fImage) {
    unsafe { free(image) }
}

/// Width in pixels.
///
/// # Safety
/// `image` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_width(image: *const Op2fImage) -> u32 {
    unsafe { image.as_ref() }.map_or(0, |i| i.0.image.width)
}

/// Height in pixels.
///
/// # Safety
/// `image` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_height(image: *const Op2fImage) -> u32 {
    unsafe { image.as_ref() }.map_or(0, |i| i.0.image.height)
}

/// The pixel that sits on the object's map position: draw the image with its top-left corner at the object's pixel
/// position minus the hotspot.
///
/// # Safety
/// `image` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_hotspot_x(image: *const Op2fImage) -> i32 {
    unsafe { image.as_ref() }.map_or(0, |i| i.0.hotspot.0)
}

/// # Safety
/// `image` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_hotspot_y(image: *const Op2fImage) -> i32 {
    unsafe { image.as_ref() }.map_or(0, |i| i.0.hotspot.1)
}

/// The pixels, RGBA with straight (not premultiplied) alpha, row by row: width * height * 4 bytes, valid until the
/// image is freed. NULL for an empty image.
///
/// # Safety
/// `image` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_rgba(image: *const Op2fImage) -> *const u8 {
    unsafe { image.as_ref() }.filter(|i| !i.0.image.rgba.is_empty()).map_or(std::ptr::null(), |i| i.0.image.rgba.as_ptr())
}

pub(crate) fn index_of<'a>(names: &'a [String], i: usize, what: &str) -> Result<&'a str, Op2fStatus> {
    index(names, i, what).map(String::as_str)
}

