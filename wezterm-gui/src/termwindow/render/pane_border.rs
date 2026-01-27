//! Pane border rendering for Claude Code status visualization.
//!
//! This module provides infrastructure for rendering colored borders around
//! terminal panes to indicate Claude Code status (Idle, Running, AwaitingPermission, Error).

use crate::quad::TripleLayerQuadAllocator;
use ::window::RectF;
use mux::tab::PositionedPane;

/// Represents a quad (rectangle) for border rendering.
///
/// Each quad has position (x, y, width, height) defining the rectangular area.
#[derive(Debug, Clone, PartialEq)]
pub struct BorderQuad {
    /// X coordinate of top-left corner (pixels)
    pub x: f32,
    /// Y coordinate of top-left corner (pixels)
    pub y: f32,
    /// Width of the quad (pixels)
    pub width: f32,
    /// Height of the quad (pixels)
    pub height: f32,
}

impl BorderQuad {
    /// Creates a new BorderQuad with the given position and dimensions.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    /// Converts this BorderQuad to a RectF for rendering.
    pub fn to_rect(&self) -> RectF {
        euclid::rect(self.x, self.y, self.width, self.height)
    }
}

/// Calculates the four border quads that form a border around a pane.
///
/// The border is rendered as 4 rectangles (top, right, bottom, left) that
/// form a frame around the pane content area.
///
/// # Arguments
///
/// * `pane_x` - X coordinate of pane top-left corner (pixels)
/// * `pane_y` - Y coordinate of pane top-left corner (pixels)
/// * `pane_width` - Width of the pane (pixels)
/// * `pane_height` - Height of the pane (pixels)
/// * `border_width` - Width of the border in pixels
///
/// # Returns
///
/// A vector of 4 BorderQuads in order: [top, right, bottom, left]
///
/// # Layout
///
/// ```text
/// +-------------------+  <- top border (full width)
/// |                   |
/// | +---------------+ |
/// | |               | |
/// | |   content     | |  <- left/right borders (inner height)
/// | |               | |
/// | +---------------+ |
/// |                   |
/// +-------------------+  <- bottom border (full width)
/// ```
pub fn calculate_border_quads(
    pane_x: f32,
    pane_y: f32,
    pane_width: f32,
    pane_height: f32,
    border_width: f32,
) -> Vec<BorderQuad> {
    // Top border: full width, at top
    let top = BorderQuad::new(pane_x, pane_y, pane_width, border_width);

    // Bottom border: full width, at bottom
    let bottom = BorderQuad::new(
        pane_x,
        pane_y + pane_height - border_width,
        pane_width,
        border_width,
    );

    // Left border: between top and bottom borders
    let left = BorderQuad::new(
        pane_x,
        pane_y + border_width,
        border_width,
        pane_height - 2.0 * border_width,
    );

    // Right border: between top and bottom borders
    let right = BorderQuad::new(
        pane_x + pane_width - border_width,
        pane_y + border_width,
        border_width,
        pane_height - 2.0 * border_width,
    );

    vec![top, right, bottom, left]
}

impl crate::TermWindow {
    /// Renders a colored border around a pane based on its Claude Code status.
    ///
    /// This function is called after terminal content rendering to overlay
    /// status-indicating borders on top of the pane content.
    ///
    /// # Arguments
    ///
    /// * `layers` - The quad allocator for rendering
    /// * `pos` - The positioned pane to render the border for
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if rendering fails.
    pub fn render_pane_border(
        &mut self,
        _layers: &mut TripleLayerQuadAllocator,
        _pos: &PositionedPane,
    ) -> anyhow::Result<()> {
        // Stub implementation - currently renders nothing.
        // Future iterations will:
        // 1. Get the pane's ClaudeStatus
        // 2. Map status to color via StatusColor::from_status()
        // 3. Calculate border quads around the pane
        // 4. Render the colored border overlay
        Ok(())
    }

