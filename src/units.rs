//! Which `op2_art.prt` animation the game draws for a unit type, colony, state and direction.
//!
//! The mapping is not in any data file: each unit type's `Init()` in Outpost2.exe stores
//! animation ids into its MapObjectType (TethysAPI `GlobalUnitStats`). `units_table.rs` was
//! extracted from the GOG/OPU Outpost2.exe by `scripts/unit_anims/extract.sh`, like this:
//!
//! 1. Find MapObjectType vtables in `.rdata`: entries 2 and 3 are the shared Save/Load
//!    (0x444FE0, 0x445030), entry 1 is `GetCodeName` (`mov eax, str; ret`, the sheet code name),
//!    entry 4 is `Init`.
//! 2. Disassemble each `Init` (objdump) and collect `mov dword [this+off], imm` stores, tracking
//!    registers loaded with immediates.
//! 3. Vehicles: `this+0x24C` is `int eden[4][16]`, `this+0x34C` is `int plymouth[4][16]`, indexed
//!    `[state][direction]` (Vehicle::GetChassisAnimationIndex at 0x44C600: Eden is MapObject
//!    flag 0x1000; a zero entry falls back to state 0). Buildings: `this+0x250` is
//!    `int eden[10]`, `this+0x278` is `int plymouth[10]`.
//! 4. Turrets: each weapon type's `Select{Lynx,Panther,Tiger,GuardPost}Graphic` (addresses
//!    from TethysAPI) reads a 16-entry table in `.data` (or an immediate for Starflare and
//!    Supernova), indexed `(rotation + 8) >> 4`; EMP and Starflare pick one of two tables by an
//!    owner-player flag (table `a` renders as Eden art).
//! 5. Game animation ids start at 101: SpriteManager indexes its animation array with
//!    `id - 101` (GetNumFrames 0x404D80, GetFrameOptionalInfo 0x404F00), and the smallest id
//!    stored anywhere is 101 (robo-dozer east). Tables here hold `id - 101`, i.e. indices
//!    into `ArtFile::animations`.
//!
//! Directions: `MapObject::rotation_` is 0..=255 with 0 = east, increasing clockwise on screen
//! (32 per UnitDirection step, south = 64). The game picks a column with
//! `((rotation + 8) & 0xF0) >> 4` (0x4857C3), see [`direction_index`]. Lynx, trucks, Scorpion,
//! Spider... have 16 distinct sprites; Panther, Tiger and Robo-Dozer have 8 (each sprite
//! fills two adjacent columns). Within a chassis the art itself runs counterclockwise, so
//! column `d` uses animation `east + (16 - d) % 16` for the 16-sprite types.
//!
//! Vehicle states (row 0 is normal): from GetChassisAnimationIndex, row 1 is chosen when the
//! vehicle's `this+0x34` object matches a condition involving `+0x58/0x5C/0x60` (likely cargo or
//! a busy action), rows 2 and 3 when the type's `+0x44C/+0x450` equals the unit's action byte
//! `+0x21` (e.g. ConVec building, Earthworker/Dozer working). Their exact meaning is unverified.
//! Building slots (Building::GetSpecialAnimationIndex 0x4836E0, chosen by the slot in MapObject +0x36): 0 =
//! construction (scaffold frames ending in the finished building, which is how a built structure stands), 1 and 2 =
//! damaged (30% and 60% of its hit points lost, Building 0x408A20), 3 = destroyed/rubble sequence, 4 = a small image
//! shared between buildings (unknown), 5..=8 = ambient animations played now and then while the structure works
//! (Building::DoAmbientAnimation 0x483580; `op2_sim::looks`). Measured: harness/oracle/input/vis_map_objects.json.

use crate::art::{ArtFile, Sprite, Sprites, Tint};
use crate::Error;

/// Animation index absent from a table.
pub const NONE: u16 = u16::MAX;
/// Game animation id of `ArtFile::animations[0]`.
pub const ANIM_ID_BASE: u32 = 101;

#[derive(Debug, Clone, Copy)]
pub struct VehicleArt {
    /// Sheet code name (`vehicles.txt`, e.g. LIGHT_TANK).
    pub code: &'static str,
    /// Address of the type's `Init()` the table came from.
    pub init: u32,
    /// `[state][direction column]` animation indices; `NONE` where unset.
    pub eden: &'static [[u16; 16]],
    pub plymouth: &'static [[u16; 16]],
}

