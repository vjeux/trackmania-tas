//! `tinyctl treelineup` — a species-by-variant lineup of trees plus the
//! cameras that shoot it at fixed distances, in one command (the trees
//! quality pass of 2026-09-10).
//!
//! ```text
//! tinyctl treelineup --host TINY.Map.Gbx --out LINEUP.Map.Gbx --views VIEWS.tsv
//!                    --at X,Y,Z --row SPECIES:HEIGHT:FILE[@Q],FILE[@Q],… [--row …]
//!                    [--pitch 11] [--row-gap 250] [--dists 5,15,40] [--v -0.08]
//!                    [--pictures DIR,…] [--yaw R] [--backdrop FILE[:SIZE]]
//! ```
//!
//! One `--row` per species: the STOCK item of that name stands first, then
//! every variant file in order, `--pitch` metres apart along +x; the rows
//! stand `--row-gap` metres apart along x too, all at `--at`'s y and z, so a
//! camera looking along z at one row sees no other. HEIGHT is the item's
//! height in metres (the half-size one): the cameras aim at mid-height. A
//! `FILE@Q` carries the placement's lightmap-quality byte Q (Normal 0, High
//! 1, VeryHigh 2, Highest 3, Lowest 4, VeryLow 5, Low 6).
//!
//! The views file (`tinyctl shoot` format, source coordinates, identity
//! anchor at scale 1) holds, per row: every item at the nearest distance from
//! the north (`r<k>i<i>d<D>`), every item at every middle distance from the
//! north and from the south (`…d<D>n` / `…d<D>s` — the sun lights one side),
//! and the whole row at the farthest distance from both sides (`r<k>d<D>n/s`).
//! `--v` is the camera's vertical angle (negative looks slightly up: a row
//! placed in the sky stands against sky, which is what `cropstats` reads
//! best).
//!
//! `--backdrop FILE[:SIZE]`: a flat SIZE-metre item (a half-size
//! DecoPlatformBase slab, 16 m) stood upright behind every row on the far
//! side from the north camera, SIZE apart and three layers high; a TWIN of
//! every row stands half a row gap further along x with its wall on the
//! other side, and the south cameras aim at the twin — the crown is then
//! measured against a uniform lit wall instead of clouds and water.
//!
//! Every `.dds` in the `--pictures` directories rides in the map archive as
//! `Items/<name>` (the variant bakes' textures; `TINY_TREE_MAT_SUFFIX` keeps
//! the names apart between variants — the game merges custom materials of
//! one name).

use std::path::{Path, PathBuf};

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn flags<'a>(args: &'a [String], name: &str) -> Vec<&'a str> {
    args.iter().enumerate().filter(|(_, a)| *a == name).filter_map(|(i, _)| args.get(i + 1)).map(|s| s.as_str()).collect()
}

struct Row {
    species: String,
    height: f32,
    files: Vec<PathBuf>,
    /// per variant file: the placement's lightmap-quality byte (`FILE@Q`), None = the donor's
    lmq: Vec<Option<u8>>,
}

fn parse_row(s: &str) -> Result<Row, String> {
    let mut parts = s.splitn(3, ':');
    let species = parts.next().filter(|x| !x.is_empty()).ok_or_else(|| format!("--row {s:?}: wants SPECIES:HEIGHT:FILE,…"))?.to_string();
    let height: f32 = parts.next().ok_or_else(|| format!("--row {s:?}: no height"))?.trim().parse().map_err(|e| format!("--row {s:?}: height: {e}"))?;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut lmq: Vec<Option<u8>> = Vec::new();
    for f in parts.next().map(|f| f.split(',').filter(|x| !x.is_empty()).collect::<Vec<_>>()).unwrap_or_default() {
        // `FILE@Q`: the placement's lightmap-quality byte for this copy
        let (path, q) = match f.rsplit_once('@') {
            Some((p, q)) if q.chars().all(|c| c.is_ascii_digit()) && !q.is_empty() => (p, Some(q.parse::<u8>().map_err(|e| format!("--row {s:?}: @{q}: {e}"))?)),
            _ => (f, None),
        };
        let p = PathBuf::from(path);
        if !p.is_file() {
            return Err(format!("--row {s:?}: {} is not a file", p.display()));
        }
        files.push(p);
        lmq.push(q);
    }
    Ok(Row { species, height, files, lmq })
}

