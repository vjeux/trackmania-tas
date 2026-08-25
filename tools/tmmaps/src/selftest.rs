//! `tmmaps selftest` — every control this tool owns, in one command.
//!
//! Three tiers, the same shape `tools/ghost` uses:
//!
//! * **PURE** — no server, no network. Container round trip, the census, the
//!   region/clear arithmetic, the movers' refusals, the origin control.
//! * **ORACLE** — the real dedicated server on the checked-in fixtures.
//!   Needs `TM_SERVER` or `--server DIR`; skipped without one.
//! * **ENGINE** — reserved; nothing here needs the engine yet.
//!
//! `--strict` makes a SKIP a failure. That flag exists because a suite whose
//! fixtures are missing prints seven green lines and proves nothing: the
//! previous version of this suite returned early from every oracle test when
//! `/tmp/m1` was absent, and reported `7 passed`.

use crate::census::{self, Box3};
use crate::map::MapFile;
use crate::oracle;
use crate::secs;
use std::path::{Path, PathBuf};

pub struct Suite {
    pub pass: usize,
    pub fail: usize,
    pub skip: usize,
    strict: bool,
}

impl Suite {
    fn new(strict: bool) -> Suite {
        Suite { pass: 0, fail: 0, skip: 0, strict }
    }
    fn ok(&mut self, tier: &str, name: &str, detail: &str) {
        self.pass += 1;
        println!("  PASS  [{}] {:<38} {}", tier, name, detail);
    }
    fn bad(&mut self, tier: &str, name: &str, detail: &str) {
        self.fail += 1;
        println!("  FAIL  [{}] {:<38} {}", tier, name, detail);
    }
    fn check(&mut self, tier: &str, name: &str, cond: bool, detail: &str) {
        if cond {
            self.ok(tier, name, detail)
        } else {
            self.bad(tier, name, detail)
        }
    }
    fn skipped(&mut self, tier: &str, name: &str, why: &str) {
        if self.strict {
            self.fail += 1;
            println!("  FAIL  [{}] {:<38} SKIP under --strict: {}", tier, name, why);
        } else {
            self.skip += 1;
            println!("  SKIP  [{}] {:<38} {}", tier, name, why);
        }
    }
}

/// Where the fixtures live: next to the crate, so the suite works from a fresh
/// clone with nothing restored.
fn testdata() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

/// `tools/testdata` — the corpus shared by every crate's tests. A fixture that
/// more than one tool checks itself against lives there rather than being
/// copied per crate: two copies of a fixture drift, and a drifted fixture is
/// how a suite goes green against a format that no longer exists.
fn shared_testdata() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../testdata")
}

