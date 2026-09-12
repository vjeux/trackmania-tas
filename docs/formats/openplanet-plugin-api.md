# The Openplanet plugin ("GhostShooter") — the render box's HTTP API

`tools/openplanet-plugin/*.as` (AngelScript, Openplanet 1.29.x, dependencies
`NadeoServices`, `VehicleState`), served on `127.0.0.1:29800` inside the game
on the render box; every driver (`shootctl`, `tinyctl`, `clip`, `haul`) talks
to it, through the wsx bridge from the devservers. It is the project's
instrument for "what did the GAME make of this file": counts, positions,
wheel surfaces, the editor's own block/item lists, a lightmap compute, a
re-save. Confidence **[VERIFIED-GAME]** (these are the game's own answers);
the route list is from `Main.as::RouteRequests` at main `62352d44`.

## 1. Conventions

* GET, plain text or TSV or JSON back; a query string for short arguments.
  **Paths never travel in the query string**: the caller writes
  `OpenplanetNext/PluginStorage/GhostShooter/arg.txt` (via the box's
  `/mnt/c/...`), then calls the route (`PathArg()`); the game cannot open
  `/home/...` paths (`/mnt/c/...` or `C:/...` only; `Maps\_shoot` is a junction
  to `C:\tm\_shoot` because OneDrive skips reparse points).
* Big answers go to files in the plugin's storage folder (`probe-out.tsv`, the
  item census; `cam.txt`, camera aiming).
* A plugin compile error takes the whole plugin down and `shootctl install`
  rolls back to the last good copy AND restarts the game; a failed install
  restarts the game (a loaded map is lost). Openplanet 1.29.14:
  `Viewport.Cameras[0].NextLocation` + `Fov` work; there is no `Camera::`
  namespace.
* Context (`/ctx`): `0` = no playground (NOT necessarily the menu: a running
  playground read as 0 for minutes; the title API `IsReady` via `/ready` is
  the menu proof), `1` track editor, `2` MediaTracker, `3` playground, `9`
  other editor. 0.3 s after `/playmap` the game shows a TRANSIENT ctx 3 /
  `playground:true` / `map:null` and drops to 0 for the load — require ctx 3
  WITH `RootMap` set stable for ~2 s. The playground is open when ctx 3 holds
  1 s AND `/wheels` answers with a car row; the vehicle state is null through
  the intro (`--wheels-ms 15000`). A fresh-uid load can hang on an `Updating
  data… $<$> (???)` wait dialog (`FrameAskYesNo`, empty text): retry with a
  fresh uid; `/dismiss` answers dialogs; a PASSWORD popup on the main menu is
  not a `CGameDialogs` frame — `shootctl key ESC`.
* A crash reads at the socket as `Connection reset by peer`, indistinguishable
  from a plugin reload; check the game's PID either side.

## 2. Routes, grouped

