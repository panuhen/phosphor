use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::modules::spotify::TrackInfo;
use crate::tui::theme::Theme;

pub struct SpotifyWidget<'a> {
    track: Option<&'a TrackInfo>,
    theme: &'a Theme,
    focused: bool,
}

impl<'a> SpotifyWidget<'a> {
    pub fn new(track: Option<&'a TrackInfo>, theme: &'a Theme, focused: bool) -> Self {
        Self { track, theme, focused }
    }
}

impl Widget for SpotifyWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(self.theme.accent)
        } else {
            Style::default().fg(self.theme.dim)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" ♫ Now Playing ")
            .title_style(Style::default().fg(self.theme.foreground));

        let inner = block.inner(area);
        block.render(area, buf);

        match self.track {
            Some(track) => self.render_track(track, inner, buf),
            None => self.render_empty(inner, buf),
        }
    }
}

impl SpotifyWidget<'_> {
    fn render_track(&self, track: &TrackInfo, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::vertical([
            Constraint::Length(1), // Track name
            Constraint::Length(1), // Artist
            Constraint::Length(1), // Album
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Progress bar
            Constraint::Length(1), // Controls hint
        ])
        .split(area);

        // Track name
        let status_icon = if track.is_playing { "▶" } else { "⏸" };
        let track_line = Line::from(vec![
            Span::styled(
                format!("{} ", status_icon),
                Style::default().fg(self.theme.accent),
            ),
            Span::styled(
                &track.name,
                Style::default()
                    .fg(self.theme.foreground)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        Paragraph::new(track_line).render(chunks[0], buf);

        // Artist
        let artist_line = Line::from(vec![
            Span::styled("  ", Style::default().fg(self.theme.dim)),
            Span::styled(&track.artist, Style::default().fg(self.theme.foreground)),
        ]);
        Paragraph::new(artist_line).render(chunks[1], buf);

        // Album
        let album_line = Line::from(vec![
            Span::styled("  ", Style::default().fg(self.theme.dim)),
            Span::styled(&track.album, Style::default().fg(self.theme.dim)),
        ]);
        Paragraph::new(album_line).render(chunks[2], buf);

        // Progress bar
        if let Some(progress) = track.progress {
            self.render_progress(progress, track.duration, chunks[4], buf);
        }

        // Controls hint
        let controls = Line::from(vec![
            Span::styled(
                "  ⏮ p  ⏸ space  ⏭ n  🔊 +/-",
                Style::default().fg(self.theme.dim),
            ),
        ]);
        Paragraph::new(controls).render(chunks[5], buf);
    }

    fn render_progress(&self, progress: u64, duration: u64, area: Rect, buf: &mut Buffer) {
        let width = area.width.saturating_sub(16) as usize;
        let pct = if duration > 0 {
            (progress as f64 / duration as f64).min(1.0)
        } else {
            0.0
        };
        let filled = (pct * width as f64) as usize;
        let empty = width.saturating_sub(filled);

        let progress_str = format!(
            "{:02}:{:02}",
            progress / 60000,
            (progress / 1000) % 60
        );
        let duration_str = format!(
            "{:02}:{:02}",
            duration / 60000,
            (duration / 1000) % 60
        );

        let bar = Line::from(vec![
            Span::styled(format!("  {} ", progress_str), Style::default().fg(self.theme.dim)),
            Span::styled("█".repeat(filled), Style::default().fg(self.theme.accent)),
            Span::styled("░".repeat(empty), Style::default().fg(self.theme.dim)),
            Span::styled(format!(" {}", duration_str), Style::default().fg(self.theme.dim)),
        ]);
        Paragraph::new(bar).render(area, buf);
    }

    fn render_empty(&self, area: Rect, buf: &mut Buffer) {
        let text = Paragraph::new("Nothing playing")
            .style(Style::default().fg(self.theme.dim))
            .alignment(Alignment::Center);
        text.render(area, buf);
    }
}
