# The project's own file formats: the pipeline's TSVs, tapes, scripts and done-files

Everything the tools write for each other. None of it is a game format, but
every one is read by another tool and a wrong column is a silent misbuild.
Conventions shared by all of them: TAB-separated, `#` comment lines, times in
SECONDS with three decimals (`22.730`) except where a game grammar owns the
number, one row per thing, idempotent rewrites. Confidence **[FILE]** (these
are ours).

## 1. Ghost-side text forms (`tools/gbx`, `tools/ghost`, `tools/tmsite`, `tools/tmtraj`)

| form | grammar | producer / consumer |
|---|---|---|
| `.gtape` | `#gtape 1`, `#source PATH`, `#chunk_version N`, `@archive k format_version= field0= start_offset_ms= packets= bitstream_bytes= bits_used=`, `@tail <hex>`, then `t=<tick> mode=<m> w=lit:<hex>\|prev\|prev2:<x>,<y> respawn=<0\|1> mouse=none\|<a>,<b> vsame=<0\|1> steer=<i8> accel=<0\|1> brake=<0\|1> flags=<hex>` (`steer32=` / `tri=` for the other modes) | `ghost tape extract` ↔ `ghost tape inject` (byte-identical round trip); `ghost-input-tape.md` §3 |
| inputs CSV | `race_ms,steer,accel,brake` (ms because TICK owns the grammar downstream; the page's JS formats) | `ghost tape csv`; every map page's `inputs/` |
| TICK script | `# 2432 ticks, ghost start offset -1.580 s, declared 22.730 s` header; `<ms> <action> <value>` with `<ms>` a multiple of 10 and actions `steer` (−127..127 \| `left` \| `right`), `accel`, `brake`, `respawn` (0/1); `--seed N` optional | `tmsite tick` → TICK (the TAS tool); `tmsite verify` replays 227/227 exactly |
| event script | `<ms> press\|release <key>` with keys `gas brake left right`; the last state is held between events | `vidread ktevents` (off a video) → `ghost script` |
| carrier bytes | `carrier-bytes.tsv`: per sample byte, the affine fit / status against the server writer | `tools/fk`; `ghost-telemetry-cplugentrecorddata.md` §4 |
| oracle transcript | the server's JSON objects, one per file (`dedicated-server-oracle.md` §2) | `ghost::oracle` → `tools/testdata/oracle_transcript.json` |
| `tmtraj export` | CSV/TSV of decoded samples (`race_ms x y z speed …`), `--fields` picks columns; `tmtraj csv` of a route | `tmtraj`; pages, `mapgeom coverage` |

## 2. Map-side build files (`mapgeom tiny-library` → `tmmaps tiny`)

`<stem>.placements.tsv` — the MAPPING, one row per thing to place
(`tools/tmmaps/src/tiny/mapping.rs::read_mapping`):

```text
BLOCK<TAB>ITEM[<TAB>MODEL_SCALE[<TAB>SX<TAB>SZ[<TAB>UNITS[<TAB>AUTO_TERRAIN[<TAB>S]]]]]   a block NAME → item (ITEM `-` = no item, on purpose)
@INDEX<TAB>ITEM…          exact authored-block placement INDEX (index rows win over name rows)
b@INDEX<TAB>ITEM…         a BAKED (generated filler) record re-emitted as an item
i@INDEX<TAB>ITEM…         an existing item placement re-pointed at an embedded copy of its model
v@ALIAS<TAB>ITEM<TAB>X<TAB>Y<TAB>Z<TAB>YAW[<TAB>PITCH]   stock vegetation to add at every placement of ALIAS
y@INDEX<TAB>DY            metres an item placement is LOWERED after the transform (a stand-in tree)
yb@INDEX<TAB>DY           the same for the item of baked record INDEX (coplanar-sinks: 0.01)
iv@INDEX<TAB>VARIANT      the variant byte an item placement carries after its re-point
xv@N<TAB>K  xvb@N<TAB>K  xvi@N<TAB>K   drop the K-th v@ tree of authored block / baked block / cluster item N
xi@INDEX                  park the item placement itself (a stand-in tree over a deck)
xf@INDEX                  a converted flag with no hidden stock driver
```

`MODEL_SCALE` = the scale already baked into the item geometry (the placement
gets scale 1 and a halved pivot); `SX SZ` the variant's footprint in cells;
`UNITS` the variant's unit cells in the block's frame (the terrain tile is
hidden under EVERY one); `AUTO_TERRAIN` the variant's declared terrain tiles +
place type; `S` = a generated SIDE clip (hung on its owner's face).

`<stem>.report.tsv` — one row per model with its outcome, so a gap is
explicit, never silent (model, source file, kind, verdict). `lib.zip` — the
item library (`Items/AC…Item.Gbx`, `Items/*.dds`, `Blocks/Water/*.Block.Gbx`).
`build.log` / `tiny.log` — the two halves' logs; `libx/` the exploded
library. `recipe.env` — `TINY_*` knobs (`--env K=V` overrides).

## 3. Shooting and comparison (`tinyctl`, `shootctl`)

| file | grammar |
|---|---|
| `views.tsv` | `NAME<TAB>ox,oy,oz<TAB>DIST<TAB>H<TAB>V` — orbital camera: target, distance, horizontal and vertical angle in RADIANS (`H=π`: +x right; `H=0`: +x LEFT; `V>0` looks down); `tinyctl views SRC --out` writes the comparison cameras, `tinyctl shoot --views` shoots them on original and tiny (tiny at the anchor-transformed target, half distance) |
| `cam-ORIG.tsv` / `cam-TINY.tsv` | the intro camera per frame (`/camlog`), aligned on the first cut by `tinyctl camcheck --anchor sx,sy,sz:tx,ty,tz` |
| `cams.tsv` | per map the render camera (`CameraGame` id: 2 chase, 6 Ext2, 1 Internal, 3 Helico) |
| `cmp-*.jpg`, `<prefix>.tsv` | side-by-side original \| tiny, and one row per flagged cell of the grid diff (`tinyctl compare`) |
| `loadloop-<tag>.tsv` | `iter  map  outcome  seconds  car  dialog_frame  dialog_text  ctx  note` — one row per load of the reliability loop; with `loadloop.log`, `done-loadloop.txt` |
| `probe-out.tsv` | the game's item census after a load (`items=N loaded=K` in the shootset log) |
| `arg.txt` | the one-line path argument of every plugin route that takes a file (`openplanet-plugin-api.md` §1) |
| `STARTCHECK.tsv` | per map the client start check: car resting position vs the Spawn placement, PASS/FAIL (tolerance 12 m) |

## 4. Shipping a set (`tinyctl ship`, `tinyctl video`, `page-status`, `mapzips`) — files under `<out>/`

| file | grammar | meaning |
|---|---|---|
| `MANIFEST.txt` / `MANIFEST.tsv` | `map  MB  items  md5  collhash  fillers_left_out` + a header note | the certified set; the md5 of the exact file published, the collision fingerprint (`collision-cplugsurface.md` §6) |
| `ANCHORS.tsv` | per map the transform anchor `sx,sy,sz:tx,ty,tz` (+ scale) | what `tinyctl shoot`/`camcheck` need |
| `CHANGES.tsv` | per map what changed against the previous set | the drop's README row source |
| `holds.tsv` | `nn<TAB>reason[<TAB>render]` — a 2-digit map number, a reason (default `held`), an optional `render` mode | a HELD map is not published; `render` holds only the render |
| `approvals.tsv` | `nn<TAB>time` | vjeux's receipt that this map AND lap may go out; or the map is listed in `prechecked.tsv` |
| `prechecked.tsv` | `nn` per line | maps whose clips may go out without a per-lap receipt |
| `ships.tsv` | `nn<TAB>time<TAB>name<TAB>done_file<TAB>status` with status `staged` \| `pending` \| a URL | the box-side publish queue; `tinyctl shipwatch` polls the done-files and swaps the page row |
| `rowbuilds.tsv` | `nn<TAB>build<TAB>map_link<TAB>note` | the page row's build label and downloadable map link (`tinyctl mapzips` writes the link) |
| `lidrows.tsv` | `nn<TAB>note` (parsed like holds) | maps whose lap rides a lid (a water class A lap) — the note goes under the row |
| `REPORT.md` | one row per rendered lap: map, time, build, files | the render log |
| `done-*.txt` | the detached job's completion marker (exit status + summary line) | what `shipwatch`/`await` poll |
| `tiny-summer-2026-NN-<build>.zip` | the exact store map file + `README.txt` (build, md5, collhash) | the downloadable map |

## 5. Store layout (`~/persistent/private-30d/tm-player/tiny/`)

`sources/NN-Summer-2026---NN.Map.Gbx` (the 25 originals), `incoming/<set>/`
(a candidate set: the maps under their published names, `README.md`,
`MANIFEST.tsv`, `STARTCHECK.tsv`, `ANCHORS.tsv`, `CHANGES.tsv`, `colldiff/`),
`incoming/ghosts-for-video/*.Ghost.Gbx`, `incoming/ship16-diagnostics/`
(`ring-bisect/`, `waterbodies.txt`, `PHYSICS-CENSUS-ship16.tsv`, `SWEEP-TRIAGE`),
`patches/` (git patch series), `coordinator-bank/` (the bridge binaries). Never
scan it recursively (a `grep -r` over it times out).

## 6. Not covered

* The web page payloads (`tmsite`: full page JSON `{"time":19538}` and the
  compact page's base64 blob) — `tools/tmsite/README.md` owns them.
* The `fk` evidence tables (`tools/fk/evidence/`).
