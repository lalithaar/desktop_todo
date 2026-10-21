//! Optional "start with Windows" support.
//!
//! The widget is registered in the per user Run key, so it starts when the
//! user logs in. The entry points at the running executable, which keeps the
//! registration valid even if the widget is started from another folder.

use windows::core::PCWSTR;
use windows::Win32::Foundation::HKEY_CURRENT_USER;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegGetValueW, RegSetValueExW, HKEY,
    KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE, RRF_RT_REG_SZ,
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "DesktopTodo";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// True when the widget is registered to start with Windows.
pub fn is_enabled() -> bool {
    unsafe {
        let mut size: u32 = 0;
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(wide(RUN_KEY).as_ptr()),
            PCWSTR(wide(VALUE_NAME).as_ptr()),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        );
        if status.is_err() || size == 0 {
            return false;
        }

        let mut buffer = vec![0u16; (size as usize / 2) + 1];
        let mut kind = REG_VALUE_TYPE(0);
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(wide(RUN_KEY).as_ptr()),
            PCWSTR(wide(VALUE_NAME).as_ptr()),
            RRF_RT_REG_SZ,
            Some(&mut kind),
            Some(buffer.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut size),
        );
        if status.is_err() {
            return false;
        }
        let text = String::from_utf16_lossy(&buffer);
        !text.trim().is_empty()
    }
}

/// Registers or removes the widget from the Run key.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    unsafe {
        let mut key = HKEY(std::ptr::null_mut());
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(wide(RUN_KEY).as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        );
        if status.is_err() {
            return Err(format!("could not open the Run key: {status:?}"));
        }

        let result = if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let command = format!("\"{}\"", exe.display());
            let bytes: Vec<u8> = wide(&command).iter().map(|c| *c as u8).collect();
            let status = RegSetValueExW(
                key,
                PCWSTR(wide(VALUE_NAME).as_ptr()),
                0,
                REG_SZ,
                Some(&bytes),
            );
            if status.is_err() {
                Err(format!("could not write the Run entry: {status:?}"))
            } else {
                Ok(())
            }
        } else {
            let status = RegDeleteValueW(key, PCWSTR(wide(VALUE_NAME).as_ptr()));
            if status.is_err() {
                Err(format!("could not remove the Run entry: {status:?}"))
            } else {
                Ok(())
            }
        };

        let _ = RegCloseKey(key);
        result
    }
}
