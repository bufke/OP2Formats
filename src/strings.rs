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
            strings.insert(id, unescape(value));
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

/// OPU's escapes in language.ini values (OPUPatch UserInterface.cpp `UnEscapeString`): each of the C escapes in turn,
/// every occurrence replaced from the end of the string back (so `\\n` is a newline, as OPU reads it, not `\n`).
/// String 455, `<C1>\n\tNot Operational`, is the Space Program page's yellow, indented status line.
fn unescape(value: &str) -> String {
    const ESCAPES: [(char, char); 11] = [
        ('a', '\x07'),
        ('b', '\x08'),
        ('f', '\x0C'),
        ('n', '\n'),
        ('r', '\r'),
        ('t', '\t'),
        ('v', '\x0B'),
        ('\\', '\\'),
        ('\'', '\''),
        ('"', '"'),
        ('?', '?'),
    ];
    let mut out: Vec<char> = value.chars().collect();
    for (escaped, plain) in ESCAPES {
        // std::string::rfind(s, p): the last match starting at or before p.
        let mut p = out.len();
        while let Some(at) = (0..out.len().saturating_sub(1)).rev().find(|&k| k <= p && out[k] == '\\' && out[k + 1] == escaped) {
            out.splice(at..at + 2, [plain]);
            p = at;
        }
    }
    out.into_iter().collect()
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
    fn unescapes_like_opu() {
        assert_eq!(unescape("<C1>\\n\\tNot Operational"), "<C1>\n\tNot Operational");
        assert_eq!(unescape("Player %s has quit.\\r\\n"), "Player %s has quit.\r\n");
        // `\n` goes before `\\`: an escaped backslash in front of an n is still read as a newline.
        assert_eq!(unescape("a\\\\nb"), "a\\\nb");
        assert_eq!(unescape("C:\\\\\\\\x"), "C:\\x");
        assert_eq!(unescape("\\\"q\\\" é"), "\"q\" é");
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
