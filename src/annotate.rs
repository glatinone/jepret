//! Annotation model and rendering: GDI+ (anti-aliased) for shapes, GDI (ClearType) for text.

use crate::util::{bgr, wide};
use crate::win32::*;
use std::ptr::null_mut;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tool { Pen, Line, Arrow, Rect, Marker, Text }

pub const TOOLS: [Tool; 6] = [Tool::Pen, Tool::Line, Tool::Arrow, Tool::Rect, Tool::Marker, Tool::Text];

/// Palette for keys 1-6 (0xRRGGBB): red, yellow, green, blue, black, white.
pub const COLORS: [u32; 6] = [0xFF3B30, 0xFFCC00, 0x34C759, 0x0A84FF, 0x111111, 0xFFFFFF];

/// One stroke/shape in overlay coordinates. Pen/Marker keep every point, Line/Arrow/Rect two, Text one (top-left).
pub struct Ann { pub tool: Tool, pub color: u32, pub size: f32, pub pts: Vec<POINT>, pub text: Vec<u16> }

impl Ann {
    /// Whether a just-finished stroke is worth keeping (a click without a drag is not a line).
    pub fn is_meaningful(&self) -> bool {
        match self.tool {
            Tool::Pen | Tool::Marker => true,
            Tool::Text => !self.text.is_empty(),
            _ => (self.pts[0].x - self.pts[1].x).abs() + (self.pts[0].y - self.pts[1].y).abs() > 2,
        }
    }
}

pub unsafe fn text_font(px: f32) -> H {
    CreateFontW(-(px as i32), 0, 0, 0, 600, 0, 0, 0, 1, 0, 0, 5, 0, wide("Segoe UI").as_ptr()) // semibold, ClearType
}

unsafe fn pen(argb: u32, w: f32, round: bool) -> H {
    let mut p = null_mut();
    GdipCreatePen1(argb, w, 2, &mut p); // UnitPixel
    if round { GdipSetPenLineCap197819(p, 2, 2, 2); GdipSetPenLineJoin(p, 2); }
    p
}

/// Draws `anns` onto `dc` shifted by (dx, dy) and clipped to `clip`. Needs a live `output::Gdip`.
pub unsafe fn draw<'a>(dc: H, dx: i32, dy: i32, clip: RECT, anns: impl Iterator<Item = &'a Ann> + Clone) {
    let mut g = null_mut();
    GdipCreateFromHDC(dc, &mut g);
    GdipSetSmoothingMode(g, 4); // AntiAlias
    GdipSetClipRectI(g, clip.l, clip.t, clip.r - clip.l, clip.b - clip.t, 0);
    let sh = |p: &POINT| POINT { x: p.x + dx, y: p.y + dy };
    for a in anns.clone() {
        let argb = 0xFF00_0000 | a.color;
        match a.tool {
            Tool::Pen | Tool::Marker => {
                // one DrawLines call, so a translucent marker does not darken where its own segments overlap
                let p = pen(if a.tool == Tool::Marker { 0x7000_0000 | a.color } else { argb }, a.size, true);
                let mut pts: Vec<POINT> = a.pts.iter().map(sh).collect();
                if pts.len() == 1 { pts.push(POINT { x: pts[0].x + 1, y: pts[0].y }); }
                GdipDrawLinesI(g, p, pts.as_ptr(), pts.len() as i32);
                GdipDeletePen(p);
            }
            Tool::Line => {
                let p = pen(argb, a.size, true);
                let (s, e) = (sh(&a.pts[0]), sh(&a.pts[1]));
                GdipDrawLineI(g, p, s.x, s.y, e.x, e.y);
                GdipDeletePen(p);
            }
            Tool::Arrow => arrow(g, argb, a.size, sh(&a.pts[0]), sh(&a.pts[1])),
            Tool::Rect => {
                let (s, e) = (sh(&a.pts[0]), sh(&a.pts[1]));
                let p = pen(argb, a.size, false);
                GdipDrawRectangleI(g, p, s.x.min(e.x), s.y.min(e.y), (s.x - e.x).abs(), (s.y - e.y).abs());
                GdipDeletePen(p);
            }
            Tool::Text => {}
        }
    }
    GdipDeleteGraphics(g);

    let saved = SaveDC(dc);
    IntersectClipRect(dc, clip.l, clip.t, clip.r, clip.b);
    SetBkMode(dc, 1); // TRANSPARENT
    for a in anns.filter(|a| a.tool == Tool::Text && !a.text.is_empty()) {
        let f = text_font(a.size);
        let old = SelectObject(dc, f);
        SetTextColor(dc, bgr(a.color));
        let p = sh(&a.pts[0]);
        TextOutW(dc, p.x, p.y, a.text.as_ptr(), a.text.len() as i32);
        SelectObject(dc, old);
        DeleteObject(f);
    }
    RestoreDC(dc, saved);
}

/// A shaft plus a filled triangular head whose size follows the stroke width.
unsafe fn arrow(g: H, argb: u32, w: f32, s: POINT, e: POINT) {
    let (vx, vy) = ((e.x - s.x) as f32, (e.y - s.y) as f32);
    let len = (vx * vx + vy * vy).sqrt();
    if len < 1.0 { return; }
    let (ux, uy) = (vx / len, vy / len);
    let head = (w * 3.5 + 6.0).min(len);
    let (bx, by) = (e.x as f32 - ux * head, e.y as f32 - uy * head);
    let p = pen(argb, w, true);
    GdipDrawLineI(g, p, s.x, s.y, (bx + ux * 1.5) as i32, (by + uy * 1.5) as i32); // overlap the head slightly, no seam
    GdipDeletePen(p);
    let hw = head * 0.55;
    let tri = [e, POINT { x: (bx - uy * hw) as i32, y: (by + ux * hw) as i32 }, POINT { x: (bx + uy * hw) as i32, y: (by - ux * hw) as i32 }];
    let mut b = null_mut();
    GdipCreateSolidFill(argb, &mut b);
    GdipFillPolygonI(g, b, tri.as_ptr(), 3, 0);
    GdipDeleteBrush(b);
}
