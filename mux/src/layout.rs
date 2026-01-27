//! Auto-tiling layout for Claude Terminal
//!
//! This module provides automatic window tiling functionality for managing
//! multiple panes in a grid layout.

use crate::pane::PaneId;
use std::collections::HashSet;

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
