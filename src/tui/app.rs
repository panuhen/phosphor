use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    style::SetBackgroundColor,
    terminal::{disable_raw_mode, enable_raw_mode, Clear as TermClear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::{Block, Clear},
    Frame, Terminal,
};
use tokio::sync::mpsc;

use crate::config::Config;
use crate::modules::{
    audio::{AudioData, AudioSource, SmoothedAudio},
    git::{CommitInfo, GitTracker, RepoStatus},
    lyrics::{fetch_lyrics, LyricsStatus, SyncedLyrics},
    spotify::{SpotifyClient, TrackInfo},
};
use crate::tui::theme::Theme;
use crate::tui::widgets::{
    album_art::{AlbumArtWidget, ArtStyle, ImageCache},
    git::HelpWidget,
    lyrics::LyricsWidget,
    spotify::SpotifyWidget,
    visualizer::{SpectrumWidget, WaveformWidget},
};
use image::DynamicImage;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Spotify,
    Lyrics,
    Spectrum,
    Waveform,
    AlbumArt,
}

impl Panel {
    fn next(self) -> Self {
        match self {
            Panel::Spotify => Panel::Lyrics,
            Panel::Lyrics => Panel::Spectrum,
            Panel::Spectrum => Panel::Waveform,
            Panel::Waveform => Panel::AlbumArt,
            Panel::AlbumArt => Panel::Spotify,
        }
    }
}

enum SpotifyCommand {
    Refresh,
    TogglePlayback,
    Next,
    Prev,
    SetVolume(u8),
}

struct App {
    config: Config,
    theme: Theme,
    audio: AudioSource,
    audio_smoother: SmoothedAudio,
    git: GitTracker,
    track_info: Option<TrackInfo>,
    audio_data: AudioData,
    repo_statuses: Vec<RepoStatus>,
    commits: Vec<CommitInfo>,
    focused_panel: Panel,
    show_help: bool,
    last_git_update: Instant,
    volume: u8,
    spotify_tx: mpsc::UnboundedSender<SpotifyCommand>,
    spotify_rx: mpsc::UnboundedReceiver<Option<TrackInfo>>,
    // Album art
    image_cache: ImageCache,
    current_album_art: Option<DynamicImage>,
    last_album_art_url: Option<String>,
    art_style: ArtStyle,
    // Lyrics
    lyrics_status: LyricsStatus,
    current_lyrics: Option<SyncedLyrics>,
    last_lyrics_track: Option<(String, String)>,
    show_lyrics: bool,
    last_spotify_poll: Instant,
    last_known_progress_ms: u64,
    was_playing: bool,
}

impl App {
    async fn new(config: Config) -> Result<Self> {
        let theme = Theme::from_config(&config.theme);

        // Initialize audio capture
        let audio = AudioSource::new(&config.audio.device, config.audio.fft_size);

        // Initialize git tracker
        let git = GitTracker::new(&config.git.repos);

        // Set up channels for async Spotify communication
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<SpotifyCommand>();
        let (track_tx, track_rx) = mpsc::unbounded_channel::<Option<TrackInfo>>();

        // Spawn background Spotify task
        let config_clone = config.clone();
        tokio::spawn(async move {
            spotify_background_task(config_clone, cmd_rx, track_tx).await;
        });

        // Request initial track info
        let _ = cmd_tx.send(SpotifyCommand::Refresh);

        // Smoother with fast attack (0.6) and slower decay (0.15) for nice visuals
        let audio_smoother = SmoothedAudio::new(config.audio.fft_size, 0.6, 0.15);

        let mut app = Self {
            theme,
            audio,
            audio_smoother,
            git,
            track_info: None,
            audio_data: AudioData {
                spectrum: vec![0.0; config.audio.fft_size / 2],
                waveform: vec![0.0; config.audio.fft_size],
            },
            repo_statuses: Vec::new(),
            commits: Vec::new(),
            focused_panel: Panel::Spotify,
            show_help: false,
            last_git_update: Instant::now() - Duration::from_secs(10),
            volume: 50,
            config,
            spotify_tx: cmd_tx,
            spotify_rx: track_rx,
            // Album art
            image_cache: ImageCache::new(),
            current_album_art: None,
            last_album_art_url: None,
            art_style: ArtStyle::Braille,
            // Lyrics
            lyrics_status: LyricsStatus::NotFound,
            current_lyrics: None,
            last_lyrics_track: None,
            show_lyrics: true,
            last_spotify_poll: Instant::now(),
            last_known_progress_ms: 0,
            was_playing: false,
        };

        // Initial git fetch
        app.update_git();

        Ok(app)
    }

