# Takes a picture of Markdownr's window from outside, the way FileCushion's tests do (tools/fc-acc.ps1
# in pke/FileCushion): starts the app, brings it into a state with input messages posted to its
# window (no real mouse or keyboard, no focus change), waits until its picture stops changing and
# saves the client area as PNG with PrintWindow (PW_CLIENTONLY | PW_RENDERFULLCONTENT: that window
# only, never the screen). The app has no screenshot code of its own.
#
#   tools/mdr-shot.ps1 -Exe target/release/markdownr.exe -Doc assets/sample.md -Out shot.png `
#       [-Settings perf.ini] [-Locale de-DE] [-Language ja] [-TimeMs 1234] [-Scroll 650] [-Keys Ctrl+T,F1] `
#       [-Type 'suche'] [-TopBar] [-OnScreen]
#
# -Scroll   DIPs to scroll down (mouse wheel messages; one notch is 3 x the font size, -FontSize)
# -Keys     keys posted in order: F1, Esc, Enter, Home, End, PageDown, PageUp, Up, Down, Left,
#           Right, Space, a letter or digit, each optionally with Ctrl+ / Shift+
# -Type     text typed after the keys (WM_CHAR)
# -TopBar   moves the (posted) mouse to the top edge, which shows the toolbar
# -OnScreen keeps the window where the app put it (default: moved beyond all monitors)
# -SettleMs / -StableMs  wait before the first picture / at most this long for a still one
#           (seasonal particles are composition visuals and appear in the picture, animated;
#           -TimeMs sets where their animation starts, MARKDOWNR_TIME_MS)
# Works in Windows PowerShell 5.1 and PowerShell 7.
param(
    [Parameter(Mandatory)][string]$Exe,
    [string]$Doc,
    [Parameter(Mandatory)][string]$Out,
    [string]$Settings,
    [string]$Locale,
    [string]$Language,
    [Nullable[long]]$TimeMs,
    [double]$Scroll = 0,
    [double]$FontSize = 16,
    [string[]]$Keys = @(),
    [string]$Type,
    [switch]$TopBar,
    [switch]$OnScreen,
    [int]$SettleMs = 1500,
    [int]$StableMs = 4000,
    [int]$TimeoutMs = 20000
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
if (-not ('MdrShot' -as [type])) {
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class MdrShot {
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr h, out RC r);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RC r);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h, ref PT p);
    [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] static extern int GetSystemMetrics(int i);
    [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool LockSetForegroundWindow(uint code);
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct STARTUPINFO { public int cb; public string reserved, desktop, title; public int x, y, w, h, xChars, yChars, fill, flags; public short show, reserved2; public IntPtr reserved3, stdIn, stdOut, stdErr; }
    [StructLayout(LayoutKind.Sequential)] struct PROCINFO { public IntPtr process, thread; public uint pid, tid; }
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool CreateProcessW(string app, StringBuilder cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags, IntPtr env, string dir, ref STARTUPINFO si, out PROCINFO pi);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);

    // Starts the app with this process's environment, shown without activation: the first ShowWindow
    // (SW_SHOWDEFAULT) takes STARTUPINFO's SW_SHOWNOACTIVATE, so the window never takes the foreground and
    // keyboard focus from whatever the user is typing in.
    public static uint StartInactive(string exe, string args) {
        var si = new STARTUPINFO();
        si.cb = Marshal.SizeOf(typeof(STARTUPINFO));
        si.flags = 0x1;   // STARTF_USESHOWWINDOW
        si.show = 4;      // SW_SHOWNOACTIVATE
        PROCINFO pi;
        if (!CreateProcessW(exe, new StringBuilder("\"" + exe + "\" " + args), IntPtr.Zero, IntPtr.Zero, false, 0, IntPtr.Zero, null, ref si, out pi))
            throw new System.ComponentModel.Win32Exception();
        CloseHandle(pi.thread);
        CloseHandle(pi.process);
        return pi.pid;
    }
    [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr h);
    [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr h, IntPtr dc);
    [DllImport("gdi32.dll")] static extern IntPtr CreateCompatibleDC(IntPtr dc);
    [DllImport("gdi32.dll")] static extern bool DeleteDC(IntPtr dc);
    [DllImport("gdi32.dll")] static extern IntPtr SelectObject(IntPtr dc, IntPtr o);
    [DllImport("gdi32.dll")] static extern bool DeleteObject(IntPtr o);
    [DllImport("gdi32.dll")] static extern IntPtr CreateDIBSection(IntPtr dc, ref BI bi, uint usage, out IntPtr bits, IntPtr sec, uint off);
    [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h, int a, out int v, int size);
    [DllImport("user32.dll")] static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("user32.dll")] static extern bool GetKeyboardState(byte[] s);
    [DllImport("user32.dll")] static extern bool SetKeyboardState(byte[] s);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] static extern bool PeekMessageW(out MSG m, IntPtr h, uint a, uint b, uint r);
    [StructLayout(LayoutKind.Sequential)] struct MSG { public IntPtr h; public uint m; public IntPtr w, l; public uint t; public int x, y; }
    struct RC { public int L, T, R, B; }
    struct PT { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] struct BI { public int Size, W, H; public short Planes, Bits; public int Compression, ImageSize, XRes, YRes, Used, Important; }

    // The process's visible top-level window of class `cls`, or zero.
    public static IntPtr Find(uint pid, string cls) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            var sb = new StringBuilder(64);
            GetClassNameW(h, sb, 64);
            uint p; GetWindowThreadProcessId(h, out p);
            if (p == pid && sb.ToString() == cls && IsWindowVisible(h)) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    // DWMWA_CLOAKED: the app keeps its window cloaked until the first frame is drawn.
    public static bool Cloaked(IntPtr h) { int v; return DwmGetWindowAttribute(h, 14, out v, 4) == 0 && v != 0; }

    // Moves the window to the left of all monitors (size unchanged), so tests do not cover the desktop.
    public static void Hide(IntPtr h) {
        RC r; GetWindowRect(h, out r);
        int left = GetSystemMetrics(76);   // SM_XVIRTUALSCREEN
        SetWindowPos(h, IntPtr.Zero, left - (r.R - r.L) - 200, r.T, 0, 0, 0x0001 | 0x0004 | 0x0010);   // NOSIZE | NOZORDER | NOACTIVATE
    }

    // The client area's pixels (PrintWindow, PW_CLIENTONLY | PW_RENDERFULLCONTENT), 0xAARRGGBB rows.
    public static int[] Client(IntPtr h, out int w, out int ht) {
        RC r; GetClientRect(h, out r);
        w = r.R; ht = r.B;
        var px = new int[Math.Max(0, w) * Math.Max(0, ht)];
        if (px.Length == 0) return px;
        var bi = new BI { Size = 40, W = w, H = -ht, Planes = 1, Bits = 32 };
        IntPtr screen = GetDC(IntPtr.Zero), dc = CreateCompatibleDC(screen), bits;
        IntPtr bmp = CreateDIBSection(dc, ref bi, 0, out bits, IntPtr.Zero, 0);
        IntPtr old = SelectObject(dc, bmp);
        PrintWindow(h, dc, 1 | 2);
        Marshal.Copy(bits, px, 0, px.Length);
        SelectObject(dc, old); DeleteObject(bmp); DeleteDC(dc); ReleaseDC(IntPtr.Zero, screen);
        for (int i = 0; i < px.Length; i++) px[i] |= unchecked((int)0xFF000000);
        return px;
    }

    // One colour everywhere: the window shows nothing yet (or no longer).
    public static bool Blank(int[] px) {
        for (int i = 1; i < px.Length; i++) if (px[i] != px[0]) return false;
        return true;
    }

    public static bool Same(int[] a, int[] b) {
        if (a == null || b == null || a.Length != b.Length) return false;
        for (int i = 0; i < a.Length; i++) if (a[i] != b[i]) return false;
        return true;
    }

    public static void SavePng(int[] px, int w, int ht, string path) {
        using (var bmp = new System.Drawing.Bitmap(w, ht, System.Drawing.Imaging.PixelFormat.Format32bppArgb)) {
            var data = bmp.LockBits(new System.Drawing.Rectangle(0, 0, w, ht), System.Drawing.Imaging.ImageLockMode.WriteOnly, bmp.PixelFormat);
            Marshal.Copy(px, 0, data.Scan0, px.Length);
            bmp.UnlockBits(data);
            bmp.Save(path, System.Drawing.Imaging.ImageFormat.Png);
        }
    }

    // A client point as the lParam of mouse messages.
    public static IntPtr Point(int x, int y) { return (IntPtr)(((y & 0xFFFF) << 16) | (x & 0xFFFF)); }

    // A screen point of the client area (WM_MOUSEWHEEL takes screen coordinates).
    public static IntPtr ScreenPoint(IntPtr h, int x, int y) { var p = new PT { X = x, Y = y }; ClientToScreen(h, ref p); return Point(p.X, p.Y); }

    // Posts WM_KEYDOWN/WM_KEYUP for `vk` while the app's thread sees `held` keys as down
    // (GetKeyState reads its input queue's key state, which posted messages never set): this
    // thread's input is attached to the app's for the time being; focus and foreground stay.
    public static void Key(IntPtr h, int vk, int[] held, int waitMs) {
        uint pid, app = GetWindowThreadProcessId(h, out pid), me = GetCurrentThreadId();
        MSG m; PeekMessageW(out m, IntPtr.Zero, 0, 0, 0);
        bool attached = held.Length > 0 && AttachThreadInput(me, app, true);
        var old = new byte[256];
        if (attached) {
            GetKeyboardState(old);
            var now = (byte[])old.Clone();
            foreach (int k in held) now[k] = 0x80;
            SetKeyboardState(now);
        }
        try {
            PostMessageW(h, 0x0100, (IntPtr)vk, (IntPtr)1);                       // WM_KEYDOWN
            PostMessageW(h, 0x0101, (IntPtr)vk, unchecked((IntPtr)0xC0000001));   // WM_KEYUP
            System.Threading.Thread.Sleep(waitMs);
        } finally {
            if (attached) {
                foreach (int k in held) old[k] = 0;
                SetKeyboardState(old);
                AttachThreadInput(me, app, false);
            }
        }
    }
}
'@
}

