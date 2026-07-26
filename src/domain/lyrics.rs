use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LyricsLine {
    pub id: i64,
    pub music_id: String,
    pub start_time: f64,
    pub end_time: f64,
    pub text: String,
}
