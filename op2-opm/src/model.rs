//! The `.opm` data model, after the OP2DotNetMissionSDK's `MissionReader/Json` classes. Members are read by name in
//! any order; an absent one takes its default (`Health` 1 and `Lights` on for units, as the SDK sets them); members
//! this model doesn't have are kept in each struct's `extra` and written back.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::enums::*;

/// Members of an object this model doesn't know, by name, kept to be written back.
pub type Extra = Map<String, Value>;

/// A whole mission (`MissionRoot`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Mission {
    /// The version tag of the SDK that wrote the file, e.g. "0".
    #[serde(rename = "SDKVersion")]
    pub sdk_version: String,
    pub level_details: LevelDetails,
    /// The variant that always applies.
    pub master_variant: Variant,
    /// Alternatives, one of which is added onto the master variant when the mission is played.
    pub mission_variants: Vec<Variant>,
    pub disasters: Vec<Disaster>,
    pub regions: Vec<Region>,
    /// The classic triggers (`OP2TriggerData`).
    pub triggers: Vec<Op2Trigger>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Mission metadata.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct LevelDetails {
    #[serde(rename = "LevelDescription")]
    pub description: String,
    /// The `.map` file the mission plays on.
    pub map_name: String,
    pub max_tech_level: i32,
    pub mission_type: MissionType,
    pub num_players: i32,
    /// The tech tree file, e.g. "MULTITEK.TXT".
    pub tech_tree_name: String,
    pub unit_only_mission: bool,
    #[serde(flatten)]
    pub extra: Extra,
}

/// The master variant or one of the alternatives.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Variant {
    pub auto_layouts: Vec<AutoLayout>,
    /// Unused (`null`) on the master variant.
    pub name: Option<String>,
    pub players: Vec<Player>,
    /// Additions for each difficulty level, parallel to each player's `difficulty_resources`.
    pub tethys_difficulties: Vec<GameData>,
    pub tethys_game: GameData,
    #[serde(flatten)]
    pub extra: Extra,
}

