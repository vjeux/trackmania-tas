//! wsx -- move files across the WhiteStick bridge, and prove they arrived.
//!
//! The render box is not on the network. Everything reaches it through
//! `~/bin/whitestick '<command>'`, which runs the string in the box's WSL
//! distro under `/bin/sh` and **does not forward stdin**. So a file has to
//! travel inside the command string itself, and the string has a hard limit:
//! measured on this devserver, 130 000 bytes of argument goes through and
//! 400 000 comes back as `Argument list too long` -- from the *local* exec,
//! before the bridge is even reached.
//!
//! Hence: base64, in chunks, appended remotely, decoded at the end, and the
//! md5 compared at both ends by the same program name on each side. A push
//! that cannot show equal md5s fails; it never reports a byte count and calls
//! that success.
//!
//! Chunk size is in *base64* characters and must be a multiple of 4 so each
//! chunk is a whole number of encoded groups -- otherwise the concatenation is
//! still valid but `base64 -d` on a partial group at a chunk boundary is a
//! decoder detail nobody should be relying on.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const BRIDGE: &str = "bin/whitestick";
/// 129 996 = 32 499 base64 groups: under the measured 130 000-byte ceiling,
/// and a multiple of 4.
const DEFAULT_CHUNK: usize = 129_996;

fn die(msg: impl AsRef<str>) -> ! {
    eprintln!("wsx: {}", msg.as_ref());
    std::process::exit(1)
}

fn bridge_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| die("HOME is not set"));
    let p = format!("{home}/{BRIDGE}");
    if !Path::new(&p).exists() {
        die(format!("no bridge at {p} -- wsx runs on the devserver, not on the render box"));
    }
    p
}

/// Run one command on the render box. Returns stdout; a non-zero exit is fatal
/// and prints what the box said, because a silent bridge failure looks exactly
/// like a file that was never there.
fn remote(cmd: &str) -> String {
    let out = Command::new(bridge_path())
        .arg(cmd)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| die(format!("cannot run the bridge: {e}")));
    if !out.status.success() {
        die(format!(
            "remote command failed ({}): {}\n  stderr: {}",
            out.status,
            cmd.chars().take(120).collect::<String>(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn local_md5(path: &str) -> String {
    let out = Command::new("md5sum")
        .arg(path)
        .output()
        .unwrap_or_else(|e| die(format!("md5sum: {e}")));
    if !out.status.success() {
        die(format!("md5sum failed on {path}"));
    }
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap_or_else(|| die("md5sum printed nothing"))
        .to_string()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        s.push(B64[(n >> 18) as usize & 63] as char);
        s.push(B64[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    s
}

fn b64_decode(text: &str) -> Vec<u8> {
    let mut rev = [255u8; 256];
    for (i, c) in B64.iter().enumerate() {
        rev[*c as usize] = i as u8;
    }
    let mut acc: u32 = 0;
    let mut bits = 0;
    let mut out = Vec::new();
    for ch in text.bytes() {
        let v = rev[ch as usize];
        if v == 255 {
            continue; // whitespace, '=' padding
        }
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

fn push(local: &str, remote_path: &str, chunk: usize) {
    let data = std::fs::read(local).unwrap_or_else(|e| die(format!("{local}: {e}")));
    let want = local_md5(local);
    let text = b64_encode(&data);
    let tmp = format!("{remote_path}.wsx.b64");

    let dir = Path::new(remote_path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ".".into());
    remote(&format!("mkdir -p '{dir}' && rm -f '{tmp}'"));

    let parts = text.len().div_ceil(chunk);
    for (i, part) in text.as_bytes().chunks(chunk).enumerate() {
        let s = std::str::from_utf8(part).expect("base64 is ascii");
        remote(&format!("printf %s '{s}' >> '{tmp}'"));
        eprint!("\r  chunk {}/{}", i + 1, parts);
        let _ = std::io::stderr().flush();
    }
    eprintln!();

    let got = remote(&format!(
        "base64 -d < '{tmp}' > '{remote_path}' && rm -f '{tmp}' && md5sum '{remote_path}'"
    ));
    let got = got.split_whitespace().next().unwrap_or("").to_string();
    if got != want {
        die(format!("md5 mismatch after push: local {want}, remote {got}"));
    }
    println!("{remote_path}  {} B  md5 {want}  OK", data.len());
}

fn pull(remote_path: &str, local: &str, chunk: usize) {
    let want = remote(&format!("md5sum '{remote_path}'"))
        .split_whitespace()
        .next()
        .unwrap_or_else(|| die("no md5 from the box -- does the file exist?"))
        .to_string();
    let len: usize = remote(&format!("base64 -w0 < '{remote_path}' | wc -c"))
        .trim()
        .parse()
        .unwrap_or_else(|e| die(format!("cannot read the encoded length: {e}")));

    let mut text = String::with_capacity(len);
    let parts = len.div_ceil(chunk);
    for i in 0..parts {
        let from = i * chunk + 1;
        let to = ((i + 1) * chunk).min(len);
        text.push_str(
            remote(&format!("base64 -w0 < '{remote_path}' | cut -c{from}-{to}")).trim(),
        );
        eprint!("\r  chunk {}/{}", i + 1, parts);
        let _ = std::io::stderr().flush();
    }
    eprintln!();

    let data = b64_decode(&text);
    std::fs::write(local, &data).unwrap_or_else(|e| die(format!("{local}: {e}")));
    let got = local_md5(local);
    if got != want {
        die(format!("md5 mismatch after pull: remote {want}, local {got}"));
    }
    println!("{local}  {} B  md5 {want}  OK", data.len());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut chunk = DEFAULT_CHUNK;
    let mut pos: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--chunk" => {
                chunk = it
                    .next()
                    .unwrap_or_else(|| die("--chunk needs a value"))
                    .parse()
                    .unwrap_or_else(|e| die(format!("--chunk: {e}")));
                if chunk % 4 != 0 {
                    die("--chunk must be a multiple of 4 (whole base64 groups)");
                }
            }
            _ => pos.push(a.clone()),
        }
    }
    match pos.first().map(String::as_str) {
        Some("push") if pos.len() == 3 => push(&pos[1], &pos[2], chunk),
        Some("pull") if pos.len() == 3 => pull(&pos[1], &pos[2], chunk),
        Some("sh") if pos.len() == 2 => print!("{}", remote(&pos[1])),
        _ => {
            eprintln!(
                "wsx -- files across the WhiteStick bridge, md5-checked at both ends\n\
                 \n\
                 wsx push LOCAL REMOTE   copy a local file to the render box\n\
                 wsx pull REMOTE LOCAL   copy a file back off it\n\
                 wsx sh 'CMD'            run one command there (no stdin, /bin/sh)\n\
                 \n\
                 --chunk N   base64 characters per bridge call (default {DEFAULT_CHUNK},\n\
                 \x20           must be a multiple of 4; over ~130000 the local exec refuses)"
            );
            std::process::exit(2)
        }
    }
}
