use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub layout: LayoutConfig,
    #[serde(default)]
    pub spotify: SpotifyConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub git: GitConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    #[serde(default = "default_background")]
    pub background: String,
    #[serde(default = "default_foreground")]
    pub foreground: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default = "default_dim")]
    pub dim: String,
}

fn default_background() -> String {
    "#1a1000".to_string()
}
fn default_foreground() -> String {
    "#ffb000".to_string()
}
fn default_accent() -> String {
    "#ffcc00".to_string()
}
fn default_dim() -> String {
    "#664400".to_string()
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            background: default_background(),
            foreground: default_foreground(),
            accent: default_accent(),
            dim: default_dim(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    #[serde(default = "default_rows")]
    pub rows: Vec<Vec<String>>,
}

fn default_rows() -> Vec<Vec<String>> {
    vec![
        vec!["spotify".to_string(), "spectrum".to_string()],
        vec!["git".to_string(), "waveform".to_string()],
    ]
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            rows: default_rows(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyConfig {
    #[serde(default)]
    pub client_id: String,
}

impl Default for SpotifyConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    #[serde(default)]
    pub device: String,
    #[serde(default = "default_fft_size")]
    pub fft_size: usize,
    #[serde(default = "default_fps")]
    pub fps: u32,
}

fn default_fft_size() -> usize {
    2048
}
fn default_fps() -> u32 {
    30
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            device: String::new(),
            fft_size: default_fft_size(),
            fps: default_fps(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    #[serde(default)]
    pub repos: Vec<String>,
    #[serde(default = "default_max_commits")]
    pub max_commits: usize,
}

fn default_max_commits() -> usize {
    10
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            repos: Vec::new(),
            max_commits: default_max_commits(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("phosphor")
            .join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::path();

        if !path.exists() {
            let config = Config::default();
            config.save()?;
            return Ok(config);
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeConfig::default(),
            layout: LayoutConfig::default(),
            spotify: SpotifyConfig::default(),
            audio: AudioConfig::default(),
            git: GitConfig::default(),
        }
    }
}
