//! `tmsite names` — is the title this repo publishes for a map the map's own
//! name?
//!
//! Written 2026-08-25, after vjeux was shown a thumbnail reading
//! `[OBJECT OBJECT] BY TAXONOMON` and asked whether we had work on that map.
//! We did: this repo published it as **"The Magnet Trial"** — a title nobody
//! outside this repo has ever used, assembled here from the skin files the map
//! declares (`magnet-trial-cp-01…16`). The map's own header says
//! `[object Object]`.
//!
//! The question that turned into is not about one map, and it is not a
//! question a human can answer by reading pages: **does this repo
//! systematically publish names that are not the maps' names?** So it is a
//! scan, and it lives next to `tmsite records`, which asks the same shape of
//! question about times.
//!
//! ## Three sources, none of them ours
//!
//! | source | what it gives | how it is read |
//! |---|---|---|
//! | the `.Map.Gbx` header | the name the map DECLARES — ground truth | `tmmaps header --names`, joined in here by uid |
//! | trackmania.io `/api/map/<uid>` | the live service's name and the author's display name | the bank `tmsite refresh` wrote |
//! | trackmania.exchange `/api/maps?id=…` | the TMX upload's name, and the id → uid resolution | the same bank |
//!
//! **A repo document is never a source here.** Our own README agreeing with
//! our own caption is not confirmation of anything, which is the failure this
//! scan exists to make impossible: every column below either comes from a map
//! file or from a public service, and the two are independent of each other.
//!
//! ## An absent map file is not agreement
//!
//! A map with no banked `.Map.Gbx` is `UNVERIFIABLE`, never `match`, even when
//! the live services agree with the page — because the file is the ground
//! truth and we did not read it. It is a separate verdict on purpose: it says
//! *go and get the map*, not *this is fine*.

use crate::json::parse;
use crate::records::{map_dirs, tmx_rows, Tmx};
use gbx::name::plain;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Opts {
    pub root: String,
    pub bank: String,
    /// A `tmmaps header --names` TSV: `path uid name rawname authorid authortime`.
    /// Several rows may share a uid — the corpus keeps working copies of a map
    /// beside the pristine one — and that is checked, not collapsed.
    pub headers: Option<String>,
    pub out: Option<String>,
}

/// What one map's header files say. `paths` is every banked copy that carried
/// this uid.
#[derive(Clone, Debug, Default)]
pub struct Hdr {
    pub name: String,
    pub raw: String,
    pub uid: String,
    pub paths: Vec<String>,
    /// Set when two banked copies of one uid disagree about the name. A
    /// working copy that renamed the map is a finding, not a detail.
    pub conflict: Option<String>,
}

/// Index a `tmmaps header --names` TSV twice: by uid, and by the map id that
/// leads each banked path (`186935/map/…`).
///
/// The second index exists because a uid join alone cannot tell "we never
/// banked this map" apart from "we banked it and its uid is not the one the
/// services list for this id" — and the second is a finding, while the first
/// is only a gap. 126859 is the case: the file we hold declares
/// `Z4p7Gy3gjXINzu8pgm_WzYYjtmg` and TMX lists something else entirely.
pub struct Headers {
    pub by_uid: BTreeMap<String, Hdr>,
    pub by_id: BTreeMap<i64, Hdr>,
}

pub fn read_headers(path: &str) -> Result<Headers, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {}: {}", path, e))?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let col = |n: &str| head.iter().position(|h| *h == n);
    let (Some(cp), Some(cu), Some(cn), Some(cr)) =
        (col("path"), col("uid"), col("name"), col("rawname"))
    else {
        return Err(format!("{}: needs columns path, uid, name, rawname", path));
    };
    let mut by_uid: BTreeMap<String, Hdr> = BTreeMap::new();
    let mut by_id: BTreeMap<i64, Hdr> = BTreeMap::new();
    for l in lines {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() <= cp.max(cu).max(cn).max(cr) {
            continue;
        }
        if c[cu] == "ERROR" {
            continue;
        }
        let mut add = |e: &mut Hdr| {
            if e.paths.is_empty() {
                e.name = c[cn].to_string();
                e.raw = c[cr].to_string();
                e.uid = c[cu].to_string();
            } else if e.name != c[cn] {
                e.conflict = Some(format!("{} says {:?}", c[cp], c[cn]));
            }
            e.paths.push(c[cp].to_string());
        };
        add(by_uid.entry(c[cu].to_string()).or_default());
        if let Some(id) = c[cp].split('/').next().and_then(|s| s.parse::<i64>().ok()) {
            add(by_id.entry(id).or_default());
        }
    }
    Ok(Headers { by_uid, by_id })
}

