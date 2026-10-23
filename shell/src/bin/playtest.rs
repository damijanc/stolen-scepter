//! Headless playtest: run the real game on a script and save what it shows.
//!
//! The same `Startup` wizard and `Session` frame the windowed game runs are
//! driven here by scripted input on a fixed 60 fps clock, and any frame can
//! be painted to a PNG by `raster.rs`, with the text it drew beside it. Saves
//! go to a directory of their own, never the player's.
//!
//! ```text
//! cargo run -p openbounty-shell --bin playtest -- --script s.txt --out shots/
//!     [--pack <name|path>] [--seed N] [--scale S] [--save-dir DIR]
//! ```
//!
//! `--script -` reads stdin. One command per line, `#` starts a comment:
//!
//! ```text
//! key Enter            press keys in one frame (`key Up Right` is a chord);
//!                      a letter, digit or Space also types its character
//! hold Up / release Up keep a key down across frames (`down` state)
//! type Sir Hugo        type text, one character per frame
//! click X Y            left click at logical pixel X,Y (the pointer moves there)
//! rclick X Y           right click
//! point X Y            move the pointer without clicking
//! wait N | wait 1.5s   let N frames, or that many seconds, pass
//! shot NAME            paint the current frame to NAME.png, its text to NAME.txt
//! text                 print the current frame's text
//! expect WORDS         fail unless the current frame's text contains WORDS
//! state                print day, gold, position and army
//! ```
//!
//! Every input command is followed by one idle frame, so a key press is a
//! single rising edge exactly as on a keyboard.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use openbounty_shell::audio::Silent;
use openbounty_shell::cursor;
use openbounty_shell::draw::DrawList;
use openbounty_shell::input::{Key, KeySource};
use openbounty_shell::layout::Layout;
use openbounty_shell::pack;
use openbounty_shell::palette::Palette;
use openbounty_shell::pointer::{Button, HitList};
use openbounty_shell::present::Present;
use openbounty_shell::raster::{self, Canvas, Images};
use openbounty_shell::savepath::SavePaths;
use openbounty_shell::session::{Context, FrameInput, FrameStatus, Session};
use openbounty_shell::sprites::SpriteCatalog;
use openbounty_shell::startup::{Startup, StartupAction};

const FPS: f64 = 60.0;

struct Args {
    script: String,
    out: PathBuf,
    pack: Option<String>,
    seed: i32,
    scale: i32,
    save_dir: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        script: String::new(),
        out: PathBuf::from("playtest-out"),
        pack: None,
        seed: 1,
        scale: 2,
        save_dir: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--script" => args.script = value()?,
            "--out" => args.out = PathBuf::from(value()?),
            "--pack" => args.pack = Some(value()?),
            "--seed" => args.seed = value()?.parse().map_err(|_| "bad --seed")?,
            "--scale" => args.scale = value()?.parse().map_err(|_| "bad --scale")?,
            "--save-dir" => args.save_dir = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if args.script.is_empty() {
        return Err("--script FILE (or -) is required".into());
    }
    Ok(args)
}

/// One frame of scripted key state.
#[derive(Default)]
struct ScriptKeys {
    pressed: BTreeSet<Key>,
    down: BTreeSet<Key>,
    chars: Vec<char>,
}

impl KeySource for ScriptKeys {
    fn pressed(&self, key: Key) -> bool {
        self.pressed.contains(&key)
    }
    fn down(&self, key: Key) -> bool {
        self.down.contains(&key) || self.pressed.contains(&key)
    }
    fn any_pressed(&self) -> bool {
        self.pressed
            .iter()
            .any(|k| !matches!(k, Key::LeftControl | Key::RightControl))
    }
    fn chars_pressed(&self) -> Vec<char> {
        self.chars.clone()
    }
}

/// What one frame receives.
#[derive(Default)]
struct Input {
    keys: ScriptKeys,
    left: bool,
    right: bool,
}

/// The startup wizard, then the game.
enum Phase<'r> {
    Startup(Startup),
    Game(Box<Session<'r>>),
    Over,
}

struct Runner<'r> {
    ctx: Context<'r>,
    seed: i32,
    phase: Phase<'r>,
    hits: HitList,
    list: DrawList,
    frame: u64,
    held: BTreeSet<Key>,
    pointer: Option<(i32, i32)>,
}

impl<'r> Runner<'r> {
    fn now(&self) -> f64 {
        self.frame as f64 / FPS
    }

