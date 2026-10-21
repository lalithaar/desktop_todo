#![windows_subsystem = "windows"]

mod app;
mod render;
mod state;

use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, SetFocus, VK_DELETE, VK_ESCAPE, VK_RETURN, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use app::{h_editor, h_input, set_editor, set_input, Task, APP, UI};
use render::{CHECK, INPUT_H, PAD, ROW_H, TEXT_X, TITLE_H, WIDGET_W};

const WM_APP_COMMIT: u32 = WM_APP + 1;
const WM_APP_CANCEL: u32 = WM_APP + 2;
const WM_APP_KEY: u32 = WM_APP + 3;

const MENU_TOGGLE: usize = 100;
const MENU_EDIT: usize = 101;
const MENU_DELETE: usize = 102;
const MENU_ADD: usize = 103;
const MENU_CLEAR: usize = 104;
const MENU_EXIT: usize = 105;

const ID_INPUT: usize = 0x1001;
const ID_EDITOR: usize = 0x1002;

const TIMER_PIN: usize = 1;

const EM_SETSEL: u32 = 0x00B1;
const EM_SETCUEBANNER: u32 = 0x1503;

const WA_ACTIVE: isize = 0;

type EditProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

static EDIT_PROC: AtomicIsize = AtomicIsize::new(0);
static EDIT_BRUSH: AtomicIsize = AtomicIsize::new(0);
static ANCHOR: AtomicIsize = AtomicIsize::new(0);
static ANCHOR_AGE: AtomicUsize = AtomicUsize::new(0);

fn main() {
    if let Err(err) = run() {
        let detail = format!("Desktop Todo failed to start.\n\n{err:?}\n");
        let _ = std::fs::create_dir_all(state::data_dir());
        let _ = std::fs::write(state::log_path(), &detail);
        unsafe {
            let _ = MessageBoxW(
                HWND(std::ptr::null_mut()),
                w!("Desktop Todo could not start. See startup.log in %APPDATA%\\DesktopTodo."),
                w!("Desktop Todo"),
                MESSAGEBOX_STYLE(MB_OK.0 | MB_ICONERROR.0),
            );
        }
    }
}

fn run() -> Result<()> {
    {
        let fresh = !state::state_path().exists();
        let mut data = APP.lock().unwrap();
        *data = state::load();
        if fresh {
            // First start: park the widget in the lower right corner.
            let (screen_w, screen_h) = unsafe {
                (
                    GetSystemMetrics(SM_CXSCREEN),
                    GetSystemMetrics(SM_CYSCREEN),
                )
            };
            let height = render::height_for(data.tasks.len());
            data.widget.x = (screen_w - WIDGET_W - 40).max(40);
            data.widget.y = (screen_h - height - 90).max(40);
        }
    }

    unsafe {
        // Only one widget at a time, a second start just wakes the first one.
        if let Ok(existing) = FindWindowW(w!("DesktopTodoWidgetClass"), PCWSTR::null()) {
            let _ = SetForegroundWindow(existing);
            return Ok(());
        }

        let instance = GetModuleHandleW(PCWSTR::null())?;
        let class_name = w!("DesktopTodoWidgetClass");

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(HINSTANCE(std::ptr::null_mut()), IDC_ARROW)?,
            style: CS_DBLCLKS,
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);

        let (x, y) = {
            let data = APP.lock().unwrap();
            (data.widget.x, data.widget.y)
        };
        let rows = APP.lock().unwrap().tasks.len();

        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            class_name,
            w!("Desktop Todo"),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            WIDGET_W,
            render::height_for(rows),
            HWND(std::ptr::null_mut()),
            HMENU(std::ptr::null_mut()),
            instance,
            None,
        )?;

        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 240, LWA_ALPHA);

        create_controls(hwnd, instance)?;
        sync_layout(hwnd);
        SetTimer(hwnd, TIMER_PIN, 400, None);
        pin_to_background(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}

