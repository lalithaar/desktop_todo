use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Task {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

/// One named task list, the widget shows one of them at a time.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct List {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub tasks: Vec<Task>,
}

impl List {
    pub fn new(name: &str) -> Self {
        List {
            name: name.to_string(),
            tasks: Vec::new(),
        }
    }
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

pub const DEFAULT_LIST: &str = "ToDo";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AppState {
    #[serde(default)]
    pub lists: Vec<List>,
    /// Index of the list that is currently on screen.
    #[serde(default)]
    pub active: usize,
    /// Completed tasks are left out of the widget while this is set.
    #[serde(default)]
    pub hide_completed: bool,
    #[serde(default)]
    pub widget: Widget,
}

impl AppState {
    pub const fn new() -> Self {
        AppState {
            lists: Vec::new(),
            active: 0,
            hide_completed: false,
            widget: Widget { x: 64, y: 64 },
        }
    }

    pub fn active_list(&self) -> Option<&List> {
        self.lists.get(self.active)
    }

    pub fn active_name(&self) -> &str {
        self.active_list()
            .map(|list| list.name.as_str())
            .unwrap_or("")
    }

    /// Tasks of the list that is currently on screen.
    pub fn tasks(&self) -> &[Task] {
        self.active_list()
            .map(|list| list.tasks.as_slice())
            .unwrap_or(&[])
    }

    pub fn tasks_mut(&mut self) -> &mut Vec<Task> {
        if self.lists.is_empty() {
            self.lists.push(List::new(DEFAULT_LIST));
            self.active = 0;
        }
        if self.active >= self.lists.len() {
            self.active = self.lists.len() - 1;
        }
        &mut self.lists[self.active].tasks
    }

    /// Indices of the tasks that are on screen right now, top to bottom.
    pub fn visible(&self) -> Vec<usize> {
        self.tasks()
            .iter()
            .enumerate()
            .filter(|(_, task)| !(self.hide_completed && task.done))
            .map(|(index, _)| index)
            .collect()
    }

    /// Steps to the next (positive) or previous (negative) list.
    pub fn cycle_list(&mut self, step: isize) -> bool {
        let len = self.lists.len();
        if len < 2 {
            return false;
        }
        let current = self.active.min(len - 1) as isize;
        self.active = (current + step).rem_euclid(len as isize) as usize;
        true
    }

    /// Appends an empty list and switches to it.
    pub fn add_list(&mut self) -> usize {
        let mut number = self.lists.len() + 1;
        let name = loop {
            let candidate = format!("List {number}");
            if !self.lists.iter().any(|list| list.name == candidate) {
                break candidate;
            }
            number += 1;
        };
        self.lists.push(List::new(&name));
        self.active = self.lists.len() - 1;
        self.active
    }

    /// Removes the visible list, the widget always keeps at least one.
    pub fn remove_active_list(&mut self) -> bool {
        if self.lists.len() < 2 {
            return false;
        }
        let index = self.active.min(self.lists.len() - 1);
        self.lists.remove(index);
        if self.active >= self.lists.len() {
            self.active = self.lists.len() - 1;
        }
        true
    }

    pub fn rename_active(&mut self, name: &str) {
        if let Some(list) = self.lists.get_mut(self.active) {
            list.name = name.to_string();
        }
    }

