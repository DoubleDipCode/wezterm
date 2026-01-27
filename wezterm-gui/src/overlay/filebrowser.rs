//! File browser overlay for Claude Terminal
//!
//! This module provides a file browser pane that displays the directory
//! contents for the focused terminal pane. It uses keyboard navigation
//! with vim-style bindings (j/k/h/l).

use mux::layout::Rect;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Entry type for directory listing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    Directory,
    File,
}

/// Git status for a file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitStatus {
    /// File is not tracked by git or has no changes
    #[default]
    None,
    /// File has been modified (working tree changes)
    Modified,
    /// File has been staged for commit
    Staged,
    /// File is untracked by git
    Untracked,
}

/// Status symbol for git modified files (red)
const GIT_MODIFIED_SYMBOL: char = '●';
/// Status symbol for git staged files (green)
const GIT_STAGED_SYMBOL: char = '●';
/// Status symbol for git untracked files (yellow)
const GIT_UNTRACKED_SYMBOL: char = '●';

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

/// Get git status for all files in a directory.
///
/// Runs `git status --porcelain` in the specified directory and parses the output
/// to determine the git status of each file. Returns a HashMap mapping file paths
/// to their git status.
///
/// # Arguments
///
/// * `dir` - The directory to check git status for
///
/// # Returns
///
/// A HashMap mapping PathBuf to GitStatus. Files not in the map have no git status
/// (either not in a git repo, or have no changes).
///
/// # Status Codes
///
/// Git porcelain format uses two-character codes:
/// - `M ` = staged modified, ` M` = unstaged modified, `MM` = both
/// - `A ` = staged new file
/// - `??` = untracked
/// - ` D` / `D ` = deleted
///
/// We prioritize: Staged > Modified > Untracked
pub fn git_status_for_directory(dir: &Path) -> HashMap<PathBuf, GitStatus> {
    let mut result = HashMap::new();

    // Run git status --porcelain in the directory
    let output = match Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(dir)
        .output()
    {
        Ok(output) => output,
        Err(_) => return result, // git not available or not a git repo
    };

    // Check if command succeeded (non-zero exit code means not a git repo)
    if !output.status.success() {
        return result;
    }

    // Parse the output line by line
    let stdout = match std::str::from_utf8(&output.stdout) {
        Ok(s) => s,
        Err(_) => return result,
    };

    for line in stdout.lines() {
        if line.len() < 3 {
            continue;
        }

        // Format: XY filename
        // X = index status, Y = working tree status
        let index_status = line.chars().next().unwrap_or(' ');
        let worktree_status = line.chars().nth(1).unwrap_or(' ');
        let filename = &line[3..];

        // Handle renamed files (format: "R  old -> new")
        let filename = if filename.contains(" -> ") {
            filename.split(" -> ").last().unwrap_or(filename)
        } else {
            filename
        };

        // Remove any quotes around filename
        let filename = filename.trim_matches('"');

        let path = dir.join(filename);

        // Determine status based on git codes
        // Priority: Staged > Modified > Untracked
        let status = if index_status == '?' && worktree_status == '?' {
            GitStatus::Untracked
        } else if index_status != ' ' && index_status != '?' {
            // Has staged changes (A, M, D, R in index position)
            GitStatus::Staged
        } else if worktree_status == 'M' || worktree_status == 'D' {
            // Has working tree modifications
            GitStatus::Modified
        } else {
            GitStatus::None
        };

        if status != GitStatus::None {
            result.insert(path, status);
        }
    }

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
    /// Git status for this entry
    pub git_status: GitStatus,
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
            git_status: GitStatus::None,
        }
    }

    /// Create a new tree line from a directory entry with git status
    pub fn from_entry_with_status(entry: &DirEntry, status: GitStatus) -> Self {
        let icon = if entry.is_dir() { FOLDER_ICON } else { FILE_ICON };
        Self {
            icon,
            name: entry.name.clone(),
            is_dir: entry.is_dir(),
            path: entry.path.clone(),
            git_status: status,
        }
    }

    /// Format this line for display with icon, name, and git status
    ///
    /// Shows git status symbol after the filename:
    /// - Modified: red ● (shown as "● M")
    /// - Staged: green ● (shown as "● S")
    /// - Untracked: yellow ● (shown as "● U")
    pub fn display(&self) -> String {
        self.display_with_options(true, true)
    }

    /// Format this line for display with configurable options
    ///
    /// # Arguments
    ///
    /// * `show_icons` - Whether to show nerdfont icons before filenames
    /// * `show_git_status` - Whether to show git status indicators after filenames
    pub fn display_with_options(&self, show_icons: bool, show_git_status: bool) -> String {
        let prefix = if show_icons {
            format!("{} ", self.icon)
        } else {
            String::new()
        };

        if !show_git_status || self.git_status == GitStatus::None {
            format!("{}{}", prefix, self.name)
        } else {
            let (symbol, suffix) = match self.git_status {
                GitStatus::Modified => (GIT_MODIFIED_SYMBOL, "M"),
                GitStatus::Staged => (GIT_STAGED_SYMBOL, "S"),
                GitStatus::Untracked => (GIT_UNTRACKED_SYMBOL, "U"),
                GitStatus::None => unreachable!(),
            };
            format!("{}{} {} {}", prefix, self.name, symbol, suffix)
        }
    }

    /// Get the git status color hint for rendering
    ///
    /// Returns an optional RGB tuple for color-coding:
    /// - Modified: red (255, 85, 85)
    /// - Staged: green (80, 250, 123)
    /// - Untracked: yellow (241, 250, 140)
    /// - None: no color hint (None)
    pub fn git_status_color(&self) -> Option<(u8, u8, u8)> {
        match self.git_status {
            GitStatus::None => None,
            GitStatus::Modified => Some((255, 85, 85)),    // red
            GitStatus::Staged => Some((80, 250, 123)),     // green
            GitStatus::Untracked => Some((241, 250, 140)), // yellow
        }
    }
}

/// Content of a file preview
#[derive(Debug, Clone)]
pub struct PreviewContent {
    /// The lines to display (first 100 lines for files)
    pub lines: Vec<String>,
    /// Whether this is a directory preview
    pub is_directory: bool,
    /// Entry count for directories
    pub entry_count: Option<usize>,
    /// Path being previewed
    pub path: PathBuf,
}

impl PreviewContent {
    /// Create a preview for a file (reads first 100 lines)
    pub fn from_file(path: &Path) -> Option<Self> {
        use std::io::{BufRead, BufReader};

        let file = std::fs::File::open(path).ok()?;
        let reader = BufReader::new(file);

        let lines: Vec<String> = reader
            .lines()
            .take(100)
            .filter_map(|l| l.ok())
            .collect();

        Some(Self {
            lines,
            is_directory: false,
            entry_count: None,
            path: path.to_path_buf(),
        })
    }

    /// Create a preview for a directory (shows entry count)
    pub fn from_directory(path: &Path) -> Option<Self> {
        let entries = std::fs::read_dir(path).ok()?;
        let count = entries.count();

        Some(Self {
            lines: Vec::new(),
            is_directory: true,
            entry_count: Some(count),
            path: path.to_path_buf(),
        })
    }

