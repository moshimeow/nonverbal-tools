use speak_pocket::{SpeechEngine, Voice, model_dir};
use std::sync::{Arc, atomic::AtomicU64};

fn main() -> anyhow::Result<()> {
    let engine = SpeechEngine::load(&model_dir())?;
    for (voice, text) in [
        (
            Voice::Emma,
            "Hello! One moment please, I'm typing my reply.",
        ),
        (
            Voice::Emma,
            "Henlo! This blorptastic floof wants to zorbulate.",
        ),
        (
            Voice::Xiaoxiao,
            "你好！我现在可以用中文说话了。现在是下午三点。",
        ),
    ] {
        let start = std::time::Instant::now();
        let audio = engine.generate(text, voice, 1.0, Arc::new(AtomicU64::new(0)), 0)?;
        anyhow::ensure!(
            audio.samples.iter().any(|s| s.abs() > 0.001),
            "Silent audio"
        );
        let seconds = audio.samples.len() as f64 / audio.sample_rate as f64;
        println!(
            "{}: {:.2}s audio at {} Hz, generated in {:.2}s",
            voice.name(),
            seconds,
            audio.sample_rate,
            start.elapsed().as_secs_f64()
        );
        if std::env::args().any(|a| a == "--play") {
            let (_stream, handle) = rodio::OutputStream::try_default()?;
            let sink = rodio::Sink::try_new(&handle)?;
            sink.append(rodio::buffer::SamplesBuffer::new(
                1,
                audio.sample_rate,
                audio.samples,
            ));
            sink.sleep_until_end();
        }
    }
    Ok(())
}
