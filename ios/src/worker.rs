use crate::output::{self, Output, Playback, PlaybackState};
use crate::{Audio, SpeechEngine, Voice, chunks, model_dir};
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
    pub history_id: Option<i64>,
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
    phrases: VecDeque<String>,
    audio: VecDeque<Audio>,
    playing: bool,
    started: bool,
}
#[derive(Default)]
struct Queue {
    entries: VecDeque<Entry>,
    recent: VecDeque<Item>,
    generating: Option<u64>,
    outcomes: Vec<(i64, String)>,
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
        let phrases = chunks::split(&item.text, item.voice).into();
        self.entries.push_back(Entry {
            item,
            phrases,
            audio: VecDeque::new(),
            playing: false,
            started: false,
        });
        if self.entries.back().is_some_and(|e| e.phrases.is_empty()) {
            let id = self.entries.back().unwrap().item.id;
            self.end(id, Some("No pronounceable text".into()));
        }
    }
    fn end(&mut self, id: u64, error: Option<String>) {
        if let Some(index) = self.entries.iter().position(|e| e.item.id == id) {
            let entry = self.entries.remove(index).unwrap();
            if let Some(history_id) = entry.item.history_id {
                let status = match &error {
                    Some(e) if entry.started => format!("Not fully played: {e}"),
                    Some(e) => format!("Not played: {e}"),
                    None => "Played".into(),
                };
                self.outcomes.push((history_id, status));
            }
            if error.is_none() {
                let mut item = entry.item;
                item.phase = Phase::Ready;
                self.recent.push_back(item);
                while self.recent.len() > 2 {
                    self.recent.pop_front();
                }
            }
        }
    }
    fn remove(&mut self, id: u64) {
        self.end(id, Some("Stopped".into()));
    }
    fn generated(&mut self, id: u64, result: anyhow::Result<Audio>) {
        if self.generating != Some(id) {
            return;
        }
        self.generating = None;
        match result {
            Ok(audio) => {
                if let Some(e) = self.entries.iter_mut().find(|e| e.item.id == id) {
                    e.audio.push_back(audio);
                    if !e.playing {
                        e.item.phase = Phase::Ready;
                    }
                }
            }
            Err(e) => self.end(id, Some(format!("Couldn't speak: {e:#}"))),
        }
    }
    // Bounded lookahead even for arbitrarily long messages: at most two ready
    // phrases per entry, and only the current/next message can generate.
    fn next_generation(&mut self) -> Option<Item> {
        if self.generating.is_some() {
            return None;
        }
        let e = self
            .entries
            .iter_mut()
            .take(2)
            .find(|e| !e.phrases.is_empty() && e.audio.len() < 2)?;
        let mut job = e.item.clone();
        job.text = e.phrases.pop_front()?;
        if !e.playing && e.audio.is_empty() {
            e.item.phase = Phase::Generating;
        }
        self.generating = Some(e.item.id);
        Some(job)
    }
    fn has_speech_work(&self) -> bool {
        !self.entries.is_empty()
    }
    fn playback_state(&mut self, id: u64, state: anyhow::Result<PlaybackState>) -> bool {
        match state {
            Ok(PlaybackState::Starting) => false,
            Ok(PlaybackState::Playing) => {
                if let Some(e) = self.entries.iter_mut().find(|e| e.item.id == id) {
                    e.item.phase = Phase::Playing;
                    e.started = true;
                }
                false
            }
            Ok(PlaybackState::Finished) => {
                if let Some(e) = self.entries.iter_mut().find(|e| e.item.id == id) {
                    e.playing = false;
                    if e.phrases.is_empty() && e.audio.is_empty() && self.generating != Some(id) {
                        self.end(id, None);
                    } else {
                        e.item.phase = if e.audio.is_empty() {
                            Phase::Generating
                        } else {
                            Phase::Ready
                        };
                    }
                }
                true
            }
            Err(e) => {
                self.end(id, Some(format!("Audio output: {e:#}")));
                true
            }
        }
    }
    fn next_playback(&mut self) -> Option<(u64, Audio)> {
        let e = self.entries.front_mut()?;
        if e.playing {
            return None;
        }
        let audio = e.audio.pop_front()?;
        e.playing = true;
        e.item.phase = Phase::Starting;
        Some((e.item.id, audio))
    }
}
#[derive(Clone, Debug, PartialEq)]
struct Draft {
    text: String,
    voice: Voice,
    speed: f32,
}
#[derive(Default)]
struct AudioCache(VecDeque<(Draft, Audio)>);
impl AudioCache {
    fn get(&self, key: &Draft) -> Option<Audio> {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, a)| a.clone())
    }
    fn insert(&mut self, key: Draft, audio: &Audio) {
        if audio.samples.len() > 2_000_000 {
            return;
        }
        self.0.retain(|(k, _)| *k != key);
        self.0.push_back((key, audio.clone()));
        while self.0.len() > 4
            || self.0.iter().map(|(_, a)| a.samples.len()).sum::<usize>() > 2_000_000
        {
            self.0.pop_front();
        }
    }
}
struct Job {
    id: u64,
    draft: Draft,
    prepare: bool,
}
const PREPARE: u64 = u64::MAX;
enum Request {
    Enqueue(Item),
    Send(
        String,
        Voice,
        f32,
        bool,
        tokio::sync::oneshot::Sender<anyhow::Result<()>>,
    ),
    Replay(i64, Voice, f32),
    Draft(Option<Draft>),
    Prepared,
    Location(i64, Option<crate::history::Location>),
    Gap(i64, f64),
    Remove(u64),
    Generated(u64, anyhow::Result<Audio>),
    Shutdown,
}
struct Handle {
    tx: mpsc::Sender<Request>,
    next_id: Arc<AtomicU64>,
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
            crate::platform::data_directory().map(|d| d.join("history.sqlite3")),
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
        let (jobs, pending) = mpsc::channel::<Job>();
        let generation = Arc::new(AtomicU64::new(0));
        let current = generation.clone();
        let results = tx.clone();
        std::thread::spawn(move || {
            // Load on launch, not on the first Send; no audio session is acquired.
            let mut engine = SpeechEngine::load(&model_dir()).ok();
            let mut cache = AudioCache::default();
            while let Ok(job) = pending.recv() {
                let result = (|| -> anyhow::Result<Audio> {
                    anyhow::ensure!(current.load(Ordering::SeqCst) == job.id, "Canceled");
                    if let Some(audio) = cache.get(&job.draft) {
                        return Ok(audio);
                    }
                    if engine.is_none() {
                        engine = Some(SpeechEngine::load(&model_dir())?);
                    }
                    let audio = engine.as_ref().unwrap().generate(
                        &job.draft.text,
                        job.draft.voice,
                        job.draft.speed,
                        current.clone(),
                        job.id,
                    )?;
                    anyhow::ensure!(current.load(Ordering::SeqCst) == job.id, "Canceled");
                    cache.insert(job.draft.clone(), &audio);
                    Ok(audio)
                })();
                let message = if job.prepare {
                    Request::Prepared
                } else {
                    Request::Generated(job.id, result)
                };
                if results.send(message).is_err() {
                    break;
                }
            }
        });
        let ids = Arc::new(AtomicU64::new(1));
        let handle = Arc::new(Handle {
            tx: tx.clone(),
            next_id: ids.clone(),
        });
        let location_sender = tx;
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
            let result = history_path.and_then(|path| crate::history::History::open(&path));
            let mut history_error = result
                .as_ref()
                .err()
                .map(|e| format!("History unavailable: {e:#}"));
            let mut history = result.ok();
            let mut output = None;
            let mut playing: Option<(u64, Playback)> = None;
            let mut last = Snapshot::default();
            let mut queue_pending = false;
            let mut draft: Option<Draft> = None;
            let mut attempted: Option<Draft> = None;
            let mut preparing = false;
            loop {
                match rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(Request::Enqueue(mut item)) => {
                        if let Some(store) = history.as_mut() {
                            match store.record_kind(&item.text, crate::history::MessageKind::Speak)
                            {
                                Ok(id) => {
                                    item.history_id = Some(id);
                                    locate(id);
                                    if let Err(e) = store.set_playback(id, "Queued") {
                                        history_error = Some(e.to_string());
                                    }
                                }
                                Err(e) => history_error = Some(e.to_string()),
                            }
                        }
                        if preparing {
                            generation.store(0, Ordering::SeqCst);
                        }
                        draft = None;
                        queue.enqueue(item);
                    }
                    Ok(Request::Send(text, voice, speed, write, reply)) => {
                        let result = (|| -> anyhow::Result<()> {
                            let store = history.as_mut().ok_or_else(|| {
                                anyhow::anyhow!("History unavailable; message was not saved")
                            })?;
                            let kind = if write {
                                crate::history::MessageKind::Write
                            } else {
                                crate::history::MessageKind::Speak
                            };
                            let id = store.record_kind(&text, kind)?;
                            locate(id);
                            if !write {
                                if let Err(e) = store.set_playback(id, "Queued") {
                                    history_error = Some(e.to_string());
                                }
                                queue.enqueue(Item {
                                    id: ids.fetch_add(1, Ordering::SeqCst),
                                    history_id: Some(id),
                                    text,
                                    voice,
                                    speed,
                                    phase: Phase::Waiting,
                                });
                            }
                            Ok(())
                        })();
                        if preparing {
                            generation.store(0, Ordering::SeqCst);
                        }
                        draft = None;
                        let _ = reply.send(result);
                    }
                    Ok(Request::Replay(id, voice, speed)) => {
                        if !queue.entries.iter().any(|e| e.item.history_id == Some(id))
                            && let Some(store) = history.as_mut()
                            && let Some(entry) = store.entries.iter().find(|e| e.id == id)
                        {
                            let text = entry.text.clone();
                            if let Err(e) = store.set_playback(id, "Queued") {
                                history_error = Some(e.to_string());
                            }
                            queue.enqueue(Item {
                                id: ids.fetch_add(1, Ordering::SeqCst),
                                history_id: Some(id),
                                text,
                                voice,
                                speed,
                                phase: Phase::Waiting,
                            });
                            if preparing {
                                generation.store(0, Ordering::SeqCst);
                            }
                        }
                    }
                    Ok(Request::Draft(next)) => {
                        if draft != next && preparing {
                            generation.store(0, Ordering::SeqCst);
                        }
                        draft = next;
                    }
                    Ok(Request::Prepared) => preparing = false,
                    Ok(Request::Location(id, location)) => {
                        if let Some(store) = history.as_mut()
                            && let Err(e) = store.set_location(id, location)
                        {
                            history_error = Some(e.to_string());
                        }
                    }
                    Ok(Request::Gap(id, gap)) => {
                        if let Some(store) = history.as_mut()
                            && let Err(e) = store.set_gap(id, gap)
                        {
                            history_error = Some(e.to_string());
                        }
                    }
                    Ok(Request::Remove(id)) => {
                        queue.remove(id);
                        if queue.generating == Some(id) {
                            generation.store(0, Ordering::SeqCst);
                        }
                    }
                    Ok(Request::Generated(id, result)) => queue.generated(id, result),
                    Ok(Request::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                // Generation failure or removal can retire an entry while an earlier phrase plays.
                if playing
                    .as_ref()
                    .is_some_and(|(id, _)| !queue.entries.iter().any(|e| e.item.id == *id))
                    && let Some((_, sink)) = playing.take()
                {
                    sink.stop();
                }
                if let Some((id, sink)) = &playing
                    && queue.playback_state(*id, sink.state())
                {
                    playing.take();
                }
                let active = queue.has_speech_work();
                if active != queue_pending {
                    queue_pending = active;
                    output::set_queue_pending(active);
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
                            queue.end(id, Some(format!("Audio output: {e:#}")));
                            output = None;
                        }
                    }
                }
                if !preparing {
                    if let Some(item) = queue.next_generation() {
                        generation.store(item.id, Ordering::SeqCst);
                        if jobs
                            .send(Job {
                                id: item.id,
                                draft: Draft {
                                    text: item.text,
                                    voice: item.voice,
                                    speed: item.speed,
                                },
                                prepare: false,
                            })
                            .is_err()
                        {
                            break;
                        }
                    } else if !active
                        && queue.generating.is_none()
                        && draft != attempted
                        && let Some(next) = draft.clone()
                    {
                        attempted = Some(next.clone());
                        preparing = true;
                        generation.store(PREPARE, Ordering::SeqCst);
                        if jobs
                            .send(Job {
                                id: PREPARE,
                                draft: next,
                                prepare: true,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                for (id, status) in queue.outcomes.drain(..) {
                    if let Some(store) = history.as_mut()
                        && let Err(e) = store.set_playback(id, &status)
                    {
                        history_error = Some(e.to_string());
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
        (Self(handle), receiver)
    }
    pub fn speak(&self, text: String, voice: Voice, speed: f32) -> anyhow::Result<u64> {
        validate(&text, speed)?;
        let id = self.0.next_id.fetch_add(1, Ordering::SeqCst);
        self.0.tx.send(Request::Enqueue(Item {
            id,
            history_id: None,
            text,
            voice,
            speed,
            phase: Phase::Waiting,
        }))?;
        Ok(id)
    }
    pub async fn send(
        &self,
        text: String,
        voice: Voice,
        speed: f32,
        write: bool,
    ) -> anyhow::Result<()> {
        validate(&text, speed)?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.0
            .tx
            .send(Request::Send(text, voice, speed, write, tx))?;
        rx.await?
    }
    pub async fn write(&self, text: String) -> anyhow::Result<()> {
        self.send(text, Voice::Emma, 1.0, true).await
    }
    pub fn replay(&self, id: i64, voice: Voice, speed: f32) {
        if speed.is_finite() && (0.5..=2.0).contains(&speed) {
            let _ = self.0.tx.send(Request::Replay(id, voice, speed));
        }
    }
    pub fn prepare(&self, text: &str, voice: Voice, speed: f32) {
        let draft = if validate(text, speed).is_ok() {
            chunks::settled_prefix(text, voice).map(|text| Draft { text, voice, speed })
        } else {
            None
        };
        let _ = self.0.tx.send(Request::Draft(draft));
    }
    pub fn set_gap(&self, id: i64, gap: f64) {
        let _ = self.0.tx.send(Request::Gap(id, gap));
    }
    pub fn remove(&self, id: u64) {
        let _ = self.0.tx.send(Request::Remove(id));
    }
}
fn validate(text: &str, speed: f32) -> anyhow::Result<()> {
    crate::validate_text(text)?;
    anyhow::ensure!(
        speed.is_finite() && (0.5..=2.0).contains(&speed),
        "Speed must be between 0.5 and 2.0"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: u64, text: &str) -> Item {
        Item {
            id,
            history_id: Some(id as i64),
            text: text.into(),
            voice: Voice::Emma,
            speed: 1.0,
            phase: Phase::Waiting,
        }
    }
    fn audio() -> anyhow::Result<Audio> {
        Ok(Audio {
            samples: vec![0.1; 100],
            sample_rate: 24000,
        })
    }
    #[test]
    fn draft_cache_never_reuses_edits_or_other_voice_or_speed_and_stays_bounded() {
        let mut cache = AudioCache::default();
        let key = Draft {
            text: "Hello there!".into(),
            voice: Voice::Emma,
            speed: 1.0,
        };
        cache.insert(key.clone(), &audio().unwrap());
        assert!(cache.get(&key).is_some());
        assert!(
            cache
                .get(&Draft {
                    text: "Hello there?".into(),
                    ..key.clone()
                })
                .is_none()
        );
        assert!(
            cache
                .get(&Draft {
                    voice: Voice::Xiaoxiao,
                    ..key.clone()
                })
                .is_none()
        );
        assert!(
            cache
                .get(&Draft {
                    speed: 1.2,
                    ..key.clone()
                })
                .is_none()
        );
        for i in 0..10 {
            cache.insert(
                Draft {
                    text: i.to_string(),
                    ..key.clone()
                },
                &audio().unwrap(),
            );
        }
        assert_eq!(cache.0.len(), 4);
        assert!(cache.get(&key).is_none());
        let big = Audio {
            samples: vec![0.1; 1_100_000],
            sample_rate: 24000,
        };
        cache.insert(key.clone(), &big);
        cache.insert(
            Draft {
                text: "Other".into(),
                ..key.clone()
            },
            &big,
        );
        assert!(cache.0.iter().map(|(_, a)| a.samples.len()).sum::<usize>() <= 2_000_000);
    }
    #[test]
    fn plays_first_phrase_before_message_finishes_generating() {
        let mut q = Queue::default();
        q.enqueue(item(1, "Are you coming? I can wait!"));
        q.enqueue(item(2, "Second message!"));
        assert_eq!(q.next_generation().unwrap().text, "Are you coming?");
        q.generated(1, audio());
        assert_eq!(q.next_playback().unwrap().0, 1);
        assert!(!q.playback_state(1, Ok(PlaybackState::Playing)));
        assert_eq!(q.next_generation().unwrap().text, "I can wait!");
        assert!(q.playback_state(1, Ok(PlaybackState::Finished)));
        assert!(q.recent.is_empty());
        assert!(q.has_speech_work());
        assert!(q.next_playback().is_none()); // Never overtake a streaming message.
        q.generated(1, audio());
        q.next_playback();
        q.playback_state(1, Ok(PlaybackState::Finished));
        assert_eq!(q.recent[0].id, 1);
        assert_eq!(q.outcomes, [(1, "Played".into())]);
        assert_eq!(q.next_generation().unwrap().id, 2);
    }
    #[test]
    fn failures_and_cancellation_retire_without_dismissal_or_late_replay() {
        let mut q = Queue::default();
        q.enqueue(item(1, "Are you coming? I can wait!"));
        q.enqueue(item(2, "Next!"));
        q.next_generation();
        q.generated(1, audio());
        q.next_playback();
        q.playback_state(1, Ok(PlaybackState::Playing));
        q.next_generation();
        q.generated(1, Err(anyhow::anyhow!("Invalid audio")));
        assert_eq!(q.entries.len(), 1);
        assert!(q.outcomes[0].1.starts_with("Not fully played:"));
        q.playback_state(1, Ok(PlaybackState::Finished));
        assert!(q.recent.is_empty());
        q.next_generation();
        q.remove(2);
        q.generated(2, audio());
        assert!(!q.has_speech_work());
        assert!(q.next_playback().is_none());
        assert_eq!(q.outcomes[1], (2, "Not played: Stopped".into()));
    }
    #[test]
    fn bounded_prefetch_and_output_failure_release_the_queue() {
        let mut q = Queue::default();
        q.enqueue(item(1, &"A sentence with context. ".repeat(100)));
        q.enqueue(item(2, "Second!"));
        q.enqueue(item(3, "Third!"));
        for _ in 0..2 {
            q.next_generation();
            q.generated(1, audio());
        }
        assert_eq!(q.next_generation().unwrap().id, 2);
        q.generated(2, audio());
        assert!(q.next_generation().is_none());
        q.next_playback();
        q.playback_state(1, Err(anyhow::anyhow!("Output unavailable")));
        assert_eq!(q.next_playback().unwrap().0, 2);
        q.remove(2);
        q.remove(3);
        assert!(!q.has_speech_work());
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
}
