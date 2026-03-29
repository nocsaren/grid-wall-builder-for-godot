//! egui application for painting a grid-based wall layout.
//!
//! The editor interaction is grid/2D-like (paint cells), but the export is 3D:
//! each merged wall segment becomes a Godot `StaticBody3D` with a `BoxMesh` and
//! `BoxShape3D`. The `z_size` setting controls the thickness of each wall along
//! Godot's Z axis.

use eframe::egui;
use rfd::FileDialog;

use crate::godot_scene::{generate_scene, ExportSettings};
use crate::grid::Grid;

pub struct App {
    export: ExportSettings,

    grid_w: usize,
    grid_h: usize,

    grid: Grid,

    name: String,
    output: String,
}

impl Default for App {
    fn default() -> Self {
        let grid_w = 10;
        let grid_h = 5;

        Self {
            export: ExportSettings {
                unit_size: 0.5,
                z_size: 0.1,
            },
            grid_w,
            grid_h,
            grid: Grid::new(grid_w, grid_h),
            name: "GridWall".to_string(),
            output: String::new(),
        }
    }
}

impl App {
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
            egui::Sense::click(),
        );

        let rect = response.rect;
        let cells = self.grid.cells_mut();

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

                if response.hovered() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        if r.contains(pos) {
                            if ui.input(|i| i.pointer.primary_clicked()) {
                                cells[x][y] = true;
                            }

                            if ui.input(|i| i.pointer.secondary_clicked()) {
                                cells[x][y] = false;
                            }
                        }
                    }
                }
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

            ui.label("Left click = place wall cell | Right click = remove");
            self.draw_grid(ui);

            ui.separator();

            if ui.button("Generate Scene").clicked() {
                self.output = self.generate();
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
