use tokio::sync::RwLock;

use crate::domain::music::Music;

/// Estado interno da fila de reprodução.
struct PlaylistState {
    items: Vec<Music>,
    current: Option<usize>,
}

/// Fila de reprodução em memória, segura para concorrência.
///
/// Mantém a lista de músicas agendadas e o índice do item ativo. Toda a
/// mutação passa por um `RwLock` interno, permitindo compartilhar a fila via
/// `Arc` entre a interface e as tarefas de reprodução.
pub struct Playlist {
    state: RwLock<PlaylistState>,
}

impl Playlist {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(PlaylistState {
                items: Vec::new(),
                current: None,
            }),
        }
    }

    /// Adiciona uma música ao final da fila.
    pub async fn add(&self, music: Music) {
        self.state.write().await.items.push(music);
    }

    /// Remove o item no índice informado, ajustando o índice ativo.
    ///
    /// Índices fora do intervalo são ignorados.
    pub async fn remove(&self, index: usize) {
        let mut state = self.state.write().await;
        if index >= state.items.len() {
            return;
        }
        state.items.remove(index);
        state.current = match state.current {
            Some(current) if current == index => None,
            Some(current) if current > index => Some(current - 1),
            other => other,
        };
    }

    /// Retorna uma cópia dos itens agendados.
    pub async fn get_items(&self) -> Vec<Music> {
        self.state.read().await.items.clone()
    }

    /// Retorna o índice do item ativo, se houver.
    #[allow(dead_code)]
    pub async fn current_index(&self) -> Option<usize> {
        self.state.read().await.current
    }

    /// Define o índice ativo; valores fora do intervalo são ignorados.
    pub async fn set_current_index(&self, index: Option<usize>) {
        let mut state = self.state.write().await;
        match index {
            Some(i) if i < state.items.len() => state.current = Some(i),
            None => state.current = None,
            _ => {}
        }
    }

    /// Retorna a música ativa, se houver.
    pub async fn current_music(&self) -> Option<Music> {
        let state = self.state.read().await;
        state.current.and_then(|i| state.items.get(i).cloned())
    }

    /// Avança para o próximo item e o retorna, ou `None` se não houver.
    pub async fn next(&self) -> Option<Music> {
        let mut state = self.state.write().await;
        let next = match state.current {
            Some(current) => current + 1,
            None => 0,
        };
        if next < state.items.len() {
            state.current = Some(next);
            state.items.get(next).cloned()
        } else {
            None
        }
    }

    /// Retorna ao item anterior e o retorna, ou `None` se não houver.
    pub async fn prev(&self) -> Option<Music> {
        let mut state = self.state.write().await;
        let prev = match state.current {
            Some(current) if current > 0 => current - 1,
            _ => return None,
        };
        state.current = Some(prev);
        state.items.get(prev).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn music(id: &str) -> Music {
        Music {
            id: id.to_string(),
            title: format!("Música {id}"),
            artist: None,
            youtube_url: format!("https://youtu.be/{id}"),
            duration: None,
            thumbnail: None,
            created_at: None,
        }
    }

    #[tokio::test]
    async fn add_appends_items_in_order() {
        let playlist = Playlist::new();
        playlist.add(music("a")).await;
        playlist.add(music("b")).await;

        let items = playlist.get_items().await;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "a");
        assert_eq!(items[1].id, "b");
    }

    #[tokio::test]
    async fn remove_deletes_item_and_shifts_current() {
        let playlist = Playlist::new();
        playlist.add(music("a")).await;
        playlist.add(music("b")).await;
        playlist.add(music("c")).await;
        playlist.set_current_index(Some(2)).await;

        playlist.remove(0).await;

        let items = playlist.get_items().await;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "b");
        assert_eq!(playlist.current_index().await, Some(1));
    }

    #[tokio::test]
    async fn remove_current_clears_active_index() {
        let playlist = Playlist::new();
        playlist.add(music("a")).await;
        playlist.set_current_index(Some(0)).await;

        playlist.remove(0).await;

        assert_eq!(playlist.current_index().await, None);
    }

    #[tokio::test]
    async fn next_advances_through_items() {
        let playlist = Playlist::new();
        playlist.add(music("a")).await;
        playlist.add(music("b")).await;

        assert_eq!(playlist.next().await.map(|m| m.id), Some("a".to_string()));
        assert_eq!(playlist.current_index().await, Some(0));
        assert_eq!(playlist.next().await.map(|m| m.id), Some("b".to_string()));
        assert_eq!(playlist.current_index().await, Some(1));
        assert_eq!(playlist.next().await.map(|m| m.id), None);
        assert_eq!(playlist.current_index().await, Some(1));
    }

    #[tokio::test]
    async fn prev_returns_to_previous_item() {
        let playlist = Playlist::new();
        playlist.add(music("a")).await;
        playlist.add(music("b")).await;
        playlist.set_current_index(Some(1)).await;

        assert_eq!(playlist.prev().await.map(|m| m.id), Some("a".to_string()));
        assert_eq!(playlist.current_index().await, Some(0));
        assert_eq!(playlist.prev().await.map(|m| m.id), None);
        assert_eq!(playlist.current_index().await, Some(0));
    }
}
