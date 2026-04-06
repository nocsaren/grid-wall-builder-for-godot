use crate::godot_scene::ExportSettings;
use crate::grid::Grid;

const EXPORT_MARKER: &str = "; generated-by=grid-wall-builder-for-godot";
const FLOAT_TOLERANCE: f32 = 0.0001;
const INFER_SCALE: f32 = 10_000.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    Unsupported(String),
    Invalid(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(message) | Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ImportError {}

#[derive(Debug)]
pub struct ImportedScene {
    pub name: String,
    pub export: ExportSettings,
    pub grid: Grid,
}

#[derive(Debug)]
struct ResourceRecord {
    width: f32,
    height: f32,
    depth: f32,
}

#[derive(Debug)]
struct RawSegment {
    width: f32,
    height: f32,
    offset_x: f32,
    offset_y: f32,
}

#[derive(Debug)]
struct Metadata {
    grid_w: usize,
    grid_h: usize,
    unit_size: f32,
    z_size: f32,
    include_backplanes: Option<bool>,
}

#[derive(Debug)]
struct SegmentImport {
    start_x: usize,
    start_y: usize,
    width: usize,
    height: usize,
}

pub fn import_scene(text: &str) -> Result<ImportedScene, ImportError> {
    let mut parser = SceneParser::new(text);

    let metadata = if parser.peek_line() == Some(EXPORT_MARKER) {
        parser.next_line();
        Some(parser.expect_metadata()?)
    } else {
        None
    };

    parser.expect_exact("[gd_scene format=3]")?;

    let mut resources = Vec::new();
    let mut raw_segments = Vec::new();
    let mut has_backplanes = false;
    let root_name;

    loop {
        match parser.peek_line() {
            Some(line) if line.starts_with("[sub_resource type=\"BoxMesh\" id=\"BoxMesh_") => {
                resources.push(parser.parse_resource_pair(resources.len())?);
            }
            Some(line) if line.starts_with("[sub_resource type=\"PlaneMesh\" id=\"PlaneMesh_") => {
                parser.skip_plane_mesh_resource()?;
            }
            Some(line)
                if line.starts_with("[node name=\"") && line.contains(" type=\"Node3D\"]") =>
            {
                root_name = parser.parse_root_node()?;
                break;
            }
            Some(line) if line.starts_with(';') => {
                return Err(ImportError::Unsupported(format!(
                    "Unsupported scene comment: {line}"
                )));
            }
            Some(line) => {
                return Err(ImportError::Unsupported(format!(
                    "Unsupported scene content: {line}"
                )));
            }
            None => {
                return Err(ImportError::Invalid(
                    "Missing root Node3D in imported scene".to_string(),
                ));
            }
        }
    }

    while parser.peek_line().is_some() {
        let line = parser.peek_line().unwrap();

        if line.starts_with("[node name=\"BoxMeshes\" type=\"Node3D\"")
            && line.contains(" parent=\".\"]")
        {
            parser.next_line();
            while parser.peek_line().map_or(false, |line| {
                line.starts_with("[node name=\"Segment_")
                    && line.contains(" type=\"StaticBody3D\"")
                    && line.contains(" parent=\"BoxMeshes\"]")
            }) {
                raw_segments.push(parser.parse_segment(&resources, "BoxMeshes")?);
            }
        } else if line.starts_with("[node name=\"BackPlanes\" type=\"Node3D\"")
            && line.contains(" parent=\".\"]")
        {
            has_backplanes = true;
            parser.next_line();
            while parser.peek_line().map_or(false, |line| {
                line.starts_with("[node name=\"") && line.contains(" parent=\"BackPlanes\"]")
            }) {
                parser.skip_node_block();
            }
        } else if line.starts_with("[node name=\"Segment_")
            && line.contains(" type=\"StaticBody3D\"")
            && line.contains(" parent=\".\"]")
        {
            raw_segments.push(parser.parse_segment(&resources, ".")?);
        } else {
            return Err(ImportError::Unsupported(format!(
                "Unsupported scene content: {line}"
            )));
        }
    }

    if raw_segments.is_empty() {
        let grid = if let Some(metadata) = metadata.as_ref() {
            Grid::new(metadata.grid_w, metadata.grid_h)
        } else {
            Grid::new(0, 0)
        };

        let export = if let Some(metadata) = metadata.as_ref() {
            ExportSettings {
                unit_size: metadata.unit_size,
                z_size: metadata.z_size,
                include_backplanes: metadata.include_backplanes.unwrap_or(has_backplanes),
            }
        } else {
            ExportSettings {
                unit_size: 1.0,
                z_size: 0.1,
                include_backplanes: has_backplanes,
            }
        };

        return Ok(ImportedScene {
            name: root_name,
            export,
            grid,
        });
    }

    let unit_size = if let Some(metadata) = metadata.as_ref() {
        metadata.unit_size
    } else {
        infer_unit_size(&resources)?
    };

    let imported_segments = if let Some(metadata) = metadata.as_ref() {
        raw_segments
            .iter()
            .map(|segment| convert_segment_with_metadata(segment, metadata, unit_size))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        normalize_legacy_segments(&raw_segments, unit_size)?
    };

    let grid_w = metadata
        .as_ref()
        .map(|metadata| metadata.grid_w)
        .unwrap_or_else(|| {
            imported_segments
                .iter()
                .map(|segment| segment.start_x + segment.width)
                .max()
                .unwrap_or(0)
        });
    let grid_h = metadata
        .as_ref()
        .map(|metadata| metadata.grid_h)
        .unwrap_or_else(|| {
            imported_segments
                .iter()
                .map(|segment| segment.start_y + segment.height)
                .max()
                .unwrap_or(0)
        });

    let mut grid = Grid::new(grid_w, grid_h);

    for segment in imported_segments {
        for dx in 0..segment.width {
            for dy in 0..segment.height {
                let x = segment.start_x + dx;
                let y = segment.start_y + dy;

                if x >= grid.width() || y >= grid.height() {
                    return Err(ImportError::Invalid(
                        "Imported scene contains cells outside the declared grid bounds"
                            .to_string(),
                    ));
                }

                if grid.cells()[x][y] {
                    return Err(ImportError::Invalid(
                        "Imported scene contains overlapping wall segments".to_string(),
                    ));
                }

                grid.cells_mut()[x][y] = true;
            }
        }
    }

    let export = if let Some(metadata) = metadata {
        ExportSettings {
            unit_size: metadata.unit_size,
            z_size: metadata.z_size,
            include_backplanes: metadata.include_backplanes.unwrap_or(has_backplanes),
        }
    } else {
        ExportSettings {
            unit_size,
            z_size: infer_z_size(&resources)?,
            include_backplanes: has_backplanes,
        }
    };

    Ok(ImportedScene {
        name: root_name,
        export,
        grid,
    })
}

fn convert_segment_with_metadata(
    segment: &RawSegment,
    metadata: &Metadata,
    unit_size: f32,
) -> Result<SegmentImport, ImportError> {
    let width = float_to_index(segment.width / unit_size, "segment width")?;
    let height = float_to_index(segment.height / unit_size, "segment height")?;
    let start_x = float_to_index(
        segment.offset_x / unit_size - width as f32 / 2.0,
        "segment start_x",
    )?;
    let start_y = float_to_index(
        metadata.grid_h as f32 - (segment.offset_y / unit_size) - height as f32 / 2.0,
        "segment start_y",
    )?;

    Ok(SegmentImport {
        start_x,
        start_y,
        width,
        height,
    })
}

fn normalize_legacy_segments(
    segments: &[RawSegment],
    unit_size: f32,
) -> Result<Vec<SegmentImport>, ImportError> {
    let top_edge_world = segments
        .iter()
        .map(|segment| segment.offset_y + segment.height / 2.0)
        .fold(f32::NEG_INFINITY, f32::max);

    segments
        .iter()
        .map(|segment| {
            let width = float_to_index(segment.width / unit_size, "segment width")?;
            let height = float_to_index(segment.height / unit_size, "segment height")?;
            let start_x = float_to_index(
                segment.offset_x / unit_size - width as f32 / 2.0,
                "segment start_x",
            )?;
            let start_y = float_to_index(
                (top_edge_world - (segment.offset_y + segment.height / 2.0)) / unit_size,
                "segment start_y",
            )?;

            Ok(SegmentImport {
                start_x,
                start_y,
                width,
                height,
            })
        })
        .collect()
}

fn infer_unit_size(resources: &[ResourceRecord]) -> Result<f32, ImportError> {
    let mut values = Vec::new();

    for resource in resources {
        values.push(resource.width.abs());
        values.push(resource.height.abs());
    }

    let scaled_values: Vec<i64> = values
        .into_iter()
        .filter(|value| *value > 0.0)
        .map(|value| (value * INFER_SCALE).round() as i64)
        .filter(|value| *value > 0)
        .collect();

    if scaled_values.is_empty() {
        return Err(ImportError::Invalid(
            "Could not infer the grid unit size from the imported scene".to_string(),
        ));
    }

    let gcd = scaled_values
        .into_iter()
        .reduce(gcd_i64)
        .ok_or_else(|| ImportError::Invalid("Could not infer the grid unit size".to_string()))?;

    Ok(gcd as f32 / INFER_SCALE)
}

fn infer_z_size(resources: &[ResourceRecord]) -> Result<f32, ImportError> {
    let mut z_size: Option<f32> = None;

    for resource in resources {
        match z_size {
            Some(existing) if (existing - resource.depth).abs() > FLOAT_TOLERANCE => {
                return Err(ImportError::Unsupported(
                    "Imported scene uses inconsistent wall thickness values".to_string(),
                ));
            }
            None => z_size = Some(resource.depth),
            _ => {}
        }
    }

    z_size
        .ok_or_else(|| ImportError::Invalid("Missing wall thickness in imported scene".to_string()))
}

fn gcd_i64(left: i64, right: i64) -> i64 {
    let mut left = left.abs();
    let mut right = right.abs();

    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }

    left
}

