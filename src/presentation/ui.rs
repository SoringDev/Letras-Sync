use std::sync::Arc;

use qmetaobject::prelude::*;
use qmetaobject::{queued_callback, QObjectBox, QPointer, QVariantList, QVariantMap};
use sqlx::sqlite::SqlitePool;
use tokio::sync::broadcast;

use crate::application::player::{PlaybackState, Player, PlayerEvent};
use crate::application::playlist::Playlist;
use crate::application::timeline::{Timeline, TimelineEvent};
use crate::domain::music::Music;
use crate::domain::settings::Settings;
use crate::infrastructure::music_repository::MusicRepository;

qrc!(register_qml_resources,
    "letras_sync/presentation" {
        "src/presentation/operator.qml" as "operator.qml",
        "src/presentation/projection.qml" as "projection.qml",
    },
);

/// Verifica se há um servidor gráfico ativo (X11/Wayland).
fn has_display() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()
}

/// Converte o estado de reprodução em texto para consumo do QML.
fn playback_state_label(state: PlaybackState) -> &'static str {
    match state {
        PlaybackState::Idle => "Idle",
        PlaybackState::Loading => "Loading",
        PlaybackState::Playing => "Playing",
        PlaybackState::Paused => "Paused",
        PlaybackState::Stopped => "Stopped",
    }
}

/// Controlador central que faz a ponte entre o QML e os serviços do backend.
///
/// Expõe propriedades reativas e métodos ao QML e escuta os canais de
/// broadcast do `Player` e da `Timeline` em tarefas de segundo plano,
/// atualizando as propriedades na thread principal do Qt via
/// `queued_callback` + `QPointer`.
#[derive(QObject, Default)]
pub struct AppController {
    base: qt_base_class!(trait QObject),

    playback_state: qt_property!(QString; NOTIFY playback_state_changed),
    playback_state_changed: qt_signal!(),

    current_time: qt_property!(f64; NOTIFY current_time_changed),
    current_time_changed: qt_signal!(),

    duration: qt_property!(f64; NOTIFY duration_changed),
    duration_changed: qt_signal!(),

    sync_offset: qt_property!(f64; NOTIFY sync_offset_changed),
    sync_offset_changed: qt_signal!(),

    volume: qt_property!(i32; NOTIFY volume_changed),
    volume_changed: qt_signal!(),

    music_title: qt_property!(QString; NOTIFY music_title_changed),
    music_title_changed: qt_signal!(),

    music_artist: qt_property!(QString; NOTIFY music_artist_changed),
    music_artist_changed: qt_signal!(),

    lyric_text: qt_property!(QString; NOTIFY lyric_text_changed),
    lyric_text_changed: qt_signal!(),

    next_lyric_text: qt_property!(QString; NOTIFY next_lyric_text_changed),
    next_lyric_text_changed: qt_signal!(),

    clear_screen: qt_property!(bool; NOTIFY clear_screen_changed),
    clear_screen_changed: qt_signal!(),

    projection_visible: qt_property!(bool; NOTIFY projection_visible_changed),
    projection_visible_changed: qt_signal!(),

    loading: qt_property!(bool; NOTIFY loading_changed),
    loading_changed: qt_signal!(),

    error_message: qt_property!(QString; NOTIFY error_message_changed),
    error_message_changed: qt_signal!(),

    loading_status: qt_property!(QString; NOTIFY loading_status_changed),
    loading_status_changed: qt_signal!(),

    current_lyrics: qt_property!(QVariantList; NOTIFY current_lyrics_changed),
    current_lyrics_changed: qt_signal!(),

    active_line_id: qt_property!(i64; NOTIFY active_line_id_changed),
    active_line_id_changed: qt_signal!(),

    playlist: qt_property!(QVariantList; NOTIFY playlist_changed),
    playlist_changed: qt_signal!(),

    font_size: qt_property!(u32; NOTIFY style_changed),
    font_family: qt_property!(QString; NOTIFY style_changed),
    font_color: qt_property!(QString; NOTIFY style_changed),
    background_color: qt_property!(QString; NOTIFY style_changed),
    projector_screen_index: qt_property!(i32; NOTIFY style_changed),
    style_changed: qt_signal!(),

    history: qt_property!(QVariantList; NOTIFY history_changed),
    history_changed: qt_signal!(),

