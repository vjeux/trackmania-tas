//! `tinyctl bake-copies NN… --tracker T.tsv --src-dir DIR --out-root R --tag x2 …` —
//! the LIGHTMAP BAKE COPY of every built map: the same build as the shipped
//! file (same source, scale, alias base, name, times and the size ladder's
//! rung read off the tracker row) with the bake knobs on top, into
//! `<out-root>/tinyNN/<bake-tag>/`:
//!
//! * `TINY_LIGHTMAP_FILL=1` — the items' lightmap charts stretched to fill
//!   their atlas, so the editor's bake gives a 16 m deck more than a handful
//!   of texels (TINY.md "Lightmaps");
//! * `TINY_VEGET_INLINE=0` on BlueBay — the jungle-cover cards are a green
//!   emitter to the lightmapper; the card-less copy has the same items in
//!   the same order, so the baked chunk transplants onto the shipped file;
//! * `--keep-zone-block` when the shipped file has no authored block and no
//!   baked record — a 0-block map crashes the editor's lightmapper; the kept
//!   tile is a block, the item list is unchanged.
//!
//! The copy's item count must equal the shipped file's (the lightmap is
//! applied by item index): a mismatch is reported and the copy deleted.
//! One row per map goes to `--report R.tsv` (map, copy, items, verdict).
//! The bake itself is `tinyctl lightmap COPY --into SHIPPED=OUT` (2026-09-22,
//! the giant campaigns: 75 shipped files).

use std::path::PathBuf;

