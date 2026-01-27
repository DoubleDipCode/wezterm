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
}
