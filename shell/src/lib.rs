//! The shell as a library: the windowed game (`main.rs`) and the headless
//! playtest runner (`bin/playtest.rs`) share every module.

pub mod anim;
pub mod audio;
pub mod audio_backend;
pub mod backend;
pub mod banner;
pub mod cheats;
pub mod chrome;
pub mod combat;
pub mod cursor;
pub mod dialog;
pub mod draw;
pub mod font;
pub mod hud;
pub mod input;
pub mod interact;
pub mod layout;
pub mod map_render;
pub mod menu;
pub mod pack;
pub mod pack_select;
pub mod palette;
pub mod pointer;
pub mod present;
pub mod prompt;
pub mod raster;
pub mod savepath;
pub mod session;
pub mod slot_picker;
pub mod sprites;
pub mod startup;
pub mod views;
pub mod walk;
