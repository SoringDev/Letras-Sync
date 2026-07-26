use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;

use crate::domain::lyrics::LyricsLine;
use crate::infrastructure::lyrics_repository::LyricsRepository;
use crate::infrastructure::providers::{lrc_parser, vtt_parser};
use crate::infrastructure::youtube::YoutubeService;

use sqlx::sqlite::SqlitePool;

/// Resultado individual retornado pela busca do LRCLib.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibResult {
    synced_lyrics: Option<String>,
}

/// Orquestra os providers de letras seguindo a cadeia de prioridade:
/// cache local → legendas do YouTube (VTT) → LRCLib (LRC) → fallback (stub).
pub struct LyricsService {
    pool: SqlitePool,
    youtube: Arc<YoutubeService>,
}

impl LyricsService {
    pub fn new(pool: SqlitePool, youtube: Arc<YoutubeService>) -> Self {
        Self { pool, youtube }
    }

    pub async fn get_lyrics(
        &self,
        music_id: &str,
        youtube_url: &str,
    ) -> Result<Vec<LyricsLine>> {
        let repository = LyricsRepository::new(&self.pool);

        // 1. Cache local.
        match repository.find_by_music_id(music_id).await {
            Ok(lines) if !lines.is_empty() => return Ok(lines),
            Ok(_) => {}
            Err(e) => tracing::warn!("falha ao consultar o cache de letras: {e}"),
        }

        // 2. Legendas do YouTube (VTT).
        if let Some(video_id) = extract_video_id(youtube_url) {
            match self.youtube.fetch_captions(&video_id).await {
                Ok(Some(vtt_content)) => {
                    let lines = vtt_parser::parse(&vtt_content, music_id);
                    if !lines.is_empty() {
                        self.persist(&repository, &lines).await;
                        return Ok(lines);
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("falha ao buscar legendas do YouTube: {e}"),
            }
        } else {
            tracing::warn!("não foi possível extrair o video_id da URL: {youtube_url}");
        }

        // 3. LRCLib (o music_id é usado como termo de busca).
        match self.fetch_from_lrclib(music_id).await {
            Ok(Some(lrc_content)) => {
                let lines = lrc_parser::parse(&lrc_content, music_id);
                if !lines.is_empty() {
                    self.persist(&repository, &lines).await;
                    return Ok(lines);
                }
            }
            Ok(None) => {}
            Err(e) => tracing::warn!("falha ao consultar o LRCLib: {e}"),
        }

        // 4. Fallback stub (Whisper ainda não implementado).
        tracing::warn!("nenhum provider encontrou letras para a música {music_id}");
        Ok(Vec::new())
    }

    async fn persist(&self, repository: &LyricsRepository<'_>, lines: &[LyricsLine]) {
        if let Err(e) = repository.save_all(lines).await {
            tracing::warn!("falha ao persistir as letras: {e}");
        }
    }

    async fn fetch_from_lrclib(&self, query: &str) -> Result<Option<String>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        let results: Vec<LrclibResult> = client
            .get("https://lrclib.net/api/search")
            .query(&[("q", query)])
            .send()
            .await?
            .json()
            .await?;

        Ok(results
            .into_iter()
            .find_map(|r| r.synced_lyrics.filter(|s| !s.is_empty())))
    }
}

/// Extrai o `video_id` de uma URL do YouTube.
///
/// Suporta o parâmetro `v=` da query string e o formato curto `youtu.be/<id>`.
/// Retorna `None` se nenhum padrão for reconhecido.
fn extract_video_id(url: &str) -> Option<String> {
    if let Some(idx) = url.find("v=") {
        let rest = &url[idx + 2..];
        let id = rest.split('&').next().unwrap_or(rest);
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }

    if let Some(idx) = url.find("youtu.be/") {
        let rest = &url[idx + "youtu.be/".len()..];
        let id = rest.split(['?', '&', '/']).next().unwrap_or(rest);
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_video_id_from_query_param() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=abc123"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn extracts_video_id_with_extra_params() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=abc123&t=10s"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn extracts_video_id_from_short_url() {
        assert_eq!(
            extract_video_id("https://youtu.be/abc123"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn extracts_video_id_from_short_url_with_query() {
        assert_eq!(
            extract_video_id("https://youtu.be/abc123?t=10"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn returns_none_for_unrecognized_url() {
        assert_eq!(extract_video_id("https://example.com/video"), None);
    }
}
