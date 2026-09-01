//! Tests that need a binary this box may not have.
//!
//! TIERS, and no silent passes. Everything pure is unit-tested next to the code
//! it tests. Everything here drives a real external program; when that program
//! is absent the test SKIPS OUT LOUD, naming what was missing, and never
//! reports a pass it did not earn. `cargo test --release` from `tools/` runs
//! the lot.
//!
//! What runs where, as of the box this was written on (a Meta devserver:
//! curl and node present; no ffmpeg, no gh, no Chrome):
//!
//!   anonymous gate      curl        RUNS -- against a stub server on loopback
//!   assembly oracle     node        RUNS -- differential, against the deleted
//!                                   `playtest.sh` splice it replaces
//!   split encode        ffmpeg      skips
//!   release chain       gh          skips (see the note on that test)
//!   browser playtest    Chrome      skips

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clip::platform::{self, which};
use clip::playtest;
use clip::ship::{self, Cfg};
use clip::split;

/// A skip that is impossible to miss.
///
/// The test harness swallows `println!`/`eprintln!` from a passing test, so a
/// skip printed with them is a skip nobody ever sees -- which is the same thing
/// as a silent pass. This writes to the process's real stderr, underneath the
/// capture.
fn skip(test: &str, reason: &str) {
    let line = format!("\n  SKIP  {test}\n        {reason}\n");
    match std::fs::OpenOptions::new().write(true).open("/dev/stderr") {
        Ok(mut f) => {
            let _ = f.write_all(line.as_bytes());
        }
        Err(_) => eprintln!("{line}"),
    }
}

// ---------------------------------------------------------------------------
// a stub of the attachment store, on loopback
// ---------------------------------------------------------------------------

/// Serves one canned response per request, then repeats the last one.
/// Returns the URL to fetch and a counter of requests actually served.
fn stub_server(script: Vec<(u16, Vec<u8>)>) -> (String, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = l.local_addr().unwrap().port();
    let served = Arc::new(AtomicUsize::new(0));
    let counter = served.clone();
    std::thread::spawn(move || {
        for conn in l.incoming() {
            let Ok(mut s) = conn else { break };
            let n = counter.fetch_add(1, Ordering::SeqCst);
            let (code, body) = script[n.min(script.len() - 1)].clone();
            let _ = read_request(&mut s);
            let head = format!(
                "HTTP/1.1 {code} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                if code == 200 { "OK" } else { "Not Found" },
                body.len()
            );
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&body);
            let _ = s.flush();
        }
    });
    (format!("http://127.0.0.1:{port}/asset.mp4"), served)
}

fn read_request(s: &mut TcpStream) -> std::io::Result<()> {
    let mut buf = [0u8; 1024];
    let mut seen = Vec::new();
    loop {
        let n = s.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        seen.extend_from_slice(&buf[..n]);
        if seen.windows(4).any(|w| w == b"\r\n\r\n") {
            return Ok(());
        }
    }
}

fn gate_cfg(curl: PathBuf) -> Cfg {
    Cfg {
        curl,
        attempts: 6,
        retry_delay: Duration::from_millis(20),
        ..Cfg::default()
    }
}

fn curl_or_skip(test: &str) -> Option<PathBuf> {
    let c = PathBuf::from(ship::DEFAULT_CURL);
    if c.is_file() {
        Some(c)
    } else {
        skip(test, &format!("no {} on this box", ship::DEFAULT_CURL));
        None
    }
}

