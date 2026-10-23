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
        present.window_to_logical(layout, win_w, win_h, present::Target::Desktop, px, py)
    })
}

/// Draw the shell's own pointer last, over everything, and hide the OS arrow
/// exactly where it shows. See `cursor.rs` for why it exists.
fn finish_pointer(list: &mut DrawList, backend: &Backend, at: Option<(i32, i32)>) {
    cursor::push_pointer(list, at);
    backend.set_os_cursor_visible(at.is_none());
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

#[macroquad::main(window_conf)]
async fn main() {
    let launch = LaunchArgs::parse(std::env::args().skip(1));
    let save_paths = launch.save_paths();
    let LaunchArgs {
        explicit_pack,
        seed_index,
        ..
    } = launch;

    // C's launch policy (`main.c`): an explicit pack must resolve; otherwise
    // none found is fatal, one loads silently, and two or more get a picker.
    // Whether the pack came from the picker, i.e. was unknown when the
    // window opened.
    let mut picked = false;
    let pack_path = match explicit_pack {
        Some(arg) => match pack::resolve_arg(&arg, &save_paths) {
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
            let entries = pack::discover(&save_paths);
            match entries.len() {
                0 => {
                    eprintln!("OpenBounty cannot start because no game pack was found. Looked in:");
                    for root in pack::discover_roots(&save_paths) {
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
                    None => return,
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
            return;
        }
    };
    let resources = match pack::load_resources(&pack) {
        Ok(resources) => resources,
        Err(err) => {
            eprintln!("could not load pack {pack_path}: {err}");
            return;
        }
    };
    let layout = match Layout::from_resources(&resources) {
        Ok(layout) => layout,
        Err(err) => {
            eprintln!("pack {pack_path} has no usable render geometry: {err:?}");
            return;
        }
    };

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
    // Open the device and load the pack's audio, porting `main.c`'s
    // `audio_init` block. A machine with no device plays nothing and grays
    // the audio rows. Files come through the pack, as C's `LoadAssetBytes`
    // does, so a `.openbounty` archive plays as a directory does.
    let audio = KiraAudio::new(&resources.audio, |path| {
        pack.read(path).map(|bytes| bytes.into_owned())
    });
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
        finish_pointer(&mut list, &backend, pointer_at);
        backend.execute(&list, &catalog, &layout, &session.present);
        next_frame().await;
    }
}
