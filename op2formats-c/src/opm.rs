//! `.opm` mission files (OP2MissionEditor's JSON), through op2-opm.
//!
//! Objects are addressed by index: `variant` 0 is the master variant and 1.. the alternatives (`MissionVariants`);
//! `difficulty` -1 is a player's base `Resources` or a variant's `TethysGame`, and 0.. the per-difficulty
//! `DifficultyResources` / `TethysDifficulties`. Enum fields in the structs are the engine's numbers where it has
//! them (`OP2F_OPM_UNKNOWN` for a name without one); every name is also readable as text. What has no getter here
//! (triggers, regions, disasters, layouts) is in `op2f_opm_json`, the mission in a canonical form.

use crate::*;
use op2_opm::{self as opm, Mission};
use std::ffi::c_char;

/// A `.opm` mission.
pub struct Op2fOpm(pub(crate) Mission);

/// An enum field whose name has no engine number (an unknown name, or an absent value).
pub const OP2F_OPM_UNKNOWN: i32 = -2147483647 - 1;
const _: () = assert!(OP2F_OPM_UNKNOWN == i32::MIN);

fn opm_err(e: opm::Error) -> Op2fStatus {
    let status = match &e {
        opm::Error::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => Op2fStatus::NotFound,
        opm::Error::Io { .. } => Op2fStatus::Io,
        opm::Error::Json(_) | opm::Error::NotUtf8 => Op2fStatus::InvalidData,
    };
    fail(status, e.to_string())
}

fn mission<'a>(m: *const Op2fOpm) -> Result<&'a Mission, Op2fStatus> {
    Ok(&unsafe { obj(m, "opm") }?.0)
}

fn variant(m: &Mission, v: usize) -> Result<&opm::Variant, Op2fStatus> {
    if v == 0 { Ok(&m.master_variant) } else { index(&m.mission_variants, v - 1, "variant") }
}

fn player(m: &Mission, v: usize, p: usize) -> Result<&opm::Player, Op2fStatus> {
    index(&variant(m, v)?.players, p, "player")
}

fn resources(m: &Mission, v: usize, p: usize, difficulty: i32) -> Result<&opm::Resources, Op2fStatus> {
    let pl = player(m, v, p)?;
    if difficulty < 0 { Ok(&pl.resources) } else { index(&pl.difficulty_resources, difficulty as usize, "difficulty") }
}

fn game(m: &Mission, v: usize, difficulty: i32) -> Result<&opm::GameData, Op2fStatus> {
    let var = variant(m, v)?;
    if difficulty < 0 { Ok(&var.tethys_game) } else { index(&var.tethys_difficulties, difficulty as usize, "difficulty") }
}

fn number(v: Option<i32>) -> i32 {
    v.unwrap_or(OP2F_OPM_UNKNOWN)
}

// ---- Missions ----

/// Reads an `.opm` file. Any known writer's layout reads: OP2MissionEditor's, the SDK tools', the older SDK layout,
/// with comments, trailing commas, `null`s and members in any order.
///
/// # Safety
/// `path` is a string; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_open(path: *const c_char, out: *mut *mut Op2fOpm) -> Op2fStatus {
    guard(|| {
        let path = unsafe { path_arg(path, "path") }?;
        unsafe { put(out, Op2fOpm(Mission::open(&path).map_err(opm_err)?)) }
    })
}

/// Parses `.opm` text (UTF-8, with or without a byte-order mark).
///
/// # Safety
/// `data` is `len` readable bytes; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_parse(data: *const u8, len: usize, out: *mut *mut Op2fOpm) -> Op2fStatus {
    guard(|| {
        let m = Mission::from_bytes(unsafe { bytes_arg(data, len) }?).map_err(opm_err)?;
        unsafe { put(out, Op2fOpm(m)) }
    })
}

/// # Safety
/// `opm` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_free(opm: *mut Op2fOpm) {
    unsafe { free(opm) }
}

