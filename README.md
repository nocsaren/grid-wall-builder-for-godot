# Grid Wall Builder for Godot

Small Rust desktop app (eframe/egui) for drawing a grid-based wall layout and exporting it as a Godot 4 `.tscn` scene.

The editor is grid-based, and the export produces 3D `StaticBody3D` wall segments with a `BoxMesh` + `BoxShape3D`.

## Workflow

1. Set the grid width and height.
2. Load a `.tscn` file exported by this tool, if you want to edit an existing layout.
3. Click cells to place wall tiles.
4. Right click to remove tiles.
5. Generate the scene.
6. Save the result as a `.tscn` file.

## Build / Run

- Debug: `cargo run`
- Release: `cargo run --release`

Quality checks:

- Format: `cargo fmt`
- Tests: `cargo test`

## Export behavior

Filled cells are merged into rectangular wall segments before export, so adjacent tiles become a single `StaticBody3D` with one mesh and one collision shape.

The exported wall thickness on Z is controlled separately with the Z size setting.

## Import behavior

The file picker only accepts supported scenes.

The loader will import scenes exported by this app, including older exports that do not have the new metadata comments. If a file uses a more complex or unrelated Godot scene structure, it is rejected instead of being partially imported.

### Coordinate system

The UI grid uses the typical screen convention: X increases to the right, Y increases downward.

Godot 3D is Y-up. The exporter maps the grid into Godot like this:

- Grid +X (right) → Godot +X
- Grid +Y (down) → Godot -Y (implemented as a flip + an upward shift by the total grid height)
- Godot +Z is used as wall thickness (the mesh and collision are centered around `z = z_size / 2`)

Net effect: the entire layout sits in **positive Godot Y** (above the X axis), and increasing grid Y moves walls “down” in the editor.

Feel free to send pull requests. Any improvements are appreciated.

### Changelog

1.2.0 Added file picker import for supported `.tscn` files with strict validation.
1.1.0 Grid painting added. Removed z offset.