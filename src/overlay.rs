//! The capture overlay: a topmost window over the whole virtual desktop showing the frozen screen,
//! dimmed outside the selection, with resize handles, a size label and the toolbar.
//!
//! Win32 delivers every message on the one UI thread, so the overlay state is a single `static mut`
//! that only this thread touches. `finish` takes it out before destroying the window, so messages
//! sent during destruction see `None` and fall through to DefWindowProc.

use crate::annotate::{self, Ann, Tool, COLORS, TOOLS};
use crate::capture::{grab, vscreen, Dib};
use crate::output::{save_png, to_clipboard, Gdip};
use crate::util::{bgr, log, wide, Config};
use crate::win32::*;
use std::ptr::{null, null_mut};

#[derive(Clone, Copy, PartialEq)]
enum Mode { Idle, Selecting, Moving, Resizing(u8), Drawing }

#[derive(Clone, Copy, PartialEq)]
enum Act { Copy, Save, Close }

/// Toolbar: Pen Line Arrow Rect Marker Text | Color Undo | Copy Save Close
const BUTTONS: usize = 11;
/// Segoe Fluent Icons glyphs; 0 = drawn by hand (line, color swatch).
const ICONS: [u16; BUTTONS] = [0xE70F, 0, 0xE72A, 0xE739, 0xE7E6, 0xE8D2, 0, 0xE7A7, 0xE8C8, 0xE74E, 0xE711];

pub const CLASS: &str = "jepret_overlay";

struct Overlay {
    hwnd: H, w: i32, h: i32, scale: f32, cfg: Config,
    shot: Dib, dim: Dib, back: Dib,
    has_sel: bool, sel: RECT, anchor: POINT, orig: RECT, mode: Mode,
    tool: Option<Tool>, color: usize, size: f32,
    anns: Vec<Ann>, cur: Option<Ann>, editing: bool,
    font_ui: H, font_icon: H,
    _gdip: Gdip, // last: shut GDI+ down after everything above is gone
}

static mut OVERLAY: Option<Box<Overlay>> = None;

pub fn is_open() -> bool { unsafe { OVERLAY.is_some() } }

fn norm(a: POINT, b: POINT) -> RECT { RECT { l: a.x.min(b.x), t: a.y.min(b.y), r: a.x.max(b.x), b: a.y.max(b.y) } }
fn inside(r: &RECT, p: POINT) -> bool { p.x >= r.l && p.x < r.r && p.y >= r.t && p.y < r.b }
fn lparam_pt(l: isize) -> POINT { POINT { x: (l & 0xFFFF) as i16 as i32, y: ((l >> 16) & 0xFFFF) as i16 as i32 } }

pub unsafe fn register_class() {
    let cls = wide(CLASS);
    let wc = WNDCLASSEXW { size: std::mem::size_of::<WNDCLASSEXW>() as u32, style: 0x8 /* CS_DBLCLKS */, proc_: wndproc, cls: 0, wnd: 0,
        inst: GetModuleHandleW(null()), icon: null_mut(), cursor: null_mut(), bg: null_mut(), menu: null(), class: cls.as_ptr(), icon_sm: null_mut() };
    RegisterClassExW(&wc);
}