fn fixture(name: &str) -> Option<PathBuf> {
    for p in [testdata().join(name), shared_testdata().join(name)] {
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// A scratch path unique to this process.
///
/// Nodes are shared and this suite runs concurrently with itself (`cargo test`
/// runs its two tests in parallel). Two runs sharing a staging root swap each
/// other's candidate files and manufacture results — measured elsewhere in
/// this project as 7 phantom finishes in 13 shared runs against 0 in 8 with
/// distinct roots. So: own root, always.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tmmaps-selftest-{}", std::process::id()));
    std::fs::create_dir_all(&d).ok();
    d.join(name)
}

pub fn run(args: &[String]) -> ! {
    let strict = crate::cli::has(args, "--strict");
    let mut s = Suite::new(strict);
    println!("tmmaps selftest — fixtures in {}", testdata().display());
    pure(&mut s);
    oracle_tier(&mut s, args);
    println!(
        "\n{} passed, {} failed, {} skipped",
        s.pass, s.fail, s.skip
    );
    let _ = std::fs::remove_dir_all(scratch(""));
    std::process::exit(if s.fail > 0 { 1 } else { 0 });
}

// ------------------------------------------------------------------ PURE

fn pure(s: &mut Suite) {
    splice_checks(s);
    name_checks(s);
    // ---- container round trip, on every fixture map
    for name in ["map1.Map.Gbx", "map2.Map.Gbx", "goth.Map.Gbx"] {
        let Some(p) = fixture(name) else {
            s.skipped("PURE", &format!("roundtrip {}", name), "fixture missing");
            continue;
        };
        let m = MapFile::load(&p);
        let rebuilt = crate::gbx::Gbx::parse(&m.build());
        s.check(
            "PURE",
            &format!("roundtrip {}", name),
            rebuilt.body == m.gbx.body && rebuilt.num_nodes == m.gbx.num_nodes,
            &format!("body {} bytes, numNodes {}", m.gbx.body.len(), m.gbx.num_nodes),
        );
    }

    // ---- the origin control: every mover, at its own current placement,
    //      must reproduce the file byte for byte
    for name in ["map1.Map.Gbx", "map2.Map.Gbx", "goth.Map.Gbx"] {
        let Some(p) = fixture(name) else {
            s.skipped("PURE", &format!("origin {}", name), "fixture missing");
            continue;
        };
        let r = crate::controls::origin(&p, false);
        s.check(
            "PURE",
            &format!("origin {}", name),
            r.failures == 0,
            &format!("{} movers exercised, {} failures", r.checked, r.failures),
        );
    }

    // ---- the census sees BOTH block chunks
    if let Some(p) = fixture("goth.Map.Gbx") {
        let m = MapFile::load(&p);
        s.check(
            "PURE",
            "census sees the baked chunk",
            !m.baked.is_empty(),
            &format!("{} unbaked + {} baked + {} items", m.blocks.len(), m.baked.len(), m.items.len()),
        );
        // Positive control for the same claim: a listing that reads only
        // 0x0304301F would report the smaller number, so state both.
        let free = m.blocks.iter().chain(m.baked.iter()).filter(|b| b.free_pos.is_some()).count();
        s.check(
            "PURE",
            "free positions parsed for both chunks",
            free > 0,
            &format!("{} blocks carry six f32 in 0x0304305F", free),
        );
    } else {
        s.skipped("PURE", "census sees the baked chunk", "goth.Map.Gbx missing");
        s.skipped("PURE", "free positions parsed for both chunks", "goth.Map.Gbx missing");
    }

    // ---- A GATE IS A STRUCTURE: the 173691 case, at the byte level.
    //
    // GothMommy's added finish is one unbaked anchor plus fifteen baked
    // pieces. This is the regression test for the pass that moved the anchor
    // and reported success.
    if let Some(p) = fixture("goth.Map.Gbx") {
        let m = MapFile::load(&p);
        let b = Box3::parse(GATE_BOX);
        let found = census::in_box(&m, b, &Some("GateExpandable".to_string()));
        let baked = found.iter().filter(|e| e.baked).count();
        s.check(
            "PURE",
            "the added gate is a STRUCTURE",
            found.len() == 16 && baked == 12,
            &format!(
                "{} pieces in the landing box, {} baked / {} unbaked — moving one anchor leaves {}",
                found.len(),
                baked,
                found.len() - baked,
                found.len() - 1
            ),
        );
        // The box a human types is a hypothesis. Ask the whole map the same
        // question and require the same answer: if the box is wrong the two
        // disagree, which is the only way a region test can catch its own
        // bounds being too small.
        let everywhere = census::entries(&m)
            .into_iter()
            .filter(|e| e.name.contains("GateExpandable"))
            .count();
        s.check(
            "PURE",
            "the box holds the WHOLE structure",
            everywhere == found.len(),
            &format!("{} in the box vs {} on the whole map", found.len(), everywhere),
        );

        // NEGATIVE CONTROL, and it is the one that matters: move only the
        // unbaked anchor, re-read, and require the box to be NOT empty. If
        // this ever passes trivially the region query has gone blind and the
        // positive test above would still be green.
        let anchor = found.iter().find(|e| !e.baked && !e.item).cloned();
        match anchor {
            None => s.bad("PURE", "anchor-only move leaves the pieces", "no unbaked anchor found"),
            Some(a) => {
                let mut m1 = MapFile::load(&p);
                let idx: usize = a.id.parse().unwrap();
                m1.move_block_free(idx, AWAY);
                let tmp = scratch("anchor.Map.Gbx");
                m1.write_to(&tmp).unwrap();
                let after = MapFile::load(&tmp);
                let left = census::in_box(&after, b, &Some("GateExpandable".to_string()));
                s.check(
                    "PURE",
                    "anchor-only move leaves the pieces",
                    left.len() == found.len() - 1 && !left.is_empty(),
                    &format!("{} of {} still standing after moving the anchor", left.len(), found.len()),
                );
                let _ = std::fs::remove_file(&tmp);
            }
        }

        // POSITIVE CONTROL: move all of them and require zero left, read back
        // out of the written file.
        let mut m2 = MapFile::load(&p);
        let mut movable = 0usize;
        for e in &found {
            if e.placement != census::Placement::Free {
                continue;
            }
            let i: usize = e.id.trim_start_matches(['b', 'i']).parse().unwrap();
            if e.item {
                m2.move_item_pos(i, AWAY);
            } else if e.baked {
                m2.move_baked_free(i, AWAY);
            } else {
                m2.move_block_free(i, AWAY);
            }
            movable += 1;
        }
        let tmp = scratch("clear.Map.Gbx");
        m2.write_to(&tmp).unwrap();
        let after = MapFile::load(&tmp);
        let left = census::in_box(&after, b, &Some("GateExpandable".to_string()));
        s.check(
            "PURE",
            "clearing the region empties it",
            left.is_empty() && movable == found.len(),
            &format!("moved {} of {}, {} left in the written file", movable, found.len(), left.len()),
        );
        // ...and the map still round-trips after that surgery.
        s.check(
            "PURE",
            "cleared map still round-trips",
            crate::gbx::Gbx::parse(&after.build()).body == after.gbx.body,
            "parse + rebuild of the edited map is byte-identical",
        );
        let _ = std::fs::remove_file(&tmp);
    } else {
        for n in [
            "the added gate is a STRUCTURE",
            "the box holds the WHOLE structure",
            "anchor-only move leaves the pieces",
            "clearing the region empties it",
            "cleared map still round-trips",
        ] {
            s.skipped("PURE", n, "goth.Map.Gbx missing");
        }
    }

    // ---- refusals
    //
    // Both of these EXPECT a panic, so the panic message is not news. Silence
    // the default hook for the duration; a refusal test that prints a
    // backtrace teaches the reader to ignore backtraces.
    if let Some(p) = fixture("goth.Map.Gbx") {
        let m = MapFile::load(&p);
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        // a baked GRID block has no position to write
        let grid_baked = m.baked.iter().find(|b| b.free_off.is_none()).map(|b| b.index);
        match grid_baked {
            None => s.skipped("PURE", "baked grid block is refused", "this map has no baked grid block"),
            Some(i) => {
                let r = std::panic::catch_unwind(move || {
                    let mut m = MapFile::load(&fixture("goth.Map.Gbx").unwrap());
                    m.move_baked_free(i, AWAY);
                });
                s.check("PURE", "baked grid block is refused", r.is_err(), "move_baked_free panics");
            }
        }
        // a free block refuses a cell move (the silent-rung case)
        let free = m.blocks.iter().find(|b| b.free_off.is_some()).map(|b| b.index);
        match free {
            None => s.skipped("PURE", "free block refuses a cell move", "this map has no free block"),
            Some(i) => {
                let r = std::panic::catch_unwind(move || {
                    let mut m = MapFile::load(&fixture("goth.Map.Gbx").unwrap());
                    m.move_block_cell(i, (10, 10, 10));
                    m.build();
                });
                // move_block_cell itself writes dead bytes; the refusal lives
                // in the CLI. Record what is actually true rather than
                // pretending the library refuses.
                s.check(
                    "PURE",
                    "free block: cell bytes are dead",
                    r.is_ok(),
                    "the library writes them and the CLI refuses — see `move`",
                );
            }
        }
        std::panic::set_hook(hook);
    }

    // ---- tmmaps refuses a container that is not a map.
    //
    // A replay carries a whole map, so a chunk walk over one finds this map's
    // blocks and every offset below is inside a nested container. The ghost
    // arm found the sharp end: a carried map's own chunk declares a size
    // running past the map's end, so a walk that "corrects" it writes four
    // bytes into the middle of a map and the file then validates to nothing.
    // The boundary is enforced rather than remembered.
    if let Some(g) = fixture("map1_wr_19538.Ghost.Gbx") {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(|| {
            MapFile::load(&fixture("map1_wr_19538.Ghost.Gbx").unwrap());
        });
        std::panic::set_hook(hook);
        s.check(
            "PURE",
            "a recording is refused, not parsed",
            r.is_err(),
            &format!("{} is GBX class 0x03092000, not 0x03043000", fname(&g)),
        );
        // positive control: the same loader on a real map succeeds, so the
        // refusal is about the class and not about the loader being broken.
        s.check(
            "PURE",
            "...and a map still loads",
            std::panic::catch_unwind(|| {
                MapFile::load(&fixture("map1.Map.Gbx").unwrap()).blocks.len()
            })
            .map(|n| n > 0)
            .unwrap_or(false),
            "map1.Map.Gbx parses through the same entry point",
        );
    }

    // ---- the oracle's own output parser, on ASYMMETRIC fixtures
    parser_checks(s);

    // ---- time formatting
    s.check(
        "PURE",
        "times print as seconds",
        secs::ms(16316) == "16.316" && secs::opt(None) == "DNF" && secs::ms(-101) == "-0.101",
        "16.316 / DNF / -0.101",
    );
}

/// The landing zone on 173691.
///
/// The banked write-up said the added gate was "an unbaked anchor at
/// (1311, 113, 434) and **fifteen baked pieces** spanning x 1271…1375,
/// **y 96…121**, z 412…466". The census says **four** unbaked pieces and
/// **twelve** baked — still sixteen in total, so the headline stands — and the
/// real vertical extent reaches **y 64**, because three `…RightVFC` pieces sit
/// at (1271, 64, 41x). A box typed off the banked y range misses those three.
///
/// That is the whole reason this constant is a comment as well as a number:
/// the region a human types is a hypothesis, and `region` is what checks it.
const GATE_BOX: &str = "1260,55,405:1385,130,475";
/// Somewhere no run reaches. Far enough that a piece "moved" to here cannot
/// still satisfy a region query by rounding.
const AWAY: [f32; 3] = [-3000.0, -3000.0, -3000.0];

/// `parse_output` against a REAL server transcript, on rows where a WRONG
/// parser gives a DIFFERENT answer.
///
/// Two design points, and both were paid for.
///
/// **The transcript is the server's own bytes**, captured from a run, not
/// hand-written. A parser tested against a format a human typed is tested
/// against a format that does not exist — the real one puts spaces around its
/// colons, ends `Desc` with a `\n`, and puts `Desc` in a different position on
/// each of the two rows.
///
/// **Both rows are asymmetric.** On an ordinary passing file `ValidatedResult`
/// and `DeclaredResult` are *equal*, so a fixture built from one cannot fail:
/// a parser that stops at the first `"Time"` and one that runs to the last are
/// indistinguishable. These two rows were produced on purpose.
///
/// * row 1 — a tape edited until it no longer finishes. `ValidatedResult` is
///   `null` and the declaration still says 19.538 with four checkpoints. A
///   parser that reads the first `"Time"` it can find reports **19.538 for a
///   run that did not finish**; one that scans forward from
///   `"ValidatedResult"` for `"NbCheckpoints"` reports **four checkpoints for
///   a run that reached none**. Both are live bugs elsewhere in this project.
/// * row 2 — a file that finishes at 19.538 while declaring 30.000. A parser
///   that keeps reading to the end of the block reports **30.000**: the file's
///   own claim, confirmed back to itself.
/// `tmmaps header --names` — the map's DECLARED identity.
///
/// Added by the 2026-08-25 name audit, which found this repo publishing a
/// title for 186935 that appears in no file the game ships: `header::read`
/// pulled the name off `<desc>`, where there is none, so every map printed
/// `name -` and nothing was ever compared against it.
fn name_checks(s: &mut Suite) {
    // Every case here is a real string out of a map in this corpus, plus the
    // structural escapes the renderer defines.
    for (raw, want, what) in [
        (
            "$o$i$aa0Kack$05ay Re$09alo$6a0ad$aa0ed $4f0#290",
            "Kacky Reloaded #290",
            "126859: styles and RGB colours",
        ),
        ("[object Object]", "[object Object]", "186935: nothing to strip"),
        ("KEKL- SAUSAGE ICE", "KEKL- SAUSAGE ICE", "134672: plain text"),
        ("$fffa$0f0b", "ab", "colours between every letter"),
        ("100$$", "100$", "`$$` is a literal dollar"),
        ("$h[www]click$h", "click", "a link keeps its text, not its target"),
        ("$zplain", "plain", "a reset"),
        ("cost $5", "cost $5", "`$5` is not a colour: three hex digits are needed"),
    ] {
        s.check(
            "PURE",
            &format!("name markup stripped — {what}"),
            crate::header::strip_fmt(raw) == want,
            &format!("{raw:?} -> {:?}, want {want:?}", crate::header::strip_fmt(raw)),
        );
    }

    // THE CONTROL. The three checks above say the stripper works. They do not
    // say this audit could have caught the bug it was written for — the bug
    // was not in the stripper, it was in WHICH ATTRIBUTE was read, and a
    // stripper test cannot see that. So assert the shape of the real header
    // XML directly: the name is on `<ident>`, and `<desc>` has none. If a
    // future refactor reads `desc` again, this fails.
    let xml = "<header type=\"map\" title=\"TMStadium\">\
               <ident uid=\"sOIkPZULktmoT_OoFbT4HlVxpOe\" name=\"[object Object]\" \
               author=\"XL0y4ZpuQfqC-1opr5LKwg\"/>\
               <desc envir=\"Stadium\" mood=\"Night (no stadium)\" validated=\"1\"/>\
               </header>";
    s.check(
        "PURE",
        "the map name is read off <ident>",
        crate::header::attr_pub(xml, "ident", "name").as_deref() == Some("[object Object]"),
        "186935's own header, verbatim",
    );
    s.check(
        "PURE",
        "...and <desc> carries no name at all",
        crate::header::attr_pub(xml, "desc", "name").is_none(),
        "the wrong read this audit fixed: it returned nothing, and printed as `-` for every map \
         in the corpus — which is why a name nobody could see was never checked",
    );
}

fn parser_checks(s: &mut Suite) {
    let Some(p) = fixture("oracle_transcript.json") else {
        for n in [
            "oracle: a DNF is a DNF, not its declaration",
            "oracle: the VALIDATED result wins",
            "oracle: both rows were attributed",
        ] {
            s.skipped("PURE", n, "oracle_transcript.json missing");
        }
        return;
    };
    let text = std::fs::read_to_string(&p).expect("read transcript");
    let rows = oracle::parse_output(&text);
    let by = |name: &str| rows.iter().find(|r| r.file == name).cloned();

    let dnf = by("edited.Ghost.Gbx");
    s.check(
        "PURE",
        "oracle: a DNF is a DNF, not its declaration",
        matches!(&dnf, Some(r) if r.sim_time.is_none() && r.declared_time == Some(19538)),
        &format!(
            "validated {} while the file declares {}",
            secs::opt(dnf.as_ref().and_then(|r| r.sim_time)),
            secs::opt(dnf.as_ref().and_then(|r| r.declared_time))
        ),
    );

    let fin = by("stale_decl.Ghost.Gbx");
    s.check(
        "PURE",
        "oracle: the VALIDATED result wins",
        matches!(&fin, Some(r) if r.sim_time == Some(19538) && r.declared_time == Some(30000)),
        &format!(
            "validated {} while the file declares {}",
            secs::opt(fin.as_ref().and_then(|r| r.sim_time)),
            secs::opt(fin.as_ref().and_then(|r| r.declared_time))
        ),
    );

    s.check(
        "PURE",
        "oracle: both rows were attributed",
        rows.len() == 2 && dnf.is_some() && fin.is_some(),
        &format!("{} rows parsed, named {:?}", rows.len(), rows.iter().map(|r| &r.file).collect::<Vec<_>>()),
    );

    // THE FIXTURE'S OWN POSITIVE CONTROL.
    //
    // The three rows above say the parser is right. They do not say the
    // transcript could ever have caught it being wrong — and that is the
    // failure mode this whole section exists to avoid, because on a passing
    // file the two results are equal and any fixture built from one is
    // vacuous. So run the wrong parser, here, on the same bytes, and require
    // it to produce the wrong answer.
    let naive = naive_last_time(&text);
    s.check(
        "PURE",
        "...and the wrong parser gets it wrong",
        naive == vec![Some(19538), Some(30000)],
        &format!(
            "a last-`Time`-wins parser reads {:?} — {} for a run that did not finish, and {} for \
             one that did 19.538. If this ever matches the right answers, the transcript has \
             stopped being a test.",
            naive.iter().map(|v| secs::opt(*v)).collect::<Vec<_>>(),
            secs::opt(naive[0]),
            secs::opt(naive[1])
        ),
    );

    // A candidate the server never read. `stage` refuses the filename rather
    // than letting it come back as an ordinary DNF.
    let bad = PathBuf::from("/tmp/best_23074");
    s.check(
        "PURE",
        "a file the server would skip is refused",
        oracle::readable_name(&bad).is_err(),
        "the server ignores anything without .Ghost.Gbx / .Replay.Gbx and returns a plain DNF",
    );
    s.check(
        "PURE",
        "a well-named file is accepted",
        oracle::readable_name(Path::new("/tmp/best_23074.Ghost.Gbx")).is_ok(),
        "positive control for the check above",
    );
}

/// The bug, implemented on purpose: read `"Time"` lines and keep the last one
/// before each `"FileName"`.
///
/// This is not a parser anybody would defend; it is what several parsers in
/// this project do by accident, and it is here so the transcript above can be
/// shown to catch it. Never call it for a result.
fn naive_last_time(text: &str) -> Vec<Option<i64>> {
    let mut out = Vec::new();
    let mut cur: Option<i64> = None;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("\"Time\"") {
            cur = t
                .split(':')
                .nth(1)
                .and_then(|s| s.trim().trim_end_matches(',').parse::<i64>().ok());
        } else if t.starts_with("\"FileName\"") {
            out.push(cur);
            cur = None;
        }
    }
    out
}

