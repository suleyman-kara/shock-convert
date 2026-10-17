use std::path::Path;

use crate::menu::Entry;

const MSG: &str = "this command only works on Windows";

pub fn init_console() {}

pub fn notify(title: &str, text: &str) {
    eprintln!("{title}: {text}");
}

pub fn register(_exe: &Path, _entries: &[Entry]) -> Result<(), String> {
    Err(MSG.into())
}

pub fn unregister() -> Result<(), String> {
    Err(MSG.into())
}

pub fn install() -> Result<(), String> {
    Err(MSG.into())
}

pub fn uninstall() -> Result<(), String> {
    Err(MSG.into())
}
