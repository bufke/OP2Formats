//! Which animations the game draws for each unit type, turrets, and what a click hits.

use crate::*;
use op2_formats::pick;
use std::ffi::c_char;

/// Game animation id of animation 0: the game's ids are art indices plus this.
pub const OP2F_ANIM_ID_BASE: u32 = 101;
const _: () = assert!(OP2F_ANIM_ID_BASE == units::ANIM_ID_BASE);

/// The direction column (0-15) the game draws for a rotation (0-255, 0 east, clockwise).
#[unsafe(no_mangle)]
pub extern "C" fn op2f_direction_column(rotation: u8) -> u32 {
    units::direction_index(rotation) as u32
}

/// The rotation at the centre of a direction column (0-15).
#[unsafe(no_mangle)]
pub extern "C" fn op2f_column_rotation(column: u32) -> u8 {
    units::column_rotation(column as usize)
}

/// A table of unit types with art.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fUnitArtTable {
    /// Vehicles (`op2f_vehicle_animation`).
    Vehicles,
    /// Structures (`op2f_building_animation`).
    Buildings,
    /// Weapons with turrets (`op2f_turret_animation`).
    Turrets,
}

fn codes(table: Op2fUnitArtTable) -> Vec<&'static str> {
    match table {
        Op2fUnitArtTable::Vehicles => units::vehicles().iter().map(|v| v.code).collect(),
        Op2fUnitArtTable::Buildings => units::buildings().iter().map(|b| b.code).collect(),
        Op2fUnitArtTable::Turrets => units::turrets().iter().map(|t| t.code).collect(),
    }
}

/// Number of unit types in a table.
#[unsafe(no_mangle)]
pub extern "C" fn op2f_unit_art_count(table: Op2fUnitArtTable) -> usize {
    codes(table).len()
}

/// The code name of entry `index` of a table, e.g. "LIGHT_TANK", as a static string; NULL when out of range.
#[unsafe(no_mangle)]
pub extern "C" fn op2f_unit_art_code(table: Op2fUnitArtTable, index: usize) -> *const c_char {
    // The table strings aren't NUL-terminated; keep one terminated copy of each for the process.
    static CODES: std::sync::OnceLock<[Vec<std::ffi::CString>; 3]> = std::sync::OnceLock::new();
    let all = CODES.get_or_init(|| {
        [Op2fUnitArtTable::Vehicles, Op2fUnitArtTable::Buildings, Op2fUnitArtTable::Turrets]
            .map(|t| codes(t).into_iter().map(|c| std::ffi::CString::new(c).expect("no NUL in a code")).collect())
    });
    all[table as usize].get(index).map_or(std::ptr::null(), |c| c.as_ptr())
}

/// The column of the ambient animation sequence a structure uses (0 = none; see `op2f_exe_ambient_sequence`).
///
/// # Safety
/// `code` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_building_ambient(code: *const c_char, plymouth: bool, out: *mut u8) -> Op2fStatus {
    guard(|| {
        let code = unsafe { str_arg(code, "code") }?;
        let art = units::building(code).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no structure {code:?}")))?;
        unsafe { put_value(out, art.ambient(plymouth)) }
    })
}

/// The turret animation for `weapon` (a `weapons.txt` code, e.g. "LASER") on `chassis` ("LIGHT_TANK"/"Lynx",
/// "MED_TANK"/"Panther", "BIG_TANK"/"Tiger" or "TOWER_GUARD" for a Guard Post), facing `rotation`.
///
/// # Safety
/// `weapon` and `chassis` are strings; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_turret_animation(
    weapon: *const c_char,
    chassis: *const c_char,
    plymouth: bool,
    rotation: u8,
    out: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let weapon = unsafe { str_arg(weapon, "weapon") }?;
        let chassis = unsafe { str_arg(chassis, "chassis") }?;
        let art = units::turret(weapon).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no weapon {weapon:?}")))?;
        let set = art
            .for_chassis(chassis)
            .ok_or_else(|| fail(Op2fStatus::NotFound, format!("{weapon} has no turret for {chassis}")))?;
        unsafe { put_value(out, set.column(plymouth, units::direction_index(rotation))) }
    })
}

