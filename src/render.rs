use crate::app::{AppState, Ui};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

// ---------------------------------------------------------------- layout ----

pub const TITLE_H: i32 = 44;
pub const ROW_H: i32 = 28;
pub const INPUT_H: i32 = 46;
pub const PAD: i32 = 16;
pub const CHECK: i32 = 16;
pub const TEXT_X: i32 = PAD + CHECK + 10;
pub const WIDGET_W: i32 = 330;
pub const MIN_ROWS: i32 = 2;
pub const MAX_ROWS: i32 = 12;

// ---------------------------------------------------------------- colors ----

const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}

const BG: u32 = rgb(0x1E, 0x1E, 0x1E);
pub const PANEL: u32 = rgb(0x26, 0x26, 0x26);
const DIVIDER: u32 = rgb(0x38, 0x38, 0x38);
const BORDER: u32 = rgb(0x4A, 0x4A, 0x4A);
pub const TEXT: u32 = rgb(0xE4, 0xE4, 0xE4);
const MUTED: u32 = rgb(0x8A, 0x8A, 0x8A);
const DONE: u32 = rgb(0x7E, 0x7E, 0x7E);
const ACCENT: u32 = rgb(0x4C, 0x8D, 0xFF);
const SELECTED: u32 = rgb(0x2E, 0x3A, 0x4A);
const WHITE: u32 = rgb(0xFF, 0xFF, 0xFF);

/// Height of the widget for a given number of tasks.
pub fn height_for(task_count: usize) -> i32 {
    let rows = (task_count as i32).clamp(MIN_ROWS, MAX_ROWS);
    TITLE_H + INPUT_H + rows * ROW_H
}

// --------------------------------------------------------------- helpers ----

unsafe fn fill(hdc: HDC, rect: &RECT, color: u32) {
    let brush = CreateSolidBrush(COLORREF(color));
    let mut r = *rect;
    FillRect(hdc, &mut r, brush);
    let _ = DeleteObject(brush);
}

unsafe fn line(hdc: HDC, x1: i32, y1: i32, x2: i32, y2: i32, color: u32) {
    let pen = CreatePen(PS_SOLID, 1, COLORREF(color));
    let old = SelectObject(hdc, pen);
    let _ = MoveToEx(hdc, x1, y1, None);
    let _ = LineTo(hdc, x2, y2);
    SelectObject(hdc, old);
    let _ = DeleteObject(pen);
}

unsafe fn outline(hdc: HDC, rect: &RECT, color: u32) {
    let pen = CreatePen(PS_SOLID, 1, COLORREF(color));
    let old = SelectObject(hdc, pen);
    let _ = MoveToEx(hdc, rect.left, rect.top, None);
    let _ = LineTo(hdc, rect.right, rect.top);
    let _ = LineTo(hdc, rect.right, rect.bottom);
    let _ = LineTo(hdc, rect.left, rect.bottom);
    let _ = LineTo(hdc, rect.left, rect.top);
    SelectObject(hdc, old);
    let _ = DeleteObject(pen);
}

unsafe fn ui_font(height: i32, bold: bool) -> HFONT {
    let weight = if bold { FW_BOLD.0 as i32 } else { FW_NORMAL.0 as i32 };
    CreateFontW(
        height,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        CLEARTYPE_QUALITY.0 as u32,
        DEFAULT_PITCH.0 as u32,
        w!("Segoe UI"),
    )
}

// --------------------------------------------------------------- drawing ----

pub fn paint(hwnd: HWND) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);

        let mut client = RECT::default();
        let _ = GetClientRect(hwnd, &mut client);
        let w = client.right - client.left;
        let h = client.bottom - client.top;

        fill(hdc, &client, BG);
        SetBkMode(hdc, TRANSPARENT);

        let app = crate::app::APP.lock().unwrap();
        let ui = crate::app::UI.lock().unwrap();

        draw_title(hdc, w, &app);
        draw_input(hdc, w, h);
        draw_list(hdc, w, h, &app, &ui);

        EndPaint(hwnd, &ps);
    }
}

