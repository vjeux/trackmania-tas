//! `clip inventory` -- what is published, per map, read off the pages.
//!
//! The re-render sweep needs one list three times over: to agree the scope
//! before filming, to pick each map's treatment while filming, and to rebuild
//! the top-level README afterwards. Reading 37 pages by eye three times is how
//! a map gets skipped, so it is read here instead.
//!
//! THE TREATMENT IS READ, NOT DECIDED. vjeux's rule for the sweep is that a map
//! which already has a video reuses whatever that video did, and a map with no
//! video gets two cars. So this does not estimate anything and has no
//! thresholds: it reports what the page says was done, and where the page does
//! not say, it says UNKNOWN rather than guessing. An UNKNOWN is a page to read,
//! not a default to apply.
//!
//! What a page is expected to look like, and all 31 published ones do:
//!
//! ```text
//! **Kacky Reloaded #290** — TAS **23.416** (−0.646) | AT 24.062 | WR 24.342 by zetos.
//!
//! https://github.com/user-attachments/assets/2e7527fc-...
//!
//! Single car: the 4.492, the tape that matches the author time.
//! ```
//!
//! The caption is the line before the URL; the note is the paragraph after it.

use std::fmt::Write as _;
use std::path::Path;

/// One published video on one page.
#[derive(Debug, Clone)]
pub struct Video {
    pub url: String,
    /// The caption line above it, verbatim, if there was one.
    pub caption: Option<String>,
    pub treatment: Treatment,
}

/// What the scene that produced a clip contained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Treatment {
    /// Our run and the record in one scene -- the project's rule, and the
    /// default for anything not yet filmed.
    TwoCar,
    /// Our run alone. Only ever done where a very slow opponent made the scene
    /// unaffordable, and every page that does it says so.
    SingleCar,
    /// Two runs composed side by side.
    Split,
    /// The page does not say. Read it.
    Unknown,
}

impl Treatment {
    pub fn label(self) -> &'static str {
        match self {
            Treatment::TwoCar => "two-car",
            Treatment::SingleCar => "single-car",
            Treatment::Split => "split",
            Treatment::Unknown => "UNKNOWN",
        }
    }
}

/// One map directory.
#[derive(Debug, Clone)]
pub struct MapPage {
    pub dir: String,
    /// The `# ` heading -- the map's NAME, which is what human-facing text uses.
    pub name: String,
    pub videos: Vec<Video>,
    /// Parsed from the caption of the first video, when it is in the standard
    /// form. `None` on a page whose caption does not parse -- which is a thing
    /// to fix on the page, not to paper over here.
    pub headline: Option<Caption>,
}

/// The standard caption, split into its parts.
#[derive(Debug, Clone)]
pub struct Caption {
    pub map: String,
    pub tas: String,
    pub delta: String,
    pub at: String,
    pub wr: String,
}

/// `**Name** — TAS **t** (d) | AT a | WR w by holder`
///
/// Matched structurally rather than by regex: the em dash and the two pipes are
/// the only fixed points, and every published page has them.
fn parse_caption(line: &str) -> Option<Caption> {
    let l = line.trim();
    if !l.starts_with("**") {
        return None;
    }
    let (map, rest) = l[2..].split_once("**")?;
    let rest = rest.trim_start().strip_prefix('—')?.trim_start();
    let rest = rest.strip_prefix("TAS")?.trim_start();
    let (tas, rest) = rest.strip_prefix("**")?.split_once("**")?;
    let mut bars = rest.split('|');
    let delta = bars.next()?.trim();
    let at = bars.next()?.trim().strip_prefix("AT")?.trim();
    let wr = bars.next()?.trim().strip_prefix("WR")?.trim();
    Some(Caption {
        map: map.to_string(),
        tas: tas.to_string(),
        delta: delta.trim_matches(|c| c == '(' || c == ')').to_string(),
        at: at.to_string(),
        wr: wr.to_string(),
    })
}

/// Which treatment a note describes.
///
/// Deliberately narrow. The phrases here are the ones the pages actually use;
/// anything else reads UNKNOWN, because a wrong guess here re-films a map the
/// wrong way and nobody finds out until the clip is watched.
fn treatment_of(note: &str) -> Treatment {
    let n = note.to_ascii_lowercase();
    // Split first: a split page also says "both", so the more specific claim
    // has to win. "pane" alone is NOT usable -- the overlay is described on
    // nearly every page as "the panel", and matching that turned The Magnet
    // Trial and SAUSAGE ICE into split-view clips they never were.
    for p in [
        "side by side",
        "split view",
        "split-screen",
        "split screen",
        "both panes",
        "two panes",
        "left pane",
        "right pane",
        "our pane",
    ] {
        if n.contains(p) {
            return Treatment::Split;
        }
    }
    for p in ["single car", "single-car", "one car", "one ghost"] {
        if n.contains(p) {
            return Treatment::SingleCar;
        }
    }
    // Two cars in one scene, in the words the pages actually use.
    for p in [
        "opponent",
        "both on screen",
        "both cars",
        "both finishes",
        "both in one camera",
        "in one camera",
        "one scene",
        "same scene",
        "both stay",
        "both start together",
        "ours against",
        "against the world record",
    ] {
        if n.contains(p) {
            return Treatment::TwoCar;
        }
    }
    Treatment::Unknown
}