impl VehicleArt {
    pub fn states(&self, plymouth: bool) -> &'static [[u16; 16]] {
        if plymouth { self.plymouth } else { self.eden }
    }

    /// Animation for a state and direction column (0..16), falling back to state 0 like the game.
    pub fn chassis(&self, plymouth: bool, state: usize, column: usize) -> Option<usize> {
        let states = self.states(plymouth);
        let column = column & 15;
        let pick = states.get(state).map(|s| s[column]).filter(|&a| a != NONE && a != 0);
        pick.or_else(|| states.first().map(|s| s[column]).filter(|&a| a != NONE)).map(usize::from)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BuildingArt {
    pub code: &'static str,
    pub init: u32,
    /// Ten animation slots per colony (see module docs: 0 construction, 1 and 2 damaged, 3 destroyed, 5.. ambient).
    /// `NONE` where unset.
    pub eden: [u16; 10],
    pub plymouth: [u16; 10],
    /// Column of the ambient animation sequence (type +0x2A6 Eden, +0x2A7 Plymouth; 0 = none), `[eden, plymouth]`.
    pub ambient: [u8; 2],
}

impl BuildingArt {
    pub fn slots(&self, plymouth: bool) -> &[u16; 10] {
        if plymouth { &self.plymouth } else { &self.eden }
    }

    /// The type's column of the ambient animation sequence for a colony (0 = no ambient animation).
    pub fn ambient(&self, plymouth: bool) -> u8 {
        self.ambient[plymouth as usize]
    }
}

/// Turret animations of one weapon on one chassis kind, by direction column.
#[derive(Debug, Clone, Copy)]
pub struct TurretSet {
    /// Address of the weapon type's `Select*Graphic` the table came from.
    pub select: u32,
    /// Used when the owning player's flag tested in `Select*Graphic` is set; `b` otherwise.
    /// Only EMP and Starflare differ (colony-specific art); see [`TurretSet::column`].
    pub a: [u16; 16],
    pub b: [u16; 16],
}

impl TurretSet {
    /// Turret animation for a direction column. `plymouth` picks between `a`/`b` for the
    /// weapons whose art differs by colony (visually verified for EMP: `a` is Eden).
    pub fn column(&self, plymouth: bool, column: usize) -> usize {
        (if plymouth { self.b } else { self.a })[column & 15] as usize
    }
}

/// Turret art for a weapon (sheet code name), per chassis.
#[derive(Debug, Clone, Copy)]
pub struct TurretArt {
    pub code: &'static str,
    pub lynx: Option<TurretSet>,
    pub panther: Option<TurretSet>,
    pub tiger: Option<TurretSet>,
    pub guard_post: Option<TurretSet>,
}

impl TurretArt {
    /// Turret set for a chassis sheet code name (LIGHT_TANK, MED_TANK, BIG_TANK, TOWER_GUARD).
    pub fn for_chassis(&self, chassis: &str) -> Option<&TurretSet> {
        match canonical(chassis).to_ascii_uppercase().as_str() {
            "LIGHT_TANK" => self.lynx.as_ref(),
            "MED_TANK" => self.panther.as_ref(),
            "BIG_TANK" => self.tiger.as_ref(),
            "TOWER_GUARD" => self.guard_post.as_ref(),
            _ => None,
        }
    }
}

include!("units_table.rs");

/// Common names for sheet code names.
const ALIASES: &[(&str, &str)] = &[
    ("LYNX", "LIGHT_TANK"),
    ("PANTHER", "MED_TANK"),
    ("TIGER", "BIG_TANK"),
    ("CARGO_TRUCK", "BIG_TRUCK"),
    ("CONVEC", "CON_TRUCK"),
    ("ROBO_SURVEYOR", "SURVEYOR"),
    ("ROBO_MINER", "MINER"),
    ("ROBO_DOZER", "BULL_DOZER"),
    ("EARTHWORKER", "WALL_TRUCK"),
    ("EVACUATION_TRANSPORT", "MOBILE_HOME"),
    ("GEOCON", "GEO_METRO"),
];

fn canonical(name: &str) -> &str {
    ALIASES.iter().find(|(a, _)| a.eq_ignore_ascii_case(name)).map_or(name, |(_, c)| c)
}

pub fn vehicles() -> &'static [VehicleArt] {
    VEHICLES
}

pub fn buildings() -> &'static [BuildingArt] {
    BUILDINGS
}

pub fn turrets() -> &'static [TurretArt] {
    TURRETS
}

pub fn turret(weapon: &str) -> Option<&'static TurretArt> {
    TURRETS.iter().find(|t| t.code.eq_ignore_ascii_case(weapon))
}

pub fn vehicle(name: &str) -> Option<&'static VehicleArt> {
    let name = canonical(name);
    VEHICLES.iter().find(|u| u.code.eq_ignore_ascii_case(name))
}

pub fn building(name: &str) -> Option<&'static BuildingArt> {
    BUILDINGS.iter().find(|u| u.code.eq_ignore_ascii_case(name))
}

/// Direction column (0..16) the game uses for a rotation (0..=255, 0 = east, clockwise).
pub fn direction_index(rotation: u8) -> usize {
    ((rotation as usize + 8) & 0xF0) >> 4
}

/// Rotation (0..=255) at the centre of a direction column.
pub fn column_rotation(column: usize) -> u8 {
    ((column & 15) * 16) as u8
}

