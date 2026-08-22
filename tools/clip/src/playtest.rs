//! `playtest` -- the trainer page, end to end in a REAL browser.
//!
//! Real canvas, real `KeyboardEvent`s, real DOM; only the frame clock is ours
//! (`playtest-pump.js` replaces `requestAnimationFrame` and `performance.now`),
//! so the run reproduces. One line of verdict per simulated player.
//!
//! `trainer/headless.js` is faster and covers the judging logic against a stub
//! DOM. This is the check that the actual page a person opens behaves the same
//! way -- it is what found a missing `setLineDash`, and what confirmed a real
//! browser scores an on-tape run S+ 100%, 27/27 perfect, 0 miss, 0 extra.
//!
//! The page is assembled here rather than by the `node -e` one-liner the shell
//! version used: it is three string splices, and Node was the only thing that
//! step needed a runtime for.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::platform::which;

/// Chrome's clock budget for the whole run, in ms. The pump advances 16 ms a
/// frame and four players are simulated, so this is generous.
pub const VIRTUAL_TIME_BUDGET_MS: u32 = 20_000;
/// How long to wait for a verdict to land in the dumped DOM.
pub const WAIT_SECONDS: u32 = 60;
/// How long the "does this browser work at all" probe waits.
pub const PROBE_SECONDS: u32 = 20;

pub const CHROME_CANDIDATES: &[&str] = &[
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
];
pub const MAC_CHROME: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

/// Build the page the browser actually loads.
///
/// `playtest-pump.js` goes in front of the page's own script (it has to replace
/// `requestAnimationFrame` *before* anything schedules a frame), and
/// `playtest-drive.js` goes last, after the DOM the driver clicks exists.
///
/// Both markers are required. The shell version spliced blind, so a page whose
/// first `<script>` had moved produced a browser run that simply never reached
/// a verdict -- reported as "chrome produced no DOM (is the path right?)",
/// which sent the reader off to check Chrome.
pub fn assemble(index_html: &str, pump_js: &str, drive_js: &str) -> Result<String, String> {
    if !index_html.contains("<script>") {
        return Err("trainer/index.html has no <script> to put the frame pump in front of".into());
    }
    if !index_html.contains("</body>") {
        return Err("trainer/index.html has no </body> to put the driver before".into());
    }
    let with_pump = index_html.replacen(
        "<script>",
        &format!("<script>{pump_js}</script><script>"),
        1,
    );
    Ok(with_pump.replacen(
        "</body>",
        &format!("<script>{drive_js}</script></body>"),
        1,
    ))
}

pub fn chrome_argv(profile: &Path, page_url: &str) -> Vec<String> {
    vec![
        "--headless=new".into(),
        "--disable-gpu".into(),
        "--no-first-run".into(),
        format!("--user-data-dir={}", profile.display()),
        format!("--virtual-time-budget={VIRTUAL_TIME_BUDGET_MS}"),
        "--dump-dom".into(),
        page_url.into(),
    ]
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// `<title>RESULT ...</title>`: one line per simulated player.
    Result(Vec<String>),
    /// `<title>ERR ...</title>`: the driver threw inside the page.
    Threw(String),
    /// No verdict yet -- including a page still wearing its own title.
    None,
}

/// Read the verdict out of a dumped DOM.
///
/// ONLY `RESULT` and `ERR` count. The shell version's last check was "does the
/// DOM contain any `<title>` at all", so a run that timed out still holding the
/// page's own title -- or the driver's `D boot` placeholder -- printed that
/// title and exited 0. A test that passes when it did not run is decoration.
pub fn read_verdict(dom: &str) -> Verdict {
    let Some(title) = first_title(dom) else {
        return Verdict::None;
    };
    if let Some(rest) = title.strip_prefix("RESULT") {
        Verdict::Result(
            std::iter::once("RESULT".to_string())
                .chain(rest.trim().split(" | ").map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .collect(),
        )
    } else if let Some(rest) = title.strip_prefix("ERR") {
        Verdict::Threw(rest.trim().to_string())
    } else {
        Verdict::None
    }
}

fn first_title(dom: &str) -> Option<&str> {
    let i = dom.find("<title>")? + "<title>".len();
    let rest = &dom[i..];
    let j = rest.find("</title>")?;
    Some(&rest[..j])
}

/// Where the trainer's sources are, if they can be found without being told.
pub fn find_trainer_dir() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("trainer"),
        PathBuf::from("."),
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../trainer")),
    ];
    candidates
        .into_iter()
        .find(|d| d.join("index.html").is_file() && d.join("playtest-drive.js").is_file())
}