    history_search_query: qt_property!(QString; NOTIFY history_search_query_changed),
    history_search_query_changed: qt_signal!(),

    load_music: qt_method!(fn load_music(&mut self, url: QString) {
        self.load_url(url.to_string());
    }),

    add_to_playlist: qt_method!(fn add_to_playlist(&mut self, url: QString) {
        let Some(player) = self.player.clone() else {
            return;
        };
        let Some(playlist) = self.playlist_handle.clone() else {
            return;
        };

        let qptr = QPointer::from(&*self);
        let refresh = queued_callback(move |()| {
            if let Some(pinned) = qptr.as_pinned() {
                pinned.borrow().spawn_playlist_refresh();
            }
        });

        let url = url.to_string();
        tokio::spawn(async move {
            match player.resolve_music(&url).await {
                Ok(music) => {
                    playlist.add(music).await;
                    refresh(());
                }
                Err(err) => {
                    tracing::error!("falha ao resolver a música para a playlist {url}: {err:?}")
                }
            }
        });
    }),

    remove_from_playlist: qt_method!(fn remove_from_playlist(&mut self, index: i32) {
        let Some(playlist) = self.playlist_handle.clone() else {
            return;
        };
        if index < 0 {
            return;
        }
        let index = index as usize;

        let qptr = QPointer::from(&*self);
        let refresh = queued_callback(move |()| {
            if let Some(pinned) = qptr.as_pinned() {
                pinned.borrow().spawn_playlist_refresh();
            }
        });

        tokio::spawn(async move {
            playlist.remove(index).await;
            refresh(());
        });
    }),

    play_playlist_item: qt_method!(fn play_playlist_item(&mut self, index: i32) {
        let Some(playlist) = self.playlist_handle.clone() else {
            return;
        };
        if index < 0 {
            return;
        }
        let index = index as usize;

        let qptr = QPointer::from(&*self);
        let start = queued_callback(move |url: String| {
            if let Some(pinned) = qptr.as_pinned() {
                pinned.borrow_mut().load_url(url);
            }
        });

        tokio::spawn(async move {
            playlist.set_current_index(Some(index)).await;
            if let Some(music) = playlist.current_music().await {
                start(music.youtube_url);
            }
        });
    }),

    play_next: qt_method!(fn play_next(&mut self) {
        let Some(playlist) = self.playlist_handle.clone() else {
            return;
        };

        let qptr = QPointer::from(&*self);
        let start = queued_callback(move |url: String| {
            if let Some(pinned) = qptr.as_pinned() {
                pinned.borrow_mut().load_url(url);
            }
        });

        tokio::spawn(async move {
            if let Some(music) = playlist.next().await {
                start(music.youtube_url);
            }
        });
    }),

    play_previous: qt_method!(fn play_previous(&mut self) {
        let Some(playlist) = self.playlist_handle.clone() else {
            return;
        };

        let qptr = QPointer::from(&*self);
        let start = queued_callback(move |url: String| {
            if let Some(pinned) = qptr.as_pinned() {
                pinned.borrow_mut().load_url(url);
            }
        });

        tokio::spawn(async move {
            if let Some(music) = playlist.prev().await {
                start(music.youtube_url);
            }
        });
    }),

    refresh_history: qt_method!(fn refresh_history(&self) {
        self.spawn_history_refresh();
    }),

    set_history_search_query: qt_method!(fn set_history_search_query(&mut self, query: QString) {
        self.history_search_query = query;
        self.history_search_query_changed();
        self.spawn_history_refresh();
    }),

    play: qt_method!(fn play(&self) {
        let Some(player) = self.player.clone() else {
            return;
        };
        tokio::spawn(async move {
            if let Err(err) = player.play().await {
                tracing::error!("falha ao retomar a reprodução: {err:?}");
            }
        });
    }),

    pause: qt_method!(fn pause(&self) {
        let Some(player) = self.player.clone() else {
            return;
        };
        tokio::spawn(async move {
            if let Err(err) = player.pause().await {
                tracing::error!("falha ao pausar a reprodução: {err:?}");
            }
        });
    }),

    stop: qt_method!(fn stop(&self) {
        let Some(player) = self.player.clone() else {
            return;
        };
        tokio::spawn(async move {
            if let Err(err) = player.stop().await {
                tracing::error!("falha ao interromper a reprodução: {err:?}");
            }
        });
    }),

