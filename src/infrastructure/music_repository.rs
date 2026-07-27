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

    pub async fn list_all(&self) -> Result<Vec<Music>> {
        let music = sqlx::query_as::<_, Music>(
            "SELECT m.*, \
             EXISTS(SELECT 1 FROM lyrics_line l WHERE l.music_id = m.id) AS has_lyrics \
             FROM music m ORDER BY m.created_at DESC",
        )
        .fetch_all(self.pool)
        .await
        .context("falha ao listar as músicas")?;

        Ok(music)
    }

    /// Indica se a música possui linhas de letra persistidas localmente.
    pub async fn has_lyrics(&self, music_id: &str) -> Result<bool> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM lyrics_line WHERE music_id = ?)")
                .bind(music_id)
                .fetch_one(self.pool)
                .await
                .with_context(|| format!("falha ao verificar letras da música {}", music_id))?;

        Ok(exists)
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

    fn sample(id: &str, created_at: &str) -> Music {
        Music {
            id: id.to_string(),
            title: format!("Título {id}"),
            artist: Some(format!("Artista {id}")),
            youtube_url: format!("https://youtu.be/{id}"),
            duration: Some(180),
            thumbnail: None,
            created_at: Some(created_at.to_string()),
            has_lyrics: None,
        }
    }

    #[tokio::test]
    async fn list_all_returns_empty_for_new_db() {
        let pool = memory_pool().await;
        let repository = MusicRepository::new(&pool);

        assert!(repository.list_all().await.expect("list_all").is_empty());
    }

    #[tokio::test]
    async fn list_all_returns_saved_music() {
        let pool = memory_pool().await;
        let repository = MusicRepository::new(&pool);

        repository
            .save(&sample("abc", "2024-01-01 00:00:00"))
            .await
            .expect("save");

        let all = repository.list_all().await.expect("list_all");

        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "abc");
        assert_eq!(all[0].title, "Título abc");
        assert_eq!(all[0].artist.as_deref(), Some("Artista abc"));
        assert_eq!(all[0].youtube_url, "https://youtu.be/abc");
    }

    #[tokio::test]
    async fn list_all_orders_by_created_at_desc() {
        let pool = memory_pool().await;
        let repository = MusicRepository::new(&pool);

        repository
            .save(&sample("older", "2024-01-01 00:00:00"))
            .await
            .expect("save older");
        repository
            .save(&sample("newer", "2024-06-01 00:00:00"))
            .await
            .expect("save newer");

        let all = repository.list_all().await.expect("list_all");

        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, "newer");
        assert_eq!(all[1].id, "older");
    }

    #[tokio::test]
    async fn list_all_reports_has_lyrics_status() {
        use crate::domain::lyrics::LyricsLine;
        use crate::infrastructure::lyrics_repository::LyricsRepository;

        let pool = memory_pool().await;
        let repository = MusicRepository::new(&pool);

        repository
            .save(&sample("com_letra", "2024-01-01 00:00:00"))
            .await
            .expect("save com_letra");
        repository
            .save(&sample("sem_letra", "2024-02-01 00:00:00"))
            .await
            .expect("save sem_letra");

        LyricsRepository::new(&pool)
            .save_all(&[LyricsLine {
                id: 0,
                music_id: "com_letra".to_string(),
                start_time: 0.0,
                end_time: 1.0,
                text: "olá".to_string(),
            }])
            .await
            .expect("save lyrics");

        let all = repository.list_all().await.expect("list_all");
        let com = all.iter().find(|m| m.id == "com_letra").expect("com_letra");
        let sem = all.iter().find(|m| m.id == "sem_letra").expect("sem_letra");

        assert_eq!(com.has_lyrics, Some(true));
        assert_eq!(sem.has_lyrics, Some(false));
    }
}