// ------------------------------------------------------------- controls -----

unsafe fn create_controls(hwnd: HWND, instance: HMODULE) -> Result<()> {
    let font = CreateFontW(
        -15,
        0,
        0,
        0,
        FW_NORMAL.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32,
        DEFAULT_PITCH.0 as u32,
        w!("Segoe UI"),
    );

    let input = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("EDIT"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | ES_AUTOHSCROLL as u32),
        0,
        0,
        10,
        10,
        hwnd,
        HMENU(ID_INPUT as *mut core::ffi::c_void),
        instance,
        None,
    )?;
    SendMessageW(input, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    set_input(input);
    subclass(input);
    set_cue(input, "Add a task...");

    let editor = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("EDIT"),
        w!(""),
        WINDOW_STYLE(WS_CHILD.0 | ES_AUTOHSCROLL as u32),
        0,
        0,
        10,
        10,
        hwnd,
        HMENU(ID_EDITOR as *mut core::ffi::c_void),
        instance,
        None,
    )?;
    SendMessageW(editor, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
    set_editor(editor);
    subclass(editor);

    Ok(())
}

unsafe fn subclass(hwnd: HWND) {
    let prev = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, edit_proc as EditProc as isize);
    EDIT_PROC.store(prev as isize, Ordering::Relaxed);
}

/// The edit controls are subclassed so Enter / Escape can be handled and so
/// the caret never leaves the widget.
unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_KEYDOWN {
        let key = wparam.0 as u32;
        let parent = GetParent(hwnd).unwrap_or(HWND(std::ptr::null_mut()));
        let tag = if hwnd.0 == h_editor().0 { 0 } else { 1 };
        if key == VK_RETURN.0 as u32 {
            SendMessageW(parent, WM_APP_COMMIT, WPARAM(tag), LPARAM(0));
            return LRESULT(0);
        }
        if key == VK_ESCAPE.0 as u32 {
            SendMessageW(parent, WM_APP_CANCEL, WPARAM(tag), LPARAM(0));
            return LRESULT(0);
        }
        // Shortcut for the selected task, it only fires when the text field
        // is empty so that typing keeps working.
        if key == VK_DELETE.0 as u32 && window_text(hwnd).is_empty() {
            SendMessageW(parent, WM_APP_KEY, WPARAM(key as usize), LPARAM(0));
            return LRESULT(0);
        }
    }
    if msg == WM_NCDESTROY {
        let prev = EDIT_PROC.load(Ordering::Relaxed);
        SetWindowLongPtrW(hwnd, GWLP_WNDPROC, prev);
    }

    let prev = EDIT_PROC.load(Ordering::Relaxed);
    let proc: EditProc = std::mem::transmute(prev as usize);
    CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam)
}

unsafe fn set_cue(hwnd: HWND, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    SendMessageW(
        hwnd,
        EM_SETCUEBANNER,
        WPARAM(1),
        LPARAM(wide.as_ptr() as isize),
    );
}

