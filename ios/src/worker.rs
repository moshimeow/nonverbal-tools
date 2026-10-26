use crate::output::{self, Output, Playback, PlaybackState};
use crate::{Audio, SpeechEngine, Voice, model_dir};
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
    Starting,
    Playing,
    Failed(String),
}
impl Phase {
    pub fn label(&self) -> &str {
        match self {
            Self::Waiting => "Queued",
            Self::Generating => "Making speech…",
            Self::Ready => "Ready",
            Self::Starting => "Preparing audio…",
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
    pub audio_error: Option<String>,
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
    fn has_speech_work(&self) -> bool {
        self.entries
            .iter()
            .any(|e| !matches!(e.item.phase, Phase::Failed(_)))
    }
    fn playback_state(&mut self, id: u64, state: anyhow::Result<PlaybackState>) -> bool {
        match state {
            Ok(PlaybackState::Starting) => false,
            Ok(PlaybackState::Playing) => {
                if let Some(entry) = self.entries.iter_mut().find(|e| e.item.id == id) {
                    entry.item.phase = Phase::Playing;
                }
                false
            }
            Ok(PlaybackState::Finished) => {
                self.finish(id);
                true
            }
            Err(e) => {
                if let Some(entry) = self.entries.iter_mut().find(|e| e.item.id == id) {
                    entry.item.phase = Phase::Failed(format!("Audio output: {e:#}"));
                }
                true
            }
        }
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
        entry.item.phase = Phase::Starting;
        Some((entry.item.id, audio))
    }
}
enum Request {
    Enqueue(Item),
    Write(String, tokio::sync::oneshot::Sender<anyhow::Result<()>>),
    Location(i64, Option<crate::history::Location>),
    Gap(i64, f64),
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
        Self::start_inner(
            crate::platform::data_directory().map(|dir| dir.join("history.sqlite3")),
            true,
        )
    }
    pub fn start_with_history_path(
        path: std::path::PathBuf,
    ) -> (Self, UnboundedReceiver<Snapshot>) {
        Self::start_inner(Ok(path), false)
    }
    fn start_inner(
        history_path: anyhow::Result<std::path::PathBuf>,
        locations: bool,
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
        let location_sender = tx.clone();
        std::thread::spawn(move || {
            let locate = |id| {
                let sender = location_sender.clone();
                if locations {
                    crate::platform::capture_location(move |location| {
                        let _ = sender.send(Request::Location(id, location));
                    });
                } else {
                    let _ = sender.send(Request::Location(id, None));
                }
            };
            let mut queue = Queue::default();
            let history_result = history_path.and_then(|path| crate::history::History::open(&path));
            let mut history_error = history_result
                .as_ref()
                .err()
                .map(|e| format!("History unavailable: {e:#}"));
            let mut history = history_result.ok();
            let mut output = None;
            let mut playing: Option<(u64, Playback)> = None;
            let mut last = Snapshot::default();
            let mut queue_pending = false;
            loop {
                match rx.recv_timeout(Duration::from_millis(15)) {
                    Ok(Request::Enqueue(item)) => {
                        if let Some(store) = history.as_mut() {
                            history_error = store
                                .record_kind(&item.text, crate::history::MessageKind::Speak)
                                .map(&locate)
                                .err()
                                .map(|e| format!("This message could not be saved: {e:#}"));
                        }
                        queue.enqueue(item);
                    }
                    Ok(Request::Write(text, reply)) => {
                        let result = history
                            .as_mut()
                            .ok_or_else(|| {
                                anyhow::anyhow!("History unavailable; message was not saved")
                            })
                            .and_then(|store| {
                                store.record_kind(&text, crate::history::MessageKind::Write)
                            })
                            .map(&locate);
                        let _ = reply.send(result);
                    }
                    Ok(Request::Location(id, location)) => {
                        if let Some(store) = history.as_mut()
                            && let Err(e) = store.set_location(id, location)
                        {
                            history_error = Some(format!("Could not save location: {e}"));
                        }
                    }
                    Ok(Request::Gap(id, gap)) => {
                        if let Some(store) = history.as_mut()
                            && let Err(e) = store.set_gap(id, gap)
                        {
                            history_error = Some(format!("Could not save spacing: {e}"));
                        }
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
                if let Some((id, sink)) = &playing
                    && queue.playback_state(*id, sink.state())
                {
                    playing.take();
                }
                // Keep music paused across queued/generated messages, but never
                // activate the native session until actual audio is ready.
                let pending = queue.has_speech_work();
                if pending != queue_pending {
                    queue_pending = pending;
                    output::set_queue_pending(pending);
                }
                if playing.is_none()
                    && let Some((id, audio)) = queue.next_playback()
                {
                    let result = (|| -> anyhow::Result<Playback> {
                        if output.is_none() {
                            output = Some(Output::new()?);
                        }
                        output.as_ref().unwrap().play(audio)
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
                snapshot.audio_error = output::session_error();
                if snapshot != last {
                    if events.send(snapshot.clone()).is_err() {
                        break;
                    }
                    last = snapshot;
                }
            }
            output::set_queue_pending(false);
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
    pub async fn write(&self, text: String) -> anyhow::Result<()> {
        crate::validate_text(&text)?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.0.tx.send(Request::Write(text, tx))?;
        rx.await?
    }
    pub fn set_gap(&self, id: i64, gap: f64) {
        let _ = self.0.tx.send(Request::Gap(id, gap));
    }
    pub fn remove(&self, id: u64) {
        let _ = self.0.tx.send(Request::Remove(id));
    }
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
    fn writing_is_durable_and_never_enters_the_audio_queue() {
        use std::{
            future::Future,
            task::{Context, Poll, Waker},
            time::Instant,
        };
        let path =
            std::env::temp_dir().join(format!("speak-write-worker-{}.sqlite", std::process::id()));
        let (speaker, mut events) = Speaker::start_with_history_path(path.clone());
        let mut send = Box::pin(speaker.write("Written, never spoken.".into()));
        let start = Instant::now();
        loop {
            if let Poll::Ready(result) = send.as_mut().poll(&mut Context::from_waker(Waker::noop()))
            {
                result.unwrap();
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(5));
        }
        loop {
            if let Ok(snapshot) = events.try_recv() {
                assert!(snapshot.queue.is_empty());
                assert!(snapshot.recent.is_empty());
                if !snapshot.history.is_empty() {
                    break;
                }
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(5));
        }
        let db = rusqlite::Connection::open(&path).unwrap();
        let (text, kind): (String, String) = db
            .query_row(
                "SELECT text,kind FROM sent_messages ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            (text.as_str(), kind.as_str()),
            ("Written, never spoken.", "write")
        );
        drop(send);
        drop(speaker);
        drop(db);
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn session_stays_pending_between_messages_and_releases_after_failure_or_cancel() {
        let mut q = Queue::default();
        q.enqueue(item(1));
        q.enqueue(item(2));
        assert!(q.has_speech_work());
        q.next_generation();
        q.generated(1, audio());
        q.next_playback();
        assert_eq!(q.entries[0].item.phase, Phase::Starting);
        assert!(!q.playback_state(1, Ok(PlaybackState::Starting)));
        assert!(!q.playback_state(1, Ok(PlaybackState::Playing)));
        assert_eq!(q.entries[0].item.phase, Phase::Playing);
        assert!(q.playback_state(1, Ok(PlaybackState::Finished)));
        assert!(q.has_speech_work()); // Keep music paused while the next item generates.
        q.next_generation();
        q.generated(2, audio());
        q.next_playback();
        assert!(q.playback_state(2, Err(anyhow::anyhow!("Output lost"))));
        assert!(!q.has_speech_work()); // Failed rows must not hold the audio session.
        assert_eq!(q.recent.iter().map(|i| i.id).collect::<Vec<_>>(), [1]);
        q.enqueue(item(3));
        q.next_generation();
        q.generated(3, audio());
        q.next_playback();
        q.remove(3); // Cancel while native setup is still pending.
        assert!(!q.has_speech_work());
        q.playback_state(3, Ok(PlaybackState::Finished)); // Late completion cannot resurrect it.
        assert_eq!(q.recent.iter().map(|i| i.id).collect::<Vec<_>>(), [1]);
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
