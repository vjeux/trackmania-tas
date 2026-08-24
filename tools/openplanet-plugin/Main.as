// GhostShooter -- HTTP-controlled MediaTracker automation for TAS ghost video rendering.
//
// WHY THIS EXISTS IN THIS FORM: the pipeline was originally driven by OCR +
// synthetic clicks, and essentially every failure came from that layer. The
// MediaTracker plugin API can do the same work directly and has never once
// given a wrong answer. These endpoints move the work off the pixels.
//
// Endpoints (GET, localhost:29800):
//   /ping                      -> "pong"
//   /state                     -> JSON: editor type, MT clip tracks, timer, playing
//   /tree /dialogtree          -> control-id dumps (debugging)
//   /mtclip                    -> JSON of MediaTracker clip tracks/blocks
//   /cantrack?type=N           -> can a track of block-type N be created?
//   /mktrack?type=N            -> create one (33 = OpponentVisibility)
//   /rmtracks                  -> RemoveAllTracks()
//   /rewind /play /stop        -> transport
//   /shoot                     -> ShootVideo()
//   /importghosts              -> open the Import Ghosts dialog
//   /select?t=&b=&k=           -> SelectItem(track, block, key)

HttpServer@ server = null;
const uint16 PORT = 29800;

void Main() { StartServer(); }

void StartServer() {
    if (server !is null) return;
    @server = HttpServer("0.0.0.0", PORT);
    @server.RequestHandler = RouteRequests;
    server.StartServer();
    trace("[GhostShooter] server started on 127.0.0.1:" + PORT);
}

CGameEditorMediaTrackerPluginAPI@ MTApi() {
    auto mt = cast<CGameEditorMediaTracker>(GetApp().Editor);
    if (mt is null) return null;
    return cast<CGameEditorMediaTrackerPluginAPI>(mt.PluginAPI);
}

// crude query-string reader: /foo?a=1&b=2
string QArg(const string &in qs, const string &in key) {
    auto parts = qs.Split("&");
    for (uint i = 0; i < parts.Length; i++) {
        auto kv = parts[i].Split("=");
        if (kv.Length == 2 && kv[0] == key) return kv[1];
    }
    return "";
}

