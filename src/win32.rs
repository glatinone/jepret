//! The slice of the Win32 API jepret uses, declared by hand (raw-dylib, no import libraries, no crates).

#![allow(non_snake_case, clippy::upper_case_acronyms, clippy::too_many_arguments)]

use std::ffi::c_void;

pub type H = *mut c_void;

#[repr(C)] #[derive(Clone, Copy, Default, PartialEq, Debug)] pub struct POINT { pub x: i32, pub y: i32 }
#[repr(C)] #[derive(Clone, Copy, Default, Debug)] pub struct RECT { pub l: i32, pub t: i32, pub r: i32, pub b: i32 }
#[repr(C)] #[derive(Default)] pub struct SIZE { pub cx: i32, pub cy: i32 }
#[repr(C)] pub struct MSG { pub hwnd: H, pub message: u32, pub wparam: usize, pub lparam: isize, pub time: u32, pub pt: POINT, pub private: u32 }
#[repr(C)] pub struct WNDCLASSEXW {
    pub size: u32, pub style: u32, pub proc_: unsafe extern "system" fn(H, u32, usize, isize) -> isize,
    pub cls: i32, pub wnd: i32, pub inst: H, pub icon: H, pub cursor: H, pub bg: H, pub menu: *const u16, pub class: *const u16, pub icon_sm: H,
}
#[repr(C)] pub struct PAINTSTRUCT { pub hdc: H, pub erase: i32, pub rc: RECT, pub restore: i32, pub inc: i32, pub res: [u8; 32] }
/// BITMAPINFOHEADER
#[repr(C)] pub struct BIH {
    pub size: u32, pub w: i32, pub h: i32, pub planes: u16, pub bits: u16, pub comp: u32,
    pub img: u32, pub xppm: i32, pub yppm: i32, pub used: u32, pub imp: u32,
}
#[repr(C)] pub struct CURSORINFO { pub size: u32, pub flags: u32, pub cursor: H, pub pt: POINT }
#[repr(C)] pub struct ICONINFO { pub icon: i32, pub xhot: u32, pub yhot: u32, pub mask: H, pub color: H }
#[repr(C)] pub struct GpInput { pub ver: u32, pub cb: *const c_void, pub nothread: i32, pub noext: i32 }
#[repr(C)] pub struct GUID { pub a: u32, pub b: u16, pub c: u16, pub d: [u8; 8] }

