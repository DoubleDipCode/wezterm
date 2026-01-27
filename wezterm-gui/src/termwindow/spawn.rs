use crate::overlay::{confirm_close_pane, start_overlay_pane};
use crate::spawn::SpawnWhere;
use config::keyassignment::{PaneDirection, SpawnCommand, SpawnTabDomain};
use config::TermConfig;
use mux::layout::TilingLayout;
use mux::pane::CloseReason;
use mux::tab::{SplitDirection, SplitRequest, SplitSize as MuxSplitSize};
use mux::Mux;
use std::sync::Arc;
use window::WindowOps;

impl super::TermWindow {
    pub fn spawn_command(&self, spawn: &SpawnCommand, spawn_where: SpawnWhere) {
        let size = if spawn_where == SpawnWhere::NewWindow {
            self.config.initial_size(
                self.dimensions.dpi as u32,
                crate::cell_pixel_dims(&self.config, self.dimensions.dpi as f64).ok(),
            )
        } else {
            self.terminal_size
        };
        let term_config = Arc::new(TermConfig::with_config(self.config.clone()));

        crate::spawn::spawn_command_impl(
            spawn,
            spawn_where,
            size,
            Some(self.mux_window_id),
            term_config,
        )
    }

    pub fn spawn_tab(&mut self, domain: &SpawnTabDomain) {
        self.spawn_command(
            &SpawnCommand {
                domain: domain.clone(),
                ..Default::default()
            },
            SpawnWhere::NewTab,
        );
    }

    /// Create a new pane using auto-tiling layout.
    /// This calculates the optimal split direction to achieve a grid layout,
    /// creates the pane, and updates the tiling state.
    pub fn auto_tile_new_pane(&mut self, spawn: &SpawnCommand) {
        let mux = Mux::get();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => {
                log::error!("auto_tile_new_pane: no active tab");
                return;
            }
        };

        // Get current pane count
        let panes = tab.iter_panes();
        let current_count = panes.len();

        // Calculate grid dimensions for n and n+1 panes
        let (current_cols, current_rows) = TilingLayout::grid_dimensions(current_count);
        let (new_cols, new_rows) = TilingLayout::grid_dimensions(current_count + 1);

        // Determine split direction based on grid change
        // If we need more columns, split horizontally
        // If we need more rows, split vertically
        let direction = if new_cols > current_cols {
            SplitDirection::Horizontal
        } else if new_rows > current_rows {
            SplitDirection::Vertical
        } else {
            // Grid dimensions didn't change, alternate based on pane position
            // For grids that are filling in, prefer horizontal splits
            if current_count % new_cols == 0 {
                SplitDirection::Vertical
            } else {
                SplitDirection::Horizontal
            }
        };

        // Calculate split size to achieve equal-size tiling
        let split_percent = self.calculate_auto_tile_split_percent(
            current_count,
            new_cols,
            new_rows,
            direction,
        );

        log::trace!(
            "auto_tile_new_pane: {} panes -> {} panes, grid {}x{} -> {}x{}, direction {:?}, split {}%",
            current_count,
            current_count + 1,
            current_cols,
            current_rows,
            new_cols,
            new_rows,
            direction,
            split_percent
        );

        // Update tiling layout state
        // Add existing panes to layout if not already tracked
        if self.tiling_layout.panes.is_empty() {
            for positioned in &panes {
                self.tiling_layout.panes.push(positioned.pane.pane_id());
            }
        }

        // Perform the split
        self.spawn_command(
            spawn,
            SpawnWhere::SplitPane(SplitRequest {
                direction,
                target_is_second: true,
                size: MuxSplitSize::Percent(split_percent),
                top_level: true, // Split from tab level for better grid layouts
            }),
        );

