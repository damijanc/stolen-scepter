//! OpenBounty Rust shell. See `specs/shell-renderer-plan.md` for the agreed
//! architecture and work order.
//!
//! Run it with no argument to use the installed pack (a picker appears when
//! there is more than one), or name one:
//!
//! ```text
//! cargo run -p openbounty-shell
//! cargo run -p openbounty-shell -- --pack <name | path to .openbounty | pack dir>
//! cargo run -p openbounty-shell -- path/to/game.json
//! ```

use openbounty_shell::*;

use macroquad::prelude::{get_time, next_frame, request_new_screen_size, Conf};

use audio_backend::KiraAudio;
use backend::{Backend, MacroquadKeys, MacroquadPointer};
use draw::DrawList;
use layout::Layout;
use pointer::{Button, HitList, PointerSource};
use present::Present;
use savepath::SavePaths;
use session::{Context, FrameInput, FrameStatus, Session};
use sprites::SpriteCatalog;
use startup::{Startup, StartupAction};

/// The pointer in LOGICAL coordinates (the `DrawList` space), or `None` over
/// the letterbox or outside the window. The one place window points become
/// logical pixels for both hit-testing and the drawn pointer.
fn logical_pointer(
    pointer: &impl PointerSource,
    backend: &Backend,
    present: &Present,
    layout: &Layout,
) -> Option<(i32, i32)> {
    let (win_w, win_h) = backend.window_size();
    pointer.position().and_then(|(px, py)| {
        present.window_to_logical(layout, win_w, win_h, present::Target::native(), px, py)
    })
}

/// Draw the shell's own pointer last, over everything, and hide the OS arrow
/// exactly where it shows. See `cursor.rs` for why it exists.
fn finish_pointer(list: &mut DrawList, backend: &Backend, at: Option<(i32, i32)>) {
    if draws_pointer() {
        cursor::push_pointer(list, at);
    }
    backend.set_os_cursor_visible(at.is_none());
}

/// A touch screen has no pointer to show: the drawn arrow would sit wherever
/// the last finger lifted.
fn draws_pointer() -> bool {
    present::Target::native() != present::Target::Mobile
}

/// The command line, read by both `window_conf` and `main`.
struct LaunchArgs {
    explicit_pack: Option<String>,
    seed_index: i32,
    save_dir: Option<String>,
}

impl LaunchArgs {
    fn parse(mut args: impl Iterator<Item = String>) -> Self {
        // `--pack <arg>` is C's flag; a bare positional is the older spelling
        // of the same thing, kept so every `.../game.json` invocation still runs.
        let mut launch = Self {
            explicit_pack: None,
            seed_index: -1,
            save_dir: None,
        };
        while let Some(arg) = args.next() {
            if arg == "--seed" {
                launch.seed_index = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(-1);
            } else if arg == "--save-dir" {
                launch.save_dir = args.next();
            } else if arg == "--pack" {
                launch.explicit_pack = args.next();
            } else if launch.explicit_pack.is_none() {
                launch.explicit_pack = Some(arg);
            }
        }
        launch
    }

    fn save_paths(&self) -> SavePaths {
        match &self.save_dir {
            Some(dir) => SavePaths::with_override(dir),
            None => SavePaths::new(),
        }
    }
}

fn window_conf() -> Conf {
    // Title and size come from the pack that will load: the window cannot
    // be retitled once open, and growing it later leaves macOS drawing for
    // the old size until the user resizes it. Only a picker launch, where the
    // pack is not known yet, opens at the legacy default and grows after.
    let launch = LaunchArgs::parse(std::env::args().skip(1));
    let window = pack::launch_window(launch.explicit_pack.as_deref(), &launch.save_paths());
    let (width, height) = window.size.unwrap_or((640, 400));
    Conf {
        window_title: window.title,
        window_width: width,
        window_height: height,
        high_dpi: true,
        ..Default::default()
    }
}

