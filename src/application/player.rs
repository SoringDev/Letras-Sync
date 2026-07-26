use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::sqlite::SqlitePool;
use tokio::sync::{RwLock, broadcast};

use crate::domain::lyrics::LyricsLine;
use crate::domain::music::Music;
use crate::infrastructure::audio::AudioEngine;
use crate::infrastructure::music_repository::MusicRepository;
use crate::infrastructure::whisper::WhisperService;
use crate::infrastructure::youtube::YoutubeService;

use super::lyrics_service::LyricsService;

/// Intervalo do loop de polling em segundo plano.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Variação mínima de posição (em segundos) que dispara `PositionUpdated`.
const POSITION_EPSILON: f64 = 0.01;

/// Capacidade do canal de eventos reativos do player.
const EVENT_CHANNEL_CAPACITY: usize = 64;

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
    PositionUpdated {
        position: f64,
        duration: Option<f64>,
    },
    PlaybackFinished,
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

/// Serviço de controle e orquestração da reprodução.
///
/// Integra o `AudioEngine`, o `YoutubeService`, o `LyricsService` e os
/// repositórios para gerenciar o estado da reprodução de forma segura para
/// concorrência e notificar os consumidores via canal `broadcast`.
pub struct Player {
    audio_engine: Arc<AudioEngine>,
    youtube_service: Arc<YoutubeService>,
    lyrics_service: Arc<LyricsService>,
    whisper_service: Arc<WhisperService>,
    pool: SqlitePool,
    state: Arc<RwLock<PlayerState>>,
    event_tx: broadcast::Sender<PlayerEvent>,
}

