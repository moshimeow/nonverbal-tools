//! Generate reproducible Mandarin frontend comparisons, without playback/history.
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig,
    OfflineTtsModelConfig,
};
fn main() -> anyhow::Result<()> {
    let dir = speak_pocket::model_dir();
    let path = |s: &str| dir.join(s).to_string_lossy().into_owned();
    let config = OfflineTtsConfig {
        model: OfflineTtsModelConfig {
            kokoro: OfflineTtsKokoroModelConfig {
                model: Some(path("model.onnx")),
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
            debug: true,
            ..Default::default()
        },
        rule_fsts: Some(path("number-zh.fst")),
        ..Default::default()
    };
    let engine =
        OfflineTts::create(&config).ok_or_else(|| anyhow::anyhow!("Could not load model"))?;
    let input = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Usage: mandarin-check CASES.tsv OUTPUT_DIR"))?;
    let out = std::path::PathBuf::from(
        std::env::args()
            .nth(2)
            .ok_or_else(|| anyhow::anyhow!("Missing output directory"))?,
    );
    std::fs::create_dir_all(&out)?;
    let app_engine = speak_pocket::SpeechEngine::load(&dir)?;
    for (index, line) in std::fs::read_to_string(input)?.lines().enumerate() {
        let (name, text) = line
            .split_once('\t')
            .ok_or_else(|| anyhow::anyhow!("Expected label and text"))?;
        eprintln!("CASE {index} {name}");
        if name.starts_with("app") {
            let phrases = if name == "app-chunked" {
                speak_pocket::chunks::split(text, speak_pocket::Voice::Xiaoxiao)
            } else {
                vec![text.to_owned()]
            };
            let mut samples = Vec::new();
            let mut rate = 24000;
            for phrase in phrases {
                let audio = app_engine.generate(
                    &phrase,
                    speak_pocket::Voice::Xiaoxiao,
                    1.0,
                    std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
                    1,
                )?;
                rate = audio.sample_rate;
                samples.extend(audio.samples);
            }
            let file = out.join(format!("{index}.wav"));
            anyhow::ensure!(
                sherpa_onnx::write(file.to_str().unwrap(), &samples, rate as i32),
                "Cannot save audio"
            );
            println!(
                "{name}: {:.3}s -> {}",
                samples.len() as f64 / rate as f64,
                file.display()
            );
            continue;
        }
        let g = GenerationConfig {
            sid: 47,
            speed: 1.0,
            phoneme_codepoints: name
                .starts_with("phones")
                .then(|| vec![text.chars().map(|c| c as i32).collect()]),
            ..Default::default()
        };
        let audio = engine
            .generate_with_config(text, &g, None::<fn(&[f32], f32) -> bool>)
            .ok_or_else(|| anyhow::anyhow!("No audio"))?;
        anyhow::ensure!(
            !audio.samples().is_empty() && audio.samples().iter().all(|s| s.is_finite()),
            "Invalid audio"
        );
        let file = out.join(format!("{index}.wav"));
        anyhow::ensure!(audio.save(file.to_str().unwrap()), "Cannot save audio");
        println!(
            "{name}: {:.3}s -> {}",
            audio.samples().len() as f64 / audio.sample_rate() as f64,
            file.display()
        );
    }
    Ok(())
}
