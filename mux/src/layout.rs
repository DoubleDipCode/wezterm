//! Auto-tiling layout for Claude Terminal
//!
//! This module provides automatic window tiling functionality for managing
//! multiple panes in a grid layout.

use crate::pane::PaneId;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Default animation duration in milliseconds (used when no config available)
pub const DEFAULT_DEFAULT_ANIMATION_DURATION_MS: u64 = 150;

/// Default minimum pane width in columns
pub const DEFAULT_MIN_COLS: u32 = 80;
/// Default minimum pane height in rows
pub const DEFAULT_MIN_ROWS: u32 = 24;

/// Minimum pane size constraints for tiling layout
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneConstraints {
    /// Minimum pane width in columns of text
    pub min_cols: u32,
    /// Minimum pane height in rows of text
    pub min_rows: u32,
    /// Width of a single character cell in pixels
    pub cell_width: u32,
    /// Height of a single character cell in pixels
    pub cell_height: u32,
}

impl PaneConstraints {
    /// Create new pane constraints
    pub fn new(min_cols: u32, min_rows: u32, cell_width: u32, cell_height: u32) -> Self {
        Self {
            min_cols,
            min_rows,
            cell_width,
            cell_height,
        }
    }

    /// Calculate minimum pane width in pixels
    pub fn min_width_px(&self) -> u32 {
        self.min_cols * self.cell_width
    }

    /// Calculate minimum pane height in pixels
    pub fn min_height_px(&self) -> u32 {
        self.min_rows * self.cell_height
    }
}

impl Default for PaneConstraints {
    fn default() -> Self {
        // Default to 80x24 with 8x16 pixel cells (common terminal font size)
        Self {
            min_cols: DEFAULT_MIN_COLS,
            min_rows: DEFAULT_MIN_ROWS,
            cell_width: 8,
            cell_height: 16,
        }
    }
}

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

/// Result of layout calculation with constraint information
#[derive(Debug, Clone)]
pub struct LayoutResult {
    /// Map of pane IDs to their calculated rectangles
    pub panes: HashMap<PaneId, Rect>,
    /// Pane IDs that were hidden due to size constraints (overflow)
    pub hidden: Vec<PaneId>,
    /// Maximum number of panes that can fit given constraints
    pub max_visible: usize,
}

impl LayoutResult {
    /// Create a new empty layout result
    pub fn new() -> Self {
        Self {
            panes: HashMap::new(),
            hidden: Vec::new(),
            max_visible: 0,
        }
    }
}

