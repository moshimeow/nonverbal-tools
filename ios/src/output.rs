//! Nonblocking playback tickets on iOS; Rodio playback on desktop.
use crate::Audio;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Starting,
    Playing,
    Finished,
}

#[cfg(not(target_os = "ios"))]
pub struct Output {
    _stream: rodio::OutputStream,
    handle: rodio::OutputStreamHandle,
}
#[cfg(not(target_os = "ios"))]
pub struct Playback(rodio::Sink);
#[cfg(not(target_os = "ios"))]
impl Output {
    pub fn new() -> anyhow::Result<Self> {
        let (_stream, handle) = rodio::OutputStream::try_default()?;
        Ok(Self { _stream, handle })
    }
    pub fn play(&self, audio: Audio) -> anyhow::Result<Playback> {
        let sink = rodio::Sink::try_new(&self.handle)?;
        sink.append(rodio::buffer::SamplesBuffer::new(
            1,
            audio.sample_rate,
            audio.samples,
        ));
        Ok(Playback(sink))
    }
}
#[cfg(not(target_os = "ios"))]
impl Playback {
    pub fn state(&self) -> anyhow::Result<PlaybackState> {
        Ok(if self.0.empty() {
            PlaybackState::Finished
        } else {
            PlaybackState::Playing
        })
    }
    pub fn stop(&self) {
        self.0.stop();
    }
}
#[cfg(not(target_os = "ios"))]
pub fn set_queue_pending(_pending: bool) {}
#[cfg(not(target_os = "ios"))]
pub fn session_error() -> Option<String> {
    None
}

#[cfg(target_os = "ios")]
pub struct Output;
#[cfg(target_os = "ios")]
pub struct Playback(std::ptr::NonNull<std::ffi::c_void>);
#[cfg(target_os = "ios")]
unsafe extern "C" {
    fn speak_play_audio(samples: *const f32, count: usize, rate: u32) -> *mut std::ffi::c_void;
    fn speak_audio_status(handle: *mut std::ffi::c_void) -> i32;
    fn speak_stop_audio(handle: *mut std::ffi::c_void);
    fn speak_release_audio(handle: *mut std::ffi::c_void);
    fn speak_audio_error(
        handle: *mut std::ffi::c_void,
        buffer: *mut std::ffi::c_char,
        capacity: usize,
    );
    fn speak_audio_queue_pending(pending: bool);
    fn speak_audio_session_error(buffer: *mut std::ffi::c_char, capacity: usize);
}
#[cfg(target_os = "ios")]
impl Output {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }
    pub fn play(&self, audio: Audio) -> anyhow::Result<Playback> {
        // Native code copies samples before returning and retains an independent
        // ticket until asynchronous setup/playback completes. No borrowed Rust
        // data is accessed later on the audio thread.
        let handle = unsafe {
            speak_play_audio(
                audio.samples.as_ptr(),
                audio.samples.len(),
                audio.sample_rate,
            )
        };
        Ok(Playback(std::ptr::NonNull::new(handle).ok_or_else(
            || anyhow::anyhow!("Cannot allocate playback ticket"),
        )?))
    }
}
#[cfg(target_os = "ios")]
impl Playback {
    pub fn state(&self) -> anyhow::Result<PlaybackState> {
        match unsafe { speak_audio_status(self.0.as_ptr()) } {
            0 => Ok(PlaybackState::Starting),
            1 => Ok(PlaybackState::Playing),
            2 => Ok(PlaybackState::Finished),
            3 => anyhow::bail!("Speech canceled"),
            _ => {
                let mut error = [0; 512];
                unsafe {
                    speak_audio_error(self.0.as_ptr(), error.as_mut_ptr(), error.len());
                }
                anyhow::bail!(
                    "{}",
                    unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) }.to_string_lossy()
                );
            }
        }
    }
    pub fn stop(&self) {
        unsafe {
            speak_stop_audio(self.0.as_ptr());
        }
    }
}
#[cfg(target_os = "ios")]
impl Drop for Playback {
    fn drop(&mut self) {
        unsafe {
            speak_release_audio(self.0.as_ptr());
        }
    }
}
#[cfg(target_os = "ios")]
pub fn set_queue_pending(pending: bool) {
    unsafe {
        speak_audio_queue_pending(pending);
    }
}
#[cfg(target_os = "ios")]
pub fn session_error() -> Option<String> {
    let mut error = [0; 512];
    unsafe {
        speak_audio_session_error(error.as_mut_ptr(), error.len());
    }
    let error = unsafe { std::ffi::CStr::from_ptr(error.as_ptr()) }.to_string_lossy();
    (!error.is_empty()).then(|| error.into_owned())
}
