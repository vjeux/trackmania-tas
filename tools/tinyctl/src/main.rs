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
mod lightmap;
mod loadloop;
mod camcheck;
mod build;
mod compare;
mod cropstats;
mod play;
mod png;
mod probe;
mod publish;
mod replaypull;
mod startcheck;
mod shoot;
mod unproject;
mod upload;
mod video;
mod views;
mod wsx;

use std::path::Path;

const USAGE: &str = r#"tinyctl — tiny-campaign operations (Rust, no shell)

  tinyctl probe SRC.Map.Gbx [--paks "--pak F:KEY ..."] [--keep]
        collection, ground row (table vs measured), fixed plane, anchor,
        genealogy zones + policy, zone-block census, waypoints, models;
        with --paks a dry library build listing the models the packs lack
  tinyctl build NN… [--src-dir /tmp/summer2026] [--out-root /tmp] [--tag auto] [--recipe /tmp/tiny3/recipe.env] [--env K=V …]
        the tiny build of campaign maps end to end (packs by collection, recipe env,
        mapgeom tiny-library, tmmaps tiny, library unzipped) into <out-root>/tinyNN/<tag>/
  tinyctl views SRC.Map.Gbx [--out VIEWS.tsv] [--gate-dist 48] [--ghost G --at MS[,MS…] [--chase-dist 30] [--chase-v 0.3] [--only-chase]]
        start / every checkpoint / finish looked at along the gate, the
        whole map from above and its four quadrants; the anchor as a comment;
        --ghost/--at add chase views (camera behind the car) at instants of a driven lap
  tinyctl shoot --orig SRC --tiny TINY --views VIEWS.tsv --tag sNN --anchor A
                [--outdir /tmp/tiny3] [--only o|t] [--pull-full] [--ab] [-v]
        push, one editor load per side on the render box (shootctl shootset),
        compare there, pull the sheets: cmpdiff-sNN-crops.png / -overview.png
        (--ab: --orig is a tiny build too — an A/B of two tiny outputs — and is
        shot through the anchor like the tiny side)
  tinyctl play --map MAP --tag T [--shots 4] [--every-ms 200] [--first-ms 300] [--timeout 600] [--outdir D]
               [--drive-ms MS [--drive-at-ms 13500]]
        the map in PLAY mode on the box (shootctl playshots): N timed frames from
        the playground opening — the MediaTracker intro — as one stacked sheet
  tinyctl startcheck --map MAP [--tag T] [--tolerance 12] [--outdir D]
        where does the CLIENT put the car? opens the playground, reads the car at
        rest, PASS/FAIL against the map's Spawn placement (no vehicle = loud FAIL)
  tinyctl loadloop --maps A[,B…] --tag T [--seq 0,1,…] [--n N] [--how play|edit] [--timeout 300]
                 [--settle-ms 3000] [--fresh|--fresh-first] [--shot-on-fail] [--outdir D]
        N loads of one map (or an A,B,A,B switch sequence) on the box, each classified
        from the object graph: OPENED (seconds, car?) / DIALOG (frame + text) / TIMEOUT / CRASH
  tinyctl replay-pull [--map MAP] [--tag T] [--drive-ms MS] [--wait-only] [--wait 900] [--out DIR]
        a REAL client recording: parks the box's autosaved replays, opens the map
        in play mode (or waits while somebody drives it), then pulls the new
        `…_PersonalBest_TimeAttack.Replay.Gbx` with its md5 verified. Only a
        FINISHED run autosaves; a map needing steering wants --wait-only or a tape
  tinyctl camcheck --orig cam-O.tsv --tiny cam-T.tsv --anchor sx,sy,sz:tx,ty,tz [--scale 0.5] [--trigger lo:hi]
        two --camlog-ms logs aligned on the intro's first camera cut: the tiny camera
        vs the original's through the transform, per 250 ms; the in-game trigger jump
  tinyctl compare --views VIEWS.tsv --dir DIR --tag sNN [--color 50] [--edge 16] [--keep-hud]
                  [--max-crops 16] [--hstack-ffmpeg BIN] [--out-prefix P]
  tinyctl compare --pair ORIG.png TINY.png [--out-prefix P]
        per-cell colour/edge diff of cmp-<tag><view>-o.png vs -t.png
  tinyctl unproject --view "ox,oy,oz,dist,h,v" --px X,Y [--size 1920,1080] [--fov 85]
                    [--ground Y] [--anchor sx,sy,sz:tx,ty,tz [--scale 0.5]] [--side o|t]
        where in the world a pixel of a shot is: the view row's orbital camera,
        the pixel's ray met with the plane y = ground (default the target's);
        --side t = the tiny side (camera through the anchor), answer in both maps
  tinyctl cropstats IMG… --crop x,y,w,h [--cells N] [--sheet OUT.png]
        a lineup row shot several times from one camera, as numbers: per image and
        cell (one per item) the non-sky share, the dark share, the foreground colour
        and the difference to the previous frame; --sheet stacks the crops
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
  tinyctl video --map NN | --all [--ghost F] [--out /tmp/tinyvid] [--maps-dir /tmp/audit/ship9] [--ghosts-dir /tmp/ghosts]
                [--cam 2] [--load-timeout 120] [--no-guard] [--store host:dir|dir] [--pull-webm] [--box-videos DIR]
        the map's driven lap as a video: REFUSES a ghost whose sample 0 is not on this
        map's start line (a render plays samples — a donor container flies off the map),
        pushes map (md5-skipped) + ghost, `shootctl render` on the box under the render
        lock, the clip to Maps\Tiny\videos\NN-ghost-<time>.webm, the 16-tile contact
        sheet (+ a 2 fps dense one) pulled into --out; --store copies clip + sheet on;
        --all renders every NN.Ghost.Gbx in --ghosts-dir whose TRAJECTORY (samples + race time, not
        the file md5: a metadata rewrite is not a new lap) is not yet in <out>/videos.tsv; --adopt records them as done

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
        "build" => build::cmd(rest),
        "views" => views::cmd(rest),
        "shoot" => shoot::cmd(rest),
        "lightmap" => lightmap::cmd(rest),
        "compare" => compare::cmd(rest),
        "cropstats" => cropstats::cmd(rest),
        "play" => play::cmd(rest),
        "startcheck" => startcheck::run(rest),
        "loadloop" => loadloop::cmd(rest),
        "replay-pull" => replaypull::cmd(rest),
        "camcheck" => camcheck::cmd(rest),
        "publish-map" => publish::publish_map_cmd(rest),
        "publish-here" => publish::publish_here_cmd(rest),
        "unproject" => unproject::cmd(rest),
        "upload" => upload::cmd(rest),
        "video" => video::cmd(rest),
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
