//! Shared result types: images, string lists and tables, progress callbacks.

use crate::*;
use std::ffi::{c_char, c_void};

/// A path argument.
///
/// # Safety
/// `p` is NULL or a NUL-terminated string.
pub(crate) unsafe fn path_arg(p: *const c_char, what: &str) -> Result<std::path::PathBuf, Op2fStatus> {
    unsafe { str_arg(p, what) }.map(std::path::PathBuf::from)
}

/// An [`Op2fImage`] of a plain picture, hotspot (0, 0).
pub(crate) fn plain_image(image: op2_formats::render::Image) -> Op2fImage {
    Op2fImage(Sprite { image, hotspot: (0, 0) })
}

/// Fails with [`Op2fStatus::Unsupported`] for a function whose Cargo feature this build left out.
#[allow(dead_code)]
pub(crate) fn without_feature(feature: &str) -> Op2fStatus {
    fail(Op2fStatus::Unsupported, format!("op2formats was built without the {feature} feature"))
}

/// Called while long work goes on, with the caller's `user` pointer, the `stage` the function documents, and how far
/// it is: `done` of `total`. May be NULL.
pub type Op2fProgress = Option<unsafe extern "C" fn(user: *mut c_void, stage: u32, done: u64, total: u64)>;

/// Calls the caller's progress function, if any.
#[allow(dead_code)]
pub(crate) fn report(progress: Op2fProgress, user: *mut c_void, stage: u32, done: u64, total: u64) {
    if let Some(f) = progress {
        unsafe { f(user, stage, done, total) }
    }
}

// ---- Images ----

/// Makes an image from RGBA pixels (`width` * `height` * 4 bytes, copied) and a hotspot, e.g. to pass to
/// `op2f_image_overlay` or `op2f_image_shrink`.
///
/// # Safety
/// `rgba` is `width * height * 4` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_new(
    width: u32,
    height: u32,
    rgba: *const u8,
    hotspot_x: i32,
    hotspot_y: i32,
    out: *mut *mut Op2fImage,
) -> Op2fStatus {
    guard(|| {
        let len = width as usize * height as usize * 4;
        let rgba = unsafe { bytes_arg(rgba, len) }?.to_vec();
        let image = op2_formats::render::Image { width, height, rgba };
        unsafe { put(out, Op2fImage(Sprite { image, hotspot: (hotspot_x, hotspot_y) })) }
    })
}

/// Box-filters an image down by `factor` (1 or more: each output pixel averages a factor x factor block); the
/// hotspot is divided too.
///
/// # Safety
/// `image` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_shrink(image: *const Op2fImage, factor: u32, out: *mut *mut Op2fImage) -> Op2fStatus {
    guard(|| {
        let image = &unsafe { obj(image, "image") }?.0;
        if factor == 0 {
            return Err(fail(Op2fStatus::OutOfRange, "shrink factor 0"));
        }
        let small = op2_formats::tile_palette::shrink(&image.image, factor);
        let f = factor as i32;
        unsafe { put(out, Op2fImage(Sprite { image: small, hotspot: (image.hotspot.0 / f, image.hotspot.1 / f) })) }
    })
}

/// The mip chain of an image for a GPU texture: the image itself, then halves down to 1x1. Writes the new images
/// (each freed with `op2f_image_free`) to `out`, which holds `cap`, and their number to `*count`. A chain longer than
/// `cap` writes nothing and gives `OP2F_STATUS_BUFFER_TOO_SMALL`; a NULL `out` with `cap` 0 asks for the length.
///
/// # Safety
/// `image` is a live handle; `out` is NULL or `cap` writable pointers; `count` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_mip_chain(
    image: *const Op2fImage,
    out: *mut *mut Op2fImage,
    cap: usize,
    count: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let image = &unsafe { obj(image, "image") }?.0;
        let chain = op2_formats::tile_palette::mip_chain(image.image.clone());
        unsafe { put_value(count, chain.len()) }?;
        if out.is_null() && cap == 0 {
            return Ok(());
        }
        if cap < chain.len() {
            return Err(fail(Op2fStatus::BufferTooSmall, format!("the chain has {} images, room for {cap}", chain.len())));
        }
        if out.is_null() {
            return Err(null("out"));
        }
        for (i, level) in chain.into_iter().enumerate() {
            unsafe { out.add(i).write(Box::into_raw(Box::new(plain_image(level)))) };
        }
        Ok(())
    })
}

