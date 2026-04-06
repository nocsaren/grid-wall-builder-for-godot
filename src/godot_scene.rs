//! Godot 4 `.tscn` generation.
//!
//! The output scene contains:
//! - One root `Node3D`.
//! - One `Node3D` named `BoxMeshes` containing the merged wall segment bodies.
//! - Optionally one `Node3D` named `BackPlanes` containing rear-facing plane meshes.
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
    /// Whether to generate rear-facing plane meshes.
    pub include_backplanes: bool,
}

/// Generate a Godot 4 scene (`.tscn`) for the provided merged segments.
///
/// `grid_h` is used for the Y-up origin shift.
pub fn generate_scene(
    root_name: &str,
    grid_w: usize,
    grid_h: usize,
    settings: &ExportSettings,
    segments: &[Segment],
) -> String {
    let mut scene = String::new();

    let _ = writeln!(scene, "; generated-by=grid-wall-builder-for-godot");
    let _ = writeln!(
        scene,
        "; grid_w={} grid_h={} unit_size={} z_size={} include_backplanes={}",
        grid_w, grid_h, settings.unit_size, settings.z_size, settings.include_backplanes
    );
    let _ = writeln!(scene, "[gd_scene format=3]\n");

    for (id, segment) in segments.iter().enumerate() {
        let width = segment.width as f32 * settings.unit_size;
        let height = segment.height as f32 * settings.unit_size;

        let _ = write!(
            scene,
            "[sub_resource type=\"BoxMesh\" id=\"BoxMesh_{id}\"]\nsize = Vector3({w}, {h}, {d})\n\n[sub_resource type=\"BoxShape3D\" id=\"BoxShape3D_{id}\"]\nsize = Vector3({w}, {h}, {d})\n\n",
            id = id,
            w = width,
            h = height,
            d = settings.z_size,
        );

        if settings.include_backplanes {
            let _ = write!(
                scene,
                "[sub_resource type=\"PlaneMesh\" id=\"PlaneMesh_{id}\"]\nsize = Vector2({w}, {h})\n\n",
                id = id,
                w = width,
                h = height,
            );
        }
    }

    let _ = write!(scene, "[node name=\"{}\" type=\"Node3D\"]\n\n", root_name);
    let _ = write!(
        scene,
        "[node name=\"BoxMeshes\" type=\"Node3D\" parent=\".\"]\n\n"
    );

    let total_height = grid_h as f32 * settings.unit_size;

    for (id, segment) in segments.iter().enumerate() {
        let width = segment.width as f32 * settings.unit_size;
        let height = segment.height as f32 * settings.unit_size;
        let world_x = segment.start_x as f32 * settings.unit_size;
        let world_y = segment.start_y as f32 * settings.unit_size;

        let offset_x = world_x + width / 2.0;
        let offset_y = total_height - (world_y + height / 2.0);

        let _ = write!(
            scene,
            "[node name=\"Segment_{id}\" type=\"StaticBody3D\" parent=\"BoxMeshes\"]\ntransform = Transform3D(1, 0, 0, 0, 1, 0, 0, 0, 1, {ox}, {oy}, 0)\n\n[node name=\"MeshInstance3D\" type=\"MeshInstance3D\" parent=\"BoxMeshes/Segment_{id}\"]\nmesh = SubResource(\"BoxMesh_{id}\")\n\n[node name=\"CollisionShape3D\" type=\"CollisionShape3D\" parent=\"BoxMeshes/Segment_{id}\"]\nshape = SubResource(\"BoxShape3D_{id}\")\n\n",
            id = id,
            ox = offset_x,
            oy = offset_y,
        );
    }

    if settings.include_backplanes {
        let _ = write!(
            scene,
            "[node name=\"BackPlanes\" type=\"Node3D\" parent=\".\"]\n\n"
        );

        for (id, segment) in segments.iter().enumerate() {
            let width = segment.width as f32 * settings.unit_size;
            let height = segment.height as f32 * settings.unit_size;
            let world_x = segment.start_x as f32 * settings.unit_size;
            let world_y = segment.start_y as f32 * settings.unit_size;

            let offset_x = world_x + width / 2.0;
            let offset_y = total_height - (world_y + height / 2.0);
            let backplane_margin = settings.z_size * 0.0005;
            let backplane_offset_z = -(settings.z_size / 2.0 + backplane_margin);

            let _ = write!(
                scene,
                "[node name=\"BackPlane_{id}\" type=\"MeshInstance3D\" parent=\"BackPlanes\"]\ntransform = Transform3D(1, 0, 0, 0, 0, -1, 0, 1, 0, {ox}, {oy}, {oz})\nmesh = SubResource(\"PlaneMesh_{id}\")\n\n",
                id = id,
                ox = offset_x,
                oy = offset_y,
                oz = backplane_offset_z,
            );
        }
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
            include_backplanes: true,
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
        assert!(scene.contains("[node name=\"BoxMeshes\" type=\"Node3D\" parent=\".\"]"));
        assert!(scene.contains("[node name=\"BackPlanes\" type=\"Node3D\" parent=\".\"]"));
    }

    #[test]
    fn scene_contains_backplane_node() {
        let settings = ExportSettings {
            unit_size: 1.0,
            z_size: 0.1,
            include_backplanes: true,
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

        assert!(
            scene.contains("[node name=\"Segment_0\" type=\"StaticBody3D\" parent=\"BoxMeshes\"]")
        );
        assert!(scene.contains(
            "[node name=\"MeshInstance3D\" type=\"MeshInstance3D\" parent=\"BoxMeshes/Segment_0\"]"
        ));
        assert!(scene.contains("[sub_resource type=\"PlaneMesh\" id=\"PlaneMesh_0\"]"));
        assert!(scene
            .contains("[node name=\"BackPlane_0\" type=\"MeshInstance3D\" parent=\"BackPlanes\"]"));
        assert!(scene.contains("mesh = SubResource(\"PlaneMesh_0\")"));
    }

    #[test]
    fn scene_omits_backplanes_when_disabled() {
        let settings = ExportSettings {
            unit_size: 1.0,
            z_size: 0.1,
            include_backplanes: false,
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

        assert!(!scene.contains("[node name=\"BackPlanes\" type=\"Node3D\" parent=\".\"]"));
        assert!(!scene.contains("[sub_resource type=\"PlaneMesh\" id=\"PlaneMesh_0\"]"));
    }
}
