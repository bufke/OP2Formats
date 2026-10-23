//! Sprites, unit art and click picking.

use crate::*;
use std::ffi::c_char;

/// The game's sprites: `op2_art.prt` (palettes, images, animations) with the pixels in `OP2_ART.BMP` and the
/// player colours in `color.bmp`.
pub struct Op2fSprites(pub(crate) Sprites);

/// Number of player colours (`color.bmp` ramps): blue, red, green, yellow, cyan, magenta.
pub const OP2F_PLAYER_COLORS: i32 = 6;
const _: () = assert!(OP2F_PLAYER_COLORS as usize == op2_formats::art::PLAYER_COLORS);

/// One image in `op2_art.prt`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fImageInfo {
    pub width: u32,
    pub height: u32,
    /// The image's type bits as stored. Bit 0: an in-game graphic (else a menu one); bit 2: a 1-bit shadow.
    pub kind: u16,
    /// Index of its palette.
    pub palette: u16,
    /// A 1-bit shadow, drawn as translucent black.
    pub shadow: bool,
}

/// One animation in `op2_art.prt`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fAnimationInfo {
    pub frame_count: u32,
    /// The frame pixel that sits on the object's position (what `op2f_image_hotspot_x`/`_y` give for its frames).
    pub displacement_x: i32,
    pub displacement_y: i32,
    /// Selection box, in the same frame coordinates: left, top, right, bottom.
    pub selection: [i32; 4],
    /// Fields of unknown meaning, as stored.
    pub unknown: u32,
    pub unknown2: u32,
    /// Clip rectangles some of its images are drawn inside (`op2f_sprites_animation_appendix`).
    pub appendix_count: usize,
}

// ---- Sprites ----

/// Loads the game's sprites (`op2_art.prt`, `OP2_ART.BMP` and, when present, `color.bmp`).
///
/// # Safety
/// `game` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_open(game: *const Op2fGame, out: *mut *mut Op2fSprites) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let sprites = Sprites::load(&game.files).map_err(lib_err)?;
        unsafe { put(out, Op2fSprites(sprites)) }
    })
}

/// # Safety
/// `sprites` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_free(sprites: *mut Op2fSprites) {
    unsafe { free(sprites) }
}

/// Number of images.
///
/// # Safety
/// `sprites` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_image_count(sprites: *const Op2fSprites) -> usize {
    unsafe { sprites.as_ref() }.map_or(0, |s| s.0.art.images.len())
}

/// Number of animations.
///
/// # Safety
/// `sprites` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_animation_count(sprites: *const Op2fSprites) -> usize {
    unsafe { sprites.as_ref() }.map_or(0, |s| s.0.art.animations.len())
}

/// # Safety
/// `sprites` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_image_info(sprites: *const Op2fSprites, index: usize, out: *mut Op2fImageInfo) -> Op2fStatus {
    guard(|| {
        let sprites = &unsafe { obj(sprites, "sprites") }?.0;
        let meta = self::index(&sprites.art.images, index, "image")?;
        let info = Op2fImageInfo {
            width: meta.width,
            height: meta.height,
            kind: meta.kind,
            palette: meta.palette,
            shadow: meta.is_shadow(),
        };
        unsafe { put_value(out, info) }
    })
}

/// # Safety
/// `sprites` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_animation_info(
    sprites: *const Op2fSprites,
    animation: usize,
    out: *mut Op2fAnimationInfo,
) -> Op2fStatus {
    guard(|| {
        let sprites = &unsafe { obj(sprites, "sprites") }?.0;
        let a = index(&sprites.art.animations, animation, "animation")?;
        let r = a.selection;
        let info = Op2fAnimationInfo {
            frame_count: a.frames.len() as u32,
            displacement_x: a.displacement.0,
            displacement_y: a.displacement.1,
            selection: [r.left, r.top, r.right, r.bottom],
            unknown: a.unknown,
            unknown2: a.unknown2,
            appendix_count: a.appendix.len(),
        };
        unsafe { put_value(out, info) }
    })
}

/// The colours to draw with: `player` < 0 for the palette as stored (player-coloured parts in player 0's blue),
/// else that player's colour (0 to `OP2F_PLAYER_COLORS` - 1).
pub(crate) fn tint(player: i32) -> Result<Tint, Op2fStatus> {
    match player {
        ..0 => Ok(Tint::Stored),
        0..OP2F_PLAYER_COLORS => Ok(Tint::Player(player as u8)),
        _ => Err(fail(Op2fStatus::OutOfRange, format!("player colour {player} out of range (0 to {})", OP2F_PLAYER_COLORS - 1))),
    }
}

/// Draws image `index` on its own as RGBA (index 0 transparent, shadows translucent black); its hotspot is (0, 0).
/// `player` as in `op2f_sprites_frame`.
///
/// # Safety
/// `sprites` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_image(
    sprites: *const Op2fSprites,
    index: usize,
    player: i32,
    out: *mut *mut Op2fImage,
) -> Op2fStatus {
    guard(|| {
        let sprites = &unsafe { obj(sprites, "sprites") }?.0;
        self::index(&sprites.art.images, index, "image")?;
        let image = sprites.image_rgba(index, tint(player)?).map_err(lib_err)?;
        unsafe { put(out, Op2fImage(Sprite { image, hotspot: (0, 0) })) }
    })
}

