//! Windows PE resources: the game's DLLs (`out2res.dll`, `op2shres.dll`) keep its UI art, cursors, dialogs, menus
//! and strings as ordinary resources. Also `.bmp`, `.cur`, `.ico` and `.ani` files.

use crate::*;
use op2_formats::pe_res::{self, AniCursor, CursorImage, Dialog, Dib, PeResources, ResId};
use std::ffi::c_char;

/// The resources of a PE file (an `.exe` or `.dll`).
pub struct Op2fResources(pub(crate) PeResources);

/// A decoded bitmap, with its palette and indices when it has one.
pub struct Op2fBitmap(pub(crate) Dib);

/// A dialog template.
pub struct Op2fDialog(pub(crate) Dialog);

/// A flattened menu template.
pub struct Op2fMenu(pub(crate) Vec<(usize, u16, String)>);

/// An animated cursor (`.ani`).
pub struct Op2fAniCursor(pub(crate) AniCursor);

/// One resource's ids and size.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fResourceInfo {
    /// The numeric type (2 bitmap, 6 string table, ...), or 0 for a named type (`op2f_resource_type`).
    pub type_id: u16,
    /// The numeric name, or 0 for a named resource (`op2f_resource_name`).
    pub name_id: u16,
    pub lang: u16,
    pub codepage: u32,
    pub size: usize,
}

fn id_text(id: &ResId) -> String {
    match id {
        ResId::Id(n) => n.to_string(),
        ResId::Name(s) => s.clone(),
    }
}

/// Opens a PE file in the game folder by name (e.g. "out2res.dll"), or by path.
///
/// # Safety
/// `game` is a live handle; `name` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resources_open(game: *const Op2fGame, name: *const c_char, out: *mut *mut Op2fResources) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let (data, _) = game.files.read(unsafe { str_arg(name, "name") }?).map_err(lib_err)?;
        unsafe { put(out, Op2fResources(PeResources::parse(data).map_err(lib_err)?)) }
    })
}

/// Reads the resources of a PE file in memory (the bytes are copied).
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resources_parse(data: *const u8, len: usize, out: *mut *mut Op2fResources) -> Op2fStatus {
    guard(|| {
        let res = PeResources::parse(unsafe { bytes_arg(data, len) }?.to_vec()).map_err(lib_err)?;
        unsafe { put(out, Op2fResources(res)) }
    })
}

/// # Safety
/// `res` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resources_free(res: *mut Op2fResources) {
    unsafe { free(res) }
}

/// Number of resources.
///
/// # Safety
/// `res` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resources_count(res: *const Op2fResources) -> usize {
    unsafe { res.as_ref() }.map_or(0, |r| r.0.resources.len())
}

/// # Safety
/// `res` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resource_info(res: *const Op2fResources, index: usize, out: *mut Op2fResourceInfo) -> Op2fStatus {
    guard(|| {
        let r = self::index(&unsafe { obj(res, "res") }?.0.resources, index, "resource")?;
        let id = |i: &ResId| if let ResId::Id(n) = i { *n } else { 0 };
        let info = Op2fResourceInfo { type_id: id(&r.kind), name_id: id(&r.name), lang: r.lang, codepage: r.codepage, size: r.len };
        unsafe { put_value(out, info) }
    })
}

/// Resource `index`'s type as text: the standard name for a numeric one ("BITMAP"), the number when it has none,
/// or the type's own name.
///
/// # Safety
/// `res` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resource_type(res: *const Op2fResources, index: usize, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let r = self::index(&unsafe { obj(res, "res") }?.0.resources, index, "resource")?;
        let text = match &r.kind {
            ResId::Id(n) => pe_res::type_name(*n).map_or_else(|| n.to_string(), str::to_string),
            ResId::Name(s) => s.clone(),
        };
        unsafe { copy_str(&text, buf, cap, len) }
    })
}

/// Resource `index`'s name as text: its number, or its name.
///
/// # Safety
/// `res` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resource_name(res: *const Op2fResources, index: usize, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let r = self::index(&unsafe { obj(res, "res") }?.0.resources, index, "resource")?;
        unsafe { copy_str(&id_text(&r.name), buf, cap, len) }
    })
}

