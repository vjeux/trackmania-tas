// RefGhost.as -- export the run the client HOLDS for the loaded map (the MediaTracker's
// "Ref. Ghost: Author ghost") through the game's own file writers.
//
// WHY (2026-09-09): vjeux playtested the 25 tiny maps in this client and finished some.
// The client re-attaches each finished run to its map BY MAP NAME as
// ChallengeParameters.RaceValidateGhost -- it is not in the map file (no ghost chunk),
// not in Replays/Autosaves, not in MediaTrackerGhosts, not in any cache folder; the only
// state written was Config/<profile>.Profile.Gbx. Those runs are the seeds the route
// search wants for the hard maps, and the object graph is the only place they exist.
//
// The game can write them itself: CGameDataFileManagerScript (the title/menu ManiaApp's
// DataFileMgr) has Map_GetAuthorGhost(Map) -> CGameGhostScript, Replay_Save(path, map,
// ghost) -> .Replay.Gbx (map + ghost) and Ghost_Upload(url, ghost, headers), which PUTs
// the ghost's own .Ghost.Gbx bytes at a URL -- and this plugin's HTTP server can be that
// URL. Both writers are exposed; the Rust side decides which output it trusts.
//
//   /refghost              what the game holds: RaceValidateGhost (time, nick, uid,
//                          duration, size) and the DataFileMgr's Map_GetAuthorGhost view
//                          (nick, trigram, result time, checkpoints)
//   /refsave?name=X        Replay_Save("vjeux-runs/X.Replay.Gbx", RootMap, author ghost);
//                          the path is relative to the user's Replays folder
//   /refupload?name=X      Ghost_Upload("http://127.0.0.1:29800/ghostsink?name=X", ghost, "")
//   /ghostsink?name=X      the upload's body, written to PluginStorage/GhostShooter/sink/
//                          X.Ghost.Gbx (any method; size reported)
//
// Paths and names stay ascii (two digits) so the query string carries them safely.

CGameDataFileManagerScript@ DataFileMgr(string &out how) {
    // The menu ManiaApp is the one the title scripts use; it is up in the editor too
    // (its UI layers are the editor dialogs -- Shoot.as).
    auto ma = ManiaApp();
    if (ma !is null && ma.DataFileMgr !is null) { how = "menu app"; return ma.DataFileMgr; }
    // The track editor's plugin map type derives from CGameManiaApp as well.
    auto fr = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (fr !is null) {
        auto pm = cast<CGameManiaApp>(fr.PluginMapType);
        if (pm !is null && pm.DataFileMgr !is null) { how = "editor plugin"; return pm.DataFileMgr; }
    }
    // In the MediaTracker the editor sits underneath, on the switcher's stack.
    auto sw = GetApp().Switcher;
    if (sw !is null) {
        for (uint i = 0; i < sw.ModuleStack.Length; i++) {
            auto f2 = cast<CGameCtnEditorFree>(sw.ModuleStack[i]);
            if (f2 is null) continue;
            auto pm2 = cast<CGameManiaApp>(f2.PluginMapType);
            if (pm2 !is null && pm2.DataFileMgr !is null) { how = "switcher editor " + i; return pm2.DataFileMgr; }
        }
    }
    how = "none";
    return null;
}

string RefGhostReport() {
    auto app = GetApp();
    auto map = app.RootMap;
    if (map is null) return "{\"map\":null}";
    string r = "{\"map\":\"" + map.MapName + "\"";
    if (map.MapInfo !is null) r += ",\"uid\":\"" + map.MapInfo.MapUid + "\"";
    auto p = map.ChallengeParameters;
    if (p is null) {
        r += ",\"params\":null";
    } else {
        r += ",\"authorTime\":" + p.AuthorTime;
        auto g = p.RaceValidateGhost;
        if (g is null) {
            r += ",\"raceValidateGhost\":null";
        } else {
            r += ",\"raceValidateGhost\":{\"time\":" + g.RaceTime
               + ",\"nick\":\"" + string(g.GhostNickname) + "\""
               + ",\"login\":\"" + g.GhostLogin + "\""
               + ",\"trigram\":\"" + g.GhostTrigram + "\""
               + ",\"uid\":\"" + g.Validate_ChallengeUid.GetName() + "\""
               + ",\"duration\":" + g.Duration
               + ",\"size\":" + g.Size
               + ",\"respawns\":" + g.NbRespawns
               + ",\"context\":\"" + g.RecordingContext + "\""
               + ",\"exe\":\"" + g.Validate_ExeVersion + "\""
               + ",\"gameMode\":\"" + g.Validate_GameMode + "\"}";
        }
    }
    string how;
    auto dfm = DataFileMgr(how);
    r += ",\"dataFileMgr\":\"" + how + "\"";
    if (dfm !is null) {
        auto gs = dfm.Map_GetAuthorGhost(map);
        if (gs is null) {
            r += ",\"authorGhost\":null";
        } else {
            r += ",\"authorGhost\":{\"nick\":\"" + string(gs.Nickname) + "\",\"trigram\":\"" + gs.Trigram + "\"";
            auto res = gs.Result;
            if (res !is null) {
                r += ",\"time\":" + res.Time + ",\"respawns\":" + res.NbRespawns + ",\"cps\":[";
                for (uint i = 0; i < res.Checkpoints.Length; i++) {
                    if (i > 0) r += ",";
                    uint cp = res.Checkpoints[i];
                    r += "" + cp;
                }
                r += "]";
            } else {
                r += ",\"result\":null";
            }
            r += "}";
            dfm.Ghost_Release(gs.Id);
        }
    }
    return r + "}";
}

