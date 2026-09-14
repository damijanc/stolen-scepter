#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Terrain {
    Grass = 0,
    Forest = 1,
    Mountain = 2,
    Water = 3,
    Desert = 4,
}

impl Default for Terrain {
    fn default() -> Self {
        Self::Grass
    }
}

impl Terrain {
    pub fn from_name(name: &str) -> Self {
        match name {
            "forest" => Self::Forest,
            "mountain" => Self::Mountain,
            "water" => Self::Water,
            "desert" => Self::Desert,
            _ => Self::Grass,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grass => "grass",
            Self::Forest => "forest",
            Self::Mountain => "mountain",
            Self::Water => "water",
            Self::Desert => "desert",
        }
    }

    pub fn walkable(self) -> bool {
        matches!(self, Self::Grass | Self::Desert)
    }

    pub fn move_cost(self) -> i32 {
        match self {
            Self::Grass => 1,
            Self::Desert => 40,
            Self::Forest | Self::Mountain | Self::Water => 0,
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::Terrain;

    #[test]
    fn terrain_walkable_and_move_cost_match_openbounty() {
        let cases = [
            (Terrain::Grass, true, 1),
            (Terrain::Desert, true, 40),
            (Terrain::Forest, false, 0),
            (Terrain::Mountain, false, 0),
            (Terrain::Water, false, 0),
        ];

        for (terrain, walkable, move_cost) in cases {
            assert_eq!(terrain.walkable(), walkable);
            assert_eq!(terrain.move_cost(), move_cost);
        }
    }
}
