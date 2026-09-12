//! `tmmaps` — everything this project does to a TM2020 **map**.
//!
//! One binary, one container implementation, a control behind every operation.
//! Its counterpart is `tools/ghost`, which owns the **ghost / replay** format;
//! the two never overlap. A recording that carries its own map is a `ghost`
//! problem until the map is out of it:
//!
//! ```text
//! ghost map extract R.Replay.Gbx --out m.Map.Gbx
//! tmmaps move m.Map.Gbx --out m2.Map.Gbx --move 2089@1520,300,600
//! ghost map set R.Replay.Gbx R2.Replay.Gbx --map m2.Map.Gbx
//! ```
//!
//! That composition is deliberate. `u02` used to reach into the replay's
//! carried map itself (`u02 movefree`), which meant two implementations of the
//! embedded-map chunk and two of the block movers. `u02` is deleted; see
//! `MAPS.md` §"What happened to u02".
//!
//! Times are printed as **seconds with a decimal** (`16.316`), never as raw
//! milliseconds.

use tmmaps::{census, controls, dropscan, header, rotate, segments, splice, selftest};

mod cmd;
use cmd::{inspect, ladder, surgery};

/// A tool whose census is 90 000 lines long will be piped into `head`, and a
/// Rust binary ignores SIGPIPE by default — so the write fails, and the
/// failure surfaces as a panic and a backtrace note on a command that worked.
/// Take the Unix default back. (Declared here rather than pulling in `libc`:
/// this crate has zero dependencies and that is worth keeping.)
fn restore_sigpipe() {
    const SIGPIPE: i32 = 13;
    const SIG_DFL: usize = 0;
    extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
    }
    unsafe {
        signal(SIGPIPE, SIG_DFL);
    }
}