const ASSET: &str = "https://github.com/user-attachments/assets/";

/// What a clip's own properties say its scene contained.
///
/// This is the measurement behind `--probe`, and it exists because six
/// published pages do not say what they filmed. vjeux's rule for those is to
/// look at the video and reuse the decision it made -- so the video is read,
/// not the prose.
///
/// The three signatures, in the order they are tested:
///
///   * **split** -- a side-by-side composition is about twice as wide as it is
///     tall. Nothing else this project renders is; every single-camera clip is
///     16:9. The bar is 2.0, comfortably clear of 1.78.
///   * **single-car** -- the clip is as long as OUR run. The MediaTracker
///     renders to the longest entity in the scene, so with only our car in it
///     the clip is our time.
///   * **two-car** -- the clip is as long as the SLOWER of the two runs. On a
///     map where the record is slower than us, that is the record's time.
///
/// The tolerances are measured, not chosen. Where the answer is known, a clip
/// lands within about a tenth of a second of the length it should be: 239.100
/// for a 239.133 run, 68.367 against a 68.442 record, 96.300 against 96.281,
/// 15.033 against 15.039, 24.433 against 24.342. The worst was 0.091 s. `NEAR`
/// is 0.5 s -- five times the worst observed miss, so a clip that does not
/// match either candidate is really not matching.
///
/// WHERE THE TWO CANDIDATES ARE THE SAME LENGTH THE PROBE REFUSES. Half these
/// maps are decided by thousandths -- our 6.342 against a 6.346 record -- and
/// no clip length can separate one car from two when both finish at the same
/// instant. `SEP_MIN` is 0.3 s, three times the worst miss: below it, and for
/// any clip that is not nearer to one candidate than to the midpoint between
/// them, the answer is UNKNOWN with the numbers attached. A guess wearing a
/// measurement's clothes is worse than no measurement.
pub fn treatment_from_clip(
    secs: f64,
    width: u32,
    height: u32,
    tas: Option<f64>,
    wr: Option<f64>,
) -> (Treatment, String) {
    let aspect = width as f64 / height.max(1) as f64;
    if aspect >= 2.0 {
        return (
            Treatment::Split,
            format!("{width}x{height} is {aspect:.2}:1 -- a side-by-side composition"),
        );
    }
    const NEAR: f64 = 0.50;
    const SEP_MIN: f64 = 0.30;
    let (Some(tas), Some(wr)) = (tas, wr) else {
        return (
            Treatment::Unknown,
            format!("{secs:.3}s at {width}x{height}, but the page has no TAS/WR pair to compare it to"),
        );
    };
    let slower = tas.max(wr);
    let sep = slower - tas;
    if sep < SEP_MIN {
        return (
            Treatment::Unknown,
            format!(
                "{secs:.3}s, but a one-car scene would be {tas:.3} and a two-car scene {slower:.3} \
                 -- {sep:.3}s apart, which this measurement cannot separate"
            ),
        );
    }
    let d_tas = (secs - tas).abs();
    let d_slower = (secs - slower).abs();
    let (t, d, other) = if d_slower <= d_tas {
        (Treatment::TwoCar, d_slower, slower)
    } else {
        (Treatment::SingleCar, d_tas, tas)
    };
    if d > NEAR || d > sep / 2.0 {
        return (
            Treatment::Unknown,
            format!(
                "{secs:.3}s is {d_tas:.3}s from our {tas:.3} and {d_slower:.3}s from the slower \
                 {slower:.3} -- not close enough to either to call it"
            ),
        );
    }
    let note = match t {
        Treatment::TwoCar => format!(
            "{secs:.3}s is {d:.3}s from the slower run {other:.3} and {d_tas:.3}s from ours \
             {tas:.3} -- the scene ran to the opponent"
        ),
        _ => format!(
            "{secs:.3}s is {d:.3}s from our run {other:.3} and {d_slower:.3}s from the slower \
             {slower:.3} -- the scene ran to our car alone"
        ),
    };
    (t, note)
}

/// Seconds out of a caption field like `24.342 by zetos.` or `4.495 (six tied)`.
fn secs_of(s: &str) -> Option<f64> {
    let t: String = s
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    t.parse().ok()
}

