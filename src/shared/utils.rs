/// Extrai o `video_id` de uma URL do YouTube.
///
/// Suporta o parâmetro `v=` da query string e o formato curto `youtu.be/<id>`.
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
    fn returns_none_for_unrecognized_url() {
        assert_eq!(extract_video_id("https://example.com/video"), None);
    }
}
