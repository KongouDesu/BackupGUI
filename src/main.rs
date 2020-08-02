mod framework;

mod gui;
use gui::GuiProgram;

mod text;
mod ui;
mod files;

fn main() {
    framework::run("Backup GUI");
}