/// Read one page.
pub fn read_page(dir: &Path) -> Result<MapPage, String> {
    let path = dir.join("README.md");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let lines: Vec<&str> = text.lines().collect();

    let name = lines
        .iter()
        .find_map(|l| l.strip_prefix("# "))
        .unwrap_or("(no heading)")
        .trim()
        .to_string();

    let mut videos = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        if !t.starts_with(ASSET) {
            continue;
        }
        // The caption is the nearest non-empty line above.
        let caption = lines[..i]
            .iter()
            .rev()
            .find(|p| !p.trim().is_empty())
            .map(|p| p.trim().to_string());
        // The note is the page's prose about THIS video: everything from the
        // URL up to the next video or the next `##` section. A single paragraph
        // was too little -- The Magnet Trial states "One car, and that is a
        // departure from this project's filming rule" in its SECOND paragraph,
        // and reading only the first called it UNKNOWN.
        let mut note = String::new();
        for l2 in lines[i + 1..].iter() {
            let s = l2.trim();
            if s.starts_with(ASSET) || s.starts_with("## ") || s.starts_with("# ") {
                break;
            }
            note.push_str(s);
            note.push(' ');
        }
        // A caption may itself carry the treatment ("Single car: ...").
        let mut hay = note.clone();
        if let Some(c) = &caption {
            hay.push_str(c);
        }
        videos.push(Video {
            url: t.to_string(),
            treatment: treatment_of(&hay),
            caption,
        });
    }

    let headline = videos
        .first()
        .and_then(|v| v.caption.as_deref())
        .and_then(parse_caption);

    // THE CAPTION'S NAME BEATS THE HEADING. A heading is often the map name
    // plus a subtitle -- "KEKL- SAUSAGE ICE -- a 2620 m ice ribbon, and the
    // author time still stands" -- and human-facing text wants the name.
    let name = headline.as_ref().map(|c| c.map.clone()).unwrap_or(name);

    Ok(MapPage {
        dir: dir.file_name().unwrap_or_default().to_string_lossy().into(),
        name,
        videos,
        headline,
    })
}

/// Every map directory under `root`, in name order.
pub fn read_root(root: &Path) -> Result<Vec<MapPage>, String> {
    let mut dirs: Vec<std::path::PathBuf> = std::fs::read_dir(root)
        .map_err(|e| format!("{}: {e}", root.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.chars().next().is_some_and(|c| c.is_ascii_digit()))
                && p.join("README.md").is_file()
        })
        .collect();
    dirs.sort();
    dirs.iter().map(|d| read_page(d)).collect()
}