/// Which browser, decided explicitly and reported when there is none.
pub fn find_chrome(explicit: Option<&str>) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        let p = PathBuf::from(p);
        return if p.is_file() {
            Ok(p)
        } else {
            Err(format!("no browser at {}", p.display()))
        };
    }
    if let Some(p) = CHROME_CANDIDATES.iter().find_map(|c| which(c)) {
        return Ok(p);
    }
    let mac = PathBuf::from(MAC_CHROME);
    if mac.is_file() {
        return Ok(mac);
    }
    Err(format!(
        "no headless browser: none of {} on PATH, and no {MAC_CHROME}. Pass one, or set CHROME.",
        CHROME_CANDIDATES.join(", ")
    ))
}

/// Can this browser complete a `--dump-dom` at all, on this box?
///
/// A found browser is not a working one. On a devserver, headless Chrome
/// starts, prints its version, and then hangs producing nothing -- no DOM for
/// `about:blank`, let alone for the trainer page. Without this probe that is
/// indistinguishable from "the page never scored a run", which sends the reader
/// off to debug the trainer instead of the box.
pub fn can_dump_dom(chrome: &Path) -> Result<(), String> {
    let dir = crate::proc::scratch_dir("playtest-probe")?;
    let dom = dir.join("blank.html");
    let r = (|| -> Result<(), String> {
        let f = std::fs::File::create(&dom).map_err(|e| format!("{e}"))?;
        let mut child = Command::new(chrome)
            .args(chrome_argv(&dir.join("profile"), "about:blank"))
            .stdout(Stdio::from(f))
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", chrome.display()))?;
        let mut ok = false;
        for _ in 0..PROBE_SECONDS {
            std::thread::sleep(Duration::from_secs(1));
            if std::fs::metadata(&dom).map(|m| m.len()).unwrap_or(0) > 0 {
                ok = true;
                break;
            }
        }
        let _ = child.kill();
        let _ = child.wait();
        if ok {
            Ok(())
        } else {
            Err(format!(
                "{} produced no DOM for about:blank in {PROBE_SECONDS}s — headless Chrome \
                 does not work on this box",
                chrome.display()
            ))
        }
    })();
    let _ = std::fs::remove_dir_all(&dir);
    r
}

/// Assemble, run, wait for the verdict, kill the browser, print what it said.
pub fn run(trainer: &Path, chrome: &Path) -> Result<Vec<String>, String> {
    let read = |name: &str| {
        std::fs::read_to_string(trainer.join(name))
            .map_err(|e| format!("cannot read {}: {e}", trainer.join(name).display()))
    };
    let page = assemble(
        &read("index.html")?,
        &read("playtest-pump.js")?,
        &read("playtest-drive.js")?,
    )?;

    let dir = crate::proc::scratch_dir("playtest")?;
    let html = dir.join("pt.html");
    let dom_path = dir.join("dom.html");
    let r = run_in(&page, &html, &dom_path, &dir, chrome);
    let _ = std::fs::remove_dir_all(&dir);
    r
}

