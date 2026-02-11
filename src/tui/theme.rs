use ratatui::style::Color;

use crate::config::ThemeConfig;

#[derive(Clone)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub accent: Color,
    pub dim: Color,
}

impl Theme {
    pub fn from_config(config: &ThemeConfig) -> Self {
        Self {
            background: parse_hex_color(&config.background).unwrap_or(Color::Rgb(26, 16, 0)),
            foreground: parse_hex_color(&config.foreground).unwrap_or(Color::Rgb(255, 176, 0)),
            accent: parse_hex_color(&config.accent).unwrap_or(Color::Rgb(255, 204, 0)),
            dim: parse_hex_color(&config.dim).unwrap_or(Color::Rgb(102, 68, 0)),
        }
    }

    pub fn gradient(&self, intensity: f32) -> Color {
        let intensity = intensity.clamp(0.0, 1.0);

        // Interpolate between dim and accent based on intensity
        let (dr, dg, db) = color_to_rgb(self.dim);
        let (ar, ag, ab) = color_to_rgb(self.accent);

        let r = (dr as f32 + (ar as f32 - dr as f32) * intensity) as u8;
        let g = (dg as f32 + (ag as f32 - dg as f32) * intensity) as u8;
        let b = (db as f32 + (ab as f32 - db as f32) * intensity) as u8;

        Color::Rgb(r, g, b)
    }
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

    Some(Color::Rgb(r, g, b))
}

fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (255, 176, 0), // Default amber
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::Rgb(26, 16, 0),
            foreground: Color::Rgb(255, 176, 0),
            accent: Color::Rgb(255, 204, 0),
            dim: Color::Rgb(102, 68, 0),
        }
    }
}
