use crate::tables::{parse_troop_abilities, TroopDef, CAT_TROOPS_MAX};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resources {
    troops: [TroopDef; CAT_TROOPS_MAX],
    troops_count: usize,
    morale_chart: [[u8; 5]; 5],
}

impl Default for Resources {
    fn default() -> Self {
        Self {
            troops: [TroopDef::default(); CAT_TROOPS_MAX],
            troops_count: 0,
            morale_chart: default_morale_chart(),
        }
    }
}

impl Resources {
    pub fn from_json_file(path: impl AsRef<std::path::Path>) -> Result<Self, ResourceError> {
        let input = std::fs::read_to_string(path).map_err(ResourceError::Io)?;
        Self::from_json_str(&input)
    }

    pub fn from_json_str(input: &str) -> Result<Self, ResourceError> {
        let root: serde_json::Value = serde_json::from_str(input).map_err(ResourceError::Json)?;
        let mut resources = Self::default();
        resources.parse_combat(&root)?;
        resources.parse_troops(&root)?;
        Ok(resources)
    }

    pub fn troops_count(&self) -> usize {
        self.troops_count
    }

    pub fn troop_by_id(&self, id: &str) -> Option<&TroopDef> {
        self.troops[..self.troops_count]
            .iter()
            .find(|troop| troop.id.as_str() == id)
    }

    pub fn troop_by_index(&self, index: i32) -> Option<&TroopDef> {
        if index < 0 || index as usize >= self.troops_count {
            return None;
        }
        self.troops.get(index as usize)
    }

    pub fn morale_result(&self, my_group: u8, their_group: u8) -> u8 {
        let a = my_group.wrapping_sub(b'A');
        let b = their_group.wrapping_sub(b'A');
        if a > 4 || b > 4 {
            return b'N';
        }
        self.morale_chart[a as usize][b as usize]
    }

    fn parse_combat(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(combat) = root.get("combat") else {
            return Ok(());
        };
        let Some(chart) = combat.get("morale_chart") else {
            return Ok(());
        };

        let rows = chart
            .as_array()
            .ok_or(ResourceError::InvalidField("combat.morale_chart"))?;
        if rows.len() != 5 {
            return Err(ResourceError::InvalidField("combat.morale_chart"));
        }

        for (row_idx, row) in rows.iter().enumerate() {
            let cols = row
                .as_array()
                .ok_or(ResourceError::InvalidField("combat.morale_chart row"))?;
            if cols.len() != 5 {
                return Err(ResourceError::InvalidField("combat.morale_chart row"));
            }
            for (col_idx, value) in cols.iter().enumerate() {
                let s = value
                    .as_str()
                    .ok_or(ResourceError::InvalidField("combat.morale_chart value"))?;
                self.morale_chart[row_idx][col_idx] = s.as_bytes().first().copied().unwrap_or(b'N');
            }
        }

        Ok(())
    }

    fn parse_troops(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let troops = root
            .get("troops")
            .ok_or(ResourceError::MissingField("troops"))?
            .as_array()
            .ok_or(ResourceError::InvalidField("troops"))?;

        if troops.len() > CAT_TROOPS_MAX {
            return Err(ResourceError::TooManyTroops {
                count: troops.len(),
                max: CAT_TROOPS_MAX,
            });
        }

        for (slot, value) in troops.iter().enumerate() {
            self.troops[slot] = parse_troop(value)?;
        }
        self.troops_count = troops.len();
        Ok(())
    }
}

#[derive(Debug)]
pub enum ResourceError {
    Io(std::io::Error),
    Json(serde_json::Error),
    MissingField(&'static str),
    InvalidField(&'static str),
    TooManyTroops { count: usize, max: usize },
}

impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "could not read resource file: {err}"),
            Self::Json(err) => write!(f, "invalid JSON: {err}"),
            Self::MissingField(field) => write!(f, "missing required field `{field}`"),
            Self::InvalidField(field) => write!(f, "invalid field `{field}`"),
            Self::TooManyTroops { count, max } => {
                write!(f, "too many troops: got {count}, max {max}")
            }
        }
    }
}

impl std::error::Error for ResourceError {}

