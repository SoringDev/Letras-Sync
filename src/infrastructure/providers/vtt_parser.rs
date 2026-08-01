use crate::domain::lyrics::LyricsLine;

/// Parseia o conteúdo bruto de um arquivo WebVTT e converte cada cue para o
/// modelo interno `LyricsLine`.
///
/// Função pura: sem I/O, sem efeitos colaterais. Tolerante a entradas
/// malformadas (retorna `Vec` vazia ou parcial, nunca dá panic).
pub fn parse(vtt_content: &str, music_id: &str) -> Vec<LyricsLine> {
    let mut lines: Vec<LyricsLine> = Vec::new();

    let mut cue_lines: Vec<String> = Vec::new();
    let mut current_start: Option<f64> = None;
    let mut current_end: Option<f64> = None;

    for raw_line in vtt_content.lines() {
        let line = raw_line.trim();

        if let Some((start, end)) = parse_timecode(line) {
            // Novo cue: finaliza o anterior se houver.
            flush_cue(
                &mut lines,
                &mut cue_lines,
                &mut current_start,
                &mut current_end,
                music_id,
            );
            current_start = Some(start);
            current_end = Some(end);
        } else if line.is_empty() {
            // Linha em branco encerra o cue atual.
            flush_cue(
                &mut lines,
                &mut cue_lines,
                &mut current_start,
                &mut current_end,
                music_id,
            );
        } else if current_start.is_some() {
            // Linha de texto pertencente ao cue atual.
            cue_lines.push(line.to_string());
        }
        // Linhas fora de um cue (cabeçalho WEBVTT, metadados, NOTE, etc.)
        // são ignoradas.
    }

    // Finaliza o último cue caso o arquivo não termine com linha em branco.
    flush_cue(
        &mut lines,
        &mut cue_lines,
        &mut current_start,
        &mut current_end,
        music_id,
    );

    lines
}

/// Finaliza o cue acumulado, adicionando uma `LyricsLine` ao resultado se o
/// texto for válido e não for duplicata do cue anterior.
fn flush_cue(
    lines: &mut Vec<LyricsLine>,
    cue_lines: &mut Vec<String>,
    current_start: &mut Option<f64>,
    current_end: &mut Option<f64>,
    music_id: &str,
) {
    let (start, end) = match (*current_start, *current_end) {
        (Some(s), Some(e)) => (s, e),
        _ => {
            cue_lines.clear();
            *current_start = None;
            *current_end = None;
            return;
        }
    };

    let text = cue_lines
        .iter()
        .map(|l| strip_tags(l))
        .collect::<Vec<_>>()
        .join(" ");
    let text = text.trim().to_string();

    cue_lines.clear();
    *current_start = None;
    *current_end = None;

    if text.is_empty() {
        return;
    }

    // Deduplica cues consecutivos com texto idêntico.
    if let Some(last) = lines.last()
        && last.text == text
    {
        return;
    }

    lines.push(LyricsLine {
        id: 0,
        music_id: music_id.to_string(),
        start_time: start,
        end_time: end,
        text,
    });
}

/// Tenta interpretar uma linha como um timecode de cue no formato
/// `HH:MM:SS.mmm --> HH:MM:SS.mmm` (ou `MM:SS.mmm --> MM:SS.mmm`).
/// Retorna `Some((start, end))` em segundos, ou `None` se não for um timecode.
fn parse_timecode(line: &str) -> Option<(f64, f64)> {
    let (left, right) = line.split_once("-->")?;
    // O lado direito pode conter settings (ex.: "align:start position:0%").
    let end_token = right.split_whitespace().next()?;
    let start = parse_timestamp(left.trim())?;
    let end = parse_timestamp(end_token)?;
    Some((start, end))
}

/// Converte um timestamp `HH:MM:SS.mmm` ou `MM:SS.mmm` em segundos (`f64`).
fn parse_timestamp(ts: &str) -> Option<f64> {
    let (time_part, millis_part) = match ts.split_once('.') {
        Some((t, m)) => (t, Some(m)),
        None => (ts, None),
    };

    let components: Vec<&str> = time_part.split(':').collect();
    let (hours, minutes, seconds) = match components.as_slice() {
        [h, m, s] => (
            h.parse::<f64>().ok()?,
            m.parse::<f64>().ok()?,
            s.parse::<f64>().ok()?,
        ),
        [m, s] => (0.0, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };

    let millis = match millis_part {
        Some(m) => m.parse::<f64>().ok()?,
        None => 0.0,
    };

    Some(hours * 3600.0 + minutes * 60.0 + seconds + millis / 1000.0)
}

/// Remove tags inline delimitadas por `<` e `>` (como `<c>`, `<b>`,
/// `<00:00:00.000>`) usando varredura simples de caracteres.
fn strip_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut inside_tag = false;

    for c in text.chars() {
        match c {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => result.push(c),
            _ => {}
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_two_cues_with_correct_timecodes() {
        let vtt = "WEBVTT\n\n\
00:00:01.000 --> 00:00:04.000\n\
Primeira linha\n\n\
00:00:05.500 --> 00:00:08.250\n\
Segunda linha\n";

        let lines = parse(vtt, "music-1");

        assert_eq!(lines.len(), 2);

        assert_eq!(lines[0].text, "Primeira linha");
        assert_eq!(lines[0].start_time, 1.0);
        assert_eq!(lines[0].end_time, 4.0);
        assert_eq!(lines[0].music_id, "music-1");
        assert_eq!(lines[0].id, 0);

        assert_eq!(lines[1].text, "Segunda linha");
        assert_eq!(lines[1].start_time, 5.5);
        assert_eq!(lines[1].end_time, 8.25);
    }

    #[test]
    fn deduplicates_consecutive_identical_text() {
        let vtt = "WEBVTT\n\n\
00:00:01.000 --> 00:00:02.000\n\
Mesmo texto\n\n\
00:00:02.000 --> 00:00:03.000\n\
Mesmo texto\n";

        let lines = parse(vtt, "music-1");

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Mesmo texto");
    }

    #[test]
    fn removes_inline_tags_and_timestamps() {
        let vtt = "WEBVTT\n\n\
00:00:01.000 --> 00:00:04.000\n\
<c>Olá<00:00:02.500> <b>mundo</b></c>\n";

        let lines = parse(vtt, "music-1");

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Olá mundo");
    }
}
