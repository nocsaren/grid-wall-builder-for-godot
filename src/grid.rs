//! Grid model and rectangle-merging logic.
//!
//! The UI paints individual cells on a discrete grid. When exporting, adjacent
//! filled cells are merged into rectangular segments to reduce the number of
//! nodes/shapes created in the Godot scene.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// A merged rectangle of filled cells (in grid coordinates).
pub struct Segment {
    pub start_x: usize,
    pub start_y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Clone, Debug)]
/// A boolean occupancy grid.
///
/// Storage is `[x][y]` to match how the UI accesses cells.
pub struct Grid {
    width: usize,
    height: usize,
    // Stored as [x][y] to match the original implementation.
    cells: Vec<Vec<bool>>,
}

impl Grid {
    /// Create a new empty grid.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![vec![false; height]; width],
        }
    }

    /// Grid width in cells.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Grid height in cells.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Mutable access to the cell storage.
    ///
    /// Indexing is `[x][y]`.
    pub fn cells_mut(&mut self) -> &mut Vec<Vec<bool>> {
        &mut self.cells
    }

    /// Resize the grid, preserving any overlapping painted cells.
    pub fn resize_preserve(&mut self, new_width: usize, new_height: usize) {
        let mut next_cells = vec![vec![false; new_height]; new_width];
        let copy_w = self.width.min(new_width);
        let copy_h = self.height.min(new_height);

        for x in 0..copy_w {
            for y in 0..copy_h {
                next_cells[x][y] = self.cells[x][y];
            }
        }

        self.width = new_width;
        self.height = new_height;
        self.cells = next_cells;
    }

    /// Merge adjacent filled cells into rectangles.
    ///
    /// Current strategy: scan left-to-right, top-to-bottom; expand width first,
    /// then expand height as long as the full rectangle is filled.
    pub fn collect_segments(&self) -> Vec<Segment> {
        let mut segments = Vec::new();
        let mut visited = vec![vec![false; self.height]; self.width];

        for x in 0..self.width {
            for y in 0..self.height {
                if !self.cells[x][y] || visited[x][y] {
                    continue;
                }

                let start_x = x;
                let start_y = y;

                let mut width = 0;
                while start_x + width < self.width
                    && self.cells[start_x + width][start_y]
                    && !visited[start_x + width][start_y]
                {
                    width += 1;
                }

                let mut height = 1;
                'expand: loop {
                    if start_y + height >= self.height {
                        break;
                    }

                    for dx in 0..width {
                        if !self.cells[start_x + dx][start_y + height]
                            || visited[start_x + dx][start_y + height]
                        {
                            break 'expand;
                        }
                    }

                    height += 1;
                }

                for dx in 0..width {
                    for dy in 0..height {
                        visited[start_x + dx][start_y + dy] = true;
                    }
                }

                segments.push(Segment {
                    start_x,
                    start_y,
                    width,
                    height,
                });
            }
        }

        segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_single_cell() {
        let mut grid = Grid::new(2, 2);
        grid.cells_mut()[0][0] = true;
        let segments = grid.collect_segments();
        assert_eq!(
            segments,
            vec![Segment {
                start_x: 0,
                start_y: 0,
                width: 1,
                height: 1,
            }]
        );
    }

    #[test]
    fn merge_rectangle() {
        let mut grid = Grid::new(4, 4);
        for x in 1..3 {
            for y in 0..2 {
                grid.cells_mut()[x][y] = true;
            }
        }

        let segments = grid.collect_segments();
        assert_eq!(
            segments,
            vec![Segment {
                start_x: 1,
                start_y: 0,
                width: 2,
                height: 2,
            }]
        );
    }
}
