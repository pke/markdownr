# Packs data/entities.txt, data/emoji.txt, data/math.txt and data/help/*.txt into data/tables.bin with the
# Windows Compression API (cabinet.dll). The app unpacks it with the same
# OS DLL at runtime, so no decompressor code ends up in the exe.
# The first three tables are sorted by key and front coded: per line one char (0x20 + bytes shared with the
# previous key), the rest of the key, a tab, the value, a newline. src/tables.rs expands it back
# to `\n key \t value \n` lines. (Measured: 1.6 KB smaller than plain lines; MSZIP, XPRESS and
# keys/values in separate blocks all came out larger than LZMS on this.)
# Format: 4-byte little-endian unpacked size, then the compressed stream of
#   entities + "\0" + emoji + "\0" + math + "\0" + help
# help: per language "#<code>\n" and its F1 texts in display order, as is (src/tables.rs help()).
# The languages are FileCushion's.
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Add-Type -TypeDefinition @'
using System; using System.Linq; using System.Text; using System.Runtime.InteropServices;
public static class CabPack {
  [DllImport("cabinet.dll", SetLastError=true)] static extern bool CreateCompressor(uint alg, IntPtr r, out IntPtr h);
  [DllImport("cabinet.dll", SetLastError=true)] static extern bool Compress(IntPtr h, byte[] src, UIntPtr n, byte[] dst, UIntPtr cap, out UIntPtr size);
  [DllImport("cabinet.dll")] static extern bool CloseCompressor(IntPtr h);
  public static byte[] Pack(uint alg, byte[] data) {
    IntPtr h; if (!CreateCompressor(alg, IntPtr.Zero, out h)) throw new Exception("CreateCompressor " + Marshal.GetLastWin32Error());
    var dst = new byte[data.Length + 65536]; UIntPtr size;
    if (!Compress(h, data, (UIntPtr)data.Length, dst, (UIntPtr)dst.Length, out size)) throw new Exception("Compress " + Marshal.GetLastWin32Error());
    CloseCompressor(h); var r = new byte[(int)size]; Array.Copy(dst, r, r.Length); return r;
  }
  public static string FrontCode(string table) {
    var rows = table.Split('\n').Where(l => l.Length > 0).Select(l => l.Split(new[] { '\t' }, 2)).OrderBy(r => r[0], StringComparer.Ordinal).ToArray();
    var sb = new StringBuilder(); string prev = "";
    foreach (var r in rows) {
      if (r[0].Any(c => c < 0x21 && c != ' ' || c > 0x7e) || r[0] == prev) throw new Exception("key not unique printable ASCII: " + r[0]);
      int p = 0; while (p < prev.Length && p < r[0].Length && p < 0x5f && prev[p] == r[0][p]) p++;
      sb.Append((char)(0x20 + p)).Append(r[0], p, r[0].Length - p).Append('\t').Append(r[1]).Append('\n');
      prev = r[0];
    }
    return sb.ToString();
  }
}
'@
$read = { param($name) [IO.File]::ReadAllText("$root\data\$name.txt") }
$text = [CabPack]::FrontCode((& $read "entities")) + "`0" + [CabPack]::FrontCode((& $read "emoji")) + "`0" + [CabPack]::FrontCode((& $read "math")) + "`0" + (-join (@('de', 'en', 'es', 'fr', 'ja', 'ko', 'pl', 'ru', 'zh') | ForEach-Object { "#$_`n" + (& $read "help\$_") }))
$raw = [Text.UTF8Encoding]::new($false).GetBytes($text)
$packed = [CabPack]::Pack(5, $raw)   # 5 = COMPRESS_ALGORITHM_LZMS
[IO.File]::WriteAllBytes("$root\data\tables.bin", [BitConverter]::GetBytes([uint32]$raw.Length) + $packed)
"tables.bin: $($raw.Length) -> $($packed.Length + 4) bytes"