HttpResponse@ RouteRequests(const string &in type, const string &in route, dictionary@ headers, MemoryBuffer@ body) {
    string r = route;
    int q = r.IndexOf("?");
    string qs = "";
    if (q >= 0) { qs = r.SubStr(q+1); r = r.SubStr(0, q); }

    if (r == "/ping") return HttpResponse(200, "pong");
    if (r == "/build") return HttpResponse(200, "B1787429440");
    if (r == "/await") return HttpResponse(200, Await(QArg(qs,"c"), Text::ParseInt(QArg(qs,"ms"))));
    if (r == "/loaded") return HttpResponse(200, LoadedMap());
    if (r == "/awaitfile") return HttpResponse(200, AwaitFileFree(PathArg(), Text::ParseInt(QArg(qs,"ms"))));
    if (r == "/greplayers") return HttpResponse(200, GrepLayers(PathArg()));
    if (r == "/whoapp") return HttpResponse(200, WhoIsManiaApp());
    if (r == "/layers") return HttpResponse(200, DumpLayers());
    if (r == "/findnod") return HttpResponse(200, FindNod(PathArg()));
    if (r == "/dlgtext") return HttpResponse(200, DialogText());
    if (r == "/setdlgstring") return HttpResponse(200, SetDialogString(PathArg()));
    if (r == "/dlgstring") return HttpResponse(200, DialogString());
    if (r == "/edtree") return HttpResponse(200, EditorTree(PathArg()));
    if (r == "/focusdlg") return HttpResponse(200, FocusDialogs());
    if (r == "/renderprobe") return HttpResponse(200, RenderProbe());
    if (r == "/shootsetup") return HttpResponse(200, ShootSetup(PathArg(),
            Text::ParseInt(QArg(qs,"fps")), Text::ParseInt(QArg(qs,"w")),
            Text::ParseInt(QArg(qs,"h")), Text::ParseInt(QArg(qs,"ext"))));
    if (r == "/menus") return HttpResponse(200, MenuReport());
    if (r == "/menutree") return HttpResponse(200, MenuTree(PathArg()));
    if (r == "/dlgnods") return HttpResponse(200, DialogNods(PathArg()));
    if (r == "/shootparams") return HttpResponse(200, ShootParamsState());
    if (r == "/shootok") return HttpResponse(200, ShootOk());
    if (r == "/shootcancel") return HttpResponse(200, ShootCancel());
    if (r == "/shootstatus") return HttpResponse(200, ShootStatus());
    if (r == "/mtquit") return HttpResponse(200, QuitMT());
    if (r == "/import") return HttpResponse(200, ImportGhostRel(PathArg()));
    if (r == "/importfile") return HttpResponse(200, ImportGhostFile(PathArg()));
    if (r == "/savepath") return HttpResponse(200, SaveAsSetPath(PathArg()));
    if (r == "/saverefresh") return HttpResponse(200, SaveAsRefresh());
    if (r == "/savedlg") return HttpResponse(200, SaveAsState());
    if (r == "/savevalidate") return HttpResponse(200, SaveAsValidate());
    if (r == "/rmenu") return HttpResponse(200, ReplayMenuState());
    if (r == "/rselall") return HttpResponse(200, ReplaySelectAll());
    if (r == "/rok") return HttpResponse(200, ReplayOk());
    if (r == "/rrefresh") return HttpResponse(200, ReplayRefresh());
    if (r == "/camstate") return HttpResponse(200, CameraState());
    if (r == "/clipend") return HttpResponse(200, ClipEnd(Text::ParseInt(QArg(qs,"ms"))));
    if (r == "/camset") return HttpResponse(200, CameraSet(Text::ParseInt(QArg(qs,"ent")), Text::ParseInt(QArg(qs,"cam"))));
    if (r == "/mt2") return HttpResponse(200, OpenMediaTracker());
    if (r == "/reload") return HttpResponse(200, ArmReload());
    if (r == "/members") return HttpResponse(200, DumpMembers(QArg(qs, "t")));
    if (r == "/state") return HttpResponse(200, GetState());
    if (r == "/tree") return HttpResponse(200, DumpTree());
    if (r == "/dialogtree") return HttpResponse(200, DumpDialogTree());
    if (r == "/mtclip") return HttpResponse(200, DumpMTClip());
    if (r == "/yes")    return HttpResponse(200, AnswerDialog("yes"));
    if (r == "/no")     return HttpResponse(200, AnswerDialog("no"));
    if (r == "/dlgok")  return HttpResponse(200, AnswerDialog("ok"));
    if (r == "/dlghide") return HttpResponse(200, AnswerDialog("hide"));
    if (r == "/dismiss") return HttpResponse(200, DismissDialog());
    if (r == "/ready")   return HttpResponse(200, TitleReady());
    if (r == "/ctx")      return HttpResponse(200, GetCtx());
    if (r == "/back")     return HttpResponse(200, GoBackToMenu());
    if (r == "/mtingame") return HttpResponse(200, OpenMTInGame());
    if (r == "/cam")      return HttpResponse(200, DumpCameras());
    if (r == "/editmap")  return HttpResponse(200, EditMapFromFile());
    if (r == "/playmap")  return HttpResponse(200, PlayMapFromFile(QArg(qs, "mode")));
    if (r == "/editmap2") return HttpResponse(200, EditMap2FromFile(QArg(qs, "dec")));
    if (r == "/editghosts2") return HttpResponse(200, EditGhostsFromFile());
    if (r == "/editmap3") return HttpResponse(200, EditMap3FromFile(QArg(qs, "adv") == "1"));
    if (r == "/editreplay") return HttpResponse(200, EditReplayFromFile(QArg(qs, "kind")));

    auto api = MTApi();
    if (r == "/cantrack") {
        if (api is null) return HttpResponse(500, "not in MediaTracker");
        int t = Text::ParseInt(QArg(qs, "type"));
        auto bt = CGameEditorMediaTrackerPluginAPI::EMediaTrackerBlockType(t);
        return HttpResponse(200, api.CanCreateTrack(bt) ? "yes" : "no");
    }
    if (r == "/mktrack") {
        if (api is null) return HttpResponse(500, "not in MediaTracker");
        int t = Text::ParseInt(QArg(qs, "type"));
        auto bt = CGameEditorMediaTrackerPluginAPI::EMediaTrackerBlockType(t);
        if (!api.CanCreateTrack(bt)) return HttpResponse(409, "CanCreateTrack=false for type " + t);
        uint before = (api.Clip !is null) ? api.Clip.Tracks.Length : 0;
        api.CreateTrack(bt);
        uint after = (api.Clip !is null) ? api.Clip.Tracks.Length : 0;
        return HttpResponse(200, "tracks " + before + " -> " + after);
    }
    if (r == "/rmtracks") {
        if (api is null) return HttpResponse(500, "not in MediaTracker");
        api.RemoveAllTracks();
        return HttpResponse(200, "tracks now " + ((api.Clip !is null) ? api.Clip.Tracks.Length : 0));
    }
    if (r == "/rewind")  { if (api is null) return HttpResponse(500, "not MT"); api.Rewind();    return HttpResponse(200, "ok"); }
    if (r == "/play")    { if (api is null) return HttpResponse(500, "not MT"); api.TimePlay();  return HttpResponse(200, "ok"); }
    if (r == "/stop")    { if (api is null) return HttpResponse(500, "not MT"); api.TimeStop();  return HttpResponse(200, "ok"); }
    if (r == "/shoot")   { if (api is null) return HttpResponse(500, "not MT"); api.ShootVideo(); return HttpResponse(200, "ok"); }
    if (r == "/importghosts") { if (api is null) return HttpResponse(500, "not MT"); api.ImportGhosts(); return HttpResponse(200, "ok"); }
    if (r == "/importok")     { if (api is null) return HttpResponse(500, "not MT"); api.ImportGhosts_OnOk(); return HttpResponse(200, "ok"); }
    if (r == "/select") {
        if (api is null) return HttpResponse(500, "not MT");
        api.SelectItem(Text::ParseInt(QArg(qs,"t")), Text::ParseInt(QArg(qs,"b")), Text::ParseInt(QArg(qs,"k")));
        return HttpResponse(200, "sel t=" + api.GetSelectedTrack() + " b=" + api.GetSelectedBlock() + " k=" + api.GetSelectedKey());
    }
    return HttpResponse(404, "no route " + r);
}

