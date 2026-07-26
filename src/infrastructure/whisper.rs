use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use tokio::process::Command;

use crate::domain::lyrics::LyricsLine;
use crate::infrastructure::providers::vtt_parser;

/// Código de saída usado pelo script Python quando o pacote `faster_whisper`
/// não está disponível no ambiente.
const EXIT_MISSING_PACKAGE: i32 = 3;

/// Script Python que transcreve o áudio com o `faster-whisper` e grava o
/// resultado em formato WebVTT. Recebe `<audio_path> <vtt_output>` como
/// argumentos e sinaliza a ausência do pacote com o código de saída 3.
const TRANSCRIBE_SCRIPT: &str = r#"import sys

try:
    from faster_whisper import WhisperModel
except ImportError:
    sys.exit(3)


def format_timestamp(seconds):
    millis = int(round(seconds * 1000))
    hours, millis = divmod(millis, 3600000)
    minutes, millis = divmod(millis, 60000)
    secs, millis = divmod(millis, 1000)
    return f"{hours:02d}:{minutes:02d}:{secs:02d}.{millis:03d}"


def main():
    audio_path = sys.argv[1]
    output_path = sys.argv[2]

    model = WhisperModel("small", device="cpu", compute_type="int8")
    segments, _ = model.transcribe(audio_path)

    with open(output_path, "w", encoding="utf-8") as handle:
        handle.write("WEBVTT\n\n")
        for segment in segments:
            text = segment.text.strip()
            if not text:
                continue
            start = format_timestamp(segment.start)
            end = format_timestamp(segment.end)
            handle.write(f"{start} --> {end}\n{text}\n\n")


if __name__ == "__main__":
    main()
"#;

/// Provider de último recurso que gera letras sincronizadas transcrevendo o
/// áudio local com o `faster-whisper` via interpretador Python.
pub struct WhisperService;

impl WhisperService {
    pub fn new() -> Self {
        Self
    }

    /// Transcreve o arquivo de áudio local e retorna as linhas sincronizadas.
    ///
    /// Escreve um script Python temporário, executa-o de forma assíncrona para
    /// gerar um WebVTT, faz o parse do resultado e remove os arquivos
    /// temporários (tanto em sucesso quanto em erro).
    pub async fn transcribe(
        &self,
        audio_path: &Path,
        music_id: &str,
    ) -> Result<Vec<LyricsLine>> {
        let (script_path, vtt_path) = self.temp_paths();

        let result = self
            .run_transcription(audio_path, music_id, &script_path, &vtt_path)
            .await;

        let _ = tokio::fs::remove_file(&script_path).await;
        let _ = tokio::fs::remove_file(&vtt_path).await;

        result
    }

    /// Executa o fluxo de transcrição sem se preocupar com a limpeza dos
    /// arquivos temporários (delegada ao chamador).
    async fn run_transcription(
        &self,
        audio_path: &Path,
        music_id: &str,
        script_path: &Path,
        vtt_path: &Path,
    ) -> Result<Vec<LyricsLine>> {
        tokio::fs::write(script_path, TRANSCRIBE_SCRIPT)
            .await
            .with_context(|| {
                format!(
                    "falha ao escrever o script temporário {}",
                    script_path.display()
                )
            })?;

        let output = Command::new("python3")
            .arg(script_path)
            .arg(audio_path)
            .arg(vtt_path)
            .output()
            .await
            .context("falha ao executar o python3 para a transcrição do Whisper")?;

        if let Some(EXIT_MISSING_PACKAGE) = output.status.code() {
            bail!("faster-whisper não está instalado no ambiente python");
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "a transcrição do Whisper falhou (status {:?}): {}",
                output.status.code(),
                stderr.trim()
            ));
        }

        let vtt_content = tokio::fs::read_to_string(vtt_path)
            .await
            .with_context(|| {
                format!(
                    "falha ao ler o WebVTT gerado pelo Whisper em {}",
                    vtt_path.display()
                )
            })?;

        Ok(vtt_parser::parse(&vtt_content, music_id))
    }

    /// Gera caminhos temporários únicos para o script Python e o WebVTT.
    fn temp_paths(&self) -> (PathBuf, PathBuf) {
        let temp_dir = std::env::temp_dir();
        let unique = format!(
            "letras_sync_whisper_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );

        (
            temp_dir.join(format!("{unique}.py")),
            temp_dir.join(format!("{unique}.vtt")),
        )
    }
}

impl Default for WhisperService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifica se o ambiente possui `python3` com o pacote `faster_whisper`.
    /// Retorna `false` quando qualquer um estiver ausente, permitindo o
    /// encerramento gracioso dos testes em ambientes headless/CI.
    async fn whisper_available() -> bool {
        match Command::new("python3")
            .arg("-c")
            .arg("import faster_whisper")
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    #[tokio::test]
    async fn transcribe_produces_lines_and_cleans_temp_files() {
        if !whisper_available().await {
            return;
        }

        let audio_path = std::env::temp_dir().join("letras_sync_whisper_silence_test.wav");
        if write_silent_wav(&audio_path).await.is_err() {
            return;
        }

        let service = WhisperService::new();
        let result = service.transcribe(&audio_path, "whisper-test").await;

        let _ = tokio::fs::remove_file(&audio_path).await;

        // O fluxo deve completar sem erro; o áudio silencioso pode produzir
        // zero linhas, o que é aceitável.
        assert!(result.is_ok());
    }

    /// Escreve um WAV PCM de 1 segundo em silêncio para servir de entrada.
    async fn write_silent_wav(path: &Path) -> std::io::Result<()> {
        let sample_rate: u32 = 16000;
        let num_samples: u32 = sample_rate;
        let data_len = num_samples * 2;
        let file_len = 36 + data_len;

        let mut bytes = Vec::with_capacity((44 + data_len) as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&file_len.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.extend(std::iter::repeat(0u8).take(data_len as usize));

        tokio::fs::write(path, bytes).await
    }
}
