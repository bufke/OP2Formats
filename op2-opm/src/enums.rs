//! The `.opm` enums. Each is written by name; a name this library doesn't know is kept as `Other` (and written back
//! as it came) rather than failing the read, since newer editors add values. Names match ignoring case
//! (`Southeast` reads as `SouthEast`) and are written in the editor's spelling.

macro_rules! opm_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = $value:expr),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vmeta])* $variant,)+
            /// A name this library doesn't know, kept as written.
            Other(String),
        }

        impl $name {
            /// The name as written in `.opm` files.
            pub fn name(&self) -> &str {
                match self {
                    $(Self::$variant => stringify!($variant),)+
                    Self::Other(s) => s,
                }
            }

            /// The engine's number for it, where known.
            pub fn value(&self) -> Option<i32> {
                match self {
                    $(Self::$variant => $value,)+
                    Self::Other(_) => None,
                }
            }

            /// Reads a name, ignoring case.
            pub fn from_name(s: &str) -> Self {
                $(if s.eq_ignore_ascii_case(stringify!($variant)) {
                    return Self::$variant;
                })+
                Self::Other(s.to_string())
            }
        }

        /// The first listed value: the one a .NET enum defaults to (its 0), or the editor's default.
        impl Default for $name {
            fn default() -> Self {
                [$(Self::$variant),+].into_iter().next().expect("at least one variant")
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.name())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                // A number where a name belongs (some writers use the value) is kept as its text.
                match serde_json::Value::deserialize(d)? {
                    serde_json::Value::String(s) => Ok(Self::from_name(&s)),
                    other => Ok(Self::Other(other.to_string())),
                }
            }
        }
    };
}

opm_enum! {
    /// The game mode (`LevelDetails.MissionType`); the values are the engine's.
    MissionType {
        Colony = Some(-8),
        MultiLastOneStanding = Some(-1),
        MultiMidas = Some(-2),
        MultiResourceRace = Some(-3),
        MultiSpaceRace = Some(-4),
        MultiLandRush = Some(-5),
        Tutorial = Some(-6),
        AutoDemo = Some(-7),
    }
}

impl MissionType {
    /// The prefix OP2MissionEditor gives the file name, e.g. "ml2" for a 2-player Last One Standing, "c" for a
    /// colony game.
    pub fn file_prefix(&self, players: i32) -> String {
        match self {
            Self::MultiLastOneStanding => format!("ml{players}"),
            Self::MultiMidas => format!("mm{players}"),
            Self::MultiResourceRace => format!("mr{players}"),
            Self::MultiSpaceRace => format!("mf{players}"),
            Self::MultiLandRush => format!("mu{players}"),
            Self::Tutorial => "t".into(),
            Self::AutoDemo => "a".into(),
            Self::Colony | Self::Other(_) => "c".into(),
        }
    }
}

opm_enum! {
    /// A player's colour; the values are the engine's colour numbers.
    Color {
        Blue = Some(0),
        Red = Some(1),
        Green = Some(2),
        Yellow = Some(3),
        Cyan = Some(4),
        Magenta = Some(5),
        Black = Some(6),
        White = Some(7),
    }
}

opm_enum! {
    /// An AI player's behaviour preset. `None` for humans.
    BotType {
        None = None,
        Balanced = None,
        LaunchStarship = None,
    }
}

opm_enum! {
    /// A starting morale level.
    MoraleLevel {
        Good = None,
        Excellent = None,
        Fair = None,
        Poor = None,
        Terrible = None,
    }
}

opm_enum! {
    /// A vehicle's facing; the values are the engine's facing index (0 east, clockwise).
    Direction {
        East = Some(0),
        SouthEast = Some(1),
        South = Some(2),
        SouthWest = Some(3),
        West = Some(4),
        NorthWest = Some(5),
        North = Some(6),
        NorthEast = Some(7),
    }
}

opm_enum! {
    /// A beacon's kind (`Beacon.MapID`).
    BeaconKind {
        MiningBeacon = None,
        Fumarole = None,
        MagmaVent = None,
    }
}

opm_enum! {
    /// A mining beacon's ore.
    OreType {
        Common = Some(0),
        Random = Some(-1),
        Rare = Some(1),
    }
}

opm_enum! {
    /// Ore yield per truckload of a beacon or mine.
    BarYield {
        Bar3 = Some(0),
        Random = Some(-1),
        Bar2 = Some(1),
        Bar1 = Some(2),
    }
}

opm_enum! {
    /// A beacon's or mine's graphic and yield-decay variant.
    BarVariant {
        Variant1 = Some(0),
        Random = Some(-1),
        Variant2 = Some(1),
        Variant3 = Some(2),
    }
}

opm_enum! {
    /// A map marker's graphic.
    MarkerType {
        Circle = None,
        Beaker = None,
        DNA = None,
        Common = None,
        Rare = None,
    }
}

opm_enum! {
    /// What a cargo truck carries (`Unit.CargoType` on a `CargoTruck`): the SDK's `TruckCargo` names. Note `Garbage`
    /// is wreckage and `Spaceport` a starship module (its id in `CargoAmount`); a gene bank has no name and comes as
    /// the number 90 (see [`crate::Cargo`]).
    TruckCargo {
        None = Some(0),
        Food = Some(1),
        CommonOre = Some(2),
        RareOre = Some(3),
        CommonMetal = Some(4),
        RareMetal = Some(5),
        CommonRubble = Some(6),
        RareRubble = Some(7),
        Spaceport = Some(8),
        Garbage = Some(9),
    }
}

opm_enum! {
    /// A combat unit's weapon (`Unit.CargoType` on a Lynx, Panther, Tiger, Spider, Scorpion or Guard Post).
    Weapon {
        Laser = None,
        Microwave = None,
        EMP = None,
        RPG = None,
        Starflare = None,
        Supernova = None,
        ESG = None,
        AcidCloud = None,
        RailGun = None,
        Stickyfoam = None,
        ThorsHammer = None,
    }
}
