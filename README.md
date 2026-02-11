# phosphor

A retro terminal dashboard with an amber CRT aesthetic. Combines a TUI dashboard with CLI subcommands for Spotify control, audio visualization, and git repository tracking.

![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- **Spotify Panel** - Now playing display with track, artist, album, and progress bar
- **Spectrum Analyzer** - Real-time FFT frequency visualization
- **Waveform Display** - Oscilloscope-style audio waveform
- **Git Tracker** - Monitor multiple repositories with branch status and recent commits
- **Amber CRT Theme** - Configurable retro color scheme (#ffb000 on #1a1000)

## Installation

### From source

```bash
git clone https://github.com/panuhen/phosphor.git
cd phosphor
cargo build --release
```

The binary will be at `./target/release/phosphor`.

### Dependencies

- **Linux**: For real audio capture, install ALSA dev libraries:
  ```bash
  sudo apt install libasound2-dev  # Debian/Ubuntu
  sudo dnf install alsa-lib-devel  # Fedora
  ```
  Then build with: `cargo build --release --features audio`

  Without these, phosphor uses a mock visualizer with animated waveforms.

## Usage

### TUI Dashboard

```bash
phosphor
```

### CLI Commands

```bash
# Spotify
phosphor spotify now          # Show currently playing track
phosphor spotify play         # Resume playback
phosphor spotify pause        # Pause playback
phosphor spotify next         # Skip to next track
phosphor spotify prev         # Previous track
phosphor spotify vol 80       # Set volume (0-100)

# Git
phosphor git status           # Show status of tracked repos
phosphor git log              # Recent commits across repos

# Config
phosphor config edit          # Open config in $EDITOR
phosphor config path          # Print config file path
```

## Key Bindings

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit |
| `Space` | Play/Pause |
| `n` | Next track |
| `p` | Previous track |
| `+` / `-` | Volume up/down |
| `Tab` | Cycle panel focus |
| `r` | Refresh git status |
| `?` | Show help |

## Configuration

Config file location: `~/.config/phosphor/config.toml`

```toml
[theme]
background = "#1a1000"
foreground = "#ffb000"
accent = "#ffcc00"
dim = "#664400"

[layout]
rows = [
    ["spotify", "spectrum"],
    ["git", "waveform"]
]

[spotify]
# Get credentials at https://developer.spotify.com/dashboard
# Or set RSPOTIFY_CLIENT_ID and RSPOTIFY_CLIENT_SECRET env vars
client_id = "your_client_id"

[audio]
device = ""        # Empty = default device
fft_size = 2048
fps = 30

[git]
repos = [
    "~/Projects/project1",
    "~/Projects/project2",
]
max_commits = 10
```

## Spotify Setup

1. Create an app at [Spotify Developer Dashboard](https://developer.spotify.com/dashboard)
2. Set redirect URI to `http://localhost:8888/callback`
3. Add your `client_id` to config or set environment variables:
   ```bash
   export RSPOTIFY_CLIENT_ID="your_client_id"
   export RSPOTIFY_CLIENT_SECRET="your_client_secret"
   ```
4. On first run, phosphor will open a browser for OAuth authorization

## License

MIT