    fn step(&mut self, mut input: Input) -> Result<(), String> {
        input.keys.down.extend(self.held.iter().copied());
        let now = self.now();
        self.frame += 1;
        let ctx = self.ctx;
        match &mut self.phase {
            Phase::Startup(startup) => {
                let hovered = self.pointer.and_then(|(x, y)| self.hits.hit(x, y));
                startup.set_hover(hovered);
                let clicked = hovered.filter(|_| input.left);
                let outcome = match clicked {
                    Some(target) => startup.click(target, ctx.resources, ctx.save_paths, now),
                    None => startup.update(&input.keys, ctx.resources, ctx.save_paths, now),
                };
                if let Some(choice) = outcome {
                    self.hits.clear();
                    if choice.action == StartupAction::Quit {
                        self.phase = Phase::Over;
                        return Ok(());
                    }
                    let session = Session::start(
                        ctx,
                        Present::for_layout(ctx.layout),
                        Box::new(Silent),
                        &choice,
                        self.seed,
                    )?;
                    self.phase = Phase::Game(Box::new(session));
                    return self.step(Input::default());
                }
                self.list.clear();
                self.hits.clear();
                startup.draw(
                    &mut self.list,
                    &mut self.hits,
                    ctx.layout,
                    ctx.palette,
                    ctx.resources,
                    now,
                );
            }
            Phase::Game(session) => {
                let frame_in = FrameInput {
                    keys: &input.keys,
                    pointer_at: self.pointer,
                    left: input.left,
                    right: input.right,
                };
                if session.frame(&frame_in, now, &mut self.list) == FrameStatus::Quit {
                    self.phase = Phase::Over;
                }
            }
            Phase::Over => return Err("the game has quit".into()),
        }
        Ok(())
    }

    /// An input frame, then an idle one so the press is a single edge.
    fn press(&mut self, input: Input) -> Result<(), String> {
        self.step(input)?;
        self.step(Input::default())
    }

    fn texts(&self) -> Vec<String> {
        raster::texts(&self.list)
    }
}

fn parse_keys(words: &[&str]) -> Result<Vec<Key>, String> {
    words
        .iter()
        .map(|w| Key::from_name(w).ok_or(format!("unknown key {w}")))
        .collect()
}

fn parse_xy(words: &[&str]) -> Result<(i32, i32), String> {
    match words {
        [x, y] => Ok((
            x.parse().map_err(|_| format!("bad x {x}"))?,
            y.parse().map_err(|_| format!("bad y {y}"))?,
        )),
        _ => Err("expected X Y".into()),
    }
}

fn shot(
    runner: &Runner,
    images: &Images,
    scale: i32,
    out: &Path,
    name: &str,
) -> Result<(), String> {
    let mut list = runner.list.clone();
    cursor::push_pointer(&mut list, runner.pointer);
    let mut canvas = Canvas::new(
        runner.ctx.layout.screen_w,
        runner.ctx.layout.screen_h,
        scale,
    );
    canvas.paint(&list, runner.ctx.catalog, images);
    let png = out.join(format!("{name}.png"));
    std::fs::write(&png, canvas.to_png()).map_err(|e| format!("{}: {e}", png.display()))?;
    std::fs::write(out.join(format!("{name}.txt")), runner.texts().join("\n"))
        .map_err(|e| e.to_string())?;
    println!("shot {}", png.display());
    Ok(())
}

