//! The game's UI strings. Outpost2.exe keeps them in its data (a table of string pointers at 0x4E88F8 indexed by
//! string id); OPU externalises the same table to `OPU/base/language.ini` (`[Game]` section, `id="text"`), which is
//! what this reads. Without it (a stock install) every lookup misses and callers fall back to code names.
//!
//! Unit and weapon names follow the sheets' row order: vehicle row `r` is string `VEHICLE_NAMES + r`, and likewise
//! for buildings and weapons (`Lynx` = 718 + 5, `LIGHT_TANK` being row 5 of vehicles.txt).

use std::collections::BTreeMap;

use crate::GameFiles;

/// First vehicle name (vehicles.txt row 0, GeoCon).
pub const VEHICLE_NAMES: u32 = 718;
/// First building name (building.txt row 0, Command Center).
pub const BUILDING_NAMES: u32 = 735;
/// First weapon name (weapons.txt row 0, Laser).
pub const WEAPON_NAMES: u32 = 819;

#[derive(Debug, Clone, Default)]
pub struct GameStrings {
    strings: BTreeMap<u32, String>,
}

impl GameStrings {
    /// `language.ini` if the install has one, else an empty table.
    pub fn load(files: &GameFiles) -> Self {
        match files.read("language.ini") {
            Ok((data, _)) => Self::parse(&data.iter().map(|&b| b as char).collect::<String>()),
            Err(_) => Self::default(),
        }
    }

    /// Parses the `[Game]` section. Values are double-quoted with `\n` escapes; `;` starts a comment.
    pub fn parse(text: &str) -> Self {
        let mut strings = BTreeMap::new();
        let mut in_game = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_game = line.eq_ignore_ascii_case("[game]");
                continue;
            }
            if !in_game || line.starts_with(';') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            let Ok(id) = key.trim().parse::<u32>() else { continue };
            let value = value.trim();
            let value = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(value);
            strings.insert(id, value.replace("\\n", "\n"));
        }
        Self { strings }
    }

    pub fn get(&self, id: u32) -> Option<&str> {
        self.strings.get(&id).map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

/// String id of the armor description the unit panel shows for an armor value (256 = none .. 40), as chosen at
/// 0x45984B: structures and vehicles use different words for the same value.
pub fn armor_name_id(armor: i32, structure: bool) -> u32 {
    if structure {
        match armor {
            256.. => 241, // No Armor
            180.. => 237, // Very Light
            130.. => 240, // Light
            90.. => 236,  // Medium-Light
            60.. => 239,  // Medium
            _ => 238,     // Heavy
        }
    } else {
        match armor {
            256.. => 241, // No Armor
            180.. => 240, // Light
            130.. => 239, // Medium
            90.. => 238,  // Heavy
            60.. => 235,  // Very Heavy
            _ => 234,     // Adamantium
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_game_section() {
        let s = GameStrings::parse(
            "[Language]\nCode=en\n[Game]\n0=\" ENGLISH\"\n; 832 (non-str)\n225=\"Dock is empty.\\nTransfers\"\n723=\"Lynx\"\n",
        );
        assert_eq!(s.get(0), Some(" ENGLISH"));
        assert_eq!(s.get(723), Some("Lynx"));
        assert_eq!(s.get(225), Some("Dock is empty.\nTransfers"));
        assert_eq!(s.get(832), None);
    }

    #[test]
    fn armor_words_match_the_panels() {
        // Measured on harness panels: Lynx/ConVec (180) Light, Vehicle Factory (60) Medium, Command Center and
        // Tokamak (40) Heavy.
        assert_eq!(armor_name_id(180, false), 240);
        assert_eq!(armor_name_id(60, true), 239);
        assert_eq!(armor_name_id(40, true), 238);
    }
}
