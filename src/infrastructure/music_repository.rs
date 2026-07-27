use anyhow::{Context, Result};
use sqlx::sqlite::SqlitePool;

use crate::domain::music::Music;

/// Repositório de persistência para a entidade [`Music`] no SQLite.
pub struct MusicRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> MusicRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn save(&self, music: &Music) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO music \
             (id, title, artist, youtube_url, duration, thumbnail, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&music.id)
        .bind(&music.title)
        .bind(&music.artist)
        .bind(&music.youtube_url)
        .bind(music.duration)
        .bind(&music.thumbnail)
        .bind(&music.created_at)
        .execute(self.pool)
        .await
        .with_context(|| format!("falha ao salvar a música {}", music.id))?;

        Ok(())
    }

    pub async fn find_by_youtube_url(&self, url: &str) -> Result<Option<Music>> {
        let music = sqlx::query_as::<_, Music>("SELECT * FROM music WHERE youtube_url = ?")
            .bind(url)
            .fetch_optional(self.pool)
            .await
            .with_context(|| format!("falha ao buscar a música por youtube_url {}", url))?;

        Ok(music)
    }
}