fn state(runner: &Runner) {
    let Phase::Game(session) = &runner.phase else {
        println!("state: not in a game");
        return;
    };
    let game = &session.game;
    let army: Vec<String> = game
        .army
        .iter()
        .filter(|s| !s.id.is_empty() && s.count > 0)
        .map(|s| format!("{} {}", s.count, s.id.as_str()))
        .collect();
    println!(
        "state: day-left {} gold {} at {} ({},{}) mode {:?} army [{}]",
        game.stats.days_left,
        game.stats.gold,
        game.position.zone.as_str(),
        game.position.x,
        game.position.y,
        game.travel_mode,
        army.join(", ")
    );
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    let save_dir = args
        .save_dir
        .clone()
        .unwrap_or_else(|| args.out.join("saves"));
    std::fs::create_dir_all(&save_dir).map_err(|e| e.to_string())?;
    let save_paths = SavePaths::with_override(&save_dir.display().to_string());

    let pack_path = match &args.pack {
        Some(arg) => pack::resolve_arg(arg, &save_paths).ok_or(format!("no pack at {arg}"))?,
        None => {
            pack::discover(&save_paths)
                .into_iter()
                .next()
                .ok_or("no pack found")?
                .path
        }
    };
    let pack = pack::Pack::open(&pack::pack_location(&pack_path)).map_err(|e| e.to_string())?;
    let resources = pack::load_resources(&pack)?;
    let layout = Layout::from_resources(&resources).map_err(|e| format!("{e:?}"))?;
    let catalog = SpriteCatalog::from_resources(&resources);
    let images = Images::load(&resources, &catalog, &pack);
    if !images.missing.is_empty() {
        eprintln!("missing art: {:?}", images.missing);
    }
    let palette = match catalog.palette_path() {
        "" => Palette::fallback(),
        rel => Palette::from_bin_or_fallback(pack.read(rel).as_deref()),
    };

    let mut script = String::new();
    if args.script == "-" {
        std::io::stdin()
            .read_to_string(&mut script)
            .map_err(|e| e.to_string())?;
    } else {
        script = std::fs::read_to_string(&args.script).map_err(|e| e.to_string())?;
    }

    let ctx = Context {
        resources: &resources,
        layout: &layout,
        catalog: &catalog,
        palette: &palette,
        save_paths: &save_paths,
    };
    let mut runner = Runner {
        ctx,
        seed: args.seed,
        phase: Phase::Startup(Startup::new(&resources)),
        hits: HitList::new(),
        list: DrawList::new(),
        frame: 0,
        held: BTreeSet::new(),
        pointer: None,
    };
    runner.step(Input::default())?;

    for (number, line) in script.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let rest = line[words[0].len()..].trim();
        let fail = |e: String| format!("line {}: `{line}`: {e}", number + 1);
        match words[0] {
            "key" => {
                let keys = parse_keys(&words[1..]).map_err(fail)?;
                let mut input = Input::default();
                input.keys.chars = keys.iter().filter_map(|k| k.typed_char()).collect();
                input.keys.pressed = keys.into_iter().collect();
                runner.press(input).map_err(fail)?;
            }
            "hold" => runner.held.extend(parse_keys(&words[1..]).map_err(fail)?),
            "release" => {
                for key in parse_keys(&words[1..]).map_err(fail)? {
                    runner.held.remove(&key);
                }
            }
            "type" => {
                for ch in rest.chars() {
                    let mut input = Input::default();
                    if let Some(key) = Key::from_name(&ch.to_string()) {
                        input.keys.pressed.insert(key);
                    } else if ch == ' ' {
                        input.keys.pressed.insert(Key::Space);
                    }
                    input.keys.chars.push(ch);
                    runner.press(input).map_err(fail)?;
                }
            }
            "click" | "rclick" => {
                runner.pointer = Some(parse_xy(&words[1..]).map_err(fail)?);
                // One frame to hover, so the click hit-tests regions drawn
                // with the pointer already there, as a real click does.
                runner.step(Input::default()).map_err(fail)?;
                let button = if words[0] == "click" {
                    Button::Left
                } else {
                    Button::Right
                };
                runner
                    .press(Input {
                        left: button == Button::Left,
                        right: button == Button::Right,
                        ..Input::default()
                    })
                    .map_err(fail)?;
            }
            "point" => {
                runner.pointer = Some(parse_xy(&words[1..]).map_err(fail)?);
                runner.step(Input::default()).map_err(fail)?;
            }
            "wait" => {
                let frames = match rest.strip_suffix('s') {
                    Some(secs) => (secs
                        .parse::<f64>()
                        .map_err(|_| fail("bad seconds".into()))?
                        * FPS)
                        .round() as u64,
                    None => rest.parse().map_err(|_| fail("bad frame count".into()))?,
                };
                for _ in 0..frames {
                    runner.step(Input::default()).map_err(fail)?;
                }
            }
            "shot" => shot(&runner, &images, args.scale, &args.out, rest).map_err(fail)?,
            "text" => println!("text: {}", runner.texts().join(" | ")),
            "expect" => {
                let all = runner.texts().join("\n");
                if !all.contains(rest) {
                    return Err(fail(format!("text not on screen; it shows:\n{all}")));
                }
            }
            "state" => state(&runner),
            other => return Err(fail(format!("unknown command {other}"))),
        }
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("playtest: {err}");
        std::process::exit(1);
    }
}
