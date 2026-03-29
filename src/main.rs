#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Grid-based wall layout builder for Godot 4.
//!
//! You paint cells on a grid, then export a `.tscn` where filled regions are
//! merged into rectangular segments. Each segment becomes a `StaticBody3D` with
//! a `BoxMesh` and `BoxShape3D`.
//!
//! On Windows, we set the subsystem to `windows` for non-debug builds to avoid
//! spawning a console window when launching the released `.exe`.

mod app;
mod godot_scene;
mod grid;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Grid Wall Builder for Godot",
        options,
        Box::new(|_cc| Box::new(app::App::default())),
    )
}
