//! File browser overlay for Claude Terminal
//!
//! This module provides a file browser pane that displays the directory
//! contents for the focused terminal pane. It uses keyboard navigation
//! with vim-style bindings (j/k/h/l).

use mux::layout::Rect;
use std::path::PathBuf;

/// Parse an OSC 7 escape sequence to extract the current working directory.
///
/// OSC 7 format: `\x1b]7;file://hostname/path\x07` or `\x1b]7;file://hostname/path\x1b\\`
///
/// This function scans the input data for OSC 7 sequences and extracts the path
/// component from the file:// URL. The hostname is ignored as we only need the
/// local path.
///
/// # Arguments
///
/// * `data` - Raw terminal output data that may contain OSC 7 sequences
///
/// # Returns
///
/// * `Some(PathBuf)` - The extracted path if a valid OSC 7 sequence was found
/// * `None` - If no valid OSC 7 sequence was found
///
/// # Examples
///
/// ```ignore
/// let data = b"\x1b]7;file://localhost/Users/test\x07";
/// assert_eq!(parse_osc7(data), Some(PathBuf::from("/Users/test")));
/// ```
pub fn parse_osc7(data: &[u8]) -> Option<PathBuf> {
    // OSC 7 starts with ESC ] 7 ; (0x1b 0x5d 0x37 0x3b)
    const OSC_START: &[u8] = b"\x1b]7;";
    // Alternative: ESC ] 7 ; can also be written as \x9d 7 ; (C1 control)
    const OSC_START_C1: &[u8] = b"\x9d7;";

    // Find the start of an OSC 7 sequence
    let start_pos = data
        .windows(OSC_START.len())
        .position(|w| w == OSC_START)
        .map(|p| p + OSC_START.len())
        .or_else(|| {
            data.windows(OSC_START_C1.len())
                .position(|w| w == OSC_START_C1)
                .map(|p| p + OSC_START_C1.len())
        })?;

    // Find the end of the OSC sequence (BEL \x07 or ST \x1b\\)
    let remaining = &data[start_pos..];
    let end_pos = remaining.iter().position(|&b| b == 0x07)
        .or_else(|| {
            remaining.windows(2)
                .position(|w| w == b"\x1b\\")
        })?;

    // Extract the URL portion
    let url_bytes = &remaining[..end_pos];
    let url_str = std::str::from_utf8(url_bytes).ok()?;

    // Parse the file:// URL
    // Format: file://hostname/path or file:///path (empty hostname)
    if !url_str.starts_with("file://") {
        return None;
    }

    let after_scheme = &url_str[7..]; // Skip "file://"

    // Find the path portion - it starts after the hostname
    // Hostname can be empty (file:///path), localhost, or a machine name
    let path_start = if after_scheme.starts_with('/') {
        // file:///path - empty hostname, path starts immediately
        0
    } else {
        // file://hostname/path - find the first / after hostname
        after_scheme.find('/')?
    };

    let path = &after_scheme[path_start..];

    // URL decode the path (handle %20 -> space, etc.)
    let decoded_path = percent_decode(path)?;

    Some(PathBuf::from(decoded_path))
}

/// Decode percent-encoded characters in a URL path.
///
/// Handles common URL escapes like %20 (space), %2F (/), etc.
fn percent_decode(input: &str) -> Option<String> {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            // Read next two hex digits
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() != 2 {
                return None;
            }
            let byte = u8::from_str_radix(&hex, 16).ok()?;
            result.push(byte as char);
        } else {
            result.push(c);
        }
    }

    Some(result)
}

/// Renderer for the file browser pane
///
/// The FileBrowserRenderer is responsible for drawing the file browser
/// contents within a pane rectangle. It displays directory listings,
/// handles keyboard navigation, and syncs with the focused terminal's
/// current working directory.
#[derive(Debug, Default)]
pub struct FileBrowserRenderer {
    /// Currently displayed directory (placeholder for future implementation)
    _current_dir: Option<std::path::PathBuf>,
    /// Currently selected item index (placeholder for future implementation)
    _selected_index: usize,
}

impl FileBrowserRenderer {
    /// Create a new file browser renderer
    pub fn new() -> Self {
        Self {
            _current_dir: None,
            _selected_index: 0,
        }
    }