// ------------------------------------------------------------- wndproc ------

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            render::paint(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_TIMER => {
            if wparam.0 == TIMER_PIN {
                pin_to_background(hwnd);
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            let active = (lparam.0 & 0xFFFF) == WA_ACTIVE;
            if !active {
                // Another window got the focus, so the widget may go back
                // into the background now.
                {
                    let mut ui = UI.lock().unwrap();
                    ui.interactive = false;
                }
                pin_to_background(hwnd);
            }
            LRESULT(0)
        }
        WM_MOUSEACTIVATE => {
            // Only take the foreground when the user aims at the text field,
            // otherwise the widget stays in the background.
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = ScreenToClient(hwnd, &mut pt);
            let mut r = RECT::default();
            let _ = GetClientRect(hwnd, &mut r);
            let h = r.bottom - r.top;
            if pt.y >= h - INPUT_H {
                LRESULT(MA_ACTIVATE as isize)
            } else {
                LRESULT(MA_NOACTIVATE as isize)
            }
        }
        WM_LBUTTONDOWN => {
            on_left_down(hwnd, lparam);
            LRESULT(0)
        }
        WM_LBUTTONDBLCLK => {
            on_double_click(hwnd, lparam);
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            on_left_up(hwnd);
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            let mut ui = UI.lock().unwrap();
            ui.dragging = false;
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            on_mouse_move(hwnd);
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            on_right_up(hwnd, lparam);
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            on_mouse_wheel(hwnd, wparam);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            on_key_down(hwnd, wparam);
            LRESULT(0)
        }
        WM_COMMAND => {
            on_command(hwnd, wparam);
            LRESULT(0)
        }
        WM_APP_COMMIT => {
            on_commit(hwnd, wparam.0);
            LRESULT(0)
        }
        WM_APP_CANCEL => {
            on_cancel(hwnd, wparam.0);
            LRESULT(0)
        }
        WM_APP_KEY => {
            on_app_key(hwnd, wparam.0);
            LRESULT(0)
        }
        WM_CTLCOLOREDIT => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            SetTextColor(hdc, COLORREF(render::TEXT));
            SetBkColor(hdc, COLORREF(render::PANEL));
            LRESULT(edit_brush().0 as isize)
        }
        WM_CLOSE => {
            save_all();
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            save_all();
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ------------------------------------------------------------- helpers ------

fn edit_brush() -> HBRUSH {
    let cached = EDIT_BRUSH.load(Ordering::Relaxed);
    if cached != 0 {
        return HBRUSH(cached as *mut core::ffi::c_void);
    }
    unsafe {
        let brush = CreateSolidBrush(COLORREF(render::PANEL));
        EDIT_BRUSH.store(brush.0 as isize, Ordering::Relaxed);
        brush
    }
}

fn save_all() {
    let app = APP.lock().unwrap();
    let _ = state::save(&app);
}

/// Keeps the widget in the background unless the user is actively working in it.
fn pin_to_background(hwnd: HWND) {
    let busy = {
        let ui = UI.lock().unwrap();
        ui.busy()
    };
    if busy {
        return;
    }
    unsafe {
        // Park the widget directly on top of the shell desktop layer so it
        // stays visible above the wallpaper but below every other window.
        let target = match cached_anchor() {
            Some(anchor) => anchor,
            None => HWND_BOTTOM,
        };
        let _ = SetWindowPos(
            hwnd,
            target,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

unsafe extern "system" fn collect_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<HWND>);
    list.push(hwnd);
    TRUE
}

unsafe fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 64];
    let len = GetClassNameW(hwnd, &mut buf).max(0) as usize;
    String::from_utf16_lossy(&buf[..len.min(buf.len())])
}

/// True when the window owns the desktop icon view.
unsafe fn owns_icon_view(hwnd: HWND) -> bool {
    FindWindowExW(
        hwnd,
        HWND(std::ptr::null_mut()),
        w!("SHELLDLL_DefView"),
        PCWSTR::null(),
    )
    .is_ok()
}

/// Picks the window the widget has to be placed behind so that it ends up
/// directly on top of the desktop icons but below every application window.
unsafe fn desktop_anchor() -> Option<HWND> {
    let mut list: Vec<HWND> = Vec::new();
    if EnumWindows(
        Some(collect_windows),
        LPARAM(&mut list as *mut Vec<HWND> as isize),
    )
    .is_err()
    {
        return None;
    }

    // The list is ordered from the topmost window to the bottommost one.
    let mut host: Option<usize> = None;
    for (index, hwnd) in list.iter().enumerate() {
        if owns_icon_view(*hwnd) {
            host = Some(index);
            break;
        }
    }

    let index = match host {
        Some(index) => index,
        None => {
            // No icon view (unusual), fall back to the shell desktop window.
            for (index, hwnd) in list.iter().enumerate() {
                let class = class_name(*hwnd);
                if class == "Progman" {
                    host = Some(index);
                    break;
                }
            }
            match host {
                Some(index) => index,
                None => return None,
            }
        }
    };

    // The window right above the desktop layer. Inserting the widget behind
    // it parks the widget directly on top of the desktop icons, so it stays
    // visible and clickable while every application window still covers it.
    if index == 0 {
        return None;
    }
    list.get(index - 1).copied()
}

/// The anchor is cached for a couple of seconds, refreshing it is not free.
unsafe fn cached_anchor() -> Option<HWND> {
    let cached = ANCHOR.load(Ordering::Relaxed);
    let age = ANCHOR_AGE.fetch_add(1, Ordering::Relaxed);
    let stale = age % 5 == 0;

    if cached != 0 && !stale {
        let anchor = HWND(cached as *mut core::ffi::c_void);
        if IsWindow(anchor).as_bool() {
            return Some(anchor);
        }
    }

    match desktop_anchor() {
        Some(anchor) => {
            ANCHOR.store(anchor.0 as isize, Ordering::Relaxed);
            Some(anchor)
        }
        None => {
            ANCHOR.store(0, Ordering::Relaxed);
            None
        }
    }
}

unsafe fn client_size(hwnd: HWND) -> (i32, i32) {
    let mut r = RECT::default();
    let _ = GetClientRect(hwnd, &mut r);
    (r.right - r.left, r.bottom - r.top)
}

unsafe fn window_text(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd).max(0) as usize;
    if len == 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len + 1];
    let copied = GetWindowTextW(hwnd, &mut buf).max(0) as usize;
    String::from_utf16_lossy(&buf[..copied.min(len)])
}

