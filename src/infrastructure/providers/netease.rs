use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::domain::lyrics::LyricsLine;
use crate::infrastructure::providers::lrc_parser;

const NETEASE_COOKIE: &str = "NMTID=00OAVK3xqDG726ITU6jopU6jF2yMk0AAAGCO8l1BA";

#[derive(Debug, Deserialize)]
struct SearchResponse {
    result: Option<SearchResult>,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    songs: Option<Vec<Song>>,
}

#[derive(Debug, Deserialize)]
struct Song {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct LyricResponse {
    lrc: Option<LyricContent>,
}

#[derive(Debug, Deserialize)]
struct LyricContent {
    lyric: Option<String>,
}

pub struct NeteaseProvider;

impl NeteaseProvider {
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
            .user_agent("Mozilla/5.0 (X11; Linux x86_64)")
            .timeout(Duration::from_secs(6))
            .build()?;

        // Passo 1: busca o ID da música pelo título
        let search: SearchResponse = client
            .post("https://music.163.com/api/search/pc")
            .header("cookie", NETEASE_COOKIE)
            .form(&[("s", query), ("type", "1"), ("offset", "0"), ("limit", "5")])
            .send()
            .await
            .context("netease: falha na busca")?
            .json()
            .await
            .context("netease: falha ao desserializar busca")?;

        let song_id = search
            .result
            .and_then(|r| r.songs)
            .and_then(|s| s.into_iter().next())
            .map(|s| s.id);

        let Some(song_id) = song_id else {
            return Ok(None);
        };

        // Passo 2: busca a letra LRC pelo ID
        let lyric: LyricResponse = client
            .get("https://music.163.com/api/song/lyric")
            .header("cookie", NETEASE_COOKIE)
            .query(&[
                ("id", &song_id.to_string()),
                ("lv", &"1".to_string()),
                ("kv", &"1".to_string()),
                ("tv", &"-1".to_string()),
            ])
            .send()
            .await
            .context("netease: falha ao buscar letra")?
            .json()
            .await
            .context("netease: falha ao desserializar letra")?;

        let lrc_text = lyric.lrc.and_then(|l| l.lyric).unwrap_or_default();
        if lrc_text.trim().is_empty() {
            return Ok(None);
        }

        let lines = lrc_parser::parse(&lrc_text, music_id);
        if lines.is_empty() {
            Ok(None)
        } else {
            Ok(Some(lines))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_instance_does_not_panic() {
        let _ = NeteaseProvider::new();
    }
}