struct SceneParser<'a> {
    lines: Vec<&'a str>,
    index: usize,
}

impl<'a> SceneParser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            lines: text.lines().collect(),
            index: 0,
        }
    }

    fn peek_line(&mut self) -> Option<&'a str> {
        self.skip_blank_lines();
        self.lines.get(self.index).copied()
    }

    fn next_line(&mut self) -> Option<&'a str> {
        self.skip_blank_lines();

        let line = self.lines.get(self.index).copied();
        if line.is_some() {
            self.index += 1;
        }

        line
    }

    fn skip_blank_lines(&mut self) {
        while self
            .lines
            .get(self.index)
            .is_some_and(|line| line.trim().is_empty())
        {
            self.index += 1;
        }
    }

    fn expect_exact(&mut self, expected: &str) -> Result<(), ImportError> {
        let line = self.next_line().ok_or_else(|| {
            ImportError::Invalid(format!("Expected `{expected}` but found end of file"))
        })?;

        if line.trim() == expected {
            Ok(())
        } else {
            Err(ImportError::Invalid(format!(
                "Expected `{expected}` but found `{line}`"
            )))
        }
    }

    fn expect_metadata(&mut self) -> Result<Metadata, ImportError> {
        let line = self.next_line().ok_or_else(|| {
            ImportError::Invalid("Expected metadata line after export marker".to_string())
        })?;

        if !line.starts_with("; grid_w=") {
            return Err(ImportError::Unsupported(format!(
                "Unsupported scene metadata: {line}"
            )));
        }

        let mut grid_w = None;
        let mut grid_h = None;
        let mut unit_size = None;
        let mut z_size = None;
        let mut include_backplanes = None;

        for part in line.trim_start_matches(';').split_whitespace() {
            let mut split = part.splitn(2, '=');
            let key = split.next().unwrap_or_default();
            let value = split.next().unwrap_or_default();

            match key {
                "grid_w" => grid_w = Some(parse_usize(value, "grid_w")?),
                "grid_h" => grid_h = Some(parse_usize(value, "grid_h")?),
                "unit_size" => unit_size = Some(parse_f32(value, "unit_size")?),
                "z_size" => z_size = Some(parse_f32(value, "z_size")?),
                "include_backplanes" => {
                    include_backplanes = Some(parse_bool(value, "include_backplanes")?)
                }
                _ => {
                    return Err(ImportError::Unsupported(format!(
                        "Unexpected metadata key `{key}`"
                    )));
                }
            }
        }

        Ok(Metadata {
            grid_w: grid_w
                .ok_or_else(|| ImportError::Invalid("Missing grid_w metadata".to_string()))?,
            grid_h: grid_h
                .ok_or_else(|| ImportError::Invalid("Missing grid_h metadata".to_string()))?,
            unit_size: unit_size
                .ok_or_else(|| ImportError::Invalid("Missing unit_size metadata".to_string()))?,
            z_size: z_size
                .ok_or_else(|| ImportError::Invalid("Missing z_size metadata".to_string()))?,
            include_backplanes,
        })
    }

    fn skip_plane_mesh_resource(&mut self) -> Result<(), ImportError> {
        let mesh_line = self
            .next_line()
            .ok_or_else(|| ImportError::Invalid("Expected PlaneMesh resource block".to_string()))?;
        if !mesh_line.starts_with("[sub_resource type=\"PlaneMesh\" id=\"PlaneMesh_") {
            return Err(ImportError::Unsupported(format!(
                "Unsupported PlaneMesh resource block: {mesh_line}"
            )));
        }

        let size_line = self
            .next_line()
            .ok_or_else(|| ImportError::Invalid("Expected PlaneMesh size line".to_string()))?;
        parse_vector2_line(size_line, "size = Vector2(")?;
        Ok(())
    }

    fn parse_resource_pair(&mut self, expected_id: usize) -> Result<ResourceRecord, ImportError> {
        let mesh_line = self
            .next_line()
            .ok_or_else(|| ImportError::Invalid("Expected BoxMesh resource block".to_string()))?;
        let mesh_id = parse_resource_id(mesh_line, "[sub_resource type=\"BoxMesh\" id=\"BoxMesh_")?;
        if mesh_id != expected_id {
            return Err(ImportError::Unsupported(format!(
                "Unexpected BoxMesh resource id {mesh_id}; expected {expected_id}"
            )));
        }

        let mesh_size_line = self
            .next_line()
            .ok_or_else(|| ImportError::Invalid("Expected BoxMesh size line".to_string()))?;
        let (mesh_width, mesh_height, mesh_depth) =
            parse_vector3_line(mesh_size_line, "size = Vector3(")?;

        let shape_line = self.next_line().ok_or_else(|| {
            ImportError::Invalid("Expected BoxShape3D resource block".to_string())
        })?;
        let shape_id = parse_resource_id(
            shape_line,
            "[sub_resource type=\"BoxShape3D\" id=\"BoxShape3D_",
        )?;
        if shape_id != expected_id {
            return Err(ImportError::Unsupported(format!(
                "Unexpected BoxShape3D resource id {shape_id}; expected {expected_id}"
            )));
        }

        let shape_size_line = self
            .next_line()
            .ok_or_else(|| ImportError::Invalid("Expected BoxShape3D size line".to_string()))?;
        let (shape_width, shape_height, shape_depth) =
            parse_vector3_line(shape_size_line, "size = Vector3(")?;

        if (mesh_width - shape_width).abs() > FLOAT_TOLERANCE
            || (mesh_height - shape_height).abs() > FLOAT_TOLERANCE
            || (mesh_depth - shape_depth).abs() > FLOAT_TOLERANCE
        {
            return Err(ImportError::Unsupported(
                "Mesh and collision shape sizes do not match".to_string(),
            ));
        }

        Ok(ResourceRecord {
            width: mesh_width,
            height: mesh_height,
            depth: mesh_depth,
        })
    }

    fn parse_root_node(&mut self) -> Result<String, ImportError> {
        let line = self
            .next_line()
            .ok_or_else(|| ImportError::Invalid("Expected root Node3D block".to_string()))?;
        parse_node_line(line, "Node3D", None)
    }

    fn parse_segment(
        &mut self,
        resources: &[ResourceRecord],
        expected_parent: &str,
    ) -> Result<RawSegment, ImportError> {
        let segment_line = self.next_line().ok_or_else(|| {
            ImportError::Invalid("Expected StaticBody3D segment block".to_string())
        })?;
        let segment_name = parse_node_line(segment_line, "StaticBody3D", Some(expected_parent))?;
        let segment_id = parse_suffix_id(&segment_name, "Segment_")?;
        let segment_path = if expected_parent == "." {
            segment_name.clone()
        } else {
            format!("{expected_parent}/{segment_name}")
        };

        let transform_line = self
            .next_line()
            .ok_or_else(|| ImportError::Invalid("Expected segment transform".to_string()))?;
        let (offset_x, offset_y, offset_z) = parse_transform(transform_line)?;
        if offset_z.abs() > FLOAT_TOLERANCE {
            return Err(ImportError::Unsupported(
                "Only flat Z transforms are supported".to_string(),
            ));
        }

        let mut mesh_id = None;
        let mut shape_id = None;

        while mesh_id.is_none() || shape_id.is_none() {
            let line = self
                .peek_line()
                .ok_or_else(|| ImportError::Invalid("Unexpected end of segment".to_string()))?;

            if line.starts_with("[node name=\"") && line.contains(" type=\"MeshInstance3D\"") {
                let mesh_node_line = self.next_line().unwrap();
                parse_node_line(mesh_node_line, "MeshInstance3D", Some(&segment_path))?;

                while let Some(next_line) = self.peek_line() {
                    if next_line.starts_with("[node name=\"") {
                        break;
                    }

                    let property_line = self.next_line().unwrap();
                    if property_line.starts_with("mesh = SubResource(\"BoxMesh_") {
                        let candidate_mesh_id = parse_resource_reference(
                            property_line,
                            "mesh = SubResource(\"BoxMesh_",
                        )?;

                        if let Some(existing_mesh_id) = mesh_id {
                            if existing_mesh_id != candidate_mesh_id {
                                return Err(ImportError::Unsupported(
                                    "Segment contains multiple MeshInstance3D nodes with different mesh ids".to_string(),
                                ));
                            }
                        } else {
                            mesh_id = Some(candidate_mesh_id);
                        }
                    }
                }
            } else if line.starts_with("[node name=\"")
                && line.contains(" type=\"CollisionShape3D\"")
            {
                let collision_line = self.next_line().unwrap();
                parse_node_line(collision_line, "CollisionShape3D", Some(&segment_path))?;

                while let Some(next_line) = self.peek_line() {
                    if next_line.starts_with("[node name=\"") {
                        break;
                    }

                    let property_line = self.next_line().unwrap();
                    if property_line.starts_with("shape = SubResource(\"BoxShape3D_") {
                        let candidate_shape_id = parse_resource_reference(
                            property_line,
                            "shape = SubResource(\"BoxShape3D_",
                        )?;

                        if let Some(existing_shape_id) = shape_id {
                            if existing_shape_id != candidate_shape_id {
                                return Err(ImportError::Unsupported(
                                    "Segment contains multiple CollisionShape3D nodes with different shape ids".to_string(),
                                ));
                            }
                        } else {
                            shape_id = Some(candidate_shape_id);
                        }
                    }
                }
            } else {
                return Err(ImportError::Invalid(format!(
                    "Unexpected segment content: {line}"
                )));
            }
        }

        let mesh_id = mesh_id
            .ok_or_else(|| ImportError::Invalid("Missing mesh instance in segment".to_string()))?;
        let shape_id = shape_id.ok_or_else(|| {
            ImportError::Invalid("Missing collision shape in segment".to_string())
        })?;

        if mesh_id != segment_id || shape_id != segment_id {
            return Err(ImportError::Unsupported(
                "Segment node ids do not match their resources".to_string(),
            ));
        }

        let resource = resources
            .get(segment_id)
            .ok_or_else(|| ImportError::Invalid("Missing segment resource block".to_string()))?;

        Ok(RawSegment {
            width: resource.width,
            height: resource.height,
            offset_x,
            offset_y,
        })
    }
}