fn main() {
    restore_sigpipe();
    // A refusal that reaches the user as a Rust panic tells them to run with
    // `RUST_BACKTRACE=1`, which is the wrong instruction: the tool worked, the
    // command was wrong. The library keeps panicking — that is what makes the
    // refusals testable with `catch_unwind` in the suite — but at the CLI
    // boundary a panic prints its message and nothing else.
    std::panic::set_hook(Box::new(|info| {
        let msg = info
            .payload()
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| info.payload().downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "internal error".to_string());
        eprintln!("tmmaps: {}", msg);
        // Exit 3, the same code `die` uses, so a refusal has one code whether
        // it was raised at the CLI boundary or deep in the library. This is
        // safe for the suite, which swaps in its own hook around every
        // `catch_unwind` and therefore never reaches this one.
        std::process::exit(3);
    }));
    // --version / -V. Compile-time only: CARGO_PKG_* come from the crate's
    // Cargo.toml (which inherits the one workspace version), and TAS_BUILD is
    // the git hash the release build sets. option_env! means an ordinary
    // `cargo build` still works and simply reports "dev". No dependency.
    if std::env::args().any(|x| x == "--version" || x == "-V") {
        println!(
            "{} {} ({})",
            option_env!("CARGO_BIN_NAME").unwrap_or(env!("CARGO_PKG_NAME")),
            env!("CARGO_PKG_VERSION"),
            option_env!("TAS_BUILD").unwrap_or("dev")
        );
        std::process::exit(0);
    }
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");
    // Every MAP-taking subcommand reads `args[2]`, and with the argument
    // missing that is `index out of bounds: the len is 2 but the index is 2` —
    // a panic where a usage line belongs. Say what is missing instead.
    const WANTS_MAP: &[&str] = &[
        "waypoints", "census", "skins", "fillers", "region", "colors", "phases", "genealogy", "tiny-catalog", "lineup", "shared-cells", "tiny", "tiny-batch", "clear", "shift", "segments", "move", "rotate", "ladder",
        "roundtrip",
        "renamecheck", "cporder", "origin", "chunks", "blockrefs", "setuid", "lmquality", "delblocks", "striplightmap", "itembytes", "mediatracker",
    ];
    if WANTS_MAP.contains(&cmd) && args.len() < 3 {
        eprintln!("tmmaps {} needs a MAP path.\n\n{}", cmd, USAGE);
        std::process::exit(2);
    }
    match cmd {
        "selftest" => selftest::run(&args),
        "region" => census::cmd_region(&args),
        "tiny-catalog" => tmmaps::tiny::catalog_cmd(&args),
        "lineup" => tmmaps::tiny::lineup_cmd(&args),
        "shared-cells" => tmmaps::tiny::shared_cells_cmd(&args),
        "tiny" => tmmaps::tiny::cmd(&args),
        "tiny-batch" => tmmaps::tiny::cmd_batch(&args),
        "clear" => census::cmd_clear(&args),
        "shift" => census::cmd_shift(&args),
        "recdump" => surgery::recdump(&args),
        "untag" => surgery::untag(&args),
        "swapplace" => surgery::swapplace(&args),
        "swaprec" => surgery::swaprec(&args),
        "swapcell" => surgery::swapcell(&args),
        "swapmodel" => surgery::swapmodel(&args),
        "wpdump" => surgery::wpdump(&args),
        "waypoints" => surgery::waypoints(&args),
        "segat" => segments::cmd_segat(&args),
        "segments" => inspect::segment_table(&args),
        "ladder" => ladder::ladder(&args),
        // ---- w612: write ONE map with several grid blocks moved.
        "rotate" => rotate::cmd(&args),
        "move" => ladder::move_blocks(&args),
        "blockprobe" => ladder::blockprobe(&args),
        "addblock" => ladder::addblock(&args),
        "recdump" => ladder::recdump(&args),
        "oracle" => ladder::oracle_stage(&args),
        "roundtrip" => controls::cmd_roundtrip(&args),
        "bodydiff" => splice::cmd_bodydiff(&args),
        "rewrite" => splice::cmd_rewrite(&args),
        "renamecheck" => surgery::renamecheck(&args),
        "cporder" => ladder::cporder(&args),
        "rungspec" => ladder::rungspec(&args),
        "origin" => controls::cmd_origin(&args),
        // `tmmaps setuid MAP --out F [--uid U]`: the map with a fresh (or the
        // given) UID. The game caches a map's computed lightmap and its
        // embedded items by UID: a rebuilt test copy under the published UID
        // plays with the OLD build's baked light (Summer 09's start deck read
        // dark in play mode until this, 2026-09-07).
        "stripghost" => surgery::stripghost(&args),
        "validate" => surgery::validate(&args),
        "setuid" => surgery::setuid(&args),
        "lmquality" => surgery::lmquality(&args),
        // `tmmaps delblocks MAP --out F [--keep-baked Sea,…] [--strip-lightmap]`: every
        // authored block deleted (the generated ones too, but for --keep-baked),
        // items kept — the 0-block form of `tmmaps tiny` on ANY map, for the
        // lightmapper-crash bisect of 2026-09-07
        "delblocks" => surgery::delblocks(&args),
        // `tmmaps striplightmap MAP --out F`: the ORIGINAL map with its stored
        // lightmap dropped (HasLightmaps = 0), blocks and items untouched — the
        // 2026-09-09 probe of whether the tiny maps' blown-out light spots are the
        // absent lightmap rather than the scaled lights
        "striplightmap" => surgery::striplightmap(&args),
        "itembytes" => surgery::itembytes(&args),
        "dropbaked" => surgery::dropbaked(&args),
        "movebaked" => surgery::movebaked(&args),
        "census" => census::cmd_census(&args),
        "skins" => census::cmd_skins(&args),
        "fillers" => tmmaps::fillers::cmd(&args),
        // archive-only variants of a tiny map for the load-failure bisect (zippad.rs)
        "zippad" => tmmaps::zippad::cmd(&args),
        "header" => header::cmd(&args),
        "dropscan" => dropscan::cmd(&args),
        "mediatracker" => tmmaps::mediatracker::cmd(&args),
        "chunks" => inspect::chunks(&args),
        "blockrefs" => inspect::blockrefs(&args),
        "genealogy" => inspect::genealogy(&args),
        "genealogy-cells" => inspect::genealogy_cells(&args),
        "colors" => inspect::colors(&args),
        "phases" => inspect::phases(&args),
        "help" | "--help" | "-h" => println!("{}", USAGE),
        other => {
            eprintln!("tmmaps: unknown subcommand `{}`\n\n{}", other, USAGE);
            std::process::exit(2);
        }
    }
}

