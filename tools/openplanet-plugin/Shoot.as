// Shoot.as -- the render itself, scripted.
//
// The last clicks in the pipeline are the AVI/Webm dialog (`C 852 2093`) and its
// OK button (`C 1747 1743`). CGameDialogShootParams carries that whole dialog --
// Duration, VideoFps, Width, Height, ExtVideo (AVI|Webm), ShootName, OnOk,
// EstimatedTime -- so the shoot can be an API call whose settings are READ
// rather than assumed.
//
// THE PROBLEM: no typed member anywhere in the API dump holds a
// CGameDialogShootParams@, and CGameDialogs::Dialog is an enum, not the nod.
// The instance exists once the dialog is up, so it is found the same way the
// camera's writable offsets were: scan the owner's memory for a nod whose
// runtime type matches, then report the offset so it can be used directly.
//
// This is a PROBE, not a guess: it prints what it found and where, and if the
// game moves it the scan simply reports nothing rather than writing somewhere
// wrong.

// A BLIND NOD SCAN CRASHES THE GAME. The first version of this walked the
// dialog owner's memory calling Dev::GetOffsetNod at every 8-byte offset and
// casting whatever came back; the process died on the first run and took the
// plugin with it. Reading an arbitrary qword as a nod pointer dereferences
// garbage -- the camera offsets were safe because Reflection NAMED them, and
// that is the difference between the two techniques. Do not reintroduce this.

// SOLVED, in ShootNod.as. The instance is not held by any typed member, but
// every CControlBase carries the nod it is BOUND to, and the dialog's controls
// are bound to this one. That is a declared member, so reading it is safe --
// the opposite of the scan described above. See ShootNod.as.
CGameDialogShootParams@ ShootDialog() {
    return ShootParams();
}

string ShootParamsState() {
    auto sp = ShootDialog();
    if (sp is null) return "{\"err\":\"no shoot dialog found\"}";
    string j = "{";
    j += "\"duration\":" + sp.Duration;
    j += ",\"fps\":" + sp.VideoFps;
    j += ",\"w\":" + sp.Width;
    j += ",\"h\":" + sp.Height;
    j += ",\"ext\":" + int(sp.ExtVideo);
    j += ",\"hq\":" + (sp.VideoHq ? "true" : "false");
    j += ",\"name\":\"" + sp.ShootName + "\"";
    j += ",\"estimated\":\"" + sp.EstimatedTime + "\"";
    j += "}";
    return j;
}

string ShootOk() {
    auto sp = ShootDialog();
    if (sp is null) return "no shoot dialog found";
    sp.OnOk();
    return "ok";
}

string ShootCancel() {
    auto sp = ShootDialog();
    if (sp is null) return "no shoot dialog found";
    sp.OnCancel();
    return "cancelled";
}

// ---------------------------------------------------------------------------
// IS THE RENDER RUNNING?
//
// The old pipeline answered this by polling the screenshots folder for a new
// .webm and then polling its SIZE until it stopped growing -- 5-second sleeps,
// up to an hour, and no way to tell "still rendering" from "crashed".
//
// `Operation_InProgress` on CGameManiaPlanet is the game's own "a long
// operation is running" flag, and MTApi::IsPlaying says whether the clip is
// being played through. Together they are a real completion signal, reported as
// numbers so the driver can wait on a CONDITION instead of a duration.
string ShootStatus() {
    auto mp = MPl();
    auto api = MTApi();
    string j = "{";
    j += "\"op\":" + ((mp !is null && mp.Operation_InProgress) ? "true" : "false");
    j += ",\"playing\":" + ((api !is null && api.IsPlaying()) ? "true" : "false");
    j += ",\"timer\":" + ((api !is null) ? "" + api.CurrentTimer : "-1");
    // IS THE SHOOT DIALOG UP? Ask the dialog itself. The driver used to wait for
    // /ctx to report "FrameDialogSaveAs" -- which is a DIFFERENT dialog (the
    // ghost-import file picker) and is often still reported from the previous
    // step, so the wait passed instantly and the OK went in before the dialog
    // existed.
    auto sp = ShootParams();
    j += ",\"shootdlg\":" + ((sp is null) ? "false" : "true");
    auto dlg = GetApp().BasicDialogs;
    if (dlg !is null && dlg.Dialogs !is null && dlg.Dialogs.CurrentFrame !is null)
        j += ",\"dialog\":\"" + dlg.Dialogs.CurrentFrame.IdName + "\"";
    else j += ",\"dialog\":null";
    j += "}";
    return j;
}