/// The mission as `.opm` text in OP2MissionEditor's layout (tabs, CRLF, members in alphabetical order, every member
/// written), which the editor opens. Members the model doesn't know are written back as they were read. It is also
/// the canonical form to read with a JSON library: names in the SDK's spelling, the older layout moved into
/// `MasterVariant`.
///
/// # Safety
/// `opm` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_json(opm: *const Op2fOpm, out: *mut *mut Op2fBytes) -> Op2fStatus {
    guard(|| unsafe { put(out, Op2fBytes(mission(opm)?.to_opm().into_bytes())) })
}

/// Writes the mission to a file, as `op2f_opm_json` gives it.
///
/// # Safety
/// `opm` is a live handle; `path` a string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_save(opm: *const Op2fOpm, path: *const c_char) -> Op2fStatus {
    guard(|| {
        let path = unsafe { path_arg(path, "path") }?;
        mission(opm)?.save(&path).map_err(opm_err)
    })
}

/// The level details' numbers.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fOpmLevel {
    /// The engine's mission type (-8 colony, -1 last one standing, ... -7 auto demo), or `OP2F_OPM_UNKNOWN`.
    pub mission_type: i32,
    pub num_players: i32,
    pub max_tech_level: i32,
    pub unit_only_mission: bool,
    /// 1 + the alternative variants: the `variant` indices are 0 to this - 1.
    pub variant_count: usize,
}

/// # Safety
/// `opm` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_level(opm: *const Op2fOpm, out: *mut Op2fOpmLevel) -> Op2fStatus {
    guard(|| {
        let m = mission(opm)?;
        let d = &m.level_details;
        let level = Op2fOpmLevel {
            mission_type: number(d.mission_type.value()),
            num_players: d.num_players,
            max_tech_level: d.max_tech_level,
            unit_only_mission: d.unit_only_mission,
            variant_count: 1 + m.mission_variants.len(),
        };
        unsafe { put_value(out, level) }
    })
}

/// A text of the mission.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fOpmText {
    SdkVersion,
    Description,
    /// The `.map` file the mission plays on.
    MapName,
    /// The tech tree file, e.g. "MULTITEK.TXT".
    TechTreeName,
    /// The mission type's name, e.g. "Colony".
    MissionType,
    /// The prefix OP2MissionEditor gives the file name, e.g. "ml2", "c".
    FilePrefix,
}

/// # Safety
/// `opm` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_text(opm: *const Op2fOpm, field: Op2fOpmText, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let m = mission(opm)?;
        let d = &m.level_details;
        let text = match field {
            Op2fOpmText::SdkVersion => m.sdk_version.clone(),
            Op2fOpmText::Description => d.description.clone(),
            Op2fOpmText::MapName => d.map_name.clone(),
            Op2fOpmText::TechTreeName => d.tech_tree_name.clone(),
            Op2fOpmText::MissionType => d.mission_type.name().to_string(),
            Op2fOpmText::FilePrefix => d.mission_type.file_prefix(d.num_players),
        };
        unsafe { copy_str(&text, buf, cap, len) }
    })
}

/// Variant `variant`'s name; `OP2F_STATUS_NOT_FOUND` when it has none (the master variant's is `null`).
///
/// # Safety
/// `opm` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_variant_name(opm: *const Op2fOpm, variant: usize, buf: *mut c_char, cap: usize, len: *mut usize) -> Op2fStatus {
    guard(|| {
        let name = self::variant(mission(opm)?, variant)?.name.as_deref();
        let name = name.ok_or_else(|| fail(Op2fStatus::NotFound, format!("variant {variant} has no name")))?;
        unsafe { copy_str(name, buf, cap, len) }
    })
}

/// Number of difficulty levels a variant has additions for (`TethysDifficulties`); `difficulty` runs to this - 1.
///
/// # Safety
/// `opm` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_difficulty_count(opm: *const Op2fOpm, variant: usize) -> usize {
    unsafe { opm.as_ref() }.and_then(|m| self::variant(&m.0, variant).ok()).map_or(0, |v| v.tethys_difficulties.len())
}

// ---- Players ----

