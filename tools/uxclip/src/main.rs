//! uxclip -- harvest UI-motion reference clips off public screen recordings.
//!
//! The job this was written for: inventory how iOS chat apps animate a sent
//! message, from footage on the open web. Nothing here knows about chat apps;
//! it knows that a reference clip is found (`search`), fetched with its
//! captions (`fetch`), located by what the narrator says (`hits`) and by what
//! moves on screen (`motion`), WATCHED before it is believed (`sheet`,
//! `stack`), and only then published (`gif`, `cut`).
//!
//! std only. ffmpeg/ffprobe: `UXCLIP_FFMPEG` / `UXCLIP_FFPROBE`, else
//! `~/uxclips/bin/{ffmpeg,ffprobe}` (a private static Linux build on the
//! render box, deliberately NOT on PATH so `clip`'s platform detection keeps
//! seeing a box with no native ffmpeg), else PATH. yt-dlp: `~/bin/yt-dlp`,
//! else PATH.

mod draw;
mod vtt;

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const USAGE: &str = "\
uxclip -- UI-motion reference clips from public screen recordings

  uxclip search [-n N] 'query' ...             YouTube flat search per query (id|dur|views|date|title)
  uxclip fetch --out DIR [--height 720] ID ...  video-only mp4 + English captions (.vtt) + ID.meta.tsv
  uxclip hits [--words w1,w2] [--gap S] FILE.vtt   when the narrator says a word; grouped windows
  uxclip transcript FILE.vtt [--from S --to S] [--every 5]   the timed words as lines
  uxclip probe FILE                             width/height/fps/duration
  uxclip motion FILE --from T --to T [--fps 10] [--crop W:H:X:Y]   per-frame change, top vs bottom of frame
  uxclip activebox FILE [--samples 40] [--threshold 28] [--min-frac 0.35]   crop= of the region that changes (the mirrored phone)
  uxclip cropdetect FILE --at T                 ffmpeg's crop= suggestion (black borders only)
  uxclip sheet FILE --from T --to T --out X.jpg [--fps 10] [--cols 8] [--width 180] [--crop W:H:X:Y] [--label TXT] [--max 400]
  uxclip stack --out X.jpg [--width 1440] A.jpg B.jpg ...         sheets stacked vertically
  uxclip gif FILE --from T --to T --out X.gif [--fps 15] [--width 360] [--speed 1.0] [--hold 0.8] [--crop ..] [--label TXT] [--stamp]
  uxclip cut FILE --from T --to T --out X.mp4 [--fps native] [--width 540] [--speed 1.0] [--hold 0] [--crop ..] [--label TXT] [--stamp]
  uxclip batch JOBS.txt                         run one uxclip command per line (quotes honoured, # comments)
  uxclip attachvars --out vars.json FILE ...    GraphQL variables for the chat-attachment bulk upload (base64)

Times are seconds (12.5) or clock (1:02.5, 00:01:02.500). -v prints every command run.
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = args.iter().any(|a| a == "-v");
    let args: Vec<String> = args.into_iter().filter(|a| a != "-v").collect();
    let Some(cmd) = args.first() else {
        eprint!("{USAGE}");
        std::process::exit(2);
    };
    let rest = &args[1..];
    let r = match cmd.as_str() {
        "search" => search(rest, verbose),
        "fetch" => fetch(rest, verbose),
        "hits" => hits(rest),
        "transcript" => transcript(rest),
        "probe" => probe_cmd(rest, verbose),
        "motion" => motion(rest, verbose),
        "activebox" => activebox(rest, verbose),
        "cropdetect" => cropdetect(rest, verbose),
        "sheet" => sheet(rest, verbose),
        "stack" => stack(rest, verbose),
        "gif" => gif(rest, verbose),
        "cut" => cut(rest, verbose),
        "batch" => batch(rest, verbose),
        "attachvars" => attachvars(rest),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown command `{other}`\n{USAGE}")),
    };
    if let Err(e) = r {
        eprintln!("uxclip: {e}");
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------- attachvars

/// `attachvars --out vars.json FILE...`: the GraphQL variables file for the
/// chat-attachment bulk upload (`xfb_metamate_nest_bulk_file_upload`), every
/// file base64-encoded with its MIME type from the extension. The one way a
/// clip reaches the person inside the chat is as an uploaded attachment, and
/// the base64 belongs in a file, not on a command line.
fn attachvars(args: &[String]) -> Result<(), String> {
    let o = parse(args, &["out"])?;
    let out = out_path(&o)?;
    if o.pos.is_empty() {
        return Err("attachvars needs at least one file".into());
    }
    let mut json = String::from("{\"inputs\":[");
    for (i, f) in o.pos.iter().enumerate() {
        let bytes = fs::read(f).map_err(|e| format!("{f}: {e}"))?;
        let name = Path::new(f).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| f.clone());
        let mime = match name.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
            "gif" => "image/gif",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "mp4" => "video/mp4",
            "html" => "text/html",
            "md" | "txt" => "text/plain",
            _ => "application/octet-stream",
        };
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"file_content\":\"{}\",\"file_content_type\":\"{mime}\",\"file_name\":\"{}\"}}",
            base64(&bytes),
            name.replace('"', "")
        ));
        println!("{name}\t{mime}\t{} B", bytes.len());
    }
    json.push_str("]}");
    fs::write(&out, json).map_err(|e| format!("{}: {e}", out.display()))?;
    confirm_output(&out)
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn words_split_like_a_shell() {
        assert_eq!(split_words("a 'b c' \"d\" e"), vec!["a", "b c", "d", "e"]);
        assert_eq!(split_words("  x  "), vec!["x"]);
        assert_eq!(split_words("--label 'Slack iOS | yG 1-2'"), vec!["--label", "Slack iOS | yG 1-2"]);
    }
}