    seek: qt_method!(fn seek(&self, seconds: f64) {
        let Some(player) = self.player.clone() else {
            return;
        };
        tokio::spawn(async move {
            if let Err(err) = player.seek(seconds).await {
                tracing::error!("falha ao reposicionar a reprodução: {err:?}");
            }
        });
    }),

    seek_relative: qt_method!(fn seek_relative(&self, delta: f64) {
        let Some(player) = self.player.clone() else {
            return;
        };
        tokio::spawn(async move {
            if let Err(err) = player.seek_relative(delta).await {
                tracing::error!("falha ao reposicionar a reprodução: {err:?}");
            }
        });
    }),

    set_volume: qt_method!(fn set_volume(&mut self, value: i32) {
        let value = value.clamp(0, 100);
        self.volume = value;
        self.volume_changed();
        self.settings.volume = value as u32;
        self.persist_settings();

        if let Some(player) = self.player.clone() {
            tokio::spawn(async move {
                if let Err(err) = player.set_volume(value as i64).await {
                    tracing::error!("falha ao ajustar o volume para {value}: {err:?}");
                }
            });
        }
    }),

    adjust_sync_offset: qt_method!(fn adjust_sync_offset(&mut self, delta: f64) {
        self.sync_offset += delta;
        self.sync_offset_changed();

        if let Some(timeline) = self.timeline.clone() {
            let offset = self.sync_offset;
            tokio::spawn(async move {
                timeline.set_offset(offset).await;
                let confirmed = timeline.get_offset().await;
                if (confirmed - offset).abs() > f64::EPSILON {
                    tracing::warn!(
                        "offset aplicado diverge do valor solicitado: solicitado={offset}, aplicado={confirmed}"
                    );
                }
            });
        }
    }),

    clear_lyrics: qt_method!(fn clear_lyrics(&self, url: QString) {
        let Some(player) = self.player.clone() else {
            return;
        };
        let url = url.to_string();
        tokio::spawn(async move {
            if let Err(err) = player.clear_lyrics_cache(&url).await {
                tracing::error!("falha ao limpar o cache de letras {url}: {err:?}");
            }
        });
    }),

    update_lyric_line: qt_method!(fn update_lyric_line(&mut self, id: i64, new_text: QString) {
        let Some(player) = self.player.clone() else {
            return;
        };
        let new_text = new_text.to_string();
        tokio::spawn(async move {
            if let Err(err) = player.update_lyrics_line(id, &new_text).await {
                tracing::error!("falha ao atualizar a linha de letra {id}: {err:?}");
            }
        });
    }),

    toggle_projection: qt_method!(fn toggle_projection(&mut self) {
        self.projection_visible = !self.projection_visible;
        self.projection_visible_changed();
    }),

    toggle_clear_screen: qt_method!(fn toggle_clear_screen(&mut self) {
        self.clear_screen = !self.clear_screen;
        self.clear_screen_changed();
    }),

    set_font_size: qt_method!(fn set_font_size(&mut self, value: u32) {
        self.font_size = value;
        self.settings.font_size = value;
        self.style_changed();
        self.persist_settings();
    }),

    set_font_family: qt_method!(fn set_font_family(&mut self, value: QString) {
        self.font_family = value.clone();
        self.settings.font_family = value.to_string();
        self.style_changed();
        self.persist_settings();
    }),

    set_font_color: qt_method!(fn set_font_color(&mut self, value: QString) {
        self.font_color = value.clone();
        self.settings.font_color = value.to_string();
        self.style_changed();
        self.persist_settings();
    }),

    set_background_color: qt_method!(fn set_background_color(&mut self, value: QString) {
        self.background_color = value.clone();
        self.settings.background_color = value.to_string();
        self.style_changed();
        self.persist_settings();
    }),

    set_projector_screen_index: qt_method!(fn set_projector_screen_index(&mut self, value: i32) {
        self.projector_screen_index = value;
        self.settings.projector_monitor = if value < 0 { None } else { Some(value as u32) };
        self.style_changed();
        self.persist_settings();
    }),

    player: Option<Arc<Player>>,
    timeline: Option<Arc<Timeline>>,
    playlist_handle: Option<Arc<Playlist>>,
    pool: Option<SqlitePool>,
    settings: Settings,
}

