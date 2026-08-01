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
                format!(
                    "falha ao salvar a linha de letra da música {}",
                    line.music_id
                )
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

    pub async fn delete_by_music_id(&self, music_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM lyrics_line WHERE music_id = ?")
            .bind(music_id)
            .execute(self.pool)
            .await
            .with_context(|| {
                format!("falha ao remover as linhas de letra da música {}", music_id)
            })?;

        Ok(())
    }

    pub async fn update_text(&self, id: i64, new_text: &str) -> Result<()> {
        let result = sqlx::query("UPDATE lyrics_line SET text = ? WHERE id = ?")
            .bind(new_text)
            .bind(id)
            .execute(self.pool)
            .await
            .with_context(|| format!("falha ao atualizar a linha de letra {}", id))?;

        if result.rows_affected() == 0 {
            anyhow::bail!("linha de letra inexistente: {id}");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn line(music_id: &str, start_time: f64) -> LyricsLine {
        LyricsLine {
            id: 0,
            music_id: music_id.to_string(),
            start_time,
            end_time: start_time + 1.0,
            text: format!("linha {start_time}"),
        }
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

    #[tokio::test]
    async fn delete_by_music_id_removes_all_lines_for_music() {
        let pool = memory_pool().await;
        insert_music(&pool, "m1").await;
        insert_music(&pool, "m2").await;
        let repository = LyricsRepository::new(&pool);

        repository
            .save_all(&[line("m1", 0.0), line("m1", 1.0), line("m2", 0.0)])
            .await
            .expect("save_all");

        repository
            .delete_by_music_id("m1")
            .await
            .expect("delete m1");

        assert!(
            repository
                .find_by_music_id("m1")
                .await
                .expect("find m1")
                .is_empty()
        );
        assert_eq!(
            repository
                .find_by_music_id("m2")
                .await
                .expect("find m2")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn delete_by_music_id_is_idempotent() {
        let pool = memory_pool().await;
        let repository = LyricsRepository::new(&pool);

        assert!(repository.delete_by_music_id("inexistente").await.is_ok());
    }

    #[tokio::test]
    async fn update_text_changes_persisted_line() {
        let pool = memory_pool().await;
        insert_music(&pool, "m1").await;
        let repository = LyricsRepository::new(&pool);

        repository
            .save_all(&[line("m1", 0.0)])
            .await
            .expect("save_all");

        let before = repository
            .find_by_music_id("m1")
            .await
            .expect("find before");
        let id = before[0].id;

        repository
            .update_text(id, "texto corrigido")
            .await
            .expect("update_text");

        let after = repository.find_by_music_id("m1").await.expect("find after");
        assert_eq!(after[0].text, "texto corrigido");
    }
}
