//! Game rules: the tab-separated "sheets" (`vehicles.txt`, `weapons.txt`, `building.txt`,
//! `space.txt`, `mines.txt`, `morale.txt`) and the tech trees (`*tek.txt`).
//!
//! Stock copies live LZH-compressed in `sheets.vol`; OPU ships edited loose copies in
//! `OPU/base/sheets` and `OPU/base/techs`, which OPU makes the game load instead (that is the
//! purpose of its `base` folder), so [`GameFiles::read`] returning the loose file matches what
//! the game uses. The values agree with what the harness dumped from the running game
//! (`harness/oracle/runs/*/gamedata.json`: hit points, move speed, turn rate, armor). OPU's
//! edits are small, e.g. GeoCon, Repair Vehicle and Scorpions become available to both
//! colonies (owner `E`/`P` -> `B`).
//!
//! Sheet layout: two header rows (each column's name is split across them), then one row per
//! unit keyed by its code name, until a blank line. The stock files end in a few bytes of
//! garbage (OPU's copies imitate it), so parsing stops at the first blank line.

use std::collections::BTreeMap;

use crate::{Error, GameFiles};

/// A tab-separated sheet: column names (header rows joined with a space) and rows of cells.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Sheet {
    pub fn parse(text: &str) -> Self {
        let mut lines = text.lines().map(|l| l.trim_end_matches('\r'));
        let split = |l: &str| l.split('\t').map(|c| c.trim().to_string()).collect::<Vec<_>>();
        let h1 = lines.next().map(split).unwrap_or_default();
        let h2 = lines.next().map(split).unwrap_or_default();
        let columns = (0..h1.len().max(h2.len()))
            .map(|i| {
                let parts = [h1.get(i), h2.get(i)];
                parts.into_iter().flatten().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" ")
            })
            .collect();
        let rows = lines.take_while(|l| !l.trim().is_empty() && !l.starts_with(';')).map(split).collect();
        Self { columns, rows }
    }

    pub fn row(&self, code: &str) -> Option<&[String]> {
        self.rows.iter().find(|r| r.first().is_some_and(|c| c.eq_ignore_ascii_case(code))).map(Vec::as_slice)
    }
}

/// Like [`Sheet`] but for `morale.txt`, whose sections are `NAME, values...` rows separated by
/// blank lines: every non-comment row with a name, keyed by name, with its non-empty values.
pub fn parse_keyed_rows(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for line in text.lines().map(|l| l.trim_end_matches('\r')) {
        if line.starts_with(';') {
            break; // trailing garbage marker in OPU copies
        }
        let mut cells = line.split('\t').map(str::trim);
        let Some(name) = cells.next().filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_graphic())) else {
            continue;
        };
        out.insert(name.to_string(), cells.filter(|c| !c.is_empty()).map(String::from).collect());
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Eden,
    Plymouth,
    Both,
    Gaia,
}

impl Owner {
    fn parse(s: &str) -> Self {
        match s {
            "E" => Owner::Eden,
            "P" => Owner::Plymouth,
            "G" => Owner::Gaia,
            _ => Owner::Both,
        }
    }
}

/// Movement class. Discriminants are the game's TrackType (gamedata.json `trackTypes`), which
/// indexes each cell type's `trackSpeed` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Track {
    Legged = 0,
    Wheeled = 1,
    Miner = 2,
    Tracked = 3,
    Hover = 4,
}

impl Track {
    fn parse(s: &str) -> Self {
        match s {
            "L" => Track::Legged,
            "M" => Track::Miner,
            "T" => Track::Tracked,
            "H" => Track::Hover,
            _ => Track::Wheeled,
        }
    }
}

/// Armor value the game derives from a sheet armor class (0-5), as loaded in memory
/// (gamedata.json `armor`): e.g. Lynx class 1 = 180, Tiger class 3 = 90, Command Center 5 = 40.
/// Presumably a damage multiplier out of 256.
pub fn armor_value(class: u32) -> u32 {
    [256, 180, 130, 90, 60, 40].get(class as usize).copied().unwrap_or(256)
}

fn num(cells: &[String], i: usize) -> i32 {
    cells.get(i).and_then(|c| c.parse().ok()).unwrap_or(0)
}

fn flag(cells: &[String], i: usize) -> bool {
    cells.get(i).is_some_and(|c| c.eq_ignore_ascii_case("Y"))
}

fn text(cells: &[String], i: usize) -> String {
    cells.get(i).cloned().unwrap_or_default()
}