// ---------------------------------------------------------------- args

/// `batch JOBS.txt`: one uxclip command per line (without the program name),
/// quotes honoured, `#` lines skipped; every job runs, failures are listed at
/// the end and make the exit status non-zero. The loop lives here so no
/// shell loop has to.
fn batch(args: &[String], verbose: bool) -> Result<(), String> {
    let file = args.first().ok_or("batch needs a jobs file")?;
    let src = fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let mut failed = Vec::new();
    for (n, line) in src.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words = split_words(line);
        let Some(cmd) = words.first() else { continue };
        println!("## {line}");
        if let Err(e) = dispatch(cmd, &words[1..], verbose) {
            println!("FAIL line {}: {e}", n + 1);
            failed.push(n + 1);
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("{} job(s) failed (lines {:?})", failed.len(), failed))
    }
}

/// Shell-like word splitting: whitespace separates, single or double quotes group.
fn split_words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut had = false;
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '\'') | (None, '"') => {
                quote = Some(c);
                had = true;
            }
            (None, c) if c.is_whitespace() => {
                if had || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    had = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if had || !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn dispatch(cmd: &str, rest: &[String], verbose: bool) -> Result<(), String> {
    match cmd {
        "search" => search(rest, verbose),
        "fetch" => fetch(rest, verbose),
        "hits" => hits(rest),
        "transcript" => transcript(rest),
        "probe" => probe_cmd(rest, verbose),
        "motion" => motion(rest, verbose),
        "activebox" => activebox(rest, verbose),
        "cropdetect" => cropdetect(rest, verbose),
        "sheet" => sheet(rest, verbose),
        "stack" => stack(rest, verbose),
        "gif" => gif(rest, verbose),
        "cut" => cut(rest, verbose),
        other => Err(format!("unknown command `{other}`")),
    }
}

struct Opts {
    flags: HashMap<String, String>,
    pos: Vec<String>,
}

fn parse(args: &[String], takes_value: &[&str]) -> Result<Opts, String> {
    let mut flags = HashMap::new();
    let mut pos = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if let Some(name) = a.strip_prefix("--") {
            if let Some((k, v)) = name.split_once('=') {
                flags.insert(k.to_string(), v.to_string());
            } else if takes_value.contains(&name) {
                i += 1;
                let v = args.get(i).ok_or_else(|| format!("--{name} needs a value"))?;
                flags.insert(name.to_string(), v.clone());
            } else {
                flags.insert(name.to_string(), "true".to_string());
            }
        } else if a == "-n" {
            i += 1;
            let v = args.get(i).ok_or("-n needs a value")?;
            flags.insert("n".to_string(), v.clone());
        } else {
            pos.push(a.clone());
        }
        i += 1;
    }
    Ok(Opts { flags, pos })
}

impl Opts {
    fn get(&self, k: &str) -> Option<&str> {
        self.flags.get(k).map(|s| s.as_str())
    }
    fn num(&self, k: &str, default: f64) -> Result<f64, String> {
        match self.get(k) {
            None => Ok(default),
            Some(v) => v.parse().map_err(|_| format!("--{k}: not a number: {v}")),
        }
    }
    fn time(&self, k: &str) -> Result<f64, String> {
        let v = self.get(k).ok_or_else(|| format!("--{k} is required"))?;
        vtt::parse_ts(v).ok_or_else(|| format!("--{k}: not a time: {v}"))
    }
    fn need(&self, k: &str) -> Result<&str, String> {
        self.get(k).ok_or_else(|| format!("--{k} is required"))
    }
}

// ---------------------------------------------------------------- tools

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/".into()))
}

fn tool(env: &str, private: &str, name: &str) -> PathBuf {
    if let Ok(p) = std::env::var(env) {
        return PathBuf::from(p);
    }
    let p = home().join(private);
    if p.is_file() {
        return p;
    }
    PathBuf::from(name)
}

fn ffmpeg() -> PathBuf {
    tool("UXCLIP_FFMPEG", "uxclips/bin/ffmpeg", "ffmpeg")
}
fn ffprobe() -> PathBuf {
    tool("UXCLIP_FFPROBE", "uxclips/bin/ffprobe", "ffprobe")
}
fn ytdlp() -> PathBuf {
    tool("UXCLIP_YTDLP", "bin/yt-dlp", "yt-dlp")
}

