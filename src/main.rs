mod framework;

mod gui;
use gui::GuiProgram;

mod text;
mod ui;
mod files;

#[allow(dead_code)]
fn main() {
    framework::run("Test");
}

// Examples:
// https://github.com/gfx-rs/wgpu-rs/blob/v0.5/examples/framework.rs
// https://github.com/gfx-rs/wgpu-rs/blob/v0.5/Cargo.toml