// ---------------------------------------------------------------- ORACLE

fn oracle_tier(s: &mut Suite, args: &[String]) {
    let server = crate::cli::server_of(args);
    if !Path::new(&server).join("TrackmaniaServer").exists() {
        for n in ["map1 segment splits", "map2 block-rename fallback", "identity candidate"] {
            s.skipped("ORACLE", n, "no dedicated server (set TM_SERVER or --server DIR)");
        }
        return;
    }
    let (Some(map1), Some(g1), Some(g2)) = (
        fixture("map1.Map.Gbx"),
        fixture("map1_wr_19538.Ghost.Gbx"),
        fixture("map1_slow_19812.Ghost.Gbx"),
    ) else {
        s.skipped("ORACLE", "map1 segment splits", "map1 fixtures missing");
        return;
    };

    // The identity candidate FIRST: the unmodified map must give each ghost
    // its own declared race time. A project bug once truncated every candidate
    // and DNF'd all 916; without this row the rest of the tier cannot tell a
    // dead harness from a real result.
    let rows = oracle::run_maps(&[(map1.clone(), vec![g1.clone(), g2.clone()])], 2, &server);
    let t = oracle::times(&rows[0]);
    let a = t.get(&fname(&g1)).cloned().flatten();
    let b = t.get(&fname(&g2)).cloned().flatten();
    s.check(
        "ORACLE",
        "identity candidate",
        a == Some(19538) && b == Some(19812),
        &format!("{} and {} on the untouched map", secs::opt(a), secs::opt(b)),
    );
    if a.is_none() {
        s.bad("ORACLE", "map1 segment splits", "identity failed; the rest would be noise");
        return;
    }

    // Every segment map reproduces the reference ghost's own declared split.
    let out = scratch("segs");
    let _ = std::fs::remove_dir_all(&out);
    let segs = match crate::segments::make_all(&map1, &out, &g1, 8, &server, false) {
        Ok(v) => v,
        Err(e) => {
            s.bad("ORACLE", "map1 segment splits", &e);
            return;
        }
    };
    let pairs: Vec<(PathBuf, Vec<PathBuf>)> =
        segs.iter().map(|s| (s.map.clone(), vec![g1.clone(), g2.clone()])).collect();
    let res = oracle::run_maps(&pairs, 8, &server);
    let want1 = crate::ghost::splits(&g1).unwrap();
    let want2 = crate::ghost::splits(&g2).unwrap();
    let mut all = true;
    let mut detail = Vec::new();
    for (k, rows) in res.iter().enumerate() {
        let t = oracle::times(rows);
        for (g, want) in [(&g1, &want1), (&g2, &want2)] {
            let got = t.get(&fname(g)).cloned().flatten();
            let w = want[k] as i64;
            if got != Some(w) {
                all = false;
            }
            detail.push(format!("seg{} {} want {}", k + 1, secs::opt(got), secs::ms(w)));
        }
    }
    s.check("ORACLE", "map1 segment splits", all, &detail.join(", "));

    // The block-rename fallback, which is the case with a KNOWN error rather
    // than an exact one: it must be early, and by the amount the measurement
    // says, not by any amount at all.
    let (Some(map2), Some(g3)) = (fixture("map2.Map.Gbx"), fixture("map2_rank1_22730.Ghost.Gbx"))
    else {
        s.skipped("ORACLE", "map2 block-rename fallback", "map2 fixtures missing");
        return;
    };
    let out2 = scratch("segs2");
    let _ = std::fs::remove_dir_all(&out2);
    let segs2 = match crate::segments::make_all(&map2, &out2, &g3, 8, &server, false) {
        Ok(v) => v,
        Err(e) => {
            s.bad("ORACLE", "map2 block-rename fallback", &e);
            return;
        }
    };
    let pairs2: Vec<(PathBuf, Vec<PathBuf>)> =
        segs2.iter().map(|s| (s.map.clone(), vec![g3.clone()])).collect();
    let res2 = oracle::run_maps(&pairs2, 8, &server);
    let want3 = crate::ghost::splits(&g3).unwrap();
    let got1 = oracle::times(&res2[0]).get(&fname(&g3)).cloned().flatten();
    let early = got1.map(|v| want3[0] as i64 - v);
    s.check(
        "ORACLE",
        "map2 block-rename fallback",
        !segs2[0].exact && matches!(early, Some(e) if (150..=200).contains(&e)),
        &format!(
            "seg1 fired {} early against the declared {} (a finish block triggers at the cell \
             entry, not the centre)",
            secs::opt(early),
            secs::ms(want3[0] as i64)
        ),
    );

    inert_instrument(s, &server, &map1, &g1);
    splice_oracle(s, &server, &map1, &g1);
}

