use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use tokio::process::Command;

use crate::domain::music::Music;
use crate::shared::utils::normalize_youtube_url;

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
        let url = normalize_youtube_url(url).unwrap_or_else(|| url.to_string());
        let output = yt_dlp_command()
            .args(["--dump-json", "--no-playlist", &url])
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

        let meta: YtDlpMetadata =
            serde_json::from_slice(&output.stdout).context("failed to parse yt-dlp JSON output")?;

        Ok(Music {
            id: meta.id,
            title: meta.title,
            artist: meta.uploader,
            youtube_url: meta.webpage_url.unwrap_or_else(|| url.to_string()),
            duration: meta.duration.map(|d| d as i64),
            thumbnail: meta.thumbnail,
            created_at: None,
            sync_offset: 0.0,
            has_lyrics: None,
        })
    }

    /// Baixa apenas o áudio da `url` e retorna o caminho do arquivo baixado.
    ///
    /// Invoca o executável `yt-dlp` de forma assíncrona sem transcodificação,
    /// para não depender de `ffmpeg`/`ffprobe`.
    pub async fn download_audio(&self, url: &str, output_template: &Path) -> Result<PathBuf> {
        let url = normalize_youtube_url(url).unwrap_or_else(|| url.to_string());
        tracing::info!(
            "iniciando download de áudio de {url} para {}",
            output_template.display()
        );

        let output = yt_dlp_command()
            .args(audio_download_args(&url, output_template))
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

        let downloaded_path = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .rev()
            .find(|line| !line.is_empty())
            .map(PathBuf::from)
            .context("yt-dlp não informou o caminho final do arquivo baixado")?;

        tracing::info!(
            "download de áudio concluído em {}",
            downloaded_path.display()
        );
        Ok(downloaded_path)
    }

    pub async fn fetch_captions(&self, video_id: &str) -> Result<Option<String>> {
        let temp_dir = std::env::temp_dir();
        let output_template = temp_dir.join(video_id);

        let status = yt_dlp_command()
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

fn audio_download_args(url: &str, output_template: &Path) -> Vec<OsString> {
    vec![
        OsString::from("-f"),
        OsString::from("ba"),
        OsString::from("--no-playlist"),
        OsString::from("--no-progress"),
        OsString::from("-o"),
        output_template.as_os_str().to_os_string(),
        OsString::from("-O"),
        OsString::from("after_move:filepath"),
        OsString::from(url),
    ]
}

fn yt_dlp_command() -> Command {
    let mut command = Command::new("yt-dlp");

    if node_runtime_available() {
        command.args(["--js-runtimes", "node"]);
    }

    command
}

fn node_runtime_available() -> bool {
    std::process::Command::new("node")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

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
        let output_path = std::env::temp_dir().join("letras_sync_download_test.%(ext)s");

        let result = service
            .download_audio("https://youtu.be/________invalid", &output_path)
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn audio_download_args_do_not_request_ffmpeg_transcoding() {
        let output_path = Path::new("/tmp/letras_sync_download_test.%(ext)s");
        let args = audio_download_args("https://youtu.be/abc123", output_path);

        assert!(args.iter().any(|arg| arg == OsStr::new("ba")));
        assert!(!args.iter().any(|arg| arg == OsStr::new("-x")));
        assert!(!args.iter().any(|arg| arg == OsStr::new("--audio-format")));
    }

    #[test]
    fn node_runtime_detection_is_consistent() {
        let command = yt_dlp_command();
        if node_runtime_available() {
            let debug = format!("{command:?}");
            assert!(debug.contains("--js-runtimes"));
            assert!(debug.contains("node"));
        }
    }
}
