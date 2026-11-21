//! The Android build of the shell: the same `main.rs`, compiled as the shared
//! library miniquad's Java activity loads. See `Cargo.toml` and
//! tools/build_apk.sh.

#[path = "../src/main.rs"]
mod app;
