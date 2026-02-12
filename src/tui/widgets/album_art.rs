use image::{DynamicImage, GenericImageView, imageops::FilterType};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Widget},
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::tui::theme::Theme;

// Block characters by density (darkest to brightest)
const BLOCK_CHARS: [char; 5] = [' ', '░', '▒', '▓', '█'];

// Braille base and dot positions for 2x4 pixel blocks
// Dots are numbered:
// 0 3
// 1 4
// 2 5
// 6 7
const BRAILLE_BASE: u32 = 0x2800;
const BRAILLE_DOTS: [u32; 8] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80];

/// Simple image cache to avoid re-downloading
pub struct ImageCache {
    cache: Arc<Mutex<HashMap<String, DynamicImage>>>,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_or_fetch(&self, url: &str) -> Option<DynamicImage> {
        let mut cache = self.cache.lock().ok()?;

        if let Some(img) = cache.get(url) {
            return Some(img.clone());
        }

        // Fetch the image (blocking, but should be called sparingly)
        let response = ureq::get(url).call().ok()?;
        let mut bytes = Vec::new();
        response.into_reader().read_to_end(&mut bytes).ok()?;

        let img = image::load_from_memory(&bytes).ok()?;
        cache.insert(url.to_string(), img.clone());
        Some(img)
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ArtStyle {
    Blocks,
    Braille,
    // Future: Edges, Ascii, etc.
}

pub struct AlbumArtWidget<'a> {
    image: Option<&'a DynamicImage>,
    theme: &'a Theme,
    focused: bool,
    style: ArtStyle,
}

impl<'a> AlbumArtWidget<'a> {
    pub fn new(image: Option<&'a DynamicImage>, theme: &'a Theme, focused: bool, style: ArtStyle) -> Self {
        Self { image, theme, focused, style }
    }

    fn render_blocks(&self, img: &DynamicImage, area: Rect, buf: &mut Buffer) {
        let width = area.width as u32;
        let height = area.height as u32;

        if width == 0 || height == 0 {
            return;
        }

        // Maintain 1:1 aspect ratio (terminal chars are ~2:1, so width = height*2 in chars)
        let square_size = width.min(height * 2);
        let img_width = square_size;
        let img_height = square_size / 2;

        // Center the image
        let x_offset = (width - img_width) / 2;
        let y_offset = (height - img_height) / 2;

        // Resize image to square dimensions
        let img = img.resize_exact(img_width, img_height * 2, FilterType::Triangle);
        let gray = img.to_luma8();

        for y in 0..img_height {
            for x in 0..img_width {
                // Average two vertical pixels for each character cell
                let p1 = gray.get_pixel(x, y * 2)[0] as u32;
                let p2 = gray.get_pixel(x, (y * 2 + 1).min(img_height * 2 - 1))[0] as u32;
                let brightness = (p1 + p2) / 2;

                // Map brightness to block character and color
                let char_idx = (brightness * 4 / 255) as usize;
                let ch = BLOCK_CHARS[char_idx.min(4)];

                // Map brightness to amber gradient
                let intensity = brightness as f32 / 255.0;
                let color = self.theme.gradient(intensity);

                let cell_x = area.x + x_offset as u16 + x as u16;
                let cell_y = area.y + y_offset as u16 + y as u16;

                if cell_x < area.x + area.width && cell_y < area.y + area.height {
                    buf[(cell_x, cell_y)]
                        .set_char(ch)
                        .set_fg(color)
                        .set_bg(self.theme.background);
                }
            }
        }
    }

    fn render_braille(&self, img: &DynamicImage, area: Rect, buf: &mut Buffer) {
        let width = area.width as u32;
        let height = area.height as u32;

        if width == 0 || height == 0 {
            return;
        }

        // Maintain 1:1 visual aspect ratio
        // Terminal chars are ~2:1 (height:width), so for square output: char_width = char_height * 2
        let char_height = height.min(width / 2);
        let char_width = char_height * 2;

        // Center the image
        let x_offset = (width - char_width) / 2;
        let y_offset = (height - char_height) / 2;

        // Each braille character is 2x4 pixels
        let img_width = char_width * 2;
        let img_height = char_height * 4;

        let img = img.resize_exact(img_width, img_height, FilterType::Triangle);
        let gray = img.to_luma8();

        // Threshold for "on" pixels (adjust for desired look)
        let threshold = 100u8;

        for cy in 0..char_height {
            for cx in 0..char_width {
                let mut braille = BRAILLE_BASE;
                let mut total_brightness = 0u32;
                let mut count = 0u32;

                // Sample the 2x4 pixel block
                for dy in 0..4 {
                    for dx in 0..2 {
                        let px = cx * 2 + dx;
                        let py = cy * 4 + dy;

                        if px < img_width && py < img_height {
                            let pixel = gray.get_pixel(px, py)[0];
                            total_brightness += pixel as u32;
                            count += 1;

                            if pixel > threshold {
                                // Map (dx, dy) to braille dot index
                                let dot_idx = dy * 2 + dx;
                                let dot_idx = match dot_idx {
                                    0 => 0, 1 => 3,
                                    2 => 1, 3 => 4,
                                    4 => 2, 5 => 5,
                                    6 => 6, 7 => 7,
                                    _ => 0,
                                };
                                braille |= BRAILLE_DOTS[dot_idx as usize];
                            }
                        }
                    }
                }

                let ch = char::from_u32(braille).unwrap_or(' ');

                // Color based on average brightness of the block
                let avg_brightness = if count > 0 { total_brightness / count } else { 0 };
                let intensity = avg_brightness as f32 / 255.0;
                let color = self.theme.gradient(intensity);

                let cell_x = area.x + x_offset as u16 + cx as u16;
                let cell_y = area.y + y_offset as u16 + cy as u16;

                if cell_x < area.x + area.width && cell_y < area.y + area.height {
                    buf[(cell_x, cell_y)]
                        .set_char(ch)
                        .set_fg(color)
                        .set_bg(self.theme.background);
                }
            }
        }
    }
}

impl Widget for AlbumArtWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(self.theme.accent)
        } else {
            Style::default().fg(self.theme.dim)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Album Art ")
            .title_style(Style::default().fg(self.theme.foreground));

        let inner = block.inner(area);
        block.render(area, buf);

        match self.image {
            Some(img) => {
                match self.style {
                    ArtStyle::Blocks => self.render_blocks(img, inner, buf),
                    ArtStyle::Braille => self.render_braille(img, inner, buf),
                }
            }
            None => {
                // Show placeholder text
                let msg = "No album art";
                let x = inner.x + (inner.width.saturating_sub(msg.len() as u16)) / 2;
                let y = inner.y + inner.height / 2;
                if y < inner.y + inner.height {
                    for (i, ch) in msg.chars().enumerate() {
                        let px = x + i as u16;
                        if px < inner.x + inner.width {
                            buf[(px, y)]
                                .set_char(ch)
                                .set_fg(self.theme.dim);
                        }
                    }
                }
            }
        }
    }
}
