//! Claude Code status detection module.
//!
//! Provides types and logic for detecting the current status of Claude Code
//! from PTY output patterns.

/// Patterns that indicate Claude Code is waiting for user permission.
/// These are checked with highest priority.
pub const PERMISSION_PATTERNS: &[&str] = &[
    "Allow this action?",
    "Allow once",
    "Allow always",
    "Do you want to proceed?",
    "Press Enter to allow",
    "Yes, proceed",
    "[Y/n]",
    "[y/N]",
    "Approve?",
    "Allow?",
    "Permission required",
    "Waiting for approval",
];

/// Patterns that indicate Claude Code is actively running/processing.
pub const RUNNING_PATTERNS: &[&str] = &[
    "Thinking...",
    "Working...",
    "Processing...",
    "Running...",
    "Executing...",
    "Loading...",
    "Analyzing...",
    "Generating...",
    "Writing...",
    "Reading...",
    "Searching...",
    "⠋",
    "⠙",
    "⠹",
    "⠸",
    "⠼",
    "⠴",
    "⠦",
    "⠧",
    "⠇",
    "⠏",
];

/// Patterns that indicate Claude Code has encountered an error.
pub const ERROR_PATTERNS: &[&str] = &[
    "Error:",
    "ERROR:",
    "error:",
    "Failed:",
    "FAILED:",
    "failed:",
    "Exception:",
    "EXCEPTION:",
    "Panic:",
    "PANIC:",
    "fatal:",
    "Fatal:",
    "FATAL:",
    "✗",
    "✘",
    "❌",
];

/// Represents the current status of a Claude Code session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClaudeStatus {
    /// Claude Code is idle, waiting for user input.
    Idle,
    /// Claude Code is actively processing/running.
    Running,
    /// Claude Code is waiting for user permission to proceed.
    AwaitingPermission,
    /// Claude Code has encountered an error.
    Error,
}

/// RGBA color for status visualization.
///
/// Color values are normalized floats in range [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusColor {
    /// Red component (0.0 - 1.0)
    pub r: f32,
    /// Green component (0.0 - 1.0)
    pub g: f32,
    /// Blue component (0.0 - 1.0)
    pub b: f32,
    /// Alpha component (0.0 - 1.0)
    pub a: f32,
}

impl StatusColor {
    /// Create a new StatusColor from RGBA components.
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create a StatusColor from a hex color string (e.g., "#ff5555").
    ///
    /// Returns None if the string is not a valid hex color.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        })
    }

    /// Get the StatusColor for a given ClaudeStatus.
    ///
    /// Color mapping:
    /// - Idle: #ff5555 (red)
    /// - Running: #50fa7b (green)
    /// - AwaitingPermission: #f1fa8c (yellow)
    /// - Error: #ffb86c (orange)
    pub fn from_status(status: ClaudeStatus) -> Self {
        match status {
            ClaudeStatus::Idle => Self {
                r: 1.0,           // 0xff = 255
                g: 85.0 / 255.0,  // 0x55 = 85
                b: 85.0 / 255.0,  // 0x55 = 85
                a: 1.0,
            },
            ClaudeStatus::Running => Self {
                r: 80.0 / 255.0,  // 0x50 = 80
                g: 250.0 / 255.0, // 0xfa = 250
                b: 123.0 / 255.0, // 0x7b = 123
                a: 1.0,
            },
            ClaudeStatus::AwaitingPermission => Self {
                r: 241.0 / 255.0, // 0xf1 = 241
                g: 250.0 / 255.0, // 0xfa = 250
                b: 140.0 / 255.0, // 0x8c = 140
                a: 1.0,
            },
            ClaudeStatus::Error => Self {
                r: 1.0,           // 0xff = 255
                g: 184.0 / 255.0, // 0xb8 = 184
                b: 108.0 / 255.0, // 0x6c = 108
                a: 1.0,
            },
        }
    }
}

impl Default for ClaudeStatus {
    fn default() -> Self {
        ClaudeStatus::Idle
    }
}