impl Default for LayoutResult {
    fn default() -> Self {
        Self::new()
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

    /// Calculate maximum number of panes that can fit given constraints.
    ///
    /// This performs a binary search to find the largest N such that
    /// grid_dimensions(N) produces cells that meet the minimum size requirements.
    pub fn max_panes_for_size(window_size: Size, constraints: &PaneConstraints) -> usize {
        let min_width = constraints.min_width_px();
        let min_height = constraints.min_height_px();

        // Edge case: window too small for even 1 pane
        if window_size.width < min_width || window_size.height < min_height {
            return 0;
        }

        // Start at a reasonable upper bound and search downward
        // Max theoretically possible panes based on minimum sizes
        let max_cols = window_size.width / min_width;
        let max_rows = window_size.height / min_height;
        let theoretical_max = (max_cols * max_rows) as usize;

        // Find the largest N where all panes meet minimum size
        for n in (1..=theoretical_max.max(10)).rev() {
            let (cols, rows) = Self::grid_dimensions(n);
            if cols == 0 || rows == 0 {
                continue;
            }

            let cell_width = window_size.width / cols as u32;
            let cell_height = window_size.height / rows as u32;

            if cell_width >= min_width && cell_height >= min_height {
                return n;
            }
        }

        // If nothing fits, return 0
        0
    }

    /// Calculate pane rectangles with minimum size constraints.
    ///
    /// This method respects minimum pane sizes: if there are too many panes
    /// to fit at the minimum size, overflow panes are marked as hidden.
    ///
    /// Returns a LayoutResult containing:
    /// - `panes`: HashMap of visible pane rectangles
    /// - `hidden`: Vec of pane IDs that couldn't fit (overflow)
    /// - `max_visible`: Maximum panes that fit given constraints
    pub fn calculate_with_constraints(
        &self,
        window_size: Size,
        constraints: &PaneConstraints,
    ) -> LayoutResult {
        let mut result = LayoutResult::new();

        // Filter out locked panes
        let unlocked_panes: Vec<PaneId> = self
            .panes
            .iter()
            .filter(|id| !self.locked.contains(id))
            .copied()
            .collect();

        let total_panes = unlocked_panes.len();
        if total_panes == 0 {
            return result;
        }

        // Calculate max panes that can fit
        let max_visible = Self::max_panes_for_size(window_size, constraints);
        result.max_visible = max_visible;

        if max_visible == 0 {
            // Window too small for any panes - all hidden
            result.hidden = unlocked_panes;
            return result;
        }

        // Split panes into visible and hidden
        let visible_count = total_panes.min(max_visible);
        let visible_panes: Vec<PaneId> = unlocked_panes.iter().take(visible_count).copied().collect();
        let hidden_panes: Vec<PaneId> = unlocked_panes.iter().skip(visible_count).copied().collect();

        result.hidden = hidden_panes;

        // Now calculate layout for visible panes only
        let n = visible_panes.len();
        let (cols, rows) = Self::grid_dimensions(n);
        if cols == 0 || rows == 0 {
            return result;
        }

        let cell_width = window_size.width / cols as u32;
        let cell_height = window_size.height / rows as u32;

        for (i, pane_id) in visible_panes.iter().enumerate() {
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

            result.panes.insert(*pane_id, Rect::new(pane_x, y, pane_width, pane_height));
        }

        result
    }
}

impl Default for TilingLayout {
    fn default() -> Self {
        Self::new()
    }
}

/// Animated rectangle with floating point coordinates for smooth interpolation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimatedRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl AnimatedRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    /// Create an AnimatedRect from a Rect (integer coordinates)
    pub fn from_rect(rect: &Rect) -> Self {
        Self {
            x: rect.x as f32,
            y: rect.y as f32,
            width: rect.width as f32,
            height: rect.height as f32,
        }
    }

    /// Interpolate between two rectangles using a 0.0-1.0 progress value
    pub fn lerp(from: &AnimatedRect, to: &AnimatedRect, t: f32) -> Self {
        Self {
            x: from.x + (to.x - from.x) * t,
            y: from.y + (to.y - from.y) * t,
            width: from.width + (to.width - from.width) * t,
            height: from.height + (to.height - from.height) * t,
        }
    }
}

/// Stores animation state for smooth pane resize transitions
#[derive(Debug, Clone)]
pub struct LayoutAnimation {
    /// Starting positions of panes (before layout change)
    pub from_positions: HashMap<PaneId, AnimatedRect>,
    /// Target positions of panes (after layout change)
    pub to_positions: HashMap<PaneId, AnimatedRect>,
    /// When the animation started
    pub start_time: Instant,
    /// Animation duration
    pub duration: Duration,
}

impl LayoutAnimation {
    /// Create a new layout animation with specified duration
    pub fn new(
        from_positions: HashMap<PaneId, AnimatedRect>,
        to_positions: HashMap<PaneId, AnimatedRect>,
        duration_ms: u64,
    ) -> Self {
        Self {
            from_positions,
            to_positions,
            start_time: Instant::now(),
            duration: Duration::from_millis(duration_ms),
        }
    }

    /// Create a new layout animation with default duration
    pub fn with_default_duration(
        from_positions: HashMap<PaneId, AnimatedRect>,
        to_positions: HashMap<PaneId, AnimatedRect>,
    ) -> Self {
        Self::new(from_positions, to_positions, DEFAULT_DEFAULT_ANIMATION_DURATION_MS)
    }

    /// Calculate the eased progress value using ease-out cubic: t = 1 - (1-t)^3
    fn ease_out_cubic(t: f32) -> f32 {
        let t_inv = 1.0 - t;
        1.0 - t_inv * t_inv * t_inv
    }

