pub mod chunks;
pub mod output;
use anyhow::{Context, Result, bail};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig,
    OfflineTtsModelConfig,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

mod frontend;
pub mod history;
mod mandarin;
pub mod platform;
pub mod worker;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    Emma,
    Xiaoxiao,
}

impl Voice {
    pub fn name(self) -> &'static str {
        match self {
            Self::Emma => "Emma · British",
            Self::Xiaoxiao => "晓晓 · 普通话",
        }
    }
    pub fn speaker_id(self) -> i32 {
        // Official sherpa Kokoro v1.0 voice table (not v1.1).
        match self {
            Self::Emma => 21,
            Self::Xiaoxiao => 47,
        }
    }
}

pub fn model_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("SPEAK_MODEL_DIR") {
        return path.into();
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        for bundled in [parent.join("kokoro"), parent.join("../Resources/kokoro")] {
            if bundled.is_dir() {
                return bundled;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/kokoro-multi-lang-v1_0")
}

pub struct SpeechEngine {
    tts: OfflineTts,
    english: frontend::English,
    mandarin: mandarin::Mandarin,
}

#[derive(Clone)]
pub struct Audio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl SpeechEngine {
    pub fn load(dir: &Path) -> Result<Self> {
        let model_name = "model.onnx";
        for name in [
            model_name,
            "voices.bin",
            "tokens.txt",
            "lexicon-gb-en.txt",
            "lexicon-zh.txt",
            "dict/jieba.dict.utf8",
            "number-zh.fst",
        ] {
            if !dir.join(name).is_file() {
                bail!("Missing {name}. Run ./ios.sh setup before building.");
            }
        }
        let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                kokoro: OfflineTtsKokoroModelConfig {
                    model: Some(path(model_name)),
                    voices: Some(path("voices.bin")),
                    tokens: Some(path("tokens.txt")),
                    lexicon: Some(format!(
                        "{},{}",
                        path("lexicon-gb-en.txt"),
                        path("lexicon-zh.txt")
                    )),
                    dict_dir: Some(path("dict")),
                    ..Default::default()
                },
                num_threads: 2,
                ..Default::default()
            },
            rule_fsts: Some(path("number-zh.fst")),
            ..Default::default()
        };
        let tts = OfflineTts::create(&config).context("Could not load Kokoro")?;
        if tts.num_speakers() <= Voice::Xiaoxiao.speaker_id() {
            bail!("Wrong voice table: expected Kokoro v1.0 bilingual voices");
        }
        Ok(Self {
            tts,
            english: frontend::English::new()?,
            mandarin: mandarin::Mandarin::load(dir)?,
        })
    }

    pub fn generate(
        &self,
        text: &str,
        voice: Voice,
        speed: f32,
        generation: Arc<AtomicU64>,
        id: u64,
    ) -> Result<Audio> {
        validate_text(text)?;
        if !speed.is_finite() || !(0.5..=2.0).contains(&speed) {
            bail!("Speed must be between 0.5 and 2.0");
        }
        let config = GenerationConfig {
            sid: voice.speaker_id(),
            phoneme_codepoints: Some(match voice {
                Voice::Emma => self.english.phonemes(text),
                Voice::Xiaoxiao => self.mandarin.phonemes(text, &self.english)?,
            }),
            speed,
            ..Default::default()
        };
        let audio = self
            .tts
            .generate_with_config(
                text,
                &config,
                Some(move |_: &[f32], _: f32| generation.load(Ordering::SeqCst) == id),
            )
            .context("Speech generation failed")?;
        let samples = audio.samples().to_vec();
        if samples.is_empty() || !samples.iter().all(|s| s.is_finite()) || audio.sample_rate() <= 0
        {
            bail!(
                "Kokoro returned invalid audio ({} samples, {} non-finite, {} Hz)",
                samples.len(),
                samples.iter().filter(|s| !s.is_finite()).count(),
                audio.sample_rate()
            );
        }
        Ok(Audio {
            samples,
            sample_rate: audio.sample_rate() as u32,
        })
    }
}

pub fn validate_text(text: &str) -> Result<()> {
    if text.trim().is_empty() {
        bail!("Type something first");
    }
    // The native API uses C strings; reject NUL instead of letting its wrapper panic.
    if text.contains('\0') {
        bail!("Text contains an unsupported NUL character");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_chinese_and_rejects_unsafe_input() {
        assert!(validate_text("你好！现在是下午三点。").is_ok());
        assert!(validate_text("   ").is_err());
        assert!(validate_text("hello\0world").is_err());
        assert!(validate_text(&"你".repeat(2001)).is_ok());
        assert!(validate_text(&"你".repeat(2000)).is_ok());
    }
}
