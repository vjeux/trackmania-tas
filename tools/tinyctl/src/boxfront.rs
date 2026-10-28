//! The game window on top of the screen for a capture, and the screen put back.
//!
//! The box's captures are screen copies (shotdpi.ps1 / CopyFromScreen): whatever
//! window sits above the game is what lands in the frame. A person's parked game
//! (Hades II, 2026-10-27 19:24, idle 40 min, TOPMOST) covered the editor and every
//! frame of three shoots was Hades. `front` gives the game the foreground AND the
//! topmost band (a focused window still draws under a topmost one), `restore` takes
//! the band away again and hands the foreground back to the window that had it.
//! Both run a PowerShell built here, passed `-EncodedCommand` (UTF-16LE base64: no
//! quoting through the WSL bridge), and both tap Alt (`keybd_event`) — the one
//! synthetic key Windows wants before a background process may change the
//! foreground. Callers run them inside `inputidle::gate`'s logged run, so that tap
//! is not read as a person's input by `box-idle`.

use crate::wsx::Wsx;

const POWERSHELL: &str = "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe";
/// Where `front` notes the window that had the foreground, for `restore`.
const PREV: &str = "C:\\Users\\vjeux\\boxfront-prev.txt";

const SIG: &str = r#"$sig = '[DllImport("user32.dll")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, System.UIntPtr dwExtraInfo); [DllImport("user32.dll")] public static extern bool SetForegroundWindow(System.IntPtr hWnd); [DllImport("user32.dll")] public static extern bool SetWindowPos(System.IntPtr hWnd, System.IntPtr after, int x, int y, int cx, int cy, uint flags); [DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow(); [DllImport("user32.dll")] public static extern int GetWindowText(System.IntPtr hWnd, System.Text.StringBuilder t, int n); [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(System.IntPtr hWnd, out uint pid); [DllImport("user32.dll")] public static extern bool IsIconic(System.IntPtr hWnd); [DllImport("user32.dll")] public static extern bool ShowWindow(System.IntPtr hWnd, int cmd);'
$k = Add-Type -MemberDefinition $sig -Name Keys -Namespace BoxFront -PassThru
"#;

fn b64_utf16le(s: &str) -> String {
    let bytes: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn run(wsx: &Wsx, script: &str, marker: &str) -> Result<String, String> {
    let enc = b64_utf16le(script);
    let out = wsx.sh(&format!("{POWERSHELL} -NoProfile -EncodedCommand {enc} 2>&1 | tr -d '\\r'"))?;
    out.lines().find(|l| l.contains(marker)).map(|l| l.trim().to_string()).ok_or_else(|| format!("boxfront: no '{marker}' line in: {}", out.trim().chars().take(300).collect::<String>()))
}

/// The game window in front: restored if it was minimized (a person had minimized
/// the game on 2026-10-27 — `IsIconic`; `SetForegroundWindow` alone leaves an iconic
/// window where it is, which is why three shoots captured the window behind it),
/// foreground + TOPMOST. Notes the previous foreground window's process id and the
/// minimized state in `PREV` for `restore`, which puts both back.
pub fn front(wsx: &Wsx) -> Result<String, String> {
    let script = format!(
        r#"{SIG}
$p = Get-Process Trackmania -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $p) {{ 'front: no Trackmania process'; exit 1 }}
$h = $p.MainWindowHandle
$prev = $k::GetForegroundWindow()
$pid0 = [uint32]0; [void]$k::GetWindowThreadProcessId($prev, [ref]$pid0)
$wasIconic = $k::IsIconic($h)
if ($prev -ne $h) {{ Set-Content -Path '{PREV}' -Value (([string]$pid0) + ' ' + ([string]$wasIconic)) }}
if ($wasIconic) {{ [void]$k::ShowWindow($h, 9); Start-Sleep -Milliseconds 700 }}
$k::keybd_event(0x12, 0x38, 0, [System.UIntPtr]::Zero); [void]$k::SetForegroundWindow($h); $k::keybd_event(0x12, 0x38, 2, [System.UIntPtr]::Zero)
[void]$k::SetWindowPos($h, [System.IntPtr](-1), 0, 0, 0, 0, 0x0003)
Start-Sleep -Milliseconds 500
$sb = New-Object System.Text.StringBuilder 256; [void]$k::GetWindowText($k::GetForegroundWindow(), $sb, 256)
'front: fg=[' + $sb.ToString() + '] prev-pid=' + $pid0 + ' was-minimized=' + $wasIconic
"#
    );
    run(wsx, &script, "front:")
}

/// The topmost band taken off the game window; the window noted by `front` gets the
/// foreground back (when its process still lives).
pub fn restore(wsx: &Wsx) -> Result<String, String> {
    let script = format!(
        r#"{SIG}
$t = Get-Process Trackmania -ErrorAction SilentlyContinue | Select-Object -First 1
if ($t) {{ [void]$k::SetWindowPos($t.MainWindowHandle, [System.IntPtr](-2), 0, 0, 0, 0, 0x0003) }}
$who = 'none'
if (Test-Path '{PREV}') {{
  $line = (Get-Content '{PREV}' | Select-Object -First 1) -split ' '
  $pid0 = [int]$line[0]
  $wasIconic = ($line.Length -gt 1 -and $line[1] -eq 'True')
  Remove-Item '{PREV}' -ErrorAction SilentlyContinue
  if ($wasIconic -and $t) {{ [void]$k::ShowWindow($t.MainWindowHandle, 6) }}
  $p = Get-Process -Id $pid0 -ErrorAction SilentlyContinue
  if ($p -and $p.MainWindowHandle -ne 0) {{
    $h = $p.MainWindowHandle
    $k::keybd_event(0x12, 0x38, 0, [System.UIntPtr]::Zero); [void]$k::SetForegroundWindow($h); $k::keybd_event(0x12, 0x38, 2, [System.UIntPtr]::Zero)
    Start-Sleep -Milliseconds 400
    $who = $p.ProcessName
  }}
}}
$sb = New-Object System.Text.StringBuilder 256; [void]$k::GetWindowText($k::GetForegroundWindow(), $sb, 256)
'restore: fg=[' + $sb.ToString() + '] gave-back=' + $who
"#
    );
    run(wsx, &script, "restore:")
}
