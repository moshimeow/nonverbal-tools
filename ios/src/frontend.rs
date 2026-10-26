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
        for token in self.0.tokens(text) {
            let mut phones = token.phonemes;
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
        }
        if !chunk.is_empty() {
            chunks.push(chunk);
        }
        chunks
    }
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
}
