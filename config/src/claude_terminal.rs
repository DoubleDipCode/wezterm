//! Claude Terminal configuration settings.
//!
//! This module defines the configuration schema for Claude Terminal features:
//! - Status colors for different Claude Code states
//! - Border rendering options
//! - Animation settings
//! - Auto-tiling parameters
//! - File browser options
//! - Status detection patterns

use crate::RgbaColor;
use luahelper::impl_lua_conversion_dynamic;
use wezterm_dynamic::{FromDynamic, ToDynamic};

/// Color configuration for Claude Code status indicators.
#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct ClaudeStatusColors {
    /// Color for idle status (default: #ff5555 - red)
    #[dynamic(default = "default_idle_color")]
    pub idle: RgbaColor,

    /// Color for running status (default: #50fa7b - green)
    #[dynamic(default = "default_running_color")]
    pub running: RgbaColor,

    /// Color for awaiting permission status (default: #f1fa8c - yellow)
    #[dynamic(default = "default_permission_color")]
    pub awaiting_permission: RgbaColor,

    /// Color for error status (default: #ffb86c - orange)
    #[dynamic(default = "default_error_color")]
    pub error: RgbaColor,
}

impl Default for ClaudeStatusColors {
    fn default() -> Self {
        Self {
            idle: default_idle_color(),
            running: default_running_color(),
            awaiting_permission: default_permission_color(),
            error: default_error_color(),
        }
    }
}

impl_lua_conversion_dynamic!(ClaudeStatusColors);

fn default_idle_color() -> RgbaColor {
    // #ff5555 (red)
    RgbaColor::from((255, 85, 85))
}

fn default_running_color() -> RgbaColor {
    // #50fa7b (green)
    RgbaColor::from((80, 250, 123))
}

fn default_permission_color() -> RgbaColor {
    // #f1fa8c (yellow)
    RgbaColor::from((241, 250, 140))
}

fn default_error_color() -> RgbaColor {
    // #ffb86c (orange)
    RgbaColor::from((255, 184, 108))
}

/// Border rendering configuration.
#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct ClaudeBorderConfig {
    /// Border width in pixels (default: 4)
    #[dynamic(default = "default_border_width")]
    pub width: u32,

    /// Glow blur radius in pixels (default: 8)
    #[dynamic(default = "default_glow_radius")]
    pub glow_radius: u32,

    /// Glow opacity from 0.0 to 1.0 (default: 0.5)
    #[dynamic(default = "default_glow_opacity")]
    pub glow_opacity: f32,
}

impl Default for ClaudeBorderConfig {
    fn default() -> Self {
        Self {
            width: default_border_width(),
            glow_radius: default_glow_radius(),
            glow_opacity: default_glow_opacity(),
        }
    }
}

impl_lua_conversion_dynamic!(ClaudeBorderConfig);

fn default_border_width() -> u32 {
    4
}

fn default_glow_radius() -> u32 {
    8
}

fn default_glow_opacity() -> f32 {
    0.5
}

/// Animation configuration.
#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct ClaudeAnimationConfig {
    /// Enable pulse animation for idle status (default: true)
    #[dynamic(default = "crate::default_true")]
    pub pulse_idle: bool,

    /// Enable blink animation for permission prompts (default: true)
    #[dynamic(default = "crate::default_true")]
    pub blink_permission: bool,

    /// Blink rate in Hz for permission prompts (default: 1)
    #[dynamic(default = "default_blink_rate")]
    pub blink_rate_hz: f32,

    /// Pulse period in seconds for idle status (default: 2.0)
    #[dynamic(default = "default_pulse_period")]
    pub pulse_period_secs: f32,
}

impl Default for ClaudeAnimationConfig {
    fn default() -> Self {
        Self {
            pulse_idle: true,
            blink_permission: true,
            blink_rate_hz: default_blink_rate(),
            pulse_period_secs: default_pulse_period(),
        }
    }
}

impl_lua_conversion_dynamic!(ClaudeAnimationConfig);

fn default_blink_rate() -> f32 {
    1.0
}

fn default_pulse_period() -> f32 {
    2.0
}

/// Auto-tiling configuration.
#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct ClaudeTilingConfig {
    /// Minimum pane width in columns (default: 80)
    #[dynamic(default = "default_min_cols")]
    pub min_pane_cols: u32,

    /// Minimum pane height in rows (default: 24)
    #[dynamic(default = "default_min_rows")]
    pub min_pane_rows: u32,

    /// Animation duration in milliseconds (default: 150)
    #[dynamic(default = "default_animation_ms")]
    pub animation_ms: u32,
}

impl Default for ClaudeTilingConfig {
    fn default() -> Self {
        Self {
            min_pane_cols: default_min_cols(),
            min_pane_rows: default_min_rows(),
            animation_ms: default_animation_ms(),
        }
    }
}

impl_lua_conversion_dynamic!(ClaudeTilingConfig);

