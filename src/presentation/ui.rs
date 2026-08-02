use std::path::PathBuf;
use std::sync::Arc;

use qmetaobject::prelude::*;
use qmetaobject::{QObjectBox, QPointer, QVariantList, QVariantMap, queued_callback};
use sqlx::sqlite::SqlitePool;
use tokio::fs;
use tokio::sync::broadcast;

use crate::application::lyrics_service::DebugLyricsProvider;
use crate::application::player::{PlaybackState, Player, PlayerEvent};
use crate::application::playlist::Playlist;
use crate::application::timeline::{Timeline, TimelineEvent};
use crate::domain::music::Music;
use crate::domain::settings::Settings;
use crate::infrastructure::lyrics_repository::LyricsRepository;
use crate::infrastructure::music_repository::MusicRepository;
use crate::infrastructure::providers::{
    louvorja::LouvorJaProvider, lrc_parser, lyrics_export, srt_parser, vtt_parser,
};

qrc!(register_qml_resources,
    "letras_sync/presentation" {
        "src/presentation/AppButton.qml" as "AppButton.qml",
        "src/presentation/Divider.qml" as "Divider.qml",
        "src/presentation/Eyebrow.qml" as "Eyebrow.qml",
        "src/presentation/IconButton.qml" as "IconButton.qml",
        "src/presentation/operator.qml" as "operator.qml",
        "src/presentation/Pill.qml" as "Pill.qml",
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

fn can_seek(duration: f64, seconds: f64) -> bool {
    duration.is_finite()
        && duration > 0.0
        && seconds.is_finite()
        && seconds >= 0.0
        && seconds <= duration
}

fn clamp_seek_seconds(duration: f64, seconds: f64) -> Option<f64> {
    if !can_seek(duration, seconds) {
        return None;
    }

    let upper_bound = (duration - 0.01).max(0.0);
    Some(seconds.min(upper_bound))
}

fn is_web_or_file_url(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("file://")
}

fn qml_path_to_pathbuf(file_path: &str) -> std::path::PathBuf {
    if let Some(path) = file_path.strip_prefix("file://") {
        std::path::PathBuf::from(path)
    } else {
        std::path::PathBuf::from(file_path)
    }
}

fn trim_lyric_line_text(text: &str) -> String {
    text.trim_end_matches(|c: char| !c.is_alphanumeric())
        .to_string()
}

fn parse_debug_lyrics_provider(value: &str) -> Option<DebugLyricsProvider> {
    match value.trim().to_lowercase().as_str() {
        "lrclib" => Some(DebugLyricsProvider::Lrclib),
        "youtube" => Some(DebugLyricsProvider::Youtube),
        "netease" => Some(DebugLyricsProvider::Netease),
        "whisper" => Some(DebugLyricsProvider::Whisper),
        _ => None,
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

    autoplay: qt_property!(bool; NOTIFY style_changed),

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

    debug_lyrics_provider_override: qt_property!(QString; NOTIFY debug_lyrics_provider_override_changed),
    debug_lyrics_provider_override_changed: qt_signal!(),

    current_lyrics: qt_property!(QVariantList; NOTIFY current_lyrics_changed),
    current_lyrics_changed: qt_signal!(),

    current_music_id: qt_property!(QString; NOTIFY current_music_id_changed),
    current_music_id_changed: qt_signal!(),

    active_line_id: qt_property!(i64; NOTIFY active_line_id_changed),
    active_line_id_changed: qt_signal!(),

    playlist: qt_property!(QVariantList; NOTIFY playlist_changed),
    playlist_changed: qt_signal!(),

    louvorja_search_results: qt_property!(QVariantList; NOTIFY louvorja_search_results_changed),
    louvorja_search_results_changed: qt_signal!(),

    font_size: qt_property!(u32; NOTIFY style_changed),
    font_family: qt_property!(QString; NOTIFY style_changed),
    font_color: qt_property!(QString; NOTIFY style_changed),
    background_color: qt_property!(QString; NOTIFY style_changed),
    projector_screen_index: qt_property!(i32; NOTIFY style_changed),
    projection_font_family: qt_property!(QString; NOTIFY style_changed),
    projection_font_size: qt_property!(u32; NOTIFY style_changed),
    projection_font_weight: qt_property!(u32; NOTIFY style_changed),
    projection_letter_spacing: qt_property!(f64; NOTIFY style_changed),
    projection_line_height_multiplier: qt_property!(f64; NOTIFY style_changed),
    projection_dynamic_font_scaling: qt_property!(bool; NOTIFY style_changed),
    projection_min_font_size: qt_property!(u32; NOTIFY style_changed),
    projection_max_font_multiplier: qt_property!(f64; NOTIFY style_changed),
    projection_margin_horizontal: qt_property!(u32; NOTIFY style_changed),
    projection_margin_vertical: qt_property!(u32; NOTIFY style_changed),
    projection_horizontal_alignment: qt_property!(QString; NOTIFY style_changed),
    projection_vertical_alignment: qt_property!(QString; NOTIFY style_changed),
    projection_font_color: qt_property!(QString; NOTIFY style_changed),
    projection_background_color: qt_property!(QString; NOTIFY style_changed),
    projection_shadow_enabled: qt_property!(bool; NOTIFY style_changed),
    projection_shadow_color: qt_property!(QString; NOTIFY style_changed),
    projection_shadow_offset_x: qt_property!(i32; NOTIFY style_changed),
    projection_shadow_offset_y: qt_property!(i32; NOTIFY style_changed),
    projection_fade_duration_ms: qt_property!(u32; NOTIFY style_changed),
    projection_fade_animation_enabled: qt_property!(bool; NOTIFY style_changed),
    style_changed: qt_signal!(),

    history: qt_property!(QVariantList; NOTIFY history_changed),
    history_changed: qt_signal!(),

    history_search_query: qt_property!(QString; NOTIFY history_search_query_changed),
    history_search_query_changed: qt_signal!(),

    load_music: qt_method!(
        fn load_music(&mut self, url: QString) {
            let input = url.to_string();
            let forced_provider =
                parse_debug_lyrics_provider(&self.debug_lyrics_provider_override.to_string());

            if forced_provider.is_some() && !is_web_or_file_url(&input) {
                self.error_message =
                    QString::from("Depuração de provider exige URL do YouTube ou arquivo local");
                self.error_message_changed();
                return;
            }

            if is_web_or_file_url(&input) {
                self.load_url(input);
            } else {
                self.search_louvorja(url);
            }
        }
    ),

    search_louvorja: qt_method!(
        fn search_louvorja(&mut self, query: QString) {
            let query = query.to_string();
            let trimmed = query.trim().to_string();

            self.louvorja_search_results = QVariantList::default();
            self.louvorja_search_results_changed();

            if trimmed.is_empty() {
                self.error_message =
                    QString::from("Erro: informe o nome da música para buscar no LouvorJA");
                self.error_message_changed();
                return;
            }

            let cache_path = PathBuf::from(self.settings.cache_path.clone());
            let qptr = QPointer::from(&*self);
            let apply = queued_callback(
                move |items: Vec<crate::infrastructure::providers::louvorja::CatalogEntry>| {
                    if let Some(pinned) = qptr.as_pinned() {
                        let mut this = pinned.borrow_mut();
                        let mut list = QVariantList::default();
                        for item in items {
                            let mut map = QVariantMap::default();
                            map.insert("id".into(), QString::from(item.id.as_str()).into());
                            map.insert("name".into(), QString::from(item.name.as_str()).into());
                            map.insert("album".into(), QString::from(item.album.as_str()).into());
                            list.push(map.into());
                        }
                        this.louvorja_search_results = list;
                        this.louvorja_search_results_changed();
                        if this.louvorja_search_results.is_empty() {
                            this.error_message =
                                QString::from("Nenhum resultado encontrado no LouvorJA");
                            this.error_message_changed();
                        } else {
                            this.error_message = QString::default();
                            this.error_message_changed();
                        }
                    }
                },
            );

            let qptr_err = QPointer::from(&*self);
            let show_error = queued_callback(move |msg: String| {
                if let Some(pinned) = qptr_err.as_pinned() {
                    let mut this = pinned.borrow_mut();
                    this.error_message = QString::from(msg.as_str());
                    this.error_message_changed();
                }
            });

            tokio::spawn(async move {
                let provider = LouvorJaProvider::new();
                match provider.search_catalog(&trimmed, &cache_path).await {
                    Ok(items) => apply(items),
                    Err(err) => {
                        tracing::error!("falha ao buscar no LouvorJA para '{trimmed}': {err:?}");
                        show_error(format!("Erro: falha ao buscar no LouvorJA: {err}"));
                    }
                }
            });
        }
    ),

    load_louvorja_song: qt_method!(
        fn load_louvorja_song(&mut self, id: QString) {
            let louvorja_id = id.to_string();
            if louvorja_id.trim().is_empty() {
                return;
            }

            self.louvorja_search_results = QVariantList::default();
            self.louvorja_search_results_changed();

            self.current_lyrics = QVariantList::default();
            self.current_lyrics_changed();
            self.current_music_id = QString::default();
            self.current_music_id_changed();
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
                match player.load_louvorja_song(&louvorja_id).await {
                    Ok(()) => refresh(()),
                    Err(err) => {
                        tracing::error!(
                            "falha ao carregar a música do LouvorJA {louvorja_id}: {err:?}"
                        );
                        show_error(format!("Erro ao carregar do LouvorJA: {err}"));
                    }
                }
            });
        }
    ),

    add_to_playlist: qt_method!(
        fn add_to_playlist(&mut self, url: QString) {
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
        }
    ),

    remove_from_playlist: qt_method!(
        fn remove_from_playlist(&mut self, index: i32) {
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
        }
    ),

    play_playlist_item: qt_method!(
        fn play_playlist_item(&mut self, index: i32) {
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
        }
    ),

    play_next: qt_method!(
        fn play_next(&mut self) {
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
        }
    ),

    play_previous: qt_method!(
        fn play_previous(&mut self) {
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
        }
    ),

    refresh_history: qt_method!(
        fn refresh_history(&self) {
            self.spawn_history_refresh();
        }
    ),

    set_history_search_query: qt_method!(
        fn set_history_search_query(&mut self, query: QString) {
            self.history_search_query = query;
            self.history_search_query_changed();
            self.spawn_history_refresh();
        }
    ),

    play: qt_method!(
        fn play(&self) {
            let Some(player) = self.player.clone() else {
                return;
            };
            tokio::spawn(async move {
                if let Err(err) = player.play().await {
                    tracing::error!("falha ao retomar a reprodução: {err:?}");
                }
            });
        }
    ),

    pause: qt_method!(
        fn pause(&self) {
            let Some(player) = self.player.clone() else {
                return;
            };
            tokio::spawn(async move {
                if let Err(err) = player.pause().await {
                    tracing::error!("falha ao pausar a reprodução: {err:?}");
                }
            });
        }
    ),

    stop: qt_method!(
        fn stop(&self) {
            let Some(player) = self.player.clone() else {
                return;
            };
            tokio::spawn(async move {
                if let Err(err) = player.stop().await {
                    tracing::error!("falha ao interromper a reprodução: {err:?}");
                }
            });
        }
    ),

    seek: qt_method!(
        fn seek(&self, seconds: f64) {
            let Some(seconds) = clamp_seek_seconds(self.duration, seconds) else {
                return;
            };

            let Some(player) = self.player.clone() else {
                return;
            };
            tokio::spawn(async move {
                if let Err(err) = player.seek(seconds).await {
                    tracing::error!("falha ao reposicionar a reprodução: {err:?}");
                }
            });
        }
    ),

    seek_relative: qt_method!(
        fn seek_relative(&self, delta: f64) {
            let Some(player) = self.player.clone() else {
                return;
            };
            tokio::spawn(async move {
                if let Err(err) = player.seek_relative(delta).await {
                    tracing::error!("falha ao reposicionar a reprodução: {err:?}");
                }
            });
        }
    ),

    set_volume: qt_method!(
        fn set_volume(&mut self, value: i32) {
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
        }
    ),

    set_autoplay: qt_method!(
        fn set_autoplay(&mut self, enabled: bool) {
            self.autoplay = enabled;
            self.settings.autoplay = enabled;
            if let Err(err) = crate::shared::config::save_settings(&self.settings) {
                tracing::warn!("falha ao salvar configuração de autoplay: {err}");
            }
            self.style_changed();
        }
    ),

    adjust_sync_offset: qt_method!(
        fn adjust_sync_offset(&mut self, delta: f64) {
            self.sync_offset += delta;
            self.sync_offset_changed();

            let offset = self.sync_offset;
            if let Some(timeline) = self.timeline.clone() {
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

            if let Some(player) = self.player.clone() {
                tokio::spawn(async move {
                    if let Err(err) = player.update_sync_offset(offset).await {
                        tracing::error!("falha ao persistir o sync_offset {offset}: {err:?}");
                    }
                });
            }
        }
    ),

    clear_lyrics: qt_method!(
        fn clear_lyrics(&self, url: QString) {
            let Some(player) = self.player.clone() else {
                return;
            };
            let url = url.to_string();
            tokio::spawn(async move {
                if let Err(err) = player.clear_lyrics_cache(&url).await {
                    tracing::error!("falha ao limpar o cache de letras {url}: {err:?}");
                }
            });
        }
    ),

    clear_database: qt_method!(
        fn clear_database(&mut self) {
            let Some(pool) = self.pool.clone() else {
                self.error_message = QString::from("Erro: banco de dados indisponível");
                self.error_message_changed();
                return;
            };

            let qptr = QPointer::from(&*self);
            let refresh = queued_callback(move |()| {
                if let Some(pinned) = qptr.as_pinned() {
                    let mut this = pinned.borrow_mut();
                    this.spawn_history_refresh();
                    this.error_message = QString::from("OK: Banco limpo");
                    this.error_message_changed();
                }
            });

            let qptr_err = QPointer::from(&*self);
            let show_error = queued_callback(move |msg: String| {
                if let Some(pinned) = qptr_err.as_pinned() {
                    let mut this = pinned.borrow_mut();
                    this.error_message = QString::from(msg.as_str());
                    this.error_message_changed();
                }
            });

            tokio::spawn(async move {
                let repository = MusicRepository::new(&pool);
                match repository.clear_all_data().await {
                    Ok(()) => refresh(()),
                    Err(err) => {
                        tracing::error!("falha ao limpar o banco de dados: {err:?}");
                        show_error(format!("Erro: falha ao limpar o banco: {err}"));
                    }
                }
            });
        }
    ),

    export_lyrics: qt_method!(
        fn export_lyrics(&self, music_id: QString, file_path: QString, format: QString) -> bool {
            let music_id = music_id.to_string();
            let file_path = file_path.to_string();
            let format = format.to_string().to_lowercase();
            let qptr = QPointer::from(self);
            let show_message = queued_callback(move |msg: String| {
                if let Some(pinned) = qptr.as_pinned() {
                    let mut this = pinned.borrow_mut();
                    this.error_message = QString::from(msg.as_str());
                    this.error_message_changed();
                }
            });

            if music_id.trim().is_empty() {
                show_message("Erro: nenhuma música selecionada".to_string());
                return false;
            }

            let Some(pool) = self.pool.clone() else {
                show_message("Erro: banco de dados indisponível".to_string());
                return false;
            };

            let format_name = match format.as_str() {
                "lrc" => "LRC",
                "srt" => "SRT",
                _ => {
                    show_message("Erro: formato de exportação inválido".to_string());
                    return false;
                }
            };

            let path = qml_path_to_pathbuf(&file_path);

            tokio::spawn(async move {
                let repository = LyricsRepository::new(&pool);
                match repository.find_by_music_id(&music_id).await {
                    Ok(lines) => {
                        let content = match format.as_str() {
                            "lrc" => lyrics_export::format_lrc(&lines),
                            "srt" => lyrics_export::format_srt(&lines),
                            _ => String::new(),
                        };

                        if let Some(parent) = path.parent()
                            && !parent.as_os_str().is_empty()
                            && let Err(err) = fs::create_dir_all(parent).await
                        {
                            show_message(format!(
                                "Erro: falha ao criar o diretório de destino: {err}"
                            ));
                            return;
                        }

                        if let Err(err) = fs::write(&path, content).await {
                            show_message(format!("Erro: falha ao salvar o arquivo: {err}"));
                            return;
                        }

                        show_message(format!("OK: Letras exportadas em {format_name}"));
                    }
                    Err(err) => {
                        show_message(format!("Erro: falha ao exportar letras: {err}"));
                    }
                }
            });

            true
        }
    ),

    import_lyrics: qt_method!(
        fn import_lyrics(&mut self, music_id: QString, file_path: QString) -> bool {
            let music_id = music_id.to_string();
            let file_path = file_path.to_string();

            if music_id.trim().is_empty() {
                self.error_message = QString::from("Erro: nenhuma música selecionada");
                self.error_message_changed();
                return false;
            }

            let Some(pool) = self.pool.clone() else {
                self.error_message = QString::from("Erro: banco de dados indisponível");
                self.error_message_changed();
                return false;
            };

            let Some(player) = self.player.clone() else {
                self.error_message = QString::from("Erro: player indisponível");
                self.error_message_changed();
                return false;
            };

            let path = qml_path_to_pathbuf(&file_path);
            let extension = match path.extension().and_then(|ext| ext.to_str()) {
                Some(ext) => ext.to_lowercase(),
                None => {
                    self.error_message = QString::from("Erro: arquivo sem extensão suportada");
                    self.error_message_changed();
                    return false;
                }
            };

            let parser_name = match extension.as_str() {
                "lrc" => "LRC",
                "srt" => "SRT",
                "vtt" => "VTT",
                _ => {
                    self.error_message = QString::from("Erro: formato de importação inválido");
                    self.error_message_changed();
                    return false;
                }
            };

            let active_music_id = self.current_music_id.to_string();
            let qptr = QPointer::from(&*self);
            let show_message = queued_callback(move |msg: String| {
                if let Some(pinned) = qptr.as_pinned() {
                    let mut this = pinned.borrow_mut();
                    this.error_message = QString::from(msg.as_str());
                    this.error_message_changed();
                }
            });

            tokio::spawn(async move {
                let content = match fs::read_to_string(&path).await {
                    Ok(content) => content,
                    Err(err) => {
                        show_message(format!("Erro: falha ao ler o arquivo: {err}"));
                        return;
                    }
                };

                let repository = LyricsRepository::new(&pool);
                let lines = match extension.as_str() {
                    "lrc" => lrc_parser::parse(&content, &music_id),
                    "srt" => srt_parser::parse(&content, &music_id),
                    "vtt" => vtt_parser::parse(&content, &music_id),
                    _ => Vec::new(),
                };

                if lines.is_empty() {
                    show_message(format!(
                        "Erro: nenhum verso válido encontrado no arquivo {parser_name}"
                    ));
                    return;
                }

                if let Err(err) = repository.delete_by_music_id(&music_id).await {
                    show_message(format!("Erro: falha ao substituir as letras: {err}"));
                    return;
                }

                if let Err(err) = repository.save_all(&lines).await {
                    show_message(format!("Erro: falha ao salvar as letras importadas: {err}"));
                    return;
                }

                if active_music_id == music_id {
                    match repository.find_by_music_id(&music_id).await {
                        Ok(saved_lines) => {
                            if let Err(err) =
                                player.replace_current_lyrics(&music_id, saved_lines).await
                            {
                                show_message(format!(
                                    "Erro: falha ao recarregar as letras ativas: {err}"
                                ));
                                return;
                            }
                        }
                        Err(err) => {
                            show_message(format!(
                                "Erro: letras importadas, mas falha ao recarregar a UI: {err}"
                            ));
                            return;
                        }
                    }
                }

                show_message("OK: Letras importadas".to_string());
            });

            true
        }
    ),

    update_lyric_line: qt_method!(
        fn update_lyric_line(&mut self, id: i64, new_text: QString) {
            let Some(player) = self.player.clone() else {
                return;
            };
            let new_text = trim_lyric_line_text(&new_text.to_string());
            tokio::spawn(async move {
                if let Err(err) = player.update_lyrics_line(id, &new_text).await {
                    tracing::error!("falha ao atualizar a linha de letra {id}: {err:?}");
                }
            });
        }
    ),

    toggle_projection: qt_method!(
        fn toggle_projection(&mut self) {
            self.projection_visible = !self.projection_visible;
            self.projection_visible_changed();
        }
    ),

    toggle_clear_screen: qt_method!(
        fn toggle_clear_screen(&mut self) {
            self.clear_screen = !self.clear_screen;
            self.clear_screen_changed();
        }
    ),

    set_font_size: qt_method!(
        fn set_font_size(&mut self, value: u32) {
            self.font_size = value;
            self.settings.font_size = value;
            self.projection_font_size = value;
            self.settings.projection.font_size = value;
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_font_family: qt_method!(
        fn set_font_family(&mut self, value: QString) {
            self.font_family = value.clone();
            self.settings.font_family = value.to_string();
            self.projection_font_family = value.clone();
            self.settings.projection.font_family = value.to_string();
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_font_color: qt_method!(
        fn set_font_color(&mut self, value: QString) {
            self.font_color = value.clone();
            self.settings.font_color = value.to_string();
            self.projection_font_color = value.clone();
            self.settings.projection.font_color = value.to_string();
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_background_color: qt_method!(
        fn set_background_color(&mut self, value: QString) {
            self.background_color = value.clone();
            self.settings.background_color = value.to_string();
            self.projection_background_color = value.clone();
            self.settings.projection.background_color = value.to_string();
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_font_size: qt_method!(
        fn set_projection_font_size(&mut self, value: u32) {
            self.font_size = value;
            self.settings.font_size = value;
            self.projection_font_size = value;
            self.settings.projection.font_size = value;
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_font_family: qt_method!(
        fn set_projection_font_family(&mut self, value: QString) {
            self.font_family = value.clone();
            self.settings.font_family = value.to_string();
            self.projection_font_family = value.clone();
            self.settings.projection.font_family = value.to_string();
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_font_weight: qt_method!(
        fn set_projection_font_weight(&mut self, value: u32) {
            self.projection_font_weight = value;
            self.settings.projection.font_weight = value;
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_letter_spacing: qt_method!(
        fn set_projection_letter_spacing(&mut self, value: f64) {
            self.projection_letter_spacing = value;
            self.settings.projection.letter_spacing = value;
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_line_height_multiplier: qt_method!(
        fn set_projection_line_height_multiplier(&mut self, value: f64) {
            self.projection_line_height_multiplier = value;
            self.settings.projection.line_height_multiplier = value;
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_margin_horizontal: qt_method!(
        fn set_projection_margin_horizontal(&mut self, value: u32) {
            self.projection_margin_horizontal = value;
            self.settings.projection.margin_horizontal = value;
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_margin_vertical: qt_method!(
        fn set_projection_margin_vertical(&mut self, value: u32) {
            self.projection_margin_vertical = value;
            self.settings.projection.margin_vertical = value;
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_horizontal_alignment: qt_method!(
        fn set_projection_horizontal_alignment(&mut self, value: QString) {
            self.projection_horizontal_alignment = value.clone();
            self.settings.projection.horizontal_alignment = value.to_string();
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_vertical_alignment: qt_method!(
        fn set_projection_vertical_alignment(&mut self, value: QString) {
            self.projection_vertical_alignment = value.clone();
            self.settings.projection.vertical_alignment = value.to_string();
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_font_color: qt_method!(
        fn set_projection_font_color(&mut self, value: QString) {
            self.font_color = value.clone();
            self.settings.font_color = value.to_string();
            self.projection_font_color = value.clone();
            self.settings.projection.font_color = value.to_string();
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_background_color: qt_method!(
        fn set_projection_background_color(&mut self, value: QString) {
            self.background_color = value.clone();
            self.settings.background_color = value.to_string();
            self.projection_background_color = value.clone();
            self.settings.projection.background_color = value.to_string();
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_shadow_enabled: qt_method!(
        fn set_projection_shadow_enabled(&mut self, value: bool) {
            self.projection_shadow_enabled = value;
            self.settings.projection.shadow_enabled = value;
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_shadow_color: qt_method!(
        fn set_projection_shadow_color(&mut self, value: QString) {
            self.projection_shadow_color = value.clone();
            self.settings.projection.shadow_color = value.to_string();
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_shadow_offset_x: qt_method!(
        fn set_projection_shadow_offset_x(&mut self, value: i32) {
            self.projection_shadow_offset_x = value;
            self.settings.projection.shadow_offset_x = value;
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_shadow_offset_y: qt_method!(
        fn set_projection_shadow_offset_y(&mut self, value: i32) {
            self.projection_shadow_offset_y = value;
            self.settings.projection.shadow_offset_y = value;
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_fade_duration_ms: qt_method!(
        fn set_projection_fade_duration_ms(&mut self, value: u32) {
            self.projection_fade_duration_ms = value;
            self.settings.projection.fade_duration_ms = value;
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projection_fade_animation_enabled: qt_method!(
        fn set_projection_fade_animation_enabled(&mut self, value: bool) {
            self.projection_fade_animation_enabled = value;
            self.settings.projection.fade_animation_enabled = value;
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_projector_screen_index: qt_method!(
        fn set_projector_screen_index(&mut self, value: i32) {
            self.projector_screen_index = value;
            self.settings.projector_monitor = if value < 0 { None } else { Some(value as u32) };
            self.style_changed();
            self.persist_settings();
        }
    ),

    set_debug_lyrics_provider_override: qt_method!(
        fn set_debug_lyrics_provider_override(&mut self, value: QString) {
            let value = value.to_string().trim().to_lowercase();
            self.debug_lyrics_provider_override = QString::from(value.as_str());
            self.debug_lyrics_provider_override_changed();
        }
    ),

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
        controller.autoplay = settings.autoplay;
        controller.font_size = settings.font_size;
        controller.font_family = QString::from(settings.font_family.as_str());
        controller.font_color = QString::from(settings.font_color.as_str());
        controller.background_color = QString::from(settings.background_color.as_str());
        controller.projector_screen_index =
            settings.projector_monitor.map(|m| m as i32).unwrap_or(-1);
        controller.projection_font_family = QString::from(settings.projection.font_family.as_str());
        controller.projection_font_size = settings.projection.font_size;
        controller.projection_font_weight = settings.projection.font_weight;
        controller.projection_letter_spacing = settings.projection.letter_spacing;
        controller.projection_line_height_multiplier = settings.projection.line_height_multiplier;
        controller.projection_dynamic_font_scaling = settings.projection.dynamic_font_scaling;
        controller.projection_min_font_size = settings.projection.min_font_size;
        controller.projection_max_font_multiplier = settings.projection.max_font_multiplier;
        controller.projection_margin_horizontal = settings.projection.margin_horizontal;
        controller.projection_margin_vertical = settings.projection.margin_vertical;
        controller.projection_horizontal_alignment =
            QString::from(settings.projection.horizontal_alignment.as_str());
        controller.projection_vertical_alignment =
            QString::from(settings.projection.vertical_alignment.as_str());
        controller.projection_font_color = QString::from(settings.projection.font_color.as_str());
        controller.projection_background_color =
            QString::from(settings.projection.background_color.as_str());
        controller.projection_shadow_enabled = settings.projection.shadow_enabled;
        controller.projection_shadow_color =
            QString::from(settings.projection.shadow_color.as_str());
        controller.projection_shadow_offset_x = settings.projection.shadow_offset_x;
        controller.projection_shadow_offset_y = settings.projection.shadow_offset_y;
        controller.projection_fade_duration_ms = settings.projection.fade_duration_ms;
        controller.projection_fade_animation_enabled = settings.projection.fade_animation_enabled;
        controller.clear_screen = false;
        controller.next_lyric_text = QString::default();
        controller.history_search_query = QString::default();
        controller.debug_lyrics_provider_override = QString::default();
        controller.player = Some(player);
        controller.timeline = Some(timeline);
        controller.playlist_handle = Some(playlist);
        controller.pool = Some(pool);
        controller.settings = settings.clone();
        controller.current_lyrics = QVariantList::default();
        controller.current_music_id = QString::default();
        controller.louvorja_search_results = QVariantList::default();
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
        self.current_lyrics = QVariantList::default();
        self.current_lyrics_changed();
        self.current_music_id = QString::default();
        self.current_music_id_changed();
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
        let forced_provider =
            parse_debug_lyrics_provider(&self.debug_lyrics_provider_override.to_string());
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
            match player.load_media_with_provider(&url, forced_provider).await {
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
                        this.current_music_id = QString::default();
                        this.current_music_id_changed();
                        this.active_line_id = -1;
                        this.active_line_id_changed();
                        this.sync_offset = 0.0;
                        this.sync_offset_changed();
                    }
                }
                PlayerEvent::MusicLoaded { music, lyrics } => {
                    this.music_title = QString::from(music.title.as_str());
                    this.music_artist = QString::from(music.artist.unwrap_or_default().as_str());
                    this.music_title_changed();
                    this.music_artist_changed();
                    this.current_music_id = QString::from(music.id.as_str());
                    this.current_music_id_changed();
                    this.sync_offset = music.sync_offset;
                    this.sync_offset_changed();
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

    let controller = QObjectBox::new(AppController::new(
        player, timeline, playlist, pool, &settings,
    ));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_url_resets_clear_screen() {
        let mut controller = AppController {
            clear_screen: true,
            ..AppController::default()
        };

        controller.load_url("https://example.com/watch?v=test".to_string());

        assert!(!controller.clear_screen);
    }

    #[test]
    fn can_seek_requires_positive_duration_and_finite_position() {
        assert!(!can_seek(0.0, 10.0));
        assert!(!can_seek(-1.0, 10.0));
        assert!(!can_seek(120.0, f64::NAN));
        assert!(!can_seek(120.0, -1.0));
        assert!(!can_seek(120.0, 120.1));
        assert!(can_seek(120.0, 0.0));
        assert!(can_seek(120.0, 119.5));
    }

    #[test]
    fn clamp_seek_seconds_keeps_position_inside_duration() {
        assert_eq!(clamp_seek_seconds(120.0, 121.0), None);
        assert_eq!(clamp_seek_seconds(120.0, 120.0), Some(119.99));
        assert_eq!(clamp_seek_seconds(120.0, 30.0), Some(30.0));
    }

    #[test]
    fn trim_lyric_line_text_removes_trailing_noise() {
        assert_eq!(
            trim_lyric_line_text("Linha final!!!   "),
            "Linha final".to_string()
        );
        assert_eq!(trim_lyric_line_text("Verso 2...??"), "Verso 2".to_string());
    }

    #[test]
    fn parse_debug_lyrics_provider_maps_known_values() {
        assert_eq!(
            parse_debug_lyrics_provider("lrclib"),
            Some(DebugLyricsProvider::Lrclib)
        );
        assert_eq!(
            parse_debug_lyrics_provider("youtube"),
            Some(DebugLyricsProvider::Youtube)
        );
        assert_eq!(
            parse_debug_lyrics_provider("netease"),
            Some(DebugLyricsProvider::Netease)
        );
        assert_eq!(
            parse_debug_lyrics_provider("whisper"),
            Some(DebugLyricsProvider::Whisper)
        );
        assert_eq!(parse_debug_lyrics_provider("invalid"), None);
    }
}
