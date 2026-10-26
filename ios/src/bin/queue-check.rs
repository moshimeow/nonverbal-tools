//! Real inference/playback integration check. Speaks short samples on this Mac.
use speak_pocket::{
    Voice,
    worker::{Phase, Speaker},
};
use std::time::{Duration, Instant};
fn main() -> anyhow::Result<()> {
    let (speaker, mut events) = Speaker::start();
    let first = speaker.speak("This is the first queued message. While I am speaking, the next message should be preparing on another thread. This sentence gives that check a little more time.".into(), Voice::Emma, 1.0)?;
    let second = speaker.speak("Henlo, blorptastic floof!".into(), Voice::Emma, 1.0)?;
    let third = speaker.speak("你好！这是排队的中文。".into(), Voice::Xiaoxiao, 1.0)?;
    let removed = speaker.speak("This should never be spoken.".into(), Voice::Emma, 1.0)?;
    speaker.remove(removed);
    let mut overlapped = false;
    let mut canceled_at = None;
    let mut cancel_seen = false;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(120) {
        if let Ok(state) = events.try_recv() {
            println!(
                "queue {:?}, recent {:?}",
                state
                    .queue
                    .iter()
                    .map(|i| (i.id, i.phase.label()))
                    .collect::<Vec<_>>(),
                state.recent.iter().map(|i| i.id).collect::<Vec<_>>()
            );
            anyhow::ensure!(
                state
                    .queue
                    .iter()
                    .all(|i| !matches!(i.phase, Phase::Failed(_))),
                "Speech failed"
            );
            anyhow::ensure!(
                !state.recent.iter().any(|i| i.id == removed),
                "Removed item played"
            );
            if state
                .queue
                .iter()
                .any(|i| i.id == first && i.phase == Phase::Playing)
                && state
                    .queue
                    .iter()
                    .any(|i| i.id == second && i.phase == Phase::Generating)
            {
                overlapped = true;
                speaker.remove(first);
                canceled_at = Some(Instant::now());
            }
            if let Some(at) = canceled_at
                && !cancel_seen
                && !state.queue.iter().any(|i| i.id == first)
            {
                anyhow::ensure!(
                    at.elapsed() < Duration::from_secs(1),
                    "Stop blocked on synthesis"
                );
                cancel_seen = true;
            }
            if state.queue.is_empty()
                && state.recent.iter().map(|i| i.id).collect::<Vec<_>>() == [second, third]
            {
                anyhow::ensure!(
                    overlapped && cancel_seen,
                    "Did not observe concurrent synthesis and responsive Stop"
                );
                let item = &state.recent[0];
                let replay = speaker.speak(item.text.clone(), item.voice, item.speed)?;
                anyhow::ensure!(replay > third, "Replay must be a new queue item");
                speaker.remove(replay);
                println!(
                    "PASS: FIFO, concurrent generation/playback, responsive cancellation, history, replay IDs"
                );
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    anyhow::bail!("Queue check timed out")
}