// The dialog's TEXT ENTRY. CGameDialogs::String @160 is the field the user
// types into -- distinct from DialogSaveAs_Path, which is the folder the list
// is showing. For the shoot dialog it is the output filename.
string DialogString() {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "{\"err\":\"no BasicDialogs\"}";
    return "{\"string\":\"" + dlg.String + "\",\"path\":\"" + dlg.DialogSaveAs_Path + "\"}";
}

string SetDialogString(const string &in v) {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "no BasicDialogs";
    uint16 off = MemberOffset("CGameDialogs", "String");
    if (off == 65535) return "could not resolve CGameDialogs::String";
    Dev::SetOffset(dlg, off, v);
    return "string now \"" + dlg.String + "\"";
}

// WHAT IS THE GAME ASKING? A modal's text is on CGameDialogs, and answering a
// dialog without reading it is how "yes to everything" once silently SAVED the
// maps we were meant to be filming unmodified.
string DialogText() {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "{\"err\":\"no BasicDialogs\"}";
    string j = "{\"msg\":\"" + dlg.Message_LabelText + "\"";
    j += ",\"button\":\"" + dlg.Message_ButtonText + "\"";
    j += ",\"wait\":\"" + dlg.WaitMessage_LabelText + "\"";
    j += ",\"progress\":" + dlg.WaitMessage_Progress;
    j += ",\"showbar\":" + (dlg.WaitMessage_ShowProgressBar ? "true" : "false");
    auto d2 = dlg.Dialogs;
    if (d2 !is null && d2.CurrentFrame !is null)
        j += ",\"frame\":\"" + d2.CurrentFrame.IdName + "\"";
    else j += ",\"frame\":null";
    j += "}";
    return j;
}

// FIND IT IN THE UI TREE, not in raw memory.
//
// The shoot dialog is live -- the screen shows it with real values -- while
// /ctx reports dialog:null, so it does NOT hang off BasicDialogs. The editor's
// own InterfaceRoot is a CControlContainer tree, and Openplanet gives every
// control a runtime type, so the dialog can be found by walking that tree and
// reporting what is there. Walking typed controls is safe; the blind
// Dev::GetOffsetNod scan that crashed the game was not.
void WalkFind(CControlBase@ c, int depth, const string &in needle, string &out sb, int maxDepth) {
    if (c is null) return;
    auto ti = Reflection::TypeOf(c);
    string tn = (ti is null) ? "?" : ti.Name;
    if (needle == "" || tn.IndexOf(needle) >= 0 || c.IdName.IndexOf(needle) >= 0) {
        string pad = "";
        for (int i = 0; i < depth; i++) pad += "  ";
        sb += pad + c.IdName + "  [" + tn + "]\n";
    }
    if (depth >= maxDepth) return;
    auto cont = cast<CControlContainer>(c);
    if (cont is null) return;
    for (uint i = 0; i < cont.Childs.Length; i++)
        WalkFind(cont.Childs[i], depth + 1, needle, sb, maxDepth);
}

string EditorTree(const string &in needle) {
    auto app = GetApp();
    string sb = "";
    // CGameEditorMediaTracker has NO EditorInterface (checked: its whole
    // hierarchy exposes only GameScene and PluginAPI). The map editor does, and
    // in-game MediaTracker runs inside it, so that is the tree to walk.
    auto fr = cast<CGameCtnEditorFree>(app.Editor);
    if (fr !is null && fr.EditorInterface !is null && fr.EditorInterface.InterfaceRoot !is null) {
        sb += "-- editor InterfaceRoot --\n";
        WalkFind(fr.EditorInterface.InterfaceRoot, 0, needle, sb, 12);
    }
    if (sb == "") sb = "nothing matching \"" + needle + "\"\n";
    return sb;
}