$vkNames = @{ F1 = 0x70; F3 = 0x72; F5 = 0x74; F11 = 0x7A; Esc = 0x1B; Enter = 0x0D; Home = 0x24; End = 0x23
    PageDown = 0x22; PageUp = 0x21; Up = 0x26; Down = 0x28; Left = 0x25; Right = 0x27; Space = 0x20 }

# The app inherits this process's environment (restored afterwards: the script may run in a session).
$envVars = @{ MARKDOWNR_SETTINGS = $null; MARKDOWNR_LOCALE = $Locale; MARKDOWNR_LANGUAGE = $Language; MARKDOWNR_TIME_MS = $null }
if ($Settings) { $envVars.MARKDOWNR_SETTINGS = (Resolve-Path $Settings).Path }
if ($null -ne $TimeMs) { $envVars.MARKDOWNR_TIME_MS = [string]$TimeMs }
$savedEnv = @{}
foreach ($v in @($envVars.Keys)) {
    $savedEnv[$v] = [Environment]::GetEnvironmentVariable($v)
    [Environment]::SetEnvironmentVariable($v, $(if ($envVars[$v]) { $envVars[$v] } else { $null }))
}
try {
    $args0 = if ($Doc) { '"' + (Resolve-Path $Doc).Path + '"' } else { '' }
    # Builds before SW_SHOWDEFAULT show with SW_SHOW, which activates: lock foreground changes as far
    # as Windows lets us (user input or the foreground lock time-out lifts it; the shot then warns).
    $locked = [MdrShot]::LockSetForegroundWindow(1)   # LSFW_LOCK
    $procId = [MdrShot]::StartInactive((Resolve-Path $Exe).Path, $args0)
} finally {
    foreach ($v in $savedEnv.Keys) { [Environment]::SetEnvironmentVariable($v, $savedEnv[$v]) }
}
$proc = [System.Diagnostics.Process]::GetProcessById([int]$procId)
try {
    $deadline = [DateTime]::Now.AddMilliseconds($TimeoutMs)
    $h = [IntPtr]::Zero
    while ([DateTime]::Now -lt $deadline) {
        $h = [MdrShot]::Find([uint32]$proc.Id, 'MarkdownrWindow')
        if ($h -ne [IntPtr]::Zero -and -not [MdrShot]::Cloaked($h)) { break }
        Start-Sleep -Milliseconds 30
    }
    if ($h -eq [IntPtr]::Zero) { throw "no Markdownr window within $TimeoutMs ms" }
    if (-not $OnScreen) { [MdrShot]::Hide($h) }

    # Scroll: wheel notches of 3 x font size DIPs; WHEEL_DELTA 120 per notch, at most 32760 per message.
    $delta = [math]::Round($Scroll / ($FontSize * 3) * 120)
    while ($delta -gt 0) {
        $d = [math]::Min($delta, 32760)
        [void][MdrShot]::PostMessageW($h, 0x020A, [IntPtr]((-$d -band 0xFFFF) -shl 16), [MdrShot]::ScreenPoint($h, 200, 300))
        $delta -= $d
    }
    foreach ($k in $Keys) {
        $held = @(); $name = $k
        while ($name -match '^(Ctrl|Shift)\+(.+)$') {
            if ($Matches[1] -eq 'Ctrl') { $held += 0x11, 0xA2 } else { $held += 0x10, 0xA0 }
            $name = $Matches[2]
        }
        $vk = if ($vkNames.ContainsKey($name)) { $vkNames[$name] } elseif ($name.Length -eq 1) { [int][char]$name.ToUpper() } else { throw "unknown key $k" }
        [MdrShot]::Key($h, $vk, [int[]]$held, 150)
    }
    if ($Type) { foreach ($c in $Type.ToCharArray()) { [void][MdrShot]::PostMessageW($h, 0x0102, [IntPtr][int]$c, [IntPtr]1) } }
    if ($TopBar) { [void][MdrShot]::PostMessageW($h, 0x0200, [IntPtr]::Zero, [MdrShot]::Point(300, 5)) }   # WM_MOUSEMOVE

    # Settle: at least SettleMs (images load, fades end), then until three pictures in a row agree,
    # for at most StableMs more (falling particles never stand still; they are in the picture).
    Start-Sleep -Milliseconds $SettleMs
    $w = 0; $ht = 0; $same = 0; $last = $null
    $stableBy = [DateTime]::Now.AddMilliseconds($StableMs)
    while ([DateTime]::Now -lt $deadline -and [DateTime]::Now -lt $stableBy -and $same -lt 2) {
        $px = [MdrShot]::Client($h, [ref]$w, [ref]$ht)
        if ([MdrShot]::Blank($px)) { $same = 0; $stableBy = $deadline }   # not drawn yet: wait (up to TimeoutMs)
        elseif ([MdrShot]::Same($px, $last)) { $same++ } else { $same = 0 }
        $last = $px
        Start-Sleep -Milliseconds 150
    }
    if ($null -eq $last) { $last = [MdrShot]::Client($h, [ref]$w, [ref]$ht) }   # -StableMs 0: the picture as it is
    # One colour all over is never a frame of the app (it always draws text): the display is off or
    # the window never painted. Fail instead of saving a picture that compares equal to anything.
    if ([MdrShot]::Blank($last)) { throw "blank capture (display off or window not painted); nothing saved" }
    $outPath = if ([System.IO.Path]::IsPathRooted($Out)) { $Out } else { [System.IO.Path]::GetFullPath((Join-Path (Get-Location) $Out)) }
    [MdrShot]::SavePng($last, $w, $ht, $outPath)
    "shot: $outPath ($w x $ht)$(if ($same -lt 2) { ', still changing' })$(if ([MdrShot]::GetForegroundWindow() -eq $h) { ', WARNING: the window took the foreground' })"
} finally {
    if (-not $proc.HasExited) {
        if ($h -ne [IntPtr]::Zero) { [void][MdrShot]::PostMessageW($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }   # WM_CLOSE
        if (-not $proc.WaitForExit(3000)) { $proc.Kill() }
    }
    if ($locked) { [void][MdrShot]::LockSetForegroundWindow(2) }   # LSFW_UNLOCK
}
