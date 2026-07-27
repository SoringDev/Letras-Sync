/// Extrai o `video_id` de uma URL do YouTube.
///
/// Suporta o parâmetro `v=` da query string, incluindo `youtube.com` e
/// `music.youtube.com`, além do formato curto `youtu.be/<id>`.
/// Retorna `None` se nenhum padrão for reconhecido.
pub fn extract_video_id(url: &str) -> Option<String> {
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

/// Normaliza qualquer URL reconhecida do YouTube para o formato canônico.
///
/// O formato resultante é `https://www.youtube.com/watch?v=<video_id>`.
pub fn normalize_youtube_url(url: &str) -> Option<String> {
    extract_video_id(url).map(|video_id| format!("https://www.youtube.com/watch?v={video_id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn extracts_video_id_from_query_param() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=abc123"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn extracts_video_id_with_extra_params() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=abc123&t=10s"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn extracts_video_id_from_short_url() {
        assert_eq!(
            extract_video_id("https://youtu.be/abc123"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn extracts_video_id_from_short_url_with_query() {
        assert_eq!(
            extract_video_id("https://youtu.be/abc123?t=10"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn extracts_video_id_from_short_url_with_si_param() {
        assert_eq!(
            extract_video_id("https://youtu.be/h3sZG-hOYfQ?si=hQbRlLgdOxC28LG9"),
            Some("h3sZG-hOYfQ".to_string())
        );
    }

    #[test]
    fn normalize_youtube_url_from_watch_with_playlist_params() {
        assert_eq!(
            normalize_youtube_url(
                "https://www.youtube.com/watch?v=kfRTPB1ukMc&list=PLkVD6Kn1p6P4rv1f0eiHwd4PgTuMQOqC1"
            ),
            Some("https://www.youtube.com/watch?v=kfRTPB1ukMc".to_string())
        );
    }

    #[test]
    fn normalize_youtube_url_from_short_link_with_si_param() {
        assert_eq!(
            normalize_youtube_url("https://youtu.be/h3sZG-hOYfQ?si=hQbRlLgdOxC28LG9"),
            Some("https://www.youtube.com/watch?v=h3sZG-hOYfQ".to_string())
        );
    }

    #[test]
    fn normalize_youtube_url_from_music_watch_url() {
        assert_eq!(
            normalize_youtube_url("https://music.youtube.com/watch?v=kfRTPB1ukMc&si=LVBiIBUOCkvS64GM"),
            Some("https://www.youtube.com/watch?v=kfRTPB1ukMc".to_string())
        );
    }

    #[test]
    fn returns_none_for_unrecognized_url() {
        assert_eq!(extract_video_id("https://example.com/video"), None);
    }
}