/// Where a turret sits on a chassis animation: the turret's hotspot goes at the chassis's hotspot plus (`*x`, `*y`).
///
/// # Safety
/// `sprites` is a live handle; `x` and `y` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_turret_offset(sprites: *const Op2fSprites, chassis_animation: usize, x: *mut i32, y: *mut i32) -> Op2fStatus {
    guard(|| {
        let s = &unsafe { obj(sprites, "sprites") }?.0;
        index(&s.art.animations, chassis_animation, "animation")?;
        let (ox, oy) = units::turret_offset(&s.art, chassis_animation);
        unsafe { put_value(x, ox) }?;
        unsafe { put_value(y, oy) }
    })
}

/// Where shots leave the barrel, relative to the chassis's hotspot (inferred from the game's code, not yet checked
/// against shots in game).
///
/// # Safety
/// `sprites` is a live handle; `x` and `y` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_muzzle_offset(
    sprites: *const Op2fSprites,
    chassis_animation: usize,
    turret_animation: usize,
    x: *mut i32,
    y: *mut i32,
) -> Op2fStatus {
    guard(|| {
        let s = &unsafe { obj(sprites, "sprites") }?.0;
        index(&s.art.animations, chassis_animation, "animation")?;
        index(&s.art.animations, turret_animation, "animation")?;
        let (ox, oy) = units::muzzle_offset(&s.art, chassis_animation, turret_animation);
        unsafe { put_value(x, ox) }?;
        unsafe { put_value(y, oy) }
    })
}

/// Draws a vehicle's chassis facing `rotation`: frame `frame` (wrapping) of its normal animation, on the
/// animation's whole canvas. `player` as in `op2f_sprites_frame`.
///
/// # Safety
/// `sprites` is a live handle; `code` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sprites_vehicle(
    sprites: *const Op2fSprites,
    code: *const c_char,
    plymouth: bool,
    rotation: u8,
    frame: usize,
    player: i32,
    out: *mut *mut Op2fImage,
) -> Op2fStatus {
    guard(|| {
        let s = &unsafe { obj(sprites, "sprites") }?.0;
        let code = unsafe { str_arg(code, "code") }?;
        let unit = units::vehicle(code).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no vehicle {code:?}")))?;
        let sprite = units::chassis_sprite(s, unit, plymouth, rotation, frame, tint(player)?).map_err(lib_err)?;
        unsafe { put(out, Op2fImage(sprite)) }
    })
}

// ---- Picking ----

/// How a unit is clicked.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fPickKind {
    /// Anywhere in the animation's selection box (vehicles, Guard Posts).
    Box,
    /// The box, then the frame's pixels (or the overlay's): structures.
    Pixels,
}

/// A unit's click shape.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fPickShape {
    pub kind: Op2fPickKind,
    /// The animation (art index) drawn.
    pub animation: usize,
    /// Its frame, for `OP2F_PICK_KIND_PIXELS`.
    pub frame: usize,
    /// A structure's second hit animation (`op2f_pick_overlay`), tested at frame 0.
    pub has_overlay: bool,
    pub overlay_animation: usize,
    /// The overlay is placed by the main animation's selection box (mines) rather than its own.
    pub overlay_main_rect: bool,
}

/// A unit the click considers.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fPickCandidate {
    /// The unit's pixel position (its sprite's hotspot).
    pub x: i32,
    pub y: i32,
    pub owner: u8,
    pub shape: Op2fPickShape,
}

fn shape(s: &Op2fPickShape) -> pick::Shape {
    match s.kind {
        Op2fPickKind::Box => pick::Shape::Box { anim: s.animation },
        Op2fPickKind::Pixels => pick::Shape::Pixels {
            anim: s.animation,
            frame: s.frame,
            overlay: s.has_overlay.then_some(pick::Overlay { anim: s.overlay_animation, main_rect: s.overlay_main_rect }),
        },
    }
}