// A TYPED scan for the shoot dialog, at NOD-POINTER offsets only.
//
// The earlier crash came from calling Dev::GetOffsetNod at EVERY 8-byte offset
// of an object -- most of those qwords are not pointers, and dereferencing them
// killed the process. Reflection knows which members ARE nods: it gives each a
// real offset (not 65535). So walk only those, cast, and recurse a couple of
// levels. Nothing is read that the game did not already declare as a nod.
void ScanNods(CMwNod@ n, const string &in wanted, int depth, string &out sb, int maxDepth) {
    if (n is null || depth > maxDepth) return;
    auto ti = Reflection::TypeOf(n);
    if (ti is null) return;
    if (ti.Name == wanted) { sb += "FOUND " + wanted + " at depth " + depth + "\n"; return; }
    for (uint i = 0; i < ti.Members.Length; i++) {
        auto m = ti.Members[i];
        if (m.Offset == 65535 || m.Offset == 0) continue;
        auto child = Dev::GetOffsetNod(n, m.Offset);
        if (child is null) continue;
        auto cti = Reflection::TypeOf(child);
        if (cti is null) continue;
        if (cti.Name == wanted) {
            sb += "FOUND " + wanted + " via " + ti.Name + "." + m.Name + " @" + m.Offset + "\n";
            continue;
        }
        if (depth < maxDepth) ScanNods(child, wanted, depth + 1, sb, maxDepth);
    }
}

string FindNod(const string &in wanted) {
    string sb = "";
    auto app = GetApp();
    ScanNods(app, wanted, 0, sb, 3);
    if (sb == "") sb = "not found from GetApp() within 3 levels\n";
    return sb;
}

// THE DIALOG IS A MANIALINK PAGE.
//
// The shoot dialog is not a CControl tree and not under BasicDialogs -- it is a
// UI LAYER: CGameManiaApp::UILayers, each with a LocalPage (CGameManialinkPage)
// whose controls are addressable BY ID. That is why every earlier search missed
// it: I was looking in the editor's control tree and in the dialog system, and
// it lives in neither.
// CGameEditorPluginMap DERIVES FROM CGameManiaApp -- the same object already
// used for EditMediatrackIngame -- so the editor's UI layers are reachable
// without hunting for a separate app object. ManiaPlanetScriptAPI is a
// different class and casts to null, which is what "no CGameManiaApp" meant.
// In the MediaTracker, app.Editor IS the CGameEditorMediaTracker and its
// PluginAPI is the MT api -- neither is a CGameManiaApp. The MT api itself is
// the thing that owns the dialog UI, so try it, then the map editor plugin
// (which does derive from CGameManiaApp) for the track-editor case.
// FOUND IT: MenuManager.MenuCustom_CurrentManiaApp is the CGameManiaAppTitle,
// and its 77 UILayers are the game menu UI -- including the editor dialogs.
// The MediaTracker api and ManiaPlanetScriptAPI are NOT CGameManiaApps, which
// is what every earlier cast was failing on.
CGameManiaApp@ ManiaApp() {
    auto mp = MPl();
    if (mp is null) return null;
    auto mm = mp.MenuManager;
    if (mm is null) return null;
    return mm.MenuCustom_CurrentManiaApp;
}

// EVERY ManiaApp, not just the menu's.
//
// The menu app's 77 layers are the MENU's UI. The shoot dialog is raised from
// inside the map editor, and the editor has a ManiaApp of its own:
// CGameEditorPluginMap DERIVES from CGameManiaApp. In the MediaTracker
// GetApp().Editor is the CGameEditorMediaTracker, so the editor has to come off
// the switcher's module stack -- it is still there, underneath.
void AllManiaApps(array<CGameManiaApp@> &out apps, array<string> &out names) {
    auto menu = ManiaApp();
    if (menu !is null) { apps.InsertLast(menu); names.InsertLast("menu"); }
    auto sw = GetApp().Switcher;
    if (sw is null) return;
    for (uint i = 0; i < sw.ModuleStack.Length; i++) {
        auto fr = cast<CGameCtnEditorFree>(sw.ModuleStack[i]);
        if (fr is null) continue;
        auto pm = cast<CGameManiaApp>(fr.PluginMapType);
        if (pm is null) continue;
        apps.InsertLast(pm);
        names.InsertLast("editor" + i);
    }
}