#[test]
fn gate_waits_out_the_registration_delay_and_then_passes() {
    let name = "gate_waits_out_the_registration_delay_and_then_passes";
    let Some(curl) = curl_or_skip(name) else { return };
    // Exactly the shape 208024 produced: 404 while the release-body edit
    // propagates, then a 200 whose bytes are still short, then the real file.
    let full = vec![7u8; 4096];
    let (url, served) = stub_server(vec![
        (404, vec![]),
        (404, vec![]),
        (200, vec![7u8; 100]),
        (200, full.clone()),
    ]);
    let dir = tempdir("gate-pass");
    let out = dir.join("anon.mp4");
    let probe = |p: &Path| {
        let n = std::fs::metadata(p).unwrap().len();
        if n >= 4096 {
            Ok(12.345)
        } else {
            Err("truncated".to_string())
        }
    };
    let passed = ship::gate(&gate_cfg(curl), &url, &out, probe).expect("gate should pass");
    assert_eq!(passed.bytes, full.len() as u64);
    assert_eq!(passed.duration, 12.345);
    assert_eq!(served.load(Ordering::SeqCst), 4, "one reading is not a verdict");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn gate_refuses_an_asset_that_stays_404() {
    let name = "gate_refuses_an_asset_that_stays_404";
    let Some(curl) = curl_or_skip(name) else { return };
    let (url, served) = stub_server(vec![(404, vec![])]);
    let dir = tempdir("gate-404");
    let e = ship::gate(&gate_cfg(curl), &url, &dir.join("a.mp4"), |_| Ok(1.0)).unwrap_err();
    assert!(e.contains("ANONYMOUS GATE FAILED: http 404"), "{e}");
    assert!(e.contains("NOT published"), "{e}");
    assert_eq!(served.load(Ordering::SeqCst), 6);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn gate_refuses_bytes_that_never_probe() {
    let name = "gate_refuses_bytes_that_never_probe";
    let Some(curl) = curl_or_skip(name) else { return };
    // 200 all the way, but the body is never a playable file: published would
    // be a link that serves an error page with a video extension.
    let (url, _) = stub_server(vec![(200, b"<html>not a video</html>".to_vec())]);
    let dir = tempdir("gate-junk");
    let e = ship::gate(&gate_cfg(curl), &url, &dir.join("a.mp4"), |_| {
        Err("does not probe".to_string())
    })
    .unwrap_err();
    assert!(e.contains("bytes that do not probe"), "{e}");
    assert!(e.contains("NOT published"), "{e}");
    let _ = std::fs::remove_dir_all(dir);
}

/// THE GATE CARRIES NO CREDENTIALS -- with a positive control.
///
/// A test that only asserts "the gate got 200" would pass whether or not the
/// environment was scrubbed. So: poison the parent environment with a proxy
/// that refuses connections. The CONTROL fetch, run the ordinary way, must
/// fail on it -- that is what proves the poison is real and the test can see
/// leakage at all. The gate, run the same instant, must reach the server
/// anyway, because `env_clear` (the Rust spelling of `env -i`) drops the proxy
/// along with every cookie jar, GH_TOKEN and netrc that a real box carries.
#[test]
fn gate_runs_with_a_scrubbed_environment() {
    let name = "gate_runs_with_a_scrubbed_environment";
    let Some(curl) = curl_or_skip(name) else { return };
    let dead = TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_port = dead.local_addr().unwrap().port();
    drop(dead); // nothing is listening there now
    let poison = format!("http://127.0.0.1:{dead_port}");
    let (url, _) = stub_server(vec![(200, vec![1u8; 512])]);
    let dir = tempdir("gate-scrub");

    // THE POISON IS A CONFIG FILE, NOT ONLY A PROXY -- and that is the fix for
    // a control that could not fail.
    //
    // This used to poison `http_proxy`/`https_proxy` alone and assert the
    // control fetch broke on them. It does not break on a normal box: a real
    // `~/.curlrc` carries
    //
    //     noproxy = "...,localhost,127.0.0.1,..."
    //
    // so curl exempts the stub server from the proxy, the control SUCCEEDS,
    // and the assertion that "the control must FAIL" fires -- reporting a
    // scrubbing bug that does not exist. Reproduced outside the test: with
    // both proxy variables pointed at a dead port, a 127.0.0.1 fetch still
    // returns 200.
    //
    // `CURL_HOME` cannot be exempted that way: curl reads its config from
    // there in preference to `$HOME`, so a `.curlrc` we write is the only one
    // it sees. `max-filesize = 1` fails the 512-byte transfer for a reason
    // that has nothing to do with networks, proxies or name resolution.
    //
    // It also tests something the proxy poison never did. `env_clear` drops
    // CURL_HOME along with everything else, so the gate reads NO user config
    // at all -- which is the actual promise: a box's own curlrc, cookie jar,
    // netrc and tokens must not reach a published fetch.
    let curl_home = dir.join("poisoned-curl-home");
    std::fs::create_dir_all(&curl_home).expect("curl home");
    std::fs::write(curl_home.join(".curlrc"), "max-filesize = 1\n").expect("poisoned curlrc");

    // control: the same fetch, inheriting an environment
    let control = Command::new(&curl)
        .env("http_proxy", &poison)
        .env("https_proxy", &poison)
        .env("CURL_HOME", &curl_home)
        .args(ship::curl_argv(&dir.join("control.bin"), &url))
        .output()
        .expect("run curl");
    // Judge the EXIT STATUS, not the printed http_code: curl reports `200` as
    // soon as the response line arrives and only then aborts the body, so the
    // code is 200 on a transfer that failed. The old assertion compared the
    // code and would have been fooled even where the poison did bite.
    assert!(
        !control.status.success(),
        "the control must FAIL, or this test proves nothing about scrubbing \
         (curl exited {:?})",
        control.status.code()
    );

    // NOTE: set_var mutates the whole process, and tests in this binary run in
    // parallel threads. It is safe only because no other test here reads these
    // names; a future test that does will need a different arrangement.
    std::env::set_var("http_proxy", &poison);
    std::env::set_var("https_proxy", &poison);
    std::env::set_var("CURL_HOME", &curl_home);
    std::env::set_var("GH_TOKEN", "definitely-not-a-real-token");
    let passed = ship::gate(&gate_cfg(curl), &url, &dir.join("anon.bin"), |_| Ok(0.5));
    std::env::remove_var("http_proxy");
    std::env::remove_var("https_proxy");
    std::env::remove_var("CURL_HOME");
    std::env::remove_var("GH_TOKEN");
    let passed = passed.expect("the scrubbed gate should reach the server");
    assert_eq!(passed.bytes, 512);
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// the release chain (gh)
// ---------------------------------------------------------------------------

/// The `gh` steps are not exercised end to end, on this box or any other.
///
/// Not because `gh` is missing here (it is), but because the only thing they
/// could be run against is the real `videos-v1` release of the real repository:
/// `release upload --clobber` and `release edit --notes-file` both write, and a
/// test that rewrites the published listing to prove it can is worse than no
/// test. What is checkable without a network is checked as pure functions next
/// to the code: the argv of all three calls, the release-body insertion in both
/// its shapes, its idempotency, and the asset-URL refusal.
#[test]
fn release_chain_is_never_exercised_against_the_live_release() {
    let name = "release_chain_is_never_exercised_against_the_live_release";
    let reason = match which("gh") {
        Some(p) => format!(
            "{} exists, but running the chain would write to the live {} release of {} \
             — argv and body-insertion are unit-tested instead",
            p.display(),
            ship::DEFAULT_RELEASE,
            ship::DEFAULT_REPO
        ),
        None => format!(
            "no gh on this box (and it would write to the live {} release anyway)",
            ship::DEFAULT_RELEASE
        ),
    };
    skip(name, &reason);
}

// ---------------------------------------------------------------------------
// the encode (ffmpeg)
// ---------------------------------------------------------------------------

/// Two real clips of different lengths, stacked: the output must be as long as
/// the LONGER input, which is the whole point of the tpad/trim pair.
#[test]
fn split_holds_the_shorter_run_to_the_length_of_the_longer() {
    let name = "split_holds_the_shorter_run_to_the_length_of_the_longer";
    let ff = match platform::from_env() {
        Ok(ff) => ff,
        Err(e) => return skip(name, &e),
    };
    if ff.font.is_none() {
        return skip(name, "an ffmpeg is present but no drawtext font is — split refuses");
    }
    if !ff.has_drawtext() {
        return skip(
            name,
            &format!(
                "{} is built without libfreetype (no drawtext filter) — the same shape as the \
                 Mac's ffmpeg, which is why this runs on the render box",
                ff.ffmpeg.display()
            ),
        );
    }
    let dir = tempdir("split-e2e");
    let (short, long, out) = (dir.join("s.mp4"), dir.join("l.mp4"), dir.join("o.mp4"));
    for (p, d) in [(&short, "2"), (&long, "5")] {
        let ok = Command::new(&ff.ffmpeg)
            .args([
                "-v", "error", "-y", "-f", "lavfi", "-i",
                &format!("testsrc=size=320x240:rate=30:duration={d}"),
                "-c:v", "libx264", "-pix_fmt", "yuv420p",
            ])
            .arg(p)
            .status();
        assert!(matches!(ok, Ok(s) if s.success()), "fixture encode failed");
    }
    split::run(&ff, &short, &long, "TAS", "HUMAN", &out).expect("split");
    let d = ff.probe_duration(&out).expect("output probes");
    assert!((4.9..5.3).contains(&d), "output was {d}s, expected the longer run's 5s");
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// the browser (Chrome) and the assembly oracle (node)
// ---------------------------------------------------------------------------

fn trainer_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../trainer"))
}

/// The real page, in a real browser, scoring real runs.
#[test]
fn the_trainer_page_scores_a_run_in_a_real_browser() {
    let name = "the_trainer_page_scores_a_run_in_a_real_browser";
    let chrome = match playtest::find_chrome(None) {
        Ok(c) => c,
        Err(e) => return skip(name, &e),
    };
    // A browser that is installed is not a browser that works: probe it on
    // about:blank first, so a box where headless Chrome cannot dump at all
    // skips for that reason instead of failing as if the page were broken.
    if let Err(e) = playtest::can_dump_dom(&chrome) {
        return skip(name, &e);
    }
    let lines = playtest::run(&trainer_dir(), &chrome).expect("the page should reach a verdict");
    assert_eq!(lines[0], "RESULT");
    assert!(
        lines.iter().any(|l| l.starts_with("on-tape ")),
        "no on-tape player in {lines:?}"
    );
    assert!(
        lines.iter().all(|l| !l.contains("DID-NOT-FINISH")),
        "{lines:?}"
    );
}

/// DIFFERENTIAL ORACLE for the assembly port.
///
/// `playtest.sh` built the page with a `node -e` one-liner; this crate builds
/// it with `str::replacen`. The claim "the port is faithful" is worth exactly
/// as much as the check behind it, so here the deleted implementation is run
/// one more time, on the real trainer sources, and the two pages are compared
/// byte for byte. Node is used only as the oracle -- nothing in the shipped
/// path needs it any more, which is the point of the port.
#[test]
fn assembly_matches_the_node_splice_it_replaces() {
    let name = "assembly_matches_the_node_splice_it_replaces";
    let Some(node) = which("node") else {
        return skip(name, "no node on this box — the assembly is covered by unit tests only");
    };
    let dir = trainer_dir();
    if !dir.join("index.html").is_file() {
        return skip(name, "trainer sources are not in this checkout");
    }
    let out = tempdir("assembly-oracle");
    let program = r#"
const fs=require("fs"), d=process.argv[1], o=process.argv[2];
let h=fs.readFileSync(d+"/index.html","utf8");
h=h.replace("<script>","<script>"+fs.readFileSync(d+"/playtest-pump.js","utf8")+"</"+"script><script>");
h=h.replace("</body>","<script>"+fs.readFileSync(d+"/playtest-drive.js","utf8")+"</"+"script></body>");
fs.writeFileSync(o+"/pt.html",h);"#;
    let st = Command::new(node)
        .args(["-e", program])
        .arg(&dir)
        .arg(&out)
        .status()
        .expect("run node");
    assert!(st.success(), "the oracle itself failed");
    let reference = std::fs::read_to_string(out.join("pt.html")).unwrap();

    let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap();
    let ours = playtest::assemble(
        &read("index.html"),
        &read("playtest-pump.js"),
        &read("playtest-drive.js"),
    )
    .expect("assemble");
    assert_eq!(ours.len(), reference.len(), "assembled pages differ in length");
    assert!(ours == reference, "assembled page differs from the node splice");
    let _ = std::fs::remove_dir_all(out);
}

fn tempdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cliptest-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}
