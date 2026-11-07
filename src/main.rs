//! jepret: a tiny screenshot and annotation tool for Windows.
//!
//!   PrtSc / F8   freeze the screen, drag a region, annotate it, Ctrl+C to copy or Ctrl+S to save
//!   Shift+PrtSc  save the whole screen at once
//!
//! While idle it is one thread blocked in GetMessage: no timers, no polling, ~1.3 MB working set.
//! The screen copy, overlay buffers and GDI+ exist only while a capture is open and are released after.

#![windows_subsystem = "windows"]
#![allow(static_mut_refs)]

mod annotate;
mod capture;
mod output;
mod overlay;
mod util;
mod win32;

use std::ptr::{null, null_mut};
use util::{log, wide};
use win32::*;

/// (id, modifiers, virtual key). F8 is kept because it was Lightshot's default key.
const HOTKEYS: [(i32, u32, u32); 3] = [(1, 0, 0x2C), (2, 0, 0x77), (3, 4 /* SHIFT */, 0x2C)];
static mut REGISTERED: [bool; 3] = [false; 3];

/// Registers whatever is still missing; true while any hotkey is still taken by another app.
unsafe fn register_hotkeys() -> bool {
    for (i, &(id, m, vk)) in HOTKEYS.iter().enumerate() {
        if !REGISTERED[i] { REGISTERED[i] = RegisterHotKey(null_mut(), id, m | 0x4000 /* NOREPEAT */, vk) != 0; }
    }
    REGISTERED.iter().any(|r| !r)
}

/// Hands freed pages back to the OS after a capture, so the idle footprint returns to ~1 MB.
pub fn trim() { unsafe { SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX); } }

fn main() {
    unsafe {
        SetProcessDpiAwarenessContext(-4); // per-monitor v2: physical pixels everywhere
        let _single = CreateMutexW(null(), 1, wide("Local\\jepret").as_ptr());
        if GetLastError() == 183 { return; } // ERROR_ALREADY_EXISTS
        overlay::register_class();

        // At logon another app can hold a key for a moment: retry every 2 s for 2 minutes.
        let mut timer = if register_hotkeys() { SetTimer(null_mut(), 0, 2000, null()) } else { 0 };
        let mut tries = 0;
        log(&format!("started, hotkeys {:?}", REGISTERED));
        trim();

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if msg.hwnd.is_null() && msg.message == WM_HOTKEY {
                match msg.wparam {
                    1 | 2 => overlay::open(),
                    3 if !overlay::is_open() => overlay::save_full_screen(),
                    _ => {}
                }
                continue;
            }
            if msg.hwnd.is_null() && msg.message == WM_TIMER && timer != 0 {
                tries += 1;
                if !register_hotkeys() || tries >= 60 {
                    KillTimer(null_mut(), timer);
                    timer = 0;
                    log(&format!("hotkeys {:?}", REGISTERED));
                }
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