string GetState() {
    auto app = GetApp();
    auto mt = cast<CGameEditorMediaTracker>(app.Editor);
    string j = "{";
    j += "\"editorNull\":" + (app.Editor is null ? "true" : "false");
    j += ",\"isMT\":" + (mt is null ? "false" : "true");
    if (mt !is null) {
        auto api = cast<CGameEditorMediaTrackerPluginAPI>(mt.PluginAPI);
        if (api !is null) {
            j += ",\"hasAPI\":true";
            auto clip = api.Clip;
            if (clip !is null) {
                j += ",\"clipName\":\"" + clip.Name + "\"";
                j += ",\"tracks\":" + clip.Tracks.Length;
                uint ghosts = 0;
                for (uint i = 0; i < clip.Tracks.Length; i++) {
                    auto tr = clip.Tracks[i];
                    for (uint b = 0; b < tr.Blocks.Length; b++) {
                        if (cast<CGameCtnMediaBlockEntity>(tr.Blocks[b]) !is null) ghosts++;
                    }
                }
                j += ",\"ghostBlocks\":" + ghosts;
            } else { j += ",\"clip\":null"; }
            j += ",\"timer\":" + api.CurrentTimer;
        } else { j += ",\"hasAPI\":false"; }
    }
    auto dlg = app.BasicDialogs;
    if (dlg !is null && dlg.Dialogs !is null && dlg.Dialogs.CurrentFrame !is null) {
        j += ",\"dialog\":\"" + dlg.Dialogs.CurrentFrame.IdName + "\"";
    } else { j += ",\"dialog\":null"; }
    j += "}";
    return j;
}

void WalkControl(CControlBase@ c, int depth, string &out sb, int maxDepth) {
    if (c is null) return;
    string indent = "";
    for (int i = 0; i < depth; i++) indent += "  ";
    sb += indent + c.IdName + "  [" + Reflection::TypeOf(c).Name + "]\n";
    if (depth >= maxDepth) return;
    auto cont = cast<CControlContainer>(c);
    if (cont !is null)
        for (uint i = 0; i < cont.Childs.Length; i++) WalkControl(cont.Childs[i], depth+1, sb, maxDepth);
}

