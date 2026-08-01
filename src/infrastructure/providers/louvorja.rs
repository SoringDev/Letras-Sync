use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::domain::lyrics::LyricsLine;

#[derive(Debug, Deserialize)]
struct SearchResponse {
    data: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    id_music: IdMusic,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum IdMusic {
    String(String),
    Number(i64),
    Unsigned(u64),
}

#[derive(Debug, Deserialize)]
struct MusicResponse {
    lyrics: Option<Vec<LyricItem>>,
}

#[derive(Debug, Deserialize)]
struct LyricItem {
    lyric: Option<String>,
    time: String,
}

pub struct LouvorJaProvider;

impl LouvorJaProvider {
    pub fn new() -> Self {
        Self
    }

    pub async fn fetch_synced_lyrics(
        &self,
        query: &str,
        music_id: &str,
    ) -> Result<Option<Vec<LyricsLine>>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(None);
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        let search_url = "https://api.louvorja.com.br/pt/musics";
        let search: SearchResponse = client
            .get(search_url)
            .query(&[("q", query)])
            .send()
            .await
            .context("falha ao consultar a busca do LouvorJA")?
            .error_for_status()
            .context("LouvorJA retornou erro na busca")?
            .json()
            .await
            .context("falha ao desserializar a busca do LouvorJA")?;

        let Some(id_music) = search
            .data
            .into_iter()
            .next()
            .map(|item| item.id_music.into_string())
        else {
            return Ok(None);
        };

        let detail_url = format!("https://api.louvorja.com.br/pt/musics/{id_music}");
        let detail: MusicResponse = client
            .get(detail_url)
            .send()
            .await
            .context("falha ao consultar o detalhe do LouvorJA")?
            .error_for_status()
            .context("LouvorJA retornou erro no detalhe")?
            .json()
            .await
            .context("falha ao desserializar o detalhe do LouvorJA")?;

        let Some(lyrics) = detail.lyrics else {
            return Ok(None);
        };

        let lines = build_lines(music_id, lyrics);
        if lines.is_empty() {
            Ok(None)
        } else {
            Ok(Some(lines))
        }
    }
}

fn parse_time(time: &str) -> Option<f64> {
    let mut parts = time.split(':');
    let hours = parts.next()?.parse::<u64>().ok()?;
    let minutes = parts.next()?.parse::<u64>().ok()?;
    let seconds = parts.next()?.parse::<u64>().ok()?;

    if parts.next().is_some() {
        return None;
    }

    Some((hours * 3600 + minutes * 60 + seconds) as f64)
}

fn build_lines(music_id: &str, items: Vec<LyricItem>) -> Vec<LyricsLine> {
    let mut lines: Vec<LyricsLine> = items
        .into_iter()
        .filter_map(|item| {
            let text = item.lyric.unwrap_or_default().trim().to_string();
            if text.is_empty() {
                return None;
            }

            let start_time = parse_time(&item.time)?;
            Some(LyricsLine {
                id: 0,
                music_id: music_id.to_string(),
                start_time,
                end_time: start_time,
                text,
            })
        })
        .collect();

    for idx in 0..lines.len() {
        let next_start = lines.get(idx + 1).map(|line| line.start_time);
        lines[idx].end_time = next_start.unwrap_or(lines[idx].start_time + 5.0);
    }

    lines
}

impl IdMusic {
    fn into_string(self) -> String {
        match self {
            IdMusic::String(value) => value,
            IdMusic::Number(value) => value.to_string(),
            IdMusic::Unsigned(value) => value.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_time_converts_hms_to_seconds() {
        assert_eq!(parse_time("00:01:14"), Some(74.0));
        assert_eq!(parse_time("01:00:00"), Some(3600.0));
    }

    #[test]
    fn parse_time_rejects_invalid_input() {
        assert_eq!(parse_time("74"), None);
        assert_eq!(parse_time("00:01"), None);
    }

    #[test]
    fn filters_empty_lyrics() {
        let lines = build_lines(
            "m1",
            vec![LyricItem {
                lyric: Some("".to_string()),
                time: "00:01:14".to_string(),
            }],
        );

        assert!(lines.is_empty());
    }

    #[test]
    fn build_lines_sets_end_time_from_next_line() {
        let lines = build_lines(
            "m1",
            vec![
                LyricItem {
                    lyric: Some("Primeira".to_string()),
                    time: "00:00:10".to_string(),
                },
                LyricItem {
                    lyric: Some("Segunda".to_string()),
                    time: "00:00:14".to_string(),
                },
            ],
        );

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_time, 10.0);
        assert_eq!(lines[0].end_time, 14.0);
        assert_eq!(lines[1].start_time, 14.0);
        assert_eq!(lines[1].end_time, 19.0);
        assert_eq!(lines[0].music_id, "m1");
    }
}