/// Number of players in a variant.
///
/// # Safety
/// `opm` is NULL or a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_player_count(opm: *const Op2fOpm, variant: usize) -> usize {
    unsafe { opm.as_ref() }.and_then(|m| self::variant(&m.0, variant).ok()).map_or(0, |v| v.players.len())
}

/// A player slot.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fOpmPlayer {
    pub id: i32,
    pub is_eden: bool,
    pub is_human: bool,
    /// The engine's colour number (0 blue ... 7 white), or `OP2F_OPM_UNKNOWN`.
    pub color: i32,
    pub ally_count: usize,
    /// Per-difficulty resources (`difficulty` 0 to this - 1).
    pub difficulty_count: usize,
}

/// # Safety
/// `opm` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_player(opm: *const Op2fOpm, variant: usize, player: usize, out: *mut Op2fOpmPlayer) -> Op2fStatus {
    guard(|| {
        let p = self::player(mission(opm)?, variant, player)?;
        let info = Op2fOpmPlayer {
            id: p.id,
            is_eden: p.is_eden,
            is_human: p.is_human,
            color: number(p.color.value()),
            ally_count: p.allies.len(),
            difficulty_count: p.difficulty_resources.len(),
        };
        unsafe { put_value(out, info) }
    })
}

/// The ids of the players a player is allied with: `ally_count` values into `out`, which holds `count`.
///
/// # Safety
/// `opm` is a live handle; `out` is `count` writable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_player_allies(opm: *const Op2fOpm, variant: usize, player: usize, out: *mut i32, count: usize) -> Op2fStatus {
    guard(|| copy_values(&self::player(mission(opm)?, variant, player)?.allies, out, count))
}

/// A text of a player.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fOpmPlayerText {
    /// "Blue", "Red", ...
    Color,
    /// "None", "Balanced", "LaunchStarship", ...
    BotType,
    /// The AI implementation ("AIv2", "TechCor"); `OP2F_STATUS_NOT_FOUND` when unset.
    AiImpl,
}

/// # Safety
/// `opm` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_player_text(
    opm: *const Op2fOpm,
    variant: usize,
    player: usize,
    field: Op2fOpmPlayerText,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let p = self::player(mission(opm)?, variant, player)?;
        let text = match field {
            Op2fOpmPlayerText::Color => p.color.name(),
            Op2fOpmPlayerText::BotType => p.bot_type.name(),
            Op2fOpmPlayerText::AiImpl => p.ai_impl.as_deref().ok_or_else(|| fail(Op2fStatus::NotFound, "no AIImpl"))?,
        };
        unsafe { copy_str(text, buf, cap, len) }
    })
}

/// A player's starting state (`Resources`, or one of its `DifficultyResources`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fOpmResources {
    pub center_x: i32,
    pub center_y: i32,
    pub common_ore: i32,
    pub rare_ore: i32,
    pub food: i32,
    pub kids: i32,
    pub workers: i32,
    pub scientists: i32,
    pub solar_satellites: i32,
    pub tech_level: i32,
    pub free_morale: bool,
    pub completed_research_count: usize,
    pub unit_count: usize,
    pub wall_tube_count: usize,
    pub trigger_count: usize,
}

/// # Safety
/// `opm` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_resources(
    opm: *const Op2fOpm,
    variant: usize,
    player: usize,
    difficulty: i32,
    out: *mut Op2fOpmResources,
) -> Op2fStatus {
    guard(|| {
        let r = self::resources(mission(opm)?, variant, player, difficulty)?;
        let info = Op2fOpmResources {
            center_x: r.center_view.x,
            center_y: r.center_view.y,
            common_ore: r.common_ore,
            rare_ore: r.rare_ore,
            food: r.food,
            kids: r.kids,
            workers: r.workers,
            scientists: r.scientists,
            solar_satellites: r.solar_satellites,
            tech_level: r.tech_level,
            free_morale: r.free_morale,
            completed_research_count: r.completed_research.len(),
            unit_count: r.units.len(),
            wall_tube_count: r.wall_tubes.len(),
            trigger_count: r.triggers.len(),
        };
        unsafe { put_value(out, info) }
    })
}

