//! `tinyctl` — the tiny-campaign operations, one binary, no shell.
//!
//! Per map, in order:
//!
//! ```text
//! tinyctl probe   SRC.Map.Gbx [--paks "$PAKS"]            what the environment needs
//! tinyctl views   SRC.Map.Gbx --out VIEWS.tsv             the comparison cameras
//! tinyctl shoot   --orig SRC --tiny TINY --views VIEWS.tsv --tag sNN --anchor A
//!                                                          both sides, one load each, diffed
//! tinyctl compare --views VIEWS.tsv --dir DIR --tag sNN   (re)diff shots already taken
//! tinyctl publish-map NN --map TINY.Map.Gbx [--items-dir DIR] [--playcheck]
//! tinyctl box-build                                        rebuild the tools on the render box
//! ```
//!
//! Box-side halves (run there by the commands above): `publish-here`,
//! `selfbuild`; the shooting half is `shootctl shootset`.

mod boxbuild;
mod compare;
mod png;
mod probe;
mod publish;
mod shoot;
mod upload;
mod views;
mod wsx;

use std::path::Path;

const USAGE: &str = r#"tinyctl — tiny-campaign operations (Rust, no shell)

  tinyctl probe SRC.Map.Gbx [--paks "--pak F:KEY ..."] [--keep]
        collection, ground row (table vs measured), fixed plane, anchor,
        genealogy zones + policy, zone-block census, waypoints, models;
        with --paks a dry library build listing the models the packs lack
  tinyctl views SRC.Map.Gbx [--out VIEWS.tsv] [--gate-dist 48]
        start / every checkpoint / finish looked at along the gate, the
        whole map from above and its four quadrants; the anchor as a comment
  tinyctl shoot --orig SRC --tiny TINY --views VIEWS.tsv --tag sNN --anchor A
                [--outdir /tmp/tiny3] [--only o|t] [--pull-full] [--ab] [-v]
        push, one editor load per side on the render box (shootctl shootset),
        compare there, pull the sheets: cmpdiff-sNN-crops.png / -overview.png
        (--ab: --orig is a tiny build too — an A/B of two tiny outputs — and is
        shot through the anchor like the tiny side)
  tinyctl compare --views VIEWS.tsv --dir DIR --tag sNN [--color 50] [--edge 16] [--keep-hud]
                  [--max-crops 16] [--hstack-ffmpeg BIN] [--out-prefix P]
  tinyctl compare --pair ORIG.png TINY.png [--out-prefix P]
        per-cell colour/edge diff of cmp-<tag><view>-o.png vs -t.png
  tinyctl publish-map NN --map TINY.Map.Gbx [--items-dir DIR --paks "--pak F:KEY …"] [--name N]
                [--club 43788] [--campaign 155555] [--position P] [--playcheck]
        item-check gate, push, upload/update on Nadeo Services, campaign
        playlist, stored-bytes md5 readback, optional play + screenshot
  tinyctl box-build [--crates shootctl,tinyctl,mapgeom,tmmaps] [--bootstrap]
        git pull + cargo build on the render box, polled to completion
  tinyctl upload FILE… [--record uploads.txt] [--meta BIN]
        JPG/PNG sheets into the agentcloud attachment store (intern GraphQL
        xfb_metamate_nest_bulk_file_upload through `meta`); prints NAME<TAB>ID,
        embed as ![..](/api/attachments/view?file_id=ID); --record appends the rows

  box-side halves: tinyctl publish-here …   tinyctl selfbuild …
  every bridge command takes --wsx PATH (default ~/bin/wsx)
"#;

pub fn compare_view_names(views: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(views).map_err(|e| format!("{}: {e}", views.display()))?;
    Ok(text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).filter_map(|l| l.split('\t').next()).map(|s| s.trim().to_string()).collect())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("tinyctl {} ({})", env!("CARGO_PKG_VERSION"), option_env!("TAS_BUILD").unwrap_or("dev"));
        return;
    }
    let Some(cmd) = args.first() else {
        eprint!("{USAGE}");
        std::process::exit(2);
    };
    let rest = &args[1..];
    let r = match cmd.as_str() {
        "probe" => probe::cmd(rest),
        "views" => views::cmd(rest),
        "shoot" => shoot::cmd(rest),
        "compare" => compare::cmd(rest),
        "publish-map" => publish::publish_map_cmd(rest),
        "publish-here" => publish::publish_here_cmd(rest),
        "upload" => upload::cmd(rest),
        "box-build" => boxbuild::box_build_cmd(rest),
        "selfbuild" => boxbuild::selfbuild_cmd(rest),
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(())
        }
        other => Err(format!("unknown command `{other}`\n{USAGE}")),
    };
    if let Err(e) = r {
        eprintln!("tinyctl {cmd}: {e}");
        std::process::exit(1);
    }
}