impl AppController {
    /// Cria o controlador já populado com o estado de estilo das configurações.
    #[allow(clippy::field_reassign_with_default)]
    fn new(
        player: Arc<Player>,
        timeline: Arc<Timeline>,
        playlist: Arc<Playlist>,
        pool: SqlitePool,
        settings: &Settings,
    ) -> Self {
        let mut controller = AppController::default();
        controller.playback_state = QString::from(playback_state_label(PlaybackState::Idle));
        controller.sync_offset = 0.0;
        controller.volume = settings.volume as i32;
        controller.font_size = settings.font_size;
        controller.font_family = QString::from(settings.font_family.as_str());
        controller.font_color = QString::from(settings.font_color.as_str());
        controller.background_color = QString::from(settings.background_color.as_str());
        controller.projector_screen_index =
            settings.projector_monitor.map(|m| m as i32).unwrap_or(-1);
        controller.clear_screen = false;
        controller.next_lyric_text = QString::default();
        controller.history_search_query = QString::default();
        controller.player = Some(player);
        controller.timeline = Some(timeline);
        controller.playlist_handle = Some(playlist);
        controller.pool = Some(pool);
        controller.settings = settings.clone();
        controller.current_lyrics = QVariantList::default();
        controller.active_line_id = -1;
        controller
    }

    /// Grava as configurações atuais no disco, registrando eventual falha.
    fn persist_settings(&self) {
        if let Err(err) = crate::shared::config::save_settings(&self.settings) {
            tracing::error!("falha ao salvar as configurações: {err:?}");
        }
    }

    /// Inicia o carregamento e a reprodução da mídia da `url`.
    ///
    /// Limpa o estado de erro/status, sinaliza o carregamento e delega ao
    /// `Player` em uma tarefa de segundo plano, propagando eventuais falhas ao
    /// QML pelo mesmo padrão `QPointer` + `queued_callback`.
    fn load_url(&mut self, url: String) {
        self.sync_offset = 0.0;
        self.sync_offset_changed();
        self.current_lyrics = QVariantList::default();
        self.current_lyrics_changed();
        self.active_line_id = -1;
        self.active_line_id_changed();
        self.lyric_text = QString::default();
        self.lyric_text_changed();
        self.next_lyric_text = QString::default();
        self.next_lyric_text_changed();
        self.clear_screen = false;
        self.clear_screen_changed();

        let Some(player) = self.player.clone() else {
            return;
        };
        if let Some(timeline) = self.timeline.clone() {
            tokio::spawn(async move {
                timeline.set_offset(0.0).await;
            });
        }
        self.error_message = QString::default();
        self.error_message_changed();
        self.loading_status = QString::default();
        self.loading_status_changed();
        self.loading = true;
        self.loading_changed();

        let qptr = QPointer::from(&*self);
        let refresh = queued_callback(move |()| {
            if let Some(pinned) = qptr.as_pinned() {
                pinned.borrow().spawn_history_refresh();
            }
        });

        let qptr_err = QPointer::from(&*self);
        let show_error = queued_callback(move |msg: String| {
            if let Some(pinned) = qptr_err.as_pinned() {
                let mut this = pinned.borrow_mut();
                this.error_message = QString::from(msg.as_str());
                this.error_message_changed();
                this.loading = false;
                this.loading_changed();
            }
        });

        tokio::spawn(async move {
            match player.load_youtube(&url).await {
                Ok(()) => refresh(()),
                Err(err) => {
                    tracing::error!("falha ao carregar a música {url}: {err:?}");
                    show_error(format!("Erro ao carregar: {err}"));
                }
            }
        });
    }