/// The starting morale level's name ("Good", "Excellent", ...).
///
/// # Safety
/// `opm` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_morale(
    opm: *const Op2fOpm,
    variant: usize,
    player: usize,
    difficulty: i32,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let r = self::resources(mission(opm)?, variant, player, difficulty)?;
        unsafe { copy_str(r.morale_level.name(), buf, cap, len) }
    })
}

/// The tech ids researched at the start: `completed_research_count` values into `out`, which holds `count`.
///
/// # Safety
/// `opm` is a live handle; `out` is `count` writable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_completed_research(
    opm: *const Op2fOpm,
    variant: usize,
    player: usize,
    difficulty: i32,
    out: *mut i32,
    count: usize,
) -> Op2fStatus {
    guard(|| copy_values(&self::resources(mission(opm)?, variant, player, difficulty)?.completed_research, out, count))
}

fn copy_values<T: Copy>(values: &[T], out: *mut T, count: usize) -> Result<(), Op2fStatus> {
    if count < values.len() {
        return Err(fail(Op2fStatus::BufferTooSmall, format!("{} values, room for {count}", values.len())));
    }
    if out.is_null() && !values.is_empty() {
        return Err(null("out"));
    }
    unsafe { std::ptr::copy_nonoverlapping(values.as_ptr(), out, values.len()) };
    Ok(())
}

// ---- Units ----

/// What a unit carries, decoded from its `CargoType` and `CargoAmount`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fOpmCargo {
    None,
    /// Ore, metal, food or rubble in a truck: `cargo` is the SDK's `TruckCargo` number (1-7), `cargo_amount` how much.
    Truck,
    /// A starship module: `cargo` is its `map_id` number (88-104).
    StarshipModule,
    Wreckage,
    GeneBank,
    /// A combat unit's weapon: its name is the `CargoType` text.
    Weapon,
    /// Something else: see the `CargoType` text.
    Other,
}

/// A placed unit.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fOpmUnit {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    /// The engine's facing (0 east ... 7 north-east), or `OP2F_OPM_UNKNOWN`.
    pub direction: i32,
    /// 0 to 1.
    pub health: f64,
    pub lights: bool,
    pub cargo_kind: Op2fOpmCargo,
    /// See `Op2fOpmCargo`; 0 otherwise.
    pub cargo: i32,
    pub cargo_amount: i32,
    /// An ore mine's yield and variant (-1 random, 0.. the engine's), or `OP2F_OPM_UNKNOWN` when not set.
    pub bar_yield: i32,
    pub bar_variant: i32,
    pub create_wall: bool,
    pub ignore_layout: bool,
    pub has_max_tubes: bool,
    pub max_tubes: i32,
    pub min_distance: i32,
    pub spawn_distance: i32,
}

/// Unit `unit` of a player's starting state.
///
/// # Safety
/// `opm` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_unit(
    opm: *const Op2fOpm,
    variant: usize,
    player: usize,
    difficulty: i32,
    unit: usize,
    out: *mut Op2fOpmUnit,
) -> Op2fStatus {
    guard(|| {
        let u = index(&self::resources(mission(opm)?, variant, player, difficulty)?.units, unit, "unit")?;
        let (cargo_kind, cargo) = match u.cargo() {
            opm::Cargo::None => (Op2fOpmCargo::None, 0),
            opm::Cargo::Truck { cargo, .. } => (Op2fOpmCargo::Truck, number(cargo.value())),
            opm::Cargo::StarshipModule(id) => (Op2fOpmCargo::StarshipModule, id),
            opm::Cargo::Wreckage => (Op2fOpmCargo::Wreckage, 0),
            opm::Cargo::GeneBank => (Op2fOpmCargo::GeneBank, 0),
            opm::Cargo::Weapon(_) => (Op2fOpmCargo::Weapon, 0),
            opm::Cargo::Other(_) => (Op2fOpmCargo::Other, 0),
        };
        let info = Op2fOpmUnit {
            id: u.id,
            x: u.position.x,
            y: u.position.y,
            direction: number(u.direction.value()),
            health: u.health,
            lights: u.lights,
            cargo_kind,
            cargo,
            cargo_amount: u.cargo_amount,
            bar_yield: u.bar_yield.as_ref().map_or(OP2F_OPM_UNKNOWN, |b| number(b.value())),
            bar_variant: u.bar_variant.as_ref().map_or(OP2F_OPM_UNKNOWN, |b| number(b.value())),
            create_wall: u.create_wall,
            ignore_layout: u.ignore_layout,
            has_max_tubes: u.max_tubes.is_some(),
            max_tubes: u.max_tubes.unwrap_or(0),
            min_distance: u.min_distance,
            spawn_distance: u.spawn_distance,
        };
        unsafe { put_value(out, info) }
    })
}

