//! wsx -- move files across the WhiteStick bridge, and prove they arrived.
//!
//! The render box is not on the network. Everything reaches it through
//! `~/bin/whitestick '<command>'`, which runs the string in the box's WSL
//! distro under `/bin/sh`. So a file has to travel inside the command string
//! itself -- base64 in, base64 out, md5 compared at both ends by the same
//! program name on each side. A push that cannot show equal md5s fails; it
//! never reports a byte count and calls that success.
//!
//! What the bridge actually costs, measured 2026-08-23 from this devserver:
//!
//! | leg                                   | cost                          |
//! |---------------------------------------|-------------------------------|
//! | POST devserver -> navibot (fwdproxy)  | 0.3 s + 1.3 MB/s              |
//! | delivery navibot -> box               | dies above ~800 000 bytes     |
//! | round trip, 4 KB command              | 0.6 s                         |
//! | round trip, 500 KB command            | 1.8 s idle, ~15 s under load  |
//! | stdout coming back                    | 0.55 s + 1.4 s/MB, dies ~5 MB |
//! | eight round trips at once             | about the cost of one         |
//!
//! Five consequences, and they are the whole design:
//!
//! 1. **Chunks go in parallel.** A dispatch costs a round trip, not bandwidth,
//!    and the bridge sometimes holds one for a flat ~15 s. Serially that toll
//!    is paid once per chunk; together it is paid once for the file. Same 4 MB
//!    video, same minute: 142 s one chunk at a time, 5-8 s with eight in
//!    flight. Everything else here is second place.
//! 2. **The command goes in on stdin, not argv.** `whitestick` reads stdin when
//!    it gets no positional argument, which sidesteps the local exec's 128 KiB
//!    single-argument ceiling. That is what makes 512 KiB chunks possible at
//!    all: the old 129 996-character chunk was an argv limit, not a bridge one,
//!    and it made a 1 MB file cost thirteen round trips instead of three.
//! 3. **A whole small file goes in one call** -- decode, land and md5 -- since
//!    500 KB costs about what 100 KB costs.
//! 4. **A pull is one call** -- md5, size and bytes in a single answer -- for
//!    anything up to 2 MiB, and parallel `dd` slices above that. It never
//!    re-encodes the whole file once per chunk, which is what made the old pull
//!    quadratic: 43 chunks of a 4 MB file meant 43 full base64 passes over it.
//! 5. **Bytes that are already there are not sent.** Both directions learn the
//!    far side's md5 first (which push needs anyway) and stop if it matches.
//!
//! Every remote write lands atomically: the payload is decoded into a
//! `$$`-suffixed temporary and renamed over the target, so a bridge failure
//! mid-transfer leaves the old file untouched rather than a half-written one.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

const BRIDGE: &str = "bin/whitestick";

/// Base64 characters per bridge call. The delivery leg to the box carries
/// 700 000 bytes and drops 900 000, so this leaves room for the command that
/// wraps the payload while staying in the flat part of the cost curve.
/// A multiple of 4, so every chunk is a whole number of base64 groups.
const DEFAULT_CHUNK: usize = 524_288;

/// Raw bytes per pull call. The response leg runs at about 700 KB/s and dies
/// somewhere between 4 MB and 7 MB, so a slice is sized to come back as ~2.8 MB
/// of base64 -- comfortably inside that, about four seconds of answer.
const PULL_CHUNK: usize = 2 << 20;

/// Chunks in flight at once. This is the single biggest thing in the file.
/// A dispatch carrying half a megabyte does not cost half a megabyte of
/// bandwidth -- it costs one round trip, and that round trip is anywhere from
/// 0.6 s to a flat ~15 s depending on where it lands in the bridge's own
/// timing. Sending the chunks one after another pays that toll once per chunk;
/// sending them together pays it once for the whole file. Measured on the
/// 4 MB video, same file, same minute: 142 s at one chunk in flight, 29 s at
/// two, 14 s at four, 5-8 s at six and eight.
const DEFAULT_JOBS: usize = 8;

