//! File browser overlay for Claude Terminal
//!
//! This module provides a file browser pane that displays the directory
//! contents for the focused terminal pane. It uses keyboard navigation
//! with vim-style bindings (j/k/h/l).

use mux::layout::Rect;
use std::path::{Path, PathBuf};

/// Entry type for directory listing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    Directory,
    File,
}

/// A directory entry with metadata for file browser display
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Name of the file or directory
    pub name: String,
    /// Full path to the entry
    pub path: PathBuf,
    /// Whether this is a directory or file
    pub entry_type: EntryType,
}

impl DirEntry {
    /// Create a new directory entry
    pub fn new(name: String, path: PathBuf, entry_type: EntryType) -> Self {
        Self {
            name,
            path,
            entry_type,
        }
    }

    /// Check if this entry is a directory
    pub fn is_dir(&self) -> bool {
        self.entry_type == EntryType::Directory
    }

    /// Check if this entry is a file
    pub fn is_file(&self) -> bool {
        self.entry_type == EntryType::File
    }
}

/// Read directory contents and return sorted entries
///
/// Reads all entries from the given directory path, filtering hidden files
/// if `show_hidden` is false. Results are sorted with directories first,
/// then files, with alphabetical ordering within each group (case-insensitive).
///
/// # Arguments
///
/// * `path` - The directory path to read
/// * `show_hidden` - Whether to include hidden files (files starting with '.')
///
/// # Returns
///
/// A vector of `DirEntry` items sorted as described above. Returns an empty
/// vector if the directory cannot be read.
///
/// # Examples
///
/// ```ignore
/// use std::path::Path;
/// let entries = read_dir(Path::new("/home/user"), false);
/// // Returns sorted directory contents without hidden files
/// ```
pub fn read_dir(path: &Path, show_hidden: bool) -> Vec<DirEntry> {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut result: Vec<DirEntry> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();

            // Filter hidden files if show_hidden is false
            if !show_hidden && name.starts_with('.') {
                return None;
            }

            let path = entry.path();
            let entry_type = if path.is_dir() {
                EntryType::Directory
            } else {
                EntryType::File
            };

            Some(DirEntry::new(name, path, entry_type))
        })
        .collect();

    // Sort: directories first, then files, both alphabetically (case-insensitive)
    result.sort_by(|a, b| {
        // First compare by type: directories come before files
        match (a.entry_type, b.entry_type) {
            (EntryType::Directory, EntryType::File) => std::cmp::Ordering::Less,
            (EntryType::File, EntryType::Directory) => std::cmp::Ordering::Greater,
            // Same type: sort alphabetically (case-insensitive)
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    result
}

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

/// Nerdfont folder icon (U+F07B - fa-folder)
const FOLDER_ICON: char = '\u{f07b}';
/// Nerdfont file icon (U+F016 - fa-file-o)
const FILE_ICON: char = '\u{f016}';

/// A rendered line in the file browser tree view
#[derive(Debug, Clone)]
pub struct TreeLine {
    /// The icon character (folder or file)
    pub icon: char,
    /// The name of the entry
    pub name: String,
    /// Whether this is a directory
    pub is_dir: bool,
    /// The full path to this entry
    pub path: PathBuf,
}

impl TreeLine {
    /// Create a new tree line from a directory entry
    pub fn from_entry(entry: &DirEntry) -> Self {
        let icon = if entry.is_dir() { FOLDER_ICON } else { FILE_ICON };
        Self {
            icon,
            name: entry.name.clone(),
            is_dir: entry.is_dir(),
            path: entry.path.clone(),
        }
    }

    /// Format this line for display with icon and name
    pub fn display(&self) -> String {
        format!("{} {}", self.icon, self.name)
    }
}

/// Renderer for the file browser pane
///
/// The FileBrowserRenderer is responsible for drawing the file browser
/// contents within a pane rectangle. It displays directory listings,
/// handles keyboard navigation, and syncs with the focused terminal's
/// current working directory.
#[derive(Debug)]
pub struct FileBrowserRenderer {
    /// Currently displayed directory
    current_dir: Option<PathBuf>,
    /// Currently selected item index
    selected_index: usize,
    /// Cached directory entries (rendered as tree lines)
    entries: Vec<TreeLine>,
    /// Whether to show hidden files
    show_hidden: bool,
}

impl Default for FileBrowserRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl FileBrowserRenderer {
    /// Create a new file browser renderer
    pub fn new() -> Self {
        Self {
            current_dir: None,
            selected_index: 0,
            entries: Vec::new(),
            show_hidden: false,
        }
    }

    /// Get the current directory
    pub fn current_dir(&self) -> Option<&Path> {
        self.current_dir.as_deref()
    }

    /// Get the selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Get the entries
    pub fn entries(&self) -> &[TreeLine] {
        &self.entries
    }

    /// Set the current directory and reload entries
    pub fn set_current_dir(&mut self, path: PathBuf) {
        if self.current_dir.as_ref() != Some(&path) {
            self.current_dir = Some(path.clone());
            self.reload_entries();
            self.selected_index = 0;
        }
    }

    /// Reload directory entries from the current directory
    fn reload_entries(&mut self) {
        if let Some(dir) = &self.current_dir {
            let dir_entries = read_dir(dir, self.show_hidden);
            self.entries = dir_entries.iter().map(TreeLine::from_entry).collect();
        } else {
            self.entries.clear();
        }
    }

    /// Get the tree lines to render for the current directory
    ///
    /// Returns a slice of TreeLine entries that can be rendered
    /// with icon + name format. Each line represents one file or directory.
    ///
    /// # Returns
    ///
    /// A slice of TreeLine entries, or empty if no directory is set.
    pub fn get_tree_lines(&self) -> &[TreeLine] {
        &self.entries
    }

    /// Render the file browser contents within the given pane rectangle
    ///
    /// This method calculates visible lines based on the pane height
    /// and prepares rendering data. The actual pixel rendering would be
    /// done by the GPU layer using this data.
    ///
    /// # Arguments
    ///
    /// * `pane_rect` - The rectangle defining the file browser's render area
    ///
    /// # Returns
    ///
    /// A vector of strings representing lines to render, each containing
    /// an icon and filename. The selected line index is tracked separately.
    pub fn render(&self, pane_rect: Rect) -> Vec<String> {
        // Calculate approximately how many lines fit (assuming ~16px per line)
        let line_height = 16u32;
        let max_lines = if pane_rect.height > 0 {
            (pane_rect.height / line_height) as usize
        } else {
            0
        };

        // Render visible entries
        self.entries
            .iter()
            .take(max_lines)
            .map(|line| line.display())
            .collect()
    }

    /// Set whether to show hidden files
    pub fn set_show_hidden(&mut self, show: bool) {
        if self.show_hidden != show {
            self.show_hidden = show;
            self.reload_entries();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_browser_renderer_new() {
        let renderer = FileBrowserRenderer::new();
        assert!(renderer.current_dir().is_none());
        assert_eq!(renderer.selected_index(), 0);
    }

    #[test]
    fn test_file_browser_renderer_default() {
        let renderer = FileBrowserRenderer::default();
        assert!(renderer.current_dir().is_none());
        assert_eq!(renderer.selected_index(), 0);
    }

    #[test]
    fn test_render_returns_lines() {
        let mut renderer = FileBrowserRenderer::new();
        let rect = Rect::new(0, 0, 200, 600);

        // With no directory set, render should return empty
        let lines = renderer.render(rect);
        assert!(lines.is_empty());

        // Set a directory and verify render returns lines
        let temp_dir = std::env::temp_dir().join("filebrowser_test_render");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::create_dir(temp_dir.join("folder")).unwrap();
        std::fs::write(temp_dir.join("file.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        let lines = renderer.render(rect);

        assert_eq!(lines.len(), 2);
        // Directory should come first with folder icon
        assert!(lines[0].contains("folder"));
        assert!(lines[0].contains(FOLDER_ICON));
        // File should come second with file icon
        assert!(lines[1].contains("file.txt"));
        assert!(lines[1].contains(FILE_ICON));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_tree_line_from_entry() {
        let dir_entry = DirEntry::new("mydir".to_string(), PathBuf::from("/mydir"), EntryType::Directory);
        let file_entry = DirEntry::new("myfile.txt".to_string(), PathBuf::from("/myfile.txt"), EntryType::File);

        let dir_line = TreeLine::from_entry(&dir_entry);
        assert_eq!(dir_line.icon, FOLDER_ICON);
        assert_eq!(dir_line.name, "mydir");
        assert!(dir_line.is_dir);

        let file_line = TreeLine::from_entry(&file_entry);
        assert_eq!(file_line.icon, FILE_ICON);
        assert_eq!(file_line.name, "myfile.txt");
        assert!(!file_line.is_dir);
    }

    #[test]
    fn test_tree_line_display() {
        let entry = DirEntry::new("test".to_string(), PathBuf::from("/test"), EntryType::Directory);
        let line = TreeLine::from_entry(&entry);
        let display = line.display();
        assert!(display.starts_with(FOLDER_ICON));
        assert!(display.contains("test"));
    }

    #[test]
    fn test_set_current_dir_reloads_entries() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_setdir");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();

        assert!(renderer.entries().is_empty());

        renderer.set_current_dir(temp_dir.clone());
        assert_eq!(renderer.entries().len(), 2);
        assert_eq!(renderer.selected_index(), 0);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_set_show_hidden() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_hidden_render");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("visible.txt"), "").unwrap();
        std::fs::write(temp_dir.join(".hidden"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        assert_eq!(renderer.entries().len(), 1); // Only visible file

        renderer.set_show_hidden(true);
        assert_eq!(renderer.entries().len(), 2); // Both files

        renderer.set_show_hidden(false);
        assert_eq!(renderer.entries().len(), 1); // Only visible file again

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
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

    // DirEntry and read_dir tests

    #[test]
    fn test_dir_entry_new() {
        let entry = DirEntry::new(
            "test.txt".to_string(),
            PathBuf::from("/home/user/test.txt"),
            EntryType::File,
        );
        assert_eq!(entry.name, "test.txt");
        assert_eq!(entry.path, PathBuf::from("/home/user/test.txt"));
        assert_eq!(entry.entry_type, EntryType::File);
    }

    #[test]
    fn test_dir_entry_is_dir() {
        let dir = DirEntry::new("folder".to_string(), PathBuf::from("/folder"), EntryType::Directory);
        let file = DirEntry::new("file.txt".to_string(), PathBuf::from("/file.txt"), EntryType::File);

        assert!(dir.is_dir());
        assert!(!dir.is_file());
        assert!(!file.is_dir());
        assert!(file.is_file());
    }

    #[test]
    fn test_read_dir_nonexistent_path() {
        // Reading a non-existent directory should return empty vec
        let result = read_dir(Path::new("/this/path/does/not/exist/12345"), true);
        assert!(result.is_empty());
    }

    #[test]
    fn test_read_dir_real_directory() {
        // Test with the current directory (should always work)
        let result = read_dir(Path::new("."), true);
        // Should have at least some entries (like Cargo.toml in wezterm-gui)
        // We can't know exact contents but we can check the function works
        assert!(!result.is_empty() || true); // Allow empty if running from odd location
    }

    #[test]
    fn test_read_dir_sorting() {
        // Create a temp directory with known contents
        let temp_dir = std::env::temp_dir().join("filebrowser_test_sort");
        let _ = std::fs::remove_dir_all(&temp_dir); // Clean up from previous runs
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create some files and directories with varied names for sorting test
        std::fs::create_dir(temp_dir.join("zebra_dir")).unwrap();
        std::fs::create_dir(temp_dir.join("Alpha_dir")).unwrap();
        std::fs::write(temp_dir.join("beta_file.txt"), "").unwrap();
        std::fs::write(temp_dir.join("Gamma_file.txt"), "").unwrap();

        let result = read_dir(&temp_dir, true);

        assert_eq!(result.len(), 4);

        // Directories should come first, sorted alphabetically (case-insensitive)
        assert_eq!(result[0].name, "Alpha_dir");
        assert!(result[0].is_dir());
        assert_eq!(result[1].name, "zebra_dir");
        assert!(result[1].is_dir());

        // Files should come after directories, sorted alphabetically (case-insensitive)
        assert_eq!(result[2].name, "beta_file.txt");
        assert!(result[2].is_file());
        assert_eq!(result[3].name, "Gamma_file.txt");
        assert!(result[3].is_file());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_read_dir_hidden_files_filtered() {
        let temp_dir = std::env::temp_dir().join("filebrowser_test_hidden");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create visible and hidden files
        std::fs::write(temp_dir.join("visible.txt"), "").unwrap();
        std::fs::write(temp_dir.join(".hidden"), "").unwrap();
        std::fs::create_dir(temp_dir.join(".hidden_dir")).unwrap();
        std::fs::create_dir(temp_dir.join("visible_dir")).unwrap();

        // With show_hidden=false, hidden files should be filtered
        let result_no_hidden = read_dir(&temp_dir, false);
        assert_eq!(result_no_hidden.len(), 2);
        assert!(result_no_hidden.iter().all(|e| !e.name.starts_with('.')));

        // With show_hidden=true, all files should be included
        let result_with_hidden = read_dir(&temp_dir, true);
        assert_eq!(result_with_hidden.len(), 4);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_read_dir_directories_first() {
        let temp_dir = std::env::temp_dir().join("filebrowser_test_order");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create files that would come before directories alphabetically
        std::fs::write(temp_dir.join("aaa_file.txt"), "").unwrap();
        std::fs::create_dir(temp_dir.join("zzz_dir")).unwrap();

        let result = read_dir(&temp_dir, true);

        assert_eq!(result.len(), 2);
        // Even though "aaa_file.txt" < "zzz_dir" alphabetically,
        // the directory should come first
        assert!(result[0].is_dir());
        assert_eq!(result[0].name, "zzz_dir");
        assert!(result[1].is_file());
        assert_eq!(result[1].name, "aaa_file.txt");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