/// One row of `vehicles.txt`.
#[derive(Debug, Clone)]
pub struct VehicleStats {
    pub code: String,
    /// Key into the game's unit-name strings (not the name itself).
    pub name_key: String,
    pub track: Track,
    pub owner: Owner,
    pub hit_points: i32,
    pub armor_class: u32,
    /// Ticks per tile scale factor: step time = move_speed * trackSpeed / 512 (tooling).
    pub move_speed: i32,
    /// Rotation units (of 256 per turn) per tick.
    pub turn_rate: i32,
    pub common_cost: i32,
    pub rare_cost: i32,
    pub build_time: i32,
    /// Built at a Vehicle Factory / Arachnid Factory.
    pub vehicle_factory: bool,
    pub arachnid_factory: bool,
    pub cargo_capacity: i32,
    pub sight_range: i32,
    pub has_weapon: bool,
    pub research_topic: u32,
    pub production_rate: i32,
    pub repair_amount: i32,
}

impl VehicleStats {
    fn from_row(r: &[String]) -> Self {
        Self {
            code: text(r, 0),
            name_key: text(r, 1),
            track: Track::parse(&text(r, 2)),
            owner: Owner::parse(&text(r, 3)),
            hit_points: num(r, 4),
            armor_class: num(r, 5) as u32,
            move_speed: num(r, 6),
            turn_rate: num(r, 7),
            common_cost: num(r, 8),
            rare_cost: num(r, 9),
            build_time: num(r, 10),
            vehicle_factory: flag(r, 11),
            arachnid_factory: flag(r, 12),
            cargo_capacity: num(r, 13),
            sight_range: num(r, 14),
            has_weapon: flag(r, 15),
            research_topic: num(r, 16) as u32,
            production_rate: num(r, 17),
            repair_amount: num(r, 18),
        }
    }

    pub fn armor(&self) -> u32 {
        armor_value(self.armor_class)
    }
}

/// One row of `weapons.txt` (weapons, disasters and explosions).
#[derive(Debug, Clone)]
pub struct WeaponStats {
    pub code: String,
    pub name_key: String,
    pub damage_radius: i32,
    pub pixels_skipped: i32,
    /// Range in tiles.
    pub range: i32,
    pub turret_turn_rate: i32,
    pub owner: Owner,
    pub concussion_damage: i32,
    pub penetration_damage: i32,
    pub common_cost: i32,
    pub rare_cost: i32,
    pub build_time: i32,
    pub reload_time: i32,
    pub research_topic: u32,
}

impl WeaponStats {
    fn from_row(r: &[String]) -> Self {
        Self {
            code: text(r, 0),
            name_key: text(r, 1),
            damage_radius: num(r, 2),
            pixels_skipped: num(r, 3),
            range: num(r, 4),
            turret_turn_rate: num(r, 5),
            owner: Owner::parse(&text(r, 6)),
            concussion_damage: num(r, 7),
            penetration_damage: num(r, 8),
            common_cost: num(r, 9),
            rare_cost: num(r, 10),
            build_time: num(r, 11),
            reload_time: num(r, 12),
            research_topic: num(r, 13) as u32,
        }
    }
}

/// One row of `building.txt`.
#[derive(Debug, Clone)]
pub struct BuildingStats {
    pub code: String,
    pub produce_list_key: String,
    pub name_key: String,
    pub width: i32,
    pub height: i32,
    pub owner: Owner,
    pub hit_points: i32,
    pub armor_class: u32,
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
    /// Docking: all vehicles, cargo trucks, ConVecs, evacuation transports.
    pub dock_all: bool,
    pub dock_truck: bool,
    pub dock_convec: bool,
    pub dock_evac: bool,
    pub dock_location_eden: Option<i32>,
    pub dock_location_plymouth: Option<i32>,
    pub storage_capacity: i32,
    pub production_capacity: i32,
    pub storage_bays: i32,
    pub auto_targeted: bool,
    pub explosion_size: i32,
    pub resource_priority: i32,
    pub research_topic: u32,
}

impl BuildingStats {
    fn from_row(r: &[String]) -> Self {
        let opt = |i: usize| r.get(i).and_then(|c| c.parse().ok());
        Self {
            code: text(r, 0),
            produce_list_key: text(r, 1),
            name_key: text(r, 2),
            width: num(r, 3),
            height: num(r, 4),
            owner: Owner::parse(&text(r, 5)),
            hit_points: num(r, 6),
            armor_class: num(r, 7) as u32,
            border: flag(r, 8),
            common_cost: num(r, 9),
            rare_cost: num(r, 10),
            structure_kit: flag(r, 11),
            build_points: num(r, 12),
            common_rubble: num(r, 13),
            rare_rubble: num(r, 14),
            power_required: num(r, 15),
            workers_required: num(r, 16),
            scientists_required: num(r, 17),
            sight_range: num(r, 18),
            dock_all: flag(r, 19),
            dock_truck: flag(r, 20),
            dock_convec: flag(r, 21),
            dock_evac: flag(r, 22),
            dock_location_eden: opt(23),
            dock_location_plymouth: opt(24),
            storage_capacity: num(r, 25),
            production_capacity: num(r, 26),
            storage_bays: num(r, 27),
            auto_targeted: flag(r, 28),
            explosion_size: num(r, 29),
            resource_priority: num(r, 30),
            research_topic: num(r, 31) as u32,
        }
    }

