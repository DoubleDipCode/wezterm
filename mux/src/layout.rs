//! Auto-tiling layout for Claude Terminal
//!
//! This module provides automatic window tiling functionality for managing
//! multiple panes in a grid layout.

use crate::pane::PaneId;
use std::collections::{HashMap, HashSet};

/// Window size in pixels
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Rectangle in pixel coordinates
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }
}

/// TilingLayout manages automatic pane arrangement in a grid layout.
///
/// Panes are arranged in an optimal grid based on their count.
/// Locked panes are excluded from automatic layout calculations.
#[derive(Debug, Clone)]
pub struct TilingLayout {
    /// Ordered list of pane IDs in the layout
    pub panes: Vec<PaneId>,
    /// Set of pane IDs that are locked (excluded from auto-tiling)
    pub locked: HashSet<PaneId>,
}

impl TilingLayout {
    /// Create a new empty TilingLayout
    pub fn new() -> Self {
        Self {
            panes: Vec::new(),
            locked: HashSet::new(),
        }
    }

    /// Calculate optimal grid dimensions (cols, rows) for n panes.
    ///
    /// Returns a tuple of (columns, rows) that provides the best layout
    /// for the given number of panes:
    /// - 1 pane: 1x1
    /// - 2 panes: 2x1 (side by side)
    /// - 3-4 panes: 2x2 grid
    /// - 5-6 panes: 3x2 grid
    /// - 7-9 panes: 3x3 grid
    /// - 10+ panes: sqrt-based calculation
    pub fn grid_dimensions(n: usize) -> (usize, usize) {
        match n {
            0 => (0, 0),
            1 => (1, 1),
            2 => (2, 1),
            3..=4 => (2, 2),
            5..=6 => (3, 2),
            7..=9 => (3, 3),
            _ => {
                // For larger counts, use ceiling of sqrt for columns,
                // then calculate rows to fit all panes
                let cols = (n as f64).sqrt().ceil() as usize;
                let rows = (n + cols - 1) / cols; // Ceiling division
                (cols, rows)
            }
        }
    }

    /// Calculate pane rectangles for equal-size grid tiling.
    ///
    /// Returns a HashMap mapping each pane ID to its calculated Rect position.
    /// Panes in the `locked` set are excluded from calculations.
    /// When the last row has fewer panes than columns, those panes expand
    /// to fill the available width evenly.
    pub fn calculate(&self, window_size: Size) -> HashMap<PaneId, Rect> {
        let mut result = HashMap::new();

        // Filter out locked panes
        let unlocked_panes: Vec<PaneId> = self
            .panes
            .iter()
            .filter(|id| !self.locked.contains(id))
            .copied()
            .collect();

        let n = unlocked_panes.len();
        if n == 0 {
            return result;
        }

        let (cols, rows) = Self::grid_dimensions(n);
        if cols == 0 || rows == 0 {
            return result;
        }

        let cell_width = window_size.width / cols as u32;
        let cell_height = window_size.height / rows as u32;

        for (i, pane_id) in unlocked_panes.iter().enumerate() {
            let row = i / cols;
            let col = i % cols;

            // Calculate how many panes are in this row
            let panes_in_last_row = n - (rows - 1) * cols;
            let is_last_row = row == rows - 1;
            let panes_in_this_row = if is_last_row { panes_in_last_row } else { cols };

            // For uneven last row, expand panes to fill width
            let (pane_width, pane_x) = if is_last_row && panes_in_this_row < cols {
                // Expand panes in last row to fill entire width
                let expanded_width = window_size.width / panes_in_this_row as u32;
                let x = col as u32 * expanded_width;
                // Handle remainder pixels for last pane in row
                let width = if col == panes_in_this_row - 1 {
                    window_size.width - x
                } else {
                    expanded_width
                };
                (width, x)
            } else {
                // Normal cell width
                let x = col as u32 * cell_width;
                // Handle remainder pixels for last column
                let width = if col == cols - 1 {
                    window_size.width - x
                } else {
                    cell_width
                };
                (width, x)
            };

            let y = row as u32 * cell_height;
            // Handle remainder pixels for last row
            let pane_height = if row == rows - 1 {
                window_size.height - y
            } else {
                cell_height
            };

            result.insert(*pane_id, Rect::new(pane_x, y, pane_width, pane_height));
        }

        result
    }
}