    fn poll_spotify(&mut self) {
        // Non-blocking receive of track updates from background task
        while let Ok(track_info) = self.spotify_rx.try_recv() {
            // Check if album art URL changed
            let new_url = track_info.as_ref().and_then(|t| t.album_art_url.clone());
            if new_url != self.last_album_art_url {
                self.last_album_art_url = new_url.clone();
                // Fetch new album art
                self.current_album_art = new_url
                    .as_ref()
                    .and_then(|url| self.image_cache.get_or_fetch(url));
            }

            // Track progress for lyrics interpolation
            if let Some(ref track) = track_info {
                self.last_known_progress_ms = track.progress.unwrap_or(0);
                self.was_playing = track.is_playing;
                self.last_spotify_poll = Instant::now();

                // Check if track changed for lyrics
                let track_key = (track.name.clone(), track.artist.clone());
                if self.last_lyrics_track.as_ref() != Some(&track_key) {
                    self.last_lyrics_track = Some(track_key);
                    self.lyrics_status = LyricsStatus::Loading;
                    self.current_lyrics = None;

                    // Fetch lyrics
                    let status = fetch_lyrics(
                        &track.name,
                        &track.artist,
                        &track.album,
                        track.duration / 1000, // Convert ms to seconds
                    );
                    if let LyricsStatus::Available(ref lyrics) = status {
                        self.current_lyrics = Some(lyrics.clone());
                    }
                    self.lyrics_status = status;
                }
            }

            self.track_info = track_info;
        }
    }

    fn current_progress_ms(&self) -> u64 {
        if !self.was_playing {
            return self.last_known_progress_ms;
        }
        let elapsed = self.last_spotify_poll.elapsed().as_millis() as u64;
        self.last_known_progress_ms + elapsed
    }

    fn update_git(&mut self) {
        if self.last_git_update.elapsed() < Duration::from_secs(30) {
            return;
        }
        self.last_git_update = Instant::now();

        self.repo_statuses = self.git.get_status().unwrap_or_default();
        self.commits = self
            .git
            .get_recent_commits(self.config.git.max_commits)
            .unwrap_or_default();
    }

    fn force_update_git(&mut self) {
        self.last_git_update = Instant::now() - Duration::from_secs(60);
        self.update_git();
    }

    fn update_audio(&mut self) {
        let raw_data = self.audio.get_data();
        self.audio_data = self.audio_smoother.update(&raw_data);
    }

    fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.show_help {
                    self.show_help = false;
                } else {
                    return true; // Quit
                }
            }
            KeyCode::Char('?') => {
                self.show_help = !self.show_help;
            }
            KeyCode::Tab => {
                self.focused_panel = self.focused_panel.next();
            }
            KeyCode::Char(' ') => {
                let _ = self.spotify_tx.send(SpotifyCommand::TogglePlayback);
            }
            KeyCode::Char('n') => {
                let _ = self.spotify_tx.send(SpotifyCommand::Next);
            }
            KeyCode::Char('p') => {
                let _ = self.spotify_tx.send(SpotifyCommand::Prev);
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.volume = (self.volume + 5).min(100);
                let _ = self.spotify_tx.send(SpotifyCommand::SetVolume(self.volume));
            }
            KeyCode::Char('-') => {
                self.volume = self.volume.saturating_sub(5);
                let _ = self.spotify_tx.send(SpotifyCommand::SetVolume(self.volume));
            }
            KeyCode::Char('r') => {
                self.force_update_git();
            }
            KeyCode::Char('a') => {
                // Toggle album art style
                self.art_style = match self.art_style {
                    ArtStyle::Blocks => ArtStyle::Braille,
                    ArtStyle::Braille => ArtStyle::Blocks,
                };
            }
            KeyCode::Char('l') => {
                // Toggle lyrics display
                self.show_lyrics = !self.show_lyrics;
            }
            _ => {}
        }
        false
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();

        // Fill entire background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                frame.buffer_mut()[(x, y)]
                    .set_bg(self.theme.background)
                    .set_char(' ');
            }
        }

        // Stacked vertical layout: Spotify, Lyrics/AlbumArt, Spectrum, Waveform
        let rows = if self.show_lyrics {
            Layout::vertical([
                Constraint::Length(9),      // Spotify - fixed height for track info
                Constraint::Min(8),         // Lyrics - takes album art space
                Constraint::Percentage(15), // Spectrum - smaller
                Constraint::Percentage(15), // Waveform - smaller
            ])
            .split(area)
        } else {
            Layout::vertical([
                Constraint::Length(9),      // Spotify - fixed height for track info
                Constraint::Percentage(20), // Spectrum - smaller
                Constraint::Percentage(20), // Waveform - smaller
                Constraint::Min(10),        // Album Art - more space
            ])
            .split(area)
        };

        // Render Spotify widget
        let spotify_widget = SpotifyWidget::new(
            self.track_info.as_ref(),
            &self.theme,
            self.focused_panel == Panel::Spotify,
        );
        frame.render_widget(spotify_widget, rows[0]);

        if self.show_lyrics {
            // Lyrics mode: Lyrics, Spectrum, Waveform
            let lyrics_widget = LyricsWidget::new(
                self.current_lyrics.as_ref(),
                &self.lyrics_status,
                self.current_progress_ms(),
                &self.theme,
                self.focused_panel == Panel::Lyrics,
            );
            frame.render_widget(lyrics_widget, rows[1]);

            let spectrum_widget = SpectrumWidget::new(
                &self.audio_data,
                &self.theme,
                self.focused_panel == Panel::Spectrum,
            );
            frame.render_widget(spectrum_widget, rows[2]);

            let waveform_widget = WaveformWidget::new(
                &self.audio_data,
                &self.theme,
                self.focused_panel == Panel::Waveform,
            );
            frame.render_widget(waveform_widget, rows[3]);
        } else {
            // Album art mode: Spectrum, Waveform, AlbumArt
            let spectrum_widget = SpectrumWidget::new(
                &self.audio_data,
                &self.theme,
                self.focused_panel == Panel::Spectrum,
            );
            frame.render_widget(spectrum_widget, rows[1]);

            let waveform_widget = WaveformWidget::new(
                &self.audio_data,
                &self.theme,
                self.focused_panel == Panel::Waveform,
            );
            frame.render_widget(waveform_widget, rows[2]);

            let album_art_widget = AlbumArtWidget::new(
                self.current_album_art.as_ref(),
                &self.theme,
                self.focused_panel == Panel::AlbumArt,
                self.art_style,
            );
            frame.render_widget(album_art_widget, rows[3]);
        }

        // Render help overlay if active
        if self.show_help {
            let help_area = centered_rect(40, 50, area);
            frame.render_widget(Clear, help_area);
            let help_block = Block::default()
                .style(Style::default().bg(self.theme.background));
            frame.render_widget(help_block, help_area);
            let help_widget = HelpWidget::new(&self.theme);
            frame.render_widget(help_widget, help_area);
        }
    }
}

