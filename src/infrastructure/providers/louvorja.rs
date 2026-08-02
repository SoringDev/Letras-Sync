use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::domain::lyrics::LyricsLine;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CatalogEntry {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct CatalogPageResponse {
    data: Vec<CatalogPageItem>,
    last_page: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct CatalogPageItem {
    id_music: CatalogIdMusic,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CatalogIdMusic {
    String(String),
    Number(i64),
    Unsigned(u64),
}

#[derive(Debug, Deserialize)]
struct MusicResponse {
    data: MusicDetail,
}

#[derive(Debug, Deserialize)]
struct MusicDetail {
    url_music: Option<String>,
    lyric: Option<Vec<LyricItem>>,
}

#[derive(Debug, Deserialize)]
struct LyricItem {
    lyric: Option<String>,
    time: String,
}

pub struct LouvorJaProvider;

impl LouvorJaProvider {
    pub fn new() -> Self {
        Self
    }

    pub async fn fetch_synced_lyrics(
        &self,
        query: &str,
        music_id: &str,
        cache_path: &Path,
    ) -> Result<Option<Vec<LyricsLine>>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(None);
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        let catalog = load_or_fetch_catalog(&client, cache_path).await?;

        let Some(entry) = find_best_match(&catalog, query) else {
            tracing::info!("louvorja: nenhum match para '{query}' no catálogo");
            return Ok(None);
        };

        tracing::info!(
            "louvorja: match encontrado '{}' (id: {})",
            entry.name,
            entry.id
        );

        let detail_url = format!("https://api.louvorja.com.br/pt/musics/{}", entry.id);
        let detail: MusicResponse = client
            .get(detail_url)
            .send()
            .await
            .context("falha ao consultar o detalhe do LouvorJA")?
            .error_for_status()
            .context("LouvorJA retornou erro no detalhe")?
            .json()
            .await
            .context("falha ao desserializar o detalhe do LouvorJA")?;

        let Some(lyrics) = detail.data.lyric else {
            return Ok(None);
        };

        let lines = build_lines(music_id, lyrics);
        if lines.is_empty() {
            Ok(None)
        } else {
            if let Some(audio_url) = detail.data.url_music.as_deref().map(str::trim)
                && !audio_url.is_empty()
                && let Err(e) =
                    download_official_audio(&client, cache_path, music_id, audio_url).await
            {
                tracing::warn!("louvorja: falha ao baixar áudio oficial: {e}");
            }

            Ok(Some(lines))
        }
    }
}

async fn download_official_audio(
    client: &reqwest::Client,
    cache_path: &Path,
    music_id: &str,
    audio_url: &str,
) -> Result<()> {
    let destination = cache_path.join(format!("{music_id}.mp3"));

    let bytes = client
        .get(audio_url)
        .send()
        .await
        .context("louvorja: falha ao baixar áudio oficial")?
        .error_for_status()
        .context("louvorja: áudio oficial retornou erro")?
        .bytes()
        .await
        .context("louvorja: falha ao ler bytes do áudio oficial")?;

    if !cache_path.as_os_str().is_empty() {
        tokio::fs::create_dir_all(cache_path).await.ok();
    }

    tokio::fs::write(&destination, bytes)
        .await
        .with_context(|| {
            format!(
                "louvorja: falha ao salvar áudio oficial em {}",
                destination.display()
            )
        })?;

    remove_previous_audio_cache_files(cache_path, music_id, &destination).await;

    tracing::info!(
        "louvorja: áudio oficial baixado com sucesso em {}",
        destination.display()
    );
    Ok(())
}

async fn remove_previous_audio_cache_files(cache_path: &Path, music_id: &str, keep: &Path) {
    for extension in ["webm", "m4a"] {
        let candidate = cache_path.join(format!("{music_id}.{extension}"));
        if candidate == keep {
            continue;
        }
        let _ = tokio::fs::remove_file(&candidate).await;
    }
}

async fn load_or_fetch_catalog(
    client: &reqwest::Client,
    cache_path: &Path,
) -> Result<Vec<CatalogEntry>> {
    let catalog_file = cache_path.join("louvorja_catalog.json");

    let needs_refresh = if catalog_file.exists() {
        let metadata = tokio::fs::metadata(&catalog_file).await?;
        let age = metadata.modified()?.elapsed().unwrap_or(Duration::MAX);
        age > Duration::from_secs(60 * 60 * 24 * 7)
    } else {
        true
    };

    if !needs_refresh {
        let content = tokio::fs::read_to_string(&catalog_file).await?;
        let entries: Vec<CatalogEntry> = serde_json::from_str(&content)?;
        tracing::info!(
            "louvorja: catálogo local carregado ({} músicas)",
            entries.len()
        );
        return Ok(entries);
    }

    tracing::info!("louvorja: baixando catálogo completo...");

    let mut all_entries: Vec<CatalogEntry> = Vec::new();
    let mut page = 1u32;

    loop {
        let page_value = page.to_string();
        let response: CatalogPageResponse = client
            .get("https://api.louvorja.com.br/pt/musics")
            .query(&[("page", page_value.as_str())])
            .send()
            .await
            .context("louvorja: falha ao baixar página do catálogo")?
            .error_for_status()
            .context("louvorja: erro ao baixar página do catálogo")?
            .json()
            .await
            .context("louvorja: falha ao desserializar página do catálogo")?;

        for item in response.data {
            let id = item.id_music.into_string();
            let name = item.name.trim().to_string();
            if !name.is_empty() {
                all_entries.push(CatalogEntry { id, name });
            }
        }

        let last_page = response.last_page.unwrap_or(1);
        if page >= last_page {
            break;
        }
        page += 1;
    }

    tracing::info!(
        "louvorja: catálogo baixado com {} músicas",
        all_entries.len()
    );

    if !cache_path.as_os_str().is_empty() {
        tokio::fs::create_dir_all(cache_path).await.ok();
        let json = serde_json::to_string(&all_entries)?;
        tokio::fs::write(&catalog_file, json).await.ok();
    }

    Ok(all_entries)
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .replace(['á', 'à', 'ã', 'â'], "a")
        .replace(['é', 'ê'], "e")
        .replace(['í'], "i")
        .replace(['ó', 'õ', 'ô'], "o")
        .replace(['ú'], "u")
        .replace(['ç'], "c")
}

fn find_best_match<'a>(catalog: &'a [CatalogEntry], query: &str) -> Option<&'a CatalogEntry> {
    let q = normalize(query);

    if let Some(entry) = catalog.iter().find(|entry| normalize(&entry.name) == q) {
        return Some(entry);
    }

    catalog.iter().find(|entry| {
        let n = normalize(&entry.name);
        n.contains(&q) || q.contains(&n)
    })
}

