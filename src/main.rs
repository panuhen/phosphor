mod cli;
mod config;
mod modules;
mod tui;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, GitCommands, SpotifyCommands, ConfigCommands, AudioCommands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Spotify { command }) => handle_spotify(command).await?,
        Some(Commands::Git { command }) => handle_git(command).await?,
        Some(Commands::Audio { command }) => handle_audio(command)?,
        Some(Commands::Config { command }) => handle_config(command)?,
        None => tui::run().await?,
    }

    Ok(())
}

async fn handle_spotify(command: SpotifyCommands) -> Result<()> {
    let config = config::Config::load()?;
    let spotify = modules::spotify::SpotifyClient::new(&config).await?;

    match command {
        SpotifyCommands::Now => {
            if let Some(track) = spotify.get_current_track().await? {
                println!("♫ {} - {}", track.name, track.artist);
                println!("  Album: {}", track.album);
                if let Some(progress) = track.progress {
                    let duration = track.duration;
                    let pct = (progress as f64 / duration as f64 * 100.0) as u32;
                    let bar_width = 30;
                    let filled = (pct as usize * bar_width / 100).min(bar_width);
                    let empty = bar_width - filled;
                    println!(
                        "  [{}{}] {:02}:{:02} / {:02}:{:02}",
                        "█".repeat(filled),
                        "░".repeat(empty),
                        progress / 60000,
                        (progress / 1000) % 60,
                        duration / 60000,
                        (duration / 1000) % 60
                    );
                }
            } else {
                println!("Nothing playing");
            }
        }
        SpotifyCommands::Play => {
            spotify.play().await?;
            println!("▶ Playing");
        }
        SpotifyCommands::Pause => {
            spotify.pause().await?;
            println!("⏸ Paused");
        }
        SpotifyCommands::Next => {
            spotify.next().await?;
            println!("⏭ Next track");
        }
        SpotifyCommands::Prev => {
            spotify.prev().await?;
            println!("⏮ Previous track");
        }
        SpotifyCommands::Vol { level } => {
            spotify.set_volume(level).await?;
            println!("🔊 Volume: {}%", level);
        }
    }

    Ok(())
}

async fn handle_git(command: GitCommands) -> Result<()> {
    let config = config::Config::load()?;
    let git = modules::git::GitTracker::new(&config.git.repos);

    match command {
        GitCommands::Status => {
            let repos = git.get_status()?;
            for repo in repos {
                let branch_icon = if repo.is_clean { "" } else { "" };
                let sync_status = match (repo.ahead, repo.behind) {
                    (0, 0) => String::new(),
                    (a, 0) => format!(" ↑{}", a),
                    (0, b) => format!(" ↓{}", b),
                    (a, b) => format!(" ↑{} ↓{}", a, b),
                };
                println!(
                    "{} {} {} {}{}",
                    branch_icon,
                    repo.name,
                    repo.branch,
                    if repo.is_clean { "✓" } else { "●" },
                    sync_status
                );
            }
        }
        GitCommands::Log => {
            let commits = git.get_recent_commits(config.git.max_commits)?;
            for commit in commits {
                println!(
                    " {} {} - {} ({})",
                    &commit.hash[..7],
                    commit.message,
                    commit.author,
                    commit.repo_name
                );
            }
        }
    }

    Ok(())
}

fn handle_config(command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Edit => {
            let path = config::Config::path();
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
            std::process::Command::new(&editor)
                .arg(&path)
                .status()?;
        }
        ConfigCommands::Path => {
            println!("{}", config::Config::path().display());
        }
    }

    Ok(())
}

#[cfg(feature = "audio")]
fn handle_audio(command: AudioCommands) -> Result<()> {
    use cpal::traits::{DeviceTrait, HostTrait};

    match command {
        AudioCommands::Devices => {
            let host = cpal::default_host();

            // Get default monitor source name
            let default_monitor = std::process::Command::new("pactl")
                .args(["get-default-sink"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| format!("{}.monitor", String::from_utf8_lossy(&o.stdout).trim()));

            println!("Audio input devices (cpal):");
            println!("─────────────────────────────");

            if let Ok(devices) = host.input_devices() {
                for device in devices {
                    let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
                    let is_default = default_monitor.as_ref().map_or(false, |m| name.contains(m));
                    let marker = if is_default { " ← default monitor" } else { "" };
                    println!("  {}{}", name, marker);
                }
            }

            println!();
            println!("PulseAudio/PipeWire sources (pactl):");
            println!("─────────────────────────────────────");
            let _ = std::process::Command::new("pactl")
                .args(["list", "short", "sources"])
                .status();
        }
    }

    Ok(())
}

#[cfg(not(feature = "audio"))]
fn handle_audio(_command: AudioCommands) -> Result<()> {
    println!("Audio feature not enabled. Rebuild with: cargo build --features audio");
    Ok(())
}
