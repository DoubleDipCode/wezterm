//! Claude Code status detection module.
//!
//! Provides types and logic for detecting the current status of Claude Code
//! from PTY output patterns.

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
