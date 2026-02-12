use serde::Deserialize;

/// A single line of lyrics with timestamp
#[derive(Debug, Clone)]
pub struct LyricLine {
    pub timestamp_ms: u64,
    pub text: String,
}

/// Parsed synced lyrics for a track
#[derive(Debug, Clone)]
pub struct SyncedLyrics {
    pub lines: Vec<LyricLine>,
}

/// Lyrics fetch status for UI feedback
#[derive(Debug, Clone)]
pub enum LyricsStatus {
    Loading,
    Available(SyncedLyrics),
    NotFound,
    Error(String),
}

#[derive(Debug, Deserialize)]
struct LrcLibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LrcLibSearchResult {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

impl SyncedLyrics {
    /// Parse LRC format: "[mm:ss.xx] text" or "[mm:ss.xxx] text"
    pub fn parse(lrc_text: &str) -> Option<Self> {
        let mut lines = Vec::new();

        for line in lrc_text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some((timestamp_ms, text)) = parse_timestamp_line(line) {
                if !text.is_empty() {
                    lines.push(LyricLine { timestamp_ms, text });
                }
            }
        }

        if lines.is_empty() {
            return None;
        }

        // Ensure sorted order
        lines.sort_by_key(|l| l.timestamp_ms);

        Some(SyncedLyrics { lines })
    }

    /// Find the current line index based on playback position using binary search
    pub fn current_line_index(&self, progress_ms: u64) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }

        // Find the last line whose timestamp <= progress_ms
        match self
            .lines
            .binary_search_by_key(&progress_ms, |l| l.timestamp_ms)
        {
            Ok(idx) => Some(idx),
            Err(idx) => {
                if idx == 0 {
                    None // Before first lyric
                } else {
                    Some(idx - 1)
                }
            }
        }
    }
}

fn parse_timestamp_line(line: &str) -> Option<(u64, String)> {
    // Pattern: [MM:SS.xx] text or [MM:SS.xxx] text
    if !line.starts_with('[') {
        return None;
    }

    let end_bracket = line.find(']')?;
    let timestamp_str = &line[1..end_bracket];
    let text = line[end_bracket + 1..].trim().to_string();

    // Parse MM:SS.xx or MM:SS.xxx
    let parts: Vec<&str> = timestamp_str.split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let minutes: u64 = parts[0].parse().ok()?;
    let sec_parts: Vec<&str> = parts[1].split('.').collect();
    if sec_parts.len() != 2 {
        return None;
    }

    let seconds: u64 = sec_parts[0].parse().ok()?;
    let frac_str = sec_parts[1];
    let fraction: u64 = frac_str.parse().ok()?;

    // Convert to ms (handle both .xx and .xxx formats)
    let frac_ms = if frac_str.len() == 2 {
        fraction * 10 // Centiseconds to ms
    } else if frac_str.len() == 3 {
        fraction // Already ms
    } else {
        0
    };

    let timestamp_ms = (minutes * 60 + seconds) * 1000 + frac_ms;

    Some((timestamp_ms, text))
}

/// Fetch lyrics from LRClib API
pub fn fetch_lyrics(
    track_name: &str,
    artist_name: &str,
    album_name: &str,
    duration_secs: u64,
) -> LyricsStatus {
    // Try exact match first
    let url = format!(
        "https://lrclib.net/api/get?track_name={}&artist_name={}&album_name={}&duration={}",
        urlencoding::encode(track_name),
        urlencoding::encode(artist_name),
        urlencoding::encode(album_name),
        duration_secs,
    );

    match fetch_from_url(&url) {
        LyricsStatus::Available(lyrics) => return LyricsStatus::Available(lyrics),
        LyricsStatus::NotFound => {
            // Fallback to search
            return fetch_lyrics_search(track_name, artist_name);
        }
        status => return status,
    }
}

fn fetch_from_url(url: &str) -> LyricsStatus {
    let response = match ureq::get(url)
        .set("User-Agent", "Phosphor/0.1.0")
        .call()
    {
        Ok(resp) => resp,
        Err(ureq::Error::Status(404, _)) => return LyricsStatus::NotFound,
        Err(e) => return LyricsStatus::Error(e.to_string()),
    };

    let json: LrcLibResponse = match response.into_json() {
        Ok(j) => j,
        Err(e) => return LyricsStatus::Error(e.to_string()),
    };

    match json.synced_lyrics {
        Some(lrc) if !lrc.trim().is_empty() => match SyncedLyrics::parse(&lrc) {
            Some(lyrics) => LyricsStatus::Available(lyrics),
            None => LyricsStatus::NotFound,
        },
        _ => LyricsStatus::NotFound,
    }
}

fn fetch_lyrics_search(track_name: &str, artist_name: &str) -> LyricsStatus {
    let url = format!(
        "https://lrclib.net/api/search?track_name={}&artist_name={}",
        urlencoding::encode(track_name),
        urlencoding::encode(artist_name),
    );

    let response = match ureq::get(&url)
        .set("User-Agent", "Phosphor/0.1.0")
        .call()
    {
        Ok(resp) => resp,
        Err(ureq::Error::Status(404, _)) => return LyricsStatus::NotFound,
        Err(e) => return LyricsStatus::Error(e.to_string()),
    };

    let results: Vec<LrcLibSearchResult> = match response.into_json() {
        Ok(j) => j,
        Err(e) => return LyricsStatus::Error(e.to_string()),
    };

    // Find first result with synced lyrics
    for result in results {
        if let Some(lrc) = result.synced_lyrics {
            if !lrc.trim().is_empty() {
                if let Some(lyrics) = SyncedLyrics::parse(&lrc) {
                    return LyricsStatus::Available(lyrics);
                }
            }
        }
    }

    LyricsStatus::NotFound
}