/// Composites frame `frame` of animation `animation` (its layers drawn in order) as RGBA. `player` < 0 draws the
/// palette as stored; 0 to `OP2F_PLAYER_COLORS` - 1 draws that player's colour. With `aligned`, the canvas covers
/// every frame of the animation, so all its frames have one size and hotspot; otherwise it is cropped to this frame.
///
/// # Safety
/// `sprites` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_frame(
    sprites: *const Op2fSprites,
    animation: usize,
    frame: usize,
    player: i32,
    aligned: bool,
    out: *mut *mut Op2fImage,
) -> Op2fStatus {
    guard(|| {
        let sprites = &unsafe { obj(sprites, "sprites") }?.0;
        let frames = &index(&sprites.art.animations, animation, "animation")?.frames;
        index(frames, frame, "frame")?;
        let tint = tint(player)?;
        let sprite = if aligned {
            sprites.frame_rgba_aligned(animation, frame, tint)
        } else {
            sprites.frame_rgba(animation, frame, tint)
        };
        unsafe { put(out, Op2fImage(sprite.map_err(lib_err)?)) }
    })
}

/// The game's click test on a frame: whether a click at (`x`, `y`) lands on it, in frame coordinates (map pixel -
/// object pixel + the animation's displacement), with the original's quirks (Outpost2.exe 0x404F40).
///
/// # Safety
/// `sprites` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_hit(
    sprites: *const Op2fSprites,
    animation: usize,
    frame: usize,
    x: i32,
    y: i32,
    out: *mut bool,
) -> Op2fStatus {
    guard(|| {
        let sprites = &unsafe { obj(sprites, "sprites") }?.0;
        let frames = &index(&sprites.art.animations, animation, "animation")?.frames;
        index(frames, frame, "frame")?;
        unsafe { put_value(out, sprites.hit(animation, frame, x, y) == Some(true)) }
    })
}

/// Which animation the game draws for a vehicle: `code` is its `vehicles.txt` code name (e.g. "LIGHT_TANK") or common
/// name ("Lynx", "ConVec"), `state` 0 for normal (1-3 are working states, falling back to 0 like the game), and
/// `rotation` its facing, 0-255 with 0 east, turning clockwise. Read from Outpost2.exe's unit types; see
/// op2-formats' units.rs.
///
/// # Safety
/// `code` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_vehicle_animation(
    code: *const c_char,
    plymouth: bool,
    state: u32,
    rotation: u8,
    out: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let code = unsafe { str_arg(code, "code") }?;
        let unit = units::vehicle(code).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no vehicle {code:?}")))?;
        let anim = unit
            .chassis(plymouth, state as usize, units::direction_index(rotation))
            .ok_or_else(|| fail(Op2fStatus::NotFound, format!("{code} has no animation for that state and direction")))?;
        unsafe { put_value(out, anim) }
    })
}

/// Which animation the game draws for a structure: `code` is its `building.txt` code name (e.g. "COMMAND"), `slot`
/// 0 the structure as built (its construction sequence, ending in the finished building), 1 and 2 damaged, 3
/// destroyed, 5-8 ambient animations. A slot the type doesn't have gives `OP2F_STATUS_NOT_FOUND`.
///
/// # Safety
/// `code` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_building_animation(code: *const c_char, plymouth: bool, slot: u32, out: *mut usize) -> Op2fStatus {
    guard(|| {
        let code = unsafe { str_arg(code, "code") }?;
        let art = units::building(code).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no structure {code:?}")))?;
        let anim = art.slots(plymouth).get(slot as usize).copied().filter(|&a| a != units::NONE);
        let anim = anim.ok_or_else(|| fail(Op2fStatus::NotFound, format!("{code} has no animation in slot {slot}")))?;
        unsafe { put_value(out, usize::from(anim)) }
    })
}

/// Draws a tank (`chassis` "LIGHT_TANK"/"Lynx", "MED_TANK"/"Panther" or "BIG_TANK"/"Tiger") with `weapon`'s turret (a `weapons.txt` code name, e.g. "LASER"), both facing `rotation`, as one
/// image with its hotspot on the unit's position. `player` as in `op2f_sprites_frame`.
///
/// # Safety
/// `sprites` is a live handle; `chassis` and `weapon` strings; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_tank(
    sprites: *const Op2fSprites,
    chassis: *const c_char,
    weapon: *const c_char,
    plymouth: bool,
    rotation: u8,
    player: i32,
    out: *mut *mut Op2fImage,
) -> Op2fStatus {
    guard(|| {
        let sprites = &unsafe { obj(sprites, "sprites") }?.0;
        let chassis = unsafe { str_arg(chassis, "chassis") }?;
        let weapon = unsafe { str_arg(weapon, "weapon") }?;
        let unit = units::vehicle(chassis).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no vehicle {chassis:?}")))?;
        let turret = units::turret(weapon).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no weapon {weapon:?}")))?;
        let sprite = units::tank_sprite(sprites, unit, turret, plymouth, rotation, tint(player)?).map_err(lib_err)?;
        unsafe { put(out, Op2fImage(sprite)) }
    })
}

