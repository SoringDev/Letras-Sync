use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::sqlite::SqlitePool;
use std::path::{Path, PathBuf};
use tokio::sync::{RwLock, broadcast};

use crate::domain::lyrics::LyricsLine;
use crate::domain::music::Music;
use crate::infrastructure::audio::AudioEngine;
use crate::infrastructure::music_repository::MusicRepository;
use crate::infrastructure::youtube::YoutubeService;
use crate::shared::utils::{extract_video_id, normalize_youtube_url};

use super::lyrics_service::LyricsService;

/// Intervalo do loop de polling em segundo plano.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Variação mínima de posição (em segundos) que dispara `PositionUpdated`.
const POSITION_EPSILON: f64 = 0.01;

/// Capacidade do canal de eventos reativos do player.
const EVENT_CHANNEL_CAPACITY: usize = 64;

/// Nome exibido para mídias carregadas do sistema de arquivos local.
const LOCAL_ARTIST_NAME: &str = "Arquivo Local";

/// Extensões de áudio que podem existir no cache do `yt-dlp`.
const CACHED_AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "webm", "opus", "ogg", "aac", "flac", "wav", "mp4", "mkv", "mka",
];

/// Estados possíveis do ciclo de vida da reprodução.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
}

/// Eventos reativos propagados aos consumidores do player.
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    StateChanged(PlaybackState),
    MusicLoaded {
        music: Music,
        lyrics: Vec<LyricsLine>,
    },
    LyricsUpdated(Vec<LyricsLine>),
    PositionUpdated {
        position: f64,
        duration: Option<f64>,
    },
    PlaybackFinished,
    LoadingStatus(String),
}

/// Estado interno compartilhado do player.
struct PlayerState {
    status: PlaybackState,
    current_music: Option<Music>,
    current_lyrics: Vec<LyricsLine>,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            status: PlaybackState::Idle,
            current_music: None,
            current_lyrics: Vec::new(),
        }
    }
}

/// Tenta resolver uma entrada como mídia local existente.
async fn resolve_local_media_path(input: &str) -> Result<Option<PathBuf>> {
    let path = if let Some(raw) = input.strip_prefix("file://") {
        Some(PathBuf::from(decode_file_uri_path(raw)))
    } else {
        let candidate = Path::new(input);
        if tokio::fs::try_exists(candidate).await.with_context(|| {
            format!("falha ao verificar o caminho local {}", candidate.display())
        })? {
            Some(candidate.to_path_buf())
        } else {
            None
        }
    };

    match path {
        Some(path) => Ok(Some(tokio::fs::canonicalize(&path).await.with_context(
            || format!("falha ao resolver o caminho local {}", path.display()),
        )?)),
        None => Ok(None),
    }
}

/// Decodifica percent-encoding simples usado em `file://`.
fn decode_file_uri_path(input: &str) -> String {
    let mut bytes_out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = bytes[i + 1];
                let lo = bytes[i + 2];
                if let (Some(hi), Some(lo)) = (from_hex_digit(hi), from_hex_digit(lo)) {
                    bytes_out.push(hi * 16 + lo);
                    i += 3;
                } else {
                    bytes_out.push(b'%');
                    i += 1;
                }
            }
            byte => {
                bytes_out.push(byte);
                i += 1;
            }
        }
    }

    String::from_utf8_lossy(&bytes_out).into_owned()
}

fn from_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn local_music_id(path: &Path) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;

    for byte in path.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    format!("local-{hash:016x}")
}

async fn media_music_id(input: &str) -> Result<String> {
    if let Some(path) = resolve_local_media_path(input).await? {
        return Ok(local_music_id(&path));
    }

    extract_video_id(input)
        .map(|id| id.to_string())
        .ok_or_else(|| anyhow::anyhow!("não foi possível identificar a mídia informada: {input}"))
}

