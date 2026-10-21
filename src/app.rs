use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use windows::Win32::Foundation::{HWND, POINT};

pub use crate::state::{AppState, Task};

/// Widget only state (never persisted).
pub struct Ui {
    pub scroll: i32,
    pub selected: Option<usize>,
    pub editing: Option<usize>,
    pub dragging: bool,
    pub drag_cursor: POINT,
    pub drag_origin: POINT,
    /// While true the widget is allowed to sit in front of other windows.
    pub interactive: bool,
    /// Last time the user touched the widget, it stays in front for a while.
    pub touched: Option<Instant>,
}

impl Ui {
    pub const fn new() -> Self {
        Ui {
            scroll: 0,
            selected: None,
            editing: None,
            dragging: false,
            drag_cursor: POINT { x: 0, y: 0 },
            drag_origin: POINT { x: 0, y: 0 },
            interactive: false,
            touched: None,
        }
    }

    /// Marks the widget as just used by the user.
    pub fn touch(&mut self) {
        self.touched = Some(Instant::now());
    }

    /// True while the widget should stay in front of the other windows.
    pub fn busy(&self) -> bool {
        if self.interactive || self.editing.is_some() || self.dragging {
            return true;
        }
        match self.touched {
            Some(at) => at.elapsed().as_millis() < 2500,
            None => false,
        }
    }
}

pub static APP: Mutex<AppState> = Mutex::new(AppState::new());
pub static UI: Mutex<Ui> = Mutex::new(Ui::new());

static H_INPUT: AtomicIsize = AtomicIsize::new(0);
static H_EDITOR: AtomicIsize = AtomicIsize::new(0);

pub fn set_input(hwnd: HWND) {
    H_INPUT.store(hwnd.0 as isize, Ordering::Relaxed);
}

pub fn set_editor(hwnd: HWND) {
    H_EDITOR.store(hwnd.0 as isize, Ordering::Relaxed);
}

pub fn h_input() -> HWND {
    HWND(H_INPUT.load(Ordering::Relaxed) as *mut core::ffi::c_void)
}

pub fn h_editor() -> HWND {
    HWND(H_EDITOR.load(Ordering::Relaxed) as *mut core::ffi::c_void)
}

/// Reads the task list and the widget state at the same time.
/// The lock order (data first, widget state second) is the same everywhere.
pub fn snapshot<R>(f: impl FnOnce(&AppState, &Ui) -> R) -> R {
    let app = APP.lock().unwrap();
    let ui = UI.lock().unwrap();
    f(&app, &ui)
}