unsafe fn draw_title(hdc: HDC, w: i32, app: &AppState) {
    let font = ui_font(-19, true);
    let old = SelectObject(hdc, font);
    SetTextColor(hdc, COLORREF(TEXT));
    let mut title_rect = RECT {
        left: PAD,
        top: 0,
        right: w - 120,
        bottom: TITLE_H - 8,
    };
    let mut title: Vec<u16> = "ToDo".encode_utf16().collect();
    DrawTextW(
        hdc,
        &mut title,
        &mut title_rect,
        DT_LEFT | DT_SINGLELINE | DT_VCENTER,
    );
    SelectObject(hdc, old);
    let _ = DeleteObject(font);

    let open = app.tasks.iter().filter(|t| !t.done).count();
    let label = if app.tasks.is_empty() {
        "no tasks".to_string()
    } else if open == 0 {
        "all done".to_string()
    } else {
        format!("{} left", open)
    };
    SetTextColor(hdc, COLORREF(MUTED));
    let mut count_rect = RECT {
        left: w - 130,
        top: 0,
        right: w - 38,
        bottom: TITLE_H - 8,
    };
    let mut count: Vec<u16> = label.encode_utf16().collect();
    DrawTextW(
        hdc,
        &mut count,
        &mut count_rect,
        DT_RIGHT | DT_SINGLELINE | DT_VCENTER,
    );

    // close button
    let cx = w - 24;
    let cy = TITLE_H / 2 - 2;
    let pen = CreatePen(PS_SOLID, 1, COLORREF(MUTED));
    let old = SelectObject(hdc, pen);
    let _ = MoveToEx(hdc, cx - 5, cy - 5, None);
    let _ = LineTo(hdc, cx + 5, cy + 5);
    let _ = MoveToEx(hdc, cx + 5, cy - 5, None);
    let _ = LineTo(hdc, cx - 5, cy + 5);
    SelectObject(hdc, old);
    let _ = DeleteObject(pen);

    line(hdc, 0, TITLE_H - 1, w, TITLE_H - 1, DIVIDER);
}

unsafe fn draw_input(hdc: HDC, w: i32, h: i32) {
    let top = h - INPUT_H;
    line(hdc, 0, top, w, top, DIVIDER);

    let font = ui_font(-16, false);
    let old = SelectObject(hdc, font);
    SetTextColor(hdc, COLORREF(ACCENT));
    let mut plus_rect = RECT {
        left: PAD - 6,
        top: top + 4,
        right: PAD + 18,
        bottom: h - 4,
    };
    let mut plus: Vec<u16> = "+".encode_utf16().collect();
    DrawTextW(
        hdc,
        &mut plus,
        &mut plus_rect,
        DT_CENTER | DT_SINGLELINE | DT_VCENTER,
    );
    SelectObject(hdc, old);
    let _ = DeleteObject(font);
}

unsafe fn draw_list(hdc: HDC, w: i32, h: i32, app: &AppState, ui: &Ui) {
    let list_top = TITLE_H;
    let list_bottom = h - INPUT_H;

    if app.tasks.is_empty() {
        SetTextColor(hdc, COLORREF(MUTED));
        let mut hint_rect = RECT {
            left: PAD,
            top: list_top + 4,
            right: w - PAD,
            bottom: list_top + 32,
        };
        let mut hint: Vec<u16> = "No tasks yet - add one below".encode_utf16().collect();
        DrawTextW(
            hdc,
            &mut hint,
            &mut hint_rect,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );
        return;
    }

    let saved = SaveDC(hdc);
    let _ = IntersectClipRect(hdc, 0, list_top, w, list_bottom);

    let font = ui_font(-15, false);
    let old = SelectObject(hdc, font);

    for (index, task) in app.tasks.iter().enumerate() {
        let row_y = list_top + index as i32 * ROW_H - ui.scroll;
        if row_y + ROW_H <= list_top || row_y >= list_bottom {
            continue;
        }

        if ui.selected == Some(index) {
            let mut sel = RECT {
                left: 0,
                top: row_y,
                right: w,
                bottom: row_y + ROW_H,
            };
            fill(hdc, &sel, SELECTED);
        }

        // checkbox
        let box_top = row_y + (ROW_H - CHECK) / 2;
        let box_rect = RECT {
            left: PAD,
            top: box_top,
            right: PAD + CHECK,
            bottom: box_top + CHECK,
        };
        fill(
            hdc,
            &box_rect,
            if task.done { ACCENT } else { PANEL },
        );
        outline(
            hdc,
            &box_rect,
            if task.done { ACCENT } else { BORDER },
        );
        if task.done {
            let pen = CreatePen(PS_SOLID, 2, COLORREF(WHITE));
            let old = SelectObject(hdc, pen);
            let _ = MoveToEx(hdc, box_rect.left + 4, box_rect.top + 8, None);
            let _ = LineTo(hdc, box_rect.left + 7, box_rect.top + 12);
            let _ = LineTo(hdc, box_rect.left + 12, box_rect.top + 4);
            SelectObject(hdc, old);
            let _ = DeleteObject(pen);
        }

        // text
        SetTextColor(hdc, COLORREF(if task.done { DONE } else { TEXT }));
        let mut text_rect = RECT {
            left: TEXT_X,
            top: row_y + 2,
            right: w - PAD,
            bottom: row_y + ROW_H - 2,
        };
        let mut text: Vec<u16> = task.text.encode_utf16().collect();
        DrawTextW(
            hdc,
            &mut text,
            &mut text_rect,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );

        // strike through finished tasks
        if task.done {
            let mut size = SIZE::default();
            let _ = GetTextExtentPoint32W(hdc, &text, &mut size);
            let available = text_rect.right - text_rect.left;
            let width = size.cx.min(available);
            let middle = row_y + ROW_H / 2;
            line(hdc, TEXT_X, middle, TEXT_X + width, middle, DONE);
        }
    }

    SelectObject(hdc, old);
    let _ = DeleteObject(font);
    RestoreDC(hdc, saved);
}