pub fn cmd(args: &[String]) -> Result<(), String> {
    let f = |k: &str| tmmaps::cli::flag(args, k).map(String::from);
    let maps: Vec<String> = args.iter().take_while(|a| !a.starts_with("--")).cloned().collect();
    if maps.is_empty() || !maps.iter().all(|m| m.len() == 2 && m.chars().all(|c| c.is_ascii_digit())) {
        return Err("bake-copies needs two-digit map numbers first".into());
    }
    let tracker = PathBuf::from(f("--tracker").ok_or("bake-copies needs --tracker T.tsv (the pipeline's)")?);
    let src_dir = f("--src-dir").ok_or("--src-dir DIR")?;
    let out_root = f("--out-root").ok_or("--out-root DIR")?;
    let tag = f("--tag").ok_or("--tag T (the shipped build's)")?;
    let bake_tag = f("--bake-tag").unwrap_or_else(|| format!("{tag}-bake"));
    let prefix = f("--out-prefix").unwrap_or_else(|| "Summer".into());
    let recipe = f("--recipe").unwrap_or_else(|| "/tmp/tiny3/recipe.env".into());
    let report = PathBuf::from(f("--report").unwrap_or_else(|| format!("{out_root}/bake-copies-{tag}.tsv")));
    if !report.exists() {
        std::fs::write(&report, "nn\tshipped\tcopy\titems_shipped\titems_copy\tblocks_shipped\trung\tverdict\n").map_err(|e| format!("{}: {e}", report.display()))?;
    }
    // the last tracker row per map: its note carries the ladder's rung
    let text = std::fs::read_to_string(&tracker).map_err(|e| format!("{}: {e}", tracker.display()))?;
    let mut last: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for l in text.lines().skip(1) {
        let cols: Vec<String> = l.split('\t').map(String::from).collect();
        if let Some(nn) = cols.first() {
            last.insert(nn.clone(), cols);
        }
    }
    let scale = crate::build::scale_of(args);
    let label = crate::build::variant_label(scale);
    let mut failed = 0usize;
    for nn in &maps {
        let row = last.get(nn).ok_or_else(|| format!("{nn}: no tracker row in {}", tracker.display()))?;
        let note = row.get(16).cloned().unwrap_or_default();
        // "fit under N B: A -> B (lod-pick 1, parts under 8125 vertices sharp)" / "(lod-pick 0 (far levels dropped))"
        let rung: Option<(String, Option<String>)> = note.split("(lod-pick ").nth(1).map(|r| {
            let level = r.chars().take_while(|c| c.is_ascii_digit()).collect::<String>();
            let verts = r.split("parts under ").nth(1).map(|v| v.chars().take_while(|c| c.is_ascii_digit()).collect::<String>()).filter(|v| !v.is_empty());
            (level, verts)
        });
        let shipped = PathBuf::from(&out_root).join(format!("tiny{nn}")).join(&tag).join(format!("{prefix}-{nn}-{label}.Map.Gbx"));
        if !shipped.exists() {
            return Err(format!("{}: no such shipped build", shipped.display()));
        }
        let sm = tmmaps::map::MapFile::load(&shipped);
        let coll = crate::views::collection_of(&sm);
        let n_ship = sm.items.len();
        let zero_block = sm.blocks.is_empty() && sm.baked.is_empty();
        let mut bargs: Vec<String> = vec![nn.clone(), "--src-dir".into(), src_dir.clone(), "--out-root".into(), out_root.clone(), "--tag".into(), bake_tag.clone(), "--recipe".into(), recipe.clone(), "--out-prefix".into(), prefix.clone()];
        for k in ["--scale", "--name-format", "--times-scale"] {
            if let Some(v) = f(k) {
                bargs.push(k.into());
                bargs.push(v);
            }
        }
        // the same env the pipeline gave the shipped build
        let mut i = 0;
        while i < args.len() {
            if args[i] == "--env" {
                if let Some(v) = args.get(i + 1) {
                    bargs.push("--env".into());
                    bargs.push(v.clone());
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        if !bargs.iter().any(|e| e.starts_with("TINY_WATER_ROADS=")) {
            bargs.push("--env".into());
            bargs.push("TINY_WATER_ROADS=0".into());
        }
        if let Some(part) = f("--alias-part").and_then(|p| p.parse::<usize>().ok()) {
            let map_no: usize = nn.parse().unwrap_or(0);
            bargs.push("--env".into());
            bargs.push(format!("TINY_ALIAS_BASE={}", (part * 100 + map_no) * 1000));
            bargs.push("--env".into());
            bargs.push(format!("TINY_PICTURE_SUFFIX=_p{part:02}{map_no:02}"));
        }
        if let Some((level, verts)) = &rung {
            bargs.push("--lod-pick".into());
            bargs.push(level.clone());
            if let Some(v) = verts {
                bargs.push("--lod-pick-min-verts".into());
                bargs.push(v.clone());
            }
        }
        // the bake knobs
        bargs.push("--env".into());
        bargs.push("TINY_LIGHTMAP_FILL=1".into());
        if coll == 0x1c {
            bargs.push("--env".into());
            bargs.push("TINY_VEGET_INLINE=0".into());
        }
        if zero_block {
            bargs.push("--keep-zone-block".into());
        }
        let rung_s = rung.as_ref().map(|(l, v)| format!("lod-pick {l}{}", v.as_ref().map(|v| format!(" min-verts {v}")).unwrap_or_default())).unwrap_or_else(|| "full".into());
        println!("\n===== {nn}: bake copy ({rung_s}{}{}) =====", if coll == 0x1c { ", card-less" } else { "" }, if zero_block { ", one zone block kept" } else { "" });
        let copy = PathBuf::from(&out_root).join(format!("tiny{nn}")).join(&bake_tag).join(format!("{prefix}-{nn}-{label}.Map.Gbx"));
        let verdict = match crate::build::cmd(&bargs) {
            Ok(()) => {
                let cm = tmmaps::map::MapFile::load(&copy);
                if cm.items.len() == n_ship {
                    format!("ok ({} items)", n_ship)
                } else {
                    failed += 1;
                    format!("ITEM COUNT MISMATCH: shipped {} vs copy {}", n_ship, cm.items.len())
                }
            }
            Err(e) => {
                failed += 1;
                format!("BUILD FAILED: {}", e.lines().next().unwrap_or(""))
            }
        };
        let n_copy = if copy.exists() { tmmaps::map::MapFile::load(&copy).items.len().to_string() } else { "-".into() };
        println!("{nn}: {verdict}");
        let line = format!("{nn}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", shipped.display(), copy.display(), n_ship, n_copy, sm.blocks.len(), rung_s, verdict);
        let mut fh = std::fs::OpenOptions::new().append(true).open(&report).map_err(|e| format!("{}: {e}", report.display()))?;
        std::io::Write::write_all(&mut fh, line.as_bytes()).map_err(|e| e.to_string())?;
    }
    if failed > 0 {
        return Err(format!("{failed} of {} bake copies failed", maps.len()));
    }
    Ok(())
}