/// Freezes the screen and shows the overlay. The screen is copied before the window exists, so menus,
/// tooltips and hover states are captured as they were when the key was pressed.
pub unsafe fn open() {
    if is_open() { return; }
    let cfg = Config::load();
    let v = vscreen();
    let shot = grab(v, cfg.capture_cursor);
    let (w, h) = (shot.w, shot.h);
    let dim = Dib::new(w, h);
    for (d, s) in dim.px().iter_mut().zip(shot.px().iter()) { *d = (s >> 1) & 0x007F_7F7F; } // 50 % darker
    let back = Dib::new(w, h);
    let gdip = Gdip::start();
    let hwnd = CreateWindowExW(0x8 | 0x80 /* TOPMOST | TOOLWINDOW */, wide(CLASS).as_ptr(), wide("jepret").as_ptr(), 0x8000_0000 /* POPUP */,
                               v.l, v.t, w, h, null_mut(), null_mut(), GetModuleHandleW(null()), null());
    let scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
    let font_ui = CreateFontW(-((12.0 * scale) as i32), 0, 0, 0, 600, 0, 0, 0, 1, 0, 0, 5, 0, wide("Segoe UI").as_ptr());
    let font_icon = CreateFontW(-((15.0 * scale) as i32), 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, wide("Segoe Fluent Icons").as_ptr());
    OVERLAY = Some(Box::new(Overlay {
        hwnd, w, h, scale, cfg, shot, dim, back,
        has_sel: false, sel: RECT::default(), anchor: POINT::default(), orig: RECT::default(), mode: Mode::Idle,
        tool: None, color: 0, size: 1.0, anns: Vec::new(), cur: None, editing: false,
        font_ui, font_icon, _gdip: gdip,
    }));
    ShowWindow(hwnd, 5);
    UpdateWindow(hwnd);
    SetForegroundWindow(hwnd); // allowed: we are handling the hotkey's input event
}

/// Renders the selection with its annotations, copies it (and saves it), then tears everything down.
unsafe fn finish(a: Act) {
    let Some(mut o) = OVERLAY.take() else { return };
    ShowWindow(o.hwnd, 0);
    if a != Act::Close && o.has_sel {
        o.commit_text();
        let r = o.sel;
        let d = Dib::new(r.r - r.l, r.b - r.t);
        BitBlt(d.dc, 0, 0, d.w, d.h, o.shot.dc, r.l, r.t, SRCCOPY);
        annotate::draw(d.dc, -r.l, -r.t, RECT { l: 0, t: 0, r: d.w, b: d.h }, o.anns.iter());
        d.opaque();
        let copied = to_clipboard(&d, o.hwnd);
        if a == Act::Save {
            log(&format!("{}x{} -> {}", d.w, d.h, save_png(&d, &o.cfg.save_dir).unwrap_or_else(|| "SAVE FAILED".into())));
        } else if !copied {
            log("clipboard busy, copy failed");
        }
    }
    ReleaseCapture();
    DestroyWindow(o.hwnd);
    DeleteObject(o.font_ui);
    DeleteObject(o.font_icon);
    drop(o);
    crate::trim();
}

impl Overlay {
    fn px(&self, v: f32) -> i32 { (v * self.scale).round() as i32 }
    fn stroke(&self, t: Tool) -> f32 {
        let base = match t { Tool::Marker => 14.0, Tool::Text => 20.0, _ => 3.0 };
        base * self.scale * self.size
    }
    fn redraw(&self) { unsafe { InvalidateRect(self.hwnd, null(), 0); } }

    // ----- geometry -----

    /// Corner and edge midpoints, clockwise from top-left.
    fn handles(&self) -> [POINT; 8] {
        let r = self.sel;
        let (cx, cy) = ((r.l + r.r) / 2, (r.t + r.b) / 2);
        [POINT { x: r.l, y: r.t }, POINT { x: cx, y: r.t }, POINT { x: r.r, y: r.t }, POINT { x: r.r, y: cy },
         POINT { x: r.r, y: r.b }, POINT { x: cx, y: r.b }, POINT { x: r.l, y: r.b }, POINT { x: r.l, y: cy }]
    }
    fn hit_handle(&self, p: POINT) -> Option<u8> {
        if !self.has_sel { return None; }
        let k = self.px(7.0);
        self.handles().iter().position(|h| (h.x - p.x).abs() <= k && (h.y - p.y).abs() <= k).map(|i| i as u8)
    }
    /// Toolbar rectangle and button size: under the selection, right-aligned; above it or inside when there is no room.
    fn bar(&self) -> (RECT, i32) {
        let b = self.px(30.0);
        let tw = b * BUTTONS as i32;
        let x = (self.sel.r - tw).clamp(0, (self.w - tw).max(0));
        let gap = self.px(6.0);
        let mut y = self.sel.b + gap;
        if y + b > self.h { y = self.sel.t - b - gap; }
        if y < 0 { y = self.sel.b - b - gap; }
        (RECT { l: x, t: y, r: x + tw, b: y + b }, b)
    }
    fn hit_bar(&self, p: POINT) -> Option<usize> {
        if !self.has_sel || self.mode == Mode::Selecting { return None; }
        let (r, b) = self.bar();
        inside(&r, p).then(|| ((p.x - r.l) / b) as usize)
    }