/// A text of a unit.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op2fOpmUnitText {
    /// The unit type's `map_id` name, e.g. "CargoTruck", "CommandCenter".
    TypeId,
    /// As written: "None", a truck cargo, a weapon, "90" for a gene bank.
    CargoType,
    /// "East", "SouthEast", ...
    Direction,
}

/// # Safety
/// `opm` is a live handle; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_unit_text(
    opm: *const Op2fOpm,
    variant: usize,
    player: usize,
    difficulty: i32,
    unit: usize,
    field: Op2fOpmUnitText,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let u = index(&self::resources(mission(opm)?, variant, player, difficulty)?.units, unit, "unit")?;
        let text = match field {
            Op2fOpmUnitText::TypeId => u.type_id.as_str(),
            Op2fOpmUnitText::CargoType => u.cargo_type.as_str(),
            Op2fOpmUnitText::Direction => u.direction.name(),
        };
        unsafe { copy_str(text, buf, cap, len) }
    })
}

fn wall_tube(w: &opm::WallTube, x: *mut i32, y: *mut i32, buf: *mut c_char, cap: usize, len: *mut usize) -> Result<(), Op2fStatus> {
    unsafe { put_value(x, w.position.x) }?;
    unsafe { put_value(y, w.position.y) }?;
    unsafe { copy_str(&w.type_id, buf, cap, len) }
}

/// Wall or tube `index` of a player's starting state: its tile and, into `buf`, its type ("Tube", "Wall",
/// "LavaWall", "MicrobeWall").
///
/// # Safety
/// `opm` is a live handle; `x` and `y` writable; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_wall_tube(
    opm: *const Op2fOpm,
    variant: usize,
    player: usize,
    difficulty: i32,
    index: usize,
    x: *mut i32,
    y: *mut i32,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let w = self::index(&self::resources(mission(opm)?, variant, player, difficulty)?.wall_tubes, index, "wall or tube")?;
        wall_tube(w, x, y, buf, cap, len)
    })
}

// ---- The map's setup ----

/// A variant's game setup (`TethysGame`, or one of its `TethysDifficulties`).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fOpmGame {
    pub daylight_everywhere: bool,
    pub daylight_moves: bool,
    pub initial_light_level: i32,
    /// Where the music loops back to in `op2f_opm_songs`.
    pub repeat_start_index: i32,
    pub song_count: usize,
    pub beacon_count: usize,
    pub marker_count: usize,
    pub wreckage_count: usize,
    /// Walls and tubes belonging to no player (some SDK 3.0 files).
    pub wall_tube_count: usize,
    pub trigger_count: usize,
}

/// # Safety
/// `opm` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_game(opm: *const Op2fOpm, variant: usize, difficulty: i32, out: *mut Op2fOpmGame) -> Op2fStatus {
    guard(|| {
        let g = self::game(mission(opm)?, variant, difficulty)?;
        let info = Op2fOpmGame {
            daylight_everywhere: g.daylight_everywhere,
            daylight_moves: g.daylight_moves,
            initial_light_level: g.initial_light_level,
            repeat_start_index: g.music_play_list.repeat_start_index,
            song_count: g.music_play_list.song_ids.len(),
            beacon_count: g.beacons.len(),
            marker_count: g.markers.len(),
            wreckage_count: g.wreckage.len(),
            wall_tube_count: g.wall_tubes.len(),
            trigger_count: g.triggers.len(),
        };
        unsafe { put_value(out, info) }
    })
}

