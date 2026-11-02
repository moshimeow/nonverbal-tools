//! Kokoro v1.0 Mandarin IPA with explicit word boundaries. Bypasses sherpa's
//! lexicon path, which appends trailing punctuation after its terminal PAD.
use anyhow::{Context, Result, ensure};
use jieba_rs::Jieba;
use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

pub struct Mandarin {
    words: HashMap<String, String>,
    segmenter: Jieba,
}
impl Mandarin {
    pub fn load(dir: &Path) -> Result<Self> {
        let words = std::fs::read_to_string(dir.join("lexicon-zh.txt"))?
            .lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                Some((parts.next()?.to_owned(), parts.collect::<String>()))
            })
            .collect();
        let mut dict = BufReader::new(File::open(dir.join("dict/jieba.dict.utf8"))?);
        let segmenter =
            Jieba::with_dict(&mut dict).context("Could not load Mandarin word segmentation")?;
        Ok(Self { words, segmenter })
    }
    pub fn phonemes(
        &self,
        text: &str,
        english: &crate::frontend::English,
    ) -> Result<Vec<Vec<i32>>> {
        let normalized = normalize(text);
        let mut phones = String::new();
        for token in self.segmenter.cut(&normalized, true) {
            let word = token.word;
            if word.trim().is_empty() {
                continue;
            }
            if word.chars().all(|c| ",.!?;:—…\"()“”".contains(c)) {
                while phones.ends_with(' ') {
                    phones.pop();
                }
                phones.push_str(word);
                phones.push(' ');
                continue;
            }
            let pronunciation = if let Some(p) = self.words.get(word) {
                p.clone()
            } else if word.chars().all(is_han) {
                let mut p = String::new();
                for c in word.chars() {
                    p.push_str(
                        self.words
                            .get(&c.to_string())
                            .with_context(|| format!("No Mandarin pronunciation for {c}"))?,
                    );
                }
                p
            } else if word.chars().any(|c| c.is_ascii_alphabetic()) {
                english
                    .phonemes(word)
                    .into_iter()
                    .flatten()
                    .filter_map(|c| char::from_u32(c as u32))
                    .collect()
            } else {
                continue;
            };
            if !phones.is_empty() && !phones.ends_with(' ') {
                phones.push(' ');
            }
            phones.push_str(&pronunciation);
        }
        // Segment on word boundaries within the same phoneme budget as English.
        let mut result = Vec::new();
        let mut current = Vec::new();
        for word in phones.split_whitespace() {
            let chars: Vec<i32> = word.chars().map(|c| c as i32).collect();
            if !current.is_empty() && current.len() + chars.len() + 1 > 400 {
                result.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ' as i32);
            }
            for c in chars {
                current.push(c);
                if current.len() == 400 {
                    result.push(std::mem::take(&mut current));
                }
            }
        }
        if !current.is_empty() {
            result.push(current);
        }
        ensure!(
            result
                .iter()
                .flatten()
                .any(|&c| char::from_u32(c as u32).is_some_and(char::is_alphabetic)),
            "No pronounceable Mandarin text"
        );
        Ok(result)
    }
}
fn is_han(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x9fff | 0x20000..=0x3134f)
}

// Common numeric forms are normalized before phonemization. Long digit strings
// (phone numbers/IDs) and years are read digit by digit, not as large cardinals.
fn normalize(text: &str) -> String {
    let chars: Vec<_> = text
        .chars()
        .map(|c| match c {
            '，' | '、' => ',',
            '。' | '．' => '.',
            '？' => '?',
            '！' => '!',
            '：' => ':',
            '；' => ';',
            '（' => '(',
            '）' => ')',
            '％' => '%',
            '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap(),
            _ => c,
        })
        .collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        let digits: String = chars[start..i].iter().collect();
        let year = chars.get(i) == Some(&'年');
        let mut spoken =
            if year || digits.len() >= 7 || (digits.len() > 1 && digits.starts_with('0')) {
                digit_names(&digits)
            } else {
                cardinal(digits.parse().unwrap())
            };
        if chars.get(i) == Some(&'.') && chars.get(i + 1).is_some_and(char::is_ascii_digit) {
            i += 1;
            let begin = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            spoken.push('点');
            spoken.push_str(&digit_names(&chars[begin..i].iter().collect::<String>()));
        } else if chars.get(i) == Some(&':') && digits.parse::<u32>().is_ok_and(|h| h < 24) {
            let count = chars[i + 1..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .count();
            if (1..=2).contains(&count) {
                let minute: String = chars[i + 1..i + 1 + count].iter().collect();
                if let Ok(m) = minute.parse::<u32>()
                    && m < 60
                {
                    i += 1 + count;
                    spoken.push('点');
                    if m > 0 {
                        spoken.push_str(&cardinal(m));
                        spoken.push('分');
                    }
                }
            }
        }
        if chars.get(i) == Some(&'%') {
            out.push_str("百分之");
            i += 1;
        }
        out.push_str(&spoken);
    }
    out
}
fn digit_names(s: &str) -> String {
    const D: [char; 10] = ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
    s.bytes().map(|b| D[(b - b'0') as usize]).collect()
}
fn cardinal(n: u32) -> String {
    if n == 0 {
        return "零".into();
    }
    if n >= 10_000 {
        let high = cardinal(n / 10_000);
        let rest = n % 10_000;
        return format!(
            "{high}万{}{}",
            if rest > 0 && rest < 1000 { "零" } else { "" },
            if rest > 0 {
                cardinal(rest)
            } else {
                String::new()
            }
        );
    }
    let mut out = String::new();
    let mut zero = false;
    for (unit, label) in [(1000, "千"), (100, "百"), (10, "十"), (1, "")] {
        let digit = n / unit % 10;
        if digit == 0 {
            if !out.is_empty() {
                zero = true;
            }
            continue;
        }
        if zero {
            out.push('零');
            zero = false;
        }
        if !(unit == 10 && digit == 1 && out.is_empty()) {
            out.push_str(&digit_names(&digit.to_string()));
        }
        out.push_str(label);
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn common_numeric_forms_and_punctuation() {
        assert_eq!(
            normalize("2026年11月1日，下午3:15，12.5%，电话18601200909。"),
            "二零二六年十一月一日,下午三点十五分,百分之十二点五,电话一八六零一二零零九零九."
        );
        assert_eq!(cardinal(10001), "一万零一");
        assert_eq!(cardinal(105), "一百零五");
        assert_eq!(normalize("０１，３。"), "零一,三.");
    }
    #[test]
    fn sample_keeps_lexical_tones_word_boundaries_and_one_question_marker() {
        let frontend = Mandarin::load(&crate::model_dir()).unwrap();
        let english = crate::frontend::English::new().unwrap();
        let text = "你好,你怎么样了? 你看,小猫从椅子调到地上.";
        let chunks = frontend.phonemes(text, &english).unwrap();
        let phones: String = chunks
            .into_iter()
            .flatten()
            .filter_map(|c| char::from_u32(c as u32))
            .collect();
        assert_eq!(
            phones,
            "ni↓xau↓, ni↓ ʦə↓nmɤja↘ŋ lɤ? ni↓ kʰa↘n, ɕjau↓mau→ ʦʰʊ↗ŋ i↓ʦɨ tjau↘ tau↘ ti↘ʂa↘ŋ."
        );
        assert_eq!(phones.matches('?').count(), 1);
        assert!(!phones.contains('\0'));
        assert!(frontend.phonemes("？！", &english).is_err());
        assert!(frontend.phonemes("现在是3:15。", &english).is_ok());
    }
}