    /// Render the file browser contents within the given pane rectangle
    ///
    /// This is currently a stub implementation that does nothing.
    /// Future implementation will render directory contents with icons,
    /// highlighting for the selected item, and git status indicators.
    ///
    /// # Arguments
    ///
    /// * `_pane_rect` - The rectangle defining the file browser's render area
    pub fn render(&self, _pane_rect: Rect) {
        // Stub implementation - rendering will be implemented in US-037
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_browser_renderer_new() {
        let renderer = FileBrowserRenderer::new();
        assert!(renderer._current_dir.is_none());
        assert_eq!(renderer._selected_index, 0);
    }

    #[test]
    fn test_file_browser_renderer_default() {
        let renderer = FileBrowserRenderer::default();
        assert!(renderer._current_dir.is_none());
        assert_eq!(renderer._selected_index, 0);
    }

    #[test]
    fn test_render_stub() {
        let renderer = FileBrowserRenderer::new();
        let rect = Rect::new(0, 0, 200, 600);
        // Should not panic
        renderer.render(rect);
    }

    // OSC 7 parsing tests

    #[test]
    fn test_parse_osc7_with_localhost() {
        // OSC 7 with localhost hostname and BEL terminator
        let data = b"\x1b]7;file://localhost/Users/test/documents\x07";
        let result = parse_osc7(data);
        assert_eq!(result, Some(PathBuf::from("/Users/test/documents")));
    }

    #[test]
    fn test_parse_osc7_with_empty_hostname() {
        // OSC 7 with empty hostname (file:///)
        let data = b"\x1b]7;file:///home/user/projects\x07";
        let result = parse_osc7(data);
        assert_eq!(result, Some(PathBuf::from("/home/user/projects")));
    }

    #[test]
    fn test_parse_osc7_with_machine_hostname() {
        // OSC 7 with actual machine hostname
        let data = b"\x1b]7;file://mymachine.local/var/log\x07";
        let result = parse_osc7(data);
        assert_eq!(result, Some(PathBuf::from("/var/log")));
    }

    #[test]
    fn test_parse_osc7_with_st_terminator() {
        // OSC 7 with ST terminator (ESC \)
        let data = b"\x1b]7;file://localhost/tmp/test\x1b\\";
        let result = parse_osc7(data);
        assert_eq!(result, Some(PathBuf::from("/tmp/test")));
    }

    #[test]
    fn test_parse_osc7_with_percent_encoding() {
        // OSC 7 with URL-encoded space (%20)
        let data = b"\x1b]7;file://localhost/Users/test/my%20folder\x07";
        let result = parse_osc7(data);
        assert_eq!(result, Some(PathBuf::from("/Users/test/my folder")));
    }

    #[test]
    fn test_parse_osc7_embedded_in_data() {
        // OSC 7 sequence embedded in other terminal output
        let data = b"some output\x1b]7;file://localhost/home/user\x07more output";
        let result = parse_osc7(data);
        assert_eq!(result, Some(PathBuf::from("/home/user")));
    }

    #[test]
    fn test_parse_osc7_no_sequence() {
        // No OSC 7 sequence present
        let data = b"just regular terminal output";
        let result = parse_osc7(data);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_osc7_incomplete_sequence() {
        // Incomplete OSC 7 sequence (no terminator)
        let data = b"\x1b]7;file://localhost/path/incomplete";
        let result = parse_osc7(data);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_osc7_invalid_url() {
        // OSC 7 with non-file URL
        let data = b"\x1b]7;http://example.com/path\x07";
        let result = parse_osc7(data);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_osc7_root_path() {
        // OSC 7 with root path
        let data = b"\x1b]7;file://localhost/\x07";
        let result = parse_osc7(data);
        assert_eq!(result, Some(PathBuf::from("/")));
    }

    #[test]
    fn test_parse_osc7_with_special_chars() {
        // OSC 7 with multiple percent-encoded characters
        let data = b"\x1b]7;file://localhost/path%20with%20spaces%2Fslash\x07";
        let result = parse_osc7(data);
        assert_eq!(result, Some(PathBuf::from("/path with spaces/slash")));
    }

    #[test]
    fn test_parse_osc7_typical_zsh_output() {
        // Typical output from zsh with shell integration
        let data = b"\x1b]7;file://MacBook-Pro.local/Users/developer/projects/myapp\x07";
        let result = parse_osc7(data);
        assert_eq!(
            result,
            Some(PathBuf::from("/Users/developer/projects/myapp"))
        );
    }

    #[test]
    fn test_parse_osc7_typical_bash_output() {
        // Typical output from bash with shell integration
        let data = b"\x1b]7;file://ubuntu-server/home/ubuntu/code\x07";
        let result = parse_osc7(data);
        assert_eq!(result, Some(PathBuf::from("/home/ubuntu/code")));
    }
}