    /// Renders Claude Code status borders for all visible panes.
    ///
    /// This is the main entry point called from the paint pass after
    /// all pane content has been rendered.
    ///
    /// # Arguments
    ///
    /// * `layers` - The quad allocator for rendering
    /// * `panes` - The list of positioned panes to render borders for
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if rendering fails.
    pub fn render_pane_borders(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        panes: &[PositionedPane],
    ) -> anyhow::Result<()> {
        for pos in panes {
            self.render_pane_border(layers, pos)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test calculate_border_quads with a 100x100 pane and 4px border
    /// as specified in the acceptance criteria.
    #[test]
    fn test_calculate_border_quads_100x100_with_4px_border() {
        let quads = calculate_border_quads(0.0, 0.0, 100.0, 100.0, 4.0);

        assert_eq!(quads.len(), 4, "Should return exactly 4 quads");

        // Top border: (0, 0) with size (100, 4)
        let top = &quads[0];
        assert_eq!(top.x, 0.0);
        assert_eq!(top.y, 0.0);
        assert_eq!(top.width, 100.0);
        assert_eq!(top.height, 4.0);

        // Right border: (96, 4) with size (4, 92)
        let right = &quads[1];
        assert_eq!(right.x, 96.0);
        assert_eq!(right.y, 4.0);
        assert_eq!(right.width, 4.0);
        assert_eq!(right.height, 92.0);

        // Bottom border: (0, 96) with size (100, 4)
        let bottom = &quads[2];
        assert_eq!(bottom.x, 0.0);
        assert_eq!(bottom.y, 96.0);
        assert_eq!(bottom.width, 100.0);
        assert_eq!(bottom.height, 4.0);

        // Left border: (0, 4) with size (4, 92)
        let left = &quads[3];
        assert_eq!(left.x, 0.0);
        assert_eq!(left.y, 4.0);
        assert_eq!(left.width, 4.0);
        assert_eq!(left.height, 92.0);
    }

    /// Test that quads form a complete frame without gaps or overlaps
    #[test]
    fn test_border_quads_coverage() {
        let quads = calculate_border_quads(0.0, 0.0, 100.0, 100.0, 4.0);

        // Verify top and bottom span full width
        assert_eq!(quads[0].width, 100.0, "Top border should span full width");
        assert_eq!(quads[2].width, 100.0, "Bottom border should span full width");

        // Verify left and right borders connect to top/bottom without overlap
        let inner_height = quads[1].height;
        assert_eq!(inner_height, 92.0, "Side borders should be height - 2*border_width");

        // Top + side + bottom heights should equal total height
        let total_vertical = quads[0].height + inner_height + quads[2].height;
        assert_eq!(total_vertical, 100.0, "Vertical coverage should equal pane height");
    }

    /// Test with non-zero pane position
    #[test]
    fn test_border_quads_with_offset() {
        let quads = calculate_border_quads(50.0, 30.0, 200.0, 150.0, 4.0);

        // Top border should be at pane position
        assert_eq!(quads[0].x, 50.0);
        assert_eq!(quads[0].y, 30.0);
        assert_eq!(quads[0].width, 200.0);

        // Right border should be at pane_x + pane_width - border_width
        assert_eq!(quads[1].x, 246.0); // 50 + 200 - 4

        // Bottom border should be at pane_y + pane_height - border_width
        assert_eq!(quads[2].y, 176.0); // 30 + 150 - 4
    }

    /// Test BorderQuad::to_rect() conversion
    #[test]
    fn test_border_quad_to_rect() {
        let quad = BorderQuad::new(10.0, 20.0, 30.0, 40.0);
        let rect = quad.to_rect();

        assert_eq!(rect.origin.x, 10.0);
        assert_eq!(rect.origin.y, 20.0);
        assert_eq!(rect.size.width, 30.0);
        assert_eq!(rect.size.height, 40.0);
    }

    /// Test with different border widths
    #[test]
    fn test_border_quads_different_widths() {
        // 2px border
        let quads_2px = calculate_border_quads(0.0, 0.0, 100.0, 100.0, 2.0);
        assert_eq!(quads_2px[0].height, 2.0);
        assert_eq!(quads_2px[1].height, 96.0); // 100 - 2*2

        // 8px border
        let quads_8px = calculate_border_quads(0.0, 0.0, 100.0, 100.0, 8.0);
        assert_eq!(quads_8px[0].height, 8.0);
        assert_eq!(quads_8px[1].height, 84.0); // 100 - 2*8
    }
}
