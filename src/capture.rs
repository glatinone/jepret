//! Screen capture into 32-bit top-down DIB sections.

use crate::win32::*;
use std::ptr::null_mut;

/// A 32 bpp top-down DIB section selected into its own memory DC. Freed on drop.
pub struct Dib { pub dc: H, bmp: H, old: H, pub bits: *mut u32, pub w: i32, pub h: i32 }

impl Dib {
    pub unsafe fn new(w: i32, h: i32) -> Dib {
        let bi = BIH { size: 40, w, h: -h, planes: 1, bits: 32, comp: 0, img: 0, xppm: 0, yppm: 0, used: 0, imp: 0 };
        let dc = CreateCompatibleDC(null_mut());
        let mut bits = null_mut();
        let bmp = CreateDIBSection(dc, &bi, 0, &mut bits, null_mut(), 0);
        let old = SelectObject(dc, bmp);
        Dib { dc, bmp, old, bits: bits as *mut u32, w, h }
    }
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn px(&self) -> &mut [u32] { std::slice::from_raw_parts_mut(self.bits, (self.w * self.h) as usize) }
    /// GDI leaves alpha at 0; set it so PNG and clipboard consumers see an opaque image.
    pub unsafe fn opaque(&self) { GdiFlush(); for p in self.px() { *p |= 0xFF00_0000; } }
}

impl Drop for Dib {
    fn drop(&mut self) { unsafe { SelectObject(self.dc, self.old); DeleteObject(self.bmp); DeleteDC(self.dc); } }
}

/// The virtual desktop (all monitors) in physical pixels; the process is per-monitor DPI aware.
pub unsafe fn vscreen() -> RECT {
    let (x, y) = (GetSystemMetrics(76), GetSystemMetrics(77));
    RECT { l: x, t: y, r: x + GetSystemMetrics(78), b: y + GetSystemMetrics(79) }
}

/// Copies rectangle `v` of the screen, optionally with the mouse cursor drawn in.
pub unsafe fn grab(v: RECT, cursor: bool) -> Dib {
    let d = Dib::new(v.r - v.l, v.b - v.t);
    let sdc = GetDC(null_mut());
    BitBlt(d.dc, 0, 0, d.w, d.h, sdc, v.l, v.t, SRCCOPY | CAPTUREBLT);
    ReleaseDC(null_mut(), sdc);
    if cursor { draw_cursor(&d, v); }
    d
}

unsafe fn draw_cursor(d: &Dib, v: RECT) {
    let mut ci = CURSORINFO { size: std::mem::size_of::<CURSORINFO>() as u32, flags: 0, cursor: null_mut(), pt: POINT::default() };
    if GetCursorInfo(&mut ci) == 0 || ci.flags & 1 == 0 { return; } // CURSOR_SHOWING
    let mut ii = ICONINFO { icon: 0, xhot: 0, yhot: 0, mask: null_mut(), color: null_mut() };
    if GetIconInfo(ci.cursor, &mut ii) == 0 { return; }
    DrawIconEx(d.dc, ci.pt.x - v.l - ii.xhot as i32, ci.pt.y - v.t - ii.yhot as i32, ci.cursor, 0, 0, 0, null_mut(), 3);
    if !ii.mask.is_null() { DeleteObject(ii.mask); }
    if !ii.color.is_null() { DeleteObject(ii.color); }
}