/// trackmania.io's name for a map, and the author's display name.
///
/// The service hands the name over with its ManiaPlanet markup intact —
/// `$903Welcome☺$903to $903wiggles` — so it is decoded here through the same
/// `gbx::name` decoder the map file goes through. Comparing a decoded header
/// name against an undecoded live name would report a difference on every
/// decorated map and none of them would be real.
pub fn tmio_name(text: &str) -> Option<(String, String)> {
    let v = parse(text).ok()?;
    Some((
        plain(v.get("name").and_then(|x| x.as_str())?),
        plain(
            v.get("authorplayer")
                .and_then(|a| a.get("name"))
                .and_then(|x| x.as_str())
                .unwrap_or(""),
        ),
    ))
}

// ---------------------------------------------------------------------------
// what this repo publishes
// ---------------------------------------------------------------------------

/// The link text the root README uses for a directory.
///
/// The rows are `**[<name>](<dir>)** — …`, and `<name>` may itself contain
/// brackets (`[Turtle Trial] Angustus`), so this anchors on the link TARGET and
/// walks back to the `**[` that opens it rather than matching brackets
/// forwards, which would stop at the wrong one.
pub fn root_link_text(readme: &str, dir: &str) -> Option<String> {
    let needle = format!("]({})", dir);
    let at = readme.find(&needle)?;
    let open = readme[..at].rfind("**[")?;
    // The opener must be on the same line as the link, or we have walked back
    // past this row into an earlier one.
    if readme[open..at].contains('\n') {
        return None;
    }
    Some(readme[open + 3..at].to_string())
}

/// The `# ` heading of a map's own page, with any trailing gloss removed:
/// `# KEKL- SAUSAGE ICE — a 2620 m ice ribbon…` is a title plus an editorial
/// subtitle, and the title is the part before the em dash.
pub fn page_title(page: &str) -> Option<String> {
    let l = page.lines().find(|l| l.starts_with("# "))?;
    let t = l[2..].trim();
    Some(t.split(" — ").next().unwrap_or(t).trim().to_string())
}

/// The bolded map name that opens the fixed caption line:
/// `**<Map>** — TAS **22.072** (−1.253) | AT 23.325 | WR 23.298 by Lukrecja666`
pub fn caption_name(page: &str) -> Option<String> {
    for l in page.lines() {
        if !(l.contains("— TAS ") && l.contains("| AT ")) {
            continue;
        }
        let l = l.trim();
        if !l.starts_with("**") {
            continue;
        }
        let end = l[2..].find("**")? + 2;
        return Some(l[2..end].to_string());
    }
    None
}

// ---------------------------------------------------------------------------
// the join
// ---------------------------------------------------------------------------

pub struct Row {
    pub tmx_id: i64,
    pub dir: String,
    pub published: String,
    pub page_title: String,
    pub caption: String,
    pub uid: String,
    pub header: Option<Hdr>,
    pub tmio: Option<String>,
    pub author: String,
    pub tmx: Option<Tmx>,
    pub verdict: String,
    pub notes: Vec<String>,
}

