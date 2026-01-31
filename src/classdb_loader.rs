use std::process::Command;

use crate::classdb::ClassDb;
use crate::error::GdEyeError;

/// Controls how the ClassDB is loaded.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassDbMode {
    /// Try the user's Godot binary first, fall back to latest bundled (default).
    Auto,
    /// Use a specific bundled version (e.g., "4.5" or "4.5.1"). Never invokes Godot.
    TargetVersion(String),
}

/// Load the ClassDB according to the given mode.
pub fn load_classdb(mode: &ClassDbMode) -> Result<ClassDb, GdEyeError> {
    match mode {
        ClassDbMode::TargetVersion(version) => ClassDb::from_bundled(Some(version)),
        ClassDbMode::Auto => match try_runtime_godot() {
            Ok(db) => Ok(db),
            Err(_) => ClassDb::from_bundled(None),
        },
    }
}

/// Attempt to discover the user's Godot binary and dump its extension API.
fn try_runtime_godot() -> Result<ClassDb, GdEyeError> {
    let godot_bin = find_godot_binary()?;
    let godot_path = godot_bin.to_string_lossy().to_string();

    // Use a unique temp directory to avoid races between concurrent processes
    let temp_dir = tempfile::tempdir()?;

    let output = Command::new(&godot_bin)
        .arg("--dump-extension-api")
        .arg("--headless")
        .current_dir(temp_dir.path())
        .output()?;

    if !output.status.success() {
        return Err(GdEyeError::GodotDumpFailed(output.status.to_string()));
    }

    let api_path = temp_dir.path().join("extension_api.json");
    let json_bytes = std::fs::read(&api_path)?;

    // temp_dir is automatically cleaned up when dropped

    ClassDb::from_extension_api(&json_bytes, &godot_path)
}

/// Find the Godot binary in PATH.
fn find_godot_binary() -> Result<std::path::PathBuf, GdEyeError> {
    for name in &["godot", "godot4"] {
        if let Ok(path) = which::which(name) {
            return Ok(path);
        }
    }
    Err(GdEyeError::GodotNotFound)
}