    pub fn armor(&self) -> u32 {
        armor_value(self.armor_class)
    }
}

/// One row of `space.txt` (satellites, spaceship modules, rockets, luxury goods).
#[derive(Debug, Clone)]
pub struct SpaceStats {
    pub code: String,
    pub name_key: String,
    pub owner: Owner,
    pub common_cost: i32,
    pub rare_cost: i32,
    pub build_time: i32,
    pub research_topic: u32,
}

/// One row of `mines.txt`: how a mine's yield changes with the number of trucks loaded.
#[derive(Debug, Clone)]
pub struct MineYield {
    pub yield_class: String,
    pub variant: i32,
    pub initial_yield_percent: i32,
    pub peak_truck: i32,
    pub peak_yield_percent: i32,
    pub min_truck: i32,
    pub min_yield_percent: i32,
}

/// A research topic from a tech tree file.
#[derive(Debug, Clone, Default)]
pub struct Tech {
    pub id: u32,
    pub name: String,
    pub category: i32,
    pub requires: Vec<u32>,
    /// `COST` (both colonies) or the colony-specific `EDEN_COST` / `PLYMOUTH_COST`.
    pub cost: Option<i32>,
    pub eden_cost: Option<i32>,
    pub plymouth_cost: Option<i32>,
    pub max_scientists: Option<i32>,
    pub lab: Option<i32>,
    pub description: String,
    pub teaser: String,
    pub improve_desc: String,
    /// `UNIT_PROP CODE Property value`: stat changes when researched.
    pub unit_props: Vec<(String, String, i32)>,
    /// Any other `KEY value` lines, verbatim.
    pub other: Vec<(String, String)>,
}

/// Parses a tech tree (`BEGIN_TECH "Name" id ... END_TECH` blocks; `;` starts a comment).
pub fn parse_techs(text: &str) -> Result<Vec<Tech>, Error> {
    let mut techs = Vec::new();
    let mut cur: Option<Tech> = None;
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        let (key, rest) = line.split_once(char::is_whitespace).map_or((line, ""), |(k, r)| (k, r.trim()));
        let quoted = || rest.trim_matches('"').to_string();
        let int = || rest.split_whitespace().next().and_then(|v| v.parse::<i32>().ok());
        let bad = |what: &str| Error::Invalid(format!("tech file line {}: {what}", n + 1));
        match key {
            "BEGIN_TECH" => {
                let (name, id) = rest.rsplit_once(char::is_whitespace).ok_or_else(|| bad("BEGIN_TECH without id"))?;
                let id = id.parse().map_err(|_| bad("bad tech id"))?;
                cur = Some(Tech { id, name: name.trim().trim_matches('"').to_string(), ..Default::default() });
            }
            "END_TECH" => techs.extend(cur.take()),
            _ => {
                let Some(t) = cur.as_mut() else {
                    // Outside a block: OPU's "garbage" trailer and the like.
                    continue;
                };
                match key {
                    "CATEGORY" => t.category = int().unwrap_or(0),
                    "REQUIRES" => t.requires.extend(int().map(|v| v as u32)),
                    "COST" => t.cost = int(),
                    "EDEN_COST" => t.eden_cost = int(),
                    "PLYMOUTH_COST" => t.plymouth_cost = int(),
                    "MAX_SCIENTISTS" => t.max_scientists = int(),
                    "LAB" => t.lab = int(),
                    "DESCRIPTION" => t.description = quoted(),
                    "TEASER" => t.teaser = quoted(),
                    "IMPROVE_DESC" => t.improve_desc = quoted(),
                    "UNIT_PROP" => {
                        let parts: Vec<&str> = rest.split_whitespace().collect();
                        if let [code, prop, value] = parts[..] {
                            let value = value.parse().map_err(|_| bad("bad UNIT_PROP value"))?;
                            t.unit_props.push((code.to_string(), prop.to_string(), value));
                        } else {
                            return Err(bad("UNIT_PROP needs CODE Property value"));
                        }
                    }
                    _ => t.other.push((key.to_string(), rest.to_string())),
                }
            }
        }
    }
    Ok(techs)
}

/// All rule sheets, as the game loads them (loose OPU copies first, else `sheets.vol`).
#[derive(Debug, Clone)]
pub struct Rules {
    pub vehicles: Vec<VehicleStats>,
    pub weapons: Vec<WeaponStats>,
    pub buildings: Vec<BuildingStats>,
    pub space: Vec<SpaceStats>,
    pub mines: Vec<MineYield>,
    /// `morale.txt` rows by name (morale levels with their columns, then event modifiers).
    pub morale: BTreeMap<String, Vec<String>>,
    /// Where each sheet came from, by file name.
    pub sources: Vec<(String, String)>,
}

