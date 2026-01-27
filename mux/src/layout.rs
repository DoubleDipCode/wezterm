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
}