impl<'a> SceneParser<'a> {
    fn skip_node_block(&mut self) {
        self.next_line();

        while let Some(line) = self.peek_line() {
            if line.starts_with("[node name=\"") {
                break;
            }
            self.next_line();
        }
    }
}

fn parse_node_line(
    line: &str,
    expected_type: &str,
    expected_parent: Option<&str>,
) -> Result<String, ImportError> {
    let trimmed = line.trim();
    if !trimmed.starts_with("[node name=\"") || !trimmed.ends_with(']') {
        return Err(ImportError::Unsupported(format!(
            "Unsupported node block: {line}"
        )));
    }

    let expected_type_fragment = format!(" type=\"{expected_type}\"");
    if !trimmed.contains(&expected_type_fragment) {
        return Err(ImportError::Unsupported(format!(
            "Unsupported node type in `{line}`"
        )));
    }

    if let Some(parent) = expected_parent {
        let expected_parent_fragment = format!(" parent=\"{parent}\"");
        if !trimmed.contains(&expected_parent_fragment) {
            return Err(ImportError::Unsupported(format!(
                "Unsupported node parent in `{line}`"
            )));
        }
    }

    let name_start = "[node name=\"".len();
    let name_end = trimmed[name_start..]
        .find('"')
        .ok_or_else(|| ImportError::Invalid(format!("Malformed node line: {line}")))?
        + name_start;

    Ok(trimmed[name_start..name_end].to_string())
}

