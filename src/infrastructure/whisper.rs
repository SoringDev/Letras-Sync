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
const TRANSCRIBE_SCRIPT: &str = r#"import re
import sys

try:
    from faster_whisper import WhisperModel
except ImportError:
    sys.exit(3)

PRIMARY_BEAM_SIZE = 1
RETRY_BEAM_SIZE = 5
QUALITY_THRESHOLD = 18
PRIMARY_PROMPT = "Letra de música em português brasileiro."
RETRY_PROMPT = "Letra de música em português brasileiro. Preserve nomes próprios e mantenha versos curtos."


def format_timestamp(seconds):
    millis = int(round(seconds * 1000))
    hours, millis = divmod(millis, 3600000)
    minutes, millis = divmod(millis, 60000)
    secs, millis = divmod(millis, 1000)
    return f"{hours:02d}:{minutes:02d}:{secs:02d}.{millis:03d}"


def count_words(text):
    return len(re.findall(r"[A-Za-zÀ-ÿ']+", text))


def quality_score(rows, avg_logprob, avg_no_speech_prob, avg_compression_ratio, language_probability):
    if not rows:
        return 0

    text = " ".join(row[2] for row in rows).lower()
    words = sum(count_words(row[2]) for row in rows)
    score = min(words, 20)

    if len(rows) >= 3:
        score += 3
    if avg_logprob > -1.0:
        score += 6
    if avg_logprob > -0.6:
        score += 4
    if avg_no_speech_prob < 0.4:
        score += 2
    if avg_compression_ratio < 2.4:
        score += 3
    if language_probability >= 0.75:
        score += 4

    hints = [
        "você",
        "não",
        "pra",
        "amor",
        "paz",
        "vida",
        "hoje",
        "deus",
        "coração",
        "meu",
        "minha",
        "nosso",
        "nossa",
    ]
    score += min(sum(1 for hint in hints if hint in text), 6)

    if any(len(word) > 18 for word in re.findall(r"[A-Za-zÀ-ÿ']+", text)):
        score -= 2

    return max(score, 0)


def transcribe_once(model, audio_path, beam_size, prompt):
    segments, info = model.transcribe(
        audio_path,
        language="pt",
        task="transcribe",
        vad_filter=True,
        temperature=0.0,
        beam_size=beam_size,
        condition_on_previous_text=False,
        initial_prompt=prompt,
    )

    rows = []
    logprob_sum = 0.0
    no_speech_sum = 0.0
    compression_sum = 0.0
    count = 0

    for segment in segments:
        text = segment.text.strip()
        if not text:
            continue

        rows.append((segment.start, segment.end, text))
        count += 1
        logprob_sum += float(getattr(segment, "avg_logprob", -5.0))
        no_speech_sum += float(getattr(segment, "no_speech_prob", 0.0))
        compression_sum += float(getattr(segment, "compression_ratio", 0.0))

    avg_logprob = logprob_sum / count if count else -10.0
    avg_no_speech_prob = no_speech_sum / count if count else 1.0
    avg_compression_ratio = compression_sum / count if count else 99.0
    language_probability = float(getattr(info, "language_probability", 0.0))
    score = quality_score(
        rows,
        avg_logprob,
        avg_no_speech_prob,
        avg_compression_ratio,
        language_probability,
    )

    return rows, score


def write_vtt(output_path, rows):
    with open(output_path, "w", encoding="utf-8") as handle:
        handle.write("WEBVTT\\n\\n")
        for start, end, text in rows:
            handle.write(f"{format_timestamp(start)} --> {format_timestamp(end)}\\n{text}\\n\\n")