impl Player {
    /// Inicializa o player e inicia a tarefa de polling em segundo plano.
    pub fn new(
        audio_engine: Arc<AudioEngine>,
        youtube_service: Arc<YoutubeService>,
        lyrics_service: Arc<LyricsService>,
        whisper_service: Arc<WhisperService>,
        pool: SqlitePool,
    ) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);

        let player = Arc::new(Self {
            audio_engine,
            youtube_service,
            lyrics_service,
            whisper_service,
            pool,
            state: Arc::new(RwLock::new(PlayerState::default())),
            event_tx,
        });

        Self::spawn_poll_loop(&player);

        player
    }

    /// Assina o canal de eventos reativos do player.
    pub fn subscribe(&self) -> broadcast::Receiver<PlayerEvent> {
        self.event_tx.subscribe()
    }

    /// Retorna o status atual da reprodução.
    pub async fn status(&self) -> PlaybackState {
        self.state.read().await.status
    }

    /// Carrega uma mídia do YouTube e inicia a reprodução.
    pub async fn load_youtube(&self, url: &str) -> Result<()> {
        let video_id = extract_video_id(url)
            .ok_or_else(|| anyhow::anyhow!("não foi possível extrair o video_id da URL: {url}"))?;

        let music = self.resolve_music(url).await?;
        let music_id = music.id.clone();

        self.set_status(PlaybackState::Loading).await;

        let local_path = self.ensure_cached_audio(url, &video_id).await?;

        let lyrics = self
            .lyrics_service
            .get_lyrics(&music_id, url, Some(&local_path))
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("falha ao obter as letras da música {video_id}: {e}");
                Vec::new()
            });

        self.audio_engine.load(&local_path.to_string_lossy())?;

        {
            let mut state = self.state.write().await;
            state.current_music = Some(music.clone());
            state.current_lyrics = lyrics.clone();
            state.status = PlaybackState::Playing;
        }

        self.emit(PlayerEvent::MusicLoaded { music, lyrics });
        self.emit(PlayerEvent::StateChanged(PlaybackState::Playing));

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

    /// Garante que o áudio da mídia esteja disponível localmente no cache.
    ///
    /// Retorna o caminho do arquivo local. Se já existir, é um *cache hit* e o
    /// arquivo é reutilizado; caso contrário, dispara o download via yt-dlp.
    /// Em falha de download, reverte o estado para `Stopped` e propaga o erro.
    async fn ensure_cached_audio(
        &self,
        url: &str,
        video_id: &str,
    ) -> Result<std::path::PathBuf> {
        let settings = crate::shared::config::load_settings()?;
        let cache_dir = std::path::Path::new(&settings.cache_path);

        tokio::fs::create_dir_all(cache_dir)
            .await
            .with_context(|| {
                format!(
                    "falha ao criar o diretório de cache {}",
                    cache_dir.display()
                )
            })?;

        let local_path = cache_dir.join(format!("{video_id}.mp3"));

        let exists = tokio::fs::try_exists(&local_path).await.with_context(|| {
            format!(
                "falha ao verificar o cache de áudio {}",
                local_path.display()
            )
        })?;

        if exists {
            tracing::info!(
                "cache hit: reproduzindo {video_id} a partir de {}",
                local_path.display()
            );
            return Ok(local_path);
        }

        tracing::info!("cache miss: baixando áudio de {video_id}");

        if let Err(e) = self
            .youtube_service
            .download_audio(url, &local_path)
            .await
        {
            self.set_status(PlaybackState::Stopped).await;
            return Err(e);
        }

        Ok(local_path)
    }

    /// Obtém a música do cache local ou, na ausência, dos metadados do YouTube.
    async fn resolve_music(&self, url: &str) -> Result<Music> {
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

/// Extrai o `video_id` de uma URL do YouTube.
///
/// Suporta o parâmetro `v=` da query string e o formato curto `youtu.be/<id>`.
/// Retorna `None` se nenhum padrão for reconhecido.
fn extract_video_id(url: &str) -> Option<String> {
    if let Some(idx) = url.find("v=") {
        let rest = &url[idx + 2..];
        let id = rest.split('&').next().unwrap_or(rest);
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }

    if let Some(idx) = url.find("youtu.be/") {
        let rest = &url[idx + "youtu.be/".len()..];
        let id = rest.split(['?', '&', '/']).next().unwrap_or(rest);
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----- Testes das funções puras (independentes de mpv/yt-dlp). -----

    #[test]
    fn extract_video_id_from_query_param() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=abc123"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn extract_video_id_from_short_url() {
        assert_eq!(
            extract_video_id("https://youtu.be/abc123?t=10"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn extract_video_id_returns_none_for_unknown_url() {
        assert_eq!(extract_video_id("https://example.com/x"), None);
    }

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

    fn build_player(pool: SqlitePool) -> Option<Arc<Player>> {
        let audio_engine = match AudioEngine::new() {
            Ok(engine) => Arc::new(engine),
            Err(_) => return None,
        };
        let youtube = Arc::new(YoutubeService::new());
        let whisper = Arc::new(WhisperService::new());
        let lyrics = Arc::new(LyricsService::new(
            pool.clone(),
            Arc::clone(&youtube),
            Arc::clone(&whisper),
        ));
        Some(Player::new(audio_engine, youtube, lyrics, whisper, pool))
    }

    #[tokio::test]
    async fn initial_status_is_idle() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool) else {
            return;
        };
        assert_eq!(player.status().await, PlaybackState::Idle);
    }

    #[tokio::test]
    async fn pause_transitions_to_paused_and_emits_event() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool) else {
            return;
        };
        let mut rx = player.subscribe();

        player.pause().await.expect("pause");

        assert_eq!(player.status().await, PlaybackState::Paused);
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

        assert_eq!(player.status().await, PlaybackState::Playing);
    }

    #[tokio::test]
    async fn stop_clears_state_and_transitions_to_stopped() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool) else {
            return;
        };
        let mut rx = player.subscribe();

        player.stop().await.expect("stop");

        assert_eq!(player.status().await, PlaybackState::Stopped);
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
        tokio::fs::write(&local_path, b"fake").await.expect("mp3 fake");

        let result = player
            .ensure_cached_audio("https://youtu.be/cache_hit_test_id", video_id)
            .await
            .expect("cache hit não deve falhar");

        assert_eq!(result, local_path);

        let _ = tokio::fs::remove_file(&local_path).await;
    }
}
