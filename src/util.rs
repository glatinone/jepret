//! Small helpers: UTF-16 strings, colors, settings and the log.

use crate::win32::*;
use std::ffi::c_void;
use std::ptr::null_mut;

pub fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(Some(0)).collect() }
pub fn env(k: &str) -> String { std::env::var(k).unwrap_or_default() }

/// 0xRRGGBB -> GDI COLORREF (0x00BBGGRR)
pub fn bgr(rgb: u32) -> u32 { ((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF) }

const KEY: &str = "Software\\jepret";

fn reg_dword(name: &str) -> Option<u32> {
    let (mut v, mut n) = (0u32, 4u32);
    let r = unsafe { RegGetValueW(HKEY_CURRENT_USER, wide(KEY).as_ptr(), wide(name).as_ptr(), 0x10, null_mut(), &mut v as *mut u32 as *mut c_void, &mut n) };
    (r == 0).then_some(v)
}

fn reg_string(name: &str) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut n = (buf.len() * 2) as u32;
    let r = unsafe { RegGetValueW(HKEY_CURRENT_USER, wide(KEY).as_ptr(), wide(name).as_ptr(), 0x2, null_mut(), buf.as_mut_ptr() as *mut c_void, &mut n) };
    if r != 0 { return None; }
    let s = String::from_utf16_lossy(&buf[..(n as usize / 2).saturating_sub(1)]);
    (!s.is_empty()).then_some(s)
}

/// Settings live in HKCU\Software\jepret and are read on every capture, so a change needs no restart.
///   SaveDir        (REG_SZ / REG_EXPAND_SZ)  default %USERPROFILE%\Pictures\Screenshots
///   CaptureCursor  (REG_DWORD)               default 1
pub struct Config { pub save_dir: String, pub capture_cursor: bool }

impl Config {
    pub fn load() -> Config {
        let save_dir = reg_string("SaveDir").unwrap_or_else(|| format!("{}\\Pictures\\Screenshots", env("USERPROFILE")));
        Config { save_dir, capture_cursor: reg_dword("CaptureCursor").unwrap_or(1) != 0 }
    }
}

pub fn log(m: &str) {
    use std::io::Write;
    let d = format!("{}\\jepret", env("LOCALAPPDATA"));
    let _ = std::fs::create_dir_all(&d);
    let p = format!("{}\\jepret.log", d);
    if std::fs::metadata(&p).map(|m| m.len() > 100_000).unwrap_or(false) { let _ = std::fs::rename(&p, format!("{}.old", p)); }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "{} {}", t, m);
    }
}