const USAGE: &str = r#"tmmaps — TM2020 map surgery. `tools/ghost` owns ghosts and replays.

Times print as seconds with a decimal (16.316), never as raw milliseconds.

READING A MAP
  tmmaps waypoints MAP
        the map's waypoints: spawn, checkpoints, goal — block# / item# indices,
        tags, cells, free positions. These indices are what every mover takes.
  tmmaps census MAP [--filter PAT] [--free]
  tmmaps skins MAP [--name SUBSTR]      item placements carrying a skin FileRef (path, url) + per-model counts
        EVERY block, unbaked (0x0304301F) and BAKED (0x03043048), tagged U/B,
        with its free-block position when it has one, as TSV. `waypoints` and
        any single-chunk listing show only one of the two: across the store
        52.7 % of blocks are baked, and eleven maps read as near-empty without
        this (197047 showed 3 blocks of 2316). A baked index is counted in its
        OWN list, so a bare `2461` pasted from a census row addresses an
        unrelated unbaked block — movers spell baked indices `bN` and REFUSE
        them rather than moving the wrong block.
  tmmaps fillers MAP [--filter PAT] [--cells X0,Z0:X1,Z1] [--summary]
        the generated clip fillers (the game's own pillar walls, skirts, end
        caps: baked records) with their variant word (a0 Middle .. a4 nothing,
        g1 TopBottom_Ground), the side they stand on, what their own cell holds
        (`-` free, `P:` pillars only) and what stands across the side;
        --summary tallies free / pillar / occupied cells per name and variant
  tmmaps dropbaked SRC --out MAP [--baked b12,b40,…] [--name PAT[,PAT…]] [--flags HEX]
        the ORIGINAL minus exact generated records (by bN index, by name
        substring and/or an exact flags word) — the ground-truth probe: shoot
        SRC and MAP from one camera; no changed pixel ⇒ the engine draws
        nothing for those records
  tmmaps region MAP --box X0,Y0,Z0:X1,Y1,Z1 [--filter PAT] [--items] [--blocks]
        everything whose position lies inside a world box. A GATE IS A
        STRUCTURE, NOT A BLOCK: run this before and after any move.
  tmmaps phases MAP [--filter PAT] [--all]
        the per-item ANIMATION PHASE OFFSET (chunk 0x03043063, one byte per item
        in eighths of the period: 4 = half) — what the editor stores when the
        author phases a pusher/rotor/tube; non-zero items, or --filter/--all
  tmmaps chunks MAP [--only 0xCHUNK --hex N]
        every skippable body chunk with its size (--only/--hex: one chunk, head dump)
  tmmaps blockrefs MAP [--groups]
        every place the file lists blocks by index or per block (the snapped-on
        tables, free-block entries, colour/lightmap bytes, macroblock refs) —
        the audit behind `remove_blocks`
  tmmaps header MAP [MAP ...] [--tsv] [--xml] [--names]
        what the file DECLARES about itself before any block is read: container
        version, node count, EXTERNAL references, the header chunk table, the
        community XML (title, exever/exebuild, envir, maptype, validated), the
        declared `<dep>` files, the embedded-objects zip and its entries, and
        the block/item counts to set them against. `--tsv` is the corpus form —
        one row per map plus the distinct value of every column, because a
        difference only one map has is a lead and one several share is not.
        Written for 146612, which the engine loads and the editor will not open.
        `--names` is the IDENTITY form — `path uid name authorid authortime`,
        one row per map — for auditing what this repo publishes a map as
        against what the map calls itself. Join it on uid against
        trackmania.io; our own documents are not an independent check.