#[link(name = "user32", kind = "raw-dylib")]
extern "system" {
    pub fn RegisterHotKey(h: H, id: i32, m: u32, vk: u32) -> i32;
    pub fn GetMessageW(m: *mut MSG, h: H, a: u32, b: u32) -> i32;
    pub fn TranslateMessage(m: *const MSG) -> i32;
    pub fn DispatchMessageW(m: *const MSG) -> isize;
    pub fn RegisterClassExW(c: *const WNDCLASSEXW) -> u16;
    pub fn CreateWindowExW(ex: u32, cls: *const u16, t: *const u16, st: u32, x: i32, y: i32, w: i32, h: i32, p: H, m: H, i: H, l: *const c_void) -> H;
    pub fn DefWindowProcW(h: H, m: u32, w: usize, l: isize) -> isize;
    pub fn ShowWindow(h: H, c: i32) -> i32;
    pub fn SetForegroundWindow(h: H) -> i32;
    pub fn DestroyWindow(h: H) -> i32;
    pub fn InvalidateRect(h: H, r: *const RECT, e: i32) -> i32;
    pub fn UpdateWindow(h: H) -> i32;
    pub fn BeginPaint(h: H, p: *mut PAINTSTRUCT) -> H;
    pub fn EndPaint(h: H, p: *const PAINTSTRUCT) -> i32;
    pub fn GetDC(h: H) -> H;
    pub fn ReleaseDC(h: H, dc: H) -> i32;
    pub fn GetSystemMetrics(i: i32) -> i32;
    pub fn LoadCursorW(i: H, n: usize) -> H;
    pub fn SetCursor(c: H) -> H;
    pub fn GetCursorPos(p: *mut POINT) -> i32;
    pub fn SetCapture(h: H) -> H;
    pub fn ReleaseCapture() -> i32;
    pub fn GetKeyState(k: i32) -> i16;
    pub fn OpenClipboard(h: H) -> i32;
    pub fn EmptyClipboard() -> i32;
    pub fn SetClipboardData(f: u32, h: H) -> H;
    pub fn CloseClipboard() -> i32;
    pub fn GetCursorInfo(c: *mut CURSORINFO) -> i32;
    pub fn GetIconInfo(c: H, i: *mut ICONINFO) -> i32;
    pub fn DrawIconEx(dc: H, x: i32, y: i32, i: H, w: i32, h: i32, s: u32, b: H, f: u32) -> i32;
    pub fn SetProcessDpiAwarenessContext(c: isize) -> i32;
    pub fn GetDpiForWindow(h: H) -> u32;
    pub fn FillRect(dc: H, r: *const RECT, b: H) -> i32;
    pub fn DrawTextW(dc: H, t: *const u16, n: i32, r: *mut RECT, f: u32) -> i32;
    pub fn SetTimer(h: H, id: usize, ms: u32, f: *const c_void) -> usize;
    pub fn KillTimer(h: H, id: usize) -> i32;
}
#[link(name = "gdi32", kind = "raw-dylib")]
extern "system" {
    pub fn CreateCompatibleDC(dc: H) -> H;
    pub fn CreateDIBSection(dc: H, bi: *const BIH, u: u32, bits: *mut *mut c_void, s: H, o: u32) -> H;
    pub fn SelectObject(dc: H, o: H) -> H;
    pub fn DeleteObject(o: H) -> i32;
    pub fn DeleteDC(dc: H) -> i32;
    pub fn BitBlt(d: H, x: i32, y: i32, w: i32, h: i32, s: H, sx: i32, sy: i32, rop: u32) -> i32;
    pub fn CreateSolidBrush(c: u32) -> H;
    pub fn CreatePen(s: i32, w: i32, c: u32) -> H;
    pub fn MoveToEx(dc: H, x: i32, y: i32, p: *mut POINT) -> i32;
    pub fn LineTo(dc: H, x: i32, y: i32) -> i32;
    pub fn CreateFontW(h: i32, w: i32, e: i32, o: i32, wt: i32, i: u32, u: u32, s: u32, cs: u32, op: u32, cp: u32, q: u32, pf: u32, face: *const u16) -> H;
    pub fn SetTextColor(dc: H, c: u32) -> u32;
    pub fn SetBkMode(dc: H, m: i32) -> i32;
    pub fn TextOutW(dc: H, x: i32, y: i32, t: *const u16, n: i32) -> i32;
    pub fn GetTextExtentPoint32W(dc: H, t: *const u16, n: i32, s: *mut SIZE) -> i32;
    pub fn SaveDC(dc: H) -> i32;
    pub fn RestoreDC(dc: H, n: i32) -> i32;
    pub fn IntersectClipRect(dc: H, l: i32, t: i32, r: i32, b: i32) -> i32;
    pub fn GdiFlush() -> i32;
}
#[link(name = "kernel32", kind = "raw-dylib")]
extern "system" {
    pub fn CreateMutexW(a: *const c_void, own: i32, n: *const u16) -> H;
    pub fn GetLastError() -> u32;
    pub fn GetModuleHandleW(n: *const u16) -> H;
    pub fn GlobalAlloc(f: u32, n: usize) -> H;
    pub fn GlobalLock(h: H) -> *mut c_void;
    pub fn GlobalUnlock(h: H) -> i32;
    pub fn GlobalFree(h: H) -> H;
    pub fn GetCurrentProcess() -> H;
    pub fn SetProcessWorkingSetSize(h: H, a: usize, b: usize) -> i32;
}
#[link(name = "advapi32", kind = "raw-dylib")]
extern "system" {
    pub fn RegGetValueW(k: isize, sub: *const u16, v: *const u16, f: u32, t: *mut u32, d: *mut c_void, n: *mut u32) -> i32;
}
/// OPENFILENAMEW
#[repr(C)] pub struct OFN {
    pub size: u32, pub owner: H, pub inst: H, pub filter: *const u16, pub custom: *mut u16, pub max_custom: u32, pub filter_index: u32,
    pub file: *mut u16, pub max_file: u32, pub title_out: *mut u16, pub max_title_out: u32, pub initial_dir: *const u16, pub title: *const u16,
    pub flags: u32, pub file_offset: u16, pub ext_offset: u16, pub def_ext: *const u16, pub data: isize, pub hook: H, pub template: *const u16,
    pub reserved: H, pub reserved2: u32, pub flags_ex: u32,
}
#[link(name = "comdlg32", kind = "raw-dylib")]
extern "system" {
    pub fn GetSaveFileNameW(o: *mut OFN) -> i32;
}
#[link(name = "gdiplus", kind = "raw-dylib")]
extern "system" {
    pub fn GdiplusStartup(t: *mut usize, i: *const GpInput, o: *mut c_void) -> i32;
    pub fn GdiplusShutdown(t: usize);
    pub fn GdipCreateFromHDC(dc: H, g: *mut H) -> i32;
    pub fn GdipDeleteGraphics(g: H) -> i32;
    pub fn GdipSetSmoothingMode(g: H, m: i32) -> i32;
    pub fn GdipSetClipRectI(g: H, x: i32, y: i32, w: i32, h: i32, c: i32) -> i32;
    pub fn GdipCreatePen1(c: u32, w: f32, u: i32, p: *mut H) -> i32;
    pub fn GdipSetPenLineCap197819(p: H, s: i32, e: i32, d: i32) -> i32;
    pub fn GdipSetPenLineJoin(p: H, j: i32) -> i32;
    pub fn GdipDeletePen(p: H) -> i32;
    pub fn GdipDrawLinesI(g: H, p: H, pts: *const POINT, n: i32) -> i32;
    pub fn GdipDrawLineI(g: H, p: H, x1: i32, y1: i32, x2: i32, y2: i32) -> i32;
    pub fn GdipDrawRectangleI(g: H, p: H, x: i32, y: i32, w: i32, h: i32) -> i32;
    pub fn GdipCreateSolidFill(c: u32, b: *mut H) -> i32;
    pub fn GdipFillPolygonI(g: H, b: H, pts: *const POINT, n: i32, m: i32) -> i32;
    pub fn GdipDeleteBrush(b: H) -> i32;
    pub fn GdipCreateBitmapFromScan0(w: i32, h: i32, s: i32, f: i32, b: *mut u8, o: *mut H) -> i32;
    pub fn GdipSaveImageToFile(i: H, f: *const u16, c: *const GUID, p: *const c_void) -> i32;
    pub fn GdipDisposeImage(i: H) -> i32;
}

pub const WM_PAINT: u32 = 0x0F;
pub const WM_CLOSE: u32 = 0x10;
pub const WM_ERASEBKGND: u32 = 0x14;
pub const WM_SETCURSOR: u32 = 0x20;
pub const WM_KEYDOWN: u32 = 0x100;
pub const WM_CHAR: u32 = 0x102;
pub const WM_TIMER: u32 = 0x113;
pub const WM_HOTKEY: u32 = 0x312;
pub const WM_MOUSEMOVE: u32 = 0x200;
pub const WM_LBUTTONDOWN: u32 = 0x201;
pub const WM_LBUTTONUP: u32 = 0x202;
pub const WM_LBUTTONDBLCLK: u32 = 0x203;
pub const WM_RBUTTONDOWN: u32 = 0x204;
pub const WM_MOUSEWHEEL: u32 = 0x20A;
pub const SRCCOPY: u32 = 0x00CC_0020;
pub const CAPTUREBLT: u32 = 0x4000_0000;
pub const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
