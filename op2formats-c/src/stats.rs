//! The rule sheets decoded into stats (as the game loads them), tech trees, keyed sheets like `morale.txt`, and the
//! game's UI strings.

use crate::*;
use op2_formats::sheets::{BuildingStats, MineYield, Owner, Rules, SpaceStats, Tech, Track, VehicleStats, WeaponStats};
use op2_formats::strings::{self, GameStrings};
use std::ffi::c_char;

/// Room for a code name or key in the stats structs, NUL included. A longer one fails with
/// `OP2F_STATUS_INVALID_DATA` rather than being cut short.
pub const OP2F_CODE_BYTES: usize = 64;

fn code(s: &str) -> Result<[c_char; OP2F_CODE_BYTES], Op2fStatus> {
    if s.len() >= OP2F_CODE_BYTES {
        return Err(fail(Op2fStatus::InvalidData, format!("{s:?} is longer than {} bytes", OP2F_CODE_BYTES - 1)));
    }
    let mut out = [0 as c_char; OP2F_CODE_BYTES];
    for (o, &b) in out.iter_mut().zip(s.as_bytes()) {
        *o = b as c_char;
    }
    Ok(out)
}

/// Which colony can use a unit or item.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fOwner {
    Eden,
    Plymouth,
    Both,
    /// Gaia: neither (disasters and the like).
    Gaia,
}

impl From<Owner> for Op2fOwner {
    fn from(o: Owner) -> Self {
        match o {
            Owner::Eden => Op2fOwner::Eden,
            Owner::Plymouth => Op2fOwner::Plymouth,
            Owner::Both => Op2fOwner::Both,
            Owner::Gaia => Op2fOwner::Gaia,
        }
    }
}

/// A vehicle's movement class; the values are the game's TrackType.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fTrack {
    Legged = 0,
    Wheeled = 1,
    Miner = 2,
    Tracked = 3,
    Hover = 4,
}

/// One row of `vehicles.txt`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fVehicleStats {
    pub code: [c_char; OP2F_CODE_BYTES],
    /// Key into the game's unit-name strings (not the name itself).
    pub name_key: [c_char; OP2F_CODE_BYTES],
    pub track: Op2fTrack,
    pub owner: Op2fOwner,
    pub hit_points: i32,
    /// The sheet's armor class, 0-5.
    pub armor_class: u32,
    /// The armor value the game derives from the class (`op2f_armor_value`).
    pub armor: u32,
    /// Ticks per tile scale factor: step time = move_speed * the cell's track speed / 512.
    pub move_speed: i32,
    /// Rotation units (of 256 per turn) per tick.
    pub turn_rate: i32,
    pub common_cost: i32,
    pub rare_cost: i32,
    pub build_time: i32,
    pub vehicle_factory: bool,
    pub arachnid_factory: bool,
    pub cargo_capacity: i32,
    pub sight_range: i32,
    pub has_weapon: bool,
    pub research_topic: u32,
    pub production_rate: i32,
    pub repair_amount: i32,
}

/// One row of `weapons.txt` (weapons, disasters and explosions).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fWeaponStats {
    pub code: [c_char; OP2F_CODE_BYTES],
    pub name_key: [c_char; OP2F_CODE_BYTES],
    pub damage_radius: i32,
    pub pixels_skipped: i32,
    /// Range in tiles.
    pub range: i32,
    pub turret_turn_rate: i32,
    pub owner: Op2fOwner,
    pub concussion_damage: i32,
    pub penetration_damage: i32,
    pub common_cost: i32,
    pub rare_cost: i32,
    pub build_time: i32,
    pub reload_time: i32,
    pub research_topic: u32,
}

/// One row of `building.txt`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fBuildingStats {
    pub code: [c_char; OP2F_CODE_BYTES],
    pub produce_list_key: [c_char; OP2F_CODE_BYTES],
    pub name_key: [c_char; OP2F_CODE_BYTES],
    pub width: i32,
    pub height: i32,
    pub owner: Op2fOwner,
    pub hit_points: i32,
    pub armor_class: u32,
    pub armor: u32,
    pub border: bool,
    pub common_cost: i32,
    pub rare_cost: i32,
    pub structure_kit: bool,
    pub build_points: i32,
    pub common_rubble: i32,
    pub rare_rubble: i32,
    pub power_required: i32,
    pub workers_required: i32,
    pub scientists_required: i32,
    pub sight_range: i32,
    pub dock_all: bool,
    pub dock_truck: bool,
    pub dock_convec: bool,
    pub dock_evac: bool,
    pub has_dock_location_eden: bool,
    pub dock_location_eden: i32,
    pub has_dock_location_plymouth: bool,
    pub dock_location_plymouth: i32,
    pub storage_capacity: i32,
    pub production_capacity: i32,
    pub storage_bays: i32,
    pub auto_targeted: bool,
    pub explosion_size: i32,
    pub resource_priority: i32,
    pub research_topic: u32,
}

