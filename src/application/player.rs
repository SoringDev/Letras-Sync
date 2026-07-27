use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::sqlite::SqlitePool;
use tokio::sync::{RwLock, broadcast};

use crate::domain::lyrics::LyricsLine;
use crate::domain::music::Music;
use crate::infrastructure::audio::AudioEngine;
use crate::infrastructure::music_repository::MusicRepository;
use crate::infrastructure::youtube::YoutubeService;
use crate::shared::utils::extract_video_id;

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
        volume: i64,
    ) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);

        let player = Arc::new(Self {
            audio_engine,
            youtube_service,
            lyrics_service,
            pool,
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

    /// Carrega uma mídia do YouTube e inicia a reprodução.
    pub async fn load_youtube(&self, url: &str) -> Result<()> {
        let video_id = extract_video_id(url)
            .ok_or_else(|| anyhow::anyhow!("não foi possível extrair o video_id da URL: {url}"))?;

        self.emit(PlayerEvent::LoadingStatus(
            "Buscando metadados da música...".to_string(),
        ));
        let music = self.resolve_music(url).await?;
        let music_id = music.id.clone();

        self.set_status(PlaybackState::Loading).await;

        let local_path = self.ensure_cached_audio(url, &video_id).await?;

        self.emit(PlayerEvent::LoadingStatus(
            "Buscando legendas sincronizadas...".to_string(),
        ));
        let lyrics_service = self.lyrics_service.clone();
        let lyrics = lyrics_service
            .get_lyrics(&music_id, url, Some(&local_path), &|status| {
                self.emit(PlayerEvent::LoadingStatus(status.to_string()));
            })
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

    /// Move a reprodução em `delta` segundos relativos à posição atual.
    pub async fn seek_relative(&self, delta: f64) -> Result<()> {
        self.audio_engine.seek_relative(delta)
    }

    /// Ajusta o volume da reprodução em porcentagem.
    pub async fn set_volume(&self, volume: i64) -> Result<()> {
        self.audio_engine.set_volume(volume)
    }

    /// Remove do cache local as letras associadas à mídia da `youtube_url`.
    pub async fn clear_lyrics_cache(&self, youtube_url: &str) -> Result<()> {
        let music_id = extract_video_id(youtube_url).ok_or_else(|| {
            anyhow::anyhow!("não foi possível extrair o video_id da URL: {youtube_url}")
        })?;

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
                .ok_or_else(|| anyhow::anyhow!("linha de letra inexistente no estado ativo: {id}"))?;
            line.text = new_text.to_string();
            state.current_lyrics.clone()
        };

        self.emit(PlayerEvent::LyricsUpdated(lyrics));
        Ok(())
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

        self.emit(PlayerEvent::LoadingStatus(
            "Baixando áudio do YouTube (isso pode demorar)...".to_string(),
        ));

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
        let settings = crate::domain::settings::Settings::default();
        Some(Player::new(
            audio_engine,
            youtube,
            lyrics,
            pool,
            settings.volume as i64,
        ))
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

    #[tokio::test]
    async fn load_youtube_with_invalid_url_returns_error() {
        let pool = memory_pool().await;
        let Some(player) = build_player(pool) else {
            return;
        };

        assert!(player.load_youtube("not-a-url").await.is_err());
    }
}
