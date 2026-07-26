use anyhow::{Context, Result, bail};
use serde::Deserialize;
use tokio::process::Command;

use crate::domain::music::Music;

#[derive(Debug, Deserialize)]
struct YtDlpMetadata {
    id: String,
    title: String,
    uploader: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    webpage_url: Option<String>,
}

pub struct YoutubeService;

impl YoutubeService {
    pub fn new() -> Self {
        Self
    }

    pub async fn fetch_metadata(&self, url: &str) -> Result<Music> {
        let output = Command::new("yt-dlp")
            .args(["--dump-json", "--no-playlist", url])
            .output()
            .await
            .context("failed to execute yt-dlp (is it installed and in PATH?)")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "yt-dlp exited with status {}: {}",
                output.status,
                stderr.trim()
            );
        }

        let meta: YtDlpMetadata = serde_json::from_slice(&output.stdout)
            .context("failed to parse yt-dlp JSON output")?;

        Ok(Music {
            id: meta.id,
            title: meta.title,
            artist: meta.uploader,
            youtube_url: meta.webpage_url.unwrap_or_else(|| url.to_string()),
            duration: meta.duration.map(|d| d as i64),
            thumbnail: meta.thumbnail,
            created_at: None,
        })
    }

    pub async fn fetch_captions(&self, video_id: &str) -> Result<Option<String>> {
        let temp_dir = std::env::temp_dir();
        let output_template = temp_dir.join(video_id);

        let status = Command::new("yt-dlp")
            .args([
                "--write-auto-subs",
                "--write-subs",
                "--sub-langs",
                "pt,pt-BR,en",
                "--sub-format",
                "vtt",
                "--skip-download",
                "--no-playlist",
                "-o",
            ])
            .arg(&output_template)
            .arg(format!("https://www.youtube.com/watch?v={video_id}"))
            .status()
            .await
            .context("failed to execute yt-dlp (is it installed and in PATH?)")?;

        if !status.success() {
            return Ok(None);
        }

        let mut entries = tokio::fs::read_dir(&temp_dir)
            .await
            .context("failed to read temp directory")?;

        let mut vtt_files: Vec<std::path::PathBuf> = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .context("failed to read temp directory entry")?
        {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.starts_with(video_id)
                && name.ends_with(".vtt")
            {
                vtt_files.push(path);
            }
        }

        if vtt_files.is_empty() {
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(&vtt_files[0]).await;

        for file in &vtt_files {
            let _ = tokio::fs::remove_file(file).await;
        }

        match content {
            Ok(text) => Ok(Some(text)),
            Err(_) => Ok(None),
        }
    }
}
