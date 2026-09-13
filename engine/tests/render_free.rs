#[test]
fn engine_manifest_does_not_name_renderer_or_ui_crates() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = std::fs::read_to_string(manifest_path)
        .expect("engine Cargo.toml should be readable")
        .to_lowercase();

    let forbidden = [
        "raylib",
        "macroquad",
        "miniquad",
        "bevy",
        "tauri",
        "winit",
        "wgpu",
        "glium",
        "sdl2",
        "ggez",
        "pixels",
        "egui",
        "iced",
    ];

    for name in forbidden {
        assert!(
            !manifest.contains(name),
            "engine crate must stay render-free; found forbidden dependency marker `{name}`"
        );
    }
}