/// One row of `space.txt` (satellites, spaceship modules, rockets, luxury goods).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fSpaceStats {
    pub code: [c_char; OP2F_CODE_BYTES],
    pub name_key: [c_char; OP2F_CODE_BYTES],
    pub owner: Op2fOwner,
    pub common_cost: i32,
    pub rare_cost: i32,
    pub build_time: i32,
    pub research_topic: u32,
}

/// One row of `mines.txt`: how a mine's yield changes with the number of truckloads.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fMineYield {
    pub yield_class: [c_char; OP2F_CODE_BYTES],
    pub variant: i32,
    pub initial_yield_percent: i32,
    pub peak_truck: i32,
    pub peak_yield_percent: i32,
    pub min_truck: i32,
    pub min_yield_percent: i32,
}

fn track(t: Track) -> Op2fTrack {
    match t {
        Track::Legged => Op2fTrack::Legged,
        Track::Wheeled => Op2fTrack::Wheeled,
        Track::Miner => Op2fTrack::Miner,
        Track::Tracked => Op2fTrack::Tracked,
        Track::Hover => Op2fTrack::Hover,
    }
}

fn vehicle(v: &VehicleStats) -> Result<Op2fVehicleStats, Op2fStatus> {
    Ok(Op2fVehicleStats {
        code: code(&v.code)?,
        name_key: code(&v.name_key)?,
        track: track(v.track),
        owner: v.owner.into(),
        hit_points: v.hit_points,
        armor_class: v.armor_class,
        armor: v.armor(),
        move_speed: v.move_speed,
        turn_rate: v.turn_rate,
        common_cost: v.common_cost,
        rare_cost: v.rare_cost,
        build_time: v.build_time,
        vehicle_factory: v.vehicle_factory,
        arachnid_factory: v.arachnid_factory,
        cargo_capacity: v.cargo_capacity,
        sight_range: v.sight_range,
        has_weapon: v.has_weapon,
        research_topic: v.research_topic,
        production_rate: v.production_rate,
        repair_amount: v.repair_amount,
    })
}

fn weapon(w: &WeaponStats) -> Result<Op2fWeaponStats, Op2fStatus> {
    Ok(Op2fWeaponStats {
        code: code(&w.code)?,
        name_key: code(&w.name_key)?,
        damage_radius: w.damage_radius,
        pixels_skipped: w.pixels_skipped,
        range: w.range,
        turret_turn_rate: w.turret_turn_rate,
        owner: w.owner.into(),
        concussion_damage: w.concussion_damage,
        penetration_damage: w.penetration_damage,
        common_cost: w.common_cost,
        rare_cost: w.rare_cost,
        build_time: w.build_time,
        reload_time: w.reload_time,
        research_topic: w.research_topic,
    })
}

fn building(b: &BuildingStats) -> Result<Op2fBuildingStats, Op2fStatus> {
    Ok(Op2fBuildingStats {
        code: code(&b.code)?,
        produce_list_key: code(&b.produce_list_key)?,
        name_key: code(&b.name_key)?,
        width: b.width,
        height: b.height,
        owner: b.owner.into(),
        hit_points: b.hit_points,
        armor_class: b.armor_class,
        armor: b.armor(),
        border: b.border,
        common_cost: b.common_cost,
        rare_cost: b.rare_cost,
        structure_kit: b.structure_kit,
        build_points: b.build_points,
        common_rubble: b.common_rubble,
        rare_rubble: b.rare_rubble,
        power_required: b.power_required,
        workers_required: b.workers_required,
        scientists_required: b.scientists_required,
        sight_range: b.sight_range,
        dock_all: b.dock_all,
        dock_truck: b.dock_truck,
        dock_convec: b.dock_convec,
        dock_evac: b.dock_evac,
        has_dock_location_eden: b.dock_location_eden.is_some(),
        dock_location_eden: b.dock_location_eden.unwrap_or(0),
        has_dock_location_plymouth: b.dock_location_plymouth.is_some(),
        dock_location_plymouth: b.dock_location_plymouth.unwrap_or(0),
        storage_capacity: b.storage_capacity,
        production_capacity: b.production_capacity,
        storage_bays: b.storage_bays,
        auto_targeted: b.auto_targeted,
        explosion_size: b.explosion_size,
        resource_priority: b.resource_priority,
        research_topic: b.research_topic,
    })
}