/// Resource `index`'s raw bytes.
///
/// # Safety
/// `res` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resource_bytes(res: *const Op2fResources, index: usize, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| {
        let res = &unsafe { obj(res, "res") }?.0;
        let r = self::index(&res.resources, index, "resource")?;
        unsafe { put(out, Op2fBytes(res.bytes(r).to_vec())) }
    })
}

/// Finds a resource. `type` is a number, a standard type name ("BITMAP" or "RT_BITMAP") or a type's own name;
/// `name` is a number or a name (names match ignoring case).
///
/// # Safety
/// `res` is a live handle; `type` and `name` strings; `index` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resources_find(
    res: *const Op2fResources,
    r#type: *const c_char,
    name: *const c_char,
    index: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let res = &unsafe { obj(res, "res") }?.0;
        let kind = ResId::parse_type(unsafe { str_arg(r#type, "type") }?);
        let name = ResId::parse(unsafe { str_arg(name, "name") }?);
        let found = res.find(&kind, &name).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no resource {kind} {name}")))?;
        let i = res.resources.iter().position(|r| std::ptr::eq(r, found)).expect("found in the list");
        unsafe { put_value(index, i) }
    })
}

/// Every string in the string tables, by id.
///
/// # Safety
/// `res` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resources_strings(res: *const Op2fResources, out: *mut *mut Op2fStringTable) -> Op2fStatus {
    guard(|| {
        let res = &unsafe { obj(res, "res") }?.0;
        unsafe { put(out, Op2fStringTable(res.strings().map_err(lib_err)?)) }
    })
}

/// A bitmap resource by name (a number or a name).
///
/// # Safety
/// `res` is a live handle; `name` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resources_bitmap(res: *const Op2fResources, name: *const c_char, out: *mut *mut Op2fBitmap) -> Op2fStatus {
    guard(|| {
        let res = &unsafe { obj(res, "res") }?.0;
        let dib = res.bitmap(&ResId::parse(unsafe { str_arg(name, "name") }?)).map_err(lib_err)?;
        unsafe { put(out, Op2fBitmap(dib)) }
    })
}

/// A cursor (`cursor` true: an RT_GROUP_CURSOR name) or icon (an RT_GROUP_ICON name), its largest image, with the
/// cursor's hotspot.
///
/// # Safety
/// `res` is a live handle; `name` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resources_cursor(res: *const Op2fResources, name: *const c_char, cursor: bool, out: *mut *mut Op2fImage) -> Op2fStatus {
    guard(|| {
        let res = &unsafe { obj(res, "res") }?.0;
        let group = if cursor { pe_res::RT_GROUP_CURSOR } else { pe_res::RT_GROUP_ICON };
        let c = res.cursor_or_icon(group, &ResId::parse(unsafe { str_arg(name, "name") }?)).map_err(lib_err)?;
        unsafe { put(out, cursor_image(c)) }
    })
}

fn cursor_image(c: CursorImage) -> Op2fImage {
    let image = op2_formats::render::Image { width: c.image.width, height: c.image.height, rgba: c.image.rgba };
    Op2fImage(Sprite { image, hotspot: (c.hotspot.0.into(), c.hotspot.1.into()) })
}

/// A dialog template by numeric id.
///
/// # Safety
/// `res` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resources_dialog(res: *const Op2fResources, id: u16, out: *mut *mut Op2fDialog) -> Op2fStatus {
    guard(|| {
        let res = &unsafe { obj(res, "res") }?.0;
        unsafe { put(out, Op2fDialog(res.dialog(id).map_err(lib_err)?)) }
    })
}

/// A menu by name (a number or a name).
///
/// # Safety
/// `res` is a live handle; `name` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resources_menu(res: *const Op2fResources, name: *const c_char, out: *mut *mut Op2fMenu) -> Op2fStatus {
    guard(|| {
        let res = &unsafe { obj(res, "res") }?.0;
        let name = ResId::parse(unsafe { str_arg(name, "name") }?);
        let r = res.find(&ResId::Id(pe_res::RT_MENU), &name).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no menu {name}")))?;
        unsafe { put(out, Op2fMenu(pe_res::parse_menu(res.bytes(r)).map_err(lib_err)?)) }
    })
}