    /// Recarrega a fila de reprodução exposta ao QML em segundo plano.
    ///
    /// Segue o mesmo padrão de `spawn_history_refresh`: a leitura ocorre em uma
    /// tarefa tokio e a `QVariantList` é montada na thread do Qt.
    fn spawn_playlist_refresh(&self) {
        let Some(playlist) = self.playlist_handle.clone() else {
            return;
        };
        let Some(pool) = self.pool.clone() else {
            return;
        };
        let qptr = QPointer::from(self);

        let apply = queued_callback(move |items: Vec<Music>| {
            let Some(pinned) = qptr.as_pinned() else {
                return;
            };
            let mut this = pinned.borrow_mut();
            let mut list = QVariantList::default();
            for music in items {
                let mut map = QVariantMap::default();
                map.insert("id".into(), QString::from(music.id.as_str()).into());
                map.insert("title".into(), QString::from(music.title.as_str()).into());
                map.insert(
                    "artist".into(),
                    QString::from(music.artist.unwrap_or_default().as_str()).into(),
                );
                map.insert(
                    "youtube_url".into(),
                    QString::from(music.youtube_url.as_str()).into(),
                );
                map.insert(
                    "has_lyrics".into(),
                    music.has_lyrics.unwrap_or(false).into(),
                );
                list.push(map.into());
            }
            this.playlist = list;
            this.playlist_changed();
        });

        tokio::spawn(async move {
            let repository = MusicRepository::new(&pool);
            let mut items = playlist.get_items().await;

            for music in &mut items {
                match repository.has_lyrics(&music.id).await {
                    Ok(has_lyrics) => music.has_lyrics = Some(has_lyrics),
                    Err(err) => {
                        tracing::error!(
                            "falha ao atualizar o status de letras da música {}: {err:?}",
                            music.id
                        );
                        music.has_lyrics = Some(false);
                    }
                }
            }

            apply(items);
        });
    }

    /// Recarrega o histórico de músicas do banco em segundo plano.
    ///
    /// A consulta ocorre em uma tarefa tokio, mas a `QVariantList` exposta ao
    /// QML é montada dentro do `queued_callback`, na thread do Qt.
    fn spawn_history_refresh(&self) {
        let Some(pool) = self.pool.clone() else {
            return;
        };
        let search_query = self.history_search_query.to_string();
        let qptr = QPointer::from(self);

        let apply = queued_callback(move |items: Vec<Music>| {
            let Some(pinned) = qptr.as_pinned() else {
                return;
            };
            let mut this = pinned.borrow_mut();
            let mut list = QVariantList::default();
            for music in items {
                let mut map = QVariantMap::default();
                map.insert("id".into(), QString::from(music.id.as_str()).into());
                map.insert("title".into(), QString::from(music.title.as_str()).into());
                map.insert(
                    "artist".into(),
                    QString::from(music.artist.unwrap_or_default().as_str()).into(),
                );
                map.insert(
                    "youtube_url".into(),
                    QString::from(music.youtube_url.as_str()).into(),
                );
                map.insert(
                    "has_lyrics".into(),
                    music.has_lyrics.unwrap_or(false).into(),
                );
                list.push(map.into());
            }
            this.history = list;
            this.history_changed();
        });

        tokio::spawn(async move {
            let repository = MusicRepository::new(&pool);
            let query = search_query.trim().to_string();
            match if query.is_empty() {
                repository.list_all(None).await
            } else {
                repository.list_all(Some(query.as_str())).await
            } {
                Ok(items) => apply(items),
                Err(err) => tracing::error!("falha ao atualizar o histórico: {err:?}"),
            }
        });
    }

    /// Inicia as tarefas que consomem os eventos do `Player` e da `Timeline`.
    ///
    /// Deve ser chamado na thread do Qt, após o objeto ser pinado. Os
    /// `queued_callback` capturam um `QPointer` e garantem que as mutações das
    /// propriedades ocorram sempre na thread principal do Qt.
    fn start_listeners(&self) {
        let Some(player) = self.player.clone() else {
            return;
        };
        let Some(timeline) = self.timeline.clone() else {
            return;
        };

        self.spawn_player_listener(player);
        self.spawn_timeline_listener(timeline);
    }

