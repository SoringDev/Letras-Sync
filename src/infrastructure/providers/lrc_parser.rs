use crate::domain::lyrics::LyricsLine;

/// Parseia o conteúdo bruto de um arquivo LRC e converte cada linha de letra
/// sincronizada para o modelo interno `LyricsLine`.
///
/// Função pura: sem I/O, sem efeitos colaterais. Tolerante a entradas
/// malformadas (retorna `Vec` vazia ou parcial, nunca dá panic).
///
/// O LRC não possui `end_time`: usa-se o `start_time` do próximo cue como
/// `end_time` do atual. Para o último cue, usa-se `start_time + 5.0`.
pub fn parse(lrc_content: &str, music_id: &str) -> Vec<LyricsLine> {
    // Coleta os pares (start_time, texto) válidos.
    let mut cues: Vec<(f64, String)> = Vec::new();

    for raw_line in lrc_content.lines() {
        let line = raw_line.trim();

        if let Some((start, text)) = parse_line(line) {
            cues.push((start, text));
        }
    }

    let mut lines: Vec<LyricsLine> = Vec::with_capacity(cues.len());

    for i in 0..cues.len() {
        let (start, ref text) = cues[i];
        let end = if i + 1 < cues.len() {
            cues[i + 1].0
        } else {
            start + 5.0
        };

        lines.push(LyricsLine {
            id: 0,
            music_id: music_id.to_string(),
            start_time: start,
            end_time: end,
            text: text.clone(),
        });
    }

    lines
}

/// Tenta interpretar uma linha LRC no formato `[MM:SS.xx]texto`.
/// Retorna `Some((start_time, texto))` se a linha começar com um timecode
/// numérico válido e tiver texto não vazio. Retorna `None` para metadados
/// (`[ar:...]`, `[ti:...]`) ou linhas inválidas.
fn parse_line(line: &str) -> Option<(f64, String)> {
    if !line.starts_with('[') {
        return None;
    }

    let close = line.find(']')?;
    let timecode = &line[1..close];
    let text = line[close + 1..].trim();

    let start = parse_timecode(timecode)?;

    if text.is_empty() {
        return None;
    }

    Some((start, text.to_string()))
}

/// Converte um timecode LRC `MM:SS.xx` (centésimos de segundo) em segundos
/// (`f64`). Retorna `None` se o conteúdo não for um timecode numérico.
fn parse_timecode(tc: &str) -> Option<f64> {
    let (minutes_part, rest) = tc.split_once(':')?;

    let (seconds_part, centis_part) = match rest.split_once('.') {
        Some((s, c)) => (s, Some(c)),
        None => (rest, None),
    };

    let minutes = minutes_part.parse::<f64>().ok()?;
    let seconds = seconds_part.parse::<f64>().ok()?;
    let centis = match centis_part {
        Some(c) => c.parse::<f64>().ok()?,
        None => 0.0,
    };

    Some(minutes * 60.0 + seconds + centis / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_three_lines_with_end_time_from_next_start() {
        let lrc = "[00:01.00]Primeira linha\n\
[00:04.50]Segunda linha\n\
[00:08.00]Terceira linha\n";

        let lines = parse(lrc, "music-1");

        assert_eq!(lines.len(), 3);

        assert_eq!(lines[0].text, "Primeira linha");
        assert_eq!(lines[0].start_time, 1.0);
        assert_eq!(lines[0].end_time, 4.5);
        assert_eq!(lines[0].music_id, "music-1");
        assert_eq!(lines[0].id, 0);

        assert_eq!(lines[1].text, "Segunda linha");
        assert_eq!(lines[1].start_time, 4.5);
        assert_eq!(lines[1].end_time, 8.0);
    }

    #[test]
    fn ignores_metadata_tags() {
        let lrc = "[ar:Artista]\n\
[ti:Título]\n\
[00:01.00]Primeira linha\n";

        let lines = parse(lrc, "music-1");

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Primeira linha");
        assert_eq!(lines[0].start_time, 1.0);
    }

    #[test]
    fn last_line_gets_start_time_plus_five() {
        let lrc = "[00:10.00]Única linha\n";

        let lines = parse(lrc, "music-1");

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].start_time, 10.0);
        assert_eq!(lines[0].end_time, 15.0);
    }
}