fn space(s: &SpaceStats) -> Result<Op2fSpaceStats, Op2fStatus> {
    Ok(Op2fSpaceStats {
        code: code(&s.code)?,
        name_key: code(&s.name_key)?,
        owner: s.owner.into(),
        common_cost: s.common_cost,
        rare_cost: s.rare_cost,
        build_time: s.build_time,
        research_topic: s.research_topic,
    })
}

fn mine(m: &MineYield) -> Result<Op2fMineYield, Op2fStatus> {
    Ok(Op2fMineYield {
        yield_class: code(&m.yield_class)?,
        variant: m.variant,
        initial_yield_percent: m.initial_yield_percent,
        peak_truck: m.peak_truck,
        peak_yield_percent: m.peak_yield_percent,
        min_truck: m.min_truck,
        min_yield_percent: m.min_yield_percent,
    })
}

/// The armor value the game derives from a sheet armor class (0-5), e.g. class 1 = 180, 5 = 40; 256 means none.
#[unsafe(no_mangle)]
pub extern "C" fn op2f_armor_value(armor_class: u32) -> u32 {
    sheets::armor_value(armor_class)
}

// ---- Rules ----

/// All rule sheets as the game loads them (OPU's loose copies first, else `sheets.vol`).
pub struct Op2fRules(pub(crate) Rules);

/// A table of the rules.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fRulesTable {
    Vehicles,
    Weapons,
    Buildings,
    Space,
    Mines,
}

/// Loads `vehicles.txt`, `weapons.txt`, `building.txt`, `space.txt`, `mines.txt` and `morale.txt`.
///
/// # Safety
/// `game` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_rules_load(game: *const Op2fGame, out: *mut *mut Op2fRules) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        unsafe { put(out, Op2fRules(Rules::load(&game.files).map_err(lib_err)?)) }
    })
}

/// # Safety
/// `rules` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_rules_free(rules: *mut Op2fRules) {
    unsafe { free(rules) }
}

/// Number of rows in a table.
///
/// # Safety
/// `rules` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_rules_count(rules: *const Op2fRules, table: Op2fRulesTable) -> usize {
    unsafe { rules.as_ref() }.map_or(0, |r| {
        let r = &r.0;
        match table {
            Op2fRulesTable::Vehicles => r.vehicles.len(),
            Op2fRulesTable::Weapons => r.weapons.len(),
            Op2fRulesTable::Buildings => r.buildings.len(),
            Op2fRulesTable::Space => r.space.len(),
            Op2fRulesTable::Mines => r.mines.len(),
        }
    })
}

/// Finds a row by its code name (for mines, its yield class), ignoring case.
///
/// # Safety
/// `rules` is a live handle; `code` a string; `row` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_rules_find(rules: *const Op2fRules, table: Op2fRulesTable, code: *const c_char, row: *mut usize) -> Op2fStatus {
    guard(|| {
        let r = &unsafe { obj(rules, "rules") }?.0;
        let code = unsafe { str_arg(code, "code") }?;
        let is = |c: &str| c.eq_ignore_ascii_case(code);
        let found = match table {
            Op2fRulesTable::Vehicles => r.vehicles.iter().position(|v| is(&v.code)),
            Op2fRulesTable::Weapons => r.weapons.iter().position(|v| is(&v.code)),
            Op2fRulesTable::Buildings => r.buildings.iter().position(|v| is(&v.code)),
            Op2fRulesTable::Space => r.space.iter().position(|v| is(&v.code)),
            Op2fRulesTable::Mines => r.mines.iter().position(|v| is(&v.yield_class)),
        };
        let i = found.ok_or_else(|| fail(Op2fStatus::NotFound, format!("no {code:?} in the {table:?} table")))?;
        unsafe { put_value(row, i) }
    })
}

/// Row `row` of `vehicles.txt`.
///
/// # Safety
/// `rules` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_rules_vehicle(rules: *const Op2fRules, row: usize, out: *mut Op2fVehicleStats) -> Op2fStatus {
    guard(|| {
        let v = index(&unsafe { obj(rules, "rules") }?.0.vehicles, row, "vehicle")?;
        unsafe { put_value(out, vehicle(v)?) }
    })
}

