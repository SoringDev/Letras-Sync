use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;

use crate::domain::lyrics::LyricsLine;
use crate::infrastructure::lyrics_repository::LyricsRepository;
use crate::infrastructure::music_repository::MusicRepository;
use crate::infrastructure::providers::{
    louvorja::LouvorJaProvider, lrc_parser, netease::NeteaseProvider, srt_parser, vtt_parser,
};
use crate::infrastructure::whisper::WhisperService;
use crate::infrastructure::youtube::YoutubeService;
use crate::shared::utils::{extract_song_title_candidates, extract_video_id};

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
    louvorja: Arc<LouvorJaProvider>,
    netease: Arc<NeteaseProvider>,
    whisper: Arc<WhisperService>,
    cache_path: PathBuf,
}

impl LyricsService {
    pub fn new(
        pool: SqlitePool,
        youtube: Arc<YoutubeService>,
        whisper: Arc<WhisperService>,
        cache_path: PathBuf,
    ) -> Self {
        Self {
            pool,
            youtube,
            louvorja: Arc::new(LouvorJaProvider::new()),
            netease: Arc::new(NeteaseProvider::new()),
            whisper,
            cache_path,
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
                        return Ok(self.persist_and_reload(&repository, music_id, &lines).await);
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::warn!("falha ao buscar legendas do YouTube: {e}"),
            }
        } else {
            tracing::warn!("não foi possível extrair o video_id da URL: {youtube_url}");
        }

        // 3. LouvorJA + 4. NetEase — tenta cada candidato de título extraído do YouTube
        let raw_title = match MusicRepository::new(&self.pool)
            .find_by_youtube_url(youtube_url)
            .await
        {
            Ok(Some(music)) if !music.title.trim().is_empty() => music.title,
            Ok(_) => music_id.to_string(),
            Err(e) => {
                tracing::warn!("falha ao consultar a música para buscar nos providers: {e}");
                music_id.to_string()
            }
        };

        let title_candidates = extract_song_title_candidates(&raw_title);
        tracing::info!("candidatos de título para busca: {:?}", title_candidates);

        for candidate in &title_candidates {
            match self
                .louvorja
                .fetch_synced_lyrics(candidate, music_id, &self.cache_path)
                .await
            {
                Ok(Some(lines)) if !lines.is_empty() => {
                    return Ok(self.persist_and_reload(&repository, music_id, &lines).await);
                }
                Ok(_) => {}
                Err(e) => tracing::warn!("falha ao consultar o LouvorJA com '{candidate}': {e}"),
            }

            match self.netease.fetch_synced_lyrics(candidate, music_id).await {
                Ok(Some(lines)) if !lines.is_empty() => {
                    return Ok(self.persist_and_reload(&repository, music_id, &lines).await);
                }
                Ok(_) => {}
                Err(e) => tracing::warn!("falha ao consultar o NetEase com '{candidate}': {e}"),
            }
        }

        // 5. LRCLib (o music_id é usado como termo de busca).
        match self.fetch_from_lrclib(music_id).await {
            Ok(Some(lrc_content)) => {
                let lines = lrc_parser::parse(&lrc_content, music_id);
                if !lines.is_empty() {
                    return Ok(self.persist_and_reload(&repository, music_id, &lines).await);
                }
            }
            Ok(None) => {}
            Err(e) => tracing::warn!("falha ao consultar o LRCLib: {e}"),
        }

        // 6. Fallback com IA (Whisper): transcreve o áudio local, se houver.
        if let Some(path) = audio_path {
            on_status("Gerando sincronização de letras via IA Whisper (isso pode demorar)...");
            match self.whisper.transcribe(path, music_id).await {
                Ok(lines) if !lines.is_empty() => {
                    return Ok(self.persist_and_reload(&repository, music_id, &lines).await);
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

    pub async fn update_text(&self, id: i64, new_text: &str) -> Result<()> {
        let repository = LyricsRepository::new(&self.pool);
        repository.update_text(id, new_text).await
    }

    async fn persist(&self, repository: &LyricsRepository<'_>, lines: &[LyricsLine]) {
        if let Err(e) = repository.save_all(lines).await {
            tracing::warn!("falha ao persistir as letras: {e}");
        }
    }

    async fn persist_and_reload(
        &self,
        repository: &LyricsRepository<'_>,
        music_id: &str,
        lines: &[LyricsLine],
    ) -> Vec<LyricsLine> {
        self.persist(repository, lines).await;

        match repository.find_by_music_id(music_id).await {
            Ok(saved) if !saved.is_empty() => saved,
            _ => lines.to_vec(),
        }
    }

    async fn fetch_from_lrclib(&self, query: &str) -> Result<Option<String>> {
        let client = reqwest::Client::builder()
            .user_agent("LetrasSync/0.1.0 (https://github.com/SamuelPS/Letras-Sync)")
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

#[cfg(test)]
mod tests {
    use super::*;

    use sqlx::sqlite::SqlitePool;

    async fn memory_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("pool em memória");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrações");
        pool
    }

    async fn insert_music(pool: &SqlitePool, id: &str) {
        sqlx::query("INSERT INTO music (id, title, youtube_url) VALUES (?, ?, ?)")
            .bind(id)
            .bind(format!("Título {id}"))
            .bind(format!("https://youtu.be/{id}"))
            .execute(pool)
            .await
            .expect("insert music");
    }

    fn line(music_id: &str, start_time: f64, text: &str) -> LyricsLine {
        LyricsLine {
            id: 0,
            music_id: music_id.to_string(),
            start_time,
            end_time: start_time + 1.0,
            text: text.to_string(),
        }
    }

    #[tokio::test]
    async fn persist_and_reload_returns_saved_lines_with_generated_ids() {
        let pool = memory_pool().await;
        insert_music(&pool, "m1").await;

        let service = LyricsService::new(
            pool.clone(),
            Arc::new(crate::infrastructure::youtube::YoutubeService::new()),
            Arc::new(crate::infrastructure::whisper::WhisperService::new()),
            std::env::temp_dir(),
        );
        let repository = LyricsRepository::new(&pool);
        let original = vec![line("m1", 0.0, "primeira"), line("m1", 1.0, "segunda")];

        let saved = service
            .persist_and_reload(&repository, "m1", &original)
            .await;

        assert_eq!(saved.len(), 2);
        assert!(saved.iter().all(|line| line.id > 0));
        assert_ne!(saved[0].id, original[0].id);
        assert_ne!(saved[1].id, original[1].id);
        assert_eq!(saved[0].text, "primeira");
        assert_eq!(saved[1].text, "segunda");
    }
}
