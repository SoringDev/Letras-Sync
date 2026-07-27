use anyhow::{Context, Result};
use sqlx::sqlite::SqlitePool;

use crate::domain::lyrics::LyricsLine;

/// Repositório de persistência para a entidade [`LyricsLine`] no SQLite.
pub struct LyricsRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> LyricsRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn save_all(&self, lines: &[LyricsLine]) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("falha ao iniciar a transação para salvar as linhas de letra")?;

        for line in lines {
            sqlx::query(
                "INSERT INTO lyrics_line (music_id, start_time, end_time, text) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(&line.music_id)
            .bind(line.start_time)
            .bind(line.end_time)
            .bind(&line.text)
            .execute(&mut *tx)
            .await
            .with_context(|| {
                format!("falha ao salvar a linha de letra da música {}", line.music_id)
            })?;
        }

        tx.commit()
            .await
            .context("falha ao confirmar a transação das linhas de letra")?;

        Ok(())
    }

    pub async fn find_by_music_id(&self, music_id: &str) -> Result<Vec<LyricsLine>> {
        let lines = sqlx::query_as::<_, LyricsLine>(
            "SELECT * FROM lyrics_line WHERE music_id = ? ORDER BY start_time",
        )
        .bind(music_id)
        .fetch_all(self.pool)
        .await
        .with_context(|| format!("falha ao buscar as linhas de letra da música {}", music_id))?;

        Ok(lines)
    }
}
