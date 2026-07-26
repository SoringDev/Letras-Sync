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

    /// Baixa apenas o áudio da `url` e o salva em `output_path` (formato mp3).
    ///
    /// Invoca o executável `yt-dlp` de forma assíncrona extraindo somente o
    /// áudio. O `output_path` é usado como template de saída (`-o`) e deve
    /// incluir a extensão `.mp3` desejada.
    pub async fn download_audio(&self, url: &str, output_path: &std::path::Path) -> Result<()> {
        tracing::info!(
            "iniciando download de áudio de {url} para {}",
            output_path.display()
        );

        let output = Command::new("yt-dlp")
            .args([
                "-x",
                "--audio-format",
                "mp3",
                "--audio-quality",
                "0",
                "--no-playlist",
                "-o",
            ])
            .arg(output_path)
            .arg(url)
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

        tracing::info!("download de áudio concluído em {}", output_path.display());
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifica se o executável `yt-dlp` está disponível no ambiente.
    async fn yt_dlp_available() -> bool {
        Command::new("yt-dlp")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    // Depende do `yt-dlp`. Quando não está disponível no ambiente, o teste é
    // encerrado graciosamente sem falhar.
    #[tokio::test]
    async fn download_audio_fails_on_invalid_url() {
        if !yt_dlp_available().await {
            return;
        }

        let service = YoutubeService::new();
        let output_path = std::env::temp_dir().join("letras_sync_download_test.mp3");

        let result = service
            .download_audio("https://youtu.be/________invalid", &output_path)
            .await;

        assert!(result.is_err());
        let _ = tokio::fs::remove_file(&output_path).await;
    }
}