/// Compare two titles the way a reader would: a difference of case or of
/// surrounding whitespace is not a different NAME, and reporting it as one
/// would bury the real findings under noise. Everything else counts —
/// including a run of spaces INSIDE the name, which is reported separately
/// rather than normalised away, because a doubled space is a real property of
/// the map's title and dropping it silently is the same class of mistake this
/// whole scan is about.
fn same_title(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// The two differ only in how many spaces sit between the same words.
fn same_but_spacing(a: &str, b: &str) -> bool {
    let squash = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    !same_title(a, b) && squash(a) == squash(b)
}

pub fn rows(o: &Opts) -> Result<Vec<Row>, String> {
    let maps = map_dirs(&o.root)?;
    let root_readme = std::fs::read_to_string(Path::new(&o.root).join("README.md"))
        .map_err(|e| format!("read root README.md: {}", e))?;

    let headers = match &o.headers {
        Some(p) => read_headers(p)?,
        None => Headers { by_uid: BTreeMap::new(), by_id: BTreeMap::new() },
    };

    let mut tmx: BTreeMap<i64, Tmx> = BTreeMap::new();
    let rd = std::fs::read_dir(&o.bank).map_err(|e| format!("read {}: {}", o.bank, e))?;
    let mut batches: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().map(|n| n.to_string_lossy().starts_with("tmx_")).unwrap_or(false))
        .collect();
    batches.sort();
    for b in &batches {
        if let Ok(text) = std::fs::read_to_string(b) {
            for (id, mut t) in tmx_rows(&text) {
                t.name = plain(&t.name);
                tmx.insert(id, t);
            }
        }
    }

    let mut out = Vec::new();
    for m in &maps {
        let page =
            std::fs::read_to_string(Path::new(&o.root).join(&m.dir).join("README.md")).unwrap_or_default();
        let published = root_link_text(&root_readme, &m.dir).unwrap_or_default();
        let title = page_title(&page).unwrap_or_default();
        let caption = caption_name(&page).unwrap_or_default();

        let t = tmx.get(&m.tmx_id).cloned();
        let uid = t.as_ref().map(|x| x.uid.clone()).unwrap_or_default();
        let io = std::fs::read_to_string(Path::new(&o.bank).join(format!("tmio_map/{}.json", m.tmx_id)))
            .ok()
            .and_then(|s| tmio_name(&s));

        let mut notes: Vec<String> = Vec::new();

        // Join on uid; fall back to the map id the banked path carries, and
        // say so — a uid that does not match is a different statement from a
        // map we never banked.
        let hdr = match headers.by_uid.get(&uid) {
            Some(h) => Some(h.clone()),
            None => match headers.by_id.get(&m.tmx_id) {
                Some(h) => {
                    notes.push(format!(
                        "UID MISMATCH: the banked map declares `{}`, the services list `{}` for this \
                         id — the file is a different upload of the map, so its name is evidence \
                         about that upload and not necessarily about the one the board tracks",
                        h.uid, uid
                    ));
                    Some(h.clone())
                }
                None => None,
            },
        };

        if let Some(h) = &hdr {
            if let Some(c) = &h.conflict {
                notes.push(format!("banked copies of this map disagree: {}", c));
            }
            if h.raw != h.name {
                notes.push(format!("header markup: {:?}", h.raw));
            }
            // The three places a name is published are checked SEPARATELY.
            // A repo can have the front page right and the map's own page
            // wrong — 284238 is `YOU LOVE WATER` in the index and
            // `You love water` on its own page — and a scan that only read one
            // of them would call that clean.
            for (what, got) in [
                ("the root README index", &published),
                ("the page's `# ` title", &title),
                ("the caption line", &caption),
            ] {
                if got.is_empty() || got.trim() == h.name.trim() {
                    continue;
                }
                if same_title(got, &h.name) {
                    notes.push(format!(
                        "{} says {:?} — the same name in a different CASE",
                        what, got
                    ));
                    continue;
                }
                if what != "the root README index" && same_title(got, &published) {
                    // Same wrongness as the index: already reported by the
                    // verdict, no need to say it twice.
                    continue;
                }
                if what == "the root README index" {
                    continue; // the verdict already covers it
                }
                notes.push(format!("{} says {:?}, a third spelling", what, got));
            }
        }

        // The verdict is about the NAME the repo publishes, against the
        // sources, in the stated order of authority: the file, then the live
        // service.
        let verdict = match (&hdr, &io) {
            (None, _) => {
                notes.push("no banked .Map.Gbx for this map".into());
                "UNVERIFIABLE".to_string()
            }
            (Some(h), io) => {
                if let Some((ion, _)) = io {
                    if !same_title(&h.name, ion) {
                        notes.push(format!(
                            "SOURCES DISAGREE: header {:?} vs trackmania.io {:?}",
                            h.name, ion
                        ));
                    }
                }
                if same_title(&published, &h.name) {
                    "match".to_string()
                } else if same_but_spacing(&published, &h.name) {
                    notes.push(format!(
                        "spacing only: the map's own name is {:?}",
                        h.name
                    ));
                    "match (spacing differs)".to_string()
                } else if io.as_ref().map(|(n, _)| same_title(&published, n)).unwrap_or(false) {
                    // The page matches the live service but not the file. That
                    // is not a pass: the file is the authority, and the two
                    // differing at all is the finding.
                    "wrong (matches trackmania.io, not the file)".to_string()
                } else if t.as_ref().map(|x| same_title(&published, &x.name)).unwrap_or(false) {
                    "wrong (matches the TMX upload's name, not the map's)".to_string()
                } else if !published.trim().is_empty()
                    && h.name.to_lowercase().contains(&published.trim().to_lowercase())
                {
                    "wrong (a fragment of the real name)".to_string()
                } else if !published.trim().is_empty()
                    && published
                        .split_whitespace()
                        .all(|w| h.name.to_lowercase().contains(&w.to_lowercase()))
                {
                    "wrong (real name, with words dropped)".to_string()
                } else {
                    "INVENTED".to_string()
                }
            }
        };

        out.push(Row {
            tmx_id: m.tmx_id,
            dir: m.dir.clone(),
            published,
            page_title: title,
            caption,
            uid,
            header: hdr,
            tmio: io.as_ref().map(|(n, _)| n.clone()),
            author: io.map(|(_, a)| a).unwrap_or_default(),
            tmx: t,
            verdict,
            notes,
        });
    }
    Ok(out)
}

