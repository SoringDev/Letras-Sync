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

/// Quebra um título típico do YouTube em segmentos candidatos ao nome da música real,
/// ordenados do mais provável para o menos provável.
pub fn extract_song_title_candidates(title: &str) -> Vec<String> {
    const NOISE_PATTERNS: &[&str] = &[
        "(lyrics)",
        "(lyric video)",
        "(official video)",
        "(official music video)",
        "(official audio)",
        "(audio)",
        "(visualizer)",
        "(clipe oficial)",
        "(clipe)",
        "(ao vivo)",
        "(live)",
        "(hd)",
        "(4k)",
        "[lyrics]",
        "[official]",
        "[hd]",
        "[ao vivo]",
        "[live]",
    ];

    fn is_numbering(s: &str) -> bool {
        let lower = s.to_lowercase();
        let prefixes = ["hino ", "hymn ", "track ", "faixa ", "música "];
        prefixes.iter().any(|p| lower.starts_with(p))
            && s.split_whitespace()
                .last()
                .is_some_and(|w| w.parse::<u32>().is_ok())
    }

    fn strip_noise_patterns(title: &str) -> String {
        let mut result = title.trim().to_string();

        fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
            let haystack_bytes = haystack.as_bytes();
            let needle_bytes = needle.as_bytes();

            if needle_bytes.is_empty() || needle_bytes.len() > haystack_bytes.len() {
                return None;
            }

            for start in 0..=haystack_bytes.len() - needle_bytes.len() {
                if haystack_bytes[start..start + needle_bytes.len()]
                    .iter()
                    .zip(needle_bytes)
                    .all(|(hay, nee)| hay.eq_ignore_ascii_case(nee))
                {
                    return Some(start);
                }
            }

            None
        }

        loop {
            let mut changed = false;

            for noise in NOISE_PATTERNS {
                if let Some(pos) = find_ascii_case_insensitive(&result, noise) {
                    result.replace_range(pos..pos + noise.len(), "");
                    changed = true;
                    break;
                }
            }

            if !changed {
                break;
            }
        }

        result.trim().to_string()
    }

    let cleaned = strip_noise_patterns(title);
    if cleaned.is_empty() {
        return Vec::new();
    }

    let mut segments: Vec<String> = Vec::new();

    for separator in ['•', '|', '—', '–'] {
        let parts: Vec<String> = cleaned
            .split(separator)
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(ToString::to_string)
            .collect();

        if parts.len() > 1 {
            segments = parts.into_iter().rev().collect();
            break;
        }
    }

    if segments.is_empty()
        && let Some((left, right)) = cleaned.split_once('-')
    {
        let left = left.trim();
        let right = right.trim();

        if !left.is_empty() && !right.is_empty() {
            segments = vec![left.to_string(), right.to_string()];
        }
    }

    if segments.is_empty() {
        segments.push(cleaned.clone());
    }

    let mut ordered = segments.into_iter().enumerate().collect::<Vec<_>>();
    ordered.sort_by(|(left_idx, left), (right_idx, right)| {
        match (is_numbering(left), is_numbering(right)) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => left_idx.cmp(right_idx),
        }
    });

    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (_, candidate) in ordered
        .into_iter()
        .chain(std::iter::once((usize::MAX, cleaned)))
    {
        let key = candidate.to_lowercase();
        if seen.insert(key) {
            candidates.push(candidate);
        }
    }

    candidates
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
            normalize_youtube_url(
                "https://music.youtube.com/watch?v=kfRTPB1ukMc&si=LVBiIBUOCkvS64GM"
            ),
            Some("https://www.youtube.com/watch?v=kfRTPB1ukMc".to_string())
        );
    }

    #[test]
    fn returns_none_for_unrecognized_url() {
        assert_eq!(extract_video_id("https://example.com/video"), None);
    }
}

#[cfg(test)]
mod tests_candidates {
    use super::*;

    #[test]
    fn extracts_song_name_from_bullet_separated_youtube_title() {
        let candidates = extract_song_title_candidates(
            "Novo Hinário Adventista • Hino 293 • O Sábado Chegou • (Lyrics)",
        );
        assert_eq!(candidates[0], "O Sábado Chegou");
    }

    #[test]
    fn extracts_from_dash_separated_title() {
        let candidates = extract_song_title_candidates("Hillsong - Oceans (Official Video)");
        assert!(candidates.iter().any(|c| c.contains("Oceans")));
    }

    #[test]
    fn single_segment_title_returns_itself() {
        let candidates = extract_song_title_candidates("Oceans");
        assert_eq!(candidates[0], "Oceans");
    }
}