string DumpTree() {
    auto editor = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (editor is null) return "no CGameCtnEditorFree";
    auto root = editor.EditorInterface.InterfaceRoot;
    if (root is null) return "no InterfaceRoot";
    string sb = ""; WalkControl(root, 0, sb, 6);
    IO::File f(IO::FromStorageFolder("tree.txt"), IO::FileMode::Write); f.Write(sb); f.Close();
    return sb;
}

string DumpDialogTree() {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null || dlg.Dialogs is null || dlg.Dialogs.CurrentFrame is null) return "no current dialog frame";
    string sb = ""; WalkControl(dlg.Dialogs.CurrentFrame, 0, sb, 8);
    IO::File f(IO::FromStorageFolder("dialog.txt"), IO::FileMode::Write); f.Write(sb); f.Close();
    return sb;
}

string DumpMTClip() {
    auto api = MTApi();
    if (api is null) return "not MT editor";
    auto clip = api.Clip;
    if (clip is null) return "no clip";
    string sb = "clip=" + clip.Name + " tracks=" + clip.Tracks.Length + "\n";
    for (uint i = 0; i < clip.Tracks.Length; i++) {
        auto tr = clip.Tracks[i];
        sb += "  track[" + i + "] name=" + tr.Name + " blocks=" + tr.Blocks.Length + "\n";
        for (uint b = 0; b < tr.Blocks.Length; b++) {
            auto blk = tr.Blocks[b];
            sb += "    block[" + b + "] " + Reflection::TypeOf(blk).Name
                + " start=" + blk.Start + " end=" + blk.End
                + " active=" + (blk.IsActive ? "1" : "0")
                + " keys=" + blk.GetKeysCount() + "\n";
        }
    }
    return sb;
}
// ---- v2 routes: everything the pixel layer used to do ----------------------
//
// The screen, the map, the MediaTracker and the camera are all reachable through
// the game's own object graph. Each of these replaces an OCR read or a blind
// click that failed in production today.
//
// GetApp() is typed CGameCtnApp; the menu manager, the title-control API and
// BackToMainMenu live on CGameManiaPlanet, so cast once and check.

CGameManiaPlanet@ MP() {
    return cast<CGameManiaPlanet>(GetApp());
}

// Where are we? A NUMBER, from the object graph -- not a guess from pixels.
//   0 menu   1 track editor   2 mediatracker   3 in a race   9 unknown
string GetCtx() {
    auto app = GetApp();
    int ctx = 9;
    string ed = "none";
    if (app.Editor is null) {
        ctx = (app.CurrentPlayground is null) ? 0 : 3;
    } else {
        auto mt = cast<CGameEditorMediaTracker>(app.Editor);
        auto fr = cast<CGameCtnEditorFree>(app.Editor);
        if (mt !is null) { ctx = 2; ed = "mediatracker"; }
        else if (fr !is null) { ctx = 1; ed = "trackeditor"; }
        else { ctx = 9; ed = Reflection::TypeOf(app.Editor).Name; }
    }
    string j = "{\"ctx\":" + ctx + ",\"editor\":\"" + ed + "\"";
    j += ",\"playground\":" + (app.CurrentPlayground is null ? "false" : "true");
    auto map = app.RootMap;
    j += ",\"map\":" + (map is null ? "null" : "\"" + map.MapName + "\"");
    auto dlg = app.BasicDialogs;
    if (dlg !is null && dlg.Dialogs !is null && dlg.Dialogs.CurrentFrame !is null)
        j += ",\"dialog\":\"" + dlg.Dialogs.CurrentFrame.IdName + "\"";
    else j += ",\"dialog\":null";
    j += "}";
    return j;
}

// Load a map straight into the editor. Replaces four menu clicks plus a tile
// hunt through a grid that only ever showed 12 of 32 maps.
//
// The path comes from a FILE, not the query string: map paths carry backslashes
// and spaces, and a hand-rolled URL decoder is one more thing to get wrong.
string EditMapFromFile() {
    auto mp = MP();
    if (mp is null) return "no CGameManiaPlanet";
    if (GetApp().Editor !is null) return "already in an editor - /back first";
    auto tc = mp.ManiaTitleControlScriptAPI;
    if (tc is null) return "no ManiaTitleControlScriptAPI";
    string p;
    if (!IO::FileExists(IO::FromStorageFolder("editmap.txt"))) return "no editmap.txt";
    IO::File f(IO::FromStorageFolder("editmap.txt"), IO::FileMode::Read);
    p = f.ReadToEnd().Trim();
    f.Close();
    if (p == "") return "editmap.txt is empty";
    tc.EditMap(p, "", "");
    return "ok " + p;
}