| group | routes | notes |
|---|---|---|
| liveness / context | `/ping` → `pong`; `/ctx`; `/ready`; `/state`; `/whoapp`; `/build` | |
| open things from a file (`arg.txt`) | `/editmap` (track editor), `/editmap2?dec=` (with a decoration), `/editmap3?adv=1`, `/playmap?mode=` (play), `/editghosts2`, `/editreplay?kind=`, `/import`, `/importfile`, `/importghosts`, `/importok` | play mode from the STORED file refuses foreign stock names where the editor does not |
| wait | `/await`, `/awaitfile`, `/loaded` | |
| the game's own view of the loaded map | `/mapitems[?name=&x0=…]` (items the game KEPT; `items=N loaded=K`, K below the placement count = drops), `/mapblocks`, `/mapblocks2?list=baked\|blocks` (the editor's Blocks/BakedBlocks with cell/dir/mobil/ground/ghost/colour — the yardstick for the bake algorithm), `/mapgates`, `/fids` (the virtual file tree: `MemoryTemp\CurrentMap_EmbeddedFiles\ContentLoaded\Items\`), `/mapstate`, `/mobils` (`HackScene.Mobils`: only 18 in the editor — block geometry is static batching), `/meshflags[?set=1]` (the tween bit of every item mesh), `/members`, `/layers`, `/findnod`, `/greplayers` | |
| save / validate | `/mapsave` (`PluginMapType.SaveMap(path)`: renames the map to the file's stem; drops loose `.dds`, the validation ghost, re-mints the uid — `map-lightmap.md` §2), `/mapvalidate`, `/savepath`, `/saverefresh`, `/savedlg`, `/savevalidate`, `/passcancel` | |
| lightmap | `/shadows?q=N` (`ComputeShadows1(EShadowsQuality)`: q=3 Default ~90 s, q=4 High ~7 min on 8022 items; reuses the cache by CONTENT and returns immediately when it can), `/shadowsq` (`ready`/`busy`; accept `ready` only after `busy`) | |
| the live car | `/car` → `wall_ms  race_ms  x y z  vx vy vz  speed  nWaypoints` (`CSmScriptPlayer`); `/carlog?ms=` a trajectory (5 s slices; a 24 s request never returned); `/respawn?name=` | `CurrentRaceTime` stays 0 in a `/playmap` playground |
| the wheels | `/wheel`, `/wheels?ms=` → TSV `wall_ms t_ms x y z vx vy vz frontspeed gas brake steer ground fl fr rl rr flslip frslip rlslip rrslip fldamp frdamp rldamp rrdamp gear grounddist` (`VehicleState::ViewingPlayerState()`; `fl..rr` = `EPlugSurfaceMaterialId`, 80 = no contact); during a ghost render it reads the editor's parked car, not the ghost | `collision-cplugsurface.md` §5 |
| cameras | `/cam` (dump), `/camstate`, `/camset`, `/camlog`, `/clipcam`, `/clipend`, `/freelook` (switch the editor's cursor preview off after every load), `/cursor` (the editor block cursor, `{"coord":[32,5,32]}`, red = cannot place) | angles in RADIANS; camera = target + dist·(sin h, ·, cos h); v > 0 looks down; the orbital camera will not go under ~10 m from its target; the opening fly-in overrides a camera written under it (wait ≥ 4 s, aim twice) |
| MediaTracker | `/mtclip`, `/mtclips`, `/mtclip1`, `/mtflags`, `/mtset`, `/mtui?hide=1`, `/mt2`, `/mtingame`, `/mtquit`, `/cantrack?type=N`, `/mktrack?type=N` (33 = OpponentVisibility), `/rmtracks`, `/rewind` `/play` `/stop`, `/select?t=&b=&k=`, `/ourclip`, `/setclip`, `/tree`, `/dialogtree`, `/edtree` | `MtProbe.as`: the MT editor's camera/trigger switches |
| shooting | `/shoot` (`ShootVideo()`), `/shootsetup`, `/shootparams`, `/shootok`, `/shootcancel`, `/shootstatus`, `/renderprobe` | frames 3840×2160 PNG (~9 MB each); the render camera is the game's `CameraGame` block by id (2 chase default, 6 Ext2, 1 Internal, 3 Helico) |
| the held run | `/authghost[?clear=]` (`RaceValidateGhost` + `AuthorTime` of the loaded map), `/refghost`, `/refsave?name=`, `/refupload`, `/ghostsink` (`RefGhost.as`: the run the client holds for the loaded map, written out by the game) | `RaceValidateGhost` and `Map_GetAuthorGhost` are NULL in the editor and the MT on these maps — the disk file (`ghost-caches.md`) is the source |
| dialogs / menus | `/yes`, `/no`, `/dlgok`, `/dlghide`, `/dismiss`, `/dlgtext`, `/dlgstring`, `/setdlgstring`, `/dlgnods`, `/focusdlg`, `/menus`, `/menutree`, `/rmenu`, `/rselall`, `/rok`, `/rrefresh`, `/back` | |
| Nadeo services | `/nadeoauth`, `/nadeotoken` (re-mints the game's own token: it rotates ~hourly → HTTP 401 on GET /maps means retry), `/nadeoget`, `/nadeopost` (`arg.txt` line 1 = URL, rest = JSON body), `/nadeowho` | the upload cap is 25 MiB; `POST /maps/{mapId}` re-uploads in place |
| plugin | `/reload` (self-reload armed in the handler, performed in `Update()` so the response is on the wire first) | |

## 3. Facts the plugin measured that live in other pages

The client keeps 2252 of 2259 items (duplicates dropped); an embedded item
dropped at instantiation is simply absent from `/mapitems`; the editor's
`Blocks` list = authored + `Grass` tiles, fillers in `BakedBlocks` (original
10: 5817 = 3513 + 2304, +6 placeholders at (−1,0,−1)); block colour and
mobil/variant bits match the file on 7280/7281 records; the map's item count
under `/mapitems` is the tell for every silent drop rule
(`map-embedded-objects.md` §3).

## 4. Not known / not built

* No route reads the lightmap chunk back or the water volumes; no route
  exposes `CGameCtnChallenge.CopperPrice` (the render oracle idea) yet.
