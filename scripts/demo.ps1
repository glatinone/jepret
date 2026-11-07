<#
  Records the README demo: a clean stage picture fills the screen, then jepret is driven with real input
  (PrtSc, drag, toolbar clicks, typing, Ctrl+S) while ffmpeg (ddagrab) records. Doubles as an end-to-end test:
  it fails if the saved PNG is missing.

  pwsh -STA -File scripts\demo.ps1 -Stage bg.png -Out docs
  Needs: jepret running, ffmpeg with ddagrab on PATH, a 1920x1200 screen (coordinates below are for that size).
#>
param([Parameter(Mandatory)][string]$Stage, [string]$Out = "docs")

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices; using System.Threading;
public static class In {
  [StructLayout(LayoutKind.Sequential)] struct KI { public ushort vk, scan; public uint flags, time; public IntPtr extra; }
  [StructLayout(LayoutKind.Explicit, Size = 40)] struct INPUT { [FieldOffset(0)] public uint type; [FieldOffset(8)] public KI ki; }
  [DllImport("user32.dll")] static extern uint SendInput(uint n, INPUT[] i, int size);
  [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr c);
  [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int t, uint f);
  [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  struct RECT { public int l, t, r, b; }
  // an Alt tap counts as user input, which lets this process take the foreground
  public static void Stage(IntPtr h) { SetWindowPos(h, (IntPtr)(-1), 0, 0, 1920, 1200, 0x40); Key(0x12, 0, 0); Key(0x12, 0, 2); SetForegroundWindow(h); }
  public static bool Covers(IntPtr h, int w, int t) { RECT r; GetWindowRect(h, out r); return GetForegroundWindow() == h && r.l <= 0 && r.t <= 0 && r.r >= w && r.b >= t; }
  struct POINT { public int x, y; }
  static void Key(ushort vk, ushort scan, uint flags) {
    var i = new INPUT[1]; i[0].type = 1; i[0].ki.vk = vk; i[0].ki.scan = scan; i[0].ki.flags = flags;
    SendInput(1, i, Marshal.SizeOf(typeof(INPUT)));
  }
  public static void Tap(ushort vk) { Key(vk, 0, 0); Key(vk, 0, 2); Thread.Sleep(60); }
  public static void Chord(ushort mod, ushort vk) { Key(mod, 0, 0); Tap(vk); Key(mod, 0, 2); Thread.Sleep(60); }
  public static void Type(string s, int ms) { foreach (char c in s) { Key(0, c, 4); Key(0, c, 6); Thread.Sleep(ms); } }
  // eased glide, like a hand on a mouse
  public static void Glide(int x, int y, int ms) {
    POINT p; GetCursorPos(out p); int steps = Math.Max(8, ms / 12);
    for (int i = 1; i <= steps; i++) { double t = (double)i / steps; t = t < .5 ? 2 * t * t : 1 - Math.Pow(-2 * t + 2, 2) / 2;
      SetCursorPos(p.x + (int)((x - p.x) * t), p.y + (int)((y - p.y) * t)); Thread.Sleep(12); }
  }
  public static void Drag(int x1, int y1, int x2, int y2, int ms) { Glide(x1, y1, 350); Thread.Sleep(120); mouse_event(2, 0, 0, 0, UIntPtr.Zero); Glide(x2, y2, ms); Thread.Sleep(80); mouse_event(4, 0, 0, 0, UIntPtr.Zero); Thread.Sleep(250); }
  public static void Click(int x, int y) { Glide(x, y, 450); Thread.Sleep(150); mouse_event(2, 0, 0, 0, UIntPtr.Zero); mouse_event(4, 0, 0, 0, UIntPtr.Zero); Thread.Sleep(300); }
}
'@
[In]::SetProcessDpiAwarenessContext([IntPtr]-4) | Out-Null
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path

# save into a scratch folder during the demo, then put the user's SaveDir back
$key = 'HKCU:\Software\jepret'
$old = Get-ItemProperty $key -ErrorAction SilentlyContinue
$tmp = Join-Path $env:TEMP "jepret-demo"; Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue; New-Item -ItemType Directory $tmp | Out-Null
New-Item $key -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty $key SaveDir $tmp; Set-ItemProperty $key CaptureCursor 0 -Type DWord   # no frozen cursor in the shot

$form = New-Object Windows.Forms.Form -Property @{ FormBorderStyle = 'None'; TopMost = $true; StartPosition = 'Manual'; ShowInTaskbar = $false; BackColor = [Drawing.Color]::FromArgb(24, 24, 27) }
$pic = New-Object Windows.Forms.PictureBox -Property @{ Dock = 'Fill'; SizeMode = 'CenterImage'; Image = [Drawing.Image]::FromFile((Resolve-Path $Stage)) }
$form.Controls.Add($pic)
$form.Show()
[In]::Stage($form.Handle)
for ($i = 0; $i -lt 20; $i++) { [Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 50 }
# never record the real desktop: the stage must be the foreground window and cover the whole screen
if (-not [In]::Covers($form.Handle, 1920, 1200)) { $form.Close(); Set-ItemProperty $key SaveDir $old.SaveDir; throw "stage window is not covering the screen, not recording" }
[In]::Glide(1700, 900, 200); Start-Sleep -Milliseconds 600

$mp4 = Join-Path $Out 'demo.mp4'
$ff = New-Object Diagnostics.Process
$ff.StartInfo = New-Object Diagnostics.ProcessStartInfo -Property @{ FileName = 'ffmpeg'; UseShellExecute = $false; RedirectStandardInput = $true; RedirectStandardError = $true; CreateNoWindow = $true
  Arguments = "-hide_banner -y -f lavfi -i ddagrab=output_idx=0:framerate=30:draw_mouse=1,hwdownload,format=bgra,crop=1920:1080:0:120 -c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p `"$mp4`"" }
$ff.Start() | Out-Null; $ff.BeginErrorReadLine()
Start-Sleep -Milliseconds 1500

try {
  # coordinates for 1920x1200 at 150 %: toolbar buttons are 45 px, the bar sits right-aligned under the selection
  $sel = @{ l = 380; t = 215; r = 1520; b = 708 }
  $bx = { param($i) $sel.r - 11 * 45 + 45 * $i + 22 }; $by = $sel.b + 9 + 22

  [In]::Glide(380, 215, 500); Start-Sleep -Milliseconds 400
  [In]::Tap(0x2C); Start-Sleep -Milliseconds 700                       # PrtSc: screen freezes, dims
  [In]::Drag($sel.l, $sel.t, $sel.r, $sel.b, 900); Start-Sleep -Milliseconds 700

  [In]::Click((& $bx 2), $by)                                          # Arrow
  [In]::Drag(1000, 690, 900, 372, 600)
  [In]::Click((& $bx 3), $by)                                          # Rectangle
  [In]::Drag(1108, 336, 1506, 600, 700)
  [In]::Click((& $bx 6), $by)                                          # color -> yellow
  [In]::Click((& $bx 4), $by)                                          # Marker
  [In]::Drag(716, 348, 890, 348, 500)
  [In]::Tap(0x31)                                                      # key 1 = red again
  [In]::Click((& $bx 0), $by)                                          # Pen: underline
  [In]::Drag(400, 528, 770, 524, 600)
  [In]::Click((& $bx 5), $by)                                          # Text
  [In]::Click(420, 690); [In]::Type('45 ms from PrtSc to this overlay', 45); [In]::Tap(0x0D)
  Start-Sleep -Milliseconds 900
  [In]::Glide((& $bx 9), $by, 500); Start-Sleep -Milliseconds 300
  [In]::Chord(0x11, 0x53)                                              # Ctrl+S
  Start-Sleep -Milliseconds 900

  $png = Get-ChildItem $tmp -Filter 'Screenshot_*.png' | Select-Object -First 1
  if (-not $png) { throw "jepret did not save a PNG" }
  $pic.Image = [Drawing.Image]::FromFile($png.FullName); [Windows.Forms.Application]::DoEvents()   # show the result
  [In]::Glide(1850, 1150, 400)
  Start-Sleep -Milliseconds 2500
}
finally {
  $ff.StandardInput.Write('q'); $ff.WaitForExit(15000) | Out-Null
  $form.Close()
  if ($old.SaveDir) { Set-ItemProperty $key SaveDir $old.SaveDir }; if ($null -ne $old.CaptureCursor) { Set-ItemProperty $key CaptureCursor $old.CaptureCursor -Type DWord }
}
Copy-Item $png.FullName (Join-Path $Out 'annotated.png') -Force
"saved $($png.Name) ($($png.Length) bytes); video $mp4"