// Load the same map the same way, but into PLAY instead of the editor.
//
// WHY THIS EXISTS. 146612 ("Spaghetti Nights 2") is loaded and simulated by the
// dedicated server and never opens in this client's editor: EditMap returns and
// `ctx` sits at 0 forever. That is two different claims -- "the editor rejects
// this map" and "this CLIENT cannot load this map at all" -- and EditMap alone
// cannot tell them apart. PlayMap is the same title API, the same path, the
// same file, and a different loader; whichever way it comes out is a fact about
// where the fault lives.
//
// `mode` is the query argument because it is plain ascii; the PATH still comes
// from editmap.txt, for the reason above it. An empty mode is what the game
// uses for the map's own declared mode.
string PlayMapFromFile(const string &in mode) {
    auto mp = MP();
    if (mp is null) return "no CGameManiaPlanet";
    if (GetApp().Editor !is null) return "already in an editor - /back first";
    auto tc = mp.ManiaTitleControlScriptAPI;
    if (tc is null) return "no ManiaTitleControlScriptAPI";
    if (!IO::FileExists(IO::FromStorageFolder("editmap.txt"))) return "no editmap.txt";
    IO::File f(IO::FromStorageFolder("editmap.txt"), IO::FileMode::Read);
    string p = f.ReadToEnd().Trim();
    f.Close();
    if (p == "") return "editmap.txt is empty";
    tc.PlayMap(p, mode, "");
    return "ok mode=\"" + mode + "\" " + p;
}

// The map path, from the same file EditMap reads. One reader, one trap avoided.
string MapPathFromFile() {
    if (!IO::FileExists(IO::FromStorageFolder("editmap.txt"))) return "";
    IO::File f(IO::FromStorageFolder("editmap.txt"), IO::FileMode::Read);
    string p = f.ReadToEnd().Trim();
    f.Close();
    return p;
}

// `EditMap2` — EditMap with the DECORATION named explicitly.
//
// 146612 hangs in EditMap (ctx 0 forever, IsReady false forever, LatestResult
// Success, no dialog, 60 MB of memory and then nothing, while the same map
// through PlayMap is in a playground in 6.2 s). The decoration is one of the
// few things the editor entry resolves that the play entry does not have to,
// and this map's is the unusual "Sunrise (no stadium)" — so: name it, and name
// a different one, and see whether either changes the answer.
string EditMap2FromFile(const string &in dec) {
    auto mp = MP();
    if (mp is null) return "no CGameManiaPlanet";
    if (GetApp().Editor !is null) return "already in an editor - /back first";
    auto tc = mp.ManiaTitleControlScriptAPI;
    if (tc is null) return "no ManiaTitleControlScriptAPI";
    string p = MapPathFromFile();
    if (p == "") return "no editmap.txt";
    tc.EditMap2(p, dec, "", "", "", "");
    return "ok dec=\"" + dec + "\" " + p;
}

// `EditGhosts(Map)` — the title API's OTHER way into a map's MediaTracker, and
// the one that matters here: it is not the track editor, so a map whose track
// editor will not open may still be filmable through it. (MEASURED 2026-08-24:
// it loads 146612 in 5.7 s — but into a PLAYGROUND, ctx 3, exactly like
// PlayMap, so it is not a MediaTracker door after all.)
string EditGhostsFromFile() {
    auto mp = MP();
    if (mp is null) return "no CGameManiaPlanet";
    if (GetApp().Editor !is null) return "already in an editor - /back first";
    auto tc = mp.ManiaTitleControlScriptAPI;
    if (tc is null) return "no ManiaTitleControlScriptAPI";
    string p = MapPathFromFile();
    if (p == "") return "no editmap.txt";
    tc.EditGhosts(p);
    return "ok " + p;
}