fn parse_resource_id(line: &str, prefix: &str) -> Result<usize, ImportError> {
    let trimmed = line.trim();
    if !trimmed.starts_with(prefix) || !trimmed.ends_with("]") {
        return Err(ImportError::Unsupported(format!(
            "Unsupported resource block: {line}"
        )));
    }

    let raw_id = trimmed
        .get(prefix.len()..trimmed.len() - 2)
        .ok_or_else(|| ImportError::Invalid(format!("Malformed resource id: {line}")))?;

    parse_usize(raw_id, "resource id")
}

fn parse_resource_reference(line: &str, prefix: &str) -> Result<usize, ImportError> {
    let trimmed = line.trim();
    if !trimmed.starts_with(prefix) || !trimmed.ends_with(")") {
        return Err(ImportError::Unsupported(format!(
            "Unsupported resource reference: {line}"
        )));
    }

    let raw_id = trimmed
        .get(prefix.len()..trimmed.len() - 2)
        .ok_or_else(|| ImportError::Invalid(format!("Malformed resource reference: {line}")))?;

    parse_usize(raw_id, "resource reference")
}

fn parse_vector3_line(line: &str, prefix: &str) -> Result<(f32, f32, f32), ImportError> {
    let trimmed = line.trim();
    if !trimmed.starts_with(prefix) || !trimmed.ends_with(')') {
        return Err(ImportError::Unsupported(format!(
            "Unsupported vector line: {line}"
        )));
    }

    let values = &trimmed[prefix.len()..trimmed.len() - 1];
    let parts: Vec<_> = values.split(',').map(|part| part.trim()).collect();

    if parts.len() != 3 {
        return Err(ImportError::Invalid(format!(
            "Expected three Vector3 components in `{line}`"
        )));
    }

    Ok((
        parse_f32(parts[0], "vector component")?,
        parse_f32(parts[1], "vector component")?,
        parse_f32(parts[2], "vector component")?,
    ))
}