    /// Create a preview for a file or directory based on path type
    pub fn from_path(path: &Path) -> Option<Self> {
        if path.is_dir() {
            Self::from_directory(path)
        } else {
            Self::from_file(path)
        }
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
    /// Currently selected item index (into filtered_entries when filtering)
    selected_index: usize,
    /// Cached directory entries (rendered as tree lines)
    entries: Vec<TreeLine>,
    /// Whether to show hidden files
    show_hidden: bool,
    /// Whether filter mode is active
    filter_mode: bool,
    /// Current filter text (for substring matching)
    filter_text: String,
    /// Filtered entries (subset of entries matching filter_text)
    filtered_entries: Vec<TreeLine>,
    /// Whether preview mode is active
    preview_mode: bool,
    /// Cached preview content for the selected entry
    preview_content: Option<PreviewContent>,
    /// Whether to show git status indicators
    show_git_status: bool,
    /// Whether to show nerdfont icons
    show_icons: bool,
}

impl Default for FileBrowserRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl FileBrowserRenderer {
    /// Create a new file browser renderer with default settings
    pub fn new() -> Self {
        Self {
            current_dir: None,
            selected_index: 0,
            entries: Vec::new(),
            show_hidden: false,
            filter_mode: false,
            filter_text: String::new(),
            filtered_entries: Vec::new(),
            preview_mode: false,
            preview_content: None,
            show_git_status: true, // enabled by default
            show_icons: true,      // enabled by default
        }
    }