    /// Calculate raw linear progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        let elapsed = self.start_time.elapsed();
        if elapsed >= self.duration {
            1.0
        } else {
            elapsed.as_secs_f32() / self.duration.as_secs_f32()
        }
    }

    /// Calculate eased progress value
    pub fn eased_progress(&self) -> f32 {
        Self::ease_out_cubic(self.progress())
    }

    /// Check if the animation is complete
    pub fn is_complete(&self) -> bool {
        self.start_time.elapsed() >= self.duration
    }

    /// Get the current interpolated position for a pane
    pub fn current_position(&self, pane_id: PaneId) -> Option<AnimatedRect> {
        let from = self.from_positions.get(&pane_id)?;
        let to = self.to_positions.get(&pane_id)?;
        let t = self.eased_progress();
        Some(AnimatedRect::lerp(from, to, t))
    }

    /// Get remaining time until animation completes (for scheduling next frame)
    pub fn remaining_duration(&self) -> Duration {
        let elapsed = self.start_time.elapsed();
        if elapsed >= self.duration {
            Duration::ZERO
        } else {
            self.duration - elapsed
        }
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

    // Tests for PaneConstraints
    #[test]
    fn test_pane_constraints_new() {
        let constraints = PaneConstraints::new(80, 24, 8, 16);
        assert_eq!(constraints.min_cols, 80);
        assert_eq!(constraints.min_rows, 24);
        assert_eq!(constraints.cell_width, 8);
        assert_eq!(constraints.cell_height, 16);
    }

    #[test]
    fn test_pane_constraints_default() {
        let constraints = PaneConstraints::default();
        assert_eq!(constraints.min_cols, DEFAULT_MIN_COLS);
        assert_eq!(constraints.min_rows, DEFAULT_MIN_ROWS);
    }

    #[test]
    fn test_pane_constraints_min_dimensions() {
        let constraints = PaneConstraints::new(80, 24, 8, 16);
        // 80 cols * 8 px = 640 px minimum width
        assert_eq!(constraints.min_width_px(), 640);
        // 24 rows * 16 px = 384 px minimum height
        assert_eq!(constraints.min_height_px(), 384);
    }

    // Tests for max_panes_for_size
    #[test]
    fn test_max_panes_window_too_small() {
        // Window smaller than minimum single pane size
        let constraints = PaneConstraints::new(80, 24, 8, 16);
        // Min size: 640x384
        // Window: 500x300 - too small
        let max = TilingLayout::max_panes_for_size(Size::new(500, 300), &constraints);
        assert_eq!(max, 0);
    }

    #[test]
    fn test_max_panes_exactly_one() {
        let constraints = PaneConstraints::new(80, 24, 8, 16);
        // Min size: 640x384
        // Window: exactly min size, should fit exactly 1
        let max = TilingLayout::max_panes_for_size(Size::new(640, 384), &constraints);
        assert_eq!(max, 1);
    }

    #[test]
    fn test_max_panes_two_side_by_side() {
        let constraints = PaneConstraints::new(80, 24, 8, 16);
        // Min size: 640x384
        // Window: 1280x384 - can fit 2 side by side
        let max = TilingLayout::max_panes_for_size(Size::new(1280, 384), &constraints);
        assert_eq!(max, 2);
    }

    #[test]
    fn test_max_panes_four_in_grid() {
        let constraints = PaneConstraints::new(80, 24, 8, 16);
        // Min size: 640x384
        // Window: 1280x768 - can fit 2x2 grid
        let max = TilingLayout::max_panes_for_size(Size::new(1280, 768), &constraints);
        assert_eq!(max, 4);
    }

    #[test]
    fn test_max_panes_large_window() {
        let constraints = PaneConstraints::new(80, 24, 8, 16);
        // Min size: 640x384
        // Window: 1920x1080 - can fit multiple panes
        // 1920/640 = 3 cols, 1080/384 = 2.8 rows = 2 rows
        // With 3x2 grid, should fit 6 panes
        let max = TilingLayout::max_panes_for_size(Size::new(1920, 1080), &constraints);
        assert!(max >= 6, "Expected at least 6 panes, got {}", max);
    }

    // Tests for calculate_with_constraints
    #[test]
    fn test_calculate_with_constraints_all_fit() {
        let mut layout = TilingLayout::new();
        layout.panes.push(1);
        layout.panes.push(2);

        // Large window that can fit both panes
        let constraints = PaneConstraints::new(80, 24, 8, 16);
        let result = layout.calculate_with_constraints(Size::new(1600, 900), &constraints);

        assert_eq!(result.panes.len(), 2);
        assert!(result.hidden.is_empty());
        assert!(result.max_visible >= 2);
    }

    #[test]
    fn test_calculate_with_constraints_hides_overflow() {
        let mut layout = TilingLayout::new();
        for i in 1..=6 {
            layout.panes.push(i);
        }

        // Window that can only fit 2 panes (side by side)
        let constraints = PaneConstraints::new(80, 24, 8, 16);
        // 1280x384 can fit 2 panes (640 each wide, 384 tall)
        let result = layout.calculate_with_constraints(Size::new(1280, 384), &constraints);

        assert_eq!(result.panes.len(), 2);
        assert_eq!(result.hidden.len(), 4);
        assert_eq!(result.max_visible, 2);

        // First 2 panes visible
        assert!(result.panes.contains_key(&1));
        assert!(result.panes.contains_key(&2));

        // Rest are hidden
        assert!(result.hidden.contains(&3));
        assert!(result.hidden.contains(&4));
        assert!(result.hidden.contains(&5));
        assert!(result.hidden.contains(&6));
    }

    #[test]
    fn test_calculate_with_constraints_window_too_small() {
        let mut layout = TilingLayout::new();
        layout.panes.push(1);
        layout.panes.push(2);

        // Window too small for any pane
        let constraints = PaneConstraints::new(80, 24, 8, 16);
        let result = layout.calculate_with_constraints(Size::new(400, 200), &constraints);

        assert!(result.panes.is_empty());
        assert_eq!(result.hidden.len(), 2);
        assert_eq!(result.max_visible, 0);
    }

    #[test]
    fn test_calculate_with_constraints_visible_panes_correct() {
        let mut layout = TilingLayout::new();
        for i in 1..=4 {
            layout.panes.push(i);
        }

        // Window that can fit exactly 1 pane
        let constraints = PaneConstraints::new(80, 24, 8, 16);
        let result = layout.calculate_with_constraints(Size::new(640, 384), &constraints);

        assert_eq!(result.panes.len(), 1);
        assert_eq!(result.hidden.len(), 3);

        // First pane should take full window
        let rect = result.panes.get(&1).unwrap();
        assert_eq!(rect.width, 640);
        assert_eq!(rect.height, 384);
    }

    #[test]
    fn test_calculate_with_constraints_excludes_locked() {
        let mut layout = TilingLayout::new();
        layout.panes.push(1);
        layout.panes.push(2);
        layout.panes.push(3);
        layout.locked.insert(2);

        let constraints = PaneConstraints::new(80, 24, 8, 16);
        let result = layout.calculate_with_constraints(Size::new(1600, 900), &constraints);

        // Only 2 unlocked panes (1 and 3)
        assert_eq!(result.panes.len(), 2);
        assert!(result.panes.contains_key(&1));
        assert!(!result.panes.contains_key(&2)); // Locked
        assert!(result.panes.contains_key(&3));
        assert!(result.hidden.is_empty());
    }

    // Tests for LayoutResult
    #[test]
    fn test_layout_result_new() {
        let result = LayoutResult::new();
        assert!(result.panes.is_empty());
        assert!(result.hidden.is_empty());
        assert_eq!(result.max_visible, 0);
    }

    #[test]
    fn test_layout_result_default() {
        let result = LayoutResult::default();
        assert!(result.panes.is_empty());
        assert!(result.hidden.is_empty());
        assert_eq!(result.max_visible, 0);
    }

    // Tests for AnimatedRect
    #[test]
    fn test_animated_rect_new() {
        let rect = AnimatedRect::new(10.0, 20.0, 100.0, 200.0);
        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.y, 20.0);
        assert_eq!(rect.width, 100.0);
        assert_eq!(rect.height, 200.0);
    }

    #[test]
    fn test_animated_rect_from_rect() {
        let rect = Rect::new(10, 20, 100, 200);
        let animated = AnimatedRect::from_rect(&rect);
        assert_eq!(animated.x, 10.0);
        assert_eq!(animated.y, 20.0);
        assert_eq!(animated.width, 100.0);
        assert_eq!(animated.height, 200.0);
    }

    #[test]
    fn test_animated_rect_lerp_at_start() {
        let from = AnimatedRect::new(0.0, 0.0, 100.0, 100.0);
        let to = AnimatedRect::new(100.0, 100.0, 200.0, 200.0);
        let result = AnimatedRect::lerp(&from, &to, 0.0);
        assert_eq!(result.x, 0.0);
        assert_eq!(result.y, 0.0);
        assert_eq!(result.width, 100.0);
        assert_eq!(result.height, 100.0);
    }

    #[test]
    fn test_animated_rect_lerp_at_end() {
        let from = AnimatedRect::new(0.0, 0.0, 100.0, 100.0);
        let to = AnimatedRect::new(100.0, 100.0, 200.0, 200.0);
        let result = AnimatedRect::lerp(&from, &to, 1.0);
        assert_eq!(result.x, 100.0);
        assert_eq!(result.y, 100.0);
        assert_eq!(result.width, 200.0);
        assert_eq!(result.height, 200.0);
    }

    #[test]
    fn test_animated_rect_lerp_midpoint() {
        let from = AnimatedRect::new(0.0, 0.0, 100.0, 100.0);
        let to = AnimatedRect::new(100.0, 100.0, 200.0, 200.0);
        let result = AnimatedRect::lerp(&from, &to, 0.5);
        assert_eq!(result.x, 50.0);
        assert_eq!(result.y, 50.0);
        assert_eq!(result.width, 150.0);
        assert_eq!(result.height, 150.0);
    }

    // Tests for LayoutAnimation
    #[test]
    fn test_layout_animation_ease_out_cubic() {
        // t=0 should return 0
        assert_eq!(LayoutAnimation::ease_out_cubic(0.0), 0.0);
        // t=1 should return 1
        assert_eq!(LayoutAnimation::ease_out_cubic(1.0), 1.0);
        // t=0.5 should return more than 0.5 (ease-out accelerates at start)
        let mid = LayoutAnimation::ease_out_cubic(0.5);
        assert!(mid > 0.5, "ease_out_cubic(0.5) = {} should be > 0.5", mid);
        // Formula: 1 - (1-0.5)^3 = 1 - 0.125 = 0.875
        assert!((mid - 0.875).abs() < 0.001, "Expected 0.875, got {}", mid);
    }

    #[test]
    fn test_layout_animation_new() {
        let mut from = HashMap::new();
        from.insert(1, AnimatedRect::new(0.0, 0.0, 100.0, 100.0));
        let mut to = HashMap::new();
        to.insert(1, AnimatedRect::new(100.0, 0.0, 200.0, 100.0));

        let anim = LayoutAnimation::new(from.clone(), to.clone(), DEFAULT_ANIMATION_DURATION_MS);
        assert_eq!(anim.from_positions.len(), 1);
        assert_eq!(anim.to_positions.len(), 1);
        assert_eq!(anim.duration, Duration::from_millis(DEFAULT_ANIMATION_DURATION_MS));
    }

    #[test]
    fn test_layout_animation_with_custom_duration() {
        let from = HashMap::new();
        let to = HashMap::new();

        let anim = LayoutAnimation::new(from, to, 200);
        assert_eq!(anim.duration, Duration::from_millis(200));
    }

    #[test]
    fn test_layout_animation_with_default_duration() {
        let from = HashMap::new();
        let to = HashMap::new();

        let anim = LayoutAnimation::with_default_duration(from, to);
        assert_eq!(anim.duration, Duration::from_millis(DEFAULT_ANIMATION_DURATION_MS));
    }

    #[test]
    fn test_layout_animation_current_position() {
        let mut from = HashMap::new();
        from.insert(1, AnimatedRect::new(0.0, 0.0, 400.0, 600.0));
        let mut to = HashMap::new();
        to.insert(1, AnimatedRect::new(0.0, 0.0, 200.0, 300.0));

        let anim = LayoutAnimation::new(from, to, DEFAULT_ANIMATION_DURATION_MS);

        // Should return Some for existing pane
        assert!(anim.current_position(1).is_some());
        // Should return None for non-existing pane
        assert!(anim.current_position(999).is_none());
    }

    #[test]
    fn test_layout_animation_is_complete() {
        let from = HashMap::new();
        let to = HashMap::new();
        let mut anim = LayoutAnimation::new(from, to, DEFAULT_ANIMATION_DURATION_MS);

        // Animation shouldn't be complete immediately
        assert!(!anim.is_complete());

        // Set start time to past to simulate completion
        anim.start_time = Instant::now() - Duration::from_millis(DEFAULT_ANIMATION_DURATION_MS + 10);
        assert!(anim.is_complete());
    }
}