    /// Moves a task of the visible list to the end of another list.
    pub fn move_task(&mut self, to: usize, index: usize) -> bool {
        if to == self.active || to >= self.lists.len() {
            return false;
        }
        let from = self.active.min(self.lists.len() - 1);
        if index >= self.lists[from].tasks.len() {
            return false;
        }
        let task = self.lists[from].tasks.remove(index);
        self.lists[to].tasks.push(task);
        true
    }
}

/// Fills in the bits a hand edited (or very old) file may be missing.
fn normalized(mut state: AppState) -> AppState {
    if state.lists.is_empty() {
        state.lists.push(List::new(DEFAULT_LIST));
    }
    for (index, list) in state.lists.iter_mut().enumerate() {
        let name = list.name.trim().to_string();
        if name.is_empty() {
            list.name = format!("List {}", index + 1);
        } else {
            list.name = name;
        }
    }
    if state.active >= state.lists.len() {
        state.active = state.lists.len() - 1;
    }
    state
}

fn from_tasks(tasks: Vec<Task>, widget: Widget) -> AppState {
    normalized(AppState {
        lists: vec![List {
            name: DEFAULT_LIST.to_string(),
            tasks,
        }],
        active: 0,
        hide_completed: false,
        widget,
    })
}

/// Shape of the file written before lists existed.
#[derive(Deserialize)]
struct LegacyState {
    #[serde(default)]
    tasks: Vec<Task>,
    #[serde(default)]
    widget: Widget,
}

/// `%APPDATA%\DesktopTodo` (falls back to `%LOCALAPPDATA%`).
pub fn data_dir() -> PathBuf {
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

/// Log file used when the widget fails to start.
pub fn log_path() -> PathBuf {
    data_dir().join("startup.log")
}

pub fn load() -> AppState {
    let path = state_path();

    if let Ok(text) = fs::read_to_string(&path) {
        if let Some(state) = parse(&text) {
            return state;
        }
    }

    // One time migration of the file old builds dropped next to the executable.
    if let Ok(text) = fs::read_to_string("tasks.json") {
        if let Ok(tasks) = serde_json::from_str::<Vec<Task>>(&text) {
            let state = from_tasks(tasks, Widget::default());
            let _ = save(&state);
            return state;
        }
    }

    normalized(AppState::new())
}

/// Reads a state file, falling back to the formats older builds wrote.
fn parse(text: &str) -> Option<AppState> {
    if let Ok(state) = serde_json::from_str::<AppState>(text) {
        // A file without any list predates lists, its tasks are still in the
        // top level `tasks` field.
        if !state.lists.is_empty() {
            return Some(normalized(state));
        }
    }
    if let Ok(old) = serde_json::from_str::<LegacyState>(text) {
        return Some(from_tasks(old.tasks, old.widget));
    }
    // Very old builds stored a bare array of tasks.
    if let Ok(tasks) = serde_json::from_str::<Vec<Task>>(text) {
        return Some(from_tasks(tasks, Widget::default()));
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    fn task(text: &str, done: bool) -> Task {
        Task {
            text: text.to_string(),
            done,
        }
    }

    #[test]
    fn reads_the_list_format() {
        let state =
            parse(r#"{"lists":[{"name":"Home","tasks":[{"text":"a","done":true}]}],"active":0}"#)
                .unwrap();
        assert_eq!(state.lists.len(), 1);
        assert_eq!(state.active_name(), "Home");
        assert_eq!(state.tasks(), &[task("a", true)]);
    }

    #[test]
    fn migrates_a_flat_task_file() {
        let state =
            parse(r#"{"tasks":[{"text":"a","done":false}],"widget":{"x":5,"y":6}}"#).unwrap();
        assert_eq!(state.lists.len(), 1);
        assert_eq!(state.active_name(), DEFAULT_LIST);
        assert_eq!(state.tasks(), &[task("a", false)]);
        assert_eq!(state.widget.x, 5);
        assert_eq!(state.widget.y, 6);
    }

    #[test]
    fn migrates_a_bare_task_array() {
        let state = parse(r#"[{"text":"a","done":false}]"#).unwrap();
        assert_eq!(state.lists.len(), 1);
        assert_eq!(state.tasks(), &[task("a", false)]);
    }

    #[test]
    fn rejects_a_broken_file() {
        assert!(parse("not json").is_none());
    }

    #[test]
    fn switches_between_lists() {
        let mut state = AppState::new();
        state.add_list();
        state.add_list();
        assert_eq!(state.active, 1);
        assert_eq!(state.active_name(), "List 2");
        assert!(state.cycle_list(1));
        assert_eq!(state.active, 0);
        assert!(state.cycle_list(-1));
        assert_eq!(state.active, 1);
    }

    #[test]
    fn keeps_the_last_list() {
        let mut state = AppState::new();
        state.tasks_mut().push(task("a", false));
        assert_eq!(state.lists.len(), 1);
        assert!(!state.remove_active_list());
        assert_eq!(state.lists.len(), 1);
    }

    #[test]
    fn hides_completed_tasks_when_asked() {
        let mut state = AppState::new();
        let tasks = state.tasks_mut();
        tasks.push(task("a", false));
        tasks.push(task("b", true));
        tasks.push(task("c", false));

        assert_eq!(state.visible(), vec![0, 1, 2]);
        state.hide_completed = true;
        assert_eq!(state.visible(), vec![0, 2]);
    }

    #[test]
    fn moves_a_task_to_another_list() {
        let mut state = AppState::new();
        state
            .tasks_mut()
            .extend([task("a", false), task("b", false)]);
        state.add_list();
        state.cycle_list(-1);
        assert_eq!(state.active_name(), DEFAULT_LIST);

        assert!(state.move_task(1, 1));
        assert_eq!(state.lists[0].tasks, vec![task("a", false)]);
        assert_eq!(state.lists[1].tasks, vec![task("b", false)]);

        assert!(!state.move_task(0, 0)); // already there
        assert!(!state.move_task(5, 0)); // no such list
        assert!(!state.move_task(1, 7)); // no such task
    }

    #[test]
    fn restores_a_valid_active_list() {
        let state = parse(r#"{"lists":[{"name":"a"},{"name":"b"}],"active":9}"#).unwrap();
        assert_eq!(state.active, 1);
        assert_eq!(state.active_name(), "b");
    }
}
