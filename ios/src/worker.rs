use crate::{Audio, SpeechEngine, Voice, model_dir};
use rodio::{OutputStream, Sink, buffer::SamplesBuffer};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Waiting,
    Generating,
    Ready,
    Playing,
    Failed(String),
}
impl Phase {
    pub fn label(&self) -> &str {
        match self {
            Self::Waiting => "Queued",
            Self::Generating => "Making speech…",
            Self::Ready => "Ready",
            Self::Playing => "Speaking…",
            Self::Failed(e) => e,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: u64,
    pub text: String,
    pub voice: Voice,
    pub speed: f32,
    pub phase: Phase,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub queue: Vec<Item>,
    pub recent: Vec<Item>,
    pub history: Arc<Vec<crate::history::SentMessage>>,
    pub history_error: Option<String>,
}
struct Entry {
    item: Item,
    audio: Option<Audio>,
}
#[derive(Default)]
struct Queue {
    entries: VecDeque<Entry>,
    recent: VecDeque<Item>,
    generating: Option<u64>,
}
impl Queue {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            queue: self.entries.iter().map(|e| e.item.clone()).collect(),
            recent: self.recent.iter().cloned().collect(),
            ..Snapshot::default()
        }
    }
    fn enqueue(&mut self, item: Item) {
        self.entries.push_back(Entry { item, audio: None });
    }
    fn remove(&mut self, id: u64) {
        self.entries.retain(|e| e.item.id != id);
    }
    fn finish(&mut self, id: u64) {
        if let Some(index) = self.entries.iter().position(|e| e.item.id == id) {
            let mut item = self.entries.remove(index).unwrap().item;
            item.phase = Phase::Ready;
            self.recent.push_back(item);
            while self.recent.len() > 2 {
                self.recent.pop_front();
            }
        }
    }
    fn generated(&mut self, id: u64, result: anyhow::Result<Audio>) {
        if self.generating != Some(id) {
            return;
        }
        self.generating = None;
        if let Some(entry) = self.entries.iter_mut().find(|e| e.item.id == id) {
            match result {
                Ok(audio) => {
                    entry.audio = Some(audio);
                    entry.item.phase = Phase::Ready;
                }
                Err(e) => entry.item.phase = Phase::Failed(format!("Couldn't speak: {e:#}")),
            }
        }
    }
    // At most the current item plus one prepared item. Failed rows don't stall speech.
    fn next_generation(&mut self) -> Option<Item> {
        if self.generating.is_some() {
            return None;
        }
        let entry = self
            .entries
            .iter_mut()
            .filter(|e| !matches!(e.item.phase, Phase::Failed(_)))
            .take(2)
            .find(|e| e.item.phase == Phase::Waiting)?;
        entry.item.phase = Phase::Generating;
        self.generating = Some(entry.item.id);
        Some(entry.item.clone())
    }
    fn next_playback(&mut self) -> Option<(u64, Audio)> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| !matches!(e.item.phase, Phase::Failed(_)))?;
        if entry.item.phase != Phase::Ready {
            return None;
        }
        let audio = entry.audio.take()?;
        entry.item.phase = Phase::Playing;
        Some((entry.item.id, audio))
    }
}
enum Request {
    Enqueue(Item),
    Remove(u64),
    Generated(u64, anyhow::Result<Audio>),
    Shutdown,
}
struct Handle {
    tx: mpsc::Sender<Request>,
    next_id: AtomicU64,
}
impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.tx.send(Request::Shutdown);
    }
}
#[derive(Clone)]
pub struct Speaker(Arc<Handle>);
impl Speaker {
    pub fn start() -> (Self, UnboundedReceiver<Snapshot>) {
        Self::start_inner(crate::platform::data_directory().map(|dir| dir.join("history.sqlite3")))
    }
    pub fn start_with_history_path(
        path: std::path::PathBuf,
    ) -> (Self, UnboundedReceiver<Snapshot>) {
        Self::start_inner(Ok(path))
    }
    fn start_inner(
        history_path: anyhow::Result<std::path::PathBuf>,
    ) -> (Self, UnboundedReceiver<Snapshot>) {
        let (tx, rx) = mpsc::channel();
        let (events, receiver) = unbounded_channel();
        let (jobs, pending) = mpsc::channel::<Item>();
        let generation = Arc::new(AtomicU64::new(0));
        let current = generation.clone();
        let results = tx.clone();
        // The inference engine lives exclusively on this thread. Playback never waits on it.
        std::thread::spawn(move || {
            let mut engine = None;
            while let Ok(item) = pending.recv() {
                let result = (|| -> anyhow::Result<Audio> {
                    if engine.is_none() {
                        engine = Some(SpeechEngine::load(&model_dir())?);
                    }
                    anyhow::ensure!(current.load(Ordering::SeqCst) == item.id, "Canceled");
                    engine.as_ref().unwrap().generate(
                        &item.text,
                        item.voice,
                        item.speed,
                        current.clone(),
                        item.id,
                    )
                })();
                if results.send(Request::Generated(item.id, result)).is_err() {
                    break;
                }
            }
        });
        std::thread::spawn(move || {
            let mut queue = Queue::default();
            let history_result = history_path.and_then(|path| crate::history::History::open(&path));
            let mut history_error = history_result
                .as_ref()
                .err()
                .map(|e| format!("History unavailable: {e:#}"));
            let mut history = history_result.ok();
            let mut output = None;
            let mut playing: Option<(u64, Sink)> = None;
            let mut last = Snapshot::default();
            loop {
                match rx.recv_timeout(Duration::from_millis(15)) {
                    Ok(Request::Enqueue(item)) => {
                        if let Some(store) = history.as_mut() {
                            history_error = store
                                .record(&item.text)
                                .err()
                                .map(|e| format!("This message could not be saved: {e:#}"));
                        }
                        queue.enqueue(item);
                    }
                    Ok(Request::Remove(id)) => {
                        queue.remove(id);
                        if queue.generating == Some(id) {
                            generation.store(0, Ordering::SeqCst);
                        }
                        if playing.as_ref().is_some_and(|(active, _)| *active == id)
                            && let Some((_, sink)) = playing.take()
                        {
                            sink.stop();
                        }
                    }
                    Ok(Request::Generated(id, audio)) => queue.generated(id, audio),
                    Ok(Request::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                if playing.as_ref().is_some_and(|(_, sink)| sink.empty()) {
                    let (id, _) = playing.take().unwrap();
                    queue.finish(id);
                }
                if playing.is_none()
                    && let Some((id, audio)) = queue.next_playback()
                {
                    let result = (|| -> anyhow::Result<Sink> {
                        configure_audio()?;
                        if output.is_none() {
                            output = Some(OutputStream::try_default()?);
                        }
                        let sink = Sink::try_new(&output.as_ref().unwrap().1)?;
                        sink.append(SamplesBuffer::new(1, audio.sample_rate, audio.samples));
                        Ok(sink)
                    })();
                    match result {
                        Ok(sink) => playing = Some((id, sink)),
                        Err(e) => {
                            if let Some(entry) = queue.entries.iter_mut().find(|e| e.item.id == id)
                            {
                                entry.item.phase = Phase::Failed(format!("Audio output: {e:#}"));
                            }
                            output = None;
                        }
                    }
                }
                if let Some(item) = queue.next_generation() {
                    generation.store(item.id, Ordering::SeqCst);
                    if jobs.send(item).is_err() {
                        break;
                    }
                }
                let mut snapshot = queue.snapshot();
                if let Some(store) = &history {
                    snapshot.history = store.entries.clone();
                }
                snapshot.history_error = history_error.clone();
                if snapshot != last {
                    if events.send(snapshot.clone()).is_err() {
                        break;
                    }
                    last = snapshot;
                }
            }
            generation.store(0, Ordering::SeqCst);
            if let Some((_, sink)) = playing {
                sink.stop();
            }
        });
        (
            Self(Arc::new(Handle {
                tx,
                next_id: AtomicU64::new(1),
            })),
            receiver,
        )
    }
    pub fn speak(&self, text: String, voice: Voice, speed: f32) -> anyhow::Result<u64> {
        crate::validate_text(&text)?;
        anyhow::ensure!(
            speed.is_finite() && (0.5..=2.0).contains(&speed),
            "Speed must be between 0.5 and 2.0"
        );
        let id = self.0.next_id.fetch_add(1, Ordering::SeqCst);
        self.0.tx.send(Request::Enqueue(Item {
            id,
            text,
            voice,
            speed,
            phase: Phase::Waiting,
        }))?;
        Ok(id)
    }
    pub fn remove(&self, id: u64) {
        let _ = self.0.tx.send(Request::Remove(id));
    }
}

#[cfg(target_os = "ios")]
fn configure_audio() -> anyhow::Result<()> {
    unsafe extern "C" {
        fn speak_configure_audio() -> bool;
    }
    if unsafe { speak_configure_audio() } {
        Ok(())
    } else {
        anyhow::bail!("Could not route speech to the iPhone speaker")
    }
}

#[cfg(not(target_os = "ios"))]
fn configure_audio() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: u64) -> Item {
        Item {
            id,
            text: format!("item {id}"),
            voice: Voice::Emma,
            speed: 1.0,
            phase: Phase::Waiting,
        }
    }
    fn audio() -> anyhow::Result<Audio> {
        Ok(Audio {
            samples: vec![0.1; 10],
            sample_rate: 24000,
        })
    }
    #[test]
    fn fifo_prefetch_and_recent_history() {
        let mut q = Queue::default();
        for id in 1..=4 {
            q.enqueue(item(id));
        }
        assert_eq!(q.next_generation().unwrap().id, 1);
        assert!(q.next_generation().is_none());
        q.generated(1, audio());
        assert_eq!(q.next_playback().unwrap().0, 1);
        assert_eq!(q.next_generation().unwrap().id, 2); // synthesis while 1 plays
        q.generated(2, audio());
        assert!(q.next_generation().is_none()); // bounded lookahead
        assert!(q.next_playback().is_none()); // no overlapping playback
        for id in 1..=3 {
            q.finish(id);
            assert_eq!(q.next_playback().unwrap().0, id + 1);
            if id < 3 {
                let next = q.next_generation().unwrap().id;
                q.generated(next, audio());
            }
        }
        assert_eq!(
            q.snapshot().recent.iter().map(|i| i.id).collect::<Vec<_>>(),
            [2, 3]
        );
    }
    #[test]
    fn cancellation_discards_late_audio_and_failure_does_not_block() {
        let mut q = Queue::default();
        for id in 1..=4 {
            q.enqueue(item(id));
        }
        q.next_generation();
        q.remove(1);
        assert!(q.next_generation().is_none()); // don't overlap inference on cancellation
        q.generated(1, audio());
        assert_eq!(q.next_generation().unwrap().id, 2);
        q.generated(2, Err(anyhow::anyhow!("test failure")));
        assert_eq!(q.next_generation().unwrap().id, 3);
        q.generated(3, audio());
        assert_eq!(q.next_playback().unwrap().0, 3);
        q.remove(4);
        q.remove(3);
        assert!(q.next_generation().is_none());
        assert!(q.snapshot().recent.is_empty());
        assert!(matches!(q.snapshot().queue[0].phase, Phase::Failed(_)));
    }
}
