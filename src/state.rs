use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Task {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Widget {
    pub x: i32,
    pub y: i32,
}

impl Default for Widget {
    fn default() -> Self {
        Widget { x: 64, y: 64 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AppState {
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub widget: Widget,
}

impl AppState {
    pub const fn new() -> Self {
        AppState {
            tasks: Vec::new(),
            widget: Widget { x: 64, y: 64 },
        }
    }
}

/// `%APPDATA%\DesktopTodo` (falls back to `%LOCALAPPDATA%`).
fn data_dir() -> PathBuf {
    let root = std::env::var("APPDATA")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            std::env::var("LOCALAPPDATA")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    root.join("DesktopTodo")
}

pub fn state_path() -> PathBuf {
    data_dir().join("tasks.json")
}

pub fn load() -> AppState {
    let path = state_path();

    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(state) = serde_json::from_str::<AppState>(&text) {
            return state;
        }
        // Older builds stored a bare array of tasks.
        if let Ok(tasks) = serde_json::from_str::<Vec<Task>>(&text) {
            return AppState {
                tasks,
                widget: Widget::default(),
            };
        }
    }

    // One time migration of the file old builds dropped next to the executable.
    if let Ok(text) = fs::read_to_string("tasks.json") {
        if let Ok(tasks) = serde_json::from_str::<Vec<Task>>(&text) {
            let state = AppState {
                tasks,
                widget: Widget::default(),
            };
            let _ = save(&state);
            return state;
        }
    }

    AppState::new()
}

pub fn save(state: &AppState) -> Result<(), String> {
    let dir = data_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    let tmp = dir.join("tasks.json.tmp");
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, dir.join("tasks.json")).map_err(|e| e.to_string())?;
    Ok(())
}