fn parse_vector2_line(line: &str, prefix: &str) -> Result<(f32, f32), ImportError> {
    let trimmed = line.trim();
    if !trimmed.starts_with(prefix) || !trimmed.ends_with(')') {
        return Err(ImportError::Unsupported(format!(
            "Unsupported vector line: {line}"
        )));
    }

    let values = &trimmed[prefix.len()..trimmed.len() - 1];
    let parts: Vec<_> = values.split(',').map(|part| part.trim()).collect();

    if parts.len() != 2 {
        return Err(ImportError::Invalid(format!(
            "Expected two Vector2 components in `{line}`"
        )));
    }

    Ok((
        parse_f32(parts[0], "vector component")?,
        parse_f32(parts[1], "vector component")?,
    ))
}

fn parse_transform(line: &str) -> Result<(f32, f32, f32), ImportError> {
    let trimmed = line.trim();
    let prefix = "transform = Transform3D(";
    if !trimmed.starts_with(prefix) || !trimmed.ends_with(')') {
        return Err(ImportError::Unsupported(format!(
            "Unsupported transform line: {line}"
        )));
    }

    let values = &trimmed[prefix.len()..trimmed.len() - 1];
    let parts: Vec<_> = values.split(',').map(|part| part.trim()).collect();
    if parts.len() != 12 {
        return Err(ImportError::Invalid(format!(
            "Expected 12 Transform3D components in `{line}`"
        )));
    }

    let identity = [
        parse_f32(parts[0], "transform component")?,
        parse_f32(parts[1], "transform component")?,
        parse_f32(parts[2], "transform component")?,
        parse_f32(parts[3], "transform component")?,
        parse_f32(parts[4], "transform component")?,
        parse_f32(parts[5], "transform component")?,
        parse_f32(parts[6], "transform component")?,
        parse_f32(parts[7], "transform component")?,
        parse_f32(parts[8], "transform component")?,
    ];

    if (identity[0] - 1.0).abs() > FLOAT_TOLERANCE
        || identity[1].abs() > FLOAT_TOLERANCE
        || identity[2].abs() > FLOAT_TOLERANCE
        || identity[3].abs() > FLOAT_TOLERANCE
        || (identity[4] - 1.0).abs() > FLOAT_TOLERANCE
        || identity[5].abs() > FLOAT_TOLERANCE
        || identity[6].abs() > FLOAT_TOLERANCE
        || identity[7].abs() > FLOAT_TOLERANCE
        || (identity[8] - 1.0).abs() > FLOAT_TOLERANCE
    {
        return Err(ImportError::Unsupported(
            "Only identity basis transforms are supported".to_string(),
        ));
    }

    Ok((
        parse_f32(parts[9], "transform offset_x")?,
        parse_f32(parts[10], "transform offset_y")?,
        parse_f32(parts[11], "transform offset_z")?,
    ))
}