fn parse_troop(value: &serde_json::Value) -> Result<TroopDef, ResourceError> {
    let melee = required_i32_array2(value, "melee")?;
    let ranged = required_i32_array3(value, "ranged")?;
    let morale_group = string_field(value, "morale_group")?
        .as_bytes()
        .first()
        .copied()
        .unwrap_or(b'A');

    Ok(TroopDef::new(
        i32_field(value, "index")?,
        string_field(value, "id")?,
        i32_field(value, "skill_level")?,
        i32_field(value, "hit_points")?,
        i32_field(value, "move_rate")?,
        melee[0],
        melee[1],
        ranged[0],
        ranged[1],
        ranged[2],
        i32_field(value, "recruit_cost")?,
        i32_field(value, "spoils_factor")?,
        parse_troop_abilities(string_field(value, "abilities")?),
        morale_group,
    ))
}

fn string_field<'a>(
    value: &'a serde_json::Value,
    key: &'static str,
) -> Result<&'a str, ResourceError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or(ResourceError::InvalidField(key))
}

fn i32_field(value: &serde_json::Value, key: &'static str) -> Result<i32, ResourceError> {
    let n = value
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .ok_or(ResourceError::InvalidField(key))?;
    i32::try_from(n).map_err(|_| ResourceError::InvalidField(key))
}

fn required_i32_array2(
    value: &serde_json::Value,
    key: &'static str,
) -> Result<[i32; 2], ResourceError> {
    let values = value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .ok_or(ResourceError::InvalidField(key))?;
    if values.len() != 2 {
        return Err(ResourceError::InvalidField(key));
    }
    Ok([
        json_array_i32(values, key, 0)?,
        json_array_i32(values, key, 1)?,
    ])
}

fn required_i32_array3(
    value: &serde_json::Value,
    key: &'static str,
) -> Result<[i32; 3], ResourceError> {
    let values = value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .ok_or(ResourceError::InvalidField(key))?;
    if values.len() != 3 {
        return Err(ResourceError::InvalidField(key));
    }
    Ok([
        json_array_i32(values, key, 0)?,
        json_array_i32(values, key, 1)?,
        json_array_i32(values, key, 2)?,
    ])
}

fn json_array_i32(
    values: &[serde_json::Value],
    key: &'static str,
    index: usize,
) -> Result<i32, ResourceError> {
    let n = values[index]
        .as_i64()
        .ok_or(ResourceError::InvalidField(key))?;
    i32::try_from(n).map_err(|_| ResourceError::InvalidField(key))
}

const fn default_morale_chart() -> [[u8; 5]; 5] {
    [*b"NNNNN", *b"NNNNN", *b"NNHNN", *b"LNLHN", *b"LLLNN"]
}

#[cfg(test)]
mod tests {
    use super::Resources;
    use crate::tables::{
        parse_troop_abilities, TROOP_ABIL_ABSORB, TROOP_ABIL_FLY, TROOP_ABIL_LEECH,
        TROOP_ABIL_UNDEAD,
    };

    #[test]
    fn parses_troop_catalog_subset() {
        let resources = Resources::from_json_str(
            r#"{
              "combat": {"morale_chart": [["N","N","N","N","N"],["N","N","N","N","N"],["N","N","H","N","N"],["L","N","L","H","N"],["L","L","L","N","N"]]},
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"},
                {"index":1,"id":"vampires","skill_level":5,"hit_points":30,"move_rate":1,"melee":[3,6],"ranged":[0,0,0],"recruit_cost":1500,"spoils_factor":150,"abilities":"FLY|LEECH|UNDEAD","morale_group":"E"}
              ]
            }"#,
        )
        .expect("fixture should parse");

        assert_eq!(resources.troops_count(), 2);
        let peasants = resources.troop_by_id("peasants").unwrap();
        assert_eq!(peasants.hit_points, 1);
        assert_eq!(peasants.id.as_str(), "peasants");

        let vampires = resources.troop_by_index(1).unwrap();
        assert_eq!(vampires.id.as_str(), "vampires");
        assert_eq!(
            vampires.abilities,
            TROOP_ABIL_FLY | TROOP_ABIL_LEECH | TROOP_ABIL_UNDEAD
        );
    }

    #[test]
    fn ability_parser_matches_openbounty_tokens() {
        assert_eq!(
            parse_troop_abilities("FLY|ABSORB|LEECH|UNDEAD"),
            TROOP_ABIL_FLY | TROOP_ABIL_ABSORB | TROOP_ABIL_LEECH | TROOP_ABIL_UNDEAD
        );
    }
}
