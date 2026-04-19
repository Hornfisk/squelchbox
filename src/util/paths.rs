//! Platform-aware data directories for SquelchBox.
//!
//! Resolves per-OS locations for presets, logs, and any other on-disk state
//! via the `directories` crate:
//!
//! | OS      | Data root                                           |
//! |---------|-----------------------------------------------------|
//! | Linux   | `$XDG_DATA_HOME/squelchbox` or `~/.local/share/squelchbox` |
//! | macOS   | `~/Library/Application Support/squelchbox`          |
//! | Windows | `%APPDATA%\squelchbox\data`                         |
//!
//! All accessors fall back to `std::env::temp_dir().join("squelchbox")` if
//! the platform dirs can't be resolved. They never panic.

use std::path::PathBuf;

const ORG_QUALIFIER: &str = "";
const ORG: &str = "SquelchBox";
const APP: &str = "squelchbox";

pub fn squelchbox_data_dir() -> PathBuf {
    directories::ProjectDirs::from(ORG_QUALIFIER, ORG, APP)
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(fallback_dir)
}

pub fn squelchbox_preset_dir() -> PathBuf {
    squelchbox_data_dir().join("presets")
}

pub fn squelchbox_log_dir() -> PathBuf {
    squelchbox_data_dir().join("logs")
}

pub fn squelchbox_last_preset_file() -> PathBuf {
    squelchbox_data_dir().join("last_preset.txt")
}

pub fn squelchbox_hidden_presets_file() -> PathBuf {
    squelchbox_data_dir().join("hidden_presets.txt")
}

pub fn squelchbox_ui_scale_file() -> PathBuf {
    squelchbox_data_dir().join("ui_scale.txt")
}

/// Read the persisted UI scale. Returns `1.0` if the file is missing or
/// unreadable. The standalone wrapper doesn't save `#[persist]` params
/// between sessions, so we serialize this one by hand.
pub fn load_ui_scale() -> f32 {
    std::fs::read_to_string(squelchbox_ui_scale_file())
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .map(|v| v.clamp(1.0, 3.0))
        .unwrap_or(1.0)
}

/// Write the UI scale to disk. Silently ignores IO failures — a lost
/// preference is never worth crashing over.
pub fn save_ui_scale(scale: f32) {
    let path = squelchbox_ui_scale_file();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, format!("{scale}\n"));
}

fn fallback_dir() -> PathBuf {
    std::env::temp_dir().join("squelchbox")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_is_absolute_and_ends_with_squelchbox() {
        let p = squelchbox_data_dir();
        assert!(p.is_absolute(), "data dir should be absolute: {:?}", p);
        let last = p
            .components()
            .next_back()
            .and_then(|c| c.as_os_str().to_str())
            .unwrap_or("");
        assert!(
            last.to_lowercase().contains("squelchbox"),
            "expected squelchbox in last component, got {:?}",
            p
        );
    }

    #[test]
    fn preset_dir_is_under_data_dir() {
        assert!(squelchbox_preset_dir().starts_with(squelchbox_data_dir()));
    }

    #[test]
    fn log_dir_is_under_data_dir() {
        assert!(squelchbox_log_dir().starts_with(squelchbox_data_dir()));
    }
}
