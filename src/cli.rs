use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "phosphor")]
#[command(about = "Retro terminal dashboard with amber CRT aesthetic")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Spotify controls
    Spotify {
        #[command(subcommand)]
        command: SpotifyCommands,
    },
    /// Git repository tracking
    Git {
        #[command(subcommand)]
        command: GitCommands,
    },
    /// Audio device management
    Audio {
        #[command(subcommand)]
        command: AudioCommands,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
pub enum SpotifyCommands {
    /// Show currently playing track
    Now,
    /// Resume playback
    Play,
    /// Pause playback
    Pause,
    /// Skip to next track
    Next,
    /// Go to previous track
    Prev,
    /// Set volume (0-100)
    Vol {
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        level: u8,
    },
}

#[derive(Subcommand)]
pub enum GitCommands {
    /// Show status of all tracked repositories
    Status,
    /// Show recent commits across all repositories
    Log,
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Open config file in $EDITOR
    Edit,
    /// Print config file path
    Path,
}

#[derive(Subcommand)]
pub enum AudioCommands {
    /// List available audio input devices
    Devices,
}
