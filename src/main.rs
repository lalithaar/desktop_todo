#![windows_subsystem = "windows"]

use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Mutex;
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::WindowsAndMessaging::*,
};

#[derive(Serialize, Deserialize, Clone)]
struct TodoItem {
    text: String,
    done: bool,
}

static ITEMS: Mutex<Vec<TodoItem>> = Mutex::new(Vec::new());

fn load_items() {
    if let Ok(data) = fs::read_to_string("tasks.json") {
        if let Ok(parsed) = serde_json::from_str(&data) {
            *ITEMS.lock().unwrap() = parsed;
            return;
        }
    }
    // Default initial items
    *ITEMS.lock().unwrap() = vec![
        TodoItem { text: "visit dentist".into(), done: false },
        TodoItem { text: "go to canara bank".into(), done: false },
        TodoItem { text: "check safe nutritional supplements".into(), done: false },
    ];
}

fn save_items() {
    let items = ITEMS.lock().unwrap();
    if let Ok(json) = serde_json::to_string_pretty(&*items) {
        let _ = fs::write("tasks.json", json);
    }
}

fn main() -> Result<()> {
    load_items();

    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class_name = w!("DesktopTodoWidgetClass");

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };

        RegisterClassW(&wc);

        // WS_EX_LAYERED enables transparency; WS_EX_TOPMOST keeps it pinned
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
            class_name,
            w!("ToDo"),
            WS_POPUP | WS_VISIBLE,
            100, 100, 360, 480,
            None, None, instance, None,
        )?;

        // Set opacity: 215 / 255 gives the dark frosted glass opacity
        SetLayeredWindowAttributes(hwnd, COLORREF(0), 215, LWA_ALPHA)?;

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            // Background fill
            let mut rect = RECT::default();
            GetClientRect(hwnd, &mut rect);
            let bg_brush = CreateSolidBrush(COLORREF(0x1E1E1E)); // Dark grey
            FillRect(hdc, &rect, bg_brush);
            DeleteObject(bg_brush);

            SetBkMode(hdc, TRANSPARENT);

            // Draw "ToDo" Title
            let title_font = CreateFontW(
                22, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0,
                DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY, DEFAULT_PITCH.0 as u32, w!("Segoe UI"),
            );
            SelectObject(hdc, title_font);
            SetTextColor(hdc, COLORREF(0xFFFFFF));
            let mut title_rect = RECT { left: 24, top: 20, right: 330, bottom: 50 };
            let title = "ToDo\0".encode_utf16().collect::<Vec<u16>>();
            DrawTextW(hdc, &title[..title.len() - 1], &mut title_rect, DT_LEFT | DT_SINGLELINE);
            DeleteObject(title_font);

            // Draw Tasks
            let text_font = CreateFontW(
                16, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0,
                DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY, DEFAULT_PITCH.0 as u32, w!("Segoe UI"),
            );
            SelectObject(hdc, text_font);

            let items = ITEMS.lock().unwrap();
            let mut y = 60;

            for item in items.iter() {
                let bullet = if item.done { "●  [DONE] " } else { "●  " };
                let full_text = format!("{}{}\0", bullet, item.text);
                let wide: Vec<u16> = full_text.encode_utf16().collect();

                let color = if item.done {
                    COLORREF(0x7A7A7A) // Dim grey
                } else {
                    COLORREF(0xDCDCDC) // Light text
                };
                SetTextColor(hdc, color);

                let mut item_rect = RECT { left: 24, top: y, right: 336, bottom: y + 24 };
                DrawTextW(hdc, &wide[..wide.len() - 1], &mut item_rect, DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS);
                y += 28;
            }

            DeleteObject(text_font);
            EndPaint(hwnd, &ps);
            LRESULT(0)
        }

        // Left-click to drag the widget anywhere on the desktop
        WM_LBUTTONDOWN => {
            let y = (lparam.0 >> 16) as i16 as i32;
            if y >= 60 {
                let idx = ((y - 60) / 28) as usize;
                let mut items = ITEMS.lock().unwrap();
                if idx < items.len() {
                    items[idx].done = !items[idx].done;
                    drop(items);
                    save_items();
                    InvalidateRect(hwnd, None, TRUE);
                    return LRESULT(0);
                }
            }

            // If not clicking an item, start dragging the window
            ReleaseCapture().unwrap_or(());
            SendMessageW(hwnd, WM_NCLBUTTONDOWN, WPARAM(HTCAPTION as _), LPARAM(0));
            LRESULT(0)
        }

        // Right-click an item to remove it
        WM_RBUTTONDOWN => {
            let y = (lparam.0 >> 16) as i16 as i32;
            if y >= 60 {
                let idx = ((y - 60) / 28) as usize;
                let mut items = ITEMS.lock().unwrap();
                if idx < items.len() {
                    items.remove(idx);
                    drop(items);
                    save_items();
                    InvalidateRect(hwnd, None, TRUE);
                }
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
