# Contributing to Claude Terminal

Thank you for your interest in contributing to Claude Terminal! This guide covers the specific additions and modifications made to the WezTerm codebase for Claude Terminal functionality.

For general WezTerm contribution guidelines, please also see [CONTRIBUTING.md](CONTRIBUTING.md).

## Build Instructions

### Prerequisites

1. **Rust Toolchain**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **macOS Dependencies** (via Homebrew)
   ```bash
   # Core build dependencies
   brew install pkg-config openssl cmake

   # Optional: for icon generation
   brew install imagemagick
   ```

3. **Nerd Font** (for file browser icons)
   ```bash
   brew tap homebrew/cask-fonts
   brew install --cask font-jetbrains-mono-nerd-font
   ```

### Building

```bash
# Clone the repository
git clone https://github.com/DoubleDipCode/wezterm.git claude-terminal
cd claude-terminal
git checkout claude-terminal-main

# Type check (fast, ~2 minutes)
cargo check

# Debug build (includes debug symbols)
cargo build

# Release build (~4 minutes, optimized)
cargo build --release
```

### Running

```bash
# Debug build
./target/debug/wezterm-gui

# Release build
./target/release/wezterm-gui

# With backtrace on panic
RUST_BACKTRACE=1 ./target/debug/wezterm-gui
```

## Development Workflow

### Iterating Quickly

Use `cargo check` for fast type checking during development:

```bash
cargo check
```

This validates your code without generating binaries (~18 seconds on warm build).

### Running Tests

```bash
# All tests
cargo test --all

# Specific crate tests
cargo test -p wezterm-gui

# Specific test module
cargo test -p wezterm-gui status_detection
cargo test -p wezterm-gui filebrowser

# With output
cargo test -p wezterm-gui -- --nocapture
```

### Key Test Modules

| Module | Location | Tests |
|--------|----------|-------|
| Status Detection | `wezterm-gui/src/status_detection.rs` | Pattern matching, color mapping |
| File Browser | `wezterm-gui/src/overlay/filebrowser.rs` | Navigation, filtering, preview |
| Auto-Tiling | `mux/src/layout.rs` | Grid calculation, animations |

### Code Formatting

```bash
rustup component add rustfmt-preview  # one-time setup
cargo fmt --all
```

### Hot Reload

WezTerm supports config hot reload. Edit `~/.wezterm.lua` and changes apply immediately without restart. For Rust code changes, you must rebuild and restart.

## Architecture Overview

Claude Terminal extends WezTerm with four main systems:

```
┌─────────────────────────────────────────────────────────────┐
│                     Claude Terminal                          │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Status    │  │   Border    │  │    File Browser     │  │
│  │  Detection  │  │  Rendering  │  │                     │  │
│  │             │  │   + Glow    │  │                     │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         │                │                     │             │
│         ▼                ▼                     ▼             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                   Auto-Tiling                         │   │
│  │              Layout Management                        │   │
│  └──────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                     WezTerm Core                             │
│  (terminal emulation, PTY handling, GPU rendering, config)   │
└─────────────────────────────────────────────────────────────┘
```

### 1. Status Detection System

**Files:**
- `wezterm-gui/src/status_detection.rs` - Pattern matching and color mapping
- `mux/src/pane.rs` - ClaudeStatus enum and Pane trait extensions
- `mux/src/localpane.rs` - Ring buffer for PTY output

**Flow:**
1. PTY output written to 8KB ring buffer (`PtyOutputBuffer`)
2. Every 100ms, polling loop reads buffer and calls `ClaudeStatus::detect()`
3. Pattern matching checks permission → error → running → idle
4. Status stored in pane, triggers border re-render

**Adding new patterns:**
```rust
// In status_detection.rs
pub const PERMISSION_PATTERNS: &[&str] = &[
    "Allow?",
    "Do you want to",
    // Add your pattern here
];
```

Or via config:
```lua
config.claude_terminal = {
  detection = {
    permission_patterns = { "Custom pattern" }
  }
}
```

### 2. Border Rendering System

**Files:**
- `wezterm-gui/src/termwindow/render/pane_border.rs` - Border geometry and pipeline
- `wezterm-gui/src/border.wgsl` - Border vertex/fragment shaders
- `wezterm-gui/src/blur_h.wgsl` - Horizontal blur pass
- `wezterm-gui/src/blur_v.wgsl` - Vertical blur pass
- `wezterm-gui/src/termwindow/webgpu.rs` - Texture management