/// The music's song ids, in order: `song_count` values into `out`, which holds `count`.
///
/// # Safety
/// `opm` is a live handle; `out` is `count` writable values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_songs(opm: *const Op2fOpm, variant: usize, difficulty: i32, out: *mut i32, count: usize) -> Op2fStatus {
    guard(|| copy_values(&self::game(mission(opm)?, variant, difficulty)?.music_play_list.song_ids, out, count))
}

/// A beacon.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fOpmBeacon {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    /// -1 random, 0 common, 1 rare; `OP2F_OPM_UNKNOWN` for another name.
    pub ore_type: i32,
    /// -1 random, 0 3-bar, 1 2-bar, 2 1-bar.
    pub bar_yield: i32,
    /// -1 random, 0-2 variants 1-3.
    pub bar_variant: i32,
}

/// Beacon `index` and, into `buf`, its kind ("MiningBeacon", "Fumarole", "MagmaVent").
///
/// # Safety
/// `opm` is a live handle; `out` writable; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_beacon(
    opm: *const Op2fOpm,
    variant: usize,
    difficulty: i32,
    index: usize,
    out: *mut Op2fOpmBeacon,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let b = self::index(&self::game(mission(opm)?, variant, difficulty)?.beacons, index, "beacon")?;
        let info = Op2fOpmBeacon {
            id: b.id,
            x: b.position.x,
            y: b.position.y,
            ore_type: number(b.ore_type.value()),
            bar_yield: number(b.bar_yield.value()),
            bar_variant: number(b.bar_variant.value()),
        };
        unsafe { put_value(out, info) }?;
        unsafe { copy_str(b.kind.name(), buf, cap, len) }
    })
}

/// Marker `index`: its id and tile and, into `buf`, its type ("Circle", "DNA", "Beaker", ...).
///
/// # Safety
/// `opm` is a live handle; `id`, `x` and `y` writable; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_marker(
    opm: *const Op2fOpm,
    variant: usize,
    difficulty: i32,
    index: usize,
    id: *mut i32,
    x: *mut i32,
    y: *mut i32,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let k = self::index(&self::game(mission(opm)?, variant, difficulty)?.markers, index, "marker")?;
        unsafe { put_value(id, k.id) }?;
        unsafe { put_value(x, k.position.x) }?;
        unsafe { put_value(y, k.position.y) }?;
        unsafe { copy_str(k.marker_type.name(), buf, cap, len) }
    })
}

/// A piece of wreckage.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Op2fOpmWreckage {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    /// The tech or starship part's number, e.g. 8000.
    pub tech_id: i32,
    pub is_visible: bool,
}

/// # Safety
/// `opm` is a live handle; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_wreckage(opm: *const Op2fOpm, variant: usize, difficulty: i32, index: usize, out: *mut Op2fOpmWreckage) -> Op2fStatus {
    guard(|| {
        let w = self::index(&self::game(mission(opm)?, variant, difficulty)?.wreckage, index, "wreckage")?;
        let info = Op2fOpmWreckage { id: w.id, x: w.position.x, y: w.position.y, tech_id: w.tech_id, is_visible: w.is_visible };
        unsafe { put_value(out, info) }
    })
}

/// Neutral wall or tube `index` of a variant's game setup, as `op2f_opm_wall_tube`.
///
/// # Safety
/// `opm` is a live handle; `x` and `y` writable; `buf` and `len` as in the string convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn op2f_opm_game_wall_tube(
    opm: *const Op2fOpm,
    variant: usize,
    difficulty: i32,
    index: usize,
    x: *mut i32,
    y: *mut i32,
    buf: *mut c_char,
    cap: usize,
    len: *mut usize,
) -> Op2fStatus {
    guard(|| {
        let w = self::index(&self::game(mission(opm)?, variant, difficulty)?.wall_tubes, index, "wall or tube")?;
        wall_tube(w, x, y, buf, cap, len)
    })
}