        // Schedule layout recalculation after pane is created
        self.schedule_auto_tile_layout();
    }

    /// Calculate the split percentage to achieve equal-size tiling
    fn calculate_auto_tile_split_percent(
        &self,
        _current_count: usize,
        new_cols: usize,
        new_rows: usize,
        direction: SplitDirection,
    ) -> u8 {
        // For a simple approach, calculate what percentage the new pane should be
        // to achieve roughly equal sizes
        match direction {
            SplitDirection::Horizontal => {
                // New column: split to give new pane 1/new_cols of width
                (100 / new_cols) as u8
            }
            SplitDirection::Vertical => {
                // New row: split to give new pane 1/new_rows of height
                (100 / new_rows) as u8
            }
        }
    }

    /// Schedule auto-tile layout recalculation
    fn schedule_auto_tile_layout(&self) {
        // Layout will be recalculated on next frame through the existing
        // TabResized notification mechanism
        if let Some(window) = &self.window {
            window.invalidate();
        }
    }

    /// Reset all panes to equal-size auto-tiled layout
    pub fn auto_tile_reset(&mut self) {
        // Clear locked panes
        self.tiling_layout.locked.clear();

        let mux = Mux::get();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => {
                log::error!("auto_tile_reset: no active tab");
                return;
            }
        };

        // Rebuild tiling layout from current panes
        let panes = tab.iter_panes();
        self.tiling_layout.panes.clear();
        for positioned in &panes {
            self.tiling_layout.panes.push(positioned.pane.pane_id());
        }

        // Trigger resize to apply equal layout
        // The resize() method will redistribute pane sizes
        tab.resize(self.terminal_size);

        if let Some(window) = &self.window {
            window.invalidate();
        }

        log::trace!(
            "auto_tile_reset: reset {} panes to equal layout",
            panes.len()
        );
    }

    /// Close the current pane and recalculate auto-tile layout for remaining panes.
    /// This removes the pane from the tiling layout, closes it, and triggers
    /// a layout recalculation so remaining panes fill the available space.
    pub fn auto_tile_close_pane(&mut self, confirm: bool) {
        let mux_window_id = self.mux_window_id;
        let mux = Mux::get();
        let tab = match mux.get_active_tab_for_window(mux_window_id) {
            Some(tab) => tab,
            None => {
                log::error!("auto_tile_close_pane: no active tab");
                return;
            }
        };
        let pane = match tab.get_active_pane() {
            Some(p) => p,
            None => {
                log::error!("auto_tile_close_pane: no active pane");
                return;
            }
        };

        let pane_id = pane.pane_id();

        // Remove from tiling layout tracking
        self.tiling_layout.panes.retain(|&id| id != pane_id);
        self.tiling_layout.locked.remove(&pane_id);

        log::trace!(
            "auto_tile_close_pane: closing pane {} (confirm={}), {} panes remaining",
            pane_id,
            confirm,
            self.tiling_layout.panes.len()
        );

        if confirm && !pane.can_close_without_prompting(CloseReason::Pane) {
            // Show confirmation dialog
            let window = self.window.clone().unwrap();
            let (overlay, future) = start_overlay_pane(self, &pane, move |pane_id, term| {
                confirm_close_pane(pane_id, term, mux_window_id, window)
            });
            self.assign_overlay_for_pane(pane_id, overlay);
            promise::spawn::spawn(future).detach();
        } else {
            // Close pane directly
            mux.remove_pane(pane_id);
        }

        // Schedule layout recalculation after pane is removed
        // The layout will be recalculated on next frame
        self.schedule_auto_tile_layout();
    }

    /// Resize the current pane manually and lock it from auto-tiling.
    /// This calls the standard pane resize function and then marks the
    /// pane as locked so it won't be affected by future auto-tiling operations.
    pub fn auto_tile_resize_pane(&mut self, direction: PaneDirection, amount: usize) {
        let mux = Mux::get();
        let tab = match mux.get_active_tab_for_window(self.mux_window_id) {
            Some(tab) => tab,
            None => {
                log::error!("auto_tile_resize_pane: no active tab");
                return;
            }
        };

        let pane = match tab.get_active_pane() {
            Some(p) => p,
            None => {
                log::error!("auto_tile_resize_pane: no active pane");
                return;
            }
        };

        let pane_id = pane.pane_id();
        let tab_id = tab.tab_id();

        // Check if there's an overlay - don't resize if overlay is active
        if self.tab_state(tab_id).overlay.is_some() {
            log::trace!("auto_tile_resize_pane: overlay active, skipping resize");
            return;
        }

        // Perform the resize using the standard pane resize function
        tab.adjust_pane_size(direction, amount);

        // Lock this pane from auto-tiling
        self.tiling_layout.locked.insert(pane_id);

        log::trace!(
            "auto_tile_resize_pane: resized pane {} {:?} by {} cells and locked it",
            pane_id,
            direction,
            amount
        );

        // Trigger a window invalidation to update the display
        if let Some(window) = self.window.as_ref() {
            window.invalidate();
        }
    }
}
