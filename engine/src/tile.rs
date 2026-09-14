#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interact {
    None = 0,
    CastleGate = 1,
    Town = 2,
    TreasureChest = 3,
    Sign = 4,
    Artifact = 5,
    DwellingPlains = 6,
    DwellingForest = 7,
    DwellingHills = 8,
    DwellingDungeon = 9,
    Alcove = 10,
    Orb = 11,
    Telecave = 12,
    Navmap = 13,
    Foe = 14,
}

impl Default for Interact {
    fn default() -> Self {
        Self::None
    }
}

impl Interact {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CastleGate => "castle_gate",
            Self::Town => "town",
            Self::TreasureChest => "treasure_chest",
            Self::Sign => "sign",
            Self::Artifact => "artifact",
            Self::DwellingPlains => "dwelling_plains",
            Self::DwellingForest => "dwelling_forest",
            Self::DwellingHills => "dwelling_hills",
            Self::DwellingDungeon => "dwelling_dungeon",
            Self::Orb => "orb",
            Self::Telecave => "telecave",
            Self::Navmap => "navmap",
            Self::Foe => "foe",
            Self::None | Self::Alcove => "none",
        }
    }

    pub fn from_dwelling_kind(kind: &str) -> Self {
        match kind {
            "forest" => Self::DwellingForest,
            "hills" | "hill" => Self::DwellingHills,
            "dungeon" => Self::DwellingDungeon,
            _ => Self::DwellingPlains,
        }
    }
}
