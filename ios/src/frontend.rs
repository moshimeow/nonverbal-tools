//! English pronunciation, including names, typos, and invented words, offline.
use anyhow::Result;
use loqui_g2p::{Dialect, G2p, OovFallback};

pub struct English(G2p);
impl English {
    pub fn new() -> Result<Self> {
        Ok(Self(G2p::new(Dialect::British, OovFallback::Neural)?))
    }

    pub fn phonemes(&self, text: &str) -> Vec<Vec<i32>> {
        let mut chunks = Vec::new();
        let mut chunk = Vec::new();
        for token in self.0.tokens(&text.replace('？', "?")) {
            let mut phones = acronym(&token.text).unwrap_or(token.phonemes);
            let question = phones.contains('?');
            // Even a word beyond the fallback model's input limit gets spoken.
            if phones.is_empty() && token.text.chars().any(|c| c.is_ascii_alphabetic()) {
                let spelled = token
                    .text
                    .chars()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                phones = self.0.phonemize(&spelled);
            }
            let mut word: Vec<i32> = phones.chars().map(|c| c as i32).collect();
            if token.whitespace {
                word.push(' ' as i32);
            }
            if chunk.len() + word.len() > 400 && !chunk.is_empty() {
                chunks.push(std::mem::take(&mut chunk));
            }
            for cp in word {
                chunk.push(cp);
                if chunk.len() == 400 {
                    chunks.push(std::mem::take(&mut chunk));
                }
            }
            // A following statement must not determine a question's final contour.
            if question && !chunk.is_empty() {
                chunks.push(std::mem::take(&mut chunk));
            }
        }
        if !chunk.is_empty() {
            chunks.push(chunk);
        }
        // Closing quotes/punctuation alone are not valid synthesis segments.
        chunks.retain(|chunk| {
            chunk
                .iter()
                .any(|&cp| char::from_u32(cp as u32).is_some_and(char::is_alphabetic))
        });
        chunks
    }
}

// British letter names in Kokoro's Misaki phoneme alphabet. A single I/A stays a word.
fn acronym(word: &str) -> Option<String> {
    if word.len() < 2 || !word.bytes().all(|b| b.is_ascii_uppercase()) {
        return None;
    }
    const LETTERS: [&str; 26] = [
        "ˈA",
        "bˈiː",
        "sˈiː",
        "dˈiː",
        "ˈiː",
        "ˈɛf",
        "ʤˈiː",
        "ˈAʧ",
        "ˈI",
        "ʤˈA",
        "kˈA",
        "ˈɛl",
        "ˈɛm",
        "ˈɛn",
        "ˈQ",
        "pˈiː",
        "kjˈuː",
        "ˈɑː",
        "ˈɛs",
        "tˈiː",
        "jˈuː",
        "vˈiː",
        "dˈʌbᵊljuː",
        "ˈɛks",
        "wˈI",
        "zˈɛd",
    ];
    Some(
        word.bytes()
            .map(|b| LETTERS[(b - b'A') as usize])
            .collect::<Vec<_>>()
            .join(" "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invented_words_are_pronounced_and_long_text_is_chunked() {
        let english = English::new().unwrap();
        for word in ["zorbulate", "floofiness", "henlo", "blorptastic"] {
            let phones = english.phonemes(word);
            assert!(!phones.is_empty(), "{word} was dropped");
            assert!(phones.iter().flatten().any(|&c| c != ' ' as i32));
            println!(
                "{word}: {}",
                phones
                    .iter()
                    .flatten()
                    .filter_map(|&c| char::from_u32(c as u32))
                    .collect::<String>()
            );
        }
        let chunks = english.phonemes(&"Hello world. ".repeat(150));
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.len() <= 400));
    }
    #[test]
    fn uppercase_words_spell_out_and_questions_keep_their_boundary() {
        let english = English::new().unwrap();
        let to_text = |chunks: Vec<Vec<i32>>| {
            chunks
                .into_iter()
                .flatten()
                .filter_map(|c| char::from_u32(c as u32))
                .collect::<String>()
        };
        for word in ["NASA", "CPU", "API", "LOL"] {
            assert!(
                to_text(english.phonemes(word)).contains(&acronym(word).unwrap()),
                "{word}"
            );
        }
        assert_eq!(acronym("I"), None);
        assert_eq!(acronym("Hello"), None);
        let chunks = english.phonemes("Are you coming? I can wait.");
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].contains(&('?' as i32)));
        assert!(!chunks[1].contains(&('?' as i32)));
        assert!(to_text(english.phonemes("Really？")).contains('?'));
    }
}