TINY MAPS (half-scale campaign: every authored block/item -> an embedded static item)
  tmmaps tiny MAP --mapping placements.tsv --library ITEMS.zip --out F [--scale 0.5]
      [--anchor x,y,z] [--host HOST.Map.Gbx] [--keep-ghost] [--name NAME | --keep-name] [--keep-zone-block]
        replace every authored block by its library item (mapping rows
        `@index<TAB>ITEM|-<TAB>model_scale<TAB>sx<TAB>sz`; `-` = intentionally
        nothing) and re-point/drop items (`i@index<TAB>ITEM|stock model|-`);
        the baked foundation stays. Inputs come from `mapgeom tiny-library`.
  tmmaps tiny-batch DIR --out DIR [--mapgeom BIN --paks "--pak F:HASH …"] [same flags]
                                                    every .Map.Gbx of a directory; with --mapgeom each map gets
                                                    its own item library (OUT/NAME.lib.zip, .placements.tsv, .report.tsv)
  tmmaps lineup MAP --out F --stock A,B,C --at X,Y,Z [--pitch 16] [--yaw R] [--items F.Item.Gbx,G.Item.Gbx] [--colors 2,3,…]
        the map plus a row of stock (pack) items by name — a vegetation species survey;
        --items continues the row with embedded item files (a test item next to its stock oracle);
        --colors gives each item of the row its placement colour byte (a stock flag at Green next to ours)
  tmmaps shared-cells MAP [--all] [--mapping placements.tsv]
        cells where a terrain tile shares its cell with another block, and whether the
        tile is hidden by that block in the tiny map (kept = a coplanar pair to watch)
  tmmaps tiny-catalog MAP --mapping T --library Z --out F [--only NAME] [--lineup A,B]
        one block per model beside its items (or the listed item files), for a look

CHANGING A MAP — position and ROTATION; no model swap, so no trigger volume changes
  tmmaps move MAP --out F --move SPEC [--move SPEC ...]
        SPEC is  N:cx,cy,cz[:dir]   a grid block, by world cell
                 N@x,y,z[/yaw]      a FREE block, in metres
                 iN@x,y,z[/yaw]     an item
                 bN                 a baked index — always refused, by name
        --cell is correct for either placement regime; --pos/@ is metres and is
        the only correct form for a free block. A free block ignores its cell
        bytes (its position is six f32 in chunk 0x0304305F), so a regime-blind
        cell write produces a map that loads, an origin control that passes,
        and a ladder in which every rung is silent.
  tmmaps rotate MAP --out F --rot BLK:yaw,pitch,roll [...]
                          --drot BLK:dyaw,dpitch,droll [...]
                          --tilt N,N,N --about X,Y,Z --dir DEG --angle RAD
        Tilt FREE blocks. A block's stored rotation turns it about ITS OWN
        ANCHOR, so giving every tile of a surface the same roll SHEARS it into a
        staircase (32 m tiles at 3.4 deg = a 1.9 m step per join, measured). The
        `--tilt` form is the honest one: one axis, position and rotation written
        together. REFUSES when a free block within --group-radius (4 m) is not in
        the rotation -- the ice kicker of 284238 is FOUR blocks sharing an anchor
        and two arms rotated one of them, which reads exactly like a null result.
  tmmaps shift MAP --out F --box X0,Y0,Z0:X1,Y1,Z1 --by DX,DY,DZ [--filter PAT]
        DISPLACE everything in the box, then re-read the written map and require
        each object to be exactly where it was sent. This is how you measure a
        clearance: move an obstacle by known amounts and ask the oracle which
        is the first that lets a tape through. A structure half-moved reads as
        a measurement, so nothing is half-moved.
  tmmaps clear MAP --out F --box X0,Y0,Z0:X1,Y1,Z1 --to X,Y,Z [--filter PAT]
        move EVERYTHING in the box, then re-read the written map and REQUIRE
        the box to be empty. This is the enforced form of the lesson below.
  tmmaps segat MAP --out F --promote W [--neutralise W,W,...] [--force-rename]
        ONE segment map, spelled out: W becomes the finish and each --neutralise
        waypoint stops being a checkpoint. Nothing is inferred and nothing is
        verified -- the caller owns the comparison. This is the only way to cut
        at a LINKED checkpoint (`segments` cannot enumerate one). Rules: every
        checkpoint at or after the cut must be neutralised, AND every other
        member of the cut's own linked group; a linked group EARLIER than the
        cut is left alone.
  tmmaps segments MAP --ref-ghost G [--out DIR] [--order W,W,...] [-j N]
        measure the checkpoint order, then build every segment map + a control.
        The order is MEASURED against the reference ghost and every round is
        checked against the ghost's own declared splits: the tool REFUSES, with
        the probe table, rather than reporting an order it cannot establish.
        Every built segment is re-validated against the ghost and must
        reproduce that checkpoint's declared split — exactly for a gate cut,
        early by <= 0.500 s for the block-rename fallback — and no two segment
        maps may share a decompressed body. Exit 2 on any refusal.
        --order gives the driving order instead of measuring it, as waypoint
        indices in driving order (`--order 439,494,440,633,492`); spell it
        `i439` / `b2089` when block and item indices collide.