    // ----- editing -----

    fn commit_text(&mut self) {
        if self.editing { if let Some(a) = self.cur.take() { if a.is_meaningful() { self.anns.push(a); } } }
        self.editing = false;
    }
    /// Ctrl+A, as in Lightshot: the selection becomes the whole virtual screen.
    fn select_all(&mut self) {
        self.has_sel = true;
        self.mode = Mode::Idle;
        self.sel = RECT { l: 0, t: 0, r: self.w, b: self.h };
    }
    fn undo(&mut self) {
        if self.editing { self.cur = None; self.editing = false; } else { self.anns.pop(); }
    }
    fn button(&mut self, i: usize) -> Option<Act> {
        match i {
            0..=5 => { self.commit_text(); let t = TOOLS[i]; self.tool = if self.tool == Some(t) { None } else { Some(t) }; }
            6 => self.color = (self.color + 1) % COLORS.len(),
            7 => self.undo(),
            8 => return Some(Act::Copy),
            9 => return Some(Act::Save),
            _ => return Some(Act::Close),
        }
        self.redraw();
        None
    }

    // ----- input -----

    fn down(&mut self, p: POINT) -> Option<Act> {
        if let Some(i) = self.hit_bar(p) { return self.button(i); }
        if let Some(h) = self.hit_handle(p) {
            self.commit_text();
            self.mode = Mode::Resizing(h);
            self.orig = self.sel;
            return None;
        }
        if self.has_sel && inside(&self.sel, p) {
            match self.tool {
                Some(Tool::Text) => {
                    self.commit_text();
                    let size = self.stroke(Tool::Text);
                    let at = POINT { x: p.x, y: p.y - (size * 0.6) as i32 }; // click lands mid-line, like a caret
                    self.cur = Some(Ann { tool: Tool::Text, color: COLORS[self.color], size, pts: vec![at], text: Vec::new() });
                    self.editing = true;
                }
                Some(t) => {
                    self.commit_text();
                    let pts = if matches!(t, Tool::Pen | Tool::Marker) { vec![p] } else { vec![p, p] };
                    self.cur = Some(Ann { tool: t, color: COLORS[self.color], size: self.stroke(t), pts, text: Vec::new() });
                    self.mode = Mode::Drawing;
                }
                None => { self.mode = Mode::Moving; self.anchor = p; self.orig = self.sel; }
            }
        } else if self.anns.is_empty() && !self.editing {
            // a fresh drag outside starts a new selection (only while nothing has been drawn yet)
            self.has_sel = true;
            self.mode = Mode::Selecting;
            self.anchor = p;
            self.sel = norm(p, p);
        } else {
            self.commit_text();
        }
        self.redraw();
        None
    }
    fn mv(&mut self, p: POINT) {
        match self.mode {
            Mode::Selecting => self.sel = norm(self.anchor, p),
            Mode::Moving => {
                let (w, h) = (self.orig.r - self.orig.l, self.orig.b - self.orig.t);
                let x = (self.orig.l + p.x - self.anchor.x).clamp(0, self.w - w);
                let y = (self.orig.t + p.y - self.anchor.y).clamp(0, self.h - h);
                self.sel = RECT { l: x, t: y, r: x + w, b: y + h };
            }
            Mode::Resizing(i) => {
                let mut r = self.orig;
                let p = POINT { x: p.x.clamp(0, self.w), y: p.y.clamp(0, self.h) };
                if matches!(i, 0 | 6 | 7) { r.l = p.x; }
                if matches!(i, 2..=4) { r.r = p.x; }
                if matches!(i, 0..=2) { r.t = p.y; }
                if matches!(i, 4..=6) { r.b = p.y; }
                self.sel = norm(POINT { x: r.l, y: r.t }, POINT { x: r.r, y: r.b });
            }
            Mode::Drawing => if let Some(a) = self.cur.as_mut() {
                if matches!(a.tool, Tool::Pen | Tool::Marker) {
                    if a.pts.last() != Some(&p) { a.pts.push(p); }
                } else {
                    a.pts[1] = p;
                }
            },
            Mode::Idle => return,
        }
        self.redraw();
    }
    fn up(&mut self) {
        match self.mode {
            Mode::Selecting | Mode::Resizing(_) => {
                if self.sel.r - self.sel.l < 3 || self.sel.b - self.sel.t < 3 { self.has_sel = false; }
            }
            Mode::Drawing => if let Some(a) = self.cur.take() { if a.is_meaningful() { self.anns.push(a); } },
            _ => {}
        }
        self.mode = Mode::Idle;
        self.redraw();
    }
    fn key(&mut self, vk: usize) -> Option<Act> {
        let ctrl = unsafe { GetKeyState(0x11) } < 0;
        if ctrl {
            match vk as u8 {
                b'A' => { self.commit_text(); self.select_all(); }
                // with nothing selected, copy/save take the whole screen instead of silently closing
                b'C' => { self.commit_text(); if !self.has_sel { self.select_all(); } return Some(Act::Copy); }
                b'S' => { self.commit_text(); if !self.has_sel { self.select_all(); } return Some(Act::Save); }
                b'Z' => self.undo(),
                _ => return None,
            }
        } else if self.editing {
            match vk { // printable keys arrive as WM_CHAR
                0x1B => { self.cur = None; self.editing = false; }
                0x0D => self.commit_text(),
                _ => return None,
            }
        } else {
            match vk {
                0x1B => return Some(Act::Close),
                0x0D => return self.has_sel.then_some(Act::Copy),
                0x31..=0x36 => self.color = vk - 0x31,
                _ => {
                    let i = match vk as u8 { b'P' => 0, b'L' => 1, b'A' => 2, b'R' => 3, b'M' => 4, b'T' => 5, _ => return None };
                    return if self.has_sel { self.button(i) } else { None };
                }
            }
        }
        self.redraw();
        None
    }
    fn char(&mut self, c: u16) {
        if !self.editing { return; }
        if let Some(a) = self.cur.as_mut() {
            if c == 8 { a.text.pop(); } else if c >= 0x20 { a.text.push(c); }
        }
        self.redraw();
    }
    fn wheel(&mut self, delta: i16) {
        self.size = (self.size * if delta > 0 { 1.25 } else { 0.8 }).clamp(0.4, 5.0);
        let ts = self.stroke(Tool::Text);
        if self.editing { if let Some(a) = self.cur.as_mut() { a.size = ts; } }
        self.redraw();
    }
    fn cursor(&self, p: POINT) -> usize {
        const SIZE: [usize; 8] = [32642, 32645, 32643, 32644, 32642, 32645, 32643, 32644]; // NWSE NS NESW WE ...
        if self.hit_bar(p).is_some() { return 32649; } // hand
        if let Some(h) = self.hit_handle(p) { return SIZE[h as usize]; }
        if self.has_sel && inside(&self.sel, p) {
            return match self.tool { None => 32646 /* move */, Some(Tool::Text) => 32513 /* I-beam */, _ => 32515 /* cross */ };
        }
        32515
    }