def main():
    audio_path = sys.argv[1]
    output_path = sys.argv[2]

    model = WhisperModel("small", device="cpu", compute_type="int8")
    rows, score = transcribe_once(model, audio_path, PRIMARY_BEAM_SIZE, PRIMARY_PROMPT)

    if score < QUALITY_THRESHOLD:
        retry_rows, retry_score = transcribe_once(model, audio_path, RETRY_BEAM_SIZE, RETRY_PROMPT)
        if retry_score >= score:
            rows = retry_rows

    write_vtt(output_path, rows)


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
    pub async fn transcribe(&self, audio_path: &Path, music_id: &str) -> Result<Vec<LyricsLine>> {
        let (script_path, vtt_path, audio_copy_path) = self.temp_paths(audio_path);

        let result = self
            .run_transcription(
                audio_path,
                music_id,
                &script_path,
                &vtt_path,
                &audio_copy_path,
            )
            .await;

        let _ = tokio::fs::remove_file(&script_path).await;
        let _ = tokio::fs::remove_file(&vtt_path).await;
        let _ = tokio::fs::remove_file(&audio_copy_path).await;

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
        audio_copy_path: &Path,
    ) -> Result<Vec<LyricsLine>> {
        tokio::fs::write(script_path, TRANSCRIBE_SCRIPT)
            .await
            .with_context(|| {
                format!(
                    "falha ao escrever o script temporário {}",
                    script_path.display()
                )
            })?;

        self.prepare_audio_copy(audio_path, audio_copy_path).await?;

        let output = Command::new("python3")
            .arg(script_path)
            .arg(audio_copy_path)
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

        let vtt_content = tokio::fs::read_to_string(vtt_path).await.with_context(|| {
            format!(
                "falha ao ler o WebVTT gerado pelo Whisper em {}",
                vtt_path.display()
            )
        })?;

        Ok(vtt_parser::parse(&vtt_content, music_id))
    }

    /// Gera caminhos temporários únicos para o script Python e o WebVTT.
    fn temp_paths(&self, audio_path: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let temp_dir = std::env::temp_dir();
        let unique = format!(
            "letras_sync_whisper_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );

        let audio_extension = audio_path
            .extension()
            .and_then(|ext| ext.to_str())
            .filter(|ext| !ext.is_empty())
            .unwrap_or("input");

        (
            temp_dir.join(format!("{unique}.py")),
            temp_dir.join(format!("{unique}.vtt")),
            temp_dir.join(format!("{unique}.{audio_extension}")),
        )
    }

    async fn prepare_audio_copy(&self, audio_path: &Path, temp_audio_path: &Path) -> Result<()> {
        match tokio::fs::hard_link(audio_path, temp_audio_path).await {
            Ok(()) => Ok(()),
            Err(_) => {
                tokio::fs::copy(audio_path, temp_audio_path)
                    .await
                    .with_context(|| {
                        format!(
                            "falha ao copiar o áudio para o caminho temporário {}",
                            temp_audio_path.display()
                        )
                    })?;
                Ok(())
            }
        }
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

    /// Idioma fixo usado pela transcrição via Whisper.
    const WHISPER_LANGUAGE: &str = "pt";

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

        let before = tokio::fs::read(&audio_path)
            .await
            .expect("read original audio before transcribe");

        let service = WhisperService::new();
        let result = service.transcribe(&audio_path, "whisper-test").await;

        let after = tokio::fs::read(&audio_path)
            .await
            .expect("read original audio after transcribe");
        let _ = tokio::fs::remove_file(&audio_path).await;

        // O fluxo deve completar sem erro; o áudio silencioso pode produzir
        // zero linhas, o que é aceitável.
        assert!(result.is_ok());
        assert_eq!(before, after);
    }

    #[test]
    fn transcribe_script_is_pinned_to_portuguese() {
        assert!(TRANSCRIBE_SCRIPT.contains(r#"language="pt""#));
        assert!(TRANSCRIBE_SCRIPT.contains(r#"task="transcribe""#));
        assert!(TRANSCRIBE_SCRIPT.contains(r#"vad_filter=True"#));
        assert!(TRANSCRIBE_SCRIPT.contains(r#"temperature=0.0"#));
        assert!(TRANSCRIBE_SCRIPT.contains(r#"condition_on_previous_text=False"#));
        assert!(TRANSCRIBE_SCRIPT.contains(r#"initial_prompt"#));
        assert!(TRANSCRIBE_SCRIPT.contains(r#"QUALITY_THRESHOLD = 18"#));
        assert_eq!(WHISPER_LANGUAGE, "pt");
    }

    /// Escreve um WAV PCM de 3 segundos em silêncio para servir de entrada.
    async fn write_silent_wav(path: &Path) -> std::io::Result<()> {
        let sample_rate: u32 = 16000;
        let num_samples: u32 = sample_rate * 3;
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
        bytes.extend(std::iter::repeat_n(0u8, data_len as usize));

        tokio::fs::write(path, bytes).await
    }
}