MEASURING WITH A MAP
  tmmaps dropscan MAP --out DIR --tape GHOST (--cells CX:CX:S,CY,CZ:CZ:S | --cell
                  CX,CY,CZ[,DIR] | --tapes DIR) [--dirs D,..] [--target X,Y,Z]
                  [--jobs N] [--at frac:F,..] [--fk PATH] [--server DIR] [--keep]
        READ THE ENVIRONMENT'S GEOMETRY WITH THE CAR. Moves the spawn to a cell,
        drives the car off it under a fixed straight-throttle tape, and reads
        the landing out of the live engine (`fk trace`): one probe is one map
        plus one trace, and the summary is apex, resting place, closest approach
        to --target, and when. This is how you ask "what is over there" on a map
        whose surfaces belong to the DECORATION and not to the map file.
        --tapes DIR instead scores a POPULATION OF TAPES on the untouched map,
        with the same readout — the poor man's state objective.
        Controls, both refusals: the spawn moved to its OWN cell must reproduce
        the untouched map byte-for-byte, and a probe at the real spawn cell must
        start where the map's spawn is. A trace that is all zeroes is REFUSED:
        it passes fk's self-check and means the cell produced no car at all
        (every cell outside the map grid does).
  tmmaps ladder MAP --spec F --ghosts G... [-j N]
        arrival-time ladders. One rung per spec line, whitespace-separated
        moves, so a rung is a CURTAIN of gates across the corridor rather than
        one 32 m cell: a single-cell rung is silent for roughly a third of
        well-chosen placements. Aborts unless a rebuild at every gate's own
        origin reproduces the untouched map, and asserts N rungs produced N
        distinct file hashes.
  tmmaps rungspec TRAJ.csv --block N --from A --to B --step S [--offset dx,dy,dz]
        emit a ladder spec placing a gate ON a reference trajectory
  tmmaps oracle --map M --ghosts G... [--map M2 --ghosts ...] [--shard] [-j N]
        validate (map, ghosts) batches; one server per map, as required —
        every segment map keeps the original mapUid, so two of them can never
        share a UserData/Maps. --shard: one map, ghosts split over -j servers.
  tmmaps cporder MAP TRAJ.csv --splits A,B,C
        which waypoint produced which declared split, from one trajectory
        decode; reports match distance and runner-up so a bad match is visible

