use crate::{SpeechEngine, Voice, model_dir};
use rodio::{OutputStream, Sink, buffer::SamplesBuffer};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

enum Request {
    Speak {
        id: u64,
        text: String,
        voice: Voice,
        speed: f32,
    },
    Stop,
}

#[derive(Clone, Debug)]
pub struct Status {
    pub id: u64,
    pub message: String,
}

#[derive(Clone)]
pub struct Speaker {
    tx: mpsc::Sender<Request>,
    generation: Arc<AtomicU64>,
}

impl Speaker {
    pub fn start() -> (Self, UnboundedReceiver<Status>) {
        let (tx, rx) = mpsc::channel();
        let (events, receiver) = unbounded_channel();
        let generation = Arc::new(AtomicU64::new(0));
        let current = generation.clone();
        std::thread::spawn(move || {
            let mut engine = None;
            // Create native audio objects on their owning thread and keep them alive.
            let mut output = None;
            let mut playing: Option<(u64, Sink)> = None;
            let status = |id, message: String| {
                let _ = events.send(Status { id, message });
            };
            loop {
                if let Some((id, sink)) = &playing
                    && (sink.empty() || current.load(Ordering::SeqCst) != *id)
                {
                    sink.stop();
                    if current.load(Ordering::SeqCst) == *id {
                        status(*id, "Ready".into());
                    }
                    playing = None;
                }
                let request = match rx.recv_timeout(Duration::from_millis(20)) {
                    Ok(r) => r,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                if let Some((_, sink)) = playing.take() {
                    sink.stop();
                }
                let Request::Speak {
                    id,
                    text,
                    voice,
                    speed,
                } = request
                else {
                    continue;
                };
                if current.load(Ordering::SeqCst) != id {
                    continue;
                }
                let result = (|| -> anyhow::Result<Sink> {
                    if engine.is_none() {
                        status(id, "Loading voice model…".into());
                        engine = Some(SpeechEngine::load(&model_dir())?);
                    }
                    if current.load(Ordering::SeqCst) != id {
                        anyhow::bail!("Stopped");
                    }
                    status(id, "Making speech…".into());
                    let audio = engine.as_ref().unwrap().generate(
                        &text,
                        voice,
                        speed,
                        current.clone(),
                        id,
                    )?;
                    if current.load(Ordering::SeqCst) != id {
                        anyhow::bail!("Stopped");
                    }
                    if output.is_none() {
                        configure_audio()?;
                        output = Some(OutputStream::try_default()?);
                    }
                    let sink = Sink::try_new(&output.as_ref().unwrap().1)?;
                    sink.append(SamplesBuffer::new(1, audio.sample_rate, audio.samples));
                    Ok(sink)
                })();
                match result {
                    Ok(sink) => {
                        status(id, "Speaking…".into());
                        playing = Some((id, sink));
                    }
                    Err(e) => status(id, format!("{e:#}")),
                }
            }
        });
        (Self { tx, generation }, receiver)
    }

    pub fn speak(&self, text: String, voice: Voice, speed: f32) -> anyhow::Result<u64> {
        crate::validate_text(&text)?;
        let id = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.tx.send(Request::Speak {
            id,
            text,
            voice,
            speed,
        })?;
        Ok(id)
    }

    pub fn stop(&self) -> u64 {
        let id = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.tx.send(Request::Stop);
        id
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
        anyhow::bail!("Could not activate iPhone audio output")
    }
}

#[cfg(not(target_os = "ios"))]
fn configure_audio() -> anyhow::Result<()> {
    Ok(())
}