/// How long a single dispatch may take before we give up on it and send it
/// again. `whitestick`'s own ceiling is 300 s, which is far past the point
/// where a stalled call is worth waiting for.
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(90);

/// Attempts for one command before wsx gives up.
const MAX_ATTEMPTS: usize = 3;

/// Below this many base64 characters, asking the box whether it already has
/// the file costs about what sending it costs, so push does not ask.
const PROBE_WORTH: usize = 65_536;

/// Four digits of chunk index, so the box's glob sorts them back into order.
const MAX_PARTS: usize = 10_000;

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

/// One dispatch: the command goes to the bridge on stdin, so its length is
/// bounded by what the bridge will carry rather than by the local exec.
///
/// Both pipes are drained by their own thread while we wait. A pull's answer
/// is megabytes; polling the child without reading it wedges at the first
/// 64 KiB, which looks exactly like a stalled bridge and is not one.
fn dispatch(cmd: &str) -> Result<String, String> {
    let mut child = Command::new(bridge_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run the bridge: {e}"))?;
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let out_reader = std::thread::spawn(move || {
        let mut o = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut o);
        o
    });
    let err_reader = std::thread::spawn(move || {
        let mut e = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stderr, &mut e);
        e
    });
    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(cmd.as_bytes())
        .map_err(|e| format!("cannot feed the bridge: {e}"))?;
    drop(child.stdin.take());

    // Wait, but not forever: a dispatch that has gone quiet is cheaper to
    // resend than to sit out. Killing the client is enough -- the commands wsx
    // sends are all safe to run twice.
    let deadline = std::time::Instant::now() + ATTEMPT_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = out_reader.join();
                    let _ = err_reader.join();
                    return Err(format!("no answer in {} s", ATTEMPT_TIMEOUT.as_secs()));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(format!("bridge did not finish: {e}")),
        }
    };
    let out = out_reader.join().map_err(|_| "reader thread died".to_string())?;
    let err = err_reader.join().map_err(|_| "reader thread died".to_string())?;
    if !status.success() {
        return Err(format!(
            "remote command failed ({}): {}\n  stderr: {}",
            status,
            cmd.chars().take(120).collect::<String>(),
            String::from_utf8_lossy(&err).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

/// Run one command on the render box, retrying a dispatch that fails or goes
/// quiet. Safe because every command wsx sends is idempotent: writes go to a
/// `$$`-suffixed temporary and are renamed into place, so running one twice
/// produces the same bytes and a reader never sees a partial file.
fn remote_try(cmd: &str) -> Result<String, String> {
    let mut last = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        match dispatch(cmd) {
            Ok(out) => return Ok(out),
            Err(e) => {
                if attempt < MAX_ATTEMPTS {
                    eprintln!("\nwsx: retrying ({e})");
                }
                last = e;
            }
        }
    }
    Err(last)
}

fn remote(cmd: &str) -> String {
    remote_try(cmd).unwrap_or_else(|e| die(e))
}

/// Remote paths are pasted into single-quoted shell words. A quote in the path
/// would end the word and hand the rest to the shell, so refuse it outright
/// rather than build a command that means something else.
fn shell_safe(path: &str) -> &str {
    if path.contains('\'') {
        die(format!("remote path contains a single quote, which wsx will not quote: {path}"));
    }
    path
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
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
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

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, e) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *e = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c = table[((c ^ *b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// A gzip member the box's `gzip -dc` will read: the 10-byte header, raw
/// deflate, then CRC32 and length. Worth it only when it shrinks the payload --
/// a `.Map.Gbx` is LZO-compressed inside and still gives up about a fifth, an
/// `.mp4` gives up nothing and is sent raw.
fn gzip(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0xff];
    out.extend_from_slice(&miniz_oxide::deflate::compress_to_vec(data, 6));
    out.extend_from_slice(&crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

fn progress(done: usize, total: usize) {
    eprint!("\r  chunk {done}/{total}");
    let _ = std::io::stderr().flush();
}

/// Run `work(i)` for every chunk index, up to `jobs` at a time, with a progress
/// counter. Chunks are independent, so they may finish in any order.
fn each_chunk(parts: usize, jobs: usize, work: impl Fn(usize) + Sync) {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done = std::sync::atomic::AtomicUsize::new(0);
    let work = &work;
    std::thread::scope(|s| {
        for _ in 0..jobs.min(parts) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= parts {
                    return;
                }
                work(i);
                progress(done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1, parts);
            });
        }
    });
    eprintln!();
}

fn push(local: &str, remote_path: &str, chunk: usize, jobs: usize) {
    let remote_path = shell_safe(remote_path);
    let data = std::fs::read(local).unwrap_or_else(|e| die(format!("{local}: {e}")));
    let want = local_md5(local);

    let squeezed = gzip(&data);
    let compressed = squeezed.len() < data.len();
    let text = b64_encode(if compressed { &squeezed } else { &data });
    // The join reads whatever the parts hold; only wsx knows whether that is a
    // gzip member, so the unwrapping is decided here and pasted in.
    let unwrap = if compressed { " | gzip -dc" } else { "" };

    let dir = Path::new(remote_path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ".".into());
    let scratch = format!("'{remote_path}.wsxpart.'* '{remote_path}.wsxtmp.'* '{remote_path}.wsxnew.'*");

    // One call prepares the directory, clears scratch a previous run may have
    // left, and says what the box already has. Skipped when the payload is
    // small enough that asking costs what sending costs.
    if text.len() >= PROBE_WORTH {
        let have = remote(&format!(
            "mkdir -p '{dir}' && rm -f {scratch}; md5sum '{remote_path}' 2>/dev/null | cut -d' ' -f1"
        ));
        if have.split_whitespace().next() == Some(want.as_str()) {
            println!("{remote_path}  {} B  md5 {want}  OK (already there, nothing sent)", data.len());
            return;
        }
    }

    let got = if text.len() <= chunk {
        // Small file: prepare, decode, land and verify in a single round trip.
        remote(&format!(
            "mkdir -p '{dir}' && printf %s '{text}' | base64 -d{unwrap} > '{remote_path}.wsxnew.'$$ \
             && mv '{remote_path}.wsxnew.'$$ '{remote_path}' && md5sum '{remote_path}'"
        ))
    } else {
        let pieces: Vec<&[u8]> = text.as_bytes().chunks(chunk).collect();
        if pieces.len() > MAX_PARTS {
            die(format!("{} chunks is past what the box's glob orders", pieces.len()));
        }
        each_chunk(pieces.len(), jobs, |i| {
            let s = std::str::from_utf8(pieces[i]).expect("base64 is ascii");
            remote(&format!(
                "printf %s '{s}' | base64 -d > '{remote_path}.wsxtmp.{i:04}.'$$ \
                 && mv '{remote_path}.wsxtmp.{i:04}.'$$ '{remote_path}.wsxpart.{i:04}'"
            ));
        });
        // The parts glob sorts by index, so the join is just cat. Its md5 is
        // printed whatever the cleanup does, so a full disk cannot masquerade
        // as a failed transfer.
        let join = format!(
            "cat '{remote_path}.wsxpart.'*{unwrap} > '{remote_path}.wsxnew.'$$ \
             && mv '{remote_path}.wsxnew.'$$ '{remote_path}' && rm -f {scratch}; md5sum '{remote_path}'"
        );
        // A retried join can find the parts already consumed by the attempt
        // whose answer went missing; the file's md5 is what settles it either
        // way.
        remote_try(&join).unwrap_or_else(|_| remote(&format!("md5sum '{remote_path}'")))
    };

    let got = got.split_whitespace().next().unwrap_or("");
    if got != want {
        die(format!("md5 mismatch after push: local {want}, remote {got}"));
    }
    println!(
        "{remote_path}  {} B  md5 {want}  OK  [{} B on the wire{}]",
        data.len(),
        text.len(),
        if compressed { ", gzip" } else { "" }
    );
}

fn pull(remote_path: &str, local: &str, jobs: usize) {
    let remote_path = shell_safe(remote_path);
    let have_local = Path::new(local).exists();

    // When there is nothing here to compare against, there is no reason to ask
    // before fetching: one call brings back the md5, the size and the bytes.
    // The base64 is capped so that an unexpectedly huge file comes back
    // truncated (detectable) instead of overrunning the response leg.
    let cap = PULL_CHUNK / 3 * 4 + 8;
    let answer = if have_local {
        remote(&format!(
            "md5sum '{remote_path}' | cut -d' ' -f1; stat -c%s '{remote_path}'"
        ))
    } else {
        remote(&format!(
            "md5sum '{remote_path}' | cut -d' ' -f1; stat -c%s '{remote_path}'; base64 -w0 < '{remote_path}' | head -c {cap}"
        ))
    };

    let mut fields = answer.split_whitespace();
    let want = fields
        .next()
        .unwrap_or_else(|| die("no md5 from the box -- does the file exist?"))
        .to_string();
    let size: usize = fields
        .next()
        .unwrap_or_else(|| die("no size from the box"))
        .parse()
        .unwrap_or_else(|e| die(format!("cannot read the remote size: {e}")));
    let blob = fields.next().unwrap_or("");

    if have_local && local_md5(local) == want {
        println!("{local}  {size} B  md5 {want}  OK (already here, nothing fetched)");
        return;
    }

    let data = if !have_local && blob.len() < cap && size <= PULL_CHUNK {
        b64_decode(blob)
    } else if size <= PULL_CHUNK {
        b64_decode(remote(&format!("base64 -w0 < '{remote_path}'")).trim())
    } else {
        // dd reads whole blocks at an offset, so each slice is independent and
        // the file is encoded once, not once per chunk.
        let parts = size.div_ceil(PULL_CHUNK);
        let slices: Vec<std::sync::Mutex<Vec<u8>>> =
            (0..parts).map(|_| std::sync::Mutex::new(Vec::new())).collect();
        each_chunk(parts, jobs, |i| {
            let text = remote(&format!(
                "dd if='{remote_path}' bs={PULL_CHUNK} skip={i} count=1 2>/dev/null | base64 -w0"
            ));
            *slices[i].lock().expect("slice lock") = b64_decode(text.trim());
        });
        slices
            .into_iter()
            .flat_map(|m| m.into_inner().expect("slice lock"))
            .collect()
    };

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
    let mut jobs = DEFAULT_JOBS;
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
            "--jobs" => {
                jobs = it
                    .next()
                    .unwrap_or_else(|| die("--jobs needs a value"))
                    .parse()
                    .unwrap_or_else(|e| die(format!("--jobs: {e}")));
                if jobs == 0 {
                    die("--jobs must be at least 1");
                }
            }
            _ => pos.push(a.clone()),
        }
    }
    match pos.first().map(String::as_str) {
        Some("push") if pos.len() == 3 => push(&pos[1], &pos[2], chunk, jobs),
        Some("pull") if pos.len() == 3 => pull(&pos[1], &pos[2], jobs),
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
                 \x20           must be a multiple of 4; the box drops a command over\n\
                 \x20           ~800000 bytes)\n\
                 --jobs N    chunks in flight at once (default {DEFAULT_JOBS})\n\
                 \n\
                 A push whose md5 already matches on the box sends nothing; a pull whose\n\
                 md5 already matches here fetches nothing."
            );
            std::process::exit(2)
        }
    }
}
