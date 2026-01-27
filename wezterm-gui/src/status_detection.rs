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
