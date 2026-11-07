# jepret design spec

## 1. Problem

On my laptop (15 W i7-1365U, 32 GB RAM) the screenshot tool, Lightshot, felt slow and sometimes did not respond to
PrtSc. I found three causes:

1. **Scheduling.** It was started by a scheduled task with priority 7, which gives the process below-normal CPU
   priority, low I/O priority and memory priority 2. Its pages were trimmed first, so every capture began by paging
   the app back in while the disk queue was busy.
2. **Size.** Lightshot is a 32-bit app that loads the Intel Direct3D stack (`igc32.dll`, `igd9dxva32.dll`,
   `media_bin_32.dll`, ...): about 70 MB private and 36 MB resident, for a tool that is idle 99.9 % of the time.
3. **Fragility.** Lightshot had died silently, so a .NET watchdog (24 MB) had been added to restart it every 5 s and
   to forward PrtSc to Lightshot's real hotkey (F8).

Raising the priority fixed the latency but not the cost. The goal of jepret is to do the same daily job for a fixed,
small cost.

## 2. Requirements

| # | Requirement | Target | Result |
|---|---|---|---|
| R1 | Idle footprint of everything the tool needs | < 10 MB | 1.3 MB working set, 4.6 MB private |
| R2 | Idle CPU | none | 0 ms/min, no timers after start |
| R3 | PrtSc to visible overlay | < 100 ms | 32-61 ms |
| R4 | Same daily workflow as Lightshot: region, annotate, copy, save | full | pen, line, arrow, rect, marker, text, colors, undo, copy, save |
| R5 | Saves continue the existing `Screenshot_<N>.png` numbering in the same folder | yes | yes |
| R6 | Correct on high-DPI and multi-monitor setups | yes | per-monitor DPI v2, virtual desktop |
| R7 | No runtime or third-party dependencies | yes | Rust std + system DLLs |
| R8 | Fully reversible migration from Lightshot | yes | archive + restore script |

Out of scope: cloud upload, "search similar images", printing, a tray icon, and video capture.

## 3. Architecture

```
logon task ──> jepret.exe (one UI thread)
                 │  RegisterHotKey: PrtSc, F8, Shift+PrtSc
                 │  GetMessage ... (blocked, 0 CPU)
                 ▼
         WM_HOTKEY ──> overlay::open
                         Config::load (registry)
                         grab()  BitBlt(screen DC, CAPTUREBLT) -> shot DIB
                         dim     shot >> 1                     -> dim DIB
                         back    double buffer                 -> back DIB
                         Gdip::start, fonts, topmost popup window
                 ▼
         input ──> Overlay state machine ──> InvalidateRect ──> paint into back ──> BitBlt to window
                 ▼
         Ctrl+C / Ctrl+S ──> finish
                         render selection + annotations into a new DIB
                         CF_DIB to clipboard, PNG via GDI+ (Ctrl+S)
                         destroy window, DIBs, fonts, GDI+
                         SetProcessWorkingSetSize(-1, -1)  -> back to ~1 MB
```

### 3.1 Idle

`main` registers three thread hotkeys (`hwnd = NULL`, `MOD_NOREPEAT`) and sits in `GetMessage`. If a key is held by
another app at logon, a 2-second thread timer retries for up to 2 minutes and is then killed. After that the process
receives no messages until a hotkey fires. A named mutex (`Local\jepret`) keeps it to one instance.

### 3.2 Capture

The process is `PER_MONITOR_AWARE_V2`, so `GetSystemMetrics(SM_*VIRTUALSCREEN)` and `BitBlt` work in physical
pixels across every monitor. The screen is copied **before** the overlay window exists, so the capture shows the
desktop exactly as it was at the key press. The cursor is drawn in with `DrawIconEx` at its hotspot when
`CaptureCursor` is set.

### 3.3 Overlay

- **Window.** A `WS_POPUP` window with `WS_EX_TOPMOST | WS_EX_TOOLWINDOW` covers the virtual desktop, with
  `CS_DBLCLKS`. `WM_ERASEBKGND` is swallowed, and every paint is one full-frame `BitBlt` from the back buffer, so
  there is no flicker.
- **Paint order.** Dimmed shot, then the bright selection, then annotations clipped to the selection, then the
  caret, frame and handles, the size label, and finally the toolbar.
