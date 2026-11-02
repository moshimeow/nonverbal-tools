//! Bounded, deterministic phrases. Draft preparation and playback use identical
//! boundaries, so only exact text/voice/speed matches can reuse prepared audio.
use crate::Voice;
pub fn split(text: &str, voice: Voice) -> Vec<String> {
    let limit = if voice == Voice::Xiaoxiao { 60 } else { 160 };
    let mut rest = text.trim();
    let mut result = Vec::new();
    while !rest.is_empty() {
        let mut end = rest.len();
        let mut space = None;
        for (n, (i, c)) in rest.char_indices().take(limit).enumerate() {
            if c.is_whitespace() {
                space = Some(i);
            }
            let next = rest[i + c.len_utf8()..].chars().next();
            // Keep decimals and abbreviations within their phrase. Prefer full
            // sentences, then clauses once they have enough context for prosody.
            let sentence = matches!(c, '?' | '!' | '。' | '？' | '！')
                || (c == '.'
                    && next.is_none_or(char::is_whitespace)
                    && n >= 12
                    && !rest[..i].ends_with(" Mr")
                    && !rest[..i].ends_with(" Dr"));
            let clause = n >= limit / 2 && matches!(c, ',' | ';' | ':' | '，' | '；' | '：');
            if sentence || clause {
                end = i + c.len_utf8();
                break;
            }
            if n + 1 >= limit {
                end = space.filter(|&p| p > 0).unwrap_or(i + c.len_utf8());
                break;
            }
        }
        let phrase = rest[..end].trim();
        if phrase.chars().any(char::is_alphanumeric) {
            result.push(phrase.to_owned());
        }
        rest = rest[end..].trim_start();
    }
    result
}
/// Prepare only a settled first chunk, with three non-space characters beyond it.
/// Use the playback splitter so punctuation, decimals, and length limits agree.
pub fn settled_prefix(text: &str, voice: Voice) -> Option<String> {
    let text = text.trim();
    let first = split(text, voice).into_iter().next()?;
    let tail = text.strip_prefix(&first)?;
    (tail.chars().filter(|c| !c.is_whitespace()).take(3).count() == 3).then_some(first)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn speculation_waits_for_stable_playback_boundary() {
        for text in [
            "Hello",
            "Hello!",
            "Hello! a",
            "Hello! ab",
            "A price of 3.14 pounds",
        ] {
            assert_eq!(settled_prefix(text, Voice::Emma), None, "{text}");
        }
        assert_eq!(
            settled_prefix("Hello! abc", Voice::Emma).as_deref(),
            Some("Hello!")
        );
        assert_eq!(
            settled_prefix("Hello! abcdef", Voice::Emma).as_deref(),
            Some("Hello!")
        );
        assert_eq!(
            settled_prefix("你好！现在呢", Voice::Xiaoxiao).as_deref(),
            Some("你好！")
        );
        let long = "a word ".repeat(40);
        assert_eq!(
            settled_prefix(&long, Voice::Emma),
            split(&long, Voice::Emma).first().cloned()
        );
    }
    #[test]
    fn boundaries_preserve_words_numbers_and_unicode() {
        let s = "This costs 3.14 pounds. Are you coming? I can wait!";
        assert_eq!(
            split(s, Voice::Emma),
            ["This costs 3.14 pounds.", "Are you coming?", "I can wait!"]
        );
        let text = "你好！现在几点？我想喝水。";
        assert_eq!(split(text, Voice::Xiaoxiao).concat(), text);
        let long = "a word and another word ".repeat(100);
        let chunks = split(&long, Voice::Emma);
        assert!(chunks.iter().all(|c| c.chars().count() <= 160));
        assert_eq!(chunks.join(" "), long.trim());
        assert!(split("?!", Voice::Emma).is_empty());
    }
}