    // ----- painting (double-buffered into `back`) -----

    unsafe fn paint(&self, out: H) {
        let dc = self.back.dc;
        BitBlt(dc, 0, 0, self.w, self.h, self.dim.dc, 0, 0, SRCCOPY);
        if self.has_sel {
            let r = self.sel;
            BitBlt(dc, r.l, r.t, r.r - r.l, r.b - r.t, self.shot.dc, r.l, r.t, SRCCOPY);
            annotate::draw(dc, 0, 0, r, self.anns.iter().chain(self.cur.iter()));
            if self.editing { self.caret(dc); }
            let (light, dark) = (CreateSolidBrush(0x00E0_E0E0), CreateSolidBrush(0x0030_3030));
            self.frame(dc, light, dark);
            self.size_label(dc, dark);
            if self.mode != Mode::Selecting { self.toolbar(dc, dark); }
            DeleteObject(light);
            DeleteObject(dark);
        }
        BitBlt(out, 0, 0, self.w, self.h, dc, 0, 0, SRCCOPY);
    }
    unsafe fn frame(&self, dc: H, light: H, dark: H) {
        let r = self.sel;
        for e in [RECT { l: r.l - 1, t: r.t - 1, r: r.r + 1, b: r.t }, RECT { l: r.l - 1, t: r.b, r: r.r + 1, b: r.b + 1 },
                  RECT { l: r.l - 1, t: r.t, r: r.l, b: r.b }, RECT { l: r.r, t: r.t, r: r.r + 1, b: r.b }] {
            FillRect(dc, &e, light);
        }
        let k = self.px(3.5);
        for p in self.handles() {
            FillRect(dc, &RECT { l: p.x - k - 1, t: p.y - k - 1, r: p.x + k + 1, b: p.y + k + 1 }, dark);
            FillRect(dc, &RECT { l: p.x - k, t: p.y - k, r: p.x + k, b: p.y + k }, light);
        }
    }
    unsafe fn size_label(&self, dc: H, dark: H) {
        let r = self.sel;
        let old = SelectObject(dc, self.font_ui);
        SetBkMode(dc, 1);
        SetTextColor(dc, 0x00FF_FFFF);
        let txt: Vec<u16> = format!("{} \u{00D7} {}", r.r - r.l, r.b - r.t).encode_utf16().collect();
        let mut sz = SIZE::default();
        GetTextExtentPoint32W(dc, txt.as_ptr(), txt.len() as i32, &mut sz);
        let (pad, lh) = (self.px(6.0), sz.cy + self.px(4.0));
        let above = r.t - lh - self.px(4.0);
        let y = if above >= 0 { above } else { r.t + self.px(4.0) };
        FillRect(dc, &RECT { l: r.l, t: y, r: r.l + sz.cx + 2 * pad, b: y + lh }, dark);
        TextOutW(dc, r.l + pad, y + self.px(2.0), txt.as_ptr(), txt.len() as i32);
        SelectObject(dc, old);
    }
    unsafe fn caret(&self, dc: H) {
        let Some(a) = self.cur.as_ref() else { return };
        let f = annotate::text_font(a.size);
        let old = SelectObject(dc, f);
        let mut sz = SIZE::default();
        GetTextExtentPoint32W(dc, a.text.as_ptr(), a.text.len() as i32, &mut sz);
        if a.text.is_empty() { sz.cy = a.size as i32; }
        SelectObject(dc, old);
        DeleteObject(f);
        let b = CreateSolidBrush(bgr(a.color));
        let x = a.pts[0].x + sz.cx + 1;
        FillRect(dc, &RECT { l: x, t: a.pts[0].y, r: x + self.px(2.0), b: a.pts[0].y + sz.cy }, b);
        DeleteObject(b);
    }
    unsafe fn toolbar(&self, dc: H, dark: H) {
        let (r, b) = self.bar();
        FillRect(dc, &r, dark);
        let (active, sep) = (CreateSolidBrush(bgr(0x0A84FF)), CreateSolidBrush(0x0055_5555));
        let old = SelectObject(dc, self.font_icon);
        SetTextColor(dc, 0x00FF_FFFF);
        for i in 0..BUTTONS {
            let mut br = RECT { l: r.l + i as i32 * b, t: r.t, r: r.l + (i as i32 + 1) * b, b: r.b };
            if i < TOOLS.len() && self.tool == Some(TOOLS[i]) { FillRect(dc, &br, active); }
            match i {
                1 => { // line: a plain diagonal
                    let pen = CreatePen(0, self.px(2.0).max(1), 0x00FF_FFFF);
                    let op = SelectObject(dc, pen);
                    let m = self.px(9.0);
                    MoveToEx(dc, br.l + m, br.b - m, null_mut());
                    LineTo(dc, br.r - m, br.t + m);
                    SelectObject(dc, op);
                    DeleteObject(pen);
                }
                6 => { // current color swatch with a white rim
                    let m = self.px(8.0);
                    let rim = CreateSolidBrush(0x00FF_FFFF);
                    FillRect(dc, &RECT { l: br.l + m - 1, t: br.t + m - 1, r: br.r - m + 1, b: br.b - m + 1 }, rim);
                    DeleteObject(rim);
                    let c = CreateSolidBrush(bgr(COLORS[self.color]));
                    FillRect(dc, &RECT { l: br.l + m, t: br.t + m, r: br.r - m, b: br.b - m }, c);
                    DeleteObject(c);
                }
                _ => { DrawTextW(dc, [ICONS[i]].as_ptr(), 1, &mut br, 0x25); } // CENTER | VCENTER | SINGLELINE
            }
            if i == 5 || i == 7 {
                FillRect(dc, &RECT { l: br.r - 1, t: br.t + self.px(6.0), r: br.r, b: br.b - self.px(6.0) }, sep);
            }
        }
        SelectObject(dc, old);
        DeleteObject(active);
        DeleteObject(sep);
    }
}

