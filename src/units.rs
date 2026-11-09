//! Which `op2_art.prt` animation the game draws for a unit type, colony, state and direction.
//!
//! The mapping is not in any data file: each unit type's `Init()` in Outpost2.exe stores animation ids into its
//! MapObjectType (TethysAPI `GlobalUnitStats`). [`UnitArt::read`] takes them from the player's executable at runtime
//! (through [`crate::exe::GameExe`]), like this:
//!
//! 1. Find the MapObjectType vtables in `.rdata`: entries 2 and 3 are the shared Save/Load (0x444FE0, 0x445030), entry
//!    1 is `GetCodeName` (`mov eax, str; ret`, the sheet code name), entry 4 is `Init`. An `Init` shared by several
//!    types (the base class's, the disasters', the tubes and walls') stores no art.
//! 2. Walk each `Init` and collect its `mov [this+off], constant` stores (`exe::code::stores`).
//! 3. Vehicles: `this+0x24C` is `int eden[4][16]`, `this+0x34C` is `int plymouth[4][16]`, indexed
//!    `[state][direction]` (Vehicle::GetChassisAnimationIndex at 0x44C600: Eden is MapObject
//!    flag 0x1000; a zero entry falls back to state 0). Buildings: `this+0x250` is
//!    `int eden[10]`, `this+0x278` is `int plymouth[10]`.
//! 4. Turrets: the weapon types' vtable entries 7..=10 are `Select{GuardPost,Tiger,Panther,Lynx}Graphic` (the base
//!    class's 0x447180..0x4471B0 where a weapon has none). Each reads a 16-entry table in `.data` (or stores an
//!    immediate in the turret graphic global 0x4EA7B8, for Starflare and Supernova), indexed `(rotation + 8) >> 4`;
//!    EMP and Starflare pick one of two tables by an owner-player flag (table `a` renders as Eden art). The guard post
//!    versions of Starflare and Supernova are their own types (TURRET_DESTRUCT, TURRET_DESTRUCT_ADV).
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
use crate::exe::{Exe, code};
use crate::Error;

/// Animation index absent from a table.
pub const NONE: u16 = u16::MAX;
/// Game animation id of `ArtFile::animations[0]`.
pub const ANIM_ID_BASE: u32 = 101;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VehicleArt {
    /// Sheet code name (`vehicles.txt`, e.g. LIGHT_TANK).
    pub code: String,
    /// Address of the type's `Init()` the table came from.
    pub init: u32,
    /// `[state][direction column]` animation indices; `NONE` where unset.
    pub eden: Vec<[u16; 16]>,
    pub plymouth: Vec<[u16; 16]>,
}

impl VehicleArt {
    pub fn states(&self, plymouth: bool) -> &[[u16; 16]] {
        if plymouth { &self.plymouth } else { &self.eden }
    }