fn cell(s: &str) -> String {
    if s.is_empty() {
        "—".to_string()
    } else {
        s.replace('|', "\\|")
    }
}

pub fn run(o: &Opts) -> Result<String, String> {
    let rows = rows(o)?;
    let mut md = String::new();
    md.push_str("| map | directory | published as | header name | trackmania.io | author | verdict |\n");
    md.push_str("|---|---|---|---|---|---|---|\n");
    for r in &rows {
        md.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} | {} |\n",
            r.tmx_id,
            r.dir,
            cell(&r.published),
            cell(r.header.as_ref().map(|h| h.name.as_str()).unwrap_or("")),
            cell(r.tmio.as_deref().unwrap_or("")),
            cell(&r.author),
            r.verdict,
        ));
    }
    let bad = rows.iter().filter(|r| !r.verdict.starts_with("match")).count();
    md.push_str(&format!(
        "\n{} maps, {} not a clean match.\n",
        rows.len(),
        bad
    ));
    for r in &rows {
        if r.notes.is_empty() && r.verdict.starts_with("match") {
            continue;
        }
        md.push_str(&format!("\n### {} `{}` — {}\n", r.tmx_id, r.dir, r.verdict));
        md.push_str(&format!("- uid `{}`\n", r.uid));
        md.push_str(&format!("- root README says: {}\n", cell(&r.published)));
        md.push_str(&format!("- page `# ` title: {}\n", cell(&r.page_title)));
        md.push_str(&format!("- caption says: {}\n", cell(&r.caption)));
        md.push_str(&format!(
            "- header: {}\n",
            cell(r.header.as_ref().map(|h| h.name.as_str()).unwrap_or(""))
        ));
        md.push_str(&format!("- trackmania.io: {}\n", cell(r.tmio.as_deref().unwrap_or(""))));
        md.push_str(&format!(
            "- trackmania.exchange: {}\n",
            cell(r.tmx.as_ref().map(|t| t.name.as_str()).unwrap_or(""))
        ));
        if let Some(h) = &r.header {
            md.push_str(&format!("- read from: {}\n", h.paths.join(", ")));
        }
        for n in &r.notes {
            md.push_str(&format!("- {}\n", n));
        }
    }
    if let Some(p) = &o.out {
        std::fs::write(p, &md).map_err(|e| format!("write {}: {}", p, e))?;
        return Ok(format!("{} maps written to {} ({} not a clean match)", rows.len(), p, bad));
    }
    Ok(md)
}
