//! Where a finished capture goes: the clipboard (CF_DIB) and PNG files (GDI+ encoder).

use crate::capture::Dib;
use crate::util::log;
use crate::win32::*;
use std::ptr::{null, null_mut};

/// GDI+ is started per capture and shut down afterwards, so it costs nothing while idle.
pub struct Gdip(usize);
impl Gdip {
    pub fn start() -> Gdip {
        let (mut tok, inp) = (0usize, GpInput { ver: 1, cb: null(), nothread: 0, noext: 0 });
        unsafe { GdiplusStartup(&mut tok, &inp, null_mut()); }
        Gdip(tok)
    }
}
impl Drop for Gdip {
    fn drop(&mut self) { unsafe { GdiplusShutdown(self.0); } }
}

pub unsafe fn to_clipboard(d: &Dib, owner: H) -> bool {
    let row = (d.w * 4) as usize;
    let g = GlobalAlloc(2, 40 + row * d.h as usize); // GMEM_MOVEABLE
    if g.is_null() { return false; }
    let p = GlobalLock(g) as *mut u8;
    let bi = BIH { size: 40, w: d.w, h: d.h, planes: 1, bits: 32, comp: 0, img: (row * d.h as usize) as u32, xppm: 0, yppm: 0, used: 0, imp: 0 };
    std::ptr::copy_nonoverlapping(&bi as *const BIH as *const u8, p, 40);
    let src = d.bits as *const u8;
    for y in 0..d.h as usize { // CF_DIB rows are bottom-up
        std::ptr::copy_nonoverlapping(src.add(y * row), p.add(40 + (d.h as usize - 1 - y) * row), row);
    }
    GlobalUnlock(g);
    for _ in 0..10 { // another app may hold the clipboard for a moment
        if OpenClipboard(owner) != 0 {
            EmptyClipboard();
            let ok = !SetClipboardData(8, g).is_null(); // CF_DIB; the system owns `g` from here
            CloseClipboard();
            if !ok { GlobalFree(g); }
            return ok;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    GlobalFree(g);
    false
}

/// Next free `Screenshot_<N>.png` in `dir`, continuing any existing numbering (Lightshot uses the same names).
pub fn next_path(dir: &str) -> String {
    let _ = std::fs::create_dir_all(dir);
    let n = std::fs::read_dir(dir).map(|it| it.filter_map(|e| {
        let n = e.ok()?.file_name().into_string().ok()?;
        n.strip_prefix("Screenshot_")?.strip_suffix(".png")?.parse::<u32>().ok()
    }).max().unwrap_or(0)).unwrap_or(0);
    format!("{}\\Screenshot_{}.png", dir, n + 1)
}

/// Encodes `d` as PNG into the next free file in `dir`. Needs a live `Gdip`.
pub unsafe fn save_png(d: &Dib, dir: &str) -> Option<String> { save_png_to(d, next_path(dir)) }

/// Ctrl+Shift+S, Lightshot's Ctrl+S: a Save As dialog that opens in `dir` with the next name filled in.
/// None when the dialog is cancelled or the save fails.
pub unsafe fn save_png_as(d: &Dib, dir: &str, owner: H) -> Option<String> {
    let mut file = [0u16; 1024];
    let start: Vec<u16> = next_path(dir).encode_utf16().take(file.len() - 1).collect();
    file[..start.len()].copy_from_slice(&start);
    let filter: Vec<u16> = "PNG image\0*.png\0\0".encode_utf16().collect();
    let (ext, dirw) = (crate::util::wide("png"), crate::util::wide(dir));
    let mut o = OFN {
        size: std::mem::size_of::<OFN>() as u32, owner, inst: null_mut(), filter: filter.as_ptr(), custom: null_mut(), max_custom: 0,
        filter_index: 1, file: file.as_mut_ptr(), max_file: file.len() as u32, title_out: null_mut(), max_title_out: 0,
        initial_dir: dirw.as_ptr(), title: null(), flags: 0x2 | 0x8 | 0x800 | 0x8_0000, // OVERWRITEPROMPT NOCHANGEDIR PATHMUSTEXIST EXPLORER
        file_offset: 0, ext_offset: 0, def_ext: ext.as_ptr(), data: 0, hook: null_mut(), template: null(),
        reserved: null_mut(), reserved2: 0, flags_ex: 0,
    };
    if GetSaveFileNameW(&mut o) == 0 { return None; }
    let n = file.iter().position(|&c| c == 0).unwrap_or(file.len());
    save_png_to(d, String::from_utf16_lossy(&file[..n]))
}

unsafe fn save_png_to(d: &Dib, path: String) -> Option<String> {
    let png = GUID { a: 0x557C_F406, b: 0x1A04, c: 0x11D3, d: [0x9A, 0x73, 0x00, 0x00, 0xF8, 0x1E, 0xF3, 0x2E] };
    let mut img = null_mut();
    if GdipCreateBitmapFromScan0(d.w, d.h, d.w * 4, 0x0002_2009, d.bits as *mut u8, &mut img) != 0 { return None; } // PixelFormat32bppRGB
    let st = GdipSaveImageToFile(img, crate::util::wide(&path).as_ptr(), &png, null());
    GdipDisposeImage(img);
    if st == 0 { Some(path) } else { log(&format!("save failed (GDI+ status {}) {}", st, path)); None }
}