impl Default for TilingLayout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_empty_layout() {
        let layout = TilingLayout::new();
        assert!(layout.panes.is_empty());
        assert!(layout.locked.is_empty());
    }

    #[test]
    fn test_default_creates_empty_layout() {
        let layout = TilingLayout::default();
        assert!(layout.panes.is_empty());
        assert!(layout.locked.is_empty());
    }

    // Tests for Size and Rect
    #[test]
    fn test_size_new() {
        let size = Size::new(800, 600);
        assert_eq!(size.width, 800);
        assert_eq!(size.height, 600);
    }

    #[test]
    fn test_rect_new() {
        let rect = Rect::new(10, 20, 100, 200);
        assert_eq!(rect.x, 10);
        assert_eq!(rect.y, 20);
        assert_eq!(rect.width, 100);
        assert_eq!(rect.height, 200);
    }

    // Tests for calculate() method
    #[test]
    fn test_calculate_empty_layout() {
        let layout = TilingLayout::new();
        let result = layout.calculate(Size::new(800, 600));
        assert!(result.is_empty());
    }

    #[test]
    fn test_calculate_single_pane() {
        let mut layout = TilingLayout::new();
        layout.panes.push(1);

        let result = layout.calculate(Size::new(800, 600));

        assert_eq!(result.len(), 1);
        let rect = result.get(&1).unwrap();
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.width, 800);
        assert_eq!(rect.height, 600);
    }

    #[test]
    fn test_calculate_two_panes_side_by_side() {
        let mut layout = TilingLayout::new();
        layout.panes.push(1);
        layout.panes.push(2);

        let result = layout.calculate(Size::new(800, 600));

        assert_eq!(result.len(), 2);

        let rect1 = result.get(&1).unwrap();
        assert_eq!(rect1.x, 0);
        assert_eq!(rect1.y, 0);
        assert_eq!(rect1.width, 400);
        assert_eq!(rect1.height, 600);

        let rect2 = result.get(&2).unwrap();
        assert_eq!(rect2.x, 400);
        assert_eq!(rect2.y, 0);
        assert_eq!(rect2.width, 400);
        assert_eq!(rect2.height, 600);
    }

    #[test]
    fn test_calculate_four_panes_800x600() {
        // Per acceptance criteria: 800x600 window, 4 panes
        let mut layout = TilingLayout::new();
        layout.panes.push(1);
        layout.panes.push(2);
        layout.panes.push(3);
        layout.panes.push(4);

        let result = layout.calculate(Size::new(800, 600));

        // 4 panes = 2x2 grid
        // Cell size: 400x300
        assert_eq!(result.len(), 4);

        // Pane 1: top-left
        let rect1 = result.get(&1).unwrap();
        assert_eq!(rect1.x, 0);
        assert_eq!(rect1.y, 0);
        assert_eq!(rect1.width, 400);
        assert_eq!(rect1.height, 300);

        // Pane 2: top-right
        let rect2 = result.get(&2).unwrap();
        assert_eq!(rect2.x, 400);
        assert_eq!(rect2.y, 0);
        assert_eq!(rect2.width, 400);
        assert_eq!(rect2.height, 300);

        // Pane 3: bottom-left
        let rect3 = result.get(&3).unwrap();
        assert_eq!(rect3.x, 0);
        assert_eq!(rect3.y, 300);
        assert_eq!(rect3.width, 400);
        assert_eq!(rect3.height, 300);

        // Pane 4: bottom-right
        let rect4 = result.get(&4).unwrap();
        assert_eq!(rect4.x, 400);
        assert_eq!(rect4.y, 300);
        assert_eq!(rect4.width, 400);
        assert_eq!(rect4.height, 300);
    }

    #[test]
    fn test_calculate_three_panes_uneven_last_row() {
        // 3 panes = 2x2 grid, but last row only has 1 pane
        // Last row pane should expand to fill entire width
        let mut layout = TilingLayout::new();
        layout.panes.push(1);
        layout.panes.push(2);
        layout.panes.push(3);

        let result = layout.calculate(Size::new(800, 600));

        assert_eq!(result.len(), 3);

        // Pane 1: top-left (normal width)
        let rect1 = result.get(&1).unwrap();
        assert_eq!(rect1.x, 0);
        assert_eq!(rect1.y, 0);
        assert_eq!(rect1.width, 400);
        assert_eq!(rect1.height, 300);

        // Pane 2: top-right (normal width)
        let rect2 = result.get(&2).unwrap();
        assert_eq!(rect2.x, 400);
        assert_eq!(rect2.y, 0);
        assert_eq!(rect2.width, 400);
        assert_eq!(rect2.height, 300);

        // Pane 3: bottom (expanded to full width)
        let rect3 = result.get(&3).unwrap();
        assert_eq!(rect3.x, 0);
        assert_eq!(rect3.y, 300);
        assert_eq!(rect3.width, 800);
        assert_eq!(rect3.height, 300);
    }

    #[test]
    fn test_calculate_five_panes_uneven_last_row() {
        // 5 panes = 3x2 grid, last row has 2 panes instead of 3
        // Both last row panes should expand to fill width evenly
        let mut layout = TilingLayout::new();
        layout.panes.push(1);
        layout.panes.push(2);
        layout.panes.push(3);
        layout.panes.push(4);
        layout.panes.push(5);

        let result = layout.calculate(Size::new(900, 600));

        assert_eq!(result.len(), 5);

        // First row: 3 panes at 300px each
        let rect1 = result.get(&1).unwrap();
        assert_eq!(rect1.x, 0);
        assert_eq!(rect1.width, 300);
        assert_eq!(rect1.height, 300);

        let rect2 = result.get(&2).unwrap();
        assert_eq!(rect2.x, 300);
        assert_eq!(rect2.width, 300);

        let rect3 = result.get(&3).unwrap();
        assert_eq!(rect3.x, 600);
        assert_eq!(rect3.width, 300);

        // Second row: 2 panes at 450px each (900/2)
        let rect4 = result.get(&4).unwrap();
        assert_eq!(rect4.x, 0);
        assert_eq!(rect4.y, 300);
        assert_eq!(rect4.width, 450);
        assert_eq!(rect4.height, 300);

        let rect5 = result.get(&5).unwrap();
        assert_eq!(rect5.x, 450);
        assert_eq!(rect5.y, 300);
        assert_eq!(rect5.width, 450);
        assert_eq!(rect5.height, 300);
    }

    #[test]
    fn test_calculate_excludes_locked_panes() {
        let mut layout = TilingLayout::new();
        layout.panes.push(1);
        layout.panes.push(2);
        layout.panes.push(3);
        layout.locked.insert(2); // Lock pane 2

        let result = layout.calculate(Size::new(800, 600));

        // Only 2 panes (1 and 3) should be in result
        assert_eq!(result.len(), 2);
        assert!(result.contains_key(&1));
        assert!(!result.contains_key(&2)); // Locked, not included
        assert!(result.contains_key(&3));

        // With 2 unlocked panes, should be side by side (2x1)
        let rect1 = result.get(&1).unwrap();
        assert_eq!(rect1.width, 400);

        let rect3 = result.get(&3).unwrap();
        assert_eq!(rect3.x, 400);
        assert_eq!(rect3.width, 400);
    }

    #[test]
    fn test_calculate_covers_entire_window() {
        // Verify all rects together cover the entire window with no gaps
        let mut layout = TilingLayout::new();
        for i in 1..=5 {
            layout.panes.push(i);
        }

        let window_size = Size::new(800, 600);
        let result = layout.calculate(window_size);

        // Check that total area equals window area
        let total_area: u64 = result
            .values()
            .map(|r| (r.width as u64) * (r.height as u64))
            .sum();
        assert_eq!(total_area, 800 * 600);
    }

    #[test]
    fn test_calculate_handles_remainder_pixels() {
        // Window size not evenly divisible by grid
        let mut layout = TilingLayout::new();
        layout.panes.push(1);
        layout.panes.push(2);

        let result = layout.calculate(Size::new(801, 601));

        // Total width should equal 801
        let rect1 = result.get(&1).unwrap();
        let rect2 = result.get(&2).unwrap();
        assert_eq!(rect1.width + rect2.width, 801);

        // Heights should equal 601
        assert_eq!(rect1.height, 601);
        assert_eq!(rect2.height, 601);
    }

    #[test]
    fn test_grid_dimensions_zero() {
        assert_eq!(TilingLayout::grid_dimensions(0), (0, 0));
    }

    #[test]
    fn test_grid_dimensions_one() {
        assert_eq!(TilingLayout::grid_dimensions(1), (1, 1));
    }

    #[test]
    fn test_grid_dimensions_two() {
        assert_eq!(TilingLayout::grid_dimensions(2), (2, 1));
    }

    #[test]
    fn test_grid_dimensions_three() {
        assert_eq!(TilingLayout::grid_dimensions(3), (2, 2));
    }

    #[test]
    fn test_grid_dimensions_four() {
        assert_eq!(TilingLayout::grid_dimensions(4), (2, 2));
    }

    #[test]
    fn test_grid_dimensions_five() {
        assert_eq!(TilingLayout::grid_dimensions(5), (3, 2));
    }

    #[test]
    fn test_grid_dimensions_six() {
        assert_eq!(TilingLayout::grid_dimensions(6), (3, 2));
    }

    #[test]
    fn test_grid_dimensions_seven() {
        assert_eq!(TilingLayout::grid_dimensions(7), (3, 3));
    }

    #[test]
    fn test_grid_dimensions_eight() {
        assert_eq!(TilingLayout::grid_dimensions(8), (3, 3));
    }

    #[test]
    fn test_grid_dimensions_nine() {
        assert_eq!(TilingLayout::grid_dimensions(9), (3, 3));
    }

    #[test]
    fn test_grid_dimensions_ten() {
        // 10 panes: sqrt(10) ≈ 3.16, ceil = 4 cols
        // rows = ceil(10/4) = 3
        assert_eq!(TilingLayout::grid_dimensions(10), (4, 3));
    }

    #[test]
    fn test_grid_dimensions_large() {
        // 16 panes: sqrt(16) = 4, perfect 4x4
        assert_eq!(TilingLayout::grid_dimensions(16), (4, 4));

        // 20 panes: sqrt(20) ≈ 4.47, ceil = 5 cols
        // rows = ceil(20/5) = 4
        assert_eq!(TilingLayout::grid_dimensions(20), (5, 4));
    }

    #[test]
    fn test_grid_dimensions_fits_all_panes() {
        // Verify that cols * rows >= n for various values
        for n in 1..=25 {
            let (cols, rows) = TilingLayout::grid_dimensions(n);
            assert!(
                cols * rows >= n,
                "Grid {}x{} cannot fit {} panes",
                cols,
                rows,
                n
            );
        }
    }
}
