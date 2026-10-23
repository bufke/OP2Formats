//! The rest of `op2_art.prt`: palettes, raw image pixels, the layers of each frame, and bounds.

use crate::*;

/// One frame of an animation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fFrameInfo {
    pub layer_count: usize,
    /// A field of unknown meaning, as stored (its top bit says `optional2` follows).
    pub unknown: u8,
    /// Two optional offsets: on a chassis the turret's position, on a turret its muzzle (`op2f_turret_offset`).
    pub has_optional1: bool,
    pub optional1: [i8; 2],
    pub has_optional2: bool,
    pub optional2: [i8; 2],
}

/// One layer of a frame: an image drawn at an offset in frame coordinates.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fLayer {
    pub image: u16,
    pub unknown: u8,
    pub index: u8,
    pub x: i16,
    pub y: i16,
}

/// Number of palettes.
///
/// # Safety
/// `sprites` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_palette_count(sprites: *const Op2fSprites) -> usize {
    unsafe { sprites.as_ref() }.map_or(0, |s| s.0.art.palettes.len())
}

/// Palette `index` as stored: 256 RGB triples, 768 bytes into `out`.
///
/// # Safety
/// `sprites` is a live handle; `out` is 768 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_palette(sprites: *const Op2fSprites, index: usize, out: *mut u8) -> Op2fStatus {
    guard(|| {
        let palette = self::index(&unsafe { obj(sprites, "sprites") }?.0.art.palettes, index, "palette")?;
        unsafe { put_value(out.cast::<[[u8; 3]; 256]>(), *palette) }
    })
}

/// The palette image `image` draws with, in `player`'s colours (as in `op2f_sprites_frame`): 768 bytes of RGB into
/// `out`. Index 0 is transparent.
///
/// # Safety
/// `sprites` is a live handle; `out` is 768 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_image_palette(sprites: *const Op2fSprites, image: usize, player: i32, out: *mut u8) -> Op2fStatus {
    guard(|| {
        let s = &unsafe { obj(sprites, "sprites") }?.0;
        let meta = index(&s.art.images, image, "image")?;
        index(&s.art.palettes, meta.palette as usize, "palette")?;
        unsafe { put_value(out.cast::<[[u8; 3]; 256]>(), s.palette(meta, tint(player)?)) }
    })
}

/// Image `image`'s raw pixels: one palette index per pixel, row by row (shadows: 0 or 1), width * height bytes into
/// `out`, which holds `cap`.
///
/// # Safety
/// `sprites` is a live handle; `out` is `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_image_indices(sprites: *const Op2fSprites, image: usize, out: *mut u8, cap: usize) -> Op2fStatus {
    guard(|| {
        let s = &unsafe { obj(sprites, "sprites") }?.0;
        index(&s.art.images, image, "image")?;
        let pixels = s.image_indices(image).map_err(lib_err)?;
        if cap < pixels.len() {
            return Err(fail(Op2fStatus::BufferTooSmall, format!("{} pixels, room for {cap}", pixels.len())));
        }
        if out.is_null() {
            return Err(null("out"));
        }
        unsafe { std::ptr::copy_nonoverlapping(pixels.as_ptr(), out, pixels.len()) };
        Ok(())
    })
}

fn frame(s: &Sprites, animation: usize, frame: usize) -> Result<&op2_formats::art::Frame, Op2fStatus> {
    let a = index(&s.art.animations, animation, "animation")?;
    index(&a.frames, frame, "frame")
}

/// # Safety
/// `sprites` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_frame_info(sprites: *const Op2fSprites, animation: usize, frame: usize, out: *mut Op2fFrameInfo) -> Op2fStatus {
    guard(|| {
        let f = self::frame(&unsafe { obj(sprites, "sprites") }?.0, animation, frame)?;
        let info = Op2fFrameInfo {
            layer_count: f.layers.len(),
            unknown: f.unknown,
            has_optional1: f.optional1.is_some(),
            optional1: f.optional1.unwrap_or_default(),
            has_optional2: f.optional2.is_some(),
            optional2: f.optional2.unwrap_or_default(),
        };
        unsafe { put_value(out, info) }
    })
}

/// Layer `layer` of a frame (layers draw in order, later on top).
///
/// # Safety
/// `sprites` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_frame_layer(
    sprites: *const Op2fSprites,
    animation: usize,
    frame: usize,
    layer: usize,
    out: *mut Op2fLayer,
) -> Op2fStatus {
    guard(|| {
        let f = self::frame(&unsafe { obj(sprites, "sprites") }?.0, animation, frame)?;
        let l = index(&f.layers, layer, "layer")?;
        unsafe { put_value(out, Op2fLayer { image: l.image, unknown: l.unknown, index: l.index, x: l.x, y: l.y }) }
    })
}

/// The bounds of a frame's layers in frame coordinates: min x, min y, max x, max y (max exclusive) into `out`.
///
/// # Safety
/// `sprites` is a live handle; `out` is 4 writable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_frame_bounds(sprites: *const Op2fSprites, animation: usize, frame: usize, out: *mut i32) -> Op2fStatus {
    guard(|| {
        let s = &unsafe { obj(sprites, "sprites") }?.0;
        let (x0, y0, x1, y1) = s.frame_bounds(self::frame(s, animation, frame)?);
        unsafe { put_value(out.cast::<[i32; 4]>(), [x0, y0, x1, y1]) }
    })
}

/// The union of every frame's bounds (as `op2f_sprites_frame_bounds`) of an animation.
///
/// # Safety
/// `sprites` is a live handle; `out` is 4 writable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_animation_bounds(sprites: *const Op2fSprites, animation: usize, out: *mut i32) -> Op2fStatus {
    guard(|| {
        let s = &unsafe { obj(sprites, "sprites") }?.0;
        let (x0, y0, x1, y1) = s.animation_bounds(index(&s.art.animations, animation, "animation")?);
        unsafe { put_value(out.cast::<[i32; 4]>(), [x0, y0, x1, y1]) }
    })
}

/// Clip rectangle `rect` of an animation (left, top, right, bottom, edges included, in frame coordinates): an image
/// whose type has bits 5-9 set to `k` is drawn only inside rectangle `k` - 1 (a truck sinking into a dock pit).
///
/// # Safety
/// `sprites` is a live handle; `out` is 4 writable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_animation_appendix(sprites: *const Op2fSprites, animation: usize, rect: usize, out: *mut u32) -> Op2fStatus {
    guard(|| {
        let a = index(&unsafe { obj(sprites, "sprites") }?.0.art.animations, animation, "animation")?;
        unsafe { put_value(out.cast::<[u32; 4]>(), *index(&a.appendix, rect, "appendix rectangle")?) }
    })
}