- **State machine.** `Idle`, `Selecting`, `Moving`, `Resizing(handle)` and `Drawing`.
- **Hit-test priority.** Toolbar first, then handles, then inside the selection (draw with the active tool, or move
  when no tool is active), then outside. A drag outside starts a new selection, but only while nothing has been
  drawn yet.
- **Toolbar.** 11 buttons of 30 dip, right-aligned under the selection. The bar flips above the selection, or inside
  it, when there is no room below. Icons are Segoe Fluent Icons glyphs; the line icon and the color swatch are drawn
  by hand.
- **Scaling.** Every UI size is in dip, multiplied by `GetDpiForWindow / 96`.

### 3.4 Annotations

An `Ann` is `{tool, color, size, pts, text}`, stored in overlay coordinates so it can be drawn either into the
overlay or into the export offset by `-sel.origin`.

- Shapes use GDI+ with `SmoothingModeAntiAlias`, round caps and round joins.
- The marker is one `DrawLines` call with alpha `0x70`. Because it is a single call, overlapping segments of the same
  stroke do not stack and darken.
- The arrow is a shaft plus a filled triangle sized `3.5 * width + 6`. The shaft overlaps the head by 1.5 px so no
  seam shows.
- Text uses GDI `TextOutW` with a ClearType Segoe UI Semibold font, because GDI+ text looks worse on screen.
- Undo pops the last annotation, or cancels the text being typed.

### 3.5 Output

- **Clipboard.** `CF_DIB`, 32 bpp `BI_RGB`, rows written bottom-up, alpha forced to 255. Windows synthesizes
  `CF_BITMAP` and `CF_DIBV5` from it. Opening the clipboard is retried up to 10 times, 15 ms apart.
- **PNG.** `GdipCreateBitmapFromScan0` (32bppRGB, wrapping the DIB bits with no copy) and then
  `GdipSaveImageToFile` with the PNG encoder CLSID.
- **File name.** The highest `Screenshot_<N>.png` in `SaveDir`, plus one.

## 4. Memory budget

| Phase | Working set | Notes |
|---|---|---|
| Idle | 1.3 MB | code pages + one thread stack; nothing allocated |
| Overlay open (1920x1200) | ~46 MB | 3 DIBs at 9.2 MB each + GDI+ + fonts |
| After close | ~0.5-1.3 MB | DIBs freed, GDI+ shut down, working set trimmed |

Trimming after a capture is safe here, unlike with Lightshot. jepret's code is only a few hundred KB, so a cold
start of the overlay soft-faults a handful of pages, and the measured latency already includes that.

## 5. Failure modes

| Failure | Behavior |
|---|---|
| Hotkey taken at logon | retry every 2 s for 2 min; the log lists which keys were registered |
| Clipboard locked by another app | 10 retries; the failure is logged; Ctrl+S still saves the file |
| Save folder missing | created on demand |
| Crash | the logon task restarts it (3 times, 1 min apart) |
| Second instance started | exits immediately (mutex) |

## 6. Testing

`scripts/demo.ps1` is an end-to-end test that uses real input through `SendInput`, `SetCursorPos` and
`mouse_event`:

1. Show a fixed stage image full screen. The script refuses to record unless the stage is the foreground window
   and covers the screen, so the real desktop never ends up in a recording.
2. Press PrtSc, drag a selection, click the toolbar buttons, and draw an arrow, a rectangle, a marker stroke, a pen
   stroke and text.
3. Press Ctrl+S, then assert that the PNG exists, and show it.
4. ffmpeg (`ddagrab`) records the run, which produced `docs/demo.mp4`, `demo.gif` and `overlay.png`.

Latency was measured from an external process: inject PrtSc, then poll `FindWindow("jepret_overlay")` together with
`IsWindowVisible` every 1 ms. Over 5 runs the results were 45, 32, 45, 46 and 61 ms.

## 7. Migration from Lightshot (what was done on the author's laptop)

Before anything was removed, every piece of Lightshot was archived to
`%LOCALAPPDATA%\startup-backup\lightshot-archived-<date>\`:

- the program folder and the .NET guard;
- the scheduled task XML;
- `HKCU\Software\Skillbrains`;
- the Task Manager startup approval entry;
- the Desktop shortcuts.

The folder also contains `restore.ps1`, which puts all of it back and turns jepret off. jepret's `SaveDir` points at
the folder Lightshot used, so saved screenshots continue the same numbering: the first jepret save after
Lightshot's `Screenshot_696.png` is `Screenshot_697.png`.