fn optional1(art: &ArtFile, anim: usize) -> (i32, i32) {
    art.animations
        .get(anim)
        .and_then(|a| a.frames.first())
        .and_then(|f| f.optional1)
        .map_or((0, 0), |[x, y]| (x as i32, y as i32))
}

/// Where a turret's reference point sits relative to the chassis reference point: frame 0's
/// first optional offset of the chassis animation for the current direction (e.g. Lynx east
/// (0, -5)). Verified visually: turrets sit centred on the hull in all 16 directions. Only `op2_art.prt` is needed
/// (the pixels in OP2_ART.BMP aren't).
pub fn turret_offset(art: &ArtFile, chassis_anim: usize) -> (i32, i32) {
    optional1(art, chassis_anim)
}

/// Where shots leave the barrel, relative to the chassis reference point: the chassis offset
/// plus the turret animation's own first optional offset (Laser east: (14, 0)). This is the
/// sum TankVehicle::SelectTurretGraphic (0x4855C0 Lynx, 0x4857A0 Panther) leaves in globals
/// 0x4EA7BC/0x4EA7C0 after the weapon's `Select*Graphic`. Tiger turrets also carry a second
/// pair (the other barrel), which the game uses when the unit has flag 0x4000; see
/// `Frame::optional2`. Inferred from the code, not yet checked against shots in game.
pub fn muzzle_offset(art: &ArtFile, chassis_anim: usize, turret_anim: usize) -> (i32, i32) {
    let (c, t) = (optional1(art, chassis_anim), optional1(art, turret_anim));
    (c.0 + t.0, c.1 + t.1)
}

/// Draws `turret` over `chassis` (both with hotspots), the turret's hotspot placed at the
/// chassis hotspot plus `offset`. The result's canvas grows to fit both.
pub fn overlay(chassis: &Sprite, turret: &Sprite, offset: (i32, i32)) -> Sprite {
    // Positions of each image's top-left relative to the chassis reference point.
    let c0 = (-chassis.hotspot.0, -chassis.hotspot.1);
    let t0 = (offset.0 - turret.hotspot.0, offset.1 - turret.hotspot.1);
    let x0 = c0.0.min(t0.0);
    let y0 = c0.1.min(t0.1);
    let x1 = (c0.0 + chassis.image.width as i32).max(t0.0 + turret.image.width as i32);
    let y1 = (c0.1 + chassis.image.height as i32).max(t0.1 + turret.image.height as i32);
    let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for (img, (ox, oy)) in [(&chassis.image, c0), (&turret.image, t0)] {
        for y in 0..img.height {
            for x in 0..img.width {
                let s = ((y * img.width + x) * 4) as usize;
                let src: [u8; 4] = img.rgba[s..s + 4].try_into().unwrap();
                if src[3] == 0 {
                    continue;
                }
                let d = ((((oy - y0) as u32 + y) * w + (ox - x0) as u32 + x) * 4) as usize;
                crate::art::blend(&mut rgba[d..d + 4], src);
            }
        }
    }
    Sprite { image: crate::render::Image { width: w, height: h, rgba }, hotspot: (-x0, -y0) }
}

/// A tank with its weapon turret, both facing `rotation`.
pub fn tank_sprite(
    sprites: &Sprites,
    unit: &VehicleArt,
    weapon: &TurretArt,
    plymouth: bool,
    rotation: u8,
    tint: Tint,
) -> Result<Sprite, Error> {
    let column = direction_index(rotation);
    let chassis_anim = unit
        .chassis(plymouth, 0, column)
        .ok_or_else(|| Error::Invalid(format!("{} has no chassis animation", unit.code)))?;
    let set = weapon
        .for_chassis(unit.code)
        .ok_or_else(|| Error::Invalid(format!("{} has no turret art for {}", weapon.code, unit.code)))?;
    let turret_anim = set.column(plymouth, column);
    let chassis = sprites.frame_rgba_aligned(chassis_anim, 0, tint)?;
    let turret = sprites.frame_rgba_aligned(turret_anim, 0, tint)?;
    Ok(overlay(&chassis, &turret, turret_offset(&sprites.art, chassis_anim)))
}

/// Draws a chassis frame for a vehicle at a rotation: the sprite plus its hotspot.
pub fn chassis_sprite(
    sprites: &Sprites,
    unit: &VehicleArt,
    plymouth: bool,
    rotation: u8,
    frame: usize,
    tint: Tint,
) -> Result<Sprite, Error> {
    let anim = unit
        .chassis(plymouth, 0, direction_index(rotation))
        .ok_or_else(|| Error::Invalid(format!("{} has no chassis animation", unit.code)))?;
    let frames = sprites.animation(anim).map_or(1, |a| a.frames.len().max(1));
    sprites.frame_rgba_aligned(anim, frame % frames, tint)
}
