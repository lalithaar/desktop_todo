# desktop-todo

A small, local, single-user todo list for Windows. Type a task, press Enter,
close it, and it is still there next time. No accounts, no sync, no network.

- **Focus:** capture. The list sits on the desktop, not in a browser tab: type,
  hit <kbd>Enter</kbd>, keep going.
- **Stack:** Rust + Win32 (`windows` crate), custom-drawn GDI widget, no runtime
  dependencies
- **Data:** a plain JSON file under `%APPDATA%\DesktopTodo\`

## Download

Grab the latest release:

[**desktop-todo.exe**](https://github.com/lalithaar/desktop_todo/releases/download/v0.1.3/desktop-todo.exe)
([releases page](https://github.com/lalithaar/desktop_todo/releases))

There is no installer. Download the `.exe`, copy it anywhere you like, and run
it. It is a single self-contained binary.

## Build and run

Requirements: Rust (MSVC toolchain) and Visual Studio Build Tools with the
Windows SDK.

```powershell
cargo run --release
```

The binary is `target\release\desktop_todo.exe`. It is self-contained: copy it
anywhere and it runs.

## Using it

The widget parks itself on the desktop: above the wallpaper, below every other
window, but still clickable. Type in the field at the bottom and press
<kbd>Enter</kbd>.

| Action | How |
| --- | --- |
| Add a task | Type in the field at the bottom, <kbd>Enter</kbd> |
| Complete a task | Click its checkbox, or select it and press <kbd>Enter</kbd> / <kbd>Space</kbd> |
| Edit a task | Double-click it |
| Select a task | Click it |
| Select several | <kbd>Ctrl</kbd>+click each row |
| Delete | Select and press <kbd>Delete</kbd>, or right-click → **Delete task** |
| Move to another list | Right-click → **Move to ▸** list |
| Switch lists | Click <kbd>&lt;</kbd> / <kbd>&gt;</kbd> in the title bar |
| Rename a list | Double-click its name |
| New / delete list | Right-click → **New list** / **Delete list** |
| Hide finished tasks | Right-click → **Hide completed** |
| Scroll | Mouse wheel |
| Move the widget | Drag the title bar |
| Close | Click the × in the title bar, or right-click → **Exit** |

The context menu applies to the whole selection, so you get `Mark 3 as done`,
`Delete 3 tasks` and `Move to ▸` for a multi-selection. Right-clicking a row
that is not selected selects it first, so the menu always acts on what you
pointed at.

The widget position, the current list and the hide-completed setting are
remembered. The app is single-instance: launching it again just exits and
wakes the existing widget.

## How it works

```
src/
  main.rs         window, message loop, hit testing, context menu
  app.rs          global state and the widget-only UI state
  render.rs       title bar, rows, input field (GDI, no controls for rows)
  state.rs        lists, tasks, JSON persistence and migration
  autostart.rs    optional "Start with Windows" registry entry
```

The widget is a layered, tool-style window that is re-inserted just above the
desktop icon layer on a timer, so it stays visible over the wallpaper while
every normal window covers it. Rows are painted directly with GDI and
hit-tested by coordinate; only the add field and the inline editor are real
`EDIT` controls. A `WM_TIMER` re-parks the window in the z-order so clicking
through applications keeps working.

The state file is written atomically (`tasks.json.tmp` + rename) on every
change, so a crash cannot leave a half-written file.

## Data

```
%APPDATA%\DesktopTodo\
  tasks.json     lists, tasks, active list, hide-completed, widget position
  startup.log    written only when the app fails to start
```

```json
{
  "lists": [
    { "name": "Inbox", "tasks": [{ "text": "visit dentist", "done": false }] }
  ],
  "active": 0,
  "hide_completed": true,
  "widget": { "x": 1183, "y": 16 }
}
```

Files written by older builds (a bare task array, or `{ "tasks": [...] }`) are
migrated into a single list on load. Deleting the folder resets the app.

## Development

Unit tests cover the state file formats, list cycling, selection filtering and
moving tasks between lists:

```powershell
cargo test
```

The release build is `cargo build --release`.
