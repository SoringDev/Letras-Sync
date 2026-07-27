use std::sync::Arc;

use tokio::sync::{RwLock, broadcast};

use crate::domain::lyrics::LyricsLine;

use super::player::{PlaybackState, Player, PlayerEvent};

/// Capacidade do canal de eventos reativos da timeline.
const EVENT_CHANNEL_CAPACITY: usize = 64;

/// Eventos reativos propagados aos consumidores da timeline.
#[derive(Debug, Clone)]
pub enum TimelineEvent {
    /// A linha de letra ativa foi alterada. `None` representa silêncio ou
    /// ausência de letra.
    LineChanged {
        active: Option<LyricsLine>,
        next: Option<LyricsLine>,
    },
}

/// Estado interno compartilhado da timeline.
#[derive(Default)]
struct TimelineState {
    lyrics: Vec<LyricsLine>,
    active_line: Option<LyricsLine>,
    position: f64,
    offset: f64,
}

/// Correlaciona o tempo de execução do player com as linhas de letra
/// sincronizadas e notifica quando uma nova linha deve ser projetada.
pub struct Timeline {
    player: Arc<Player>,
    state: Arc<RwLock<TimelineState>>,
    event_tx: broadcast::Sender<TimelineEvent>,
}

impl Timeline {
    /// Inicializa a timeline, assina o canal de eventos do player e inicia o
    /// processamento dos eventos em segundo plano.
    pub fn new(player: Arc<Player>) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);

        let timeline = Arc::new(Self {
            player,
            state: Arc::new(RwLock::new(TimelineState::default())),
            event_tx,
        });

        Self::spawn_event_loop(&timeline);

        timeline
    }

    /// Assina o canal de eventos reativos da timeline.
    pub fn subscribe(&self) -> broadcast::Receiver<TimelineEvent> {
        self.event_tx.subscribe()
    }

    /// Propaga um evento, ignorando a ausência de assinantes.
    fn emit(&self, event: TimelineEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Inicia o loop de processamento dos eventos do player em segundo plano.
    fn spawn_event_loop(timeline: &Arc<Self>) {
        let timeline = Arc::clone(timeline);
        let mut rx = timeline.player.subscribe();

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => timeline.handle_player_event(event).await,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    /// Processa um evento recebido do player e atualiza o estado da timeline.
    async fn handle_player_event(&self, event: PlayerEvent) {
        match event {
            PlayerEvent::MusicLoaded { lyrics, .. } => {
                let mut state = self.state.write().await;
                state.lyrics = lyrics;
                state.active_line = None;
                state.position = 0.0;
                drop(state);
                self.emit(TimelineEvent::LineChanged {
                    active: None,
                    next: None,
                });
            }
            PlayerEvent::LyricsUpdated(lyrics) => {
                let mut state = self.state.write().await;
                state.lyrics = lyrics;
                let adjusted_position = state.position + state.offset;
                let (active, next) = active_and_next_line(&state.lyrics, adjusted_position);
                state.active_line = active.clone();
                drop(state);
                self.emit(TimelineEvent::LineChanged { active, next });
            }
            PlayerEvent::StateChanged(PlaybackState::Stopped)
            | PlayerEvent::PlaybackFinished => {
                let mut state = self.state.write().await;
                state.lyrics = Vec::new();
                state.active_line = None;
                state.position = 0.0;
                drop(state);
                self.emit(TimelineEvent::LineChanged {
                    active: None,
                    next: None,
                });
            }
            PlayerEvent::PositionUpdated { position, .. } => {
                self.update_active_line(position).await;
            }
            PlayerEvent::StateChanged(_) | PlayerEvent::LoadingStatus(_) => {}
        }
    }

    /// Ajusta o offset aplicado ao tempo atual da música.
    pub async fn set_offset(&self, offset: f64) {
        let mut state = self.state.write().await;
        state.offset = offset;

        let adjusted_position = state.position + state.offset;
        let (active, next) = active_and_next_line(&state.lyrics, adjusted_position);

        if same_line(&state.active_line, &active) {
            return;
        }

        state.active_line = active.clone();
        drop(state);
        self.emit(TimelineEvent::LineChanged { active, next });
    }

    /// Retorna o offset atual aplicado à timeline.
    pub async fn get_offset(&self) -> f64 {
        self.state.read().await.offset
    }

    /// Recalcula a linha ativa para a posição informada e, havendo mudança
    /// real, atualiza o estado e emite `LineChanged`.
    async fn update_active_line(&self, position: f64) {
        let mut state = self.state.write().await;
        state.position = position;
        let adjusted_position = state.position + state.offset;
        let (active, next) = active_and_next_line(&state.lyrics, adjusted_position);

        if same_line(&state.active_line, &active) {
            return;
        }

        state.active_line = active.clone();
        drop(state);
        self.emit(TimelineEvent::LineChanged { active, next });
    }
}

/// Encontra a linha ativa para a posição informada.
///
/// Uma linha é ativa quando `start_time <= position < end_time`.
fn find_active_line(lines: &[LyricsLine], position: f64) -> Option<&LyricsLine> {
    lines
        .iter()
        .find(|line| line.start_time <= position && position < line.end_time)
}

fn active_and_next_line(
    lines: &[LyricsLine],
    position: f64,
) -> (Option<LyricsLine>, Option<LyricsLine>) {
    let Some(active) = find_active_line(lines, position) else {
        return (None, None);
    };

    let Some(index) = lines.iter().position(|line| line.id == active.id) else {
        return (None, None);
    };

    let active = Some(active.clone());
    let next = lines.get(index + 1).cloned();
    (active, next)
}

/// Compara duas linhas por identidade (`id`), tratando `None` como silêncio.
fn same_line(current: &Option<LyricsLine>, next: &Option<LyricsLine>) -> bool {
    match (current, next) {
        (Some(a), Some(b)) => a.id == b.id,
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: i64, start: f64, end: f64) -> LyricsLine {
        LyricsLine {
            id,
            music_id: "m".to_string(),
            start_time: start,
            end_time: end,
            text: format!("line {id}"),
        }
    }

    fn sample_lines() -> Vec<LyricsLine> {
        vec![
            line(1, 0.0, 2.0),
            line(2, 2.0, 4.0),
            // gap de silêncio entre 4.0 e 5.0
            line(3, 5.0, 7.0),
        ]
    }

    #[test]
    fn find_active_line_matches_exact_start() {
        let lines = sample_lines();
        let active = find_active_line(&lines, 0.0);
        assert_eq!(active.map(|l| l.id), Some(1));
    }

    #[test]
    fn find_active_line_matches_within_range() {
        let lines = sample_lines();
        let active = find_active_line(&lines, 3.0);
        assert_eq!(active.map(|l| l.id), Some(2));
    }

    #[test]
    fn find_active_line_end_time_is_exclusive() {
        let lines = sample_lines();
        // 2.0 é o fim da linha 1 (exclusivo) e o início da linha 2 (inclusivo).
        let active = find_active_line(&lines, 2.0);
        assert_eq!(active.map(|l| l.id), Some(2));
    }

    #[test]
    fn find_active_line_returns_none_in_silence_gap() {
        let lines = sample_lines();
        let active = find_active_line(&lines, 4.5);
        assert!(active.is_none());
    }

    #[test]
    fn find_active_line_returns_none_before_first_line() {
        let lines = sample_lines();
        // Não há linha antes de 0.0 neste conjunto; usamos início negativo.
        let shifted = vec![line(1, 1.0, 2.0)];
        assert!(find_active_line(&shifted, 0.5).is_none());
        let _ = lines;
    }

    #[test]
    fn find_active_line_returns_none_after_last_line() {
        let lines = sample_lines();
        let active = find_active_line(&lines, 9.0);
        assert!(active.is_none());
    }

    #[test]
    fn find_active_line_returns_none_for_empty_lines() {
        let lines: Vec<LyricsLine> = Vec::new();
        assert!(find_active_line(&lines, 1.0).is_none());
    }

    #[test]
    fn find_active_line_handles_backward_seek() {
        let lines = sample_lines();
        // Avança para a linha 3 e depois retrocede (seek) para a linha 1.
        assert_eq!(find_active_line(&lines, 6.0).map(|l| l.id), Some(3));
        assert_eq!(find_active_line(&lines, 1.0).map(|l| l.id), Some(1));
    }

    #[test]
    fn find_active_line_handles_positive_offset_earlier() {
        let lines = vec![line(1, 1.0, 2.0), line(2, 2.0, 3.0)];

        assert!(find_active_line(&lines, 0.5).is_none());
        assert_eq!(find_active_line(&lines, 0.5 + 1.0).map(|l| l.id), Some(1));
    }

    #[test]
    fn same_line_true_for_same_id() {
        let a = Some(line(1, 0.0, 2.0));
        let b = Some(line(1, 0.0, 2.0));
        assert!(same_line(&a, &b));
    }

    #[test]
    fn same_line_false_for_different_id() {
        let a = Some(line(1, 0.0, 2.0));
        let b = Some(line(2, 2.0, 4.0));
        assert!(!same_line(&a, &b));
    }

    #[test]
    fn same_line_true_for_both_none() {
        assert!(same_line(&None, &None));
    }

    #[test]
    fn same_line_false_for_none_vs_some() {
        let b = Some(line(1, 0.0, 2.0));
        assert!(!same_line(&None, &b));
    }

    #[test]
    fn active_and_next_line_returns_subsequent_line() {
        let lines = sample_lines();
        let (active, next) = active_and_next_line(&lines, 2.5);

        assert_eq!(active.as_ref().map(|l| l.id), Some(2));
        assert_eq!(next.as_ref().map(|l| l.id), Some(3));
    }
}