fn parse_suffix_id(value: &str, prefix: &str) -> Result<usize, ImportError> {
    let suffix = value
        .strip_prefix(prefix)
        .ok_or_else(|| ImportError::Invalid(format!("Expected `{prefix}` prefix in `{value}`")))?;
    parse_usize(suffix, "node id")
}

fn parse_f32(value: &str, label: &str) -> Result<f32, ImportError> {
    value
        .parse::<f32>()
        .map_err(|_| ImportError::Invalid(format!("Failed to parse {label} from `{value}`")))
}

fn parse_usize(value: &str, label: &str) -> Result<usize, ImportError> {
    value
        .parse::<usize>()
        .map_err(|_| ImportError::Invalid(format!("Failed to parse {label} from `{value}`")))
}

fn parse_bool(value: &str, label: &str) -> Result<bool, ImportError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ImportError::Invalid(format!(
            "Failed to parse {label} from `{value}`"
        ))),
    }
}

fn float_to_index(value: f32, label: &str) -> Result<usize, ImportError> {
    if !value.is_finite() || value < 0.0 {
        return Err(ImportError::Invalid(format!(
            "Invalid {label} value: {value}"
        )));
    }

    let rounded = value.round();
    if (rounded - value).abs() > FLOAT_TOLERANCE {
        return Err(ImportError::Invalid(format!(
            "{label} is not aligned to the grid: {value}"
        )));
    }

    Ok(rounded as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::godot_scene::generate_scene;
    use crate::grid::Segment;

    #[test]
    fn new_export_round_trips_with_metadata() {
        let settings = ExportSettings {
            unit_size: 0.5,
            z_size: 0.1,
            include_backplanes: true,
        };

        let scene = generate_scene(
            "GridWall",
            4,
            3,
            &settings,
            &[
                Segment {
                    start_x: 0,
                    start_y: 0,
                    width: 2,
                    height: 1,
                },
                Segment {
                    start_x: 2,
                    start_y: 1,
                    width: 1,
                    height: 2,
                },
            ],
        );

        let imported = import_scene(&scene).expect("scene should import");

        assert_eq!(imported.name, "GridWall");
        assert_eq!(imported.export.unit_size, 0.5);
        assert_eq!(imported.export.z_size, 0.1);
        assert!(imported.export.include_backplanes);
        assert_eq!(imported.grid.width(), 4);
        assert_eq!(imported.grid.height(), 3);
        assert!(imported.grid.cells()[0][0]);
        assert!(imported.grid.cells()[1][0]);
        assert!(imported.grid.cells()[2][1]);
        assert!(imported.grid.cells()[2][2]);
    }

    #[test]
    fn legacy_export_without_metadata_is_still_supported() {
        let scene = generate_scene(
            "Root",
            2,
            2,
            &ExportSettings {
                unit_size: 0.5,
                z_size: 0.1,
                include_backplanes: true,
            },
            &[Segment {
                start_x: 0,
                start_y: 0,
                width: 1,
                height: 1,
            }],
        );
        let scene = scene.lines().skip(2).collect::<Vec<_>>().join("\n") + "\n";

        let imported = import_scene(&scene).expect("legacy scene should import");

        assert_eq!(imported.name, "Root");
        assert_eq!(imported.export.unit_size, 0.5);
        assert_eq!(imported.export.z_size, 0.1);
        assert!(imported.export.include_backplanes);
        assert_eq!(imported.grid.width(), 1);
        assert_eq!(imported.grid.height(), 1);
        assert!(imported.grid.cells()[0][0]);
    }

    #[test]
    fn export_without_backplanes_round_trips_setting() {
        let scene = generate_scene(
            "Root",
            2,
            2,
            &ExportSettings {
                unit_size: 0.5,
                z_size: 0.1,
                include_backplanes: false,
            },
            &[Segment {
                start_x: 0,
                start_y: 0,
                width: 1,
                height: 1,
            }],
        );

        let imported = import_scene(&scene).expect("scene should import");

        assert!(!imported.export.include_backplanes);
    }

    #[test]
    fn importer_rejects_unexpected_scene_structure() {
        let scene = "[gd_scene format=3]\n\n[node name=\"Root\" type=\"Node2D\"]\n";

        assert!(matches!(
            import_scene(scene),
            Err(ImportError::Unsupported(_))
        ));
    }
}