pub fn cmd(args: &[String]) -> Result<(), String> {
    let host = flag(args, "--host").ok_or("treelineup needs --host TINY.Map.Gbx")?;
    let out = flag(args, "--out").ok_or("treelineup needs --out LINEUP.Map.Gbx")?;
    let views_out = flag(args, "--views").ok_or("treelineup needs --views VIEWS.tsv")?;
    let at: Vec<f32> = flag(args, "--at").ok_or("treelineup needs --at X,Y,Z")?.split(',').map(|x| x.trim().parse::<f32>().map_err(|e| format!("--at: {e}"))).collect::<Result<_, _>>()?;
    if at.len() != 3 {
        return Err("--at wants X,Y,Z".into());
    }
    let pitch: f32 = flag(args, "--pitch").unwrap_or("11").parse().map_err(|e| format!("--pitch: {e}"))?;
    let row_gap: f32 = flag(args, "--row-gap").unwrap_or("250").parse().map_err(|e| format!("--row-gap: {e}"))?;
    let v: f32 = flag(args, "--v").unwrap_or("-0.08").parse().map_err(|e| format!("--v: {e}"))?;
    let yaw: f32 = flag(args, "--yaw").unwrap_or("0").parse().map_err(|e| format!("--yaw: {e}"))?;
    let dists: Vec<f32> = flag(args, "--dists").unwrap_or("5,15,40").split(',').map(|x| x.trim().parse::<f32>().map_err(|e| format!("--dists: {e}"))).collect::<Result<_, _>>()?;
    if dists.is_empty() {
        return Err("--dists wants at least one distance".into());
    }
    let backdrop: Option<(PathBuf, f32)> = match flag(args, "--backdrop") {
        Some(s) => {
            let (f, size) = match s.rsplit_once(':') {
                Some((f, sz)) if sz.parse::<f32>().is_ok() => (f, sz.parse::<f32>().unwrap()),
                _ => (s, 16.0),
            };
            let p = PathBuf::from(f);
            if !p.is_file() {
                return Err(format!("--backdrop {f}: not a file"));
            }
            Some((p, size))
        }
        None => None,
    };
    let rows: Vec<Row> = flags(args, "--row").iter().map(|s| parse_row(s)).collect::<Result<_, _>>()?;
    if rows.is_empty() {
        return Err("treelineup needs at least one --row SPECIES:HEIGHT:FILE,…".into());
    }
    if !Path::new(host).is_file() {
        return Err(format!("--host {host}: not a file"));
    }

    // poses: the stock species of every row first (tmmaps lineup places the
    // --stock names before the --items), then the variant files row by row,
    // then (with a backdrop) the twin rows and the walls
    let row_x = |k: usize| at[0] + k as f32 * row_gap;
    let twin_x = |k: usize| at[0] + k as f32 * row_gap + row_gap / 2.0;
    let mut stock_names: Vec<String> = Vec::new();
    let mut stock_poses: Vec<String> = Vec::new();
    let mut item_poses: Vec<String> = Vec::new();
    let mut items: Vec<String> = Vec::new();
    // the lightmap-quality bytes in placement order: the stock rows keep the donor's (0)
    let mut lmqs: Vec<u8> = Vec::new();
    let mut any_lmq = false;
    let pose = |x: f32, dy: f32, dz: f32, yaw: f32, pitch_r: f32| format!("{:.2},{:.2},{:.2},{yaw},{pitch_r:.4},0", x, at[1] + dy, at[2] + dz);
    for (k, r) in rows.iter().enumerate() {
        stock_names.push(r.species.clone());
        stock_poses.push(pose(row_x(k), 0.0, 0.0, yaw, 0.0));
        if backdrop.is_some() {
            stock_names.push(r.species.clone());
            stock_poses.push(pose(twin_x(k), 0.0, 0.0, yaw, 0.0));
        }
    }
    for (k, r) in rows.iter().enumerate() {
        for (i, f) in r.files.iter().enumerate() {
            item_poses.push(pose(row_x(k) + (i + 1) as f32 * pitch, 0.0, 0.0, yaw, 0.0));
            items.push(f.display().to_string());
            lmqs.push(r.lmq[i].unwrap_or(0));
            any_lmq |= r.lmq[i].is_some();
        }
    }
    let mut wall_poses: Vec<String> = Vec::new();
    if let Some((_, size)) = &backdrop {
        for (k, r) in rows.iter().enumerate() {
            for (i, f) in r.files.iter().enumerate() {
                item_poses.push(pose(twin_x(k) + (i + 1) as f32 * pitch, 0.0, 0.0, yaw, 0.0));
                items.push(f.display().to_string());
                lmqs.push(r.lmq[i].unwrap_or(0));
            }
            let span = (r.files.len() + 1) as f32 * pitch;
            let n_walls = ((span + 2.0 * size) / size).ceil() as usize;
            // the wall is a flat slab stood on its edge (pitch pi/2): three layers
            // cover the row's height whichever way the rotation sends its span
            for (x0, dz) in [(row_x(k), 7.0f32), (twin_x(k), -7.0)] {
                for w in 0..n_walls {
                    for dy in [-16.0f32, 0.0, 16.0] {
                        wall_poses.push(pose(x0 - size + w as f32 * size, dy, dz, 0.0, std::f32::consts::FRAC_PI_2));
                    }
                }
            }
        }
    }
    let n_stock = stock_poses.len();
    let n_walls_total = wall_poses.len();
    let places: Vec<String> = stock_poses.into_iter().chain(item_poses).chain(wall_poses).collect();
    // the stock rows carry no explicit byte (0); the walls neither
    let mut lmq_all: Vec<u8> = vec![0; n_stock];
    lmq_all.extend(lmqs);
    lmq_all.extend(std::iter::repeat(0u8).take(n_walls_total));

    // the textures next to the items
    let mut extra: Vec<String> = Vec::new();
    for dir in flag(args, "--pictures").map(|s| s.split(',').filter(|x| !x.is_empty()).collect::<Vec<_>>()).unwrap_or_default() {
        let mut names: Vec<PathBuf> = std::fs::read_dir(dir).map_err(|e| format!("--pictures {dir}: {e}"))?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map(|x| x.eq_ignore_ascii_case("dds")).unwrap_or(false) || p.file_name().map(|n| n.to_string_lossy().ends_with(".Mesh.Gbx") || n.to_string_lossy().ends_with(".DynaObject.Gbx")).unwrap_or(false)).collect();
        names.sort();
        for p in names {
            let name = p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            let entry = format!("Items/{name}={}", p.display());
            if !extra.iter().any(|e| e.starts_with(&format!("Items/{name}="))) {
                extra.push(entry);
            }
        }
    }

    let mut largs: Vec<String> = vec!["tmmaps".into(), "lineup".into(), host.into(), "--out".into(), out.into(), "--at".into(), format!("{},{},{}", at[0], at[1], at[2]), "--stock".into(), stock_names.join(","), "--place".into(), places.join(";"), "--yaw".into(), format!("{yaw}")];
    if let Some((wall, _)) = &backdrop {
        if n_walls_total > 0 {
            items.push(format!("{}*{n_walls_total}", wall.display()));
        }
    }
    if !items.is_empty() {
        largs.push("--items".into());
        largs.push(items.join(","));
    }
    if !extra.is_empty() {
        largs.push("--extra".into());
        largs.push(extra.join(","));
    }
    if any_lmq {
        largs.push("--lmq".into());
        largs.push(lmq_all.iter().map(|q| q.to_string()).collect::<Vec<_>>().join(","));
    }
    tmmaps::tiny::lineup_cmd(&largs);

    // the cameras
    let near = dists[0];
    let far = *dists.last().unwrap();
    let mut lines: Vec<String> = vec![
        format!("# treelineup views for {out}; NAME<TAB>ox,oy,oz<TAB>DIST<TAB>H<TAB>V (identity anchor, scale 1)"),
        "# anchor 0,0,0:0,0,0".to_string(),
    ];
    let mut n_views = 0usize;
    for (k, r) in rows.iter().enumerate() {
        let ty = at[1] + r.height / 2.0;
        let n_items = r.files.len() + 1;
        let x_of = |i: usize| row_x(k) + i as f32 * pitch;
        // with a backdrop the south cameras aim at the twin row (its wall is on the south side)
        let xs_of = |i: usize| if backdrop.is_some() { twin_x(k) + i as f32 * pitch } else { x_of(i) };
        for i in 0..n_items {
            lines.push(format!("r{k}i{i}d{}\t{:.2},{:.2},{:.2}\t{:.1}\t0.0000\t{v:.4}", near as i32, x_of(i), ty, at[2], near));
            n_views += 1;
            for d in dists.iter().skip(1).take(dists.len().saturating_sub(2)) {
                lines.push(format!("r{k}i{i}d{}n\t{:.2},{:.2},{:.2}\t{:.1}\t0.0000\t{v:.4}", *d as i32, x_of(i), ty, at[2], d));
                lines.push(format!("r{k}i{i}d{}s\t{:.2},{:.2},{:.2}\t{:.1}\t{:.4}\t{v:.4}", *d as i32, xs_of(i), ty, at[2], d, std::f32::consts::PI));
                n_views += 2;
            }
        }
        if dists.len() > 1 {
            let cx = row_x(k) + (n_items as f32 - 1.0) / 2.0 * pitch;
            let cxs = xs_of(0) + (n_items as f32 - 1.0) / 2.0 * pitch;
            lines.push(format!("r{k}d{}n\t{cx:.2},{ty:.2},{:.2}\t{far:.1}\t0.0000\t{v:.4}", far as i32, at[2]));
            lines.push(format!("r{k}d{}s\t{cxs:.2},{ty:.2},{:.2}\t{far:.1}\t{:.4}\t{v:.4}", far as i32, at[2], std::f32::consts::PI));
            n_views += 2;
        }
    }
    std::fs::write(views_out, lines.join("\n") + "\n").map_err(|e| format!("{views_out}: {e}"))?;
    println!("wrote {views_out}: {n_views} views over {} rows ({} items in all{})", rows.len(), rows.iter().map(|r| r.files.len() + 1).sum::<usize>(), if n_walls_total > 0 { format!(", twin rows and {n_walls_total} wall pieces") } else { String::new() });
    Ok(())
}