/// Draws `top` over `bottom` with `top`'s hotspot placed at `bottom`'s hotspot plus (`x`, `y`): how a turret sits
/// on its chassis (`op2f_turret_offset`). The canvas grows to fit both; the result's hotspot is `bottom`'s point.
///
/// # Safety
/// `bottom` and `top` are live handles; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_image_overlay(
    bottom: *const Op2fImage,
    top: *const Op2fImage,
    x: i32,
    y: i32,
    out: *mut *mut Op2fImage,
) -> Op2fStatus {
    guard(|| {
        let bottom = &unsafe { obj(bottom, "bottom") }?.0;
        let top = &unsafe { obj(top, "top") }?.0;
        unsafe { put(out, Op2fImage(units::overlay(bottom, top, (x, y)))) }
    })
}

// ---- String lists ----

/// A list of strings the library made.
pub struct Op2fStringList(pub(crate) Vec<String>);

/// Strings with numeric ids (a string table), sorted by id.
pub struct Op2fStringTable(pub(crate) Vec<(u32, String)>);

/// Key and value pairs, in their source's order.
pub struct Op2fStringPairs(pub(crate) Vec<(String, String)>);

/// # Safety
/// `list` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_string_list_free(list: *mut Op2fStringList) {
    unsafe { free(list) }
}

/// # Safety
/// `list` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_string_list_count(list: *const Op2fStringList) -> usize {
    unsafe { list.as_ref() }.map_or(0, |l| l.0.len())
}

/// # Safety
/// `list` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_string_list_get(
    list: *const Op2fStringList,
    index: usize,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let list = &unsafe { obj(list, "list") }?.0;
        unsafe { copy_str(index_of(list, index, "string")?, buf, cap, len) }
    })
}

/// # Safety
/// `table` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_string_table_free(table: *mut Op2fStringTable) {
    unsafe { free(table) }
}

/// # Safety
/// `table` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_string_table_count(table: *const Op2fStringTable) -> usize {
    unsafe { table.as_ref() }.map_or(0, |t| t.0.len())
}

/// The id and text of string `index` (0 to count - 1, in id order); `id` may be NULL.
///
/// # Safety
/// `table` is a live handle; `id` NULL or writable; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_string_table_entry(
    table: *const Op2fStringTable,
    index: usize,
    id: *mut u32,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let (i, text) = self::index(&unsafe { obj(table, "table") }?.0, index, "string")?;
        if !id.is_null() {
            unsafe { id.write(*i) };
        }
        unsafe { copy_str(text, buf, cap, len) }
    })
}

/// The text of the string with id `id`; `OP2F_STATUS_NOT_FOUND` when there's none.
///
/// # Safety
/// `table` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_string_table_find(
    table: *const Op2fStringTable,
    id: u32,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let table = &unsafe { obj(table, "table") }?.0;
        let at = table.binary_search_by_key(&id, |(i, _)| *i).map_err(|_| fail(Op2fStatus::NotFound, format!("no string {id}")))?;
        unsafe { copy_str(&table[at].1, buf, cap, len) }
    })
}

/// # Safety
/// `pairs` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_string_pairs_free(pairs: *mut Op2fStringPairs) {
    unsafe { free(pairs) }
}

/// # Safety
/// `pairs` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_string_pairs_count(pairs: *const Op2fStringPairs) -> usize {
    unsafe { pairs.as_ref() }.map_or(0, |p| p.0.len())
}

/// The key of pair `index`.
///
/// # Safety
/// `pairs` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_string_pairs_key(
    pairs: *const Op2fStringPairs,
    index: usize,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let (key, _) = self::index(&unsafe { obj(pairs, "pairs") }?.0, index, "pair")?;
        unsafe { copy_str(key, buf, cap, len) }
    })
}

/// The value of pair `index`.
///
/// # Safety
/// `pairs` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_string_pairs_value(
    pairs: *const Op2fStringPairs,
    index: usize,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let (_, value) = self::index(&unsafe { obj(pairs, "pairs") }?.0, index, "pair")?;
        unsafe { copy_str(value, buf, cap, len) }
    })
}
