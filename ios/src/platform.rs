use anyhow::{Context, Result, ensure};
use std::{
    ffi::{CStr, CString, c_char},
    path::PathBuf,
};
unsafe extern "C" {
    fn speak_data_directory() -> *const c_char;
    fn speak_copy_text(text: *const c_char) -> bool;
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