**Rendering pipeline:**
1. Collect border data (color, position) for each pane
2. Generate quad vertices for 4px borders
3. Render borders to offscreen texture
4. Apply 2-pass Gaussian blur (horizontal + vertical)
5. Composite blurred glow onto screen with alpha blending

**Animations:**
- Idle: Sin-wave pulse (2s period)
- Awaiting Permission: 1Hz blink (on/dim toggle)
- Running/Error: Solid color

### 3. Auto-Tiling System

**Files:**
- `mux/src/layout.rs` - Grid calculation, constraints, animations
- `wezterm-gui/src/termwindow/spawn.rs` - Keybinding handlers
- `wezterm-gui/src/termwindow/mod.rs` - Animation integration

**Key types:**
- `TilingLayout` - Pane ordering and lock state
- `PaneConstraints` - Minimum size (80x24 chars default)
- `LayoutAnimation` - Smooth 150ms transitions

**Grid dimensions for N panes:**
| Panes | Grid |
|-------|------|
| 1 | 1x1 |
| 2 | 2x1 |
| 3-4 | 2x2 |
| 5-6 | 3x2 |
| 7-9 | 3x3 |

### 4. File Browser System

**Files:**
- `wezterm-gui/src/overlay/filebrowser.rs` - Core renderer and logic
- `mux/src/pane.rs` - PaneType enum, CWD tracking

**Features:**
- Vim-style navigation (j/k/h/l)
- Fuzzy filtering (/)
- File preview (Space)
- Git status indicators
- OSC 7 CWD tracking

**State management:**
- `FileBrowserRenderer` holds all state (entries, selection, filter)
- Updates on focus change via `update_file_browser_for_focus()`
- Keyboard events processed by overlay system

## Configuration

Configuration is defined in `config/src/claude_terminal.rs`:

```rust
pub struct ClaudeTerminalConfig {
    pub status_colors: ClaudeStatusColors,
    pub border: ClaudeBorderConfig,
    pub animation: ClaudeAnimationConfig,
    pub tiling: ClaudeTilingConfig,
    pub file_browser: ClaudeFileBrowserConfig,
    pub detection: ClaudeDetectionConfig,
}
```

Access in code:
```rust
let config = config::configuration();
let border_width = config.claude_terminal.border.width;
```

## Key Crates

| Crate | Purpose |
|-------|---------|
| `wezterm-gui` | GUI rendering, keybindings, overlays |
| `mux` | Multiplexer, panes, tabs, layout |
| `config` | Configuration parsing and defaults |
| `term` | Terminal emulation (upstream WezTerm) |
| `window` | Window management (upstream WezTerm) |

## Common Patterns

### Adding a Keybinding

1. Add variant to `KeyAssignment` enum in `config/src/keyassignment.rs`
2. Add command definition in `wezterm-gui/src/commands.rs`
3. Add handler in `wezterm-gui/src/termwindow/mod.rs` `perform_key_assignment()`
4. Implement action in appropriate module (e.g., `spawn.rs`)

### Adding Configuration Option

1. Add field to appropriate struct in `config/src/claude_terminal.rs`
2. Add `#[dynamic(default = "default_fn")]` attribute
3. Access via `config::configuration().claude_terminal.section.field`

### Adding a Test

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_behavior() {
        // Arrange
        let input = ...;

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

## Known Issues

- `mux` crate tests don't run in isolation due to `chrono` feature gating
- Use `cargo test -p wezterm-gui` for reliable test runs
- Some warnings from upstream WezTerm code (184 from `window` crate)

## WezTerm Documentation

For deeper understanding of the WezTerm codebase:

- [WezTerm Documentation](https://wezfurlong.org/wezterm/)
- [WezTerm GitHub](https://github.com/wezterm/wezterm)
- [Terminal Escape Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
- [wgpu Documentation](https://docs.rs/wgpu/latest/wgpu/) (GPU rendering)

## Getting Help

- Open an issue for bugs or feature requests
- Check existing issues before creating new ones
- Include reproduction steps and environment details

## License

Claude Terminal inherits WezTerm's MIT license. See [LICENSE](LICENSE.md) for details.
