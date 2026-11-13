# Packs data/emoji.txt and data/entities.txt into data/tables.bin with the
# Windows Compression API (cabinet.dll). The app unpacks it with the same
# OS DLL at runtime, so no decompressor code ends up in the exe.
# Format: 4-byte little-endian unpacked size, then the compressed stream of
#   emoji.txt + "\0" + entities.txt
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
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
}
'@
$raw = [IO.File]::ReadAllBytes("$root\data\emoji.txt") + [byte[]](0) + [IO.File]::ReadAllBytes("$root\data\entities.txt")
$packed = [CabPack]::Pack(5, $raw)   # 5 = COMPRESS_ALGORITHM_LZMS
[IO.File]::WriteAllBytes("$root\data\tables.bin", [BitConverter]::GetBytes([uint32]$raw.Length) + $packed)
"tables.bin: $($raw.Length) -> $($packed.Length + 4) bytes"
