//! Plays `touch_only.txt` through the real `playtest` binary as the mobile
//! build (`--mobile`): a game started, a castle visited, a fight moved,
//! shot, cancelled and given up, the army managed and the game quit, all
//! with left clicks alone, as a phone plays it. The script must stay
//! click-only, which this test also checks.

use std::path::Path;
use std::process::Command;

/// Run the script on the mobile build with `extra` arguments; true when it
/// played through.
fn play(extra: &[&str], tag: &str) -> bool {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = dir.join("tests/touch_only.txt");
    let out = std::env::temp_dir().join(format!(
        "openbounty-touch-only-{tag}-{}",
        std::process::id()
    ));
    let status = Command::new(env!("CARGO_BIN_EXE_playtest"))
        .arg("--script")
        .arg(&script)
        .arg("--out")
        .arg(&out)
        .arg("--save-dir")
        .arg(out.join("saves"))
        .args(["--seed", "7", "--mobile"])
        .args(extra)
        .current_dir(dir.join(".."))
        .status()
        .expect("playtest runs");
    let _ = std::fs::remove_dir_all(&out);
    status.success()
}

#[test]
fn a_game_plays_by_touch_alone() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(dir.join("tests/touch_only.txt")).expect("script reads");
    for line in text.lines().map(str::trim) {
        let command = line.split_whitespace().next().unwrap_or("");
        assert!(
            matches!(
                command,
                "" | "click" | "wait" | "expect" | "shot" | "text" | "state" | "point"
            ) || line.starts_with('#'),
            "not a touch command: {line:?}"
        );
    }
    assert!(play(&[], "play"), "touch-only playthrough failed");
}

/// Every tap region any frame of the playthrough publishes is at least
/// `sheet::MIN_BUTTON_H` (26 logical px, about 7.6 mm on a Pixel 9a) tall,
/// apart from the map tiles, combat cells and sidebar panels. A full-screen
/// backdrop passes as the large region it is. The playtest's `--audit-taps`
/// names any offender and the script line that showed it.
#[test]
fn every_mobile_tap_region_is_finger_sized() {
    assert!(
        play(&["--audit-taps"], "audit"),
        "a tap region is under a finger's size"
    );
}