// `EditMap3` — the same load with `UpgradeToAdvancedEditor` given explicitly.
// TM2020 has a simple and an advanced track editor, and which one a map is
// opened into is decided during the entry that hangs on 146612. If the choice
// is the trigger, this says so in one call.
string EditMap3FromFile(bool advanced) {
    auto mp = MP();
    if (mp is null) return "no CGameManiaPlanet";
    if (GetApp().Editor !is null) return "already in an editor - /back first";
    auto tc = mp.ManiaTitleControlScriptAPI;
    if (tc is null) return "no ManiaTitleControlScriptAPI";
    string p = MapPathFromFile();
    if (p == "") return "no editmap.txt";
    tc.EditMap3(p, "", "", "", "", "", advanced);
    return "ok advanced=" + (advanced ? "1" : "0") + " " + p;
}

// `EditReplay2(ReplayList, EReplayEditType)` — open a REPLAY in the editor, and
// with `Shoot` that is the MediaTracker.
//
// WHY THIS MATTERS BEYOND ONE MAP. Everything in this pipeline reaches the
// MediaTracker through the TRACK EDITOR: `EditMap` to ctx 1, then
// `EditMediatrackIngame()`. 146612 has no track editor — `EditMap` hangs
// forever on it while `PlayMap` and `EditGhosts` load the same file in about
// 6 s — so that map is unfilmable by the only route the pipeline has. This is
// a second door, and it does not depend on the track editor at all.
//
// The path is the REPLAY, read from `editmap.txt` like every other path here,
// and it is RELATIVE to the Replays folder (the same rule the ghost import
// follows; a full C:/ path is accepted by the field and loads nothing).
// `kind`: shoot (2) | edit (0) | view (1).
string EditReplayFromFile(const string &in kind) {
    auto mp = MP();
    if (mp is null) return "no CGameManiaPlanet";
    if (GetApp().Editor !is null) return "already in an editor - /back first";
    auto tc = mp.ManiaTitleControlScriptAPI;
    if (tc is null) return "no ManiaTitleControlScriptAPI";
    string p = MapPathFromFile();
    if (p == "") return "no editmap.txt";
    MwFastBuffer<wstring> list;
    list.Add(p);
    auto t = CGameManiaTitleControlScriptAPI::EReplayEditType::Shoot;
    if (kind == "edit") t = CGameManiaTitleControlScriptAPI::EReplayEditType::Edit;
    if (kind == "view") t = CGameManiaTitleControlScriptAPI::EReplayEditType::View;
    tc.EditReplay2(list, t);
    return "ok kind=" + kind + " n=" + list.Length + " " + p;
}

// Open the In Game MediaTracker sequence -- the call the "EDIT" button makes.
string OpenMTInGame() {
    auto mp = MP();
    if (mp is null) return "no CGameManiaPlanet";
    auto menus = mp.MenuManager;
    if (menus is null) return "no MenuManager";
    menus.DialogEditCutScenes_OnInGameEdit();
    return "ok";
}

string GoBackToMenu() {
    auto mp = MP();
    if (mp is null) return "no CGameManiaPlanet";
    mp.BackToMainMenu();
    return "ok";
}

// Camera blocks as NUMBERS. Replaces reading the camera panel with OCR -- which
// is how a camera aimed at a driver who wasn't there produced hours of entirely
// black renders that passed every size and duration check.
//   gamecam: 0 Default 1 Internal 2 External 3 Helico 4 Free 5 Spectator 6 Ext2
string DumpCameras() {
    auto api = MTApi();
    if (api is null) return "not MT editor";
    auto clip = api.Clip;
    if (clip is null) return "no clip";
    string sb = "";
    for (uint i = 0; i < clip.Tracks.Length; i++) {
        auto tr = clip.Tracks[i];
        for (uint b = 0; b < tr.Blocks.Length; b++) {
            auto cg = cast<CGameCtnMediaBlockCameraGame>(tr.Blocks[b]);
            if (cg is null) continue;
            sb += "cam track=" + i + " block=" + b
                + " gamecam=" + int(cg.GameCam)
                + " entid=" + cg.ClipEntId
                + " target=" + cg.TargetClipEntName + "\n";
        }
    }
    if (sb == "") sb = "no camera blocks\n";
    return sb;
}