fn default_min_cols() -> u32 {
    80
}

fn default_min_rows() -> u32 {
    24
}

fn default_animation_ms() -> u32 {
    150
}

/// File browser configuration.
#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct ClaudeFileBrowserConfig {
    /// Enable file browser (default: true)
    #[dynamic(default = "crate::default_true")]
    pub enabled: bool,

    /// Show hidden files (default: false)
    #[dynamic(default)]
    pub show_hidden: bool,

    /// Show git status indicators (default: true)
    #[dynamic(default = "crate::default_true")]
    pub show_git_status: bool,

    /// Show icons for files and folders (default: true)
    #[dynamic(default = "crate::default_true")]
    pub icons: bool,
}

impl Default for ClaudeFileBrowserConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            show_hidden: false,
            show_git_status: true,
            icons: true,
        }
    }
}

impl_lua_conversion_dynamic!(ClaudeFileBrowserConfig);

/// Status detection configuration.
#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct ClaudeDetectionConfig {
    /// Additional permission detection patterns (appended to built-in patterns)
    #[dynamic(default)]
    pub permission_patterns: Vec<String>,

    /// Additional running detection patterns (appended to built-in patterns)
    #[dynamic(default)]
    pub running_patterns: Vec<String>,

    /// Additional error detection patterns (appended to built-in patterns)
    #[dynamic(default)]
    pub error_patterns: Vec<String>,

    /// Status detection polling interval in milliseconds (default: 100)
    #[dynamic(default = "default_detection_interval")]
    pub polling_interval_ms: u32,

    /// Time in milliseconds after which a non-Idle status resets to Idle
    /// if no new activity patterns are detected (default: 3000 = 3 seconds).
    /// Set to 0 to disable timeout.
    #[dynamic(default = "default_idle_timeout")]
    pub idle_timeout_ms: u32,
}

impl Default for ClaudeDetectionConfig {
    fn default() -> Self {
        Self {
            permission_patterns: Vec::new(),
            running_patterns: Vec::new(),
            error_patterns: Vec::new(),
            polling_interval_ms: default_detection_interval(),
            idle_timeout_ms: default_idle_timeout(),
        }
    }
}

impl_lua_conversion_dynamic!(ClaudeDetectionConfig);

fn default_detection_interval() -> u32 {
    100
}

fn default_idle_timeout() -> u32 {
    3000 // 3 seconds
}

/// Main Claude Terminal configuration.
#[derive(Debug, Clone, FromDynamic, ToDynamic)]
pub struct ClaudeTerminalConfig {
    /// Status color configuration
    #[dynamic(default)]
    pub status_colors: ClaudeStatusColors,

    /// Border rendering configuration
    #[dynamic(default)]
    pub border: ClaudeBorderConfig,

    /// Animation configuration
    #[dynamic(default)]
    pub animation: ClaudeAnimationConfig,

    /// Auto-tiling configuration
    #[dynamic(default)]
    pub tiling: ClaudeTilingConfig,

    /// File browser configuration
    #[dynamic(default)]
    pub file_browser: ClaudeFileBrowserConfig,

    /// Status detection configuration
    #[dynamic(default)]
    pub detection: ClaudeDetectionConfig,
}

impl Default for ClaudeTerminalConfig {
    fn default() -> Self {
        Self {
            status_colors: ClaudeStatusColors::default(),
            border: ClaudeBorderConfig::default(),
            animation: ClaudeAnimationConfig::default(),
            tiling: ClaudeTilingConfig::default(),
            file_browser: ClaudeFileBrowserConfig::default(),
            detection: ClaudeDetectionConfig::default(),
        }
    }
}

impl_lua_conversion_dynamic!(ClaudeTerminalConfig);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ClaudeTerminalConfig::default();
        assert_eq!(config.border.width, 4);
        assert_eq!(config.border.glow_radius, 8);
        assert!((config.border.glow_opacity - 0.5).abs() < 0.001);
        assert!(config.animation.pulse_idle);
        assert!(config.animation.blink_permission);
        assert_eq!(config.tiling.min_pane_cols, 80);
        assert_eq!(config.tiling.min_pane_rows, 24);
        assert!(config.file_browser.enabled);
        assert!(!config.file_browser.show_hidden);
        assert!(config.file_browser.show_git_status);
    }

    #[test]
    fn test_default_colors() {
        let colors = ClaudeStatusColors::default();
        // Just verify they're created without panicking
        let _ = colors.idle;
        let _ = colors.running;
        let _ = colors.awaiting_permission;
        let _ = colors.error;
    }

    #[test]
    fn test_detection_defaults() {
        let detection = ClaudeDetectionConfig::default();
        assert!(detection.permission_patterns.is_empty());
        assert!(detection.running_patterns.is_empty());
        assert!(detection.error_patterns.is_empty());
        assert_eq!(detection.polling_interval_ms, 100);
    }
}