fn show(cmd: &Command) -> String {
    let mut s = cmd.get_program().to_string_lossy().into_owned();
    for a in cmd.get_args() {
        let a = a.to_string_lossy();
        if a.contains(' ') || a.contains('[') || a.contains(';') {
            s.push_str(&format!(" '{a}'"));
        } else {
            s.push(' ');
            s.push_str(&a);
        }
    }
    s
}

/// Run to completion; stdout captured, stderr passed through on failure.
fn run(cmd: &mut Command, verbose: bool) -> Result<String, String> {
    if verbose {
        eprintln!("+ {}", show(cmd));
    }
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run {}: {e}", cmd.get_program().to_string_lossy()))?;
    if !out.status.success() {
        return Err(format!(
            "{} failed ({}):\n{}",
            cmd.get_program().to_string_lossy(),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run; stdout AND stderr inherited (for the long yt-dlp downloads).
fn run_live(cmd: &mut Command, verbose: bool) -> Result<(), String> {
    if verbose {
        eprintln!("+ {}", show(cmd));
    }
    let st = cmd
        .stdin(Stdio::null())
        .status()
        .map_err(|e| format!("cannot run {}: {e}", cmd.get_program().to_string_lossy()))?;
    if !st.success() {
        return Err(format!("{} failed ({st})", cmd.get_program().to_string_lossy()));
    }
    Ok(())
}

// ---------------------------------------------------------------- search / fetch

fn search(args: &[String], verbose: bool) -> Result<(), String> {
    let o = parse(args, &["n"])?;
    let n = o.num("n", 10.0)? as usize;
    if o.pos.is_empty() {
        return Err("search needs at least one query".into());
    }
    for q in &o.pos {
        println!("## {q}");
        let mut c = Command::new(ytdlp());
        c.args(["--flat-playlist", "--no-warnings", "--ignore-errors", "--print"])
            .arg("%(id)s\t%(duration)s\t%(view_count)s\t%(upload_date)s\t%(title)s")
            .arg(format!("ytsearch{n}:{q}"));
        match run(&mut c, verbose) {
            Ok(s) => print!("{s}"),
            Err(e) => println!("(search failed: {})", e.lines().last().unwrap_or("")),
        }
    }
    Ok(())
}

fn fetch(args: &[String], verbose: bool) -> Result<(), String> {
    let o = parse(args, &["out", "height"])?;
    let out = PathBuf::from(o.need("out")?);
    let h = o.num("height", 720.0)? as u32;
    fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;
    if o.pos.is_empty() {
        return Err("fetch needs at least one video id or URL".into());
    }
    let fmt = format!(
        "bv*[ext=mp4][height<={h}][vcodec^=avc1]/bv*[ext=mp4][height<={h}]/bv*[height<={h}]/b[height<={h}]/b"
    );
    let mut failed = Vec::new();
    for id in &o.pos {
        let mut c = Command::new(ytdlp());
        c.args(["-f", &fmt, "--no-playlist", "--no-warnings", "--no-progress", "--ignore-errors"])
            .args(["--write-auto-subs", "--write-subs", "--sub-langs", "en,en-orig", "--sub-format", "vtt", "--sleep-subtitles", "2"])
            .arg("-o")
            .arg(out.join("%(id)s.%(ext)s"))
            .arg("--no-simulate")
            .arg("--print-to-file")
            .arg("%(id)s\t%(duration)s\t%(fps)s\t%(width)sx%(height)s\t%(upload_date)s\t%(title)s\t%(webpage_url)s")
            .arg(out.join("%(id)s.meta.tsv"))
            .arg("--")
            .arg(id);
        if let Err(e) = run_live(&mut c, verbose) {
            eprintln!("uxclip: {id}: {e}");
            failed.push(id.clone());
        }
    }
    // What landed, so the caller never has to guess extensions.
    for id in &o.pos {
        let id_short = id.rsplit(['/', '=']).next().unwrap_or(id);
        let mut have = Vec::new();
        if let Ok(rd) = fs::read_dir(&out) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.starts_with(id_short) {
                    have.push(n);
                }
            }
        }
        have.sort();
        println!("{id_short}\t{}", have.join(" "));
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("{} download(s) failed: {}", failed.len(), failed.join(" ")))
    }
}

// ---------------------------------------------------------------- hits

/// The timed words as readable lines, one per ~`--every` seconds.
fn transcript(args: &[String]) -> Result<(), String> {
    let o = parse(args, &["from", "to", "every"])?;
    let file = o.pos.first().ok_or("transcript needs a .vtt file")?;
    let from = o.num("from", 0.0)?;
    let to = o.num("to", f64::MAX)?;
    let every = o.num("every", 5.0)?;
    let src = fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let words = vtt::words(&src);
    let mut line_start: Option<f64> = None;
    let mut buf: Vec<&str> = Vec::new();
    for w in words.iter().filter(|w| w.t >= from && w.t <= to) {
        match line_start {
            Some(s) if w.t - s < every => buf.push(&w.text),
            _ => {
                if let Some(s) = line_start {
                    println!("{s:7.1}  {}", buf.join(" "));
                }
                line_start = Some(w.t);
                buf = vec![&w.text];
            }
        }
    }
    if let Some(s) = line_start {
        println!("{s:7.1}  {}", buf.join(" "));
    }
    Ok(())
}

fn hits(args: &[String]) -> Result<(), String> {
    let o = parse(args, &["words", "gap", "context"])?;
    let needles: Vec<String> = o
        .get("words")
        .unwrap_or("send,sent,sending,enter,submit")
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    let gap = o.num("gap", 4.0)?;
    let ctx = o.num("context", 7.0)? as usize;
    if o.pos.is_empty() {
        return Err("hits needs at least one .vtt file".into());
    }
    for file in &o.pos {
        let src = fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
        let words = vtt::words(&src);
        println!("# {} timed words in {file}", words.len());
        let idx = vtt::find(&words, &needles);
        for &i in &idx {
            println!("{:8.2}\t{}\t{}", words[i].t, words[i].text, vtt::context(&words, i, ctx));
        }
        let times: Vec<f64> = idx.iter().map(|&i| words[i].t).collect();
        for (a, b) in vtt::windows(&times, gap) {
            println!("WINDOW\t{:.1}\t{:.1}", (a - 2.0).max(0.0), b + 3.0);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- probe

#[derive(Debug, Clone)]
struct Probe {
    width: u32,
    height: u32,
    fps: f64,
    duration: f64,
}

fn probe(file: &str, verbose: bool) -> Result<Probe, String> {
    let mut c = Command::new(ffprobe());
    c.args([
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=width,height,r_frame_rate,avg_frame_rate:format=duration",
        "-of",
        "default=nw=1",
        file,
    ]);
    let s = run(&mut c, verbose)?;
    let mut p = Probe { width: 0, height: 0, fps: 0.0, duration: 0.0 };
    for line in s.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        match k {
            "width" => p.width = v.parse().unwrap_or(0),
            "height" => p.height = v.parse().unwrap_or(0),
            "avg_frame_rate" | "r_frame_rate" => {
                if p.fps == 0.0 || k == "avg_frame_rate" {
                    if let Some((a, b)) = v.split_once('/') {
                        let (a, b): (f64, f64) = (a.parse().unwrap_or(0.0), b.parse().unwrap_or(1.0));
                        if b > 0.0 && a > 0.0 {
                            p.fps = a / b;
                        }
                    }
                }
            }
            "duration" => p.duration = v.parse().unwrap_or(0.0),
            _ => {}
        }
    }
    Ok(p)
}

fn probe_cmd(args: &[String], verbose: bool) -> Result<(), String> {
    let o = parse(args, &[])?;
    for f in &o.pos {
        let p = probe(&safe(f), verbose)?;
        println!("{f}\t{}x{}\t{:.2} fps\t{:.2} s", p.width, p.height, p.fps, p.duration);
    }
    Ok(())
}

// ---------------------------------------------------------------- motion

/// Per-frame mean absolute change, split into the top 60% and bottom 40% of
/// the (cropped) frame. A message landing by the composer lights the bottom;
/// a list scrolling to make room lights both. Typing lights only a sliver at
/// the very bottom, which is why the split is 60/40 and not 50/50.
fn motion(args: &[String], verbose: bool) -> Result<(), String> {
    let o = parse(args, &["from", "to", "fps", "crop"])?;
    let file = o.pos.first().ok_or("motion needs a file")?;
    let from = o.time("from")?;
    let to = o.time("to")?;
    let fps = o.num("fps", 10.0)?;
    let (w, h) = (48usize, 96usize);
    let mut vf = format!("fps={fps}");
    if let Some(c) = o.get("crop") {
        vf.push_str(&format!(",crop={c}"));
    }
    vf.push_str(&format!(",scale={w}:{h},format=gray"));
    let mut c = Command::new(ffmpeg());
    c.args(["-v", "error", "-ss", &from.to_string(), "-t", &(to - from).to_string(), "-i", &safe(file)])
        .args(["-vf", &vf, "-f", "rawvideo", "-"]);
    if verbose {
        eprintln!("+ {}", show(&c));
    }
    let out = c.stdin(Stdio::null()).output().map_err(|e| format!("ffmpeg: {e}"))?;
    if !out.status.success() {
        return Err(format!("ffmpeg failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let frame = w * h;
    let frames: Vec<&[u8]> = out.stdout.chunks_exact(frame).collect();
    let split = (h as f64 * 0.6) as usize * w;
    println!("# t\ttop\tbottom   (mean |delta| per pixel, 0..255; {} frames @ {fps} fps)", frames.len());
    let mut series: Vec<(f64, f64, f64)> = Vec::new();
    for (i, win) in frames.windows(2).enumerate() {
        let (a, b) = (win[0], win[1]);
        let mut top = 0u64;
        let mut bot = 0u64;
        for k in 0..frame {
            let d = (a[k] as i32 - b[k] as i32).unsigned_abs() as u64;
            if k < split {
                top += d;
            } else {
                bot += d;
            }
        }
        let t = from + (i as f64 + 1.0) / fps;
        let top = top as f64 / split as f64;
        let bot = bot as f64 / (frame - split) as f64;
        series.push((t, top, bot));
        println!("{t:8.2}\t{top:6.2}\t{bot:6.2}");
    }
    // Peaks of bottom change, at least 1 s apart.
    let mut peaks: Vec<(f64, f64, f64)> = series.clone();
    peaks.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    let mut chosen: Vec<(f64, f64, f64)> = Vec::new();
    for p in peaks {
        if chosen.iter().all(|c| (c.0 - p.0).abs() >= 1.0) {
            chosen.push(p);
        }
        if chosen.len() >= 8 {
            break;
        }
    }
    chosen.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    println!("# bottom-change peaks (t, top, bottom):");
    for (t, top, bot) in chosen {
        println!("PEAK\t{t:.2}\t{top:.2}\t{bot:.2}");
    }
    Ok(())
}

// ---------------------------------------------------------------- activebox

/// Where the screen recording sits inside the frame: the bounding box of the
/// pixels that CHANGE over the video. A phone mirrored onto a static
/// background is the common tutorial layout and `cropdetect` cannot see it
/// (the background is a colour, not black). Profiles, not a raw bounding box:
/// a column counts only if at least `--min-frac` of its rows change, which
/// keeps a talking-head overlay in a corner from stretching the box.
fn activebox(args: &[String], verbose: bool) -> Result<(), String> {
    let o = parse(args, &["samples", "threshold", "min-frac", "from", "to"])?; // --map prints the grid
    if o.pos.is_empty() {
        return Err("activebox needs at least one file".into());
    }
    for file in &o.pos {
        match activebox_one(file, &o, verbose) {
            Ok(line) => println!("{file}\t{line}"),
            Err(e) => println!("{file}\tERROR\t{e}"),
        }
    }
    Ok(())
}

fn activebox_one(file: &str, o: &Opts, verbose: bool) -> Result<String, String> {
    let p = probe(&safe(file), verbose)?;
    if p.width == 0 || p.height == 0 || p.duration <= 0.0 {
        return Err(format!("{file}: could not probe dimensions/duration"));
    }
    let samples = o.num("samples", 40.0)?.max(4.0);
    let threshold = o.num("threshold", 28.0)?;
    let min_frac = o.num("min-frac", 0.35)?;
    let from = o.num("from", 0.0)?;
    let to = o.num("to", p.duration)?.min(p.duration);
    if to <= from {
        return Err("--to must be after --from".into());
    }
    let (w, h) = (192usize, ((192.0 * p.height as f64 / p.width as f64) as usize).max(8));
    let step = ((to - from) / samples).max(0.2);
    let vf = format!("fps=1/{step},scale={w}:{h},format=gray");
    let mut c = Command::new(ffmpeg());
    c.args(["-v", "error", "-ss", &from.to_string(), "-t", &(to - from).to_string(), "-i", &safe(file), "-vf", &vf, "-f", "rawvideo", "-"]);
    if verbose {
        eprintln!("+ {}", show(&c));
    }
    let out = c.stdin(Stdio::null()).output().map_err(|e| format!("ffmpeg: {e}"))?;
    if !out.status.success() {
        return Err(format!("ffmpeg failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let frame = w * h;
    let frames: Vec<&[u8]> = out.stdout.chunks_exact(frame).collect();
    if frames.len() < 2 {
        return Err("too few frames decoded".into());
    }
    let mut lo = vec![255u8; frame];
    let mut hi = vec![0u8; frame];
    for f in &frames {
        for k in 0..frame {
            lo[k] = lo[k].min(f[k]);
            hi[k] = hi[k].max(f[k]);
        }
    }
    let active: Vec<bool> = (0..frame).map(|k| (hi[k] - lo[k]) as f64 > threshold).collect();
    if o.get("map").is_some() {
        // 32 columns of cells, each a digit 0-9: the share of its pixels that change.
        let cell = w / 32;
        let rows = h / cell;
        println!("{file}: activity map, {} cols x {rows} rows, cell = {cell}px of {w}x{h}", w / cell);
        for cy in 0..rows {
            let mut line = String::new();
            for cx in 0..(w / cell) {
                let mut n = 0;
                let mut on = 0;
                for y in cy * cell..(cy + 1) * cell {
                    for x in cx * cell..(cx + 1) * cell {
                        n += 1;
                        if active[y * w + x] {
                            on += 1;
                        }
                    }
                }
                let d = ((on as f64 / n as f64) * 9.0).round() as u8;
                line.push(if d == 0 { '.' } else { (b'0' + d) as char });
            }
            println!("{line}");
        }
    }
    let col_frac: Vec<f64> = (0..w)
        .map(|x| (0..h).filter(|&y| active[y * w + x]).count() as f64 / h as f64)
        .collect();
    let (x0, x1) = longest_run(&col_frac, min_frac).ok_or("no active columns: is the video static?")?;
    let row_frac: Vec<f64> = (0..h)
        .map(|y| (x0..=x1).filter(|&x| active[y * w + x]).count() as f64 / (x1 - x0 + 1) as f64)
        .collect();
    let (y0, y1) = longest_run(&row_frac, min_frac).ok_or("no active rows")?;
    let sx = p.width as f64 / w as f64;
    let sy = p.height as f64 / h as f64;
    let even = |v: f64| ((v / 2.0).round() * 2.0) as u32;
    let cx = even(x0 as f64 * sx);
    let cy = even(y0 as f64 * sy);
    let cw = even((x1 - x0 + 1) as f64 * sx).min(p.width - cx);
    let ch = even((y1 - y0 + 1) as f64 * sy).min(p.height - cy);
    let frac = (cw * ch) as f64 / (p.width * p.height) as f64;
    Ok(format!("{cw}:{ch}:{cx}:{cy}\t{:.0}% of {}x{}\t{} samples", frac * 100.0, p.width, p.height, frames.len()))
}

/// Longest contiguous run of indices whose value is at least `min`.
fn longest_run(v: &[f64], min: f64) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    let mut start: Option<usize> = None;
    for i in 0..=v.len() {
        let on = i < v.len() && v[i] >= min;
        match (on, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                let run = (s, i - 1);
                if best.map(|(a, b)| run.1 - run.0 > b - a).unwrap_or(true) {
                    best = Some(run);
                }
                start = None;
            }
            _ => {}
        }
    }
    best
}

// ---------------------------------------------------------------- cropdetect

fn cropdetect(args: &[String], verbose: bool) -> Result<(), String> {
    let o = parse(args, &["at", "limit"])?;
    let file = o.pos.first().ok_or("cropdetect needs a file")?;
    let at = o.time("at")?;
    let limit = o.num("limit", 24.0)?;
    let mut c = Command::new(ffmpeg());
    c.args(["-v", "info", "-ss", &at.to_string(), "-t", "2", "-i", &safe(file)])
        .args(["-vf", &format!("cropdetect=limit={limit}:round=2:reset=0"), "-f", "null", "-"]);
    if verbose {
        eprintln!("+ {}", show(&c));
    }
    let out = c.stdin(Stdio::null()).output().map_err(|e| format!("ffmpeg: {e}"))?;
    let err = String::from_utf8_lossy(&out.stderr);
    let last = err
        .lines()
        .filter_map(|l| l.split("crop=").nth(1))
        .last()
        .ok_or("cropdetect printed no crop= line")?;
    println!("{}", last.split_whitespace().next().unwrap_or(last));
    Ok(())
}

// ---------------------------------------------------------------- frames in, pictures out
//
// ffmpeg decodes (seek, fps, crop, scale) to raw rgb24 on a pipe; the
// compositing -- tiles, stamps, labels, the end-hold of a GIF -- happens here
// in Rust; ffmpeg encodes the result from a pipe. No `drawtext` anywhere: the
// static build on the render box has none.

fn safe(file: &str) -> String {
    // A video id can start with '-' and ffmpeg would read it as an option.
    if file.starts_with('-') {
        format!("./{file}")
    } else {
        file.to_string()
    }
}

fn parse_crop(c: &str) -> Result<(u32, u32, u32, u32), String> {
    let v: Vec<u32> = c.split(':').map(|s| s.trim().parse::<u32>()).collect::<Result<_, _>>().map_err(|_| format!("bad --crop {c} (want W:H:X:Y)"))?;
    if v.len() != 4 {
        return Err(format!("bad --crop {c} (want W:H:X:Y)"));
    }
    Ok((v[0], v[1], v[2], v[3]))
}

struct Frames {
    bytes: Vec<u8>,
    w: usize,
    h: usize,
}

impl Frames {
    fn count(&self) -> usize {
        self.bytes.len() / (self.w * self.h * 3)
    }
    fn frame(&self, i: usize) -> &[u8] {
        let n = self.w * self.h * 3;
        &self.bytes[i * n..(i + 1) * n]
    }
}

/// Decode `[from, to)` at `fps`, cropped, scaled to `width` columns.
fn decode(file: &str, from: f64, to: f64, fps: f64, crop: Option<&str>, width: u32, verbose: bool) -> Result<Frames, String> {
    let p = probe(&safe(file), verbose)?;
    let (cw, ch) = match crop {
        Some(c) => {
            let (w, h, _, _) = parse_crop(c)?;
            (w, h)
        }
        None => (p.width, p.height),
    };
    if cw == 0 || ch == 0 {
        return Err("zero-sized source".into());
    }
    let fw = (width / 2 * 2).max(2);
    let fh = (((fw as f64 * ch as f64 / cw as f64) / 2.0).round() as u32 * 2).max(2);
    let mut vf = format!("fps={fps}");
    if let Some(c) = crop {
        vf.push_str(&format!(",crop={c}"));
    }
    vf.push_str(&format!(",scale={fw}:{fh}:flags=lanczos,format=rgb24"));
    let mut c = Command::new(ffmpeg());
    c.args(["-v", "error", "-ss", &from.to_string(), "-t", &(to - from).to_string(), "-i", &safe(file)])
        .args(["-vf", &vf, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]);
    if verbose {
        eprintln!("+ {}", show(&c));
    }
    let out = c.stdin(Stdio::null()).output().map_err(|e| format!("ffmpeg: {e}"))?;
    if !out.status.success() {
        return Err(format!("ffmpeg failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let f = Frames { bytes: out.stdout, w: fw as usize, h: fh as usize };
    if f.count() == 0 {
        return Err(format!("no frames decoded from {file} at {from}..{to} (past the end?)"));
    }
    Ok(f)
}

/// Encode one rgb24 picture through ffmpeg (jpg/png by extension).
fn encode_image(c: &draw::Canvas, out: &Path, verbose: bool) -> Result<(), String> {
    let mut cmd = Command::new(ffmpeg());
    cmd.args(["-y", "-v", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &format!("{}x{}", c.w, c.h), "-i", "-"])
        .args(["-frames:v", "1", "-q:v", "3"])
        .arg(out);
    pipe_in(&mut cmd, &c.px, verbose)
}

/// Encode a run of same-sized rgb24 frames as a looping GIF at `rate` fps.
fn encode_gif(frames: &[Vec<u8>], w: usize, h: usize, rate: f64, out: &Path, verbose: bool) -> Result<(), String> {
    let mut cmd = Command::new(ffmpeg());
    cmd.args(["-y", "-v", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &format!("{w}x{h}"), "-framerate", &format!("{rate}"), "-i", "-"])
        .args([
            "-filter_complex",
            "split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
            "-loop",
            "0",
        ])
        .arg(out);
    let mut all = Vec::with_capacity(frames.len() * w * h * 3);
    for f in frames {
        all.extend_from_slice(f);
    }
    pipe_in(&mut cmd, &all, verbose)
}

/// Encode same-sized rgb24 frames as an H.264 mp4 at `rate` fps.
fn encode_mp4(frames: &[Vec<u8>], w: usize, h: usize, rate: f64, out: &Path, verbose: bool) -> Result<(), String> {
    let mut cmd = Command::new(ffmpeg());
    cmd.args(["-y", "-v", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &format!("{w}x{h}"), "-framerate", &format!("{rate}"), "-i", "-"])
        .args(["-an", "-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p", "-movflags", "+faststart"])
        .arg(out);
    let mut all = Vec::with_capacity(frames.len() * w * h * 3);
    for f in frames {
        all.extend_from_slice(f);
    }
    pipe_in(&mut cmd, &all, verbose)
}

fn pipe_in(cmd: &mut Command, bytes: &[u8], verbose: bool) -> Result<(), String> {
    if verbose {
        eprintln!("+ {} ({} B on stdin)", show(cmd), bytes.len());
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run {}: {e}", cmd.get_program().to_string_lossy()))?;
    {
        let mut stdin = child.stdin.take().ok_or("no stdin")?;
        stdin.write_all(bytes).map_err(|e| format!("writing to ffmpeg: {e}"))?;
    }
    let out = child.wait_with_output().map_err(|e| format!("ffmpeg: {e}"))?;
    if !out.status.success() {
        return Err(format!("ffmpeg failed ({}):\n{}", out.status, String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

fn out_path(o: &Opts) -> Result<PathBuf, String> {
    let p = PathBuf::from(o.need("out")?);
    if let Some(d) = p.parent() {
        if !d.as_os_str().is_empty() {
            fs::create_dir_all(d).map_err(|e| format!("mkdir {}: {e}", d.display()))?;
        }
    }
    Ok(p)
}

fn confirm_output(p: &Path) -> Result<(), String> {
    let md = fs::metadata(p).map_err(|e| format!("{} was not written: {e}", p.display()))?;
    if md.len() == 0 {
        return Err(format!("{} is empty", p.display()));
    }
    println!("{}\t{} B", p.display(), md.len());
    Ok(())
}

const INK: [u8; 3] = [255, 255, 255];
const PAPER: [u8; 3] = [32, 32, 32];
const STAMP_BG: [u8; 3] = [0, 0, 0];

// ---------------------------------------------------------------- sheet

/// A contact sheet: `cols` tiles per row, every tile stamped with its video
/// time, an optional label strip on top.
fn sheet(args: &[String], verbose: bool) -> Result<(), String> {
    let o = parse(args, &["from", "to", "out", "fps", "cols", "width", "crop", "label", "max"])?;
    let file = o.pos.first().ok_or("sheet needs a file")?;
    let from = o.time("from")?;
    let to = o.time("to")?;
    if to <= from {
        return Err("--to must be after --from".into());
    }
    let fps = o.num("fps", 10.0)?;
    let cols = o.num("cols", 8.0)?.max(1.0) as usize;
    let width = o.num("width", 180.0)? as u32;
    let max = o.num("max", 400.0)? as usize;
    let out = out_path(&o)?;
    let f = decode(file, from, to, fps, o.get("crop"), width, verbose)?;
    let n = f.count().min(max);
    let rows = (n + cols - 1) / cols;
    let pad = 2usize;
    let label_h = if o.get("label").is_some() { 24 } else { 0 };
    let cw = f.w + pad;
    let ch = f.h + pad;
    let mut canvas = draw::Canvas::new(cols * cw + pad, rows * ch + pad + label_h, PAPER);
    if let Some(l) = o.get("label") {
        canvas.text(6, 6, l, 2, INK, None);
    }
    let scale = if f.w >= 150 { 2 } else { 1 };
    for i in 0..n {
        let (r, c) = (i / cols, i % cols);
        let x = (pad + c * cw) as i64;
        let y = (pad + label_h + r * ch) as i64;
        canvas.blit(f.frame(i), f.w, f.h, x, y);
        let t = from + i as f64 / fps;
        canvas.text(x + 3, y + f.h as i64 - (7 * scale) as i64 - 4, &draw::hms(t), scale, INK, Some(STAMP_BG));
    }
    encode_image(&canvas, &out, verbose)?;
    confirm_output(&out)
}

/// Several sheets (or any images) stacked vertically at one width.
fn stack(args: &[String], verbose: bool) -> Result<(), String> {
    let o = parse(args, &["out", "width"])?;
    let out = out_path(&o)?;
    let width = o.num("width", 1440.0)? as u32;
    if o.pos.len() < 2 {
        return Err("stack needs at least two images".into());
    }
    let mut c = Command::new(ffmpeg());
    c.args(["-y", "-v", "error"]);
    let mut graph = String::new();
    for (i, f) in o.pos.iter().enumerate() {
        c.args(["-i", &safe(f)]);
        graph.push_str(&format!("[{i}:v]scale={width}:-2,format=rgb24[s{i}];"));
    }
    for i in 0..o.pos.len() {
        graph.push_str(&format!("[s{i}]"));
    }
    graph.push_str(&format!("vstack=inputs={}", o.pos.len()));
    c.args(["-filter_complex", &graph, "-frames:v", "1", "-q:v", "3"]).arg(&out);
    run(&mut c, verbose)?;
    confirm_output(&out)
}

// ---------------------------------------------------------------- gif / cut

/// Frames with the optional stamp and label drawn on, plus the end hold.
fn dress(f: &Frames, from: f64, fps: f64, o: &Opts, hold_frames: usize) -> Vec<Vec<u8>> {
    let stamp = o.get("stamp").is_some();
    let label = o.get("label");
    let mut frames: Vec<Vec<u8>> = Vec::with_capacity(f.count() + hold_frames);
    for i in 0..f.count() {
        if !stamp && label.is_none() {
            frames.push(f.frame(i).to_vec());
            continue;
        }
        let mut c = draw::Canvas { w: f.w, h: f.h, px: f.frame(i).to_vec() };
        if let Some(l) = label {
            c.text(4, 4, l, 2, INK, Some(STAMP_BG));
        }
        if stamp {
            let t = from + i as f64 / fps;
            c.text(4, f.h as i64 - 14 - 4, &draw::hms(t), 2, INK, Some(STAMP_BG));
        }
        frames.push(c.px);
    }
    if let Some(last) = frames.last().cloned() {
        for _ in 0..hold_frames {
            frames.push(last.clone());
        }
    }
    frames
}

fn gif(args: &[String], verbose: bool) -> Result<(), String> {
    let o = parse(args, &["from", "to", "out", "fps", "width", "speed", "hold", "crop", "label"])?;
    let file = o.pos.first().ok_or("gif needs a file")?;
    let from = o.time("from")?;
    let to = o.time("to")?;
    if to <= from {
        return Err("--to must be after --from".into());
    }
    let fps = o.num("fps", 15.0)?;
    let width = o.num("width", 360.0)? as u32;
    let speed = o.num("speed", 1.0)?;
    let hold = o.num("hold", 0.8)?;
    let out = out_path(&o)?;
    let f = decode(file, from, to, fps, o.get("crop"), width, verbose)?;
    let rate = fps * speed;
    let frames = dress(&f, from, fps, &o, (hold * rate).round() as usize);
    encode_gif(&frames, f.w, f.h, rate, &out, verbose)?;
    confirm_output(&out)
}

/// An mp4 clip: native frame rate unless `--fps`, optional slow-down, crop,
/// width, stamp and label.
fn cut(args: &[String], verbose: bool) -> Result<(), String> {
    let o = parse(args, &["from", "to", "out", "fps", "width", "speed", "hold", "crop", "label"])?;
    let file = o.pos.first().ok_or("cut needs a file")?;
    let from = o.time("from")?;
    let to = o.time("to")?;
    if to <= from {
        return Err("--to must be after --from".into());
    }
    let p = probe(&safe(file), verbose)?;
    let fps = o.num("fps", if p.fps > 0.0 { p.fps } else { 30.0 })?;
    let width = o.num("width", 540.0)? as u32;
    let speed = o.num("speed", 1.0)?;
    let hold = o.num("hold", 0.0)?;
    let out = out_path(&o)?;
    let f = decode(file, from, to, fps, o.get("crop"), width, verbose)?;
    let rate = fps * speed;
    let frames = dress(&f, from, fps, &o, (hold * rate).round() as usize);
    encode_mp4(&frames, f.w, f.h, rate, &out, verbose)?;
    confirm_output(&out)
}