/// A structure's overlay for its animation state (0 built, 5 operating, ...; MapObject +0x36), into `shape`'s overlay
/// fields; `OP2F_STATUS_NOT_FOUND` when it has none (`has_overlay` is then cleared).
///
/// # Safety
/// `code` is a string; `shape` a writable, initialised shape.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_pick_overlay(code: *const c_char, state: u16, plymouth: bool, shape: *mut Op2fPickShape) -> Op2fStatus {
    guard(|| {
        let code = unsafe { str_arg(code, "code") }?;
        let shape = unsafe { shape.as_mut() }.ok_or_else(|| null("shape"))?;
        let overlay = pick::overlay(&code.to_ascii_uppercase(), state, plymouth);
        shape.has_overlay = overlay.is_some();
        let o = overlay.ok_or_else(|| fail(Op2fStatus::NotFound, format!("{code} has no overlay in state {state}")))?;
        shape.overlay_animation = o.anim;
        shape.overlay_main_rect = o.main_rect;
        Ok(())
    })
}

/// An animation's selection box relative to the unit's pixel: left, top, width, height into `out`.
///
/// # Safety
/// `sprites` is a live handle; `out` is 4 writable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_pick_selection_box(sprites: *const Op2fSprites, animation: usize, out: *mut i32) -> Op2fStatus {
    guard(|| {
        let s = &unsafe { obj(sprites, "sprites") }?.0;
        let (l, t, w, h) = pick::selection_box(s, animation).ok_or_else(|| fail(Op2fStatus::OutOfRange, format!("no animation {animation}")))?;
        unsafe { put_value(out.cast::<[i32; 4]>(), [l, t, w, h]) }
    })
}

/// Whether a click (`dx`, `dy`) pixels from a unit's pixel hits it, as the game decides.
///
/// # Safety
/// `sprites` is a live handle; `shape` readable; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_pick_hits(sprites: *const Op2fSprites, shape: *const Op2fPickShape, dx: i32, dy: i32, out: *mut bool) -> Op2fStatus {
    guard(|| {
        let s = &unsafe { obj(sprites, "sprites") }?.0;
        let shape = self::shape(unsafe { obj(shape, "shape") }?);
        unsafe { put_value(out, pick::hits(s, shape, dx, dy)) }
    })
}

/// # Safety
/// `candidates` is `count` readable candidates.
unsafe fn candidates(candidates: *const Op2fPickCandidate, count: usize) -> Result<Vec<pick::Candidate>, Op2fStatus> {
    if candidates.is_null() && count > 0 {
        return Err(null("candidates"));
    }
    let list = if count == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(candidates, count) } };
    Ok(list.iter().map(|c| pick::Candidate { x: c.x, y: c.y, owner: c.owner, shape: shape(&c.shape) }).collect())
}

/// What a left click at map pixel (`x`, `y`) selects for `player`: the last of the player's units hit, i.e. the one
/// drawn on top. `candidates` must be sorted by y, as the game's object list is; `*index` receives the position in
/// it, or `OP2F_STATUS_NOT_FOUND` when nothing is hit.
///
/// # Safety
/// `sprites` is a live handle; `candidates` is `count` readable candidates; `index` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_pick_select(
    sprites: *const Op2fSprites,
    candidates: *const Op2fPickCandidate,
    count: usize,
    x: i32,
    y: i32,
    player: u8,
    index: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let s = &unsafe { obj(sprites, "sprites") }?.0;
        let list = unsafe { self::candidates(candidates, count) }?;
        let hit = pick::select(s, &list, x, y, player).ok_or_else(|| fail(Op2fStatus::NotFound, "nothing selected"))?;
        unsafe { put_value(index, hit) }
    })
}

/// The unit a command (Attack, Repair, ...) at map pixel (`x`, `y`) targets: the first unit of any player hit, i.e.
/// the one drawn underneath. Otherwise as `op2f_pick_select`.
///
/// # Safety
/// `sprites` is a live handle; `candidates` is `count` readable candidates; `index` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_pick_target(
    sprites: *const Op2fSprites,
    candidates: *const Op2fPickCandidate,
    count: usize,
    x: i32,
    y: i32,
    index: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let s = &unsafe { obj(sprites, "sprites") }?.0;
        let list = unsafe { self::candidates(candidates, count) }?;
        let hit = pick::target(s, &list, x, y).ok_or_else(|| fail(Op2fStatus::NotFound, "nothing targeted"))?;
        unsafe { put_value(index, hit) }
    })
}