async fn spotify_background_task(
    config: Config,
    mut cmd_rx: mpsc::UnboundedReceiver<SpotifyCommand>,
    track_tx: mpsc::UnboundedSender<Option<TrackInfo>>,
) {
    // Initialize Spotify client (may fail if not configured)
    let spotify = match SpotifyClient::new(&config).await {
        Ok(client) => client,
        Err(_) => return, // No Spotify, exit task
    };

    let mut last_refresh = Instant::now() - Duration::from_secs(10);
    let refresh_interval = Duration::from_secs(1);

    loop {
        // Process any pending commands (non-blocking)
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                SpotifyCommand::Refresh => {
                    // Force refresh on next iteration
                    last_refresh = Instant::now() - Duration::from_secs(10);
                }
                SpotifyCommand::TogglePlayback => {
                    let _ = spotify.toggle_playback().await;
                    last_refresh = Instant::now() - Duration::from_secs(10);
                }
                SpotifyCommand::Next => {
                    let _ = spotify.next().await;
                    last_refresh = Instant::now() - Duration::from_secs(10);
                }
                SpotifyCommand::Prev => {
                    let _ = spotify.prev().await;
                    last_refresh = Instant::now() - Duration::from_secs(10);
                }
                SpotifyCommand::SetVolume(vol) => {
                    let _ = spotify.set_volume(vol).await;
                }
            }
        }

        // Periodic track info refresh
        if last_refresh.elapsed() >= refresh_interval {
            last_refresh = Instant::now();
            let track_info = spotify.get_current_track().await.ok().flatten();
            if track_tx.send(track_info).is_err() {
                break; // Main app closed
            }
        }

        // Small sleep to avoid busy-spinning
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}

pub async fn run() -> Result<()> {
    let config = Config::load()?;
    let fps = config.audio.fps;

    // Parse background color for terminal clear
    let bg_color = parse_hex_to_crossterm(&config.theme.background)
        .unwrap_or(crossterm::style::Color::Rgb { r: 26, g: 16, b: 0 });

    // Setup terminal with background color
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        SetBackgroundColor(bg_color),
        TermClear(ClearType::All),
        EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new(config).await?;

    let tick_rate = Duration::from_millis(1000 / fps as u64);
    let mut last_tick = Instant::now();

    loop {
        // Draw
        terminal.draw(|f| app.draw(f))?;

        // Handle events
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.handle_key(key.code) {
                        break;
                    }
                }
            }
        }

        // Update on tick
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
            app.update_audio();
            app.poll_spotify(); // Non-blocking check for track updates
            app.update_git();
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn parse_hex_to_crossterm(hex: &str) -> Option<crossterm::style::Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(crossterm::style::Color::Rgb { r, g, b })
}