    fn spawn_player_listener(&self, player: Arc<Player>) {
        let qptr = QPointer::from(self);

        let apply = queued_callback(move |event: PlayerEvent| {
            let Some(pinned) = qptr.as_pinned() else {
                return;
            };
            let mut this = pinned.borrow_mut();
            match event {
                PlayerEvent::StateChanged(state) => {
                    this.playback_state = QString::from(playback_state_label(state));
                    this.playback_state_changed();
                    if state != PlaybackState::Loading && this.loading {
                        this.loading = false;
                        this.loading_changed();
                    }
                    if state == PlaybackState::Stopped {
                        this.current_lyrics = QVariantList::default();
                        this.current_lyrics_changed();
                        this.active_line_id = -1;
                        this.active_line_id_changed();
                    }
                }
                PlayerEvent::MusicLoaded { music, lyrics } => {
                    this.music_title = QString::from(music.title.as_str());
                    this.music_artist =
                        QString::from(music.artist.unwrap_or_default().as_str());
                    this.music_title_changed();
                    this.music_artist_changed();
                    let mut list = QVariantList::default();
                    for line in lyrics {
                        let mut map = QVariantMap::default();
                        map.insert("id".into(), line.id.into());
                        map.insert("start_time".into(), line.start_time.into());
                        map.insert("text".into(), QString::from(line.text.as_str()).into());
                        list.push(map.into());
                    }
                    this.current_lyrics = list;
                    this.current_lyrics_changed();
                    this.active_line_id = -1;
                    this.active_line_id_changed();
                    if let Some(duration) = music.duration {
                        this.duration = duration as f64;
                        this.duration_changed();
                    }
                    this.loading = false;
                    this.loading_changed();
                }
                PlayerEvent::LyricsUpdated(lyrics) => {
                    let mut list = QVariantList::default();
                    for line in lyrics {
                        let mut map = QVariantMap::default();
                        map.insert("id".into(), line.id.into());
                        map.insert("start_time".into(), line.start_time.into());
                        map.insert("text".into(), QString::from(line.text.as_str()).into());
                        list.push(map.into());
                    }
                    this.current_lyrics = list;
                    this.current_lyrics_changed();
                }
                PlayerEvent::PositionUpdated { position, duration } => {
                    this.current_time = position;
                    this.current_time_changed();
                    if let Some(duration) = duration
                        && (duration - this.duration).abs() > f64::EPSILON
                    {
                        this.duration = duration;
                        this.duration_changed();
                    }
                }
                PlayerEvent::PlaybackFinished => {
                    this.current_time = 0.0;
                    this.current_time_changed();
                    this.current_lyrics = QVariantList::default();
                    this.current_lyrics_changed();
                    this.active_line_id = -1;
                    this.active_line_id_changed();
                    this.play_next();
                }
                PlayerEvent::LoadingStatus(status) => {
                    this.loading_status = QString::from(status.as_str());
                    this.loading_status_changed();
                }
            }
        });

        let mut rx = player.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => apply(event),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    fn spawn_timeline_listener(&self, timeline: Arc<Timeline>) {
        let qptr = QPointer::from(self);

        let apply = queued_callback(move |event: TimelineEvent| {
            let Some(pinned) = qptr.as_pinned() else {
                return;
            };
            let mut this = pinned.borrow_mut();
            match event {
                TimelineEvent::LineChanged { active, next } => {
                    this.active_line_id = active.as_ref().map(|l| l.id).unwrap_or(-1);
                    this.active_line_id_changed();
                    let text = active.map(|l| l.text).unwrap_or_default();
                    this.lyric_text = QString::from(text.as_str());
                    this.lyric_text_changed();
                    let next_text = next.map(|l| l.text).unwrap_or_default();
                    this.next_lyric_text = QString::from(next_text.as_str());
                    this.next_lyric_text_changed();
                }
            }
        });

        let mut rx = timeline.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => apply(event),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_url_resets_clear_screen() {
        let mut controller = AppController::default();
        controller.clear_screen = true;

        controller.load_url("https://example.com/watch?v=test".to_string());

        assert!(!controller.clear_screen);
    }
}

/// Inicializa a interface do operador e a janela de projeção.
///
/// Em ambientes sem servidor gráfico (ex.: testes automatizados headless),
/// registra um aviso e retorna sem abrir as janelas, evitando quebra.
pub fn run_operator_ui(
    player: Arc<Player>,
    timeline: Arc<Timeline>,
    playlist: Arc<Playlist>,
    pool: SqlitePool,
    settings: Settings,
) -> anyhow::Result<()> {
    if !has_display() {
        tracing::warn!(
            "Nenhum servidor gráfico ativo (WAYLAND_DISPLAY/DISPLAY ausente); \
             pulando a inicialização da interface QML."
        );
        return Ok(());
    }

    register_qml_resources();

    let controller =
        QObjectBox::new(AppController::new(player, timeline, playlist, pool, &settings));
    let pinned = controller.pinned();

    let mut engine = QmlEngine::new();
    // Expõe o controlador ao QML; isso cria o objeto C++ subjacente, que é
    // pré-requisito para `QPointer::from` usado pelos listeners.
    engine.set_object_property("appController".into(), pinned);
    pinned.borrow().start_listeners();

    engine.load_file("qrc:/letras_sync/presentation/operator.qml".into());
    tracing::info!("Interface do operador inicializada.");
    engine.exec();

    Ok(())
}
