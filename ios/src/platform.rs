use anyhow::{Context, Result, ensure};
use std::{
    ffi::{CStr, CString, c_char},
    path::PathBuf,
};
unsafe extern "C" {
    fn speak_configure_input();
    fn speak_data_directory() -> *const c_char;
    fn speak_copy_text(text: *const c_char) -> bool;
}
pub fn configure_input() {
    unsafe {
        speak_configure_input();
    }
}
pub fn data_directory() -> Result<PathBuf> {
    // Tests and CLI checks use an isolated directory, never the user's transcript.
    if let Some(path) = std::env::var_os("SPEAK_DATA_DIR") {
        return Ok(path.into());
    }
    let path = unsafe { speak_data_directory() };
    ensure!(
        !path.is_null(),
        "Application Support directory is unavailable"
    );
    Ok(PathBuf::from(unsafe { CStr::from_ptr(path) }.to_str()?))
}
pub fn copy_text(text: &str) -> Result<()> {
    let text = CString::new(text).context("Text contains a NUL character")?;
    ensure!(
        unsafe { speak_copy_text(text.as_ptr()) },
        "Could not copy to clipboard"
    );
    Ok(())
}

type LocationCallback = Box<dyn FnOnce(Option<crate::history::Location>) + Send>;
unsafe extern "C" {
    fn speak_location(
        context: *mut std::ffi::c_void,
        reply: unsafe extern "C" fn(*mut std::ffi::c_void, bool, f64, f64, f64, f64),
    );
    fn speak_timestamp(seconds: f64, buffer: *mut c_char, capacity: usize);
}
pub fn capture_location(callback: impl FnOnce(Option<crate::history::Location>) + Send + 'static) {
    unsafe extern "C" fn reply(
        context: *mut std::ffi::c_void,
        ok: bool,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
        captured_at: f64,
    ) {
        // Native code completes each request exactly once, including denial and timeout.
        let callback = unsafe { Box::from_raw(context.cast::<LocationCallback>()) };
        callback(ok.then_some(crate::history::Location {
            latitude,
            longitude,
            accuracy,
            captured_at,
        }));
    }
    let callback: Box<LocationCallback> = Box::new(Box::new(callback));
    unsafe {
        speak_location(Box::into_raw(callback).cast(), reply);
    }
}
pub fn format_timestamp(seconds: i64) -> String {
    let mut buffer = [0 as c_char; 256];
    unsafe {
        speak_timestamp(seconds as f64, buffer.as_mut_ptr(), buffer.len());
        CStr::from_ptr(buffer.as_ptr())
            .to_string_lossy()
            .into_owned()
    }
}
