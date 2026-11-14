# Builds the app icon after the Markdownr Windows (Store) app's logo (see below):
# renders it with WPF at the sizes Windows asks for, writes
# assets/markdownr.ico (PNG-compressed entries: a few KB instead of ~100 KB of bitmaps) and
# compiles data/app.res with rc.exe. build.rs links data/app.res, so building needs no rc.exe.
# Run with Windows PowerShell 5.1 (WPF):  powershell -ExecutionPolicy Bypass -File scripts\make-icon.ps1
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Add-Type -AssemblyName PresentationCore, WindowsBase
# Palette PNG writer: the icon has three colours plus their anti-aliasing, so an indexed PNG is
# lossless and a fraction of WPF's 32-bit one. Falls back to WPF's PNG above 256 colours.
Add-Type -TypeDefinition @'
using System; using System.IO; using System.IO.Compression; using System.Collections.Generic;
public static class IndexedPng {
  static uint[] crcTable;
  static uint Crc(byte[] b, int o, int n) {
    if (crcTable == null) { crcTable = new uint[256]; for (uint i = 0; i < 256; i++) { uint c = i; for (int k = 0; k < 8; k++) c = (c & 1) != 0 ? 0xEDB88320 ^ (c >> 1) : c >> 1; crcTable[i] = c; } }
    uint r = 0xFFFFFFFF; for (int i = o; i < o + n; i++) r = crcTable[(r ^ b[i]) & 0xFF] ^ (r >> 8); return r ^ 0xFFFFFFFF;
  }
  static void BE(Stream s, uint v) { s.WriteByte((byte)(v >> 24)); s.WriteByte((byte)(v >> 16)); s.WriteByte((byte)(v >> 8)); s.WriteByte((byte)v); }
  static void Chunk(Stream s, string type, byte[] data) {
    BE(s, (uint)data.Length); var t = new byte[4 + data.Length];
    for (int i = 0; i < 4; i++) t[i] = (byte)type[i]; Array.Copy(data, 0, t, 4, data.Length);
    s.Write(t, 0, t.Length); BE(s, Crc(t, 0, t.Length));
  }
  // pbgra: premultiplied BGRA rows. Returns null if there are more than 256 colours.
  public static byte[] Encode(byte[] pbgra, int w, int h) {
    var index = new Dictionary<uint, int>(); var colors = new List<uint>(); var raw = new byte[(w + 1) * h];
    for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) {
      int o = (y * w + x) * 4; int a = pbgra[o + 3];
      Func<int, byte> un = c => (byte)(a == 0 ? 0 : Math.Min(255, (c * 255 + a / 2) / a));
      uint rgba = a == 0 ? 0 : (uint)(un(pbgra[o + 2]) << 24 | un(pbgra[o + 1]) << 16 | un(pbgra[o]) << 8 | a);
      int i; if (!index.TryGetValue(rgba, out i)) { if (colors.Count == 256) return null; i = colors.Count; index[rgba] = i; colors.Add(rgba); }
      raw[y * (w + 1) + 1 + x] = (byte)i;
    }
    var ms = new MemoryStream(); ms.Write(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 }, 0, 8);
    var ihdr = new MemoryStream(); BE(ihdr, (uint)w); BE(ihdr, (uint)h); ihdr.Write(new byte[] { 8, 3, 0, 0, 0 }, 0, 5); Chunk(ms, "IHDR", ihdr.ToArray());
    var plte = new byte[colors.Count * 3]; var trns = new byte[colors.Count]; int last = 0;
    for (int i = 0; i < colors.Count; i++) { plte[i * 3] = (byte)(colors[i] >> 24); plte[i * 3 + 1] = (byte)(colors[i] >> 16); plte[i * 3 + 2] = (byte)(colors[i] >> 8); trns[i] = (byte)colors[i]; if (trns[i] != 255) last = i + 1; }
    Chunk(ms, "PLTE", plte);
    if (last > 0) { var t = new byte[last]; Array.Copy(trns, t, last); Chunk(ms, "tRNS", t); }
    var z = new MemoryStream(); z.WriteByte(0x78); z.WriteByte(0xDA);
    using (var d = new DeflateStream(z, CompressionLevel.Optimal, true)) d.Write(raw, 0, raw.Length);
    uint s1 = 1, s2 = 0; foreach (byte b in raw) { s1 = (s1 + b) % 65521; s2 = (s2 + s1) % 65521; } BE(z, s2 << 16 | s1);
    Chunk(ms, "IDAT", z.ToArray()); Chunk(ms, "IEND", new byte[0]);
    return ms.ToArray();
  }
}
'@
$brush = { param($hex) $b = [Windows.Media.SolidColorBrush]::new([Windows.Media.ColorConverter]::ConvertFromString($hex)); $b.Freeze(); $b }
# The Markdownr Windows (Store) app's logo: a white rounded rectangle with "M" and the down arrow
# cut out, in 48x48 units. The Store app draws it white on transparent with #9a00f5 as its
# colour; a desktop icon needs to show on light backgrounds too, so it sits on a #9a00f5 plate.
$plate = & $brush "#9a00f5"
$white = & $brush "#ffffff"
$logo = [Windows.Media.GeometryGroup]::new()
$logo.FillRule = [Windows.Media.FillRule]::EvenOdd
$logo.Children.Add([Windows.Media.RectangleGeometry]::new([Windows.Rect]::new(-0.003, 5.998, 48, 36), 3.5, 3.5))
$logo.Children.Add([Windows.Media.Geometry]::Parse("M5.99699 34.4977V13.4979h5.99991l4.5 7.4999 4.4999-7.4999h5.9999v20.9998h-5.9999V23.9978l-4.4999 5.9999-4.5-5.9999v10.4999H5.99699Z"))
$logo.Children.Add([Windows.Media.Geometry]::Parse("M35.9966 34.4977l-7.4999-10.4999h4.5046l-.0047-10.4999h5.9999v10.4999h4.5l-7.4999 10.4999Z"))
$logo.Freeze()
$sizes = 16, 24, 32, 48, 256   # Windows scales 20, 40, 64, ... down from the next larger one
$pngs = foreach ($size in $sizes) {
    $visual = [Windows.Media.DrawingVisual]::new()
    $dc = $visual.RenderOpen()
    $dc.PushTransform([Windows.Media.ScaleTransform]::new($size / 48, $size / 48))
    $corner = 48 * 0.2
    $dc.DrawRoundedRectangle($plate, $null, [Windows.Rect]::new(0, 0, 48, 48), $corner, $corner)
    $dc.PushTransform([Windows.Media.MatrixTransform]::new(0.8, 0, 0, 0.8, 4.8, 4.8))   # logo at 80 %, centred
    $dc.DrawGeometry($white, $null, $logo)
    $dc.Pop(); $dc.Pop(); $dc.Close()
    $bmp = [Windows.Media.Imaging.RenderTargetBitmap]::new($size, $size, 96, 96, [Windows.Media.PixelFormats]::Pbgra32)
    $bmp.Render($visual)
    $px = [byte[]]::new($size * $size * 4)
    $bmp.CopyPixels($px, $size * 4, 0)
    $indexed = [IndexedPng]::Encode($px, $size, $size)
    if ($indexed) { , $indexed; continue }
    $enc = [Windows.Media.Imaging.PngBitmapEncoder]::new()
    $enc.Frames.Add([Windows.Media.Imaging.BitmapFrame]::Create($bmp))
    $ms = [IO.MemoryStream]::new(); $enc.Save($ms); , $ms.ToArray()
}
# ICO: header, one directory entry per size, then the PNGs.
$ico = [IO.MemoryStream]::new(); $w = [IO.BinaryWriter]::new($ico)
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $s = $sizes[$i] % 256   # 0 means 256
    $w.Write([byte]$s); $w.Write([byte]$s); $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([uint16]1); $w.Write([uint16]32); $w.Write([uint32]$pngs[$i].Length); $w.Write([uint32]$offset)
    $offset += $pngs[$i].Length
}
foreach ($p in $pngs) { $w.Write($p) }
$w.Flush()
[IO.File]::WriteAllBytes("$root\assets\markdownr.ico", $ico.ToArray())
"markdownr.ico: $($ico.Length) bytes (" + (($sizes | ForEach-Object { $_ }) -join ", ") + ")"
$rc = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\rc.exe" | Sort-Object FullName | Select-Object -Last 1
Set-Content "$root\data\app.rc" "1 ICON `"../assets/markdownr.ico`"" -Encoding ascii
& $rc.FullName /nologo /fo "$root\data\app.res" "$root\data\app.rc"
Remove-Item "$root\data\app.rc"
"app.res: $((Get-Item "$root\data\app.res").Length) bytes"