/// Row `row` of `weapons.txt`.
///
/// # Safety
/// `rules` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_rules_weapon(rules: *const Op2fRules, row: usize, out: *mut Op2fWeaponStats) -> Op2fStatus {
    guard(|| {
        let w = index(&unsafe { obj(rules, "rules") }?.0.weapons, row, "weapon")?;
        unsafe { put_value(out, weapon(w)?) }
    })
}

/// Row `row` of `building.txt`.
///
/// # Safety
/// `rules` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_rules_building(rules: *const Op2fRules, row: usize, out: *mut Op2fBuildingStats) -> Op2fStatus {
    guard(|| {
        let b = index(&unsafe { obj(rules, "rules") }?.0.buildings, row, "building")?;
        unsafe { put_value(out, building(b)?) }
    })
}

/// Row `row` of `space.txt`.
///
/// # Safety
/// `rules` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_rules_space(rules: *const Op2fRules, row: usize, out: *mut Op2fSpaceStats) -> Op2fStatus {
    guard(|| {
        let s = index(&unsafe { obj(rules, "rules") }?.0.space, row, "space item")?;
        unsafe { put_value(out, space(s)?) }
    })
}

/// Row `row` of `mines.txt`.
///
/// # Safety
/// `rules` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_rules_mine(rules: *const Op2fRules, row: usize, out: *mut Op2fMineYield) -> Op2fStatus {
    guard(|| {
        let m = index(&unsafe { obj(rules, "rules") }?.0.mines, row, "mine yield")?;
        unsafe { put_value(out, mine(m)?) }
    })
}

/// `morale.txt` as a keyed sheet (see `op2f_sheet_parse_keyed`).
///
/// # Safety
/// `rules` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_rules_morale(rules: *const Op2fRules, out: *mut *mut Op2fSheet) -> Op2fStatus {
    guard(|| {
        let r = &unsafe { obj(rules, "rules") }?.0;
        unsafe { put(out, Op2fSheet(keyed(&r.morale))) }
    })
}

/// Where each sheet was read from, by file name (e.g. "vehicles.txt" -> a path, or "sheets.vol:vehicles.txt").
///
/// # Safety
/// `rules` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_rules_sources(rules: *const Op2fRules, out: *mut *mut Op2fStringPairs) -> Op2fStatus {
    guard(|| {
        let r = &unsafe { obj(rules, "rules") }?.0;
        unsafe { put(out, Op2fStringPairs(r.sources.clone())) }
    })
}

// ---- Keyed sheets ----

fn keyed(rows: &std::collections::BTreeMap<String, Vec<String>>) -> Sheet {
    let rows = rows.iter().map(|(name, values)| std::iter::once(name.clone()).chain(values.iter().cloned()).collect()).collect();
    Sheet { columns: Vec::new(), rows }
}

/// Parses a keyed sheet like `morale.txt` (rows of a name and its values, in sections separated by blank lines):
/// every named row, sorted by name, with column 0 the name and the non-empty values after it. It has no columns.
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sheet_parse_keyed(data: *const u8, len: usize, out: *mut *mut Op2fSheet) -> Op2fStatus {
    guard(|| {
        let text: String = unsafe { bytes_arg(data, len) }?.iter().map(|&b| b as char).collect();
        unsafe { put(out, Op2fSheet(keyed(&sheets::parse_keyed_rows(&text)))) }
    })
}

/// Reads a keyed sheet from the game (e.g. "morale.txt"); see `op2f_sheet_parse_keyed`.
///
/// # Safety
/// `game` is a live handle; `name` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_sheet_open_keyed(game: *const Op2fGame, name: *const c_char, out: *mut *mut Op2fSheet) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let (text, _) = sheets::read_text(&game.files, unsafe { str_arg(name, "name") }?).map_err(lib_err)?;
        unsafe { put(out, Op2fSheet(keyed(&sheets::parse_keyed_rows(&text)))) }
    })
}

// ---- Tech trees ----

/// A tech tree file (`edentek.txt`, `ply_tek.txt`, `multitek.txt`, `tutortek.txt`): its research topics.
pub struct Op2fTechTree(pub(crate) Vec<Tech>);