unsafe extern "system" fn wndproc(h: H, m: u32, w: usize, l: isize) -> isize {
    let Some(o) = OVERLAY.as_mut() else { return DefWindowProcW(h, m, w, l) };
    if o.hwnd != h { return DefWindowProcW(h, m, w, l); }
    let act = match m {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT { hdc: null_mut(), erase: 0, rc: RECT::default(), restore: 0, inc: 0, res: [0; 32] };
            let dc = BeginPaint(h, &mut ps);
            o.paint(dc);
            EndPaint(h, &ps);
            return 0;
        }
        WM_ERASEBKGND => return 1,
        WM_SETCURSOR => {
            let (mut p, v) = (POINT::default(), vscreen());
            GetCursorPos(&mut p);
            SetCursor(LoadCursorW(null_mut(), o.cursor(POINT { x: p.x - v.l, y: p.y - v.t })));
            return 1;
        }
        WM_LBUTTONDOWN => { SetCapture(h); o.down(lparam_pt(l)) }
        WM_MOUSEMOVE => { o.mv(lparam_pt(l)); None }
        WM_LBUTTONUP => { ReleaseCapture(); o.up(); None }
        WM_LBUTTONDBLCLK => {
            let p = lparam_pt(l);
            if o.tool.is_none() && o.has_sel && inside(&o.sel, p) { Some(Act::Copy) } else { o.down(p) }
        }
        WM_RBUTTONDOWN => if o.editing { o.commit_text(); o.redraw(); None } else { Some(Act::Close) },
        WM_KEYDOWN => o.key(w),
        WM_CHAR => { o.char(w as u16); None }
        WM_MOUSEWHEEL => { o.wheel(((w >> 16) & 0xFFFF) as i16); None }
        WM_CLOSE => Some(Act::Close),
        _ => return DefWindowProcW(h, m, w, l),
    };
    if let Some(a) = act { finish(a); }
    0
}

/// Shift+PrtSc: the whole desktop straight to a file and the clipboard, no overlay.
pub unsafe fn save_full_screen() {
    let cfg = Config::load();
    let gdip = Gdip::start();
    let d = grab(vscreen(), cfg.capture_cursor);
    d.opaque();
    let path = save_png(&d, &cfg.save_dir);
    to_clipboard(&d, null_mut());
    drop(d);
    drop(gdip);
    log(&format!("full screen -> {}", path.unwrap_or_else(|| "SAVE FAILED".into())));
    crate::trim();
}
