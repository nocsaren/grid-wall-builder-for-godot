# Grid Wall Builder for Godot

Small Rust desktop app (eframe/egui) for drawing a grid-based wall layout and exporting it as a Godot 4 `.tscn` scene.

The editor is grid-based, and the export produces 3D `StaticBody3D` wall segments with a `BoxMesh` + `BoxShape3D`.

## Features

- Paint a wall layout directly on a 2D grid.
- Clear the whole grid with one click.
- Mirror the grid left-to-right.
- Merge adjacent filled cells into larger rectangular wall segments.
- Export Godot 4 scenes with a `BoxMeshes` container for wall bodies.
- Load scenes previously exported by this tool and continue editing them.
- Save exported scenes with `Save Scene As` only, to avoid accidental overwrites.

## Workflow

1. Set the grid width and height.
2. Load a `.tscn` file exported by this tool, if you want to edit an existing layout.
3. Click cells to place wall tiles.
4. Right click to remove tiles.
5. Use `Clear Grid` or `Mirror Left-Right` when needed.
6. Adjust `Unit Size` and `Z Size` as needed.
7. Generate the scene.
8. Use `Save Scene As` to write the scene to a path you choose.

## Build / Run

- Debug: `cargo run`
- Release: `cargo run --release`

Quality checks:

- Format: `cargo fmt`
- Tests: `cargo test`

## Export behavior

Filled cells are merged into rectangular wall segments before export, so adjacent tiles become a single `StaticBody3D` with one mesh and one collision shape.

The exported wall thickness on Z is controlled separately with the Z size setting.

The root scene hierarchy is:

- `Root Node3D`
- `BoxMeshes`

Under `BoxMeshes`, each merged segment becomes a `StaticBody3D` with:

- one `MeshInstance3D` using `BoxMesh`
- one `CollisionShape3D` using `BoxShape3D`

## Import behavior

The file picker only accepts supported scenes.

The loader will import scenes exported by this app, including older exports that may still contain the removed back-plane data. If a file uses a more complex or unrelated Godot scene structure, it is rejected instead of being partially imported.

When a scene is loaded, the app remembers that file path for reference, but exporting still uses `Save Scene As` so writes are always explicit.

### Coordinate system

The UI grid uses the typical screen convention: X increases to the right, Y increases downward.

Godot 3D is Y-up. The exporter maps the grid into Godot like this:

- Grid +X (right) → Godot +X
- Grid +Y (down) → Godot -Y (implemented as a flip + an upward shift by the total grid height)

Net effect: the entire layout sits in **positive Godot Y** (above the X axis), and increasing grid Y moves walls “down” in the editor.

Feel free to send pull requests. Any improvements are appreciated.

### Changelog

- 2.0.0 Removed plane exports entirely, added clear and mirror grid actions, and made exporting `Save Scene As` only.
- 1.3.0 Added optional rear PlaneMesh export, split output into BoxMeshes and BackPlanes, and added remembered save paths with overwrite support after loading.
- 1.2.0 Added file picker import for supported `.tscn` files with strict validation.
- 1.1.0 Grid painting added. Removed z offset.