// Answer the game's own modal dialogs. The "map has been modified, really
// quit?" FrameAskYesNo used to be a blind click at (1963,1006); the yes/no/ok
// handlers are on CGameDialogs, so the answer is an API call and the question
// is already reported by /state and /ctx.
string AnswerDialog(const string &in what) {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "no BasicDialogs";
    if (what == "yes")    { dlg.AskYesNo_Yes();  return "ok yes"; }
    if (what == "no")     { dlg.AskYesNo_No();   return "ok no"; }
    if (what == "cancel") { dlg.AskYesNo_Cancel(); return "ok cancel"; }
    if (what == "ok")     { dlg.DoMessage_Ok();  return "ok message"; }
    if (what == "hide")   { dlg.HideDialogs();   return "ok hide"; }
    return "unknown: yes|no|cancel|ok|hide";
}

// Dismiss whatever modal is up, correctly, by ASKING WHICH ONE IT IS.
//
// Leaving a map after MediaTracker edits raises a chain: FrameMessage, then
// AskYesNo, then DialogSaveAs. Answering "yes" to all of them SAVES the map --
// silently editing the very maps we are supposed to be filming unmodified.
// So each frame gets its own correct answer, and the default is to decline.
string DismissDialog() {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "no BasicDialogs";
    if (dlg.Dialogs is null || dlg.Dialogs.CurrentFrame is null) return "none";
    string id = dlg.Dialogs.CurrentFrame.IdName;
    if (id == "FrameDialogSaveAs") { dlg.DialogSaveAs_OnCancel(); return "cancelled saveas"; }
    if (id == "FrameAskYesNo")     { dlg.AskYesNo_No();           return "answered no"; }
    if (id == "FrameMessage")      { dlg.DoMessage_Ok();          return "acked message"; }
    dlg.HideDialogs();
    return "hid " + id;
}

// Is the title control API actually READY to accept EditMap? CGameManiaTitle-
// ControlScriptAPI carries IsReady, and EditMap on a not-ready title silently
// does nothing -- returns without error and no map loads. That is the failure
// we have been reading as "map did not open".
string TitleReady() {
    auto mp = MP();
    if (mp is null) return "{\"err\":\"no CGameManiaPlanet\"}";
    auto tc = mp.ManiaTitleControlScriptAPI;
    if (tc is null) return "{\"err\":\"no ManiaTitleControlScriptAPI\"}";
    string j = "{\"isReady\":" + (tc.IsReady ? "true" : "false");
    j += ",\"latestResult\":" + int(tc.LatestResult);
    // THE TITLE API'S OWN ERROR CHANNEL. EditMap returns void, so the only
    // thing it can ever say about a map it declined is here: LatestResult is
    // the EResult enum (0 Success, 1 Error_Internal, 2 Error_DataMgr, ...) and
    // CustomResultType/Data is what a title script fills in when it refuses.
    // A map that never opens with LatestResult 0 and an empty custom result is
    // a different fact from one that comes back Error_DataMgr, and until this
    // was printed nobody could tell which 146612 was.
    j += ",\"customResultType\":\"" + tc.CustomResultType + "\"";
    j += ",\"customResultData\":[";
    for (uint i = 0; i < tc.CustomResultData.Length; i++) {
        if (i > 0) j += ",";
        j += "\"" + tc.CustomResultData[i] + "\"";
    }
    j += "]";
    auto menus = mp.MenuManager;
    j += ",\"menuManager\":" + (menus is null ? "null" : "\"present\"");
    j += ",\"loadedTitle\":" + (mp.LoadedManiaTitle is null ? "null" : "\"present\"");
    j += "}";
    return j;
}

// Openplanet calls this every frame. It is where the self-reload actually
// happens: arming in the request handler and performing it here means the HTTP
// response is already on the wire when the script engine tears this module down.
void Update(float dt) {
    ReloadTick();
}

// Paths come from a FILE, never the query string: they carry backslashes,
// spaces and non-ascii, and a hand-rolled URL decoder is one more thing to
// be wrong about. The caller writes arg.txt, then calls the route.
string PathArg() {
    string f = IO::FromStorageFolder("arg.txt");
    if (!IO::FileExists(f)) return "";
    IO::File h(f, IO::FileMode::Read);
    string v = h.ReadToEnd().Trim();
    h.Close();
    return v;
}