CONTROLS — run these before you trust a map-surgery result
  tmmaps selftest [--engine] [--strict]
        the whole suite in one command. --engine adds the dedicated server.
  tmmaps bodydiff STOCK EDITED
        what an edit changed, on the DECOMPRESSED bodies, attributed to the
        placement each differing byte belongs to — plus how much of the file
        itself the two share. A SPLICED edit differs in the bytes of the edit
        and nowhere else; a re-emitted map shares the header and then nothing.
  tmmaps rewrite MAP --out F [--reemit]
        write a map back with NO edit. The default output is the stock file
        byte for byte; --reemit rebuilds the compressed stream, which is what
        every map this project wrote before the splice path existed. The pair
        isolates the WRITER from the EDIT — the one question the dedicated
        server cannot be asked, because it accepts both.
  tmmaps roundtrip MAP
        parse and re-emit unchanged; compares DECOMPRESSED bodies (LZO is not
        bit-reproducible, so file hashes are the wrong level)
  tmmaps origin MAP
        return-to-origin at the byte level: every waypoint AND every item
        through its own mover with its own current placement, output required
        byte-identical. No oracle calls. This is what catches a mover writing
        dead bytes.
  tmmaps renamecheck MAP [--ghosts G...]
        the RENAME round-trip, which roundtrip cannot be: renames a waypoint
        four ways — including to ITSELF with byte-identical output required —
        forcing the two-region Id stream through the rename re-encoder.
        --ghosts also renames an off-route decoration and requires the control
        ghosts' times to be UNCHANGED. Required gate for any rename change.
        A `lookback table length N -> N+1 ... may not resolve` warning is
        CONSERVATIVE: it fires on maps whose game check then passes. Do not
        read it as a failure, and do not read it as an all-clear.

LESSONS THE CODE ENFORCES — see MAPS.md for the full list
  A GATE IS A STRUCTURE, NOT A BLOCK.  Moving GothMommy's added finish on
  173691 moved one unbaked anchor and left FIFTEEN baked `GateExpandable*`
  pieces standing in the landing zone; the run drove into them and stopped, and
  a human spotted it in the video before any instrument did. `region` counts
  what is there and `clear` refuses to succeed while anything is left.

  NEVER PROBE BY SWAPPING A GATE MODEL.  The old `gate` / `gateat` / `probe`
  commands relocated a waypoint by swapping its item model to `GateFinish32m`
  first. On 285885 that quadruples the trigger volume (the origin control then
  returns 50.589 instead of 61.229 — it fabricates discoveries); on 279197 it
  deletes a custom Goal item and everything DNFs. Those commands are deleted.
  Every mover here is position-only. `segments` still promotes a gate, because
  a promoted gate is a fine RULER — it is an unsafe OBJECTIVE.

  A MAP-SURGERY CONTROL CAN BE INERT.  Gate-removed, deck-removed and
  road-removed maps once all returned identical output; the road control proved
  the instrument was dead, not the maps identical. `ladder` requires N distinct
  file hashes for N rungs, and `oracle` refuses a candidate the server would
  silently skip (see below).

  THE SERVER IGNORES A FILE WITHOUT A `.Ghost.Gbx` / `.Replay.Gbx` SUFFIX and
  returns a plain DNF, indistinguishable from a run that did not finish. The
  oracle driver refuses such a path instead of staging it.

env: TMMAPS_DEBUG=1 (lookback table sizes)
     TMMAPS_DEBUG_NODES=1 (trace body node refs as INLINE / backref)
     TMMAPS_NO_BAKED=1 — A LANDMINE. Safe ONLY for position-only surgery: with
     it set the baked chunk is not parsed, so its Id words are not renumbered
     and any RENAME silently mis-encodes the map — and every baked block
     disappears from the census. Nothing has needed it since the
     shared-node-index fix.

exit: 0 ok · 1 a control failed · 2 usage · 3 refused (the command was wrong,
      and the message says what to do instead — never a backtrace)
"#;