/// A research topic's numbers; its texts come from `op2f_tech_text`, its lists from the other `op2f_tech_*` calls.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fTechInfo {
    pub id: u32,
    pub category: i32,
    /// `COST`, for both colonies.
    pub has_cost: bool,
    pub cost: i32,
    pub has_eden_cost: bool,
    pub eden_cost: i32,
    pub has_plymouth_cost: bool,
    pub plymouth_cost: i32,
    pub has_max_scientists: bool,
    pub max_scientists: i32,
    pub has_lab: bool,
    pub lab: i32,
    pub requires_count: usize,
    pub unit_prop_count: usize,
    /// Other `KEY value` lines (`op2f_tech_other`).
    pub other_count: usize,
}

/// A text of a research topic.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fTechText {
    Name,
    Description,
    Teaser,
    ImproveDescription,
}

/// A stat a research topic changes when researched (`UNIT_PROP CODE Property value`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fTechUnitProp {
    pub code: [c_char; OP2F_CODE_BYTES],
    pub property: [c_char; OP2F_CODE_BYTES],
    pub value: i32,
}

/// The tech tree files a game has, by name.
#[unsafe(no_mangle)]
pub extern "C" fn op2f_tech_file_count() -> usize {
    sheets::TECH_FILES.len()
}

/// Tech tree file name `index` as a static string, e.g. "edentek.txt"; NULL when out of range.
#[unsafe(no_mangle)]
pub extern "C" fn op2f_tech_file_name(index: usize) -> *const c_char {
    const NAMES: [&str; 4] = ["edentek.txt\0", "ply_tek.txt\0", "multitek.txt\0", "tutortek.txt\0"];
    const _: () = assert!(NAMES.len() == sheets::TECH_FILES.len());
    NAMES.get(index).map_or(std::ptr::null(), |n| n.as_ptr().cast())
}

/// Reads and parses a tech tree from the game.
///
/// # Safety
/// `game` is a live handle; `name` a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tech_tree_open(game: *const Op2fGame, name: *const c_char, out: *mut *mut Op2fTechTree) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        let (text, _) = sheets::read_text(&game.files, unsafe { str_arg(name, "name") }?).map_err(lib_err)?;
        unsafe { put(out, Op2fTechTree(sheets::parse_techs(&text).map_err(lib_err)?)) }
    })
}

/// Parses a tech tree from memory (Windows-1252 text).
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tech_tree_parse(data: *const u8, len: usize, out: *mut *mut Op2fTechTree) -> Op2fStatus {
    guard(|| {
        let text: String = unsafe { bytes_arg(data, len) }?.iter().map(|&b| b as char).collect();
        unsafe { put(out, Op2fTechTree(sheets::parse_techs(&text).map_err(lib_err)?)) }
    })
}

/// # Safety
/// `tree` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tech_tree_free(tree: *mut Op2fTechTree) {
    unsafe { free(tree) }
}

/// Number of research topics, in file order.
///
/// # Safety
/// `tree` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tech_count(tree: *const Op2fTechTree) -> usize {
    unsafe { tree.as_ref() }.map_or(0, |t| t.0.len())
}

/// Finds a topic by its id.
///
/// # Safety
/// `tree` is a live handle; `index` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tech_find(tree: *const Op2fTechTree, id: u32, index: *mut usize) -> Op2fStatus {
    guard(|| {
        let techs = &unsafe { obj(tree, "tree") }?.0;
        let i = techs.iter().position(|t| t.id == id).ok_or_else(|| fail(Op2fStatus::NotFound, format!("no tech {id}")))?;
        unsafe { put_value(index, i) }
    })
}

fn tech<'a>(tree: *const Op2fTechTree, index: usize) -> Result<&'a Tech, Op2fStatus> {
    self::index(&unsafe { obj(tree, "tree") }?.0, index, "tech")
}

/// # Safety
/// `tree` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tech_info(tree: *const Op2fTechTree, index: usize, out: *mut Op2fTechInfo) -> Op2fStatus {
    guard(|| {
        let t = tech(tree, index)?;
        let info = Op2fTechInfo {
            id: t.id,
            category: t.category,
            has_cost: t.cost.is_some(),
            cost: t.cost.unwrap_or(0),
            has_eden_cost: t.eden_cost.is_some(),
            eden_cost: t.eden_cost.unwrap_or(0),
            has_plymouth_cost: t.plymouth_cost.is_some(),
            plymouth_cost: t.plymouth_cost.unwrap_or(0),
            has_max_scientists: t.max_scientists.is_some(),
            max_scientists: t.max_scientists.unwrap_or(0),
            has_lab: t.lab.is_some(),
            lab: t.lab.unwrap_or(0),
            requires_count: t.requires.len(),
            unit_prop_count: t.unit_props.len(),
            other_count: t.other.len(),
        };
        unsafe { put_value(out, info) }
    })
}