fn parse_time(time: &str) -> Option<f64> {
    let mut parts = time.split(':');
    let hours = parts.next()?.parse::<u64>().ok()?;
    let minutes = parts.next()?.parse::<u64>().ok()?;
    let seconds = parts.next()?.parse::<u64>().ok()?;

    if parts.next().is_some() {
        return None;
    }

    Some((hours * 3600 + minutes * 60 + seconds) as f64)
}

fn build_lines(music_id: &str, items: Vec<LyricItem>) -> Vec<LyricsLine> {
    let mut lines: Vec<LyricsLine> = items
        .into_iter()
        .filter_map(|item| {
            let text = item.lyric.unwrap_or_default().trim().to_string();
            if text.is_empty() {
                return None;
            }

            let start_time = parse_time(&item.time)?;
            Some(LyricsLine {
                id: 0,
                music_id: music_id.to_string(),
                start_time,
                end_time: start_time,
                text,
            })
        })
        .collect();

    for idx in 0..lines.len() {
        let next_start = lines.get(idx + 1).map(|line| line.start_time);
        lines[idx].end_time = next_start.unwrap_or(lines[idx].start_time + 5.0);
    }

    lines
}

impl CatalogIdMusic {
    fn into_string(self) -> String {
        match self {
            CatalogIdMusic::String(value) => value,
            CatalogIdMusic::Number(value) => value.to_string(),
            CatalogIdMusic::Unsigned(value) => value.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_time_converts_hms_to_seconds() {
        assert_eq!(parse_time("00:01:14"), Some(74.0));
        assert_eq!(parse_time("01:00:00"), Some(3600.0));
    }

    #[test]
    fn parse_time_rejects_invalid_input() {
        assert_eq!(parse_time("74"), None);
        assert_eq!(parse_time("00:01"), None);
    }

    #[test]
    fn filters_empty_lyrics() {
        let lines = build_lines(
            "m1",
            vec![LyricItem {
                lyric: Some("".to_string()),
                time: "00:01:14".to_string(),
            }],
        );

        assert!(lines.is_empty());
    }

    #[test]
    fn build_lines_sets_end_time_from_next_line() {
        let lines = build_lines(
            "m1",
            vec![
                LyricItem {
                    lyric: Some("Primeira".to_string()),
                    time: "00:00:10".to_string(),
                },
                LyricItem {
                    lyric: Some("Segunda".to_string()),
                    time: "00:00:14".to_string(),
                },
            ],
        );

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_time, 10.0);
        assert_eq!(lines[0].end_time, 14.0);
        assert_eq!(lines[1].start_time, 14.0);
        assert_eq!(lines[1].end_time, 19.0);
        assert_eq!(lines[0].music_id, "m1");
    }

    #[test]
    fn normalize_removes_accents() {
        assert_eq!(normalize("O Sábado Chegou"), "o sabado chegou");
    }

    #[test]
    fn find_best_match_prefers_exact_match() {
        let catalog = vec![
            CatalogEntry {
                id: "1".to_string(),
                name: "Novo Hinário Adventista".to_string(),
            },
            CatalogEntry {
                id: "2".to_string(),
                name: "O Sábado Chegou".to_string(),
            },
        ];

        let entry = find_best_match(&catalog, "O Sábado Chegou").expect("match");
        assert_eq!(entry.id, "2");
    }

    #[test]
    fn find_best_match_allows_containment() {
        let catalog = vec![CatalogEntry {
            id: "2".to_string(),
            name: "O Sábado Chegou".to_string(),
        }];

        let entry = find_best_match(&catalog, "O Sábado Chegou (Lyrics)").expect("match");
        assert_eq!(entry.id, "2");
    }

    #[test]
    fn deserializes_detail_response_with_nested_data_and_lyric_field() {
        let json = r#"
        {
            "data": {
                "id_music": 794,
                "name": "O Sábado Chegou",
                "url_music": "https://example.com/o-sabado-chegou.mp3",
                "lyric": [
                    {
                        "id_lyric": 10909,
                        "lyric": "Lento e calmo foge o dia",
                        "time": "00:00:24"
                    }
                ]
            }
        }
        "#;

        let response: MusicResponse = serde_json::from_str(json).expect("desserializar");
        assert_eq!(
            response.data.url_music.as_deref(),
            Some("https://example.com/o-sabado-chegou.mp3")
        );
        let Some(lyrics) = response.data.lyric else {
            panic!("lyric ausente");
        };

        assert_eq!(lyrics.len(), 1);
        assert_eq!(lyrics[0].lyric.as_deref(), Some("Lento e calmo foge o dia"));
        assert_eq!(lyrics[0].time, "00:00:24");
    }
}
