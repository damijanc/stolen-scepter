//! Plays `touch_only.txt` through the real `playtest` binary: a game started,
//! fought, managed and quit with left clicks alone, as a phone plays it. The
//! script must stay click-only, which this test also checks.

use std::path::Path;
use std::process::Command;

#[test]
fn a_game_plays_by_touch_alone() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = dir.join("tests/touch_only.txt");
    let text = std::fs::read_to_string(&script).expect("script reads");
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

    let out = std::env::temp_dir().join("openbounty-touch-only");
    let status = Command::new(env!("CARGO_BIN_EXE_playtest"))
        .arg("--script")
        .arg(&script)
        .arg("--out")
        .arg(&out)
        .arg("--save-dir")
        .arg(out.join("saves"))
        .args(["--seed", "7"])
        .current_dir(dir.join(".."))
        .status()
        .expect("playtest runs");
    assert!(status.success(), "touch-only playthrough failed");
}
