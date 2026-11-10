//! Reads and writes `.opm` files: the JSON missions OP2MissionEditor saves (one beside each mission's `.map` and
//! plugin DLL). The format is described in OP2OpmTools' `OPM_FORMAT.md`; the data model follows the
//! OP2DotNetMissionSDK's `MissionReader/Json` classes.
//!
//! Reading accepts what the known writers produce: the editor's (tabs, members in alphabetical order, every member
//! written) and the SDK 3.0 tools' (two-space indents, declaration order, defaults left out), members in any order,
//! `null` for anything, unknown members (kept and written back), and the older SDK layout with the players and
//! `TethysGame` at the top level (moved into the master variant). Writing produces the editor's layout, so
//! OP2MissionEditor can open the result.

mod enums;
mod model;
mod write;

use std::path::Path;

use serde_json::Value;

pub use enums::*;
pub use model::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("the file isn't UTF-8 text")]
    NotUtf8,
}

impl Mission {
    /// Parses `.opm` text. Like Newtonsoft.Json, which the SDK tools read with, this accepts `//` and `/* */`
    /// comments and trailing commas (hand-edited files have them).
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut value: Value = serde_json::from_str(&relax(text.trim_start_matches('\u{feff}')))?;
        drop_nulls(&mut value);
        upgrade_legacy(&mut value);
        Ok(serde_json::from_value(value)?)
    }

    /// Parses an `.opm` file's bytes (UTF-8, with or without a byte-order mark).
    pub fn from_bytes(data: &[u8]) -> Result<Self, Error> {
        Self::parse(std::str::from_utf8(data).map_err(|_| Error::NotUtf8)?)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let data = std::fs::read(path).map_err(|source| Error::Io { path: path.display().to_string(), source })?;
        Self::from_bytes(&data)
    }

    /// The mission as `.opm` text in OP2MissionEditor's layout: tabs, CRLF line ends, members in alphabetical order.
    pub fn to_opm(&self) -> String {
        write::editor_style(&serde_json::to_value(self).expect("a mission always converts to JSON"))
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        let path = path.as_ref();
        std::fs::write(path, self.to_opm()).map_err(|source| Error::Io { path: path.display().to_string(), source })
    }
}

/// Blanks out comments and drops commas before a closing bracket, outside strings; line breaks stay, so error
/// positions still match the file.
fn relax(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let (mut i, mut in_string) = (0, false);
    while i < b.len() {
        let c = b[i];
        if in_string {
            out.push(c);
            if c == b'\\' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 1;
            } else if c == b'"' {
                in_string = false;
            }
        } else if c == b'"' {
            in_string = true;
            out.push(c);
        } else if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                out.push(if b[i] == b'\n' { b'\n' } else { b' ' });
                i += 1;
            }
            i += 2;
            continue;
        } else if c == b',' {
            let next = b[i + 1..].iter().position(|c| !c.is_ascii_whitespace()).map(|p| b[i + 1 + p]);
            if !matches!(next, Some(b']' | b'}')) {
                out.push(c);
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    // Only ASCII bytes were removed or replaced, so the rest is still UTF-8.
    String::from_utf8(out).expect("still UTF-8")
}

/// Removes `null` members, so each takes its default (or `None`) like an absent one: writers emit `null` for unset
/// strings, lists and objects as well as for optional values.
fn drop_nulls(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            map.values_mut().for_each(drop_nulls);
        }
        Value::Array(items) => items.iter_mut().for_each(drop_nulls),
        _ => {}
    }
}

/// The older SDK layout keeps the master variant's members (`Players`, `TethysGame`, ...) at the top level; moves them
/// into a `MasterVariant`. Anything else stays where it is.
fn upgrade_legacy(v: &mut Value) {
    let Value::Object(root) = v else { return };
    if root.contains_key("MasterVariant") {
        return;
    }
    let mut master = serde_json::Map::new();
    for key in ["Players", "TethysGame", "TethysDifficulties", "AutoLayouts"] {
        if let Some(value) = root.remove(key) {
            master.insert(key.into(), value);
        }
    }
    if !master.is_empty() {
        root.insert("MasterVariant".into(), Value::Object(master));
    }
}

/// What a unit's `CargoType` and `CargoAmount` mean together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cargo {
    /// "None", "Empty", or empty.
    None,
    /// Ore, metal, food or rubble in a cargo truck (`TruckCargo` 1-7), `amount` of it.
    Truck { cargo: TruckCargo, amount: i32 },
    /// A starship module (`CargoType` "Spaceport"): its `map_id` number (88-104) from `CargoAmount`.
    StarshipModule(i32),
    /// Wreckage (`CargoType` "Garbage", which the SDK's `TruckCargo` calls wreckage).
    Wreckage,
    /// A gene bank, truck cargo 10, which the SDK's `TruckCargo` has no name for: the editor writes the number 90. The
    /// game keeps a truck cargo's low 4 bits (`Unit::SetTruckCargo`, 0x406CD0), so 90 and 10 load the same truck.
    GeneBank,
    /// A ConVec's structure kit: the structure's `map_id` name (`CommandCenter`, `Tokamak`, ...) and `CargoAmount`,
    /// which the SDK passes to the game as the kit's weapon (a `map_id` number, 0 for none).
    Kit { structure: String, weapon: i32 },
    /// A combat unit's weapon.
    Weapon(Weapon),
    /// Anything else, as written.
    Other(String),
}

/// `TruckCargo` names by number, 0 to 9.
const TRUCK_CARGO: [&str; 10] =
    ["Empty", "Food", "CommonOre", "RareOre", "CommonMetal", "RareMetal", "CommonRubble", "RareRubble", "Spaceport", "Garbage"];

impl Unit {
    /// Decodes `cargo_type` with `cargo_amount`. A ConVec's is a structure kit; a number (the SDK reads one as the
    /// cargo's value) is a truck cargo, decoded from its low 4 bits as the game stores it.
    pub fn cargo(&self) -> Cargo {
        let t = self.cargo_type.trim();
        if t.is_empty() || t.eq_ignore_ascii_case("None") {
            return Cargo::None;
        }
        if self.type_id.eq_ignore_ascii_case("ConVec") {
            return Cargo::Kit { structure: t.to_string(), weapon: self.cargo_amount };
        }
        let truck = match t.parse::<i32>() {
            Ok(n) => match (n & 0xF) as usize {
                10 => return Cargo::GeneBank,
                low if low < TRUCK_CARGO.len() => TruckCargo::from_name(TRUCK_CARGO[low]),
                _ => return Cargo::Other(t.to_string()),
            },
            Err(_) => TruckCargo::from_name(t),
        };
        match truck {
            TruckCargo::Spaceport => return Cargo::StarshipModule(self.cargo_amount),
            TruckCargo::Garbage => return Cargo::Wreckage,
            TruckCargo::Empty => return Cargo::None,
            TruckCargo::Other(_) => {}
            cargo => return Cargo::Truck { cargo, amount: self.cargo_amount },
        }
        match Weapon::from_name(t) {
            Weapon::Other(_) => Cargo::Other(t.to_string()),
            w => Cargo::Weapon(w),
        }
    }
}
