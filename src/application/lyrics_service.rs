use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;

use crate::domain::lyrics::LyricsLine;
use crate::infrastructure::lyrics_repository::LyricsRepository;
use crate::infrastructure::providers::{lrc_parser, srt_parser, vtt_parser};
use crate::infrastructure::whisper::WhisperService;
use crate::infrastructure::youtube::YoutubeService;
use crate::shared::utils::extract_video_id;

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
    whisper: Arc<WhisperService>,
}

impl LyricsService {
    pub fn new(
        pool: SqlitePool,
        youtube: Arc<YoutubeService>,
        whisper: Arc<WhisperService>,
    ) -> Self {
        Self {
            pool,
            youtube,
            whisper,
        }
    }

    pub async fn get_lyrics(
        &self,
        music_id: &str,
        youtube_url: &str,
        audio_path: Option<&std::path::Path>,
        on_status: &(dyn Fn(&str) + Send + Sync),
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
                    let lines = if vtt_content.trim_start().starts_with("WEBVTT") {
                        vtt_parser::parse(&vtt_content, music_id)
                    } else {
                        srt_parser::parse(&vtt_content, music_id)
                    };
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

        // 4. Fallback com IA (Whisper): transcreve o áudio local, se houver.
        if let Some(path) = audio_path {
            on_status("Gerando sincronização de letras via IA Whisper (isso pode demorar)...");
            match self.whisper.transcribe(path, music_id).await {
                Ok(lines) if !lines.is_empty() => {
                    self.persist(&repository, &lines).await;
                    return Ok(lines);
                }
                Ok(_) => {}
                Err(e) => tracing::warn!("falha na transcrição do Whisper: {e}"),
            }
        }

        tracing::warn!("nenhum provider encontrou letras para a música {music_id}");
        Ok(Vec::new())
    }

    pub async fn clear_cache(&self, music_id: &str) -> Result<()> {
        let repository = LyricsRepository::new(&self.pool);
        repository.delete_by_music_id(music_id).await
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
