use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Widget},
};

use crate::modules::audio::AudioData;
use crate::tui::theme::Theme;

const BAR_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub struct SpectrumWidget<'a> {
    data: &'a AudioData,
    theme: &'a Theme,
    focused: bool,
}

impl<'a> SpectrumWidget<'a> {
    pub fn new(data: &'a AudioData, theme: &'a Theme, focused: bool) -> Self {
        Self { data, theme, focused }
    }
}

impl Widget for SpectrumWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(self.theme.accent)
        } else {
            Style::default().fg(self.theme.dim)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title("  Spectrum ")
            .title_style(Style::default().fg(self.theme.foreground));

        let inner = block.inner(area);
        block.render(area, buf);

        self.render_spectrum(inner, buf);
    }
}

impl SpectrumWidget<'_> {
    fn render_spectrum(&self, area: Rect, buf: &mut Buffer) {
        let width = area.width as usize;
        let height = area.height as usize;

        if width == 0 || height == 0 || self.data.spectrum.is_empty() {
            return;
        }

        // Focus on lower frequencies (more musical content there)
        let useful_bins = self.data.spectrum.len().min(width * 2);
        let bins_per_bar = (useful_bins / width).max(1);

        // Find max for normalization
        let max_val = self.data.spectrum[..useful_bins]
            .iter()
            .cloned()
            .fold(0.0f32, f32::max)
            .max(0.0001); // Avoid division by zero

        for x in 0..width {
            let start = x * bins_per_bar;
            let end = ((x + 1) * bins_per_bar).min(self.data.spectrum.len());

            if start >= self.data.spectrum.len() {
                break;
            }

            // Average the bins for this bar
            let avg: f32 = self.data.spectrum[start..end].iter().sum::<f32>()
                / (end - start) as f32;

            // Normalize to max and apply some boost for visibility
            let normalized = (avg / max_val).sqrt(); // sqrt gives nicer curve
            let bar_height = (normalized * height as f32).min(height as f32) as usize;

            // Draw the bar from bottom up
            for y in 0..height {
                let cell_y = area.y + (height - 1 - y) as u16;
                let cell_x = area.x + x as u16;

                if y < bar_height {
                    let intensity = y as f32 / height as f32;
                    let color = self.theme.gradient(intensity);
                    buf[(cell_x, cell_y)]
                        .set_char('█')
                        .set_fg(color);
                } else if y == bar_height && bar_height > 0 {
                    // Partial block at top
                    let frac = (normalized * height as f32) - bar_height as f32 + 1.0;
                    let char_idx = ((frac * 8.0) as usize).min(7);
                    let intensity = y as f32 / height as f32;
                    let color = self.theme.gradient(intensity);
                    buf[(cell_x, cell_y)]
                        .set_char(BAR_CHARS[char_idx])
                        .set_fg(color);
                }
            }
        }
    }
}

pub struct WaveformWidget<'a> {
    data: &'a AudioData,
    theme: &'a Theme,
    focused: bool,
}

impl<'a> WaveformWidget<'a> {
    pub fn new(data: &'a AudioData, theme: &'a Theme, focused: bool) -> Self {
        Self { data, theme, focused }
    }
}

impl Widget for WaveformWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(self.theme.accent)
        } else {
            Style::default().fg(self.theme.dim)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title("  Waveform ")
            .title_style(Style::default().fg(self.theme.foreground));

        let inner = block.inner(area);
        block.render(area, buf);

        self.render_waveform(inner, buf);
    }
}

impl WaveformWidget<'_> {
    fn render_waveform(&self, area: Rect, buf: &mut Buffer) {
        let width = area.width as usize;
        let height = area.height as usize;

        if width == 0 || height == 0 || self.data.waveform.is_empty() {
            return;
        }

        let samples_per_point = self.data.waveform.len() / width;
        let mid_y = height / 2;

        for x in 0..width {
            let start = x * samples_per_point;
            let end = ((x + 1) * samples_per_point).min(self.data.waveform.len());

            if start >= self.data.waveform.len() {
                break;
            }

            // Get min and max in this slice for better visualization
            let slice = &self.data.waveform[start..end];
            let min_val = slice.iter().cloned().fold(f32::INFINITY, f32::min);
            let max_val = slice.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

            // Convert to screen coordinates
            let y_min = ((1.0 - max_val) * 0.5 * height as f32) as usize;
            let y_max = ((1.0 - min_val) * 0.5 * height as f32) as usize;

            let y_min = y_min.min(height - 1);
            let y_max = y_max.min(height - 1);

            // Draw vertical line from min to max
            for y in y_min..=y_max {
                let cell_x = area.x + x as u16;
                let cell_y = area.y + y as u16;

                let distance_from_center = ((y as i32 - mid_y as i32).abs() as f32) / (height as f32 / 2.0);
                let intensity = 1.0 - distance_from_center * 0.5;
                let color = self.theme.gradient(intensity);

                buf[(cell_x, cell_y)]
                    .set_char('│')
                    .set_fg(color);
            }
        }

        // Draw center line
        for x in 0..width {
            let cell_x = area.x + x as u16;
            let cell_y = area.y + mid_y as u16;

            if buf[(cell_x, cell_y)].symbol() == " " {
                buf[(cell_x, cell_y)]
                    .set_char('─')
                    .set_fg(self.theme.dim);
            }
        }
    }
}