/// The version resource's strings ("FileVersion", "ProductName", ...).
///
/// # Safety
/// `res` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_resources_version(res: *const Op2fResources, out: *mut *mut Op2fStringPairs) -> Op2fStatus {
    guard(|| {
        let res = &unsafe { obj(res, "res") }?.0;
        let r = res
            .of_type(&ResId::Id(pe_res::RT_VERSION))
            .next()
            .ok_or_else(|| fail(Op2fStatus::NotFound, "no version resource"))?;
        unsafe { put(out, Op2fStringPairs(pe_res::version_strings(res.bytes(r)))) }
    })
}

// ---- Bitmaps ----

/// A bitmap's format.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fBitmapInfo {
    pub width: u32,
    pub height: u32,
    pub bit_count: u16,
    /// BI_RGB 0, BI_RLE8 1, ...
    pub compression: u32,
    /// Palette entries (0 for true-colour bitmaps).
    pub palette_count: usize,
}

/// Decodes a DIB: a bitmap resource's bytes, or a `.bmp` file (its 14-byte file header is skipped).
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_bitmap_parse(data: *const u8, len: usize, out: *mut *mut Op2fBitmap) -> Op2fStatus {
    guard(|| {
        let data = unsafe { bytes_arg(data, len) }?;
        let data = if data.starts_with(b"BM") { data.get(14..).unwrap_or(&[]) } else { data };
        unsafe { put(out, Op2fBitmap(Dib::parse(data).map_err(lib_err)?)) }
    })
}

/// # Safety
/// `bitmap` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_bitmap_free(bitmap: *mut Op2fBitmap) {
    unsafe { free(bitmap) }
}

/// # Safety
/// `bitmap` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_bitmap_info(bitmap: *const Op2fBitmap, out: *mut Op2fBitmapInfo) -> Op2fStatus {
    guard(|| {
        let d = &unsafe { obj(bitmap, "bitmap") }?.0;
        let info = Op2fBitmapInfo { width: d.width, height: d.height, bit_count: d.bit_count, compression: d.compression, palette_count: d.palette.len() };
        unsafe { put_value(out, info) }
    })
}

/// The bitmap as RGBA, top row first.
///
/// # Safety
/// `bitmap` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_bitmap_image(bitmap: *const Op2fBitmap, out: *mut *mut Op2fImage) -> Op2fStatus {
    guard(|| {
        let d = &unsafe { obj(bitmap, "bitmap") }?.0;
        unsafe { put(out, plain_image(op2_formats::render::Image { width: d.width, height: d.height, rgba: d.rgba.clone() })) }
    })
}

/// The palette as RGB: `palette_count * 3` bytes into `out`, which holds `cap`.
///
/// # Safety
/// `bitmap` is a live handle; `out` is `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_bitmap_palette(bitmap: *const Op2fBitmap, out: *mut u8, cap: usize) -> Op2fStatus {
    guard(|| {
        let p = &unsafe { obj(bitmap, "bitmap") }?.0.palette;
        let bytes: Vec<u8> = p.iter().flatten().copied().collect();
        copy_bytes(&bytes, out, cap)
    })
}

/// The palette indices (bitmaps of 8 bits or fewer per pixel), one byte per pixel, top row first: width * height
/// bytes into `out`, which holds `cap`; `OP2F_STATUS_NOT_FOUND` for a true-colour bitmap.
///
/// # Safety
/// `bitmap` is a live handle; `out` is `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_bitmap_indices(bitmap: *const Op2fBitmap, out: *mut u8, cap: usize) -> Op2fStatus {
    guard(|| {
        let d = &unsafe { obj(bitmap, "bitmap") }?.0;
        if d.indices.is_empty() {
            return Err(fail(Op2fStatus::NotFound, "a true-colour bitmap has no palette indices"));
        }
        copy_bytes(&d.indices, out, cap)
    })
}