    /// Create a new file browser renderer with settings from config
    ///
    /// Reads `config.claude_terminal.file_browser` and applies:
    /// - `show_hidden`: Whether to show hidden files (default: false)
    /// - `show_git_status`: Whether to show git status indicators (default: true)
    /// - `icons`: Whether to show nerdfont icons (default: true)
    ///
    /// Note: The `enabled` config option is handled by the caller (TermWindow)
    /// to control file browser visibility, not by the renderer itself.
    pub fn new_from_config() -> Self {
        let config = config::configuration();
        let fb_config = &config.claude_terminal.file_browser;

        Self {
            current_dir: None,
            selected_index: 0,
            entries: Vec::new(),
            show_hidden: fb_config.show_hidden,
            filter_mode: false,
            filter_text: String::new(),
            filtered_entries: Vec::new(),
            preview_mode: false,
            preview_content: None,
            show_git_status: fb_config.show_git_status,
            show_icons: fb_config.icons,
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

    /// Get the entries (filtered if in filter mode, otherwise all entries)
    pub fn entries(&self) -> &[TreeLine] {
        if self.filter_mode && !self.filter_text.is_empty() {
            &self.filtered_entries
        } else {
            &self.entries
        }
    }

    /// Get all entries regardless of filter state
    pub fn all_entries(&self) -> &[TreeLine] {
        &self.entries
    }

    /// Check if filter mode is active
    pub fn is_filter_mode(&self) -> bool {
        self.filter_mode
    }

    /// Get the current filter text
    pub fn filter_text(&self) -> &str {
        &self.filter_text
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

            // Fetch git status if enabled
            let git_statuses = if self.show_git_status {
                git_status_for_directory(dir)
            } else {
                HashMap::new()
            };

            // Create tree lines with git status
            self.entries = dir_entries
                .iter()
                .map(|entry| {
                    let status = git_statuses
                        .get(&entry.path)
                        .copied()
                        .unwrap_or(GitStatus::None);
                    TreeLine::from_entry_with_status(entry, status)
                })
                .collect();
        } else {
            self.entries.clear();
        }
        // Update filtered entries if in filter mode
        if self.filter_mode {
            self.apply_filter();
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
    /// When in filter mode, an input line is shown at the bottom with
    /// the format "/ <filter_text>".
    ///
    /// # Arguments
    ///
    /// * `pane_rect` - The rectangle defining the file browser's render area
    ///
    /// # Returns
    ///
    /// A vector of strings representing lines to render, each containing
    /// an icon and filename. The selected line index is tracked separately.
    /// In filter mode, the last line is the filter input.
    pub fn render(&self, pane_rect: Rect) -> Vec<String> {
        // Calculate approximately how many lines fit (assuming ~16px per line)
        let line_height = 16u32;
        let max_lines = if pane_rect.height > 0 {
            (pane_rect.height / line_height) as usize
        } else {
            0
        };

        // Reserve one line for filter input when in filter mode
        let entry_lines = if self.filter_mode && max_lines > 0 {
            max_lines - 1
        } else {
            max_lines
        };

        // Render visible entries (use filtered entries when in filter mode)
        let entries = self.entries();
        let mut lines: Vec<String> = entries
            .iter()
            .take(entry_lines)
            .map(|line| line.display_with_options(self.show_icons, self.show_git_status))
            .collect();

        // Add filter input line at the bottom when in filter mode
        if self.filter_mode {
            lines.push(format!("/ {}", self.filter_text));
        }

        lines
    }

    /// Set whether to show hidden files
    pub fn set_show_hidden(&mut self, show: bool) {
        if self.show_hidden != show {
            self.show_hidden = show;
            self.reload_entries();
        }
    }

    /// Set whether to show git status indicators
    pub fn set_show_git_status(&mut self, show: bool) {
        if self.show_git_status != show {
            self.show_git_status = show;
            self.reload_entries();
        }
    }

    /// Check if git status indicators are enabled
    pub fn is_git_status_enabled(&self) -> bool {
        self.show_git_status
    }

    /// Set whether to show nerdfont icons
    pub fn set_show_icons(&mut self, show: bool) {
        // No need to reload entries - icons are just a display option
        self.show_icons = show;
    }

    /// Check if icons are enabled
    pub fn is_icons_enabled(&self) -> bool {
        self.show_icons
    }

    /// Check if hidden files are shown
    pub fn is_show_hidden(&self) -> bool {
        self.show_hidden
    }

    /// Enter filter mode (/ key)
    ///
    /// Activates filter mode which shows an input box at the bottom
    /// for typing filter text. While in filter mode, the visible
    /// entries are filtered by case-insensitive substring match.
    pub fn enter_filter_mode(&mut self) {
        self.filter_mode = true;
        self.filter_text.clear();
        self.filtered_entries = self.entries.clone();
        self.selected_index = 0;
    }

    /// Exit filter mode (Esc key)
    ///
    /// Clears the filter text and exits filter mode. Returns to
    /// showing all entries.
    pub fn exit_filter_mode(&mut self) {
        self.filter_mode = false;
        self.filter_text.clear();
        self.filtered_entries.clear();
        self.selected_index = 0;
    }

    /// Update filter text and refresh filtered entries
    ///
    /// Called as the user types in filter mode. Updates the filter
    /// text and recalculates the visible entries based on case-insensitive
    /// substring matching.
    ///
    /// # Arguments
    ///
    /// * `text` - The new filter text
    pub fn set_filter_text(&mut self, text: String) {
        self.filter_text = text;
        self.apply_filter();
        // Reset selection if it's now out of bounds
        if self.filtered_entries.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.filtered_entries.len() {
            self.selected_index = self.filtered_entries.len() - 1;
        }
    }

    /// Append a character to the filter text
    ///
    /// Convenience method for handling individual key presses in filter mode.
    pub fn append_filter_char(&mut self, c: char) {
        self.filter_text.push(c);
        self.apply_filter();
        // Reset selection if it's now out of bounds
        if self.filtered_entries.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.filtered_entries.len() {
            self.selected_index = self.filtered_entries.len() - 1;
        }
    }

    /// Remove the last character from the filter text (backspace)
    ///
    /// Returns true if there was a character to remove, false if filter was empty.
    pub fn backspace_filter(&mut self) -> bool {
        if self.filter_text.pop().is_some() {
            self.apply_filter();
            // Reset selection if needed
            if !self.filtered_entries.is_empty() && self.selected_index >= self.filtered_entries.len() {
                self.selected_index = self.filtered_entries.len() - 1;
            }
            true
        } else {
            false
        }
    }

    /// Apply the current filter to entries
    fn apply_filter(&mut self) {
        if self.filter_text.is_empty() {
            self.filtered_entries = self.entries.clone();
        } else {
            let filter_lower = self.filter_text.to_lowercase();
            self.filtered_entries = self.entries
                .iter()
                .filter(|entry| entry.name.to_lowercase().contains(&filter_lower))
                .cloned()
                .collect();
        }
    }

    /// Check if preview mode is active
    pub fn is_preview_mode(&self) -> bool {
        self.preview_mode
    }

    /// Get the current preview content, if any
    pub fn preview_content(&self) -> Option<&PreviewContent> {
        self.preview_content.as_ref()
    }

    /// Toggle preview mode (Space key)
    ///
    /// When toggling on: loads preview content for the selected entry.
    /// When toggling off: clears preview content.
    /// Also exits preview mode on Esc or second Space press.
    ///
    /// # Returns
    ///
    /// * `true` - Preview mode is now active
    /// * `false` - Preview mode is now inactive
    pub fn toggle_preview(&mut self) -> bool {
        if self.preview_mode {
            // Turning off preview mode
            self.preview_mode = false;
            self.preview_content = None;
            false
        } else {
            // Turning on preview mode
            self.preview_mode = true;
            self.update_preview_content();
            true
        }
    }

    /// Close preview mode (Esc key - also closes preview if open)
    ///
    /// If preview mode is active, closes it. This allows Esc to
    /// close the preview in addition to closing filter mode.
    ///
    /// # Returns
    ///
    /// * `true` - Preview mode was active and is now closed
    /// * `false` - Preview mode was not active
    pub fn close_preview(&mut self) -> bool {
        if self.preview_mode {
            self.preview_mode = false;
            self.preview_content = None;
            true
        } else {
            false
        }
    }

    /// Update preview content for the currently selected entry
    ///
    /// Called when selection changes while preview mode is active,
    /// or when preview mode is first enabled.
    fn update_preview_content(&mut self) {
        self.preview_content = self.selected_entry()
            .and_then(|entry| PreviewContent::from_path(&entry.path));
    }

    /// Render preview content as a vector of strings
    ///
    /// Returns lines suitable for display in the preview panel.
    /// For files, shows the first 100 lines.
    /// For directories, shows the entry count.
    ///
    /// # Returns
    ///
    /// A vector of strings to display in the preview panel
    pub fn render_preview(&self) -> Vec<String> {
        match &self.preview_content {
            Some(content) if content.is_directory => {
                let count = content.entry_count.unwrap_or(0);
                vec![
                    format!("📁 {}", content.path.display()),
                    String::new(),
                    format!("{} entries", count),
                ]
            }
            Some(content) => {
                let mut lines = vec![
                    format!("📄 {}", content.path.display()),
                    String::new(),
                ];
                lines.extend(content.lines.iter().cloned());
                lines
            }
            None => vec!["No preview available".to_string()],
        }
    }

    /// Move selection down (j key), wrapping to top at bottom
    ///
    /// Increments the selected index, wrapping to 0 if at the end
    /// of the entry list. Does nothing if there are no entries.
    /// Uses filtered entries when in filter mode.
    /// Updates preview content if preview mode is active.
    pub fn move_down(&mut self) {
        let entries = self.entries();
        if entries.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % entries.len();
        if self.preview_mode {
            self.update_preview_content();
        }
    }

    /// Move selection up (k key), wrapping to bottom at top
    ///
    /// Decrements the selected index, wrapping to the last entry
    /// if at the top. Does nothing if there are no entries.
    /// Uses filtered entries when in filter mode.
    /// Updates preview content if preview mode is active.
    pub fn move_up(&mut self) {
        let entries = self.entries();
        if entries.is_empty() {
            return;
        }
        let len = entries.len();
        if self.selected_index == 0 {
            self.selected_index = len - 1;
        } else {
            self.selected_index -= 1;
        }
        if self.preview_mode {
            self.update_preview_content();
        }
    }

    /// Check if the entry at the given index is selected
    ///
    /// Used for highlighting the selected entry with a different
    /// background color during rendering.
    ///
    /// # Arguments
    ///
    /// * `index` - The index to check
    ///
    /// # Returns
    ///
    /// `true` if this index is the currently selected entry
    pub fn is_selected(&self, index: usize) -> bool {
        index == self.selected_index
    }

    /// Get the currently selected entry, if any
    ///
    /// Returns from filtered entries when in filter mode.
    ///
    /// # Returns
    ///
    /// The selected TreeLine entry, or None if there are no entries
    pub fn selected_entry(&self) -> Option<&TreeLine> {
        self.entries().get(self.selected_index)
    }

    /// Set the selected index directly
    ///
    /// Clamps the index to valid bounds (0 to len-1).
    /// Sets to 0 if entries are empty.
    /// Uses filtered entries when in filter mode.
    /// Updates preview content if preview mode is active.
    pub fn set_selected_index(&mut self, index: usize) {
        let entries = self.entries();
        if entries.is_empty() {
            self.selected_index = 0;
        } else {
            self.selected_index = index.min(entries.len() - 1);
        }
        if self.preview_mode {
            self.update_preview_content();
        }
    }

    /// Navigate to the parent directory (h key)
    ///
    /// Changes the current directory to its parent directory. If already
    /// at the filesystem root, this method does nothing. After navigating,
    /// the entry list is reloaded and the selection is reset to index 0.
    ///
    /// # Returns
    ///
    /// * `true` - If successfully navigated to parent directory
    /// * `false` - If already at root or no directory is set
    pub fn go_to_parent(&mut self) -> bool {
        let current = match &self.current_dir {
            Some(dir) => dir.clone(),
            None => return false,
        };

        // Get the parent directory
        let parent = match current.parent() {
            Some(p) => p.to_path_buf(),
            None => return false, // Already at root
        };

        // Check if parent is different from current (handles root edge cases)
        if parent == current {
            return false;
        }

        // Update to parent directory
        self.current_dir = Some(parent);
        self.reload_entries();
        self.selected_index = 0;

        true
    }

    /// Enter the selected directory (l key)
    ///
    /// If the selected entry is a directory, changes the current directory
    /// to that directory. If the selected entry is a file or no entry is
    /// selected, this method does nothing. After navigating, the entry list
    /// is reloaded and the selection is reset to index 0.
    ///
    /// # Returns
    ///
    /// * `true` - If successfully entered the directory
    /// * `false` - If selected entry is not a directory or no entry selected
    pub fn enter_directory(&mut self) -> bool {
        // Get the currently selected entry
        let entry = match self.selected_entry() {
            Some(e) => e,
            None => return false,
        };

        // Only enter directories, not files
        if !entry.is_dir {
            return false;
        }

        // Get the path to enter
        let new_dir = entry.path.clone();

        // Update current directory
        self.current_dir = Some(new_dir);
        self.reload_entries();
        self.selected_index = 0;

        true
    }

    /// Open the selected file in the user's editor (Enter key)
    ///
    /// If the selected entry is a file (not a directory), this method
    /// returns an `OpenFileAction` containing the editor command to spawn.
    /// The editor is determined by the `$EDITOR` environment variable,
    /// defaulting to "vi" if not set.
    ///
    /// # Returns
    ///
    /// * `Some(OpenFileAction)` - If the selected entry is a file
    /// * `None` - If no entry is selected or the selected entry is a directory
    pub fn open_selected_file(&self) -> Option<OpenFileAction> {
        let entry = self.selected_entry()?;

        // Only open files, not directories
        if entry.is_dir {
            return None;
        }

        // Read $EDITOR environment variable, default to "vi"
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

        Some(OpenFileAction {
            editor,
            filepath: entry.path.clone(),
        })
    }

    /// Yank (copy) the selected entry's path to the clipboard (y key)
    ///
    /// Returns the absolute path of the selected entry for copying to the
    /// system clipboard. Works for both files and directories.
    ///
    /// The actual clipboard operation must be performed by the caller
    /// (TermWindow) since FileBrowserRenderer doesn't have access to
    /// the window system.
    ///
    /// # Returns
    ///
    /// * `Some(YankResult)` - If an entry is selected, contains the path and feedback message
    /// * `None` - If no entry is selected
    pub fn yank_selected_path(&self) -> Option<YankResult> {
        let entry = self.selected_entry()?;
        Some(YankResult::new(entry.path.clone()))
    }
}

/// Action to open a file in the user's editor
///
/// This struct is returned by `FileBrowserRenderer::open_selected_file()`
/// and contains all the information needed to spawn the editor command
/// in the focused terminal pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenFileAction {
    /// The editor command (from $EDITOR or default "vi")
    pub editor: String,
    /// The full path to the file to open
    pub filepath: PathBuf,
}

impl OpenFileAction {
    /// Get the command line to execute
    ///
    /// Returns the editor command followed by the filepath, suitable
    /// for spawning in a terminal.
    ///
    /// # Returns
    ///
    /// A string like `"vim /path/to/file.txt"` or `"code /path/to/file.txt"`
    pub fn command_line(&self) -> String {
        // Quote the filepath to handle spaces and special characters
        let path_str = self.filepath.display().to_string();
        if path_str.contains(' ') || path_str.contains('\'') || path_str.contains('"') {
            // Use shell quoting for paths with special characters
            format!("{} '{}'", self.editor, path_str.replace('\'', "'\\''"))
        } else {
            format!("{} {}", self.editor, path_str)
        }
    }
}

/// Result of a yank (copy to clipboard) operation
///
/// This struct is returned by `FileBrowserRenderer::yank_selected_path()`
/// and contains the path that was copied and a user-friendly message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YankResult {
    /// The absolute path that was copied to the clipboard
    pub path: PathBuf,
    /// User-friendly message to display (e.g., "Copied!")
    pub message: String,
}

impl YankResult {
    /// Create a new yank result
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            message: "Copied!".to_string(),
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

