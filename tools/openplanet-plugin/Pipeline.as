// Pipeline.as -- the render pipeline as API calls.
//
// Everything here replaces a synthetic click or a blind sleep in render2.sh.
// House rule for this file: NO SCREEN COORDINATES, and nothing reports success
// unless the object graph says the thing actually happened.

CGameManiaPlanet@ MPl() { return cast<CGameManiaPlanet>(GetApp()); }

CGameCtnMenus@ Menus() {
    auto mp = MPl();
    if (mp is null) return null;
    return mp.MenuManager;
}

CGameEditorPluginMap@ EditorPlugin() {
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return null;
    return cast<CGameEditorPluginMap>(ed.PluginMapType);
}

// THE MEDIATRACKER, DIRECTLY. The old path clicked the EDIT button at
// (3343, 2075) to raise the "edit cut scenes" dialog and then called
// DialogEditCutScenes_OnInGameEdit -- that dialog's own button handler, which
// does nothing when the dialog is not up. It returned "ok" and left the editor
// where it was, and the shell covered the gap with three retries and 13 s of
// sleeps. CGameEditorPluginMap::EditMediatrackIngame is the call the button
// itself makes.
string OpenMediaTracker() {
    auto pm = EditorPlugin();
    if (pm is null) return "not in the map editor";
    pm.EditMediatrackIngame();
    return "ok";
}

// ---------------------------------------------------------------------------
// GHOST IMPORT -- the picker is the REPLAY MENU, and it has a real API.
//
// The old code computed page and row indices for a 12-row paged list from
// `ls | sort`, twice (folder list, then ghost list), and clicked them. Every
// clip was one stray file away from importing the wrong car; an `old/`
// subdirectory left in the folder did exactly that today, and a 17-file folder
// silently imported nothing the week before because row 13 is off the page.
//
// CGameCtnMenus carries the whole dialog: MenuReplay_CurPath, ReplayList,
// MenuReplay_OnSelectAll, MenuReplay_OnOk, MenuReplay_Flatten. So the import
// becomes: point it at a folder holding EXACTLY the ghosts for this render,
// select all, ok.
// ---------------------------------------------------------------------------

string ReplayMenuState() {
    auto m = Menus();
    if (m is null) return "{\"err\":\"no MenuManager\"}";
    string j = "{";
    j += "\"path\":\"" + m.MenuReplay_CurPathToDisplay + "\"";
    j += ",\"count\":" + m.MenuReplay_ReplaysCount;
    j += ",\"flatten\":" + (m.MenuReplay_Flatten ? "true" : "false");
    j += ",\"replays\":[";
    for (uint i = 0; i < m.ReplayList.Length; i++) {
        if (i > 0) j += ",";
        j += "\"" + m.ReplayList[i].Name + "\"";
    }
    j += "],\"dirs\":[";
    for (uint i = 0; i < m.ReplayDirsList.Length; i++) {
        if (i > 0) j += ",";
        j += "\"" + m.ReplayDirsList[i].Name + "\"";
    }
    j += "]}";
    return j;
}

string ReplaySelectAll() {
    auto m = Menus();
    if (m is null) return "no MenuManager";
    m.MenuReplay_OnSelectAll();
    return "selected all of " + m.ReplayList.Length;
}

string ReplayOk() {
    auto m = Menus();
    if (m is null) return "no MenuManager";
    m.MenuReplay_OnOk();
    return "ok";
}

string ReplayRefresh() {
    auto m = Menus();
    if (m is null) return "no MenuManager";
    m.MenuReplay_FilterAndRedraw();
    return "refreshed, " + m.ReplayList.Length + " replays";
}

// ---------------------------------------------------------------------------
// THE CAMERA, as numbers.
//
// Old: click "Tracks +", click "Player camera", then click a cycle button up to
// twelve times while re-reading the result, because the new block starts on
// entity 0 (nobody) and gamecam Default. A camera aimed at nobody renders black
// and passed every size and duration check we had.
// ---------------------------------------------------------------------------

string CameraState() {
    auto api = MTApi();
    if (api is null) return "{\"err\":\"not MT\"}";
    auto clip = api.Clip;
    if (clip is null) return "{\"err\":\"no clip\"}";
    string j = "{\"cams\":[";
    bool first = true;
    for (uint i = 0; i < clip.Tracks.Length; i++) {
        for (uint b = 0; b < clip.Tracks[i].Blocks.Length; b++) {
            auto cg = cast<CGameCtnMediaBlockCameraGame>(clip.Tracks[i].Blocks[b]);
            if (cg is null) continue;
            if (!first) j += ",";
            first = false;
            j += "{\"track\":" + i + ",\"block\":" + b
               + ",\"gamecam\":" + int(cg.GameCam)
               + ",\"entid\":" + cg.ClipEntId
               + ",\"target\":\"" + cg.TargetClipEntName + "\"}";
        }
    }
    j += "]}";
    return j;
}