/// The pack to play, its display path, and whether the picker chose it.
/// `None` ends the launch quietly (ESC in the picker, or an unreadable pack).
#[cfg(not(target_os = "android"))]
async fn open_pack(
    explicit_pack: Option<String>,
    save_paths: &SavePaths,
) -> Option<(pack::Pack, String, bool)> {
    // C's launch policy (`main.c`): an explicit pack must resolve; otherwise
    // none found is fatal, one loads silently, and two or more get a picker.
    // Whether the pack came from the picker, i.e. was unknown when the
    // window opened.
    let mut picked = false;
    let pack_path = match explicit_pack {
        Some(arg) => match pack::resolve_arg(&arg, save_paths) {
            Some(path) => path,
            None => {
                eprintln!("Could not find a game pack at:\n\n    {arg}\n");
                eprintln!(
                    "Pass a path to a *.openbounty file, or to a directory containing game.json."
                );
                std::process::exit(1);
            }
        },
        None => {
            let entries = pack::discover(save_paths);
            match entries.len() {
                0 => {
                    eprintln!("OpenBounty cannot start because no game pack was found. Looked in:");
                    for root in pack::discover_roots(save_paths) {
                        // Scanning uses C's "."; the message names it.
                        let shown = std::fs::canonicalize(&root).unwrap_or(root);
                        eprintln!("    {}", shown.display());
                    }
                    eprintln!(
                        "Place a *.openbounty file, or a directory containing game.json, in one of \
                         them, or pass --pack <path>."
                    );
                    std::process::exit(1);
                }
                1 => entries.into_iter().next().expect("one entry").path,
                _ => match pack_select::run(&entries).await {
                    Some(index) => {
                        picked = true;
                        entries.into_iter().nth(index).expect("in range").path
                    }
                    // ESC: load nothing. C returns 0 here.
                    None => return None,
                },
            }
        }
    };
    let pack_path = pack_path.display().to_string();

    let pack_location = pack::pack_location(std::path::Path::new(&pack_path));
    let pack = match pack::Pack::open(&pack_location) {
        Ok(pack) => pack,
        Err(err) => {
            eprintln!(
                "cannot open game pack at {}: {err}",
                pack_location.display()
            );
            return None;
        }
    };
    Some((pack, pack_path, picked))
}

/// The APK's one bundled pack. An APK asset has no path, so this is the
/// archive's bytes; discovery, `--pack` and the picker do not apply.
#[cfg(target_os = "android")]
async fn open_pack(
    _explicit_pack: Option<String>,
    _save_paths: &SavePaths,
) -> Option<(pack::Pack, String, bool)> {
    const BUNDLED: &str = "game.openbounty";
    let bytes = backend::load_bundled(BUNDLED).await;
    let pack = bytes.and_then(|raw| pack::Pack::from_archive_bytes(&raw).ok());
    pack.map(|pack| (pack, BUNDLED.to_owned(), false))
}

/// miniquad's Java activity starts the game by calling this symbol in the
/// loaded library. `cargo quad-apk` injects the same two lines; we build the
/// library with plain cargo (tools/build_apk.sh), so it lives here.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn quad_main() {
    // miniquad runs the game on a `thread::spawn` thread, which gets Rust's
    // 2 MB default stack; the desktop main thread has 8 MB, and the engine's
    // fixed-capacity structs need it. Read once, at the first spawn, which
    // has not happened yet.
    std::env::set_var("RUST_MIN_STACK", (16 * 1024 * 1024).to_string());
    main();
}

#[macroquad::main(window_conf)]
async fn main() {
    run().await;
    // miniquad never finishes the Android activity, so a quit would leave a
    // black, still-running app behind. Ask Android to close it.
    #[cfg(target_os = "android")]
    backend::android_finish();
}

