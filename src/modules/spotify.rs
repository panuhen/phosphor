use anyhow::{Context, Result};
use rspotify::{
    model::{AdditionalType, PlayableItem},
    prelude::*,
    scopes, AuthCodePkceSpotify, Credentials, OAuth,
};
use std::path::PathBuf;

use crate::config::Config;

const DEFAULT_CLIENT_ID: &str = "1f14edc73f6548dc97f7791dfec833aa";

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TrackInfo {
    pub name: String,
    pub artist: String,
    pub album: String,
    pub duration: u64,
    pub progress: Option<u64>,
    pub is_playing: bool,
    pub album_art_url: Option<String>,
}

pub struct SpotifyClient {
    client: AuthCodePkceSpotify,
}

impl SpotifyClient {
    pub async fn new(config: &Config) -> Result<Self> {
        // Use bundled client ID (PKCE doesn't need secret), allow override via env/config
        let client_id = std::env::var("SPOTIPY_CLIENT_ID")
            .or_else(|_| std::env::var("RSPOTIFY_CLIENT_ID"))
            .unwrap_or_else(|_| {
                if !config.spotify.client_id.is_empty() {
                    config.spotify.client_id.clone()
                } else {
                    DEFAULT_CLIENT_ID.to_string()
                }
            });

        let creds = Credentials::new_pkce(&client_id);

        let redirect_uri = std::env::var("SPOTIPY_REDIRECT_URI")
            .or_else(|_| std::env::var("RSPOTIFY_REDIRECT_URI"))
            .unwrap_or_else(|_| "http://127.0.0.1:8888/callback".to_string());

        let oauth = OAuth {
            redirect_uri,
            scopes: scopes!(
                "user-read-playback-state",
                "user-modify-playback-state",
                "user-read-currently-playing"
            ),
            ..Default::default()
        };

        let config_rspotify = rspotify::Config {
            cache_path: Self::cache_path(),
            token_cached: true,
            token_refreshing: true,
            ..Default::default()
        };

        let mut client = AuthCodePkceSpotify::with_config(creds, oauth, config_rspotify);

        // Try to read cached token, or prompt for auth
        let url = client.get_authorize_url(None)?;
        client.prompt_for_token(&url).await?;

        Ok(Self { client })
    }

    fn cache_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".phosphor-spotify-token")
    }

    pub async fn get_current_track(&self) -> Result<Option<TrackInfo>> {
        // Handle parse errors gracefully (ads, unsupported content types, etc.)
        let context = match self
            .client
            .current_playing(None, Some([&AdditionalType::Track]))
            .await
        {
            Ok(ctx) => ctx,
            Err(_) => return Ok(None), // Likely an ad or unsupported content
        };

        let Some(context) = context else {
            return Ok(None);
        };

        let Some(item) = context.item else {
            return Ok(None);
        };

        let track_info = match item {
            PlayableItem::Track(track) => {
                let artist = track
                    .artists
                    .iter()
                    .map(|a| a.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ");

                let album_art_url = track.album.images.first().map(|i| i.url.clone());

                TrackInfo {
                    name: track.name,
                    artist,
                    album: track.album.name,
                    duration: track.duration.num_milliseconds() as u64,
                    progress: context.progress.map(|d| d.num_milliseconds() as u64),
                    is_playing: context.is_playing,
                    album_art_url,
                }
            }
            PlayableItem::Episode(episode) => TrackInfo {
                name: episode.name,
                artist: episode.show.name,
                album: "Podcast".to_string(),
                duration: episode.duration.num_milliseconds() as u64,
                progress: context.progress.map(|d| d.num_milliseconds() as u64),
                is_playing: context.is_playing,
                album_art_url: episode.images.first().map(|i| i.url.clone()),
            },
        };

        Ok(Some(track_info))
    }

    pub async fn play(&self) -> Result<()> {
        self.client
            .resume_playback(None, None)
            .await
            .context("Failed to resume playback")?;
        Ok(())
    }

    pub async fn pause(&self) -> Result<()> {
        self.client
            .pause_playback(None)
            .await
            .context("Failed to pause playback")?;
        Ok(())
    }

    pub async fn next(&self) -> Result<()> {
        self.client
            .next_track(None)
            .await
            .context("Failed to skip to next track")?;
        Ok(())
    }

    pub async fn prev(&self) -> Result<()> {
        self.client
            .previous_track(None)
            .await
            .context("Failed to go to previous track")?;
        Ok(())
    }

    pub async fn set_volume(&self, volume: u8) -> Result<()> {
        self.client
            .volume(volume, None)
            .await
            .context("Failed to set volume")?;
        Ok(())
    }

    pub async fn toggle_playback(&self) -> Result<()> {
        if let Some(track) = self.get_current_track().await? {
            if track.is_playing {
                self.pause().await?;
            } else {
                self.play().await?;
            }
        }
        Ok(())
    }
}