fn run_in(
    page: &str,
    html: &Path,
    dom_path: &Path,
    dir: &Path,
    chrome: &Path,
) -> Result<Vec<String>, String> {
    std::fs::write(html, page).map_err(|e| format!("cannot write {}: {e}", html.display()))?;
    let dom_file = std::fs::File::create(dom_path)
        .map_err(|e| format!("cannot create {}: {e}", dom_path.display()))?;

    // Chrome on this box does not always exit after --dump-dom, so: run it
    // detached, wait for the verdict to land in the file, then kill it.
    //
    // Never pipe its stdout — closing the pipe early wedges it. `dom.html` is a
    // real file handed to the child as its stdout, which is a redirect and not
    // a pipe; `Stdio::piped()` here would reintroduce exactly that hang.
    let mut child = Command::new(chrome)
        .args(chrome_argv(&dir.join("profile"), &format!("file://{}", html.display())))
        .stdout(Stdio::from(dom_file))
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", chrome.display()))?;

    let mut verdict = Verdict::None;
    for _ in 0..WAIT_SECONDS {
        std::thread::sleep(Duration::from_secs(1));
        let mut dom = String::new();
        if let Ok(mut f) = std::fs::File::open(dom_path) {
            let _ = f.read_to_string(&mut dom);
        }
        verdict = read_verdict(&dom);
        if verdict != Verdict::None {
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();

    match verdict {
        Verdict::Result(lines) => Ok(lines),
        Verdict::Threw(msg) => Err(format!("playtest: the page threw — {msg}")),
        Verdict::None => Err(match can_dump_dom(chrome) {
            // the browser works, so the page is what failed
            Ok(()) => format!(
                "playtest: no verdict after {WAIT_SECONDS}s — the browser never scored a run \
                 (dumped DOM was {} bytes)",
                std::fs::metadata(dom_path).map(|m| m.len()).unwrap_or(0)
            ),
            Err(e) => format!("playtest: {e}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = "<!doctype html><html><head><title>TAS Trainer</title></head>\
<body><canvas></canvas><script>PAGE();</script></body></html>";

    #[test]
    fn the_pump_lands_before_the_pages_own_script() {
        let out = assemble(INDEX, "PUMP();", "DRIVE();").unwrap();
        let pump = out.find("PUMP();").unwrap();
        let page = out.find("PAGE();").unwrap();
        let drive = out.find("DRIVE();").unwrap();
        assert!(pump < page, "the pump must replace rAF before a frame is scheduled");
        assert!(page < drive, "the driver needs the page's DOM to exist");
        assert!(out.find("DRIVE();").unwrap() < out.find("</body>").unwrap());
    }

    #[test]
    fn only_the_first_script_tag_is_spliced() {
        let two = "<body><script>A</script><script>B</script></body>";
        let out = assemble(two, "P", "D").unwrap();
        assert_eq!(out.matches('P').count(), 1);
        assert!(out.contains("<script>P</script><script>A</script>"));
    }

    #[test]
    fn a_page_missing_a_marker_is_named_not_guessed() {
        let e = assemble("<body>no scripts</body>", "P", "D").unwrap_err();
        assert!(e.contains("<script>"), "{e}");
        let e = assemble("<script>x</script>", "P", "D").unwrap_err();
        assert!(e.contains("</body>"), "{e}");
    }

    #[test]
    fn a_result_title_is_one_line_per_player() {
        let dom = "<html><head><title>RESULT on-tape g=S+ acc=100% | 60ms-late g=C acc=72.2%\
</title></head><body></body></html>";
        assert_eq!(
            read_verdict(dom),
            Verdict::Result(vec![
                "RESULT".into(),
                "on-tape g=S+ acc=100%".into(),
                "60ms-late g=C acc=72.2%".into(),
            ])
        );
    }

    #[test]
    fn a_thrown_driver_is_a_failure_with_its_message() {
        assert_eq!(
            read_verdict("<title>ERR sp is null</title>"),
            Verdict::Threw("sp is null".into())
        );
    }

    #[test]
    fn a_page_that_never_scored_is_not_a_pass() {
        // the page's own title, and the driver's boot placeholder: both mean
        // "no verdict", and both used to exit 0
        assert_eq!(read_verdict("<title>TAS Trainer — 6.323</title>"), Verdict::None);
        assert_eq!(read_verdict("<title>D boot</title>"), Verdict::None);
        assert_eq!(read_verdict(""), Verdict::None);
        assert_eq!(read_verdict("<title>unterminated"), Verdict::None);
    }

    #[test]
    fn chrome_is_headless_with_its_own_profile_and_our_clock() {
        let a = chrome_argv(Path::new("/tmp/x/profile"), "file:///tmp/x/pt.html");
        assert!(a.contains(&"--headless=new".to_string()));
        assert!(a.contains(&"--dump-dom".to_string()));
        assert!(a.contains(&"--user-data-dir=/tmp/x/profile".to_string()));
        assert!(a.contains(&"--virtual-time-budget=20000".to_string()));
        assert_eq!(a[a.len() - 1], "file:///tmp/x/pt.html");
    }

    #[test]
    fn a_missing_browser_says_what_was_looked_for() {
        let e = find_chrome(Some("/nonexistent/chrome")).unwrap_err();
        assert!(e.contains("no browser at"), "{e}");
    }
}
