//! `tinyctl icon-crops --tag T --views VIEWS.tsv --out-dir DIR [--size 256] [--frac 0.60]`
//! — for every view of a finished `tinyctl shoot --tag T` run, the centre square of the
//! box capture `cmp-T<view>-o.png` (side = `frac` × the capture height) scaled to
//! `size`×`size` and dumped as raw 32-bit BGRA (`DIR/<view>.bgra`), the input of
//! `mapgeom item-icon`. One PowerShell per run on the box (System.Drawing: crop, scale,
//! LockBits → bytes), then one pull per view.
use std::path::{Path, PathBuf};

use crate::wsx::Wsx;

const POWERSHELL: &str = "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe";

fn b64_utf16le(s: &str) -> String {
    let bytes: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(|s| s.to_string());
    let tag = f("--tag").ok_or("--tag T")?;
    let views = f("--views").ok_or("--views VIEWS.tsv")?;
    let out_dir = PathBuf::from(f("--out-dir").ok_or("--out-dir DIR")?);
    let size: u32 = f("--size").and_then(|v| v.parse().ok()).unwrap_or(256);
    let frac: f32 = f("--frac").and_then(|v| v.parse().ok()).unwrap_or(0.60);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let names: Vec<String> = std::fs::read_to_string(&views)
        .map_err(|e| format!("{views}: {e}"))?
        .lines()
        .filter_map(|l| l.split('\t').next().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .collect();
    if names.is_empty() {
        return Err(format!("{views}: no views"));
    }
    let wsx = Wsx::new(args);
    let dir_win = format!("C:\\Users\\vjeux\\tinyshots\\{tag}");
    let list = names.iter().map(|n| format!("'{n}'")).collect::<Vec<_>>().join(",");
    let script = format!(
        "Add-Type -AssemblyName System.Drawing\n\
         $names = @({list})\n\
         foreach ($v in $names) {{\n\
           $src = \"{dir_win}\\cmp-{tag}$v-o.png\"\n\
           if (-not (Test-Path $src)) {{ Write-Output \"MISSING $v\"; continue }}\n\
           $i = [System.Drawing.Image]::FromFile($src)\n\
           $side = [int]($i.Height * {frac})\n\
           $x0 = [int](($i.Width - $side) / 2); $y0 = [int](($i.Height - $side) / 2)\n\
           $b = New-Object System.Drawing.Bitmap({size}, {size}, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)\n\
           $g = [System.Drawing.Graphics]::FromImage($b)\n\
           $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic\n\
           $g.DrawImage($i, (New-Object System.Drawing.Rectangle(0, 0, {size}, {size})), (New-Object System.Drawing.Rectangle($x0, $y0, $side, $side)), [System.Drawing.GraphicsUnit]::Pixel)\n\
           $g.Dispose(); $i.Dispose()\n\
           $rect = New-Object System.Drawing.Rectangle(0, 0, {size}, {size})\n\
           $d = $b.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)\n\
           $bytes = New-Object byte[] ({size} * {size} * 4)\n\
           [System.Runtime.InteropServices.Marshal]::Copy($d.Scan0, $bytes, 0, $bytes.Length)\n\
           $b.UnlockBits($d); $b.Dispose()\n\
           [System.IO.File]::WriteAllBytes(\"{dir_win}\\icon-$v.bgra\", $bytes)\n\
           Write-Output \"OK $v $side\"\n\
         }}\n"
    );
    let enc = b64_utf16le(&script);
    let out = wsx.sh(&format!("{POWERSHELL} -NoProfile -EncodedCommand {enc} 2>&1 | tr -d '\\r'"))?;
    let mut ok = 0usize;
    for l in out.lines() {
        if l.starts_with("OK ") {
            ok += 1;
        } else if !l.trim().is_empty() {
            eprintln!("  {l}");
        }
    }
    eprintln!("icon-crops: {ok}/{} crops on the box", names.len());
    let mut pulled = 0usize;
    for n in &names {
        let remote = format!("/mnt/c/Users/vjeux/tinyshots/{tag}/icon-{n}.bgra");
        let local = out_dir.join(format!("{n}.bgra"));
        match wsx.pull(&remote, &local) {
            Ok(b) if b == (size as u64) * (size as u64) * 4 => pulled += 1,
            Ok(b) => eprintln!("  {n}: {b} bytes (expected {})", (size as u64) * (size as u64) * 4),
            Err(e) => eprintln!("  {n}: {e}"),
        }
    }
    println!("{pulled}/{} crops in {}", names.len(), out_dir.display());
    let _ = Path::new(&views);
    Ok(())
}