unsafe fn set_text(hwnd: HWND, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = SetWindowTextW(hwnd, PCWSTR(wide.as_ptr()));
}

/// Index of the task that lives under the given client y coordinate.
fn row_at(y: i32, scroll: i32, count: usize) -> Option<usize> {
    if y < TITLE_H {
        return None;
    }
    let rel = y - TITLE_H + scroll;
    if rel < 0 {
        return None;
    }
    let index = (rel / ROW_H) as usize;
    if index < count {
        Some(index)
    } else {
        None
    }
}

fn clamp_scroll(hwnd: HWND) {
    unsafe {
        let (_, h) = client_size(hwnd);
        let list_height = (h - TITLE_H - INPUT_H).max(0);
        let count = APP.lock().unwrap().tasks.len() as i32;
        let max = (count * ROW_H - list_height).max(0);
        let mut ui = UI.lock().unwrap();
        ui.scroll = ui.scroll.clamp(0, max);
    }
}

/// Resizes the widget to fit the task list and keeps the input field in place.
unsafe fn sync_layout(hwnd: HWND) {
    let count = APP.lock().unwrap().tasks.len();
    let height = render::height_for(count);
    let (_, h) = client_size(hwnd);
    if h != height {
        let mut wr = RECT::default();
        let _ = GetWindowRect(hwnd, &mut wr);
        let _ = SetWindowPos(
            hwnd,
            HWND(std::ptr::null_mut()),
            0,
            0,
            wr.right - wr.left,
            height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    let (w, h) = client_size(hwnd);
    let _ = MoveWindow(
        h_input(),
        TEXT_X,
        h - INPUT_H + 11,
        (w - TEXT_X - PAD).max(40),
        24,
        TRUE,
    );
    clamp_scroll(hwnd);
}

fn toggle_task(index: usize) {
    {
        let mut app = APP.lock().unwrap();
        if index < app.tasks.len() {
            app.tasks[index].done = !app.tasks[index].done;
        }
    }
    save_all();
}

fn delete_task(index: usize) {
    {
        let mut app = APP.lock().unwrap();
        if index < app.tasks.len() {
            app.tasks.remove(index);
        }
    }
    let mut hide_editor = false;
    {
        let mut ui = UI.lock().unwrap();
        ui.selected = None;
        match ui.editing {
            Some(edit) if edit == index => {
                ui.editing = None;
                hide_editor = true;
            }
            Some(edit) if edit > index => ui.editing = Some(edit - 1),
            _ => {}
        }
    }
    if hide_editor {
        unsafe {
            let _ = ShowWindow(h_editor(), SW_HIDE);
        }
    }
    save_all();
}

fn clear_completed() {
    {
        let mut app = APP.lock().unwrap();
        app.tasks.retain(|task| !task.done);
    }
    save_all();
}

// ------------------------------------------------------------- handlers -----

unsafe fn on_left_down(hwnd: HWND, lparam: LPARAM) {
    {
        let mut ui = UI.lock().unwrap();
        ui.touch();
    }
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    let (w, h) = client_size(hwnd);

    if y < TITLE_H {
        if x >= w - 44 {
            save_all();
            let _ = DestroyWindow(hwnd);
            return;
        }
        // drag the widget around
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let mut wr = RECT::default();
        let _ = GetWindowRect(hwnd, &mut wr);
        {
            let mut ui = UI.lock().unwrap();
            ui.dragging = true;
            ui.drag_cursor = pt;
            ui.drag_origin = POINT {
                x: wr.left,
                y: wr.top,
            };
        }
        let _ = SetCapture(hwnd);
        return;
    }

    if y >= h - INPUT_H {
        // the little "+" next to the input field
        focus_input(hwnd);
        return;
    }

    let (scroll, count) = app::snapshot(|app, ui| (ui.scroll, app.tasks.len()));
    if let Some(index) = row_at(y, scroll, count) {
        if x >= PAD - 6 && x <= PAD + CHECK + 6 {
            toggle_task(index);
        } else {
            let mut ui = UI.lock().unwrap();
            ui.selected = Some(index);
        }
        let _ = InvalidateRect(hwnd, None, TRUE);
    }
}

unsafe fn on_double_click(hwnd: HWND, lparam: LPARAM) {
    {
        let mut ui = UI.lock().unwrap();
        ui.touch();
    }
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    let (_, h) = client_size(hwnd);
    if y >= h - INPUT_H {
        return;
    }
    if x >= PAD - 6 && x <= PAD + CHECK + 6 {
        return;
    }
    let (scroll, count) = app::snapshot(|app, ui| (ui.scroll, app.tasks.len()));
    if let Some(index) = row_at(y, scroll, count) {
        start_editing(hwnd, index);
    }
}

unsafe fn on_left_up(hwnd: HWND) {
    let dragging = {
        let mut ui = UI.lock().unwrap();
        let dragging = ui.dragging;
        ui.dragging = false;
        dragging
    };
    if dragging {
        let _ = ReleaseCapture();
        let mut wr = RECT::default();
        let _ = GetWindowRect(hwnd, &mut wr);
        {
            let mut app = APP.lock().unwrap();
            app.widget.x = wr.left;
            app.widget.y = wr.top;
        }
        save_all();
    }
}

unsafe fn on_mouse_move(hwnd: HWND) {
    let drag = {
        let ui = UI.lock().unwrap();
        if ui.dragging {
            Some((ui.drag_cursor, ui.drag_origin))
        } else {
            None
        }
    };
    if let Some((cursor, origin)) = drag {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // Move the widget without touching its size or its place in the
        // z-order, it has to stay glued to the desktop while dragging.
        let _ = SetWindowPos(
            hwnd,
            HWND(std::ptr::null_mut()),
            origin.x + (pt.x - cursor.x),
            origin.y + (pt.y - cursor.y),
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

unsafe fn on_right_up(hwnd: HWND, lparam: LPARAM) {
    {
        let mut ui = UI.lock().unwrap();
        ui.touch();
    }
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    let (_, h) = client_size(hwnd);

    let target = if y >= TITLE_H && y < h - INPUT_H {
        let (scroll, count) = app::snapshot(|app, ui| (ui.scroll, app.tasks.len()));
        row_at(y, scroll, count)
    } else {
        None
    };

    let mut pt = POINT { x, y };
    let _ = ClientToScreen(hwnd, &mut pt);
    show_menu(hwnd, pt.x, pt.y, target);
}

unsafe fn on_mouse_wheel(hwnd: HWND, wparam: WPARAM) {
    let delta = (wparam.0 >> 16) as i16 as i32;
    {
        let mut ui = UI.lock().unwrap();
        ui.touch();
        ui.scroll -= delta / 120 * ROW_H;
    }
    clamp_scroll(hwnd);
    let _ = InvalidateRect(hwnd, None, TRUE);
}

unsafe fn on_key_down(hwnd: HWND, wparam: WPARAM) {
    let key = wparam.0 as u32;
    let selected = UI.lock().unwrap().selected;
    match key {
        k if k == VK_DELETE.0 as u32 => {
            if let Some(index) = selected {
                delete_task(index);
                sync_layout(hwnd);
            }
        }
        k if k == VK_RETURN.0 as u32 || k == VK_SPACE.0 as u32 => {
            if let Some(index) = selected {
                toggle_task(index);
            }
        }
        k if k == VK_ESCAPE.0 as u32 => {
            let mut ui = UI.lock().unwrap();
            ui.selected = None;
        }
        _ => {}
    }
    let _ = InvalidateRect(hwnd, None, TRUE);
}

unsafe fn on_app_key(hwnd: HWND, key: usize) {
    let selected = UI.lock().unwrap().selected;
    if let Some(index) = selected {
        if key == VK_DELETE.0 as usize {
            delete_task(index);
            sync_layout(hwnd);
        }
    }
    let _ = InvalidateRect(hwnd, None, TRUE);
}

unsafe fn on_command(hwnd: HWND, wparam: WPARAM) {
    let raw = wparam.0 as u32;
    let id = (raw & 0xFFFF) as usize;
    let code = (raw >> 16) & 0xFFFF;
    if code != EN_KILLFOCUS {
        return;
    }
    if id == ID_INPUT {
        // the user clicked away from the input field
        {
            let mut ui = UI.lock().unwrap();
            ui.interactive = false;
        }
        pin_to_background(hwnd);
    } else if id == ID_EDITOR {
        // the inline editor lost the focus, close it
        let editing = {
            let mut ui = UI.lock().unwrap();
            ui.editing.take()
        };
        if editing.is_some() {
            let _ = ShowWindow(h_editor(), SW_HIDE);
        }
    }
}

unsafe fn on_commit(hwnd: HWND, tag: usize) {
    if tag == 0 {
        // inline editor
        let editor = h_editor();
        let text = window_text(editor);
        let index = {
            let mut ui = UI.lock().unwrap();
            ui.editing.take()
        };
        if let Some(index) = index {
            let trimmed = text.trim().to_string();
            let mut app = APP.lock().unwrap();
            if index < app.tasks.len() {
                if trimmed.is_empty() {
                    app.tasks.remove(index);
                } else {
                    app.tasks[index].text = trimmed;
                }
            }
        }
        let _ = ShowWindow(editor, SW_HIDE);
        {
            let mut ui = UI.lock().unwrap();
            ui.interactive = false;
        }
        save_all();
        sync_layout(hwnd);
        let _ = InvalidateRect(hwnd, None, TRUE);
    } else {
        // the add field
        let input = h_input();
        let text = window_text(input);
        let trimmed = text.trim().to_string();
        if !trimmed.is_empty() {
            let mut app = APP.lock().unwrap();
            app.tasks.push(Task {
                text: trimmed,
                done: false,
            });
        }
        set_text(input, "");
        let _ = SetFocus(input);
        save_all();
        sync_layout(hwnd);
        let _ = InvalidateRect(hwnd, None, TRUE);
    }
}

unsafe fn on_cancel(hwnd: HWND, tag: usize) {
    if tag == 0 {
        let _ = ShowWindow(h_editor(), SW_HIDE);
        let mut ui = UI.lock().unwrap();
        ui.editing = None;
        ui.interactive = false;
    } else {
        set_text(h_input(), "");
    }
    let _ = InvalidateRect(hwnd, None, TRUE);
}

unsafe fn start_editing(hwnd: HWND, index: usize) {
    let editor = h_editor();
    let text = {
        let app = APP.lock().unwrap();
        app.tasks.get(index).map(|t| t.text.clone()).unwrap_or_default()
    };
    set_text(editor, &text);

    let (w, _) = client_size(hwnd);
    let scroll = UI.lock().unwrap().scroll;
    let row_y = TITLE_H + index as i32 * ROW_H - scroll;
    let _ = MoveWindow(
        editor,
        TEXT_X - 6,
        row_y + 3,
        (w - TEXT_X - PAD + 6).max(40),
        ROW_H - 6,
        TRUE,
    );
    let _ = ShowWindow(editor, SW_SHOW);
    SendMessageW(editor, EM_SETSEL, WPARAM(0), LPARAM(-1));

    {
        let mut ui = UI.lock().unwrap();
        ui.editing = Some(index);
        ui.interactive = true;
    }
    let _ = SetForegroundWindow(hwnd);
    let _ = SetFocus(editor);
}

unsafe fn focus_input(hwnd: HWND) {
    {
        let mut ui = UI.lock().unwrap();
        ui.interactive = true;
    }
    let _ = SetForegroundWindow(hwnd);
    let _ = SetFocus(h_input());
    let _ = InvalidateRect(hwnd, None, TRUE);
}

unsafe fn show_menu(hwnd: HWND, x: i32, y: i32, target: Option<usize>) {
    let menu = match CreatePopupMenu() {
        Ok(menu) => menu,
        Err(_) => return,
    };

    if let Some(index) = target {
        let done = APP
            .lock()
            .unwrap()
            .tasks
            .get(index)
            .map(|task| task.done)
            .unwrap_or(false);
        let label = if done {
            "Mark as not done"
        } else {
            "Mark as done"
        };
        append_menu(menu, MENU_TOGGLE, label);
        append_menu(menu, MENU_EDIT, "Edit task");
        append_menu(menu, MENU_DELETE, "Delete task");
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR(std::ptr::null()));
    }

    append_menu(menu, MENU_ADD, "Add task");
    append_menu(menu, MENU_CLEAR, "Clear completed");
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR(std::ptr::null()));
    append_menu(menu, MENU_EXIT, "Exit");

    let _ = SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_RIGHTBUTTON,
        x,
        y,
        0,
        hwnd,
        None,
    );
    let _ = DestroyMenu(menu);
    let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));

    {
        let mut ui = UI.lock().unwrap();
        ui.interactive = false;
    }

    match cmd.0 as usize {
        MENU_TOGGLE => {
            if let Some(index) = target {
                toggle_task(index);
            }
        }
        MENU_EDIT => {
            if let Some(index) = target {
                start_editing(hwnd, index);
            }
        }
        MENU_DELETE => {
            if let Some(index) = target {
                delete_task(index);
                sync_layout(hwnd);
            }
        }
        MENU_ADD => focus_input(hwnd),
        MENU_CLEAR => {
            clear_completed();
            sync_layout(hwnd);
        }
        MENU_EXIT => {
            save_all();
            let _ = DestroyWindow(hwnd);
        }
        _ => {}
    }

    pin_to_background(hwnd);
    let _ = InvalidateRect(hwnd, None, TRUE);
}

unsafe fn append_menu(menu: HMENU, id: usize, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = AppendMenuW(menu, MF_STRING, id, PCWSTR(wide.as_ptr()));
}