fn copy_bytes(bytes: &[u8], out: *mut u8, cap: usize) -> Result<(), Op2fStatus> {
    if cap < bytes.len() {
        return Err(fail(Op2fStatus::BufferTooSmall, format!("{} bytes, room for {cap}", bytes.len())));
    }
    if out.is_null() && !bytes.is_empty() {
        return Err(null("out"));
    }
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
    Ok(())
}

/// A `.cur` or `.ico` file: its first image, with the cursor's hotspot.
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_cursor_parse(data: *const u8, len: usize, out: *mut *mut Op2fImage) -> Op2fStatus {
    guard(|| {
        let c = pe_res::parse_cur_file(unsafe { bytes_arg(data, len) }?).map_err(lib_err)?;
        unsafe { put(out, cursor_image(c)) }
    })
}

// ---- Animated cursors ----

/// Parses an animated cursor (`.ani`).
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_ani_cursor_parse(data: *const u8, len: usize, out: *mut *mut Op2fAniCursor) -> Op2fStatus {
    guard(|| {
        let ani = AniCursor::parse(unsafe { bytes_arg(data, len) }?).map_err(lib_err)?;
        unsafe { put(out, Op2fAniCursor(ani)) }
    })
}

/// # Safety
/// `ani` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_ani_cursor_free(ani: *mut Op2fAniCursor) {
    unsafe { free(ani) }
}

/// Number of distinct frame images.
///
/// # Safety
/// `ani` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_ani_cursor_frame_count(ani: *const Op2fAniCursor) -> usize {
    unsafe { ani.as_ref() }.map_or(0, |a| a.0.frames.len())
}

/// Frame image `frame`, with its hotspot.
///
/// # Safety
/// `ani` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_ani_cursor_frame(ani: *const Op2fAniCursor, frame: usize, out: *mut *mut Op2fImage) -> Op2fStatus {
    guard(|| {
        let f = index(&unsafe { obj(ani, "ani") }?.0.frames, frame, "frame")?;
        unsafe { put(out, cursor_image(f.clone())) }
    })
}

/// Number of animation steps (each shows one frame for a time).
///
/// # Safety
/// `ani` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_ani_cursor_step_count(ani: *const Op2fAniCursor) -> usize {
    unsafe { ani.as_ref() }.map_or(0, |a| {
        let a = &a.0;
        if a.seq.is_empty() { a.frames.len() } else { a.seq.len() }
    })
}

/// Step `step`: which frame it shows and for how long, in jiffies (1/60 s).
///
/// # Safety
/// `ani` is a live handle; `frame` and `jiffies` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_ani_cursor_step(ani: *const Op2fAniCursor, step: usize, frame: *mut usize, jiffies: *mut u32) -> Op2fStatus {
    guard(|| {
        let a = &unsafe { obj(ani, "ani") }?.0;
        let steps = if a.seq.is_empty() { a.frames.len() } else { a.seq.len() };
        if step >= steps {
            return Err(fail(Op2fStatus::OutOfRange, format!("step {step} out of range (count {steps})")));
        }
        let f = a.seq.get(step).map_or(step, |&f| f as usize);
        unsafe { put_value(frame, f) }?;
        unsafe { put_value(jiffies, a.rates.get(step).copied().unwrap_or(a.default_rate)) }
    })
}

/// The cursor's title; `OP2F_STATUS_NOT_FOUND` without one.
///
/// # Safety
/// `ani` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_ani_cursor_title(ani: *const Op2fAniCursor, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let a = &unsafe { obj(ani, "ani") }?.0;
        let title = a.title.as_deref().ok_or_else(|| fail(Op2fStatus::NotFound, "no title"))?;
        unsafe { copy_str(title, buf, cap, len) }
    })
}

// ---- Dialogs ----

/// A dialog's frame, in dialog units.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fDialogInfo {
    pub style: u32,
    /// x, y, width, height.
    pub rect: [i16; 4],
    pub has_font: bool,
    pub font_size: u16,
    pub control_count: usize,
}

/// One control of a dialog.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fDialogControl {
    pub id: u32,
    pub style: u32,
    /// x, y, width, height in dialog units.
    pub rect: [i16; 4],
}

