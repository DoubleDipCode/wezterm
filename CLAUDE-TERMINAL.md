# Claude Terminal

<img height="128" alt="Claude Terminal Icon" src="assets/icon/claude-terminal/placeholder-1024.png" align="left"> *A specialized terminal emulator for running multiple Claude Code instances with intelligent status visualization, auto-tiling, and integrated file browsing.*

Claude Terminal is built as a fork of [WezTerm](https://wezterm.org/), adding features specifically designed for AI-assisted coding workflows.

![Status Borders Demo](docs/screenshots/claude-terminal-demo.png)
*Claude Terminal showing three Claude Code sessions with status-colored borders*

## Features

- **Status-Colored Borders**: Glowing borders indicate Claude Code state at a glance
  - Red (pulsing): Idle - Claude is waiting for input
  - Green: Running - Claude is actively working
  - Yellow (blinking): Awaiting Permission - action required
  - Orange: Error state

- **Auto-Tiling**: Smart pane layouts that automatically arrange 2-6 Claude Code sessions

- **Integrated File Browser**: Vim-style navigation with git status indicators

## Installation

### From Source (macOS)

1. **Install Rust toolchain**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **Clone and build**
   ```bash
   git clone https://github.com/DoubleDipCode/wezterm.git claude-terminal
   cd claude-terminal
   git checkout claude-terminal-main
   cargo build --release
   ```

3. **Run**
   ```bash
   ./target/release/wezterm-gui
   ```

### macOS App Bundle

After building, you can create an app bundle:

```bash
# Copy binary to app bundle
cp target/release/wezterm-gui assets/macos/ClaudeTerminal.app/Contents/MacOS/

# Launch app
open assets/macos/ClaudeTerminal.app
```

### Homebrew (Coming Soon)

```bash
brew install claude-terminal
```

## Keybindings

### Pane Management (Auto-Tiling)

| Keybinding | Action |
|------------|--------|
| `Cmd+N` | Create new pane (auto-tiles) |
| `Cmd+W` | Close current pane (re-tiles remaining) |
| `Cmd+0` | Reset to equal-size layout |
| `Cmd+Shift+Left` | Resize pane left (locks from auto-tile) |
| `Cmd+Shift+Right` | Resize pane right (locks from auto-tile) |
| `Cmd+Shift+Up` | Resize pane up (locks from auto-tile) |
| `Cmd+Shift+Down` | Resize pane down (locks from auto-tile) |

### File Browser

| Keybinding | Action |
|------------|--------|
| `Cmd+Shift+F` | Toggle file browser visibility |
| `j` | Move selection down |
| `k` | Move selection up |
| `l` | Enter directory |
| `h` | Go to parent directory |
| `Enter` | Open file in $EDITOR |
| `/` | Start fuzzy filter |
| `Esc` | Exit filter mode / close preview |
| `Space` | Toggle file preview |
| `y` | Yank (copy) path to clipboard |

## Configuration

Claude Terminal uses the standard WezTerm Lua configuration with additional options. Edit `~/.wezterm.lua`:

```lua
local wezterm = require 'wezterm'
local config = {}

-- Claude Terminal specific settings
config.claude_terminal = {
  -- Status border colors (Dracula theme defaults)
  status_colors = {
    idle = "#ff5555",              -- Red
    running = "#50fa7b",           -- Green
    awaiting_permission = "#f1fa8c", -- Yellow
    error = "#ffb86c",             -- Orange
  },

  -- Border rendering
  border = {
    width = 4,           -- Border width in pixels
    glow_radius = 8,     -- Glow blur radius
    glow_opacity = 0.5,  -- Glow opacity (0.0 - 1.0)
  },

  -- Animations
  animation = {
    pulse_idle = true,        -- Pulse animation for idle status
    blink_permission = true,  -- Blink animation for permission prompts
    blink_rate_hz = 1.0,      -- Blink frequency in Hz
    pulse_period_secs = 2.0,  -- Pulse cycle duration
  },

  -- Auto-tiling
  tiling = {
    min_pane_cols = 80,    -- Minimum pane width in columns
    min_pane_rows = 24,    -- Minimum pane height in rows
    animation_ms = 150,    -- Resize animation duration
  },

  -- File browser
  file_browser = {
    enabled = true,         -- Enable file browser
    show_hidden = false,    -- Show hidden files (dotfiles)
    show_git_status = true, -- Show git status indicators
    icons = true,           -- Show nerdfont icons
  },

  -- Status detection (add custom patterns)
  detection = {
    permission_patterns = {
      "Custom permission prompt",
    },
    running_patterns = {
      "Custom running indicator",
    },
    error_patterns = {
      "Custom error message",
    },
    polling_interval_ms = 100, -- Status check interval in ms
    idle_timeout_ms = 3000,    -- Reset to Idle after 3s of no activity (0 = disabled)
  },
}

return config
```

### Status Colors

The default colors follow the Dracula theme:

| Status | Color | Hex | Description |
|--------|-------|-----|-------------|
| Idle | Red | `#ff5555` | Claude is waiting for input |
| Running | Green | `#50fa7b` | Claude is actively processing |
| Awaiting Permission | Yellow | `#f1fa8c` | User action required |
| Error | Orange | `#ffb86c` | An error occurred |

### Minimal Configuration

For users who want just the essentials:

```lua
local wezterm = require 'wezterm'
return {
  claude_terminal = {
    -- Disable animations if they're distracting
    animation = {
      pulse_idle = false,
      blink_permission = false,
    },
    -- Larger borders for visibility
    border = {
      width = 6,
    },
  },
}
```

## Requirements

- **macOS**: 10.14 Mojave or later (primary platform)
- **Font**: A [Nerd Font](https://www.nerdfonts.com/) is recommended for file browser icons
- **Shell Integration**: For file browser CWD tracking, use a shell with OSC 7 support:
  - **zsh**: Works out of the box with most prompt themes
  - **bash**: Add to `.bashrc`: `PROMPT_COMMAND='printf "\e]7;file://%s%s\e\\" "$HOSTNAME" "$PWD"'`
  - **fish**: Works out of the box

## Architecture

Claude Terminal is built on WezTerm's architecture:

- **Rust + wgpu**: GPU-accelerated rendering at 60fps
- **Status Detection**: Pattern matching on PTY output every 100ms
- **Gaussian Blur**: Two-pass blur for glow effects (horizontal + vertical)
- **Auto-Tiling**: Grid-based layout with minimum size constraints

## Troubleshooting

### Borders Not Showing

1. Ensure you're running Claude Code in the terminal
2. Check that status detection patterns match your Claude Code version
3. Verify GPU acceleration is working: `wezterm show-keys`

### File Browser Not Updating

1. Verify shell integration is enabled (OSC 7 support)
2. Check that the shell is sending CWD updates
3. Try restarting the terminal

### Performance Issues

1. Reduce `glow_radius` for better performance
2. Disable animations with `pulse_idle = false` and `blink_permission = false`
3. Check GPU driver is up to date

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## Credits

- Built on [WezTerm](https://wezterm.org/) by [@wez](https://github.com/wez)
- Status colors inspired by [Dracula Theme](https://draculatheme.com/)

## License

Claude Terminal inherits WezTerm's MIT license. See [LICENSE](LICENSE.md) for details.
