use crate::game::Game;

pub const SAVE_VERSION: u32 = 11;

pub fn build_snapshot(game: Option<&Game>) -> serde_json::Value {
    let mut root = serde_json::Map::new();
    root.insert("version".to_owned(), serde_json::json!(SAVE_VERSION));

    let Some(game) = game else {
        root.insert("mode".to_owned(), serde_json::json!("unattached"));
        return serde_json::Value::Object(root);
    };

    root.insert("mode".to_owned(), serde_json::json!("adventure"));
    root.insert(
        "seed_from_catalog".to_owned(),
        serde_json::json!(game.seed_from_catalog),
    );
    if game.seed_from_catalog {
        root.insert("seed_index".to_owned(), serde_json::json!(game.seed_index));
    } else {
        root.insert("seed".to_owned(), serde_json::json!(game.seed));
    }

    serde_json::Value::Object(root)
}

pub fn snapshot_to_string(game: Option<&Game>) -> String {
    serde_json::to_string(&build_snapshot(game)).expect("snapshot value serializes")
}

#[cfg(test)]
mod tests {
    use super::{build_snapshot, snapshot_to_string, SAVE_VERSION};
    use crate::game::Game;

    #[test]
    fn catalog_seed_snapshot_uses_openbounty_seed_identity_fields() {
        let mut game = Game::default();
        game.init_seeded(Some("Hero"), 0, 0, 42);

        assert_eq!(
            build_snapshot(Some(&game)),
            serde_json::json!({
                "version": SAVE_VERSION,
                "mode": "adventure",
                "seed_from_catalog": true,
                "seed_index": 42
            })
        );
    }

    #[test]
    fn raw_seed_snapshot_writes_seed_instead_of_catalog_index() {
        let mut game = Game::with_raw_seed(12_345);
        game.init_seeded(Some("Hero"), 0, 0, -1);

        assert_eq!(
            build_snapshot(Some(&game)),
            serde_json::json!({
                "version": SAVE_VERSION,
                "mode": "adventure",
                "seed_from_catalog": false,
                "seed": 12_345_u64
            })
        );
    }

    #[test]
    fn unattached_snapshot_matches_openbounty_mode() {
        assert_eq!(
            build_snapshot(None),
            serde_json::json!({
                "version": SAVE_VERSION,
                "mode": "unattached"
            })
        );
    }

    #[test]
    fn snapshot_string_is_compact_json() {
        let mut game = Game::default();
        game.init_seeded(Some("Hero"), 0, 0, 1);

        assert_eq!(
            snapshot_to_string(Some(&game)),
            r#"{"mode":"adventure","seed_from_catalog":true,"seed_index":1,"version":11}"#
        );
    }
}