/// A text of a dialog.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fDialogText {
    Title,
    /// `OP2F_STATUS_NOT_FOUND` without a font.
    FontName,
    /// The menu's id or name; `OP2F_STATUS_NOT_FOUND` without one.
    Menu,
    /// The window class; `OP2F_STATUS_NOT_FOUND` for the default.
    Class,
}

/// # Safety
/// `dialog` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_dialog_free(dialog: *mut Op2fDialog) {
    unsafe { free(dialog) }
}

/// # Safety
/// `dialog` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_dialog_info(dialog: *const Op2fDialog, out: *mut Op2fDialogInfo) -> Op2fStatus {
    guard(|| {
        let d = &unsafe { obj(dialog, "dialog") }?.0;
        let info = Op2fDialogInfo {
            style: d.style,
            rect: d.rect,
            has_font: d.font.is_some(),
            font_size: d.font.as_ref().map_or(0, |f| f.0),
            control_count: d.controls.len(),
        };
        unsafe { put_value(out, info) }
    })
}

/// # Safety
/// `dialog` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_dialog_text(
    dialog: *const Op2fDialog,
    field: Op2fDialogText,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let d = &unsafe { obj(dialog, "dialog") }?.0;
        let none = |what: &str| fail(Op2fStatus::NotFound, format!("the dialog has no {what}"));
        let text = match field {
            Op2fDialogText::Title => d.title.clone(),
            Op2fDialogText::FontName => d.font.as_ref().ok_or_else(|| none("font"))?.1.clone(),
            Op2fDialogText::Menu => id_text(d.menu.as_ref().ok_or_else(|| none("menu"))?),
            Op2fDialogText::Class => id_text(d.class.as_ref().ok_or_else(|| none("class"))?),
        };
        unsafe { copy_str(&text, buf, cap, len) }
    })
}

/// Control `control`'s id, style and frame.
///
/// # Safety
/// `dialog` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_dialog_control(dialog: *const Op2fDialog, control: usize, out: *mut Op2fDialogControl) -> Op2fStatus {
    guard(|| {
        let c = index(&unsafe { obj(dialog, "dialog") }?.0.controls, control, "control")?;
        unsafe { put_value(out, Op2fDialogControl { id: c.id, style: c.style, rect: c.rect }) }
    })
}

/// Control `control`'s window class ("BUTTON", "EDIT", "STATIC", ... or a registered class name) and, when `text`
/// isn't NULL, its text (or "#id" for a resource id) into `text`, which holds `text_cap`.
///
/// # Safety
/// `dialog` is a live handle; the buffers as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_dialog_control_class(
    dialog: *const Op2fDialog,
    control: usize,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
    text: *mut c_char,
    text_cap: usize,
) -> Op2fStatus {
    guard(|| {
        let c = index(&unsafe { obj(dialog, "dialog") }?.0.controls, control, "control")?;
        if !text.is_null() {
            let t = match &c.text {
                ResId::Id(n) => format!("#{n}"),
                ResId::Name(s) => s.clone(),
            };
            unsafe { copy_str(&t, text, text_cap, std::ptr::null_mut()) }?;
        }
        unsafe { copy_str(&c.class_name(), buf, cap, len) }
    })
}

// ---- Menus ----

/// # Safety
/// `menu` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_menu_free(menu: *mut Op2fMenu) {
    unsafe { free(menu) }
}

/// Number of items, in order, submenus flattened after their popup item.
///
/// # Safety
/// `menu` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_menu_item_count(menu: *const Op2fMenu) -> usize {
    unsafe { menu.as_ref() }.map_or(0, |m| m.0.len())
}

/// Item `index`: its depth (0 top level) and command id (0 for a popup) and its text.
///
/// # Safety
/// `menu` is a live handle; `depth` and `id` NULL or writable; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_menu_item(
    menu: *const Op2fMenu,
    index: usize,
    depth: *mut usize,
    id: *mut u16,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let (d, i, text) = self::index(&unsafe { obj(menu, "menu") }?.0, index, "menu item")?;
        if !depth.is_null() {
            unsafe { depth.write(*d) };
        }
        if !id.is_null() {
            unsafe { id.write(*i) };
        }
        unsafe { copy_str(text, buf, cap, len) }
    })
}
