use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
pub struct Music {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub youtube_url: String,
    pub duration: Option<i64>,
    pub thumbnail: Option<String>,
    pub created_at: Option<String>,
    pub sync_offset: f64,
    #[sqlx(default)]
    pub has_lyrics: Option<bool>,
}