// Wait for a DataFileMgr task, report it, release it. `done` says whether the task
// finished inside the budget -- the ghost handle is only released once it has.
string TaskReport(CGameDataFileManagerScript@ dfm, CWebServicesTaskResult@ task, uint budgetMs, bool &out done) {
    done = true;
    if (task is null) return "\"task\":null";
    uint t0 = Time::Now;
    while (task.IsProcessing && Time::Now - t0 < budgetMs) yield();
    done = !task.IsProcessing;
    string r = "\"processing\":" + (task.IsProcessing ? "1" : "0")
             + ",\"ok\":" + (task.HasSucceeded ? "1" : "0")
             + ",\"failed\":" + (task.HasFailed ? "1" : "0")
             + ",\"ms\":" + (Time::Now - t0)
             + ",\"error\":\"" + task.ErrorType + "/" + task.ErrorCode + " " + string(task.ErrorDescription) + "\"";
    if (done) dfm.TaskResult_Release(task.Id);
    return r;
}

string RefSave(const string &in name) {
    if (name == "") return "usage: /refsave?name=NN";
    string how;
    auto dfm = DataFileMgr(how);
    if (dfm is null) return "no DataFileMgr (" + how + ")";
    auto map = GetApp().RootMap;
    if (map is null) return "no RootMap";
    auto gs = dfm.Map_GetAuthorGhost(map);
    if (gs is null) return "{\"map\":\"" + map.MapName + "\",\"authorGhost\":null}";
    string path = "vjeux-runs/" + name + ".Replay.Gbx";
    auto task = dfm.Replay_Save(path, map, gs);
    bool done;
    string r = "{\"map\":\"" + map.MapName + "\",\"path\":\"" + path + "\",\"via\":\"" + how + "\","
             + TaskReport(dfm, task, 60000, done) + "}";
    if (done) dfm.Ghost_Release(gs.Id);
    return r;
}

string RefUpload(const string &in name) {
    if (name == "") return "usage: /refupload?name=NN";
    string how;
    auto dfm = DataFileMgr(how);
    if (dfm is null) return "no DataFileMgr (" + how + ")";
    auto map = GetApp().RootMap;
    if (map is null) return "no RootMap";
    auto gs = dfm.Map_GetAuthorGhost(map);
    if (gs is null) return "{\"map\":\"" + map.MapName + "\",\"authorGhost\":null}";
    string url = "http://127.0.0.1:" + PORT + "/ghostsink?name=" + name;
    auto task = dfm.Ghost_Upload(url, gs, "");
    bool done;
    string r = "{\"map\":\"" + map.MapName + "\",\"url\":\"" + url + "\",\"via\":\"" + how + "\","
             + TaskReport(dfm, task, 60000, done) + "}";
    if (done) dfm.Ghost_Release(gs.Id);
    return r;
}

string GhostSink(const string &in type, const string &in name, MemoryBuffer@ body) {
    string dir = IO::FromStorageFolder("sink");
    if (!IO::FolderExists(dir)) IO::CreateFolder(dir);
    string f = dir + "/" + (name == "" ? "ghost" : name) + ".Ghost.Gbx";
    if (body is null) {
        IO::File h0(f + ".nobody.txt", IO::FileMode::Write);
        h0.Write(type + " without a body");
        h0.Close();
        return "{\"method\":\"" + type + "\",\"bytes\":0,\"file\":\"" + f + "\"}";
    }
    body.Seek(0);
    IO::File h(f, IO::FileMode::Write);
    h.Write(body);
    h.Close();
    return "{\"method\":\"" + type + "\",\"bytes\":" + body.GetSize() + ",\"file\":\"" + f + "\"}";
}
