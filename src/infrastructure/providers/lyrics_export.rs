use std::fmt::Write;

use crate::domain::lyrics::LyricsLine;

/// Serializa linhas de letra sincronizada para LRC.
pub fn format_lrc(lines: &[LyricsLine]) -> String {
    let mut output = String::new();

    for line in lines {
        let _ = writeln!(
            output,
            "[{}]{}",
            format_lrc_time(line.start_time),
            line.text
        );
    }

    output
}

/// Serializa linhas de letra sincronizada para SRT.
pub fn format_srt(lines: &[LyricsLine]) -> String {
    let mut blocks = Vec::with_capacity(lines.len());

    for (index, line) in lines.iter().enumerate() {
        let mut block = String::new();
        let _ = writeln!(block, "{}", index + 1);
        let _ = writeln!(
            block,
            "{} --> {}",
            format_srt_time(line.start_time),
            format_srt_time(line.end_time)
        );
        block.push_str(&line.text);
        blocks.push(block);
    }

    if blocks.is_empty() {
        String::new()
    } else {
        format!("{}\n\n", blocks.join("\n\n"))
    }
}

fn format_lrc_time(seconds: f64) -> String {
    let total_centis = (seconds.max(0.0) * 100.0).round() as i64;
    let minutes = total_centis / 6_000;
    let remaining_centis = total_centis % 6_000;
    let secs = remaining_centis / 100;
    let centis = remaining_centis % 100;

    format!("{minutes:02}:{secs:02}.{centis:02}")
}

fn format_srt_time(seconds: f64) -> String {
    let total_millis = (seconds.max(0.0) * 1_000.0).round() as i64;
    let hours = total_millis / 3_600_000;
    let remaining_millis = total_millis % 3_600_000;
    let minutes = remaining_millis / 60_000;
    let remaining_millis = remaining_millis % 60_000;
    let secs = remaining_millis / 1_000;
    let millis = remaining_millis % 1_000;

    format!("{hours:02}:{minutes:02}:{secs:02},{millis:03}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(start_time: f64, end_time: f64, text: &str) -> LyricsLine {
        LyricsLine {
            id: 0,
            music_id: "music-1".to_string(),
            start_time,
            end_time,
            text: text.to_string(),
        }
    }

    #[test]
    fn formats_lrc_with_centisecond_precision() {
        let lines = vec![line(1.234, 2.0, "Primeira"), line(61.998, 64.0, "Segunda")];

        assert_eq!(
            format_lrc(&lines),
            "[00:01.23]Primeira\n[01:02.00]Segunda\n"
        );
    }

    #[test]
    fn formats_srt_with_sequential_numbers_and_timestamp_precision() {
        let lines = vec![
            line(1.234, 2.5, "Primeira"),
            line(3661.999, 3664.25, "Segunda"),
        ];

        assert_eq!(
            format_srt(&lines),
            "1\n00:00:01,234 --> 00:00:02,500\nPrimeira\n\n\
2\n01:01:01,999 --> 01:01:04,250\nSegunda\n\n"
        );
    }

    #[test]
    fn returns_empty_string_for_empty_inputs() {
        assert!(format_lrc(&[]).is_empty());
        assert!(format_srt(&[]).is_empty());
    }
}
