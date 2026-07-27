use std::sync::Arc;

use qmetaobject::prelude::*;
use qmetaobject::{queued_callback, QObjectBox, QPointer};
use tokio::sync::broadcast;

use crate::application::player::{PlaybackState, Player, PlayerEvent};
use crate::application::timeline::{Timeline, TimelineEvent};
use crate::domain::settings::Settings;

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

    music_title: qt_property!(QString; NOTIFY music_title_changed),
    music_title_changed: qt_signal!(),

    music_artist: qt_property!(QString; NOTIFY music_artist_changed),
    music_artist_changed: qt_signal!(),

    lyric_text: qt_property!(QString; NOTIFY lyric_text_changed),
    lyric_text_changed: qt_signal!(),

    projection_visible: qt_property!(bool; NOTIFY projection_visible_changed),
    projection_visible_changed: qt_signal!(),

    loading: qt_property!(bool; NOTIFY loading_changed),
    loading_changed: qt_signal!(),

    font_size: qt_property!(u32; NOTIFY style_changed),
    font_family: qt_property!(QString; NOTIFY style_changed),
    font_color: qt_property!(QString; NOTIFY style_changed),
    background_color: qt_property!(QString; NOTIFY style_changed),
    projector_screen_index: qt_property!(i32; NOTIFY style_changed),
    style_changed: qt_signal!(),

    load_music: qt_method!(fn load_music(&mut self, url: QString) {
        let Some(player) = self.player.clone() else {
            return;
        };
        self.loading = true;
        self.loading_changed();

        let url = url.to_string();
        tokio::spawn(async move {
            if let Err(err) = player.load_youtube(&url).await {
                tracing::error!("falha ao carregar a música {url}: {err:?}");
            }
        });
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

    toggle_projection: qt_method!(fn toggle_projection(&mut self) {
        self.projection_visible = !self.projection_visible;
        self.projection_visible_changed();
    }),

    player: Option<Arc<Player>>,
    timeline: Option<Arc<Timeline>>,
}

impl AppController {
    /// Cria o controlador já populado com o estado de estilo das configurações.
    #[allow(clippy::field_reassign_with_default)]
    fn new(player: Arc<Player>, timeline: Arc<Timeline>, settings: &Settings) -> Self {
        let mut controller = AppController::default();
        controller.playback_state = QString::from(playback_state_label(PlaybackState::Idle));
        controller.font_size = settings.font_size;
        controller.font_family = QString::from(settings.font_family.as_str());
        controller.font_color = QString::from(settings.font_color.as_str());
        controller.background_color = QString::from(settings.background_color.as_str());
        controller.projector_screen_index =
            settings.projector_monitor.map(|m| m as i32).unwrap_or(-1);
        controller.player = Some(player);
        controller.timeline = Some(timeline);
        controller
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
                }
                PlayerEvent::MusicLoaded { music, .. } => {
                    this.music_title = QString::from(music.title.as_str());
                    this.music_artist =
                        QString::from(music.artist.unwrap_or_default().as_str());
                    this.music_title_changed();
                    this.music_artist_changed();
                    if let Some(duration) = music.duration {
                        this.duration = duration as f64;
                        this.duration_changed();
                    }
                    this.loading = false;
                    this.loading_changed();
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
                TimelineEvent::LineChanged(line) => {
                    let text = line.map(|l| l.text).unwrap_or_default();
                    this.lyric_text = QString::from(text.as_str());
                    this.lyric_text_changed();
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

    let controller = QObjectBox::new(AppController::new(player, timeline, &settings));
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