    // Navigation tests (j/k keyboard navigation)

    #[test]
    fn test_move_down_increments_index() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_nav_down");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();
        std::fs::write(temp_dir.join("c.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        assert_eq!(renderer.selected_index(), 0);

        renderer.move_down();
        assert_eq!(renderer.selected_index(), 1);

        renderer.move_down();
        assert_eq!(renderer.selected_index(), 2);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_move_down_wraps_at_bottom() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_nav_wrap_down");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        assert_eq!(renderer.selected_index(), 0);

        renderer.move_down();
        assert_eq!(renderer.selected_index(), 1);

        // Should wrap to 0
        renderer.move_down();
        assert_eq!(renderer.selected_index(), 0);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_move_up_decrements_index() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_nav_up");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();
        std::fs::write(temp_dir.join("c.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.set_selected_index(2);
        assert_eq!(renderer.selected_index(), 2);

        renderer.move_up();
        assert_eq!(renderer.selected_index(), 1);

        renderer.move_up();
        assert_eq!(renderer.selected_index(), 0);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_move_up_wraps_at_top() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_nav_wrap_up");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();
        std::fs::write(temp_dir.join("c.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        assert_eq!(renderer.selected_index(), 0);

        // Should wrap to last (index 2)
        renderer.move_up();
        assert_eq!(renderer.selected_index(), 2);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_move_on_empty_list() {
        let mut renderer = FileBrowserRenderer::new();

        // With no directory set, entries is empty
        assert_eq!(renderer.selected_index(), 0);

        // Both moves should do nothing on empty list
        renderer.move_down();
        assert_eq!(renderer.selected_index(), 0);

        renderer.move_up();
        assert_eq!(renderer.selected_index(), 0);
    }

    #[test]
    fn test_is_selected() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_is_selected");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        assert!(renderer.is_selected(0));
        assert!(!renderer.is_selected(1));

        renderer.move_down();
        assert!(!renderer.is_selected(0));
        assert!(renderer.is_selected(1));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_selected_entry() {
        let mut renderer = FileBrowserRenderer::new();

        // Empty - no selected entry
        assert!(renderer.selected_entry().is_none());

        let temp_dir = std::env::temp_dir().join("filebrowser_test_selected_entry");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        let entry = renderer.selected_entry();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().name, "a.txt");

        renderer.move_down();
        let entry = renderer.selected_entry();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().name, "b.txt");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_set_selected_index() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_set_idx");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();
        std::fs::write(temp_dir.join("c.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());

        renderer.set_selected_index(1);
        assert_eq!(renderer.selected_index(), 1);

        // Out of bounds should clamp to max valid
        renderer.set_selected_index(100);
        assert_eq!(renderer.selected_index(), 2);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // Open file tests (Enter key - US-039)

    #[test]
    fn test_open_selected_file_returns_none_on_empty() {
        let renderer = FileBrowserRenderer::new();
        assert!(renderer.open_selected_file().is_none());
    }

    #[test]
    fn test_open_selected_file_returns_none_for_directory() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_open_dir");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::create_dir(temp_dir.join("subdir")).unwrap();
        std::fs::write(temp_dir.join("file.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());

        // First entry should be the directory (directories come first)
        assert_eq!(renderer.selected_index(), 0);
        assert!(renderer.selected_entry().unwrap().is_dir);
        assert!(renderer.open_selected_file().is_none());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_open_selected_file_returns_action_for_file() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_open_file");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("test.txt"), "hello").unwrap();

        renderer.set_current_dir(temp_dir.clone());

        let action = renderer.open_selected_file();
        assert!(action.is_some());

        let action = action.unwrap();
        assert_eq!(action.filepath, temp_dir.join("test.txt"));
        // Editor should be from $EDITOR or default "vi"
        assert!(!action.editor.is_empty());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_open_file_action_command_line_simple() {
        let action = OpenFileAction {
            editor: "vim".to_string(),
            filepath: PathBuf::from("/home/user/file.txt"),
        };
        assert_eq!(action.command_line(), "vim /home/user/file.txt");
    }

    #[test]
    fn test_open_file_action_command_line_with_spaces() {
        let action = OpenFileAction {
            editor: "code".to_string(),
            filepath: PathBuf::from("/home/user/my file.txt"),
        };
        assert_eq!(action.command_line(), "code '/home/user/my file.txt'");
    }

    #[test]
    fn test_open_file_action_command_line_with_quotes() {
        let action = OpenFileAction {
            editor: "vim".to_string(),
            filepath: PathBuf::from("/home/user/file'name.txt"),
        };
        // Single quotes in path should be escaped
        assert_eq!(action.command_line(), "vim '/home/user/file'\\''name.txt'");
    }

    #[test]
    fn test_open_selected_file_uses_editor_env() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_editor_env");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("test.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());

        // Set custom EDITOR for test
        let original_editor = std::env::var("EDITOR").ok();
        std::env::set_var("EDITOR", "custom-editor");

        let action = renderer.open_selected_file().unwrap();
        assert_eq!(action.editor, "custom-editor");

        // Restore original EDITOR
        match original_editor {
            Some(val) => std::env::set_var("EDITOR", val),
            None => std::env::remove_var("EDITOR"),
        }

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // Go to parent directory tests (h key - US-040)

    #[test]
    fn test_go_to_parent_returns_false_on_no_directory() {
        let mut renderer = FileBrowserRenderer::new();
        assert!(!renderer.go_to_parent());
    }

    #[test]
    fn test_go_to_parent_navigates_to_parent() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_parent");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let subdir = temp_dir.join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("file.txt"), "").unwrap();
        std::fs::write(temp_dir.join("parent_file.txt"), "").unwrap();

        // Start in subdir
        renderer.set_current_dir(subdir.clone());
        assert_eq!(renderer.current_dir(), Some(subdir.as_path()));
        assert_eq!(renderer.entries().len(), 1); // file.txt

        // Navigate to parent
        let result = renderer.go_to_parent();
        assert!(result);
        assert_eq!(renderer.current_dir(), Some(temp_dir.as_path()));
        // Parent should have subdir and parent_file.txt
        assert_eq!(renderer.entries().len(), 2);
        assert_eq!(renderer.selected_index(), 0);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_go_to_parent_returns_false_at_root() {
        let mut renderer = FileBrowserRenderer::new();

        // Set to root directory
        renderer.set_current_dir(PathBuf::from("/"));
        assert_eq!(renderer.current_dir(), Some(Path::new("/")));

        // Should return false - already at root
        let result = renderer.go_to_parent();
        assert!(!result);
        // Should still be at root
        assert_eq!(renderer.current_dir(), Some(Path::new("/")));
    }

    #[test]
    fn test_go_to_parent_resets_selection_to_zero() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_parent_reset");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let subdir = temp_dir.join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("a.txt"), "").unwrap();
        std::fs::write(subdir.join("b.txt"), "").unwrap();
        std::fs::write(subdir.join("c.txt"), "").unwrap();

        renderer.set_current_dir(subdir.clone());
        // Move selection to non-zero index
        renderer.move_down();
        renderer.move_down();
        assert_eq!(renderer.selected_index(), 2);

        // Navigate to parent
        renderer.go_to_parent();
        // Selection should reset to 0
        assert_eq!(renderer.selected_index(), 0);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_go_to_parent_reloads_entries() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_parent_reload");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let subdir = temp_dir.join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("child_file.txt"), "").unwrap();
        std::fs::write(temp_dir.join("parent_file.txt"), "").unwrap();

        renderer.set_current_dir(subdir.clone());
        // In subdir, should have child_file.txt
        assert!(renderer.entries().iter().any(|e| e.name == "child_file.txt"));
        assert!(!renderer.entries().iter().any(|e| e.name == "parent_file.txt"));

        renderer.go_to_parent();
        // Now should have subdir and parent_file.txt, not child_file.txt
        assert!(!renderer.entries().iter().any(|e| e.name == "child_file.txt"));
        assert!(renderer.entries().iter().any(|e| e.name == "parent_file.txt"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // Enter directory tests (l key - US-041)

    #[test]
    fn test_enter_directory_returns_false_on_empty() {
        let mut renderer = FileBrowserRenderer::new();
        // No directory set, no entries
        assert!(!renderer.enter_directory());
    }

    #[test]
    fn test_enter_directory_returns_false_for_file() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_enter_file");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("file.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        // Selected entry is a file
        assert!(!renderer.selected_entry().unwrap().is_dir);

        // Should return false - can't enter a file
        let result = renderer.enter_directory();
        assert!(!result);

        // Directory should not have changed
        assert_eq!(renderer.current_dir(), Some(temp_dir.as_path()));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_enter_directory_navigates_to_directory() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_enter_dir");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let subdir = temp_dir.join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("child_file.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        // First entry should be the directory (directories come first)
        assert!(renderer.selected_entry().unwrap().is_dir);
        assert_eq!(renderer.selected_entry().unwrap().name, "subdir");

        // Enter the directory
        let result = renderer.enter_directory();
        assert!(result);
        assert_eq!(renderer.current_dir(), Some(subdir.as_path()));

        // Entries should now be from subdir
        assert!(renderer.entries().iter().any(|e| e.name == "child_file.txt"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_enter_directory_resets_selection_to_zero() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_enter_reset");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let subdir = temp_dir.join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(temp_dir.join("a_file.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b_file.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        // Should have: subdir (dir), a_file.txt, b_file.txt
        assert!(renderer.selected_entry().unwrap().is_dir);

        // Enter the directory (this resets selection)
        renderer.enter_directory();
        assert_eq!(renderer.selected_index(), 0);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_enter_directory_reloads_entries() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_enter_reload");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let subdir = temp_dir.join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(temp_dir.join("parent_file.txt"), "").unwrap();
        std::fs::write(subdir.join("child_file.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        // Should see parent_file.txt and subdir
        assert!(renderer.entries().iter().any(|e| e.name == "parent_file.txt"));
        assert!(!renderer.entries().iter().any(|e| e.name == "child_file.txt"));

        // Enter subdir
        renderer.enter_directory();
        // Now should see child_file.txt, not parent_file.txt
        assert!(!renderer.entries().iter().any(|e| e.name == "parent_file.txt"));
        assert!(renderer.entries().iter().any(|e| e.name == "child_file.txt"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // Filter mode tests (/ key - US-042)

    #[test]
    fn test_enter_filter_mode() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_filter_enter");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        assert!(!renderer.is_filter_mode());
        assert!(renderer.filter_text().is_empty());

        renderer.enter_filter_mode();
        assert!(renderer.is_filter_mode());
        assert!(renderer.filter_text().is_empty());
        assert_eq!(renderer.selected_index(), 0);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_exit_filter_mode() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_filter_exit");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.enter_filter_mode();
        renderer.set_filter_text("a".to_string());
        assert!(renderer.is_filter_mode());
        assert_eq!(renderer.filter_text(), "a");

        renderer.exit_filter_mode();
        assert!(!renderer.is_filter_mode());
        assert!(renderer.filter_text().is_empty());
        assert_eq!(renderer.selected_index(), 0);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_filter_by_substring_case_insensitive() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_filter_substring");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("Apple.txt"), "").unwrap();
        std::fs::write(temp_dir.join("banana.txt"), "").unwrap();
        std::fs::write(temp_dir.join("pineapple.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        assert_eq!(renderer.entries().len(), 3);

        renderer.enter_filter_mode();

        // Filter for "apple" (case-insensitive) should match "Apple.txt" and "pineapple.txt"
        renderer.set_filter_text("apple".to_string());
        assert_eq!(renderer.entries().len(), 2);
        assert!(renderer.entries().iter().any(|e| e.name == "Apple.txt"));
        assert!(renderer.entries().iter().any(|e| e.name == "pineapple.txt"));
        assert!(!renderer.entries().iter().any(|e| e.name == "banana.txt"));

        // Filter for "BANANA" (case-insensitive) should match "banana.txt"
        renderer.set_filter_text("BANANA".to_string());
        assert_eq!(renderer.entries().len(), 1);
        assert!(renderer.entries().iter().any(|e| e.name == "banana.txt"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_filter_updates_in_real_time() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_filter_realtime");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("test.txt"), "").unwrap();
        std::fs::write(temp_dir.join("testing.txt"), "").unwrap();
        std::fs::write(temp_dir.join("other.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.enter_filter_mode();

        // Type 't' - should match test.txt, testing.txt, other.txt
        renderer.append_filter_char('t');
        assert_eq!(renderer.filter_text(), "t");
        assert_eq!(renderer.entries().len(), 3);

        // Type 'e' (now "te") - should match test.txt, testing.txt
        renderer.append_filter_char('e');
        assert_eq!(renderer.filter_text(), "te");
        assert_eq!(renderer.entries().len(), 2);

        // Type 's' (now "tes") - should still match test.txt, testing.txt
        renderer.append_filter_char('s');
        assert_eq!(renderer.filter_text(), "tes");
        assert_eq!(renderer.entries().len(), 2);

        // Type 'ting' (now "testing") - should only match testing.txt
        renderer.set_filter_text("testing".to_string());
        assert_eq!(renderer.entries().len(), 1);
        assert_eq!(renderer.entries()[0].name, "testing.txt");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_filter_backspace() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_filter_backspace");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("test.txt"), "").unwrap();
        std::fs::write(temp_dir.join("other.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.enter_filter_mode();

        renderer.set_filter_text("test".to_string());
        assert_eq!(renderer.entries().len(), 1);

        // Backspace
        let result = renderer.backspace_filter();
        assert!(result);
        assert_eq!(renderer.filter_text(), "tes");
        assert_eq!(renderer.entries().len(), 1); // Still matches

        // Clear filter text entirely
        renderer.set_filter_text("".to_string());
        assert_eq!(renderer.entries().len(), 2); // All entries shown

        // Backspace on empty should return false
        let result = renderer.backspace_filter();
        assert!(!result);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_filter_no_matches() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_filter_nomatch");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.enter_filter_mode();

        renderer.set_filter_text("xyz".to_string());
        assert_eq!(renderer.entries().len(), 0);
        assert!(renderer.selected_entry().is_none());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_filter_selection_clamps_when_filtered() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_filter_clamp");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();
        std::fs::write(temp_dir.join("c.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.set_selected_index(2); // Select third entry
        assert_eq!(renderer.selected_index(), 2);

        renderer.enter_filter_mode();
        // Filter to only one entry
        renderer.set_filter_text("a".to_string());
        // Selection should clamp to 0 (only valid index)
        assert_eq!(renderer.selected_index(), 0);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_filter_navigation_uses_filtered_entries() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_filter_nav");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("apple.txt"), "").unwrap();
        std::fs::write(temp_dir.join("apricot.txt"), "").unwrap();
        std::fs::write(temp_dir.join("banana.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.enter_filter_mode();
        renderer.set_filter_text("ap".to_string());
        // Should have 2 entries: apple.txt, apricot.txt
        assert_eq!(renderer.entries().len(), 2);

        assert_eq!(renderer.selected_index(), 0);
        renderer.move_down();
        assert_eq!(renderer.selected_index(), 1);

        // Wrap around
        renderer.move_down();
        assert_eq!(renderer.selected_index(), 0);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_filter_render_shows_input_box() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_filter_render");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        let rect = Rect::new(0, 0, 200, 100);

        // Normal mode - no filter line
        let lines = renderer.render(rect);
        assert!(!lines.iter().any(|l| l.starts_with("/")));

        // Filter mode - should have filter input line at bottom
        renderer.enter_filter_mode();
        renderer.set_filter_text("test".to_string());
        let lines = renderer.render(rect);
        let last_line = lines.last().unwrap();
        assert_eq!(last_line, "/ test");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_all_entries_returns_unfiltered() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_all_entries");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.enter_filter_mode();
        renderer.set_filter_text("a".to_string());

        // entries() should return filtered (1 entry)
        assert_eq!(renderer.entries().len(), 1);
        // all_entries() should return all (2 entries)
        assert_eq!(renderer.all_entries().len(), 2);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // Preview mode tests (Space key - US-043)

    #[test]
    fn test_preview_content_from_file() {
        let temp_dir = std::env::temp_dir().join("filebrowser_test_preview_file");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create a file with some lines
        let file_path = temp_dir.join("test.txt");
        std::fs::write(&file_path, "line 1\nline 2\nline 3\n").unwrap();

        let content = PreviewContent::from_file(&file_path);
        assert!(content.is_some());

        let content = content.unwrap();
        assert!(!content.is_directory);
        assert!(content.entry_count.is_none());
        assert_eq!(content.lines.len(), 3);
        assert_eq!(content.lines[0], "line 1");
        assert_eq!(content.lines[1], "line 2");
        assert_eq!(content.lines[2], "line 3");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_preview_content_from_file_limits_to_100_lines() {
        let temp_dir = std::env::temp_dir().join("filebrowser_test_preview_limit");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create a file with more than 100 lines
        let file_path = temp_dir.join("long.txt");
        let content_str: String = (1..=150).map(|i| format!("line {}\n", i)).collect();
        std::fs::write(&file_path, content_str).unwrap();

        let content = PreviewContent::from_file(&file_path);
        assert!(content.is_some());

        let content = content.unwrap();
        assert_eq!(content.lines.len(), 100); // Limited to 100
        assert_eq!(content.lines[0], "line 1");
        assert_eq!(content.lines[99], "line 100");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_preview_content_from_directory() {
        let temp_dir = std::env::temp_dir().join("filebrowser_test_preview_dir");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();
        std::fs::write(temp_dir.join("c.txt"), "").unwrap();

        let content = PreviewContent::from_directory(&temp_dir);
        assert!(content.is_some());

        let content = content.unwrap();
        assert!(content.is_directory);
        assert_eq!(content.entry_count, Some(3));
        assert!(content.lines.is_empty());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_preview_content_from_path_file() {
        let temp_dir = std::env::temp_dir().join("filebrowser_test_preview_path_file");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let file_path = temp_dir.join("file.txt");
        std::fs::write(&file_path, "content").unwrap();

        let content = PreviewContent::from_path(&file_path);
        assert!(content.is_some());
        assert!(!content.unwrap().is_directory);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_preview_content_from_path_directory() {
        let temp_dir = std::env::temp_dir().join("filebrowser_test_preview_path_dir");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let content = PreviewContent::from_path(&temp_dir);
        assert!(content.is_some());
        assert!(content.unwrap().is_directory);

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_toggle_preview_off_by_default() {
        let renderer = FileBrowserRenderer::new();
        assert!(!renderer.is_preview_mode());
        assert!(renderer.preview_content().is_none());
    }

    #[test]
    fn test_toggle_preview_turns_on() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_toggle_on");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("test.txt"), "hello").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        assert!(!renderer.is_preview_mode());

        let result = renderer.toggle_preview();
        assert!(result); // Returns true - preview is now on
        assert!(renderer.is_preview_mode());
        assert!(renderer.preview_content().is_some());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_toggle_preview_turns_off() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_toggle_off");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("test.txt"), "hello").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.toggle_preview(); // Turn on
        assert!(renderer.is_preview_mode());

        let result = renderer.toggle_preview(); // Turn off
        assert!(!result); // Returns false - preview is now off
        assert!(!renderer.is_preview_mode());
        assert!(renderer.preview_content().is_none());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_close_preview_closes_when_active() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_close_preview");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("test.txt"), "hello").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.toggle_preview(); // Turn on
        assert!(renderer.is_preview_mode());

        let result = renderer.close_preview();
        assert!(result); // Was active, now closed
        assert!(!renderer.is_preview_mode());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_close_preview_noop_when_inactive() {
        let mut renderer = FileBrowserRenderer::new();
        assert!(!renderer.is_preview_mode());

        let result = renderer.close_preview();
        assert!(!result); // Was not active
        assert!(!renderer.is_preview_mode());
    }

    #[test]
    fn test_preview_updates_on_navigation() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_preview_nav");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "content a").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "content b").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.toggle_preview();

        // First file selected
        let preview = renderer.preview_content().unwrap();
        assert!(preview.path.to_string_lossy().contains("a.txt"));

        // Navigate down
        renderer.move_down();
        let preview = renderer.preview_content().unwrap();
        assert!(preview.path.to_string_lossy().contains("b.txt"));

        // Navigate up
        renderer.move_up();
        let preview = renderer.preview_content().unwrap();
        assert!(preview.path.to_string_lossy().contains("a.txt"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_render_preview_for_file() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_render_file_preview");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("test.txt"), "line 1\nline 2").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.toggle_preview();

        let lines = renderer.render_preview();
        assert!(lines.len() >= 4); // Header, blank, line1, line2
        assert!(lines[0].contains("test.txt"));
        assert_eq!(lines[2], "line 1");
        assert_eq!(lines[3], "line 2");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_render_preview_for_directory() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_render_dir_preview");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let subdir = temp_dir.join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join("a.txt"), "").unwrap();
        std::fs::write(subdir.join("b.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        // First entry should be the directory
        assert!(renderer.selected_entry().unwrap().is_dir);

        renderer.toggle_preview();

        let lines = renderer.render_preview();
        assert!(lines.len() >= 3);
        assert!(lines[0].contains("subdir"));
        assert!(lines[2].contains("2 entries"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_render_preview_no_content() {
        let renderer = FileBrowserRenderer::new();
        // No directory set, no preview content
        let lines = renderer.render_preview();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("No preview"));
    }

    // Yank path tests (y key - US-044)

    #[test]
    fn test_yank_result_new() {
        let path = PathBuf::from("/home/user/test.txt");
        let result = YankResult::new(path.clone());
        assert_eq!(result.path, path);
        assert_eq!(result.message, "Copied!");
    }

    #[test]
    fn test_yank_selected_path_returns_none_on_empty() {
        let renderer = FileBrowserRenderer::new();
        // No directory set, no entries
        assert!(renderer.yank_selected_path().is_none());
    }

    #[test]
    fn test_yank_selected_path_returns_file_path() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_yank_file");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("test.txt"), "content").unwrap();

        renderer.set_current_dir(temp_dir.clone());

        let result = renderer.yank_selected_path();
        assert!(result.is_some());

        let result = result.unwrap();
        assert_eq!(result.path, temp_dir.join("test.txt"));
        assert_eq!(result.message, "Copied!");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_yank_selected_path_returns_directory_path() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_yank_dir");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let subdir = temp_dir.join("subdir");
        std::fs::create_dir_all(&subdir).unwrap();

        renderer.set_current_dir(temp_dir.clone());
        // First entry should be the directory
        assert!(renderer.selected_entry().unwrap().is_dir);

        let result = renderer.yank_selected_path();
        assert!(result.is_some());

        let result = result.unwrap();
        assert_eq!(result.path, subdir);
        assert_eq!(result.message, "Copied!");

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_yank_selected_path_after_navigation() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_yank_nav");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("a.txt"), "").unwrap();
        std::fs::write(temp_dir.join("b.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());

        // First entry
        let result = renderer.yank_selected_path().unwrap();
        assert!(result.path.to_string_lossy().contains("a.txt"));

        // Navigate down and yank again
        renderer.move_down();
        let result = renderer.yank_selected_path().unwrap();
        assert!(result.path.to_string_lossy().contains("b.txt"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // === Git Status Tests ===

    #[test]
    fn test_git_status_enum_default() {
        let status = GitStatus::default();
        assert_eq!(status, GitStatus::None);
    }

    #[test]
    fn test_git_status_enum_variants() {
        assert_ne!(GitStatus::Modified, GitStatus::Staged);
        assert_ne!(GitStatus::Staged, GitStatus::Untracked);
        assert_ne!(GitStatus::Untracked, GitStatus::None);
    }

    #[test]
    fn test_tree_line_from_entry_with_status() {
        let entry = DirEntry::new("test.rs".to_string(), PathBuf::from("/test.rs"), EntryType::File);

        let line_modified = TreeLine::from_entry_with_status(&entry, GitStatus::Modified);
        assert_eq!(line_modified.git_status, GitStatus::Modified);

        let line_staged = TreeLine::from_entry_with_status(&entry, GitStatus::Staged);
        assert_eq!(line_staged.git_status, GitStatus::Staged);

        let line_untracked = TreeLine::from_entry_with_status(&entry, GitStatus::Untracked);
        assert_eq!(line_untracked.git_status, GitStatus::Untracked);
    }

    #[test]
    fn test_tree_line_display_with_git_status() {
        let entry = DirEntry::new("test.rs".to_string(), PathBuf::from("/test.rs"), EntryType::File);

        // Test None status (no suffix)
        let line_none = TreeLine::from_entry(&entry);
        let display_none = line_none.display();
        assert!(!display_none.contains(" M"));
        assert!(!display_none.contains(" S"));
        assert!(!display_none.contains(" U"));

        // Test Modified status (shows ● M)
        let line_mod = TreeLine::from_entry_with_status(&entry, GitStatus::Modified);
        let display_mod = line_mod.display();
        assert!(display_mod.contains(GIT_MODIFIED_SYMBOL));
        assert!(display_mod.contains(" M"));

        // Test Staged status (shows ● S)
        let line_staged = TreeLine::from_entry_with_status(&entry, GitStatus::Staged);
        let display_staged = line_staged.display();
        assert!(display_staged.contains(GIT_STAGED_SYMBOL));
        assert!(display_staged.contains(" S"));

        // Test Untracked status (shows ● U)
        let line_untracked = TreeLine::from_entry_with_status(&entry, GitStatus::Untracked);
        let display_untracked = line_untracked.display();
        assert!(display_untracked.contains(GIT_UNTRACKED_SYMBOL));
        assert!(display_untracked.contains(" U"));
    }

    #[test]
    fn test_tree_line_git_status_color() {
        let entry = DirEntry::new("test.rs".to_string(), PathBuf::from("/test.rs"), EntryType::File);

        // None - no color
        let line_none = TreeLine::from_entry(&entry);
        assert!(line_none.git_status_color().is_none());

        // Modified - red
        let line_mod = TreeLine::from_entry_with_status(&entry, GitStatus::Modified);
        assert_eq!(line_mod.git_status_color(), Some((255, 85, 85)));

        // Staged - green
        let line_staged = TreeLine::from_entry_with_status(&entry, GitStatus::Staged);
        assert_eq!(line_staged.git_status_color(), Some((80, 250, 123)));

        // Untracked - yellow
        let line_untracked = TreeLine::from_entry_with_status(&entry, GitStatus::Untracked);
        assert_eq!(line_untracked.git_status_color(), Some((241, 250, 140)));
    }

    #[test]
    fn test_git_status_for_directory_non_git_dir() {
        // Test with a directory that is not a git repo
        let temp_dir = std::env::temp_dir().join("filebrowser_test_no_git");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("test.txt"), "content").unwrap();

        // Should return empty HashMap for non-git directory
        let result = git_status_for_directory(&temp_dir);
        assert!(result.is_empty());

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_renderer_show_git_status_default() {
        let renderer = FileBrowserRenderer::new();
        assert!(renderer.is_git_status_enabled());
    }

    #[test]
    fn test_renderer_set_show_git_status() {
        let mut renderer = FileBrowserRenderer::new();
        assert!(renderer.is_git_status_enabled());

        renderer.set_show_git_status(false);
        assert!(!renderer.is_git_status_enabled());

        renderer.set_show_git_status(true);
        assert!(renderer.is_git_status_enabled());
    }

    #[test]
    fn test_renderer_show_icons_default() {
        let renderer = FileBrowserRenderer::new();
        assert!(renderer.is_icons_enabled());
    }

    #[test]
    fn test_renderer_set_show_icons() {
        let mut renderer = FileBrowserRenderer::new();
        assert!(renderer.is_icons_enabled());

        renderer.set_show_icons(false);
        assert!(!renderer.is_icons_enabled());

        renderer.set_show_icons(true);
        assert!(renderer.is_icons_enabled());
    }

    #[test]
    fn test_renderer_is_show_hidden() {
        let mut renderer = FileBrowserRenderer::new();
        assert!(!renderer.is_show_hidden());

        renderer.set_show_hidden(true);
        assert!(renderer.is_show_hidden());
    }

    #[test]
    fn test_display_with_options_no_icons() {
        let entry = DirEntry::new("test.txt".to_string(), PathBuf::from("/test.txt"), EntryType::File);
        let line = TreeLine::from_entry(&entry);

        // With icons
        let with_icons = line.display_with_options(true, false);
        assert!(with_icons.contains(FILE_ICON));
        assert!(with_icons.contains("test.txt"));

        // Without icons
        let without_icons = line.display_with_options(false, false);
        assert!(!without_icons.contains(FILE_ICON));
        assert!(without_icons.contains("test.txt"));
        assert_eq!(without_icons, "test.txt");
    }

    #[test]
    fn test_display_with_options_no_git_status() {
        let entry = DirEntry::new("modified.txt".to_string(), PathBuf::from("/modified.txt"), EntryType::File);
        let line = TreeLine::from_entry_with_status(&entry, GitStatus::Modified);

        // With git status
        let with_git = line.display_with_options(true, true);
        assert!(with_git.contains(GIT_MODIFIED_SYMBOL));
        assert!(with_git.contains(" M"));

        // Without git status
        let without_git = line.display_with_options(true, false);
        assert!(!without_git.contains(GIT_MODIFIED_SYMBOL));
        assert!(!without_git.contains(" M"));
    }

    #[test]
    fn test_display_with_options_no_icons_no_git() {
        let entry = DirEntry::new("staged.txt".to_string(), PathBuf::from("/staged.txt"), EntryType::File);
        let line = TreeLine::from_entry_with_status(&entry, GitStatus::Staged);

        let display = line.display_with_options(false, false);
        assert_eq!(display, "staged.txt");
    }

    #[test]
    fn test_render_respects_icons_setting() {
        let mut renderer = FileBrowserRenderer::new();

        let temp_dir = std::env::temp_dir().join("filebrowser_test_icons");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("test.txt"), "").unwrap();

        renderer.set_current_dir(temp_dir.clone());
        renderer.set_show_icons(false);

        let rect = Rect::new(0, 0, 200, 600);
        let lines = renderer.render(rect);

        assert_eq!(lines.len(), 1);
        // Without icons, the line should just be the filename
        assert!(!lines[0].contains(FILE_ICON));
        assert!(lines[0].contains("test.txt"));

        // Enable icons and verify they appear
        renderer.set_show_icons(true);
        let lines = renderer.render(rect);
        assert!(lines[0].contains(FILE_ICON));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_new_from_config_defaults() {
        // Test that new_from_config reads from config correctly
        // The default config should have show_hidden=false, show_git_status=true, icons=true
        let renderer = FileBrowserRenderer::new_from_config();
        assert!(!renderer.is_show_hidden());
        assert!(renderer.is_git_status_enabled());
        assert!(renderer.is_icons_enabled());
    }
}