async fn run() {
    let launch = LaunchArgs::parse(std::env::args().skip(1));
    let save_paths = launch.save_paths();
    let LaunchArgs {
        explicit_pack,
        seed_index,
        ..
    } = launch;

    #[cfg(target_os = "android")]
    backend::android_init();
    let Some((pack, pack_path, picked)) = open_pack(explicit_pack, &save_paths).await else {
        return;
    };
    let resources = match pack::load_resources(&pack) {
        Ok(resources) => resources,
        Err(err) => {
            eprintln!("could not load pack {pack_path}: {err}");
            return;
        }
    };
    let mut layout = match Layout::from_resources(&resources) {
        Ok(layout) => layout,
        Err(err) => {
            eprintln!("pack {pack_path} has no usable render geometry: {err:?}");
            return;
        }
    };
    // The touch build draws the mobile UI (specs/mobile-ui-design.md).
    layout.touch = present::Target::native() == present::Target::Mobile;

    // A picker launch opened the window at the legacy default before the
    // pack was known; grow it to what this pack wants, or an HD (or modern)
    // pack renders larger than its window and gets cropped. Every other
    // launch already opened at this size (`window_conf`), and asking again
    // is not harmless: macOS then keeps the stale size until a manual resize.
    if picked {
        request_new_screen_size(layout.window_w() as f32, layout.window_h() as f32);
    }

    let catalog = SpriteCatalog::from_resources(&resources);
    let backend = Backend::load(&resources, &catalog, &pack);
    let present = Present::for_layout(&layout);

    println!(
        "loaded {} textures for a {}x{} {} screen",
        backend.texture_count(),
        layout.screen_w,
        layout.screen_h,
        if layout.is_modern { "modern" } else { "legacy" }
    );
    if !backend.missing().is_empty() {
        eprintln!("missing art: {:?}", backend.missing());
    }

    let mut list = DrawList::new();
    // The wizard's own regions; the session keeps the game's.
    let mut hits = HitList::new();
    let keys = MacroquadKeys;
    let pointer = MacroquadPointer;

    // Open the device and load the pack's audio, porting `main.c`'s
    // `audio_init` block. A machine with no device plays nothing and grays
    // the audio rows. Files come through the pack, as C's `LoadAssetBytes`
    // does, so a `.openbounty` archive plays as a directory does. It opens
    // before the wizard so the wizard's taps tick on mobile; until a game
    // exists, the pack's default Sounds and Volume options govern them.
    let mut audio = KiraAudio::new(&resources.audio, |path| {
        pack.read(path).map(|bytes| bytes.into_owned())
    });
    let defaults = &resources.world.default_options;
    audio.set_sounds_enabled(defaults[audio::OPTION_SOUNDS] != 0);
    audio.set_master_volume(defaults[audio::OPTION_VOLUME]);

    // Pre-game wizard: class select, then name/difficulty. It blocks on the
    // player, exactly as C's `startup_flow` does before the game exists.
    let mut startup = Startup::new(&resources);
    let choice = loop {
        // Same shape as the main loop: hit-test the previous frame's regions,
        // and a click resolves to what the matching key does.
        let at = logical_pointer(&pointer, &backend, &present, &layout);
        let hovered = at.and_then(|(lx, ly)| hits.hit(lx, ly));
        startup.set_hover(hovered);
        let clicked = hovered.filter(|_| pointer.pressed(Button::Left));
        if layout.touch && clicked.is_some_and(audio::tap_sounds) {
            audio.play_tune(audio::Tune::Tap);
        }
        let outcome = match clicked {
            Some(target) => startup.click(target, &resources, &save_paths, get_time()),
            None => startup.update(&keys, &resources, &save_paths, get_time()),
        };
        if let Some(choice) = outcome {
            break choice;
        }
        list.clear();
        hits.clear();
        startup.draw(
            &mut list,
            &mut hits,
            &layout,
            backend.palette(),
            &resources,
            get_time(),
        );
        finish_pointer(&mut list, &backend, at);
        backend.execute(&list, &catalog, &layout, &present);
        next_frame().await;
    };
    // The wizard's regions must not be hit-tested by the game's first frame.
    hits.clear();
    if choice.action == StartupAction::Quit {
        return;
    }

    let ctx = Context {
        resources: &resources,
        layout: &layout,
        catalog: &catalog,
        palette: backend.palette(),
        save_paths: &save_paths,
    };
    let mut session = match Session::start(ctx, present, Box::new(audio), &choice, seed_index) {
        Ok(session) => session,
        Err(err) => {
            eprintln!("{err}");
            return;
        }
    };

    loop {
        let pointer_at = logical_pointer(&pointer, &backend, &session.present, &layout);
        let input = FrameInput {
            keys: &keys,
            pointer_at,
            left: pointer.pressed(Button::Left),
            right: pointer.pressed(Button::Right),
        };
        if session.frame(&input, get_time(), &mut list) == FrameStatus::Quit {
            return;
        }
        if draws_pointer() {
            cursor::push_shape(&mut list, pointer_at, session.pointer_shape());
        }
        backend.set_os_cursor_visible(pointer_at.is_none());
        backend.execute(&list, &catalog, &layout, &session.present);
        next_frame().await;
    }
}
