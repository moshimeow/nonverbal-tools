//! Real-model phrase latency check; no audio playback or user-history access.
use speak_pocket::{SpeechEngine, Voice, chunks, model_dir};
use std::{
    sync::{Arc, atomic::AtomicU64},
    time::Instant,
};
fn main() -> anyhow::Result<()> {
    let start = Instant::now();
    let engine = SpeechEngine::load(&model_dir())?;
    println!("Model load: {:.3}s", start.elapsed().as_secs_f64());
    let token = Arc::new(AtomicU64::new(1));
    engine.generate("Warm up!", Voice::Emma, 1.0, token.clone(), 1)?;
    for (voice, text) in [
        (
            Voice::Emma,
            "Hello! One moment please, I'm typing my reply. I use this app to speak while my voice recovers, and I would like the first sentence to start before the rest has finished generating. Thanks for waiting!",
        ),
        (
            Voice::Xiaoxiao,
            "你好！请稍等，我正在打字。我用这个应用说话，希望第一句话可以先播放，不用等到所有的内容都生成完毕。谢谢你的耐心。",
        ),
    ] {
        let start = Instant::now();
        let whole = engine.generate(text, voice, 1.0, token.clone(), 1)?;
        let baseline = start.elapsed().as_secs_f64();
        let phrases = chunks::split(text, voice);
        let start = Instant::now();
        let mut first = 0.0;
        let mut duration = 0.0;
        for (n, phrase) in phrases.iter().enumerate() {
            let chunk = engine.generate(phrase, voice, 1.0, token.clone(), 1)?;
            if n == 0 {
                first = start.elapsed().as_secs_f64();
            }
            duration += chunk.samples.len() as f64 / chunk.sample_rate as f64;
        }
        println!(
            "{}: whole {:.3}s ({:.1}s audio), first phrase {:.3}s, {} phrases total {:.3}s ({:.1}s audio)",
            voice.name(),
            baseline,
            whole.samples.len() as f64 / whole.sample_rate as f64,
            first,
            phrases.len(),
            start.elapsed().as_secs_f64(),
            duration
        );
        anyhow::ensure!(
            first < baseline,
            "First-phrase generation did not improve latency"
        );
    }
    Ok(())
}
