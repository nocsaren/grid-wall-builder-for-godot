//! Godot 4 `.tscn` generation.
//!
//! The output scene contains:
//! - One root `Node3D`.
//! - One `StaticBody3D` per merged wall segment.
//! - A `MeshInstance3D` and `CollisionShape3D` under each `StaticBody3D`.
//!
//! Coordinate mapping notes:
//! - The UI grid is X-right / Y-down.
//! - Godot 3D is Y-up.
//! - We map grid X directly to Godot X.
//! - We map grid Y to Godot -Y, and shift up by the total grid height so the
//!   full layout ends up in positive Y.

use std::fmt::Write;

use crate::grid::Segment;

#[derive(Clone, Debug)]
/// Export settings expressed in Godot world units.
pub struct ExportSettings {
    /// Size of one grid cell in world units.
    pub unit_size: f32,
    /// Wall thickness along Godot's Z axis.
    pub z_size: f32,
}

/// Generate a Godot 4 scene (`.tscn`) for the provided merged segments.
///
/// `grid_h` is used for the Y-up origin shift.
pub fn generate_scene(
    root_name: &str,
    _grid_w: usize,
    grid_h: usize,
    settings: &ExportSettings,
    segments: &[Segment],
) -> String {
    let mut scene = String::from("[gd_scene format=3]\n\n");

    for (id, segment) in segments.iter().enumerate() {
        let width = segment.width as f32 * settings.unit_size;
        let height = segment.height as f32 * settings.unit_size;

        let _ = write!(
            scene,
            "[sub_resource type=\"BoxMesh\" id=\"BoxMesh_{id}\"]\nsize = Vector3({w}, {h}, {d})\n\n[sub_resource type=\"BoxShape3D\" id=\"BoxShape3D_{id}\"]\nsize = Vector3({w}, {h}, {d})\n\n",
            id = id,
            w = width,
            h = height,
            d = settings.z_size
        );
    }

    let _ = write!(scene, "[node name=\"{}\" type=\"Node3D\"]\n\n", root_name);

    let total_height = grid_h as f32 * settings.unit_size;

    for (id, segment) in segments.iter().enumerate() {
        let width = segment.width as f32 * settings.unit_size;
        let height = segment.height as f32 * settings.unit_size;

        let world_x = segment.start_x as f32 * settings.unit_size;
        let world_y = segment.start_y as f32 * settings.unit_size;

        // Godot 3D uses +Y up. The UI grid uses +Y down.
        // Mapping used by this tool:
        // - grid +X (right) -> Godot +X
        // - grid +Y (down)  -> Godot -Y, with an origin shift by total grid height
        //   so the full layout ends up in +Y.
        let offset_x = world_x + width / 2.0;
        let offset_y = total_height - (world_y + height / 2.0);
        let offset_z = 0.0; // flat on the X/Y plane, with thickness along Z

        let _ = write!(
            scene,
            "[node name=\"Segment_{id}\" type=\"StaticBody3D\" parent=\".\"]\ntransform = Transform3D(1, 0, 0, 0, 1, 0, 0, 0, 1, {ox}, {oy}, {oz})\n\n[node name=\"MeshInstance3D\" type=\"MeshInstance3D\" parent=\"Segment_{id}\"]\nmesh = SubResource(\"BoxMesh_{id}\")\n\n[node name=\"CollisionShape3D\" type=\"CollisionShape3D\" parent=\"Segment_{id}\"]\nshape = SubResource(\"BoxShape3D_{id}\")\n\n",
            id = id,
            ox = offset_x,
            oy = offset_y,
            oz = offset_z
        );
    }

    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_contains_root_node() {
        let settings = ExportSettings {
            unit_size: 1.0,
            z_size: 0.1,
        };

        let scene = generate_scene(
            "Root",
            1,
            1,
            &settings,
            &[Segment {
                start_x: 0,
                start_y: 0,
                width: 1,
                height: 1,
            }],
        );

        assert!(scene.contains("[node name=\"Root\" type=\"Node3D\"]"));
    }
}
