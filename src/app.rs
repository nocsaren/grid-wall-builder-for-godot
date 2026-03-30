//! egui application for painting a grid-based wall layout.
//!
//! The editor interaction is grid/2D-like (paint cells), but the export is 3D:
//! each merged wall segment becomes a Godot `StaticBody3D` with a `BoxMesh` and
//! `BoxShape3D`. The `z_size` setting controls the thickness of each wall along
//! Godot's Z axis.

use eframe::egui;
use rfd::FileDialog;

use crate::godot_scene::{generate_scene, ExportSettings};
use crate::godot_scene_import::{import_scene, ImportedScene};
use crate::grid::Grid;

pub struct App {
    export: ExportSettings,

    grid_w: usize,
    grid_h: usize,

    grid: Grid,

    paint_value: Option<bool>,
    last_painted_cell: Option<(usize, usize)>,

    name: String,
    output: String,
}

impl Default for App {
    fn default() -> Self {
        let grid_w = 12;
        let grid_h = 6;

        Self {
            export: ExportSettings {
                unit_size: 0.5,
                z_size: 0.1,
            },
            grid_w,
            grid_h,
            grid: Grid::new(grid_w, grid_h),
            paint_value: None,
            last_painted_cell: None,
            name: "GridWall".to_string(),
            output: String::new(),
        }
    }
}

impl App {
    fn apply_imported_scene(&mut self, imported: ImportedScene) {
        self.name = imported.name;
        self.export = imported.export;
        self.grid_w = imported.grid.width();
        self.grid_h = imported.grid.height();
        self.grid = imported.grid;
        self.output = self.generate();
        self.paint_value = None;
        self.last_painted_cell = None;
    }

    fn paint_cell(&mut self, x: usize, y: usize, value: bool) {
        if x < self.grid.width() && y < self.grid.height() {
            self.grid.cells_mut()[x][y] = value;
        }
    }

    fn paint_line(&mut self, from: (usize, usize), to: (usize, usize), value: bool) {
        let (mut x0, mut y0) = (from.0 as isize, from.1 as isize);
        let (x1, y1) = (to.0 as isize, to.1 as isize);

        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.paint_cell(x0 as usize, y0 as usize, value);

            if x0 == x1 && y0 == y1 {
                break;
            }

            let twice_err = 2 * err;
            if twice_err >= dy {
                err += dy;
                x0 += sx;
            }
            if twice_err <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn ensure_grid_size(&mut self) {
        if self.grid.width() != self.grid_w || self.grid.height() != self.grid_h {
            self.grid.resize_preserve(self.grid_w, self.grid_h);
        }
    }

    fn generate(&self) -> String {
        let segments = self.grid.collect_segments();
        generate_scene(
            &self.name,
            self.grid.width(),
            self.grid.height(),
            &self.export,
            &segments,
        )
    }

    fn draw_grid(&mut self, ui: &mut egui::Ui) {
        let cell_size = 30.0;

        let grid_w = self.grid.width();
        let grid_h = self.grid.height();

        let (response, painter) = ui.allocate_painter(
            egui::vec2(grid_w as f32 * cell_size, grid_h as f32 * cell_size),
            egui::Sense::click_and_drag(),
        );

        let rect = response.rect;
        let paint_value = ui.input(|input| {
            if input.pointer.button_down(egui::PointerButton::Primary) {
                Some(true)
            } else if input.pointer.button_down(egui::PointerButton::Secondary) {
                Some(false)
            } else {
                None
            }
        });

        if let Some(paint_value) = paint_value {
            if self.paint_value != Some(paint_value) {
                self.paint_value = Some(paint_value);
                self.last_painted_cell = None;
            }

            if let Some(pos) = response.interact_pointer_pos() {
                if rect.contains(pos) {
                    let x = ((pos.x - rect.left()) / cell_size).floor() as usize;
                    let y = ((pos.y - rect.top()) / cell_size).floor() as usize;

                    if x < grid_w && y < grid_h {
                        let current_cell = (x, y);

                        if let Some(previous_cell) = self.last_painted_cell {
                            self.paint_line(previous_cell, current_cell, paint_value);
                        } else {
                            self.paint_cell(x, y, paint_value);
                        }

                        self.last_painted_cell = Some(current_cell);
                    }
                }
            }
        } else {
            self.paint_value = None;
            self.last_painted_cell = None;
        }

        let cells = self.grid.cells();

        for x in 0..grid_w {
            for y in 0..grid_h {
                let x0 = rect.left() + x as f32 * cell_size;
                let y0 = rect.top() + y as f32 * cell_size;

                let r =
                    egui::Rect::from_min_size(egui::pos2(x0, y0), egui::vec2(cell_size, cell_size));

                let filled = cells[x][y];

                let color = if filled {
                    egui::Color32::LIGHT_BLUE
                } else {
                    egui::Color32::from_gray(30)
                };

                painter.rect_filled(r, 0.0, color);
                painter.rect_stroke(r, 0.0, (1.0, egui::Color32::DARK_GRAY));
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Grid Wall Builder for Godot");

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Grid W");
                ui.add(egui::DragValue::new(&mut self.grid_w));

                ui.label("Grid H");
                ui.add(egui::DragValue::new(&mut self.grid_h));
            });

            self.ensure_grid_size();

            ui.separator();

            ui.label("Unit Size");
            ui.add(egui::DragValue::new(&mut self.export.unit_size).speed(0.1));

            ui.label("Z Size");
            ui.add(egui::DragValue::new(&mut self.export.z_size).speed(0.05));

            ui.separator();

            ui.label("Left drag = paint wall cell | Right drag = remove");
            self.draw_grid(ui);

            ui.separator();

            if ui.button("Generate Scene").clicked() {
                self.output = self.generate();
            }

            if ui.button("Load Scene").clicked() {
                if let Some(path) = FileDialog::new()
                    .add_filter("Godot Scene", &["tscn"])
                    .pick_file()
                {
                    match std::fs::read_to_string(&path) {
                        Ok(text) => match import_scene(&text) {
                            Ok(imported) => self.apply_imported_scene(imported),
                            Err(error) => {
                                self.output = format!("Import failed: {error}");
                            }
                        },
                        Err(error) => {
                            self.output = format!("Could not read file: {error}");
                        }
                    }
                }
            }

            if ui.button("Save Scene").clicked() {
                if let Some(path) = FileDialog::new()
                    .add_filter("Godot Scene", &["tscn"])
                    .save_file()
                {
                    std::fs::write(path, &self.output).unwrap();
                }
            }

            ui.separator();

            ui.label("Output:");
            ui.add(
                egui::TextEdit::multiline(&mut self.output)
                    .desired_rows(15)
                    .code_editor(),
            );
        });
    }
}