    /// Animation for a state and direction column (0..16), falling back to state 0 like the game.
    pub fn chassis(&self, plymouth: bool, state: usize, column: usize) -> Option<usize> {
        let states = self.states(plymouth);
        let column = column & 15;
        let pick = states.get(state).map(|s| s[column]).filter(|&a| a != NONE && a != 0);
        pick.or_else(|| states.first().map(|s| s[column]).filter(|&a| a != NONE)).map(usize::from)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingArt {
    pub code: String,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurretArt {
    pub code: String,
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

/// Cargo Truck sets: `[colony][cargo - 1][direction column]` game animation ids (food, common and rare ore, common and
/// rare metal, rubble, spacecraft, wreckage, gene bank, and the tenth kind).
pub type CargoArt = [[[u16; 16]; 10]; 2];

/// Every unit type's animations, read from Outpost2.exe ([`UnitArt::read`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitArt {
    pub vehicles: Vec<VehicleArt>,
    pub buildings: Vec<BuildingArt>,
    pub turrets: Vec<TurretArt>,
    /// The Cargo Truck's chassis with each cargo ([`CargoArt`]).
    pub cargo: CargoArt,
    /// The Cargo Truck's dock and undock animations by cargo (0 = empty), `[eden, plymouth][cargo]` game ids; 0 =
    /// none, the plain chassis (`TRUCK_DOCKING`).
    pub truck_dock: [[u16; 11]; 2],
    pub truck_undock: [[u16; 11]; 2],
    /// A structure's death explosion by footprint, `[width - 1][height - 1]` game ids (`STRUCTURE_EXPLOSIONS`).
    pub structure_explosions: [[u16; 4]; 5],
    /// Each type's selection sound (MapObjectType +0x1F0, stored by its `Init`): a SoundID.
    pub select_sounds: Vec<(String, u32)>,
}

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

/// MapObjectType's shared Save and Load (vtable entries 2 and 3), by which the type vtables are found.
const TYPE_SAVE: u32 = 0x444FE0;
const TYPE_LOAD: u32 = 0x445030;
/// MapEntityType's own Select{GuardPost,Tiger,Panther,Lynx}Graphic (vtable entries 7..=10), which draw no turret.
const NO_TURRET: [u32; 4] = [0x447180, 0x447190, 0x4471A0, 0x4471B0];
/// The global a `Select*Graphic` leaves the turret's animation id in (read by TankVehicle::SelectTurretGraphic).
const TURRET_GRAPHIC: u32 = 0x4EA7B8;
/// The guard post types of the self-destruct weapons, whose turret joins their tank weapon's.
const GUARD_POST_TWINS: [(&str, &str); 2] = [("TURRET_DESTRUCT", "SELF_DESTRUCT"), ("TURRET_DESTRUCT_ADV", "SELF_DESTRUCT_ADV")];
/// The Small Capacity Air Transport, unused in the game: its `Init` stores no art worth drawing.
const SKIPPED: &str = "SCAT";
/// CargoTruck::GetChassisAnimationIndex's jump table by cargo (0x4065FC, used at 0x40645E): each case loads the Eden
/// table into EDX and the Plymouth one into ESI, the last load of each being the one for a truck that isn't docking.
const CARGO_CASES: u32 = 0x4065FC;

/// The Cargo Truck's docking tables: 11 words each, 0x18 apart: Eden dock (0x4DEAE0, 120 frames, the truck tipping its
/// load), Eden undock (0x4DEAF8, 125 frames), Plymouth dock (0x4DEB10) and undock (0x4DEB28).
const TRUCK_DOCKING: u32 = 0x4DEAE0;
/// Building::ProcessDeath's explosions (0x408B6E), dwords indexed `width * 4 + height` from 0x4DEB34 for widths 1..=5
/// and heights 1..=4: the 20 entries from 0x4DEB48.
const STRUCTURE_EXPLOSIONS: u32 = 0x4DEB34;

/// One type vtable: its address, code name and `Init`.
struct TypeVtable {
    at: u32,
    code: String,
    init: u32,
}

fn type_vtables(exe: &Exe) -> Vec<TypeVtable> {
    exe.section_dwords(".rdata")
        .filter(|&va| exe.u32(va + 8) == Some(TYPE_SAVE) && exe.u32(va + 12) == Some(TYPE_LOAD))
        .filter_map(|at| {
            // GetCodeName is `mov eax, imm32; ret`.
            let get_code_name = exe.u32(at + 4)?;
            let b = exe.bytes(get_code_name, 6)?;
            if b[0] != 0xB8 || b[5] != 0xC3 {
                return None;
            }
            let code = exe.cstr(u32::from_le_bytes([b[1], b[2], b[3], b[4]]))?;
            Some(TypeVtable { at, code, init: exe.u32(at + 16)? })
        })
        .collect()
}

/// What a `Select*Graphic` picks from: its tables' addresses and its immediate turret ids, in code order.
type TurretPick = (Vec<u32>, Vec<u32>);

/// The [`TurretPick`] of a `Select*Graphic`; `None` if it isn't one (it never sets the turret graphic).
fn turret_select(exe: &Exe, select: u32) -> Option<TurretPick> {
    use iced_x86::{OpKind, Register};
    let body = code::until_ret(exe, select, 0x90)?;
    let (mut tables, mut imms) = (Vec::new(), Vec::new());
    for ins in &body {
        for k in 0..ins.op_count() {
            let low_base = matches!(ins.memory_base(), Register::EAX | Register::EBX | Register::ECX | Register::EDX);
            let disp = ins.memory_displacement32();
            if ins.op_kind(k) == OpKind::Memory && low_base && ins.memory_index() == Register::None && disp >> 16 == 0x4E {
                tables.push(disp);
            }
        }
        if ins.mnemonic() == iced_x86::Mnemonic::Mov
            && ins.op0_kind() == OpKind::Memory
            && ins.memory_base() == Register::None
            && ins.memory_displacement32() == TURRET_GRAPHIC
            && matches!(ins.op1_kind(), OpKind::Immediate32 | OpKind::Immediate8to32)
        {
            imms.push(ins.immediate(1) as u32);
        }
    }
    let writes_graphic = !imms.is_empty() || body.iter().any(|i| i.memory_displacement32() == TURRET_GRAPHIC);
    writes_graphic.then_some((tables, imms))
}

impl UnitArt {
    /// The unit types' animations from `exe` (see the module docs).
    pub fn read(exe: &Exe) -> Result<Self, Error> {
        let bad = |why: String| Error::Invalid(format!("Outpost2.exe: unit animations: {why}"));
        let vtables = type_vtables(exe);
        let art = |v: u32| (v as u16).wrapping_sub(ANIM_ID_BASE as u16);
        let mut out = UnitArt::default();
        for t in &vtables {
            if t.code == SKIPPED || vtables.iter().filter(|o| o.init == t.init).count() > 1 {
                continue;
            }
            let s = code::stores(exe, t.init, 0x1400).ok_or_else(|| bad(format!("{}'s Init isn't code", t.code)))?;
            let at = |off: u32| s.this_dwords.get(&off).copied();
            if let Some(sound) = at(0x1F0) {
                out.select_sounds.push((t.code.clone(), sound));
            }
            if at(0x24C).is_some() {
                let rows = |base: u32| {
                    let mut rows: Vec<[u16; 16]> =
                        (0..4).map(|st| std::array::from_fn(|d| at(base + 4 * (st * 16 + d as u32)).map_or(NONE, art))).collect();
                    while rows.last().is_some_and(|r| r.iter().all(|&a| a == NONE || a == 0)) {
                        rows.pop();
                    }
                    rows
                };
                out.vehicles.push(VehicleArt { code: t.code.clone(), init: t.init, eden: rows(0x24C), plymouth: rows(0x34C) });
            } else if at(0x1F0).is_some() && s.this_dwords.range(0x250..0x2A0).next().is_some() {
                let slots = |base: u32| std::array::from_fn(|i| at(base + 4 * i as u32).filter(|&v| v != 0).map_or(NONE, art));
                let ambient = [0x2A6, 0x2A7].map(|o| s.this_bytes.get(&o).copied().unwrap_or(0));
                out.buildings.push(BuildingArt { code: t.code.clone(), init: t.init, eden: slots(0x250), plymouth: slots(0x278), ambient });
            }
        }
        for t in &vtables {
            let slots: Vec<u32> = (7..=10).filter_map(|k| exe.u32(t.at + 4 * k)).collect();
            if slots.len() != 4 || slots.iter().zip(NO_TURRET).all(|(&s, n)| s == n) {
                continue;
            }
            // [guard post, tiger, panther, lynx]: the base class's (draws nothing), a turret pick, or something else.
            let picks: Vec<Option<Option<TurretPick>>> =
                slots.iter().zip(NO_TURRET).map(|(&select, none)| (select != none).then(|| turret_select(exe, select))).collect();
            // A vtable with no turret pick isn't a weapon type's (the words after a shorter vtable).
            if !picks.iter().any(|p| matches!(p, Some(Some(_)))) {
                continue;
            }
            let mut sets = [None; 4];
            for (k, (pick, &select)) in picks.into_iter().zip(&slots).enumerate() {
                let Some(pick) = pick else { continue };
                let (tables, imms) = pick.ok_or_else(|| bad(format!("{}'s turret routine at {select:#x} isn't one op2-rs reads", t.code)))?;
                let column = |t: u32| -> Option<[u16; 16]> {
                    let mut c = [0; 16];
                    for (i, v) in c.iter_mut().enumerate() {
                        *v = art(exe.u32(t + 4 * i as u32)?);
                    }
                    Some(c)
                };
                let pick: Vec<[u16; 16]> = if !tables.is_empty() {
                    tables.iter().map(|&t| column(t)).collect::<Option<_>>().ok_or_else(|| bad(format!("{}'s turret table", t.code)))?
                } else {
                    imms.iter().map(|&v| [art(v); 16]).collect()
                };
                let &a = pick.first().ok_or_else(|| bad(format!("{}'s turret routine at {select:#x} names no animation", t.code)))?;
                sets[k] = Some(TurretSet { select, a, b: *pick.get(1).unwrap_or(&a) });
            }
            // A guard post twin gives its weapon the guard post's turret, the weapon its tanks'; each chassis has one.
            let twin = GUARD_POST_TWINS.iter().find(|(gp, _)| *gp == t.code);
            let code = twin.map_or(t.code.as_str(), |(_, w)| w);
            let entry = match out.turrets.iter_mut().position(|w| w.code == code) {
                Some(i) => &mut out.turrets[i],
                None => {
                    out.turrets.push(TurretArt { code: code.to_string(), lynx: None, panther: None, tiger: None, guard_post: None });
                    out.turrets.last_mut().expect("just pushed")
                }
            };
            let [gp, tiger, panther, lynx] = sets;
            for (slot, new) in [(&mut entry.guard_post, gp), (&mut entry.tiger, tiger), (&mut entry.panther, panther), (&mut entry.lynx, lynx)] {
                if new.is_some() && slot.is_some() {
                    return Err(bad(format!("{} and {} both draw a turret for one chassis", code, t.code)));
                }
                *slot = slot.or(new);
            }
        }
        out.cargo = cargo_art(exe).ok_or_else(|| bad("no Cargo Truck tables".into()))?;
        let words = |at: u32| -> Option<[u16; 11]> {
            let mut w = [0; 11];
            for (i, v) in w.iter_mut().enumerate() {
                *v = exe.u16(at + 2 * i as u32)?;
            }
            Some(w)
        };
        let docking = |k: u32| words(TRUCK_DOCKING + 0x18 * k).ok_or_else(|| bad("no Cargo Truck docking tables".into()));
        (out.truck_dock, out.truck_undock) = ([docking(0)?, docking(2)?], [docking(1)?, docking(3)?]);
        for (w, row) in out.structure_explosions.iter_mut().enumerate() {
            for (h, v) in row.iter_mut().enumerate() {
                let at = STRUCTURE_EXPLOSIONS + 4 * ((w as u32 + 1) * 4 + h as u32 + 1);
                *v = exe.u32(at).filter(|&a| a < 0x1000).ok_or_else(|| bad("no structure explosion table".into()))? as u16;
            }
        }
        if out.vehicles.is_empty() || out.buildings.is_empty() || out.turrets.is_empty() {
            return Err(bad("no unit types found".into()));
        }
        Ok(out)
    }

    pub fn vehicle(&self, name: &str) -> Option<&VehicleArt> {
        let name = canonical(name);
        self.vehicles.iter().find(|u| u.code.eq_ignore_ascii_case(name))
    }

    pub fn building(&self, name: &str) -> Option<&BuildingArt> {
        self.buildings.iter().find(|u| u.code.eq_ignore_ascii_case(name))
    }

    pub fn turret(&self, weapon: &str) -> Option<&TurretArt> {
        self.turrets.iter().find(|t| t.code.eq_ignore_ascii_case(weapon))
    }

    /// A type's selection sound (a SoundID), by sheet code name.
    pub fn select_sound(&self, code: &str) -> Option<u32> {
        self.select_sounds.iter().find(|(c, _)| c.eq_ignore_ascii_case(code)).map(|&(_, s)| s)
    }
}

/// The Cargo Truck's tables ([`CARGO_CASES`]): per cargo 1..=10, the last `mov edx, imm` (Eden) and `mov esi, imm`
/// (Plymouth) of its case, each the address of 16 words.
fn cargo_art(exe: &Exe) -> Option<CargoArt> {
    use iced_x86::{Decoder, DecoderOptions, FlowControl, Mnemonic, OpKind, Register};
    let mut out = [[[0u16; 16]; 10]; 2];
    for cargo in 1..=10u32 {
        let case = exe.u32(CARGO_CASES + 4 * cargo)?;
        let (mut eden, mut plymouth) = (None, None);
        for ins in Decoder::with_ip(32, exe.code(case, 0x40)?, case as u64, DecoderOptions::NONE) {
            if ins.mnemonic() == Mnemonic::Mov && ins.op0_kind() == OpKind::Register && ins.op1_kind() == OpKind::Immediate32 {
                match ins.op0_register() {
                    Register::EDX => eden = Some(ins.immediate(1) as u32),
                    Register::ESI => plymouth = Some(ins.immediate(1) as u32),
                    _ => {}
                }
            }
            if matches!(ins.flow_control(), FlowControl::UnconditionalBranch | FlowControl::Return) {
                break;
            }
        }
        for (colony, table) in [eden?, plymouth?].into_iter().enumerate() {
            for (column, v) in out[colony][cargo as usize - 1].iter_mut().enumerate() {
                *v = exe.u16(table + 2 * column as u32)?;
            }
        }
    }
    Some(out)
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

/// A Tiger turret's other barrel: frame 0's second optional offset ((0, 0) without one), which the turret graphics
/// take for a shot with flag 0x4000 (Tiger stickyfoam 0x4A6348: the high word of 0x404F00's result).
pub fn second_barrel(art: &ArtFile, turret_anim: usize) -> (i32, i32) {
    art.animations
        .get(turret_anim)
        .and_then(|a| a.frames.first())
        .and_then(|f| f.optional2)
        .map_or((0, 0), |[x, y]| (x as i32, y as i32))
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
        .for_chassis(&unit.code)
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

