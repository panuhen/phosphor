use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::modules::lyrics::{LyricsStatus, SyncedLyrics};
use crate::tui::theme::Theme;

pub struct LyricsWidget<'a> {
    lyrics: Option<&'a SyncedLyrics>,
    status: &'a LyricsStatus,
    progress_ms: u64,
    theme: &'a Theme,
    focused: bool,
}

impl<'a> LyricsWidget<'a> {
    pub fn new(
        lyrics: Option<&'a SyncedLyrics>,
        status: &'a LyricsStatus,
        progress_ms: u64,
        theme: &'a Theme,
        focused: bool,
    ) -> Self {
        Self {
            lyrics,
            status,
            progress_ms,
            theme,
            focused,
        }
    }
}

impl Widget for LyricsWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(self.theme.accent)
        } else {
            Style::default().fg(self.theme.dim)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" ♪ Lyrics ")
            .title_style(Style::default().fg(self.theme.foreground));

        let inner = block.inner(area);
        block.render(area, buf);

        match self.status {
            LyricsStatus::Loading => {
                self.render_centered("Loading lyrics...", inner, buf);
            }
            LyricsStatus::NotFound => {
                self.render_centered("No lyrics available", inner, buf);
            }
            LyricsStatus::Error(msg) => {
                let text = format!("Error: {}", truncate(msg, 40));
                self.render_centered(&text, inner, buf);
            }
            LyricsStatus::Available(_) => {
                if let Some(lyrics) = self.lyrics {
                    self.render_lyrics(lyrics, inner, buf);
                }
            }
        }
    }
}

impl LyricsWidget<'_> {
    fn render_centered(&self, text: &str, area: Rect, buf: &mut Buffer) {
        let paragraph = Paragraph::new(text)
            .style(Style::default().fg(self.theme.dim))
            .alignment(Alignment::Center);

        // Center vertically
        let y_offset = area.height / 2;
        if y_offset < area.height {
            let centered_area = Rect::new(area.x, area.y + y_offset, area.width, 1);
            paragraph.render(centered_area, buf);
        }
    }

    fn render_lyrics(&self, lyrics: &SyncedLyrics, area: Rect, buf: &mut Buffer) {
        let height = area.height as usize;
        if height == 0 || lyrics.lines.is_empty() {
            return;
        }

        let current_idx = lyrics.current_line_index(self.progress_ms);
        let center_offset = height / 2;

        // Calculate start index to center current line
        let start_idx = current_idx
            .map(|idx| idx.saturating_sub(center_offset))
            .unwrap_or(0);

        for (row, line_idx) in (start_idx..).take(height).enumerate() {
            if line_idx >= lyrics.lines.len() {
                break;
            }

            let line = &lyrics.lines[line_idx];
            let y = area.y + row as u16;

            // Determine style based on position relative to current
            let style = match current_idx {
                Some(curr) if line_idx == curr => {
                    // Current line: bright accent, bold
                    Style::default()
                        .fg(self.theme.accent)
                        .add_modifier(Modifier::BOLD)
                }
                Some(curr) if line_idx < curr => {
                    // Past line: dim
                    Style::default().fg(self.theme.dim)
                }
                _ => {
                    // Future line or no current: normal foreground
                    Style::default().fg(self.theme.foreground)
                }
            };

            // Truncate if needed
            let text = truncate(&line.text, area.width as usize);
            let line_widget = Line::from(text);

            let paragraph = Paragraph::new(line_widget)
                .style(style)
                .alignment(Alignment::Center);

            paragraph.render(Rect::new(area.x, y, area.width, 1), buf);
        }
    }
}

fn truncate(text: &str, max_width: usize) -> String {
    if text.chars().count() <= max_width {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_width.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}