pub const SHEET_FILES: [&str; 6] = ["vehicles.txt", "weapons.txt", "building.txt", "space.txt", "mines.txt", "morale.txt"];
pub const TECH_FILES: [&str; 4] = ["edentek.txt", "ply_tek.txt", "multitek.txt", "tutortek.txt"];

/// Reads a text file from the game (sheets are Windows-1252; non-ASCII only appears in garbage).
pub fn read_text(files: &GameFiles, name: &str) -> Result<(String, String), Error> {
    let (data, source) = files.read(name)?;
    Ok((data.iter().map(|&b| b as char).collect(), source.to_string()))
}

impl Rules {
    pub fn load(files: &GameFiles) -> Result<Self, Error> {
        let mut sources = Vec::new();
        let mut sheet = |name: &str| -> Result<String, Error> {
            let (text, source) = read_text(files, name)?;
            sources.push((name.to_string(), source));
            Ok(text)
        };
        let vehicles = Sheet::parse(&sheet("vehicles.txt")?).rows.iter().map(|r| VehicleStats::from_row(r)).collect();
        let weapons = Sheet::parse(&sheet("weapons.txt")?).rows.iter().map(|r| WeaponStats::from_row(r)).collect();
        let buildings = Sheet::parse(&sheet("building.txt")?).rows.iter().map(|r| BuildingStats::from_row(r)).collect();
        let space = Sheet::parse(&sheet("space.txt")?)
            .rows
            .iter()
            .map(|r| SpaceStats {
                code: text(r, 0),
                name_key: text(r, 1),
                owner: Owner::parse(&text(r, 2)),
                common_cost: num(r, 3),
                rare_cost: num(r, 4),
                build_time: num(r, 5),
                research_topic: num(r, 6) as u32,
            })
            .collect();
        // mines.txt has a single header row.
        let mines_text = sheet("mines.txt")?;
        let mines = mines_text
            .lines()
            .skip(1)
            .map(|l| l.trim_end_matches('\r'))
            .take_while(|l| !l.trim().is_empty())
            .map(|l| {
                let r: Vec<String> = l.split('\t').map(|c| c.trim().to_string()).collect();
                MineYield {
                    yield_class: text(&r, 0),
                    variant: num(&r, 1),
                    initial_yield_percent: num(&r, 2),
                    peak_truck: num(&r, 3),
                    peak_yield_percent: num(&r, 4),
                    min_truck: num(&r, 5),
                    min_yield_percent: num(&r, 6),
                }
            })
            .collect();
        let morale = parse_keyed_rows(&sheet("morale.txt")?);
        Ok(Self { vehicles, weapons, buildings, space, mines, morale, sources })
    }

    pub fn vehicle(&self, code: &str) -> Option<&VehicleStats> {
        self.vehicles.iter().find(|v| v.code.eq_ignore_ascii_case(code))
    }

    pub fn weapon(&self, code: &str) -> Option<&WeaponStats> {
        self.weapons.iter().find(|v| v.code.eq_ignore_ascii_case(code))
    }

    pub fn building(&self, code: &str) -> Option<&BuildingStats> {
        self.buildings.iter().find(|v| v.code.eq_ignore_ascii_case(code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sheet_and_stops_at_garbage() {
        let text = "Code_Name\tUnit Name\tHit\r\n\t\tPoints\r\nLIGHT_TANK\tf\t300\r\nSCOUT\tm\t125\r\n\r\n; Garbage \u{fe}\r\n";
        let sheet = Sheet::parse(text);
        assert_eq!(sheet.columns, ["Code_Name", "Unit Name", "Hit Points"]);
        assert_eq!(sheet.rows.len(), 2);
        assert_eq!(sheet.row("scout").unwrap()[2], "125");
    }

    #[test]
    fn parses_tech_blocks() {
        let text = "; comment\nBEGIN_TECH \"Rail Gun\" 05506\n    CATEGORY 3\n    REQUIRES 05201\n    REQUIRES 05102\n\
                    EDEN_COST 1200\n    DESCRIPTION \"Fires \"\n    UNIT_PROP LIGHT_TANK Move_Speed 2\n    LAB 2\nEND_TECH\n";
        let techs = parse_techs(text).unwrap();
        assert_eq!(techs.len(), 1);
        let t = &techs[0];
        assert_eq!((t.id, t.name.as_str(), t.category), (5506, "Rail Gun", 3));
        assert_eq!(t.requires, [5201, 5102]);
        assert_eq!((t.eden_cost, t.cost, t.lab), (Some(1200), None, Some(2)));
        assert_eq!(t.unit_props, [("LIGHT_TANK".to_string(), "Move_Speed".to_string(), 2)]);
    }
}