/// Serviço de controle e orquestração da reprodução.
///
/// Integra o `AudioEngine`, o `YoutubeService`, o `LyricsService` e os
/// repositórios para gerenciar o estado da reprodução de forma segura para
/// concorrência e notificar os consumidores via canal `broadcast`.
pub struct Player {
    audio_engine: Arc<AudioEngine>,
    youtube_service: Arc<YoutubeService>,
    lyrics_service: Arc<LyricsService>,
    pool: SqlitePool,
    cache_path: PathBuf,
    state: Arc<RwLock<PlayerState>>,
    event_tx: broadcast::Sender<PlayerEvent>,
}

impl Player {
    /// Inicializa o player e inicia a tarefa de polling em segundo plano.
    pub fn new(
        audio_engine: Arc<AudioEngine>,
        youtube_service: Arc<YoutubeService>,
        lyrics_service: Arc<LyricsService>,
        pool: SqlitePool,
        cache_path: PathBuf,
        volume: i64,
    ) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);

        let player = Arc::new(Self {
            audio_engine,
            youtube_service,
            lyrics_service,
            pool,
            cache_path,
            state: Arc::new(RwLock::new(PlayerState::default())),
            event_tx,
        });

        if let Err(err) = player.audio_engine.set_volume(volume) {
            tracing::warn!("falha ao aplicar o volume inicial {volume}: {err:?}");
        } else {
            let applied_volume = player.audio_engine.volume();
            if applied_volume != volume {
                tracing::warn!(
                    "volume inicial divergente após aplicação: solicitado={volume}, aplicado={applied_volume}"
                );
            }
        }

        Self::spawn_poll_loop(&player);

        player
    }

    /// Assina o canal de eventos reativos do player.
    pub fn subscribe(&self) -> broadcast::Receiver<PlayerEvent> {
        self.event_tx.subscribe()
    }

    /// Carrega uma mídia local ou do YouTube e inicia a reprodução.
    pub async fn load_youtube(&self, url: &str) -> Result<()> {
        self.load_media(url).await
    }

    /// Carrega uma mídia local ou remota e inicia a reprodução.
    pub async fn load_media(&self, input: &str) -> Result<()> {
        if let Some(local_path) = resolve_local_media_path(input).await? {
            return self.load_local_media(&local_path).await;
        }

        let normalized =
            crate::shared::utils::normalize_youtube_url(input).unwrap_or_else(|| input.to_string());

        self.load_remote_youtube(&normalized).await
    }

    async fn load_remote_youtube(&self, url: &str) -> Result<()> {
        let canonical_url = normalize_youtube_url(url)
            .ok_or_else(|| anyhow::anyhow!("não foi possível extrair o video_id da URL: {url}"))?;
        let video_id = extract_video_id(&canonical_url)
            .ok_or_else(|| anyhow::anyhow!("não foi possível extrair o video_id da URL: {url}"))?;

        self.emit(PlayerEvent::LoadingStatus(
            "Buscando metadados da música...".to_string(),
        ));
        let music = self.resolve_music(&canonical_url).await?;
        let music_id = music.id.clone();

        self.set_status(PlaybackState::Loading).await;

        self.emit(PlayerEvent::LoadingStatus(
            "Buscando legendas sincronizadas...".to_string(),
        ));
        let lyrics_service = self.lyrics_service.clone();
        let mut lyrics = lyrics_service
            .get_lyrics(&music_id, &canonical_url, None, &|status| {
                self.emit(PlayerEvent::LoadingStatus(status.to_string()));
            })
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("falha ao obter as letras da música {video_id}: {e}");
                Vec::new()
            });

        let local_path = if !lyrics.is_empty() {
            self.ensure_cached_audio(&canonical_url, &video_id).await?
        } else {
            let local_path = self.ensure_cached_audio(&canonical_url, &video_id).await?;

            self.emit(PlayerEvent::LoadingStatus(
                "Buscando legendas sincronizadas...".to_string(),
            ));
            lyrics = lyrics_service
                .get_lyrics(&music_id, &canonical_url, Some(&local_path), &|status| {
                    self.emit(PlayerEvent::LoadingStatus(status.to_string()));
                })
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!("falha ao obter as letras da música {video_id}: {e}");
                    Vec::new()
                });

            local_path
        };

        self.audio_engine.load(&local_path.to_string_lossy())?;
        self.audio_engine.pause()?;

        {
            let mut state = self.state.write().await;
            state.current_music = Some(music.clone());
            state.current_lyrics = lyrics.clone();
            state.status = PlaybackState::Paused;
        }

        self.emit(PlayerEvent::MusicLoaded { music, lyrics });
        self.emit(PlayerEvent::StateChanged(PlaybackState::Paused));

        Ok(())
    }

    async fn load_local_media(&self, local_path: &Path) -> Result<()> {
        let music = self.resolve_local_music(local_path).await?;
        let music_id = music.id.clone();
        let canonical_path = PathBuf::from(&music.youtube_url);

        self.set_status(PlaybackState::Loading).await;

        self.emit(PlayerEvent::LoadingStatus(
            "Buscando letras do arquivo local...".to_string(),
        ));
        let lyrics_service = self.lyrics_service.clone();
        let lyrics = lyrics_service
            .get_lyrics(
                &music_id,
                &music.youtube_url,
                Some(&canonical_path),
                &|status| {
                    self.emit(PlayerEvent::LoadingStatus(status.to_string()));
                },
            )
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "falha ao obter as letras do arquivo local {}: {e}",
                    canonical_path.display()
                );
                Vec::new()
            });

        self.audio_engine.load(&canonical_path.to_string_lossy())?;
        self.audio_engine.pause()?;

        {
            let mut state = self.state.write().await;
            state.current_music = Some(music.clone());
            state.current_lyrics = lyrics.clone();
            state.status = PlaybackState::Paused;
        }

        self.emit(PlayerEvent::MusicLoaded { music, lyrics });
        self.emit(PlayerEvent::StateChanged(PlaybackState::Paused));

        Ok(())
    }

    /// Retoma a reprodução.
    pub async fn play(&self) -> Result<()> {
        self.audio_engine.play()?;
        self.set_status(PlaybackState::Playing).await;
        Ok(())
    }

    /// Pausa a reprodução.
    pub async fn pause(&self) -> Result<()> {
        self.audio_engine.pause()?;
        self.set_status(PlaybackState::Paused).await;
        Ok(())
    }

    /// Interrompe a reprodução e limpa os dados da mídia ativa.
    pub async fn stop(&self) -> Result<()> {
        self.audio_engine.stop()?;

        {
            let mut state = self.state.write().await;
            state.current_music = None;
            state.current_lyrics = Vec::new();
            state.status = PlaybackState::Stopped;
        }

        self.emit(PlayerEvent::StateChanged(PlaybackState::Stopped));
        Ok(())
    }

    /// Move a reprodução para a posição absoluta em segundos.
    pub async fn seek(&self, seconds: f64) -> Result<()> {
        self.audio_engine.seek(seconds)
    }

    /// Move a reprodução em `delta` segundos relativos à posição atual.
    pub async fn seek_relative(&self, delta: f64) -> Result<()> {
        self.audio_engine.seek_relative(delta)
    }

    /// Ajusta o volume da reprodução em porcentagem.
    pub async fn set_volume(&self, volume: i64) -> Result<()> {
        self.audio_engine.set_volume(volume)
    }

    /// Atualiza e persiste o offset de sincronismo da música ativa.
    pub async fn update_sync_offset(&self, offset: f64) -> Result<()> {
        let music_id = {
            let mut state = self.state.write().await;
            let Some(music) = state.current_music.as_mut() else {
                anyhow::bail!("nenhuma música ativa para atualizar o sync_offset");
            };
            music.sync_offset = offset;
            music.id.clone()
        };

        let repository = MusicRepository::new(&self.pool);
        repository.update_sync_offset(&music_id, offset).await
    }

    /// Remove do cache local as letras associadas à mídia da `youtube_url`.
    pub async fn clear_lyrics_cache(&self, youtube_url: &str) -> Result<()> {
        let music_id = media_music_id(youtube_url).await?;

        self.lyrics_service.clear_cache(&music_id).await
    }

    /// Atualiza o texto de uma linha de letra da música ativa.
    pub async fn update_lyrics_line(&self, id: i64, new_text: &str) -> Result<()> {
        {
            let state = self.state.read().await;
            if state.current_music.is_none() {
                anyhow::bail!("nenhuma música ativa para edição de letra");
            }
        }

        self.lyrics_service.update_text(id, new_text).await?;

        let lyrics = {
            let mut state = self.state.write().await;
            let line = state
                .current_lyrics
                .iter_mut()
                .find(|line| line.id == id)
                .ok_or_else(|| {
                    anyhow::anyhow!("linha de letra inexistente no estado ativo: {id}")
                })?;
            line.text = new_text.to_string();
            state.current_lyrics.clone()
        };

        self.emit(PlayerEvent::LyricsUpdated(lyrics));
        Ok(())
    }

    /// Substitui as letras da música ativa e reemite o estado atualizado.
    pub async fn replace_current_lyrics(
        &self,
        music_id: &str,
        lyrics: Vec<LyricsLine>,
    ) -> Result<bool> {
        let updated_lyrics = {
            let mut state = self.state.write().await;
            let Some(current_music) = state.current_music.as_ref() else {
                return Ok(false);
            };
            if current_music.id != music_id {
                return Ok(false);
            }

            state.current_lyrics = lyrics;
            state.current_lyrics.clone()
        };

        self.emit(PlayerEvent::LyricsUpdated(updated_lyrics));
        Ok(true)
    }

    /// Garante que o áudio da mídia esteja disponível localmente no cache.
    ///
    /// Retorna o caminho do arquivo local. Se já existir, é um *cache hit* e o
    /// arquivo é reutilizado; caso contrário, dispara o download via yt-dlp.
    /// Em falha de download, reverte o estado para `Stopped` e propaga o erro.
    async fn ensure_cached_audio(&self, url: &str, video_id: &str) -> Result<std::path::PathBuf> {
        let cache_dir = self.cache_path.as_path();

        self.emit(PlayerEvent::LoadingStatus(
            "Verificando arquivos de áudio locais...".to_string(),
        ));

        tokio::fs::create_dir_all(cache_dir)
            .await
            .with_context(|| {
                format!(
                    "falha ao criar o diretório de cache {}",
                    cache_dir.display()
                )
            })?;

        if let Some(local_path) = find_cached_audio(cache_dir, video_id).await? {
            tracing::info!(
                "cache hit: reproduzindo {video_id} a partir de {}",
                local_path.display()
            );
            return Ok(local_path);
        }

        tracing::info!("cache miss: baixando áudio de {video_id}");

        self.emit(PlayerEvent::LoadingStatus(
            "Baixando áudio do YouTube (isso pode demorar)...".to_string(),
        ));

        let output_template = cache_dir.join(format!("{video_id}.%(ext)s"));

        if let Err(e) = self
            .youtube_service
            .download_audio(url, &output_template)
            .await
        {
            self.set_status(PlaybackState::Stopped).await;
            return Err(e);
        }

        find_cached_audio(cache_dir, video_id)
            .await?
            .with_context(|| {
                format!(
                    "yt-dlp concluiu o download, mas o arquivo não foi encontrado no cache para {video_id}"
                )
            })
    }

    /// Obtém a música do cache local ou, na ausência, dos metadados do YouTube.
    pub async fn resolve_music(&self, url: &str) -> Result<Music> {
        let repository = MusicRepository::new(&self.pool);

        if let Some(music) = repository.find_by_youtube_url(url).await? {
            return Ok(music);
        }

        let music = self.youtube_service.fetch_metadata(url).await?;

        if let Err(e) = repository.save(&music).await {
            tracing::warn!("falha ao salvar a música {} no cache: {e}", music.id);
        }

        Ok(music)
    }

    async fn resolve_local_music(&self, canonical_path: &Path) -> Result<Music> {
        let repository = MusicRepository::new(&self.pool);
        let path_string = canonical_path.to_string_lossy().to_string();

        if let Some(music) = repository.find_by_youtube_url(&path_string).await? {
            return Ok(music);
        }

        let title = canonical_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Arquivo Local")
            .to_string();

        let music = Music {
            id: local_music_id(canonical_path),
            title,
            artist: Some(LOCAL_ARTIST_NAME.to_string()),
            youtube_url: path_string,
            duration: None,
            thumbnail: None,
            created_at: None,
            sync_offset: 0.0,
            has_lyrics: None,
        };

        if let Err(e) = repository.save(&music).await {
            tracing::warn!(
                "falha ao salvar o arquivo local {} no cache: {e}",
                canonical_path.display()
            );
        }

        Ok(music)
    }

    /// Atualiza o status e propaga `StateChanged`.
    async fn set_status(&self, status: PlaybackState) {
        self.state.write().await.status = status;
        self.emit(PlayerEvent::StateChanged(status));
    }

    /// Propaga um evento, ignorando a ausência de assinantes.
    fn emit(&self, event: PlayerEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Inicia o loop de polling em segundo plano.
    fn spawn_poll_loop(player: &Arc<Self>) {
        let player = Arc::clone(player);

        tokio::spawn(async move {
            let mut last_position = f64::NAN;

            loop {
                tokio::time::sleep(POLL_INTERVAL).await;

                let position = player.audio_engine.position();
                let duration = player.audio_engine.duration();
                let is_idle = player.audio_engine.is_idle();

                let status = player.state.read().await.status;

                if status == PlaybackState::Playing && is_idle {
                    {
                        let mut state = player.state.write().await;
                        state.status = PlaybackState::Stopped;
                    }
                    player.emit(PlayerEvent::PlaybackFinished);
                    player.emit(PlayerEvent::StateChanged(PlaybackState::Stopped));
                    last_position = f64::NAN;
                    continue;
                }

                if should_emit_position(last_position, position) {
                    last_position = position;
                    player.emit(PlayerEvent::PositionUpdated { position, duration });
                }
            }
        });
    }
}

