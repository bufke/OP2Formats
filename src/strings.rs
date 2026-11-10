//! The game's UI strings. Outpost2.exe keeps them in its data (a table of string pointers at 0x4E88F8 indexed by
//! string id); OPU externalises the same table to `OPU/base/language.ini` (`[Game]` section, `id="text"`), which is
//! what this reads. [`GameStrings::load`] takes what is there (a stock install has none: every lookup misses);
//! [`GameStrings::load_checked`] refuses a table that lacks any of the ids op2-rs reads ([`LAST_ID`]), so a caller
//! that uses it as its one source of UI text never meets a hole.
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

/// The highest string id op2-rs reads (fixed ids in the panes, dialogs and messages, and the name ranges above).
pub const LAST_ID: u32 = 870;
/// Ids in `0..=LAST_ID` that are not strings (the table has a hole there).
const NOT_STRINGS: [u32; 1] = [832];

/// Why a game's strings can't be used.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("OPU's base/language.ini not found: op2-rs needs OPU 1.4.1 (`op2 setup` installs it)")]
    Missing,
    #[error(
        "OPU's base/language.ini lacks {count} of the {total} strings op2-rs reads (first missing: {first}); op2-rs needs \
         OPU 1.4.1's",
        total = LAST_ID + 1 - NOT_STRINGS.len() as u32
    )]
    Incomplete { count: usize, first: u32 },
}

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

    /// The install's `language.ini`, checked to hold every string op2-rs reads.
    pub fn load_checked(files: &GameFiles) -> Result<Self, Error> {
        let (data, _) = files.read("language.ini").map_err(|_| Error::Missing)?;
        Self::parse_checked(&data.iter().map(|&b| b as char).collect::<String>())
    }

    /// [`parse`](Self::parse) of a whole table, refusing one without every id in `0..=LAST_ID`.
    pub fn parse_checked(text: &str) -> Result<Self, Error> {
        let s = Self::parse(text);
        let missing: Vec<u32> = (0..=LAST_ID).filter(|id| !NOT_STRINGS.contains(id) && !s.strings.contains_key(id)).collect();
        match missing.first() {
            Some(&first) => Err(Error::Incomplete { count: missing.len(), first }),
            None => Ok(s),
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

    /// A string by a fixed id, which [`load_checked`](Self::load_checked) has checked is there.
    pub fn text(&self, id: u32) -> &str {
        self.get(id).unwrap_or_else(|| {
            debug_assert!(false, "string {id} is not in the table");
            ""
        })
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

    /// A table is used only whole: a missing id is reported with how many are, and the install's table passes.
    #[test]
    fn a_table_without_every_id_is_refused() {
        let table = |skip: &[u32]| {
            let mut text = String::from("[Game]\n");
            for id in (0..=LAST_ID).filter(|id| !NOT_STRINGS.contains(id) && !skip.contains(id)) {
                text.push_str(&format!("{id}=\"s{id}\"\n"));
            }
            text
        };
        let all = GameStrings::parse_checked(&table(&[])).expect("a whole table");
        assert_eq!(all.text(30), "s30");
        let err = GameStrings::parse_checked(&table(&[30, 400])).unwrap_err();
        assert!(matches!(err, Error::Incomplete { count: 2, first: 30 }), "{err}");
        assert!(matches!(GameStrings::parse_checked(""), Err(Error::Incomplete { first: 0, .. })));
        let Some(files) = crate::testdata::game_files() else { return };
        GameStrings::load_checked(&files).expect("the install's language.ini holds every string op2-rs reads");
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
