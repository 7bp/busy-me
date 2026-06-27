use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub enabled: bool,
    pub poll_interval_ms: u64,
    pub busy_color: [u8; 3],
    pub free_color: [u8; 3],
    pub speaker_color: [u8; 3],
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_ms: 1000,
            busy_color: [255, 40, 40],
            free_color: [40, 230, 40],
            speaker_color: [255, 160, 40],
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        let base = dirs();
        base.join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            fs::write(&path, json).ok();
        }
    }
}

fn dirs() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("busy-me")
}
