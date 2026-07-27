mod application;
mod domain;
mod infrastructure;
mod presentation;
mod shared;

use std::sync::Arc;

use anyhow::Context;

use application::lyrics_service::LyricsService;
use application::player::Player;
use application::playlist::Playlist;
use application::timeline::Timeline;
use infrastructure::audio::AudioEngine;
use infrastructure::database::Database;
use infrastructure::whisper::WhisperService;
use infrastructure::youtube::YoutubeService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Letras Sync iniciado com sucesso.");

    let settings = match shared::config::load_settings() {
        Ok(settings) => {
            tracing::info!("Configurações carregadas: {settings:?}");
            match shared::config::config_file_path() {
                Ok(path) => tracing::info!("Arquivo de configuração: {}", path.display()),
                Err(err) => tracing::warn!(
                    "Não foi possível resolver o caminho do arquivo de configuração: {err:?}"
                ),
            }
            settings
        }
        Err(err) => {
            tracing::error!("Falha ao carregar as configurações: {err:?}");
            return Err(err);
        }
    };

    let database = Database::new().await?;
    tracing::info!("Banco de dados conectado e migrações aplicadas com sucesso.");
    let pool = database.pool().clone();

    let audio_engine = Arc::new(AudioEngine::new().context("falha ao inicializar o motor de áudio")?);
    let youtube = Arc::new(YoutubeService::new());
    let whisper = Arc::new(WhisperService::new());
    let lyrics = Arc::new(LyricsService::new(
        pool.clone(),
        Arc::clone(&youtube),
        Arc::clone(&whisper),
    ));

    let player = Player::new(audio_engine, youtube, lyrics, pool.clone());
    let timeline = Timeline::new(Arc::clone(&player));
    let playlist = Arc::new(Playlist::new());

    presentation::ui::run_operator_ui(player, timeline, playlist, pool, settings)?;

    Ok(())
}