impl ClaudeStatus {
    /// Detect the current Claude Code status from PTY output and process name.
    ///
    /// Checks patterns in priority order:
    /// 1. Permission patterns (highest priority - user action required)
    /// 2. Error patterns (problems need attention)
    /// 3. Running patterns (actively processing)
    /// 4. Idle (default - no activity detected)
    ///
    /// # Arguments
    /// * `pty_output` - The recent PTY output to analyze
    /// * `process_name` - The name of the running process (for future use)
    ///
    /// # Returns
    /// The detected `ClaudeStatus` based on pattern matching
    pub fn detect(pty_output: &str, _process_name: &str) -> Self {
        // Check permission patterns first (highest priority)
        for pattern in PERMISSION_PATTERNS {
            if pty_output.contains(pattern) {
                return ClaudeStatus::AwaitingPermission;
            }
        }

        // Check error patterns second
        for pattern in ERROR_PATTERNS {
            if pty_output.contains(pattern) {
                return ClaudeStatus::Error;
            }
        }

        // Check running patterns third
        for pattern in RUNNING_PATTERNS {
            if pty_output.contains(pattern) {
                return ClaudeStatus::Running;
            }
        }

        // Default to idle
        ClaudeStatus::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_idle() {
        // No patterns present - should be idle
        let output = "$ claude\nWelcome to Claude Code!\n>";
        assert_eq!(ClaudeStatus::detect(output, "claude"), ClaudeStatus::Idle);

        // Empty output is idle
        assert_eq!(ClaudeStatus::detect("", "claude"), ClaudeStatus::Idle);

        // Random text is idle
        assert_eq!(
            ClaudeStatus::detect("Hello world\nSome random text", "claude"),
            ClaudeStatus::Idle
        );
    }

    #[test]
    fn test_detect_running() {
        // Spinner characters
        assert_eq!(
            ClaudeStatus::detect("⠋ Fetching data...", "claude"),
            ClaudeStatus::Running
        );
        assert_eq!(
            ClaudeStatus::detect("⠙ Working on it", "claude"),
            ClaudeStatus::Running
        );

        // Text patterns
        assert_eq!(
            ClaudeStatus::detect("Thinking...\n", "claude"),
            ClaudeStatus::Running
        );
        assert_eq!(
            ClaudeStatus::detect("Processing... please wait", "claude"),
            ClaudeStatus::Running
        );
        assert_eq!(
            ClaudeStatus::detect("Analyzing...", "claude"),
            ClaudeStatus::Running
        );
    }

    #[test]
    fn test_detect_awaiting_permission() {
        // Permission prompts
        assert_eq!(
            ClaudeStatus::detect("Allow this action? [Y/n]", "claude"),
            ClaudeStatus::AwaitingPermission
        );
        assert_eq!(
            ClaudeStatus::detect("Do you want to proceed?", "claude"),
            ClaudeStatus::AwaitingPermission
        );
        assert_eq!(
            ClaudeStatus::detect("Permission required to continue", "claude"),
            ClaudeStatus::AwaitingPermission
        );
        assert_eq!(
            ClaudeStatus::detect("Waiting for approval", "claude"),
            ClaudeStatus::AwaitingPermission
        );
    }

    #[test]
    fn test_detect_error() {
        // Error patterns
        assert_eq!(
            ClaudeStatus::detect("Error: something went wrong", "claude"),
            ClaudeStatus::Error
        );
        assert_eq!(
            ClaudeStatus::detect("FAILED: build failed", "claude"),
            ClaudeStatus::Error
        );
        assert_eq!(
            ClaudeStatus::detect("✗ Test failed", "claude"),
            ClaudeStatus::Error
        );
        assert_eq!(
            ClaudeStatus::detect("fatal: not a git repository", "claude"),
            ClaudeStatus::Error
        );
    }

    #[test]
    fn test_priority_permission_over_error() {
        // Permission should take priority over error
        let output = "Error: something failed\nAllow this action?";
        assert_eq!(
            ClaudeStatus::detect(output, "claude"),
            ClaudeStatus::AwaitingPermission
        );
    }

    #[test]
    fn test_priority_permission_over_running() {
        // Permission should take priority over running
        let output = "⠋ Working...\nDo you want to proceed?";
        assert_eq!(
            ClaudeStatus::detect(output, "claude"),
            ClaudeStatus::AwaitingPermission
        );
    }

    #[test]
    fn test_priority_error_over_running() {
        // Error should take priority over running
        let output = "Thinking...\nError: failed to compile";
        assert_eq!(ClaudeStatus::detect(output, "claude"), ClaudeStatus::Error);
    }

    // StatusColor tests

    #[test]
    fn test_status_color_from_status_idle() {
        // Idle = #ff5555
        let color = StatusColor::from_status(ClaudeStatus::Idle);
        assert_eq!(color.r, 1.0);
        assert!((color.g - 85.0 / 255.0).abs() < 0.001);
        assert!((color.b - 85.0 / 255.0).abs() < 0.001);
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn test_status_color_from_status_running() {
        // Running = #50fa7b
        let color = StatusColor::from_status(ClaudeStatus::Running);
        assert!((color.r - 80.0 / 255.0).abs() < 0.001);
        assert!((color.g - 250.0 / 255.0).abs() < 0.001);
        assert!((color.b - 123.0 / 255.0).abs() < 0.001);
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn test_status_color_from_status_permission() {
        // Permission = #f1fa8c
        let color = StatusColor::from_status(ClaudeStatus::AwaitingPermission);
        assert!((color.r - 241.0 / 255.0).abs() < 0.001);
        assert!((color.g - 250.0 / 255.0).abs() < 0.001);
        assert!((color.b - 140.0 / 255.0).abs() < 0.001);
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn test_status_color_from_status_error() {
        // Error = #ffb86c
        let color = StatusColor::from_status(ClaudeStatus::Error);
        assert_eq!(color.r, 1.0);
        assert!((color.g - 184.0 / 255.0).abs() < 0.001);
        assert!((color.b - 108.0 / 255.0).abs() < 0.001);
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn test_status_color_from_hex_valid() {
        // Test with #ff5555
        let color = StatusColor::from_hex("#ff5555").unwrap();
        assert_eq!(color.r, 1.0);
        assert!((color.g - 85.0 / 255.0).abs() < 0.001);
        assert!((color.b - 85.0 / 255.0).abs() < 0.001);
        assert_eq!(color.a, 1.0);

        // Test without # prefix
        let color = StatusColor::from_hex("50fa7b").unwrap();
        assert!((color.r - 80.0 / 255.0).abs() < 0.001);
        assert!((color.g - 250.0 / 255.0).abs() < 0.001);
        assert!((color.b - 123.0 / 255.0).abs() < 0.001);
    }

    #[test]
    fn test_status_color_from_hex_invalid() {
        // Too short
        assert!(StatusColor::from_hex("#fff").is_none());
        // Too long
        assert!(StatusColor::from_hex("#ff5555ff").is_none());
        // Invalid hex
        assert!(StatusColor::from_hex("#gggggg").is_none());
    }

    #[test]
    fn test_status_color_new() {
        let color = StatusColor::new(0.5, 0.6, 0.7, 0.8);
        assert_eq!(color.r, 0.5);
        assert_eq!(color.g, 0.6);
        assert_eq!(color.b, 0.7);
        assert_eq!(color.a, 0.8);
    }
}