/// # Safety
/// `tree` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tech_text(
    tree: *const Op2fTechTree,
    index: usize,
    field: Op2fTechText,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let t = tech(tree, index)?;
        let text = match field {
            Op2fTechText::Name => &t.name,
            Op2fTechText::Description => &t.description,
            Op2fTechText::Teaser => &t.teaser,
            Op2fTechText::ImproveDescription => &t.improve_desc,
        };
        unsafe { copy_str(text, buf, cap, len) }
    })
}

/// The ids of the topics this one requires (`requires_count` of them) into `out`, which holds `count`.
///
/// # Safety
/// `tree` is a live handle; `out` is `count` writable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tech_requires(tree: *const Op2fTechTree, index: usize, out: *mut u32, count: usize) -> Op2fStatus {
    guard(|| {
        let r = &tech(tree, index)?.requires;
        if count < r.len() {
            return Err(fail(Op2fStatus::BufferTooSmall, format!("{} requirements, room for {count}", r.len())));
        }
        if out.is_null() && !r.is_empty() {
            return Err(null("out"));
        }
        unsafe { std::ptr::copy_nonoverlapping(r.as_ptr(), out, r.len()) };
        Ok(())
    })
}

/// Stat change `prop` (0 to `unit_prop_count` - 1) of topic `index`.
///
/// # Safety
/// `tree` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tech_unit_prop(tree: *const Op2fTechTree, index: usize, prop: usize, out: *mut Op2fTechUnitProp) -> Op2fStatus {
    guard(|| {
        let (c, p, v) = self::index(&tech(tree, index)?.unit_props, prop, "unit property")?;
        unsafe { put_value(out, Op2fTechUnitProp { code: code(c)?, property: code(p)?, value: *v }) }
    })
}

/// Topic `index`'s other `KEY value` lines, verbatim.
///
/// # Safety
/// `tree` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_tech_other(tree: *const Op2fTechTree, index: usize, out: *mut *mut Op2fStringPairs) -> Op2fStatus {
    guard(|| {
        let t = tech(tree, index)?;
        unsafe { put(out, Op2fStringPairs(t.other.clone())) }
    })
}

// ---- UI strings ----

/// String id of the first vehicle name: vehicle row `r` of `vehicles.txt` is named by string `OP2F_VEHICLE_NAMES + r`.
pub const OP2F_VEHICLE_NAMES: u32 = 718;
/// String id of the first structure name (row 0 of `building.txt`).
pub const OP2F_BUILDING_NAMES: u32 = 735;
/// String id of the first weapon name (row 0 of `weapons.txt`).
pub const OP2F_WEAPON_NAMES: u32 = 819;
const _: () = assert!(
    OP2F_VEHICLE_NAMES == strings::VEHICLE_NAMES
        && OP2F_BUILDING_NAMES == strings::BUILDING_NAMES
        && OP2F_WEAPON_NAMES == strings::WEAPON_NAMES
);

fn table(s: &GameStrings) -> Op2fStringTable {
    // GameStrings keeps its map private; ids run below 4096 in OPU's language.ini.
    Op2fStringTable((0..=u16::MAX as u32).filter_map(|id| s.get(id).map(|t| (id, t.to_string()))).collect())
}

/// The game's UI strings: OPU's `language.ini` (`[Game]` section). A stock install has none, giving an empty table.
///
/// # Safety
/// `game` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_strings_load(game: *const Op2fGame, out: *mut *mut Op2fStringTable) -> Op2fStatus {
    guard(|| {
        let game = unsafe { obj(game, "game") }?;
        unsafe { put(out, table(&GameStrings::load(&game.files))) }
    })
}

/// Parses a `language.ini` from memory (Windows-1252 text), unescaping its values as OPU does.
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_game_strings_parse(data: *const u8, len: usize, out: *mut *mut Op2fStringTable) -> Op2fStatus {
    guard(|| {
        let text: String = unsafe { bytes_arg(data, len) }?.iter().map(|&b| b as char).collect();
        unsafe { put(out, table(&GameStrings::parse(&text))) }
    })
}

/// The string id of the armor description the unit panel shows for an armor value (256 = none, down to 40);
/// structures and vehicles use different words.
#[unsafe(no_mangle)]
pub extern "C" fn op2f_armor_name_id(armor: i32, structure: bool) -> u32 {
    strings::armor_name_id(armor, structure)
}