string DumpLayers() {
    array<CGameManiaApp@> apps;
    array<string> names;
    AllManiaApps(apps, names);
    if (apps.Length == 0) return "no CGameManiaApp";
    string sb = "";
    for (uint a = 0; a < apps.Length; a++) {
        auto ma = apps[a];
        sb += "## " + names[a] + ": " + ma.UILayers.Length + " layers\n";
        for (uint i = 0; i < ma.UILayers.Length; i++) {
            auto l = ma.UILayers[i];
            if (l is null) continue;
            sb += "[" + i + "] visible=" + (l.IsVisible ? "1" : "0")
                + " attach=" + l.AttachId
                + " page=" + (l.LocalPage is null ? "null" : "yes") + "\n";
            // the first 200 characters of the page tell you which dialog it is
            string p = l.ManialinkPageUtf8;
            if (p.Length > 0) {
                string head = (p.Length > 160) ? p.SubStr(0, 160) : p;
                sb += "     " + head + "\n";
            }
        }
    }
    return sb;
}

// Which of the reachable objects IS a CGameManiaApp? Report rather than guess.
string WhoIsManiaApp() {
    string sb = "";
    auto app = GetApp();
    auto mp = MPl();
    sb += "app.Editor: " + ((app.Editor is null) ? "null" : Reflection::TypeOf(app.Editor).Name) + "\n";
    auto api = MTApi();
    sb += "MTApi: " + ((api is null) ? "null" : Reflection::TypeOf(api).Name)
        + "  isManiaApp=" + ((api !is null && cast<CGameManiaApp>(api) !is null) ? "yes" : "no") + "\n";
    if (mp !is null) {
        auto s1 = mp.ManiaPlanetScriptAPI;
        sb += "ManiaPlanetScriptAPI: " + ((s1 is null) ? "null" : Reflection::TypeOf(s1).Name)
            + "  isManiaApp=" + ((s1 !is null && cast<CGameManiaApp>(s1) !is null) ? "yes" : "no") + "\n";
        auto mm = mp.MenuManager;
        sb += "MenuManager: " + ((mm is null) ? "null" : Reflection::TypeOf(mm).Name) + "\n";
        if (mm !is null) {
            auto mc = mm.MenuCustom_CurrentManiaApp;
            sb += "MenuCustom_CurrentManiaApp: " + ((mc is null) ? "null" : Reflection::TypeOf(mc).Name)
                + "  layers=" + ((mc is null) ? "-" : "" + mc.UILayers.Length) + "\n";
        }
    }
    return sb;
}

// The page CONTENT is in ManialinkPage (wstring), not ManialinkPageUtf8, and a
// dialog's identity is in its <label>/<quad> ids. Search all layers for a
// needle and report which ones match, with a window of context.
string GrepLayers(const string &in needle) {
    array<CGameManiaApp@> apps;
    array<string> names;
    AllManiaApps(apps, names);
    if (apps.Length == 0) return "no CGameManiaApp";
    string sb = "";
    for (uint a = 0; a < apps.Length; a++) {
        auto ma = apps[a];
        for (uint i = 0; i < ma.UILayers.Length; i++) {
            auto l = ma.UILayers[i];
            if (l is null || l.LocalPage is null) continue;
            string p = l.ManialinkPage;
            if (p.Length == 0) continue;
            int at = p.IndexOf(needle);
            if (at < 0) continue;
            int st = (at > 120) ? at - 120 : 0;
            int len = 320;
            if (uint(st + len) > p.Length) len = int(p.Length) - st;
            sb += names[a] + "[" + i + "] visible=" + (l.IsVisible ? "1" : "0")
                + " len=" + p.Length + " @" + at + "\n"
                + "    ..." + p.SubStr(st, len) + "...\n";
        }
    }
    if (sb == "") sb = "no layer page contains \"" + needle + "\"\n";
    return sb;
}