/// `clip inventory [--root D] [--tsv] [--probe]`
pub fn main(args: &[String]) -> Result<(), String> {
    let mut root = ".".to_string();
    let mut tsv = false;
    let mut probe = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                root = args.get(i + 1).ok_or("--root needs a directory")?.clone();
                i += 2;
            }
            "--tsv" => {
                tsv = true;
                i += 1;
            }
            "--probe" => {
                probe = true;
                i += 1;
            }
            other => return Err(format!("inventory: unknown flag {other:?}")),
        }
    }

    let pages = read_root(Path::new(&root))?;
    // Only built when asked: the probe needs ffprobe and the open internet, and
    // the plain listing must keep working on a box with neither.
    let ff = if probe { Some(crate::platform::from_env()?) } else { None };
    let cfg = crate::ship::Cfg::from_env();
    let scratch = if probe { Some(crate::proc::scratch_dir("clip-probe")?) } else { None };

    let mut out = String::new();
    let (mut with, mut without, mut unknown) = (0, 0, 0);

    for p in &pages {
        let v = p.videos.first();
        // A PAGE WITH TWO CLIPS OFTEN ANNOTATES ONLY ONE OF THEM. Where the
        // headline clip carries no note, take the first video on the page that
        // does say -- one page's clips have always been the same treatment.
        let mut treatment = p
            .videos
            .iter()
            .map(|v| v.treatment)
            .find(|t| *t != Treatment::Unknown)
            .unwrap_or(Treatment::Unknown);
        let mut why = String::new();

        // THE PROBE ONLY RUNS WHERE THE PAGE IS SILENT. A page that says what
        // it filmed is the better witness -- it was written by whoever filmed
        // it -- and re-measuring it would only invite a disagreement nobody
        // needs to adjudicate.
        if let (true, Some(v), Treatment::Unknown, Some(ff), Some(dir)) =
            (probe, v, treatment, ff.as_ref(), scratch.as_ref())
        {
            let file = dir.join("probe.mp4");
            match crate::ship::gate(&cfg, &v.url, &file, |f| ff.probe_duration(f)) {
                Ok(passed) => match ff.probe_dims(&file) {
                    Ok((w, h)) => {
                        let (t, note) = treatment_from_clip(
                            passed.duration,
                            w,
                            h,
                            p.headline.as_ref().and_then(|c| secs_of(&c.tas)),
                            p.headline.as_ref().and_then(|c| secs_of(&c.wr)),
                        );
                        treatment = t;
                        why = note;
                    }
                    Err(e) => why = format!("probed the length but not the picture: {e}"),
                },
                Err(e) => why = format!("could not fetch it anonymously: {e}"),
            }
            let _ = std::fs::remove_file(&file);
        }

        match v {
            Some(_) => with += 1,
            None => without += 1,
        }
        if v.is_some() && treatment == Treatment::Unknown {
            unknown += 1;
        }
        let (tas, at, wr) = match &p.headline {
            Some(c) => (c.tas.clone(), c.at.clone(), c.wr.clone()),
            None => ("?".into(), "?".into(), "?".into()),
        };
        let plan = if v.is_none() { "two-car (no video)" } else { treatment.label() };
        if tsv {
            let _ = writeln!(
                out,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                p.dir,
                p.name,
                tas,
                at,
                wr,
                p.videos.len(),
                treatment.label(),
                plan,
                why
            );
        } else {
            let _ = writeln!(
                out,
                "{:<40} {:<34} TAS {:>10}  AT {:>10}  WR {:>10}  vids {}  {}",
                p.dir,
                p.name,
                tas,
                at,
                wr,
                p.videos.len(),
                plan
            );
            if !why.is_empty() {
                let _ = writeln!(out, "{:<40}   probe: {why}", "");
            }
        }
    }
    if let Some(d) = scratch {
        let _ = std::fs::remove_dir_all(d);
    }
    print!("{out}");
    if !tsv {
        println!(
            "\n{} maps: {with} with a video, {without} without. {unknown} published page(s) do not \
             say what their scene contained{}.",
            pages.len(),
            if probe { " and could not be measured either" } else { " -- re-run with --probe" }
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every number here was measured off a published clip.
    #[test]
    fn a_split_composition_is_recognised_by_its_shape() {
        let (t, _) = treatment_from_clip(60.0, 2560, 720, Some(10.0), Some(20.0));
        assert_eq!(t, Treatment::Split);
    }

    #[test]
    fn a_clip_that_ran_to_the_opponent_is_two_car() {
        // Kacky Reloaded #290: clip 24.433, ours 23.416, record 24.342.
        let (t, w) = treatment_from_clip(24.433, 1280, 720, Some(23.416), Some(24.342));
        assert_eq!(t, Treatment::TwoCar, "{w}");
        // impossible at for ssano: 15.033 against 14.289 / 15.039.
        let (t, w) = treatment_from_clip(15.033, 1280, 720, Some(14.289), Some(15.039));
        assert_eq!(t, Treatment::TwoCar, "{w}");
    }

    #[test]
    fn a_clip_that_ran_to_our_car_alone_is_single_car() {
        // U10S_32 shape: ours 7.463, record 7.893, filmed single.
        let (t, w) = treatment_from_clip(7.480, 1280, 720, Some(7.463), Some(7.893));
        assert_eq!(t, Treatment::SingleCar, "{w}");
    }

    #[test]
    fn thousandths_apart_is_refused_rather_than_guessed() {
        // unluckE: ours 6.342, record 6.346. No clip length can tell these
        // scenes apart, and answering anyway would be a fabricated result.
        let (t, w) = treatment_from_clip(6.400, 1280, 720, Some(6.342), Some(6.346));
        assert_eq!(t, Treatment::Unknown, "{w}");
    }

    #[test]
    fn a_length_matching_neither_candidate_is_refused() {
        let (t, w) = treatment_from_clip(50.0, 1280, 720, Some(23.416), Some(24.342));
        assert_eq!(t, Treatment::Unknown, "{w}");
    }

    #[test]
    fn a_page_with_no_times_cannot_be_measured() {
        let (t, _) = treatment_from_clip(12.733, 1280, 720, None, None);
        assert_eq!(t, Treatment::Unknown);
    }

    #[test]
    fn caption_parses_into_its_parts() {
        let c = parse_caption(
            "**Kacky Reloaded #290** — TAS **23.416** (−0.646) | AT 24.062 | WR 24.342 by zetos.",
        )
        .expect("should parse");
        assert_eq!(c.map, "Kacky Reloaded #290");
        assert_eq!(secs_of(&c.tas), Some(23.416));
        assert_eq!(secs_of(&c.at), Some(24.062));
        assert_eq!(secs_of(&c.wr), Some(24.342));
    }

    #[test]
    fn the_overlay_panel_is_not_a_split_view() {
        // "the panel is this run's own inputs" appears on nearly every page.
        assert_eq!(treatment_of("The panel is this run's own inputs."), Treatment::Unknown);
    }
}