/// Decide se a variação de posição justifica a emissão de `PositionUpdated`.
fn should_emit_position(last: f64, current: f64) -> bool {
    if last.is_nan() {
        return true;
    }
    (current - last).abs() >= POSITION_EPSILON
}

async fn find_cached_audio(cache_dir: &Path, video_id: &str) -> Result<Option<PathBuf>> {
    for extension in CACHED_AUDIO_EXTENSIONS {
        let candidate = cache_dir.join(format!("{video_id}.{extension}"));
        if tokio::fs::try_exists(&candidate).await.with_context(|| {
            format!(
                "falha ao verificar o cache de áudio {}",
                candidate.display()
            )
        })? {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::lyrics_repository::LyricsRepository;
    use crate::infrastructure::music_repository::MusicRepository;
    use crate::infrastructure::whisper::WhisperService;

    // ----- Testes das funções puras (independentes de mpv/yt-dlp). -----

    #[test]
    fn should_emit_position_on_first_read() {
        assert!(should_emit_position(f64::NAN, 0.0));
    }

    #[test]
    fn should_emit_position_when_change_is_significant() {
        assert!(should_emit_position(1.0, 1.05));
    }

    #[test]
    fn should_not_emit_position_when_change_is_negligible() {
        assert!(!should_emit_position(1.0, 1.005));
    }

    // ----- Testes de integração das transições de estado. -----
    //
    // Dependem do `AudioEngine` (libmpv). Quando o libmpv não está disponível
    // no ambiente, o teste é encerrado graciosamente sem falhar.

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

    fn write_silent_wav(path: &std::path::Path) -> std::io::Result<()> {
        let sample_rate: u32 = 16_000;
        let num_samples: u32 = sample_rate / 10;
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

        std::fs::write(path, bytes)
    }

    fn build_player(pool: SqlitePool) -> Option<Arc<Player>> {
        let audio_engine = match AudioEngine::new() {
            Ok(engine) => Arc::new(engine),
            Err(_) => return None,
        };
        let youtube = Arc::new(YoutubeService::new());
        let whisper = Arc::new(WhisperService::new());
        let settings = match crate::shared::config::load_settings() {
            Ok(settings) => settings,
            Err(_) => return None,
        };
        let cache_path = PathBuf::from(settings.cache_path.clone());
        let lyrics = Arc::new(LyricsService::new(
            pool.clone(),
            Arc::clone(&youtube),
            Arc::clone(&whisper),
            cache_path.clone(),
        ));
        Some(Player::new(
            audio_engine,
            youtube,
            lyrics,
            pool,
            cache_path,
            crate::domain::settings::Settings::default().volume as i64,
        ))
    }

    #[test]
    fn decode_file_uri_path_unescapes_spaces() {
        assert_eq!(
            decode_file_uri_path("/tmp/Meu%20Arquivo.wav"),
            "/tmp/Meu Arquivo.wav"
        );
    }

    #[test]
    fn local_music_id_depends_on_absolute_path() {
        let path_a = Path::new("/tmp/letras_sync_a.wav");
        let path_b = Path::new("/tmp/letras_sync_b.wav");

        assert_eq!(local_music_id(path_a), local_music_id(path_a));
        assert_ne!(local_music_id(path_a), local_music_id(path_b));
    }

    #[tokio::test]
    async fn resolve_local_media_path_accepts_file_uri_and_canonicalizes() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("letras_sync_local_uri_test.wav");
        if write_silent_wav(&path).is_err() {
            return;
        }

        let uri = format!("file://{}", path.display());
        let resolved = resolve_local_media_path(&uri)
            .await
            .expect("resolver file uri");

        let canonical = std::fs::canonicalize(&path).expect("canonical");
        let _ = std::fs::remove_file(&path);

        assert_eq!(resolved, Some(canonical));
    }

    #[test]
    fn music_loaded_event_preserves_lyrics_payload() {
        let music = Music {
            id: "m1".to_string(),
            title: "Título".to_string(),
            artist: Some("Artista".to_string()),
            youtube_url: "https://youtu.be/m1".to_string(),
            duration: Some(180),
            thumbnail: None,
            created_at: None,
            sync_offset: 0.0,
            has_lyrics: Some(true),
        };
        let lyrics = vec![
            LyricsLine {
                id: 1,
                music_id: "m1".to_string(),
                start_time: 0.0,
                end_time: 1.0,
                text: "linha 1".to_string(),
            },
            LyricsLine {
                id: 2,
                music_id: "m1".to_string(),
                start_time: 1.0,
                end_time: 2.0,
                text: "linha 2".to_string(),
            },
        ];

        let event = PlayerEvent::MusicLoaded {
            music,
            lyrics: lyrics.clone(),
        };

        match event {
            PlayerEvent::MusicLoaded {
                lyrics: payload, ..
            } => {
                assert_eq!(payload.len(), lyrics.len());
                for (left, right) in payload.iter().zip(lyrics.iter()) {
                    assert_eq!(left.id, right.id);
                    assert_eq!(left.music_id, right.music_id);
                    assert_eq!(left.start_time, right.start_time);
                    assert_eq!(left.end_time, right.end_time);
                    assert_eq!(left.text, right.text);
                }
            }
            _ => panic!("evento inesperado"),
        }
    }

    #[tokio::test]
    async fn initial_status_is_idle() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool) else {
            return;
        };
        assert_eq!(player.state.read().await.status, PlaybackState::Idle);
    }

    #[tokio::test]
    async fn pause_transitions_to_paused_and_emits_event() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool) else {
            return;
        };
        let mut rx = player.subscribe();

        player.pause().await.expect("pause");

        assert_eq!(player.state.read().await.status, PlaybackState::Paused);
        let event = rx.recv().await.expect("evento");
        assert!(matches!(
            event,
            PlayerEvent::StateChanged(PlaybackState::Paused)
        ));
    }

    #[tokio::test]
    async fn play_transitions_to_playing() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool) else {
            return;
        };

        player.play().await.expect("play");

        assert_eq!(player.state.read().await.status, PlaybackState::Playing);
    }

    #[tokio::test]
    async fn seek_relative_returns_error_without_media() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool) else {
            return;
        };

        assert!(player.seek_relative(10.0).await.is_err());
    }

    #[tokio::test]
    async fn stop_clears_state_and_transitions_to_stopped() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool) else {
            return;
        };
        let mut rx = player.subscribe();

        player.stop().await.expect("stop");

        assert_eq!(player.state.read().await.status, PlaybackState::Stopped);
        {
            let state = player.state.read().await;
            assert!(state.current_music.is_none());
            assert!(state.current_lyrics.is_empty());
        }
        let event = rx.recv().await.expect("evento");
        assert!(matches!(
            event,
            PlayerEvent::StateChanged(PlaybackState::Stopped)
        ));
    }

    #[tokio::test]
    async fn update_lyrics_line_updates_state_and_emits_event() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool.clone()) else {
            return;
        };

        let music = Music {
            id: "m1".to_string(),
            title: "Título".to_string(),
            artist: Some("Artista".to_string()),
            youtube_url: "https://youtu.be/m1".to_string(),
            duration: Some(120),
            thumbnail: None,
            created_at: Some("2024-01-01 00:00:00".to_string()),
            sync_offset: 0.0,
            has_lyrics: Some(true),
        };

        MusicRepository::new(&pool)
            .save(&music)
            .await
            .expect("save music");
        LyricsRepository::new(&pool)
            .save_all(&[
                LyricsLine {
                    id: 1,
                    music_id: music.id.clone(),
                    start_time: 0.0,
                    end_time: 1.0,
                    text: "linha original".to_string(),
                },
                LyricsLine {
                    id: 2,
                    music_id: music.id.clone(),
                    start_time: 1.0,
                    end_time: 2.0,
                    text: "linha seguinte".to_string(),
                },
            ])
            .await
            .expect("save lyrics");

        {
            let mut state = player.state.write().await;
            state.current_music = Some(music);
            state.current_lyrics = vec![
                LyricsLine {
                    id: 1,
                    music_id: "m1".to_string(),
                    start_time: 0.0,
                    end_time: 1.0,
                    text: "linha original".to_string(),
                },
                LyricsLine {
                    id: 2,
                    music_id: "m1".to_string(),
                    start_time: 1.0,
                    end_time: 2.0,
                    text: "linha seguinte".to_string(),
                },
            ];
        }

        let mut rx = player.subscribe();

        player
            .update_lyrics_line(1, "linha corrigida")
            .await
            .expect("update lyrics line");

        let state = player.state.read().await;
        assert_eq!(state.current_lyrics[0].text, "linha corrigida");

        let event = rx.recv().await.expect("evento");
        match event {
            PlayerEvent::LyricsUpdated(lyrics) => {
                assert_eq!(lyrics[0].text, "linha corrigida");
                assert_eq!(lyrics[1].text, "linha seguinte");
            }
            other => panic!("evento inesperado: {other:?}"),
        }
    }

    #[tokio::test]
    async fn update_sync_offset_persists_active_music_offset() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool.clone()) else {
            return;
        };

        let music = Music {
            id: "offset-music".to_string(),
            title: "Título".to_string(),
            artist: Some("Artista".to_string()),
            youtube_url: "https://youtu.be/offset-music".to_string(),
            duration: Some(120),
            thumbnail: None,
            created_at: Some("2024-01-01 00:00:00".to_string()),
            sync_offset: 0.0,
            has_lyrics: Some(true),
        };

        MusicRepository::new(&pool)
            .save(&music)
            .await
            .expect("save music");

        {
            let mut state = player.state.write().await;
            state.current_music = Some(music.clone());
        }

        player
            .update_sync_offset(2.25)
            .await
            .expect("update sync offset");

        {
            let state = player.state.read().await;
            let current = state.current_music.as_ref().expect("current music");
            assert_eq!(current.sync_offset, 2.25);
        }

        let stored = MusicRepository::new(&pool)
            .find_by_youtube_url(&music.youtube_url)
            .await
            .expect("find music")
            .expect("music exists");

        assert_eq!(stored.sync_offset, 2.25);
    }

    // ----- Testes do cache de áudio local. -----
    //
    // Dependem do `AudioEngine` (libmpv). Quando o libmpv não está disponível
    // no ambiente, o teste é encerrado graciosamente sem falhar.

    #[tokio::test]
    async fn ensure_cached_audio_returns_local_path_on_cache_hit() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool) else {
            return;
        };

        let settings = match crate::shared::config::load_settings() {
            Ok(settings) => settings,
            Err(_) => return,
        };
        let cache_dir = std::path::Path::new(&settings.cache_path);
        if tokio::fs::create_dir_all(cache_dir).await.is_err() {
            return;
        }

        let video_id = "cache_hit_test_id";
        let local_path = cache_dir.join(format!("{video_id}.mp3"));
        tokio::fs::write(&local_path, b"fake")
            .await
            .expect("mp3 fake");

        let result = player
            .ensure_cached_audio("https://youtu.be/cache_hit_test_id", video_id)
            .await
            .expect("cache hit não deve falhar");

        assert_eq!(result, local_path);

        let _ = tokio::fs::remove_file(&local_path).await;
    }

    #[tokio::test]
    async fn find_cached_audio_accepts_non_mp3_extensions() {
        let temp_dir = std::env::temp_dir().join("letras_sync_cache_variant_test");
        if tokio::fs::create_dir_all(&temp_dir).await.is_err() {
            return;
        }

        let video_id = "cache_variant_test_id";
        let candidate = temp_dir.join(format!("{video_id}.webm"));
        if tokio::fs::write(&candidate, b"fake").await.is_err() {
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return;
        }

        let found = find_cached_audio(&temp_dir, video_id)
            .await
            .expect("buscar cache variant");

        assert_eq!(found, Some(candidate));

        let _ = tokio::fs::remove_file(temp_dir.join(format!("{video_id}.webm"))).await;
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn load_youtube_with_invalid_url_returns_error() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool) else {
            return;
        };

        assert!(player.load_youtube("not-a-url").await.is_err());
    }

    #[tokio::test]
    async fn load_local_media_saves_music_and_uses_local_metadata() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool.clone()) else {
            return;
        };

        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("letras_sync_local_load_test.wav");
        if write_silent_wav(&path).is_err() {
            return;
        }

        let uri = format!("file://{}", path.display());
        let canonical = std::fs::canonicalize(&path).expect("canonical path");
        let expected_id = local_music_id(&canonical);
        let music = Music {
            id: expected_id.clone(),
            title: "letras_sync_local_load_test".to_string(),
            artist: Some(LOCAL_ARTIST_NAME.to_string()),
            youtube_url: canonical.to_string_lossy().to_string(),
            duration: None,
            thumbnail: None,
            created_at: None,
            sync_offset: 1.25,
            has_lyrics: None,
        };

        MusicRepository::new(&pool)
            .save(&music)
            .await
            .expect("seed local music");

        LyricsRepository::new(&pool)
            .save_all(&[LyricsLine {
                id: 0,
                music_id: expected_id.clone(),
                start_time: 0.0,
                end_time: 1.0,
                text: "linha local".to_string(),
            }])
            .await
            .expect("seed local lyrics");

        player.load_youtube(&uri).await.expect("load local file");

        let repository = MusicRepository::new(&pool);
        let stored = repository
            .find_by_youtube_url(&canonical.to_string_lossy())
            .await
            .expect("find local music")
            .expect("local music saved");

        assert_eq!(stored.id, expected_id);
        assert_eq!(stored.title, "letras_sync_local_load_test");
        assert_eq!(stored.artist.as_deref(), Some(LOCAL_ARTIST_NAME));

        {
            let state = player.state.read().await;
            let current = state.current_music.as_ref().expect("current music");
            assert_eq!(current.id, expected_id);
            assert_eq!(current.title, "letras_sync_local_load_test");
            assert_eq!(current.artist.as_deref(), Some(LOCAL_ARTIST_NAME));
            assert_eq!(current.sync_offset, 1.25);
            assert_eq!(state.current_lyrics.len(), 1);
            assert_eq!(state.current_lyrics[0].text, "linha local");
        }

        let _ = std::fs::remove_file(&path);
    }
}