/// One player slot (`PlayerData`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Player {
    pub allies: Vec<i32>,
    pub bot_type: BotType,
    /// Which AI implementation runs the slot (SDK 3.0: empty or "TechCor" the reference bot, "AIv2" the other);
    /// only written when set.
    #[serde(rename = "AIImpl", skip_serializing_if = "Option::is_none")]
    pub ai_impl: Option<String>,
    pub color: Color,
    pub difficulty_resources: Vec<Resources>,
    #[serde(rename = "ID")]
    pub id: i32,
    pub is_eden: bool,
    pub is_human: bool,
    pub resources: Resources,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A player's starting economy, units, walls and tubes (`ResourceData`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Resources {
    /// The tile the camera starts on.
    pub center_view: Point,
    pub common_ore: i32,
    /// Tech ids researched at the start.
    pub completed_research: Vec<i32>,
    pub food: i32,
    pub free_morale: bool,
    pub kids: i32,
    pub morale_level: MoraleLevel,
    pub rare_ore: i32,
    pub scientists: i32,
    pub solar_satellites: i32,
    pub tech_level: i32,
    pub triggers: Vec<Trigger>,
    pub units: Vec<Unit>,
    pub wall_tubes: Vec<WallTube>,
    pub workers: i32,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Lighting, music and the neutral objects on the map (`GameData`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct GameData {
    pub beacons: Vec<Beacon>,
    pub daylight_everywhere: bool,
    pub daylight_moves: bool,
    pub initial_light_level: i32,
    pub markers: Vec<Marker>,
    pub music_play_list: MusicPlayList,
    pub triggers: Vec<Trigger>,
    /// Walls and tubes belonging to no player: written by some SDK 3.0 builds, not by the editor (only written when
    /// there are some).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub wall_tubes: Vec<WallTube>,
    pub wreckage: Vec<Wreckage>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// The background music.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct MusicPlayList {
    /// Where in `song_ids` to loop back to after the last.
    pub repeat_start_index: i32,
    #[serde(rename = "SongIDs")]
    pub song_ids: Vec<i32>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A mining beacon, fumarole or magma vent.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Beacon {
    pub bar_variant: BarVariant,
    pub bar_yield: BarYield,
    #[serde(rename = "ID")]
    pub id: i32,
    #[serde(rename = "MapID")]
    pub kind: BeaconKind,
    pub ore_type: OreType,
    pub position: Point,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A map marker.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Marker {
    #[serde(rename = "ID")]
    pub id: i32,
    pub marker_type: MarkerType,
    pub position: Point,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Wreckage to discover.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Wreckage {
    #[serde(rename = "ID")]
    pub id: i32,
    pub is_visible: bool,
    pub position: Point,
    /// The tech or starship part, as its number (e.g. 8000), unlike `Unit::type_id`.
    #[serde(rename = "TechID")]
    pub tech_id: i32,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A placed vehicle or structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Unit {
    /// For ore mines; `None` (written `null`) otherwise.
    pub bar_variant: Option<BarVariant>,
    pub bar_yield: Option<BarYield>,
    /// A quantity, or a starship module's id (see [`Unit::cargo`]).
    pub cargo_amount: i32,
    /// Truck cargo, a weapon, or "None", as written; [`Unit::cargo`] decodes it.
    pub cargo_type: String,
    pub create_wall: bool,
    pub direction: Direction,
    /// 0 to 1.
    pub health: f64,
    #[serde(rename = "ID")]
    pub id: i32,
    pub ignore_layout: bool,
    pub lights: bool,
    pub max_tubes: Option<i32>,
    pub min_distance: i32,
    pub position: Point,
    pub spawn_distance: i32,
    /// The unit type's `map_id` name, e.g. "CargoTruck", "CommandCenter".
    #[serde(rename = "TypeID")]
    pub type_id: String,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for Unit {
    /// As the SDK reads a unit with nothing set: full health, lights on (its `OnDeserializing`), no cargo.
    fn default() -> Self {
        Self {
            bar_variant: None,
            bar_yield: None,
            cargo_amount: 0,
            cargo_type: "None".into(),
            create_wall: false,
            direction: Direction::default(),
            health: 1.0,
            id: 0,
            ignore_layout: false,
            lights: true,
            max_tubes: None,
            min_distance: 0,
            position: Point::default(),
            spawn_distance: 0,
            type_id: String::new(),
            extra: Extra::new(),
        }
    }
}

/// A wall or tube tile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct WallTube {
    /// "Tube", "Wall", "LavaWall" or "MicrobeWall".
    #[serde(rename = "TypeID")]
    pub type_id: String,
    /// Some SDK builds write this as `Location`; both read, `Position` is written.
    #[serde(alias = "Location")]
    pub position: Point,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A tile position, in the editor's coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// A tile rectangle, corners included.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct DataRect {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

/// A base layout placed around a centre point.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AutoLayout {
    #[serde(rename = "BaseCenterPt")]
    pub base_center: Point,
    #[serde(rename = "PlayerID")]
    pub player_id: i32,
    pub units: Vec<Unit>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A scheduled disaster.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Disaster {
    pub dest_rect: DataRect,
    pub duration: i32,
    pub end_time: i32,
    #[serde(rename = "ID")]
    pub id: i32,
    pub max_delay: i32,
    pub min_delay: i32,
    pub size: i32,
    pub src_rect: DataRect,
    pub start_time: i32,
    /// "Meteor", "Earthquake", "Lightning", "Tornado", "Eruption" or "Blight".
    #[serde(rename = "Type")]
    pub kind: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A named rectangle triggers refer to.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Region {
    pub name: String,
    pub rect: DataRect,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A classic flat trigger (`OP2TriggerData`, in `Mission::triggers`). Which members matter depends on `kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Op2Trigger {
    pub actions: Vec<Op2TriggerAction>,
    pub cargo_amount: i32,
    pub cargo_or_weapon_type: String,
    pub cargo_type: String,
    #[serde(rename = "ChildTriggerIDs")]
    pub child_trigger_ids: Vec<i32>,
    pub compare_type: String,
    pub count: i32,
    pub enabled: bool,
    pub height: i32,
    #[serde(rename = "ID")]
    pub id: i32,
    pub max_time: i32,
    pub message: String,
    pub min_time: i32,
    /// How many children of a `Set` trigger must fire (0: all).
    pub needed_triggers: i32,
    pub one_shot: bool,
    #[serde(rename = "PlayerID")]
    pub player_id: i32,
    pub resource_type: String,
    #[serde(rename = "TechID")]
    pub tech_id: i32,
    pub time: i32,
    #[serde(rename = "TriggerID")]
    pub trigger_id: i32,
    /// "Victory", "Failure", "Research", "Time", "Set", ...
    #[serde(rename = "Type")]
    pub kind: String,
    pub unit_type: String,
    pub width: i32,
    pub x: i32,
    pub y: i32,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for Op2Trigger {
    fn default() -> Self {
        Self {
            actions: Vec::new(),
            cargo_amount: 0,
            cargo_or_weapon_type: String::new(),
            cargo_type: String::new(),
            child_trigger_ids: Vec::new(),
            compare_type: String::new(),
            count: 0,
            enabled: true,
            height: 0,
            id: 0,
            max_time: 0,
            message: String::new(),
            min_time: 0,
            needed_triggers: 0,
            one_shot: false,
            player_id: 0,
            resource_type: String::new(),
            tech_id: 0,
            time: 0,
            trigger_id: 0,
            kind: String::new(),
            unit_type: String::new(),
            width: 0,
            x: 0,
            y: 0,
            extra: Extra::new(),
        }
    }
}

/// What a classic trigger does when it fires (`ActionData`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Op2TriggerAction {
    pub message: String,
    #[serde(rename = "PlayerID")]
    pub player_id: i32,
    #[serde(rename = "SoundID")]
    pub sound_id: i32,
    #[serde(rename = "Type")]
    pub kind: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A condition/action trigger (`TriggerData`, in `GameData::triggers` and `Resources::triggers`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Trigger {
    pub actions: Vec<TriggerAction>,
    /// The JSON key is the singular `Condition`.
    #[serde(rename = "Condition")]
    pub conditions: Vec<TriggerCondition>,
    pub enabled: bool,
    /// When it's evaluated: "OnGameTick", "OnVehicleEnteredRegion", ...
    pub event_type: String,
    #[serde(rename = "ID")]
    pub id: i32,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for Trigger {
    fn default() -> Self {
        Self { actions: Vec::new(), conditions: Vec::new(), enabled: true, event_type: String::new(), id: 0, extra: Extra::new() }
    }
}

/// A trigger's condition; the `value`s mean what its `kind` says.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct TriggerCondition {
    pub comparison: String,
    pub subject: String,
    #[serde(rename = "Type")]
    pub kind: String,
    pub value: String,
    pub value2: String,
    pub value3: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A trigger's action; the `value`s mean what its `kind` says.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct TriggerAction {
    /// "Set", "Add" or "Subtract".
    pub modifier: String,
    pub subject: String,
    pub subject2: String,
    pub subject_player: String,
    pub subject_quantity: i32,
    pub subject_region: String,
    #[serde(rename = "Type")]
    pub kind: String,
    pub value: String,
    pub value2: String,
    pub value3: String,
    pub value4: String,
    pub value5: String,
    #[serde(flatten)]
    pub extra: Extra,
}