/// IS THE INSTRUMENT ALIVE?
///
/// On 173691 a gate-removed map, a deck-removed map and a road-removed map all
/// re-simulated to the identical 3102 rows. The tempting reading is "the map
/// does not matter here". The true reading was that the surgery never reached
/// the simulation at all — the recording carried its own map, and the file on
/// disk was decoration. **The road control is what proved the instrument was
/// dead rather than the maps identical**, and it is only a control because
/// removing the road under a car MUST change its run.
///
/// So this pair, in both directions, on the fixture map:
///
/// * move the checkpoint block the reference ghost drives through, far away →
///   the run must stop being what it was;
/// * move an off-route decoration by the same mover → the time must be
///   **exactly** unchanged.
///
/// One without the other proves nothing. A dead instrument passes the second
/// row on its own; a broken writer passes the first row on its own.
fn inert_instrument(s: &mut Suite, server: &str, map1: &Path, g1: &Path) {
    let m0 = MapFile::load(map1);
    // block#617 is the checkpoint whose split is 16.316 — the golden above
    // measures it, so this is the same object under two questions.
    let Some(cp) = m0.blocks.iter().find(|b| b.index == 617).cloned() else {
        s.skipped("ORACLE", "map surgery reaches the simulation", "block#617 not on this map");
        return;
    };
    let dir = scratch("inert");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let moved = dir.join("cp_moved.Map.Gbx");
    let mut m = MapFile::load(map1);
    m.move_block_cell(cp.index, (2, 2, 2));
    m.write_to(&moved).unwrap();

    // The off-route twin: the same mover, on a block the car never touches.
    // Chosen as the grid block furthest from the checkpoint, so the choice is
    // a rule and not a lucky index.
    let far = m0
        .blocks
        .iter()
        .filter(|b| b.waypoint_tag.is_none() && b.free_off.is_none())
        .max_by_key(|b| {
            let (x, _, z) = b.coords();
            let (cx, _, cz) = cp.coords();
            (x - cx).pow(2) + (z - cz).pow(2)
        })
        .cloned();
    let Some(far) = far else {
        s.skipped("ORACLE", "an off-route move changes nothing", "no off-route block");
        return;
    };
    let offroute = dir.join("offroute_moved.Map.Gbx");
    let mut m = MapFile::load(map1);
    let (fx, fy, fz) = far.coords();
    m.move_block_cell(far.index, (fx, fy + 4, fz));
    m.write_to(&offroute).unwrap();

    let res = oracle::run_maps(
        &[
            (moved.clone(), vec![g1.to_path_buf()]),
            (offroute.clone(), vec![g1.to_path_buf()]),
        ],
        2,
        server,
    );
    let after_cp = oracle::times(&res[0]).get(&fname(g1)).cloned().flatten();
    let after_far = oracle::times(&res[1]).get(&fname(g1)).cloned().flatten();
    s.check(
        "ORACLE",
        "map surgery reaches the simulation",
        after_cp != Some(19538),
        &format!(
            "moving the checkpoint block the run drives through: {} (was 19.538). A map edit that \
             changes nothing here means the instrument is dead, not that the map does not matter.",
            secs::opt(after_cp)
        ),
    );
    s.check(
        "ORACLE",
        "an off-route move changes nothing",
        after_far == Some(19538),
        &format!(
            "moving block#{} {} four cells up: {} — the same mover, off the line",
            far.index,
            far.name,
            secs::opt(after_far)
        ),
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn fname(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

// ------------------------------------------------------------------ SPLICE

/// THE WRITER'S OWN CONTROLS.
///
/// Everything above asks whether the right BODY was produced. These ask
/// whether the right FILE was — the question the dedicated server cannot be
/// asked, because it accepts a rebuilt map as readily as a spliced one, and
/// the game client is the only thing that has ever said otherwise.
///
/// Four checks per fixture, and the fourth is what makes the third mean
/// anything: the re-emitted form of the SAME body must share nothing, or an
/// "identical" verdict would be one a broken comparison also returns.
fn splice_checks(s: &mut Suite) {
    let mut covered: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for name in ["map1.Map.Gbx", "map2.Map.Gbx", "goth.Map.Gbx"] {
        let Some(p) = fixture(name) else {
            s.skipped("PURE", &format!("splice {}", name), "fixture missing");
            continue;
        };
        let m = MapFile::load(&p);
        let stock_file = std::fs::read(&p).unwrap();
        let stock_stream = m.gbx.comp.clone().expect("a map fixture is LZO-compressed");

        // 1. THE WALK AGREES WITH liblzo2. This module decides which stream
        //    bytes are literals; a walk that disagrees with the real decoder
        //    would patch the wrong bytes and every later check would be about
        //    a file nobody can read.
        match crate::splice::scan(&stock_stream) {
            Ok((sc, matches)) => {
                let replayed = sc.replay(&stock_stream, &matches);
                s.check(
                    "PURE",
                    &format!("splice.scan_agrees {}", name),
                    replayed == m.gbx.body,
                    &format!(
                        "{} literal runs, {} matches, {} cut points, {} bytes of body reproduced \
                         from the walk alone",
                        sc.lits.len(),
                        matches.len(),
                        sc.cuts.len(),
                        sc.out_len
                    ),
                );
            }
            Err(e) => s.bad("PURE", &format!("splice.scan_agrees {}", name), &e),
        }

        // 2. NO EDIT, NO CHANGE — at the FILE level. `roundtrip` compares
        //    decompressed bodies, which is the right level for a re-emitting
        //    writer and far too weak for this one.
        let (written, sp) = m.build_reporting();
        s.check(
            "PURE",
            &format!("splice.no_edit_is_byte_identical {}", name),
            written == stock_file,
            &format!("{} bytes in, {} out, method {:?}", stock_file.len(), written.len(), sp.method),
        );

        // 3. ONE MOVE CHANGES THE BYTES OF THAT MOVE AND NOTHING ELSE — for
        //    each REGIME the movers have, because they write different
        //    amounts: a grid cell is three bytes, a free block is twelve f32
        //    bytes of position, an item is twelve more. A splicer that only
        //    ever met a cell byte would be untested against the two edits that
        //    actually carry a gate.
        let grid = m
            .blocks
            .iter()
            .find(|b| b.free_off.is_none() && b.flags != 0xFFFF_FFFF)
            .map(|b| (format!("grid block#{}", b.index), Regime::Grid(b.index), 3));
        let free = m
            .blocks
            .iter()
            .find(|b| b.free_off.is_some())
            .map(|b| (format!("free block#{}", b.index), Regime::Free(b.index), 12));
        let item = m
            .items
            .first()
            .map(|it| (format!("item#{}", it.index), Regime::Item(it.index), 12));
        for (label, regime, most) in [grid.clone(), free.clone(), item.clone()].into_iter().flatten() {
            let mut m2 = MapFile::load(&p);
            match regime {
                Regime::Grid(i) => {
                    let (cx, cy, cz) = m.blocks[i].coords();
                    m2.move_block_cell(i, (cx, cy + 1, cz));
                }
                Regime::Free(i) => {
                    let mut v = m.blocks[i].free_pos.unwrap();
                    v[1] += 8.0;
                    m2.move_block_free(i, v);
                }
                Regime::Item(i) => {
                    let mut v = m.items[i].pos;
                    v[1] += 8.0;
                    m2.move_item_pos(i, v);
                }
            }
            let (edited, sp2) = m2.build_reporting();
            let body2 = crate::gbx::Gbx::parse(&edited).body;
            let diff = m.gbx.body.iter().zip(&body2).filter(|(x, y)| x != y).count();
            let carried = sp2.shared_prefix + sp2.shared_suffix;
            s.check(
                "PURE",
                &format!("splice.edit_is_local {} {}", name, label.split(' ').next().unwrap()),
                body2.len() == m.gbx.body.len()
                    && diff <= most
                    && carried * 100 / stock_stream.len() >= 90,
                &format!(
                    "{} moved: {} body bytes differ (at most {}), {} % of the stock stream carried \
                     verbatim, method {:?}",
                    label,
                    diff,
                    most,
                    carried * 100 / stock_stream.len(),
                    sp2.method
                ),
            );
        }

        // Which regimes this fixture could exercise is accumulated and
        // asserted once, after the loop: a regime no fixture has is a real gap
        // in the suite, while a regime THIS map does not have is not.
        for (kind, present) in
            [("grid", grid.is_some()), ("free", free.is_some()), ("item", item.is_some())]
        {
            if present {
                *covered.entry(kind).or_insert(0) += 1;
            }
        }

        // 4. THE NEGATIVE CONTROL FOR 3. Recompressing the same body shares
        //    essentially nothing with the stock stream, so check 3 is a test
        //    and not a tautology.
        let reemit = crate::gbx::lzo_compress(&m.gbx.body);
        let shared = reemit.iter().zip(&stock_stream).take_while(|(x, y)| x == y).count();
        s.check(
            "PURE",
            &format!("splice.reemit_shares_nothing {}", name),
            shared * 100 / stock_stream.len() < 10,
            &format!(
                "the same body recompressed: {} bytes against the stock stream's {}, {} shared \
                 from the front",
                reemit.len(),
                stock_stream.len(),
                shared
            ),
        );

        // 5. A RENAME CANNOT BE SPLICED, AND THE WRITER SAYS SO rather than
        //    pretending. The name is a length change in the body, so every
        //    offset after it moves and no part of the stock stream survives.
        let Some((_, Regime::Grid(bi), _)) = grid else {
            s.skipped("PURE", &format!("splice.rename_falls_back {}", name), "no grid block");
            continue;
        };
        let mut m3 = MapFile::load(&p);
        m3.set_block_name(bi, &format!("{}XX", m.blocks[bi].name));
        let (_, sp3) = m3.build_reporting();
        s.check(
            "PURE",
            &format!("splice.rename_falls_back {}", name),
            sp3.method == crate::splice::Method::Reemit,
            &format!("renaming block#{} reports method {:?}", bi, sp3.method),
        );
    }
    // Every regime must have been exercised SOMEWHERE. A fixture without free
    // blocks is fine; a suite without a free-block splice anywhere is not.
    for kind in ["grid", "free", "item"] {
        let n = covered.get(kind).copied().unwrap_or(0);
        s.check(
            "PURE",
            &format!("splice.regime_covered {}", kind),
            n > 0,
            &format!("the {} mover was spliced on {} of 3 fixtures", kind, n),
        );
    }
}

/// Which mover a splice check is exercising. The three regimes write different
/// numbers of bytes into different chunks, and a writer tested on one of them
/// is untested on the other two.
#[derive(Clone, Copy)]
enum Regime {
    Grid(usize),
    Free(usize),
    Item(usize),
}

/// THE TWO WRITERS MUST AGREE WITH THE ENGINE.
///
/// A splice that changed the map in any way the simulation can see would be a
/// worse bug than the one it fixes, and the byte checks above cannot see a
/// simulation. So: the same edit, written both ways, must give the same time
/// to the millisecond — and the pair is only a control because the edit is one
/// the run DOES feel (the checkpoint the reference ghost drives through),
/// which the row before it establishes by returning something other than
/// 19.538.
fn splice_oracle(s: &mut Suite, server: &str, map1: &Path, g1: &Path) {
    let dir = scratch("splice");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let m0 = MapFile::load(map1);
    // An OFF-ROUTE block, deliberately: the two writers must agree on a number
    // and not on a DNF. `inert_instrument` supplies the other half — the same
    // splice writer moving the checkpoint the run drives through, which DNFs —
    // so "19.538 both ways" here is not a writer that did nothing.
    let Some(b) = m0
        .blocks
        .iter()
        .find(|b| b.name == "Beach" && b.free_off.is_none())
        .or_else(|| m0.blocks.iter().find(|b| b.waypoint_tag.is_none() && b.free_off.is_none()))
        .cloned()
    else {
        s.skipped("ORACLE", "splice.same_time_as_reemit", "no off-route grid block");
        return;
    };
    let (cx, cy, cz) = b.coords();

    let spliced = dir.join("spliced.Map.Gbx");
    let mut m = MapFile::load(map1);
    m.move_block_cell(b.index, (cx, cy + 4, cz));
    let (bytes, sp) = m.build_reporting();
    std::fs::write(&spliced, &bytes).unwrap();

    // The same body, written the old way.
    let reemitted = dir.join("reemitted.Map.Gbx");
    let body = m.patched_body();
    let stream = crate::gbx::lzo_compress(&body);
    std::fs::write(&reemitted, m.gbx.file_with_stream(&body, &stream)).unwrap();

    let res = oracle::run_maps(
        &[
            (spliced.clone(), vec![g1.to_path_buf()]),
            (reemitted.clone(), vec![g1.to_path_buf()]),
        ],
        2,
        server,
    );
    let a = oracle::times(&res[0]).get(&fname(g1)).cloned().flatten();
    let b2 = oracle::times(&res[1]).get(&fname(g1)).cloned().flatten();
    s.check(
        "ORACLE",
        "splice.same_time_as_reemit",
        a == b2 && a == Some(19538),
        &format!(
            "block#{} {} four cells up, written by {:?}: {} — and re-emitted: {}",
            b.index,
            b.name,
            sp.method,
            secs::opt(a),
            secs::opt(b2)
        ),
    );
    let _ = std::fs::remove_dir_all(&dir);
}
