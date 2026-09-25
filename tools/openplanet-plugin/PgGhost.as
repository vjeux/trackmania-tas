// PgGhost.as -- a ghost FILE shown in the PLAYGROUND (play mode), the way the
// game shows a leaderboard ghost: DataFileMgr.Replay_Load(path), then
// GhostMgr.Ghost_Add for every ghost the file holds.
//
// The PLAY-mode skin-locator test of 2026-09-12: the MediaTracker EDITOR never
// fetched a ghost skin's PackDesc URL; does the played game, where leaderboard
// ghosts get their skins, fetch it for a ghost added this way?
//
//   /pgghost     arg.txt = the ghost or replay file (a C:/… path); loads it and
//                adds every ghost in it to the current playground -> JSON
//   /pgghostrm   GhostMgr.Ghost_RemoveAll()

// /pgghost?offset=MS|now  the ghost's clock: `now` = its t=0 is the current
// race time (added mid-race it starts from the grid this instant);
// ?phys=1 adds it PHYSICALIZED (collidable, the Clones machinery) with
// PlaySpeed ?speed=1.0
string PgGhostAdd(const string &in qs) {
    string how;
    int offset = 0;
    string so = QArg(qs, "offset");
    if (so == "now") {
        auto sp = ScriptPlayer();
        if (sp !is null) offset = sp.CurrentRaceTime;
    } else if (so != "") {
        offset = Text::ParseInt(so);
    }
    bool phys = QArg(qs, "phys") == "1";
    float speed = 1.0f;
    if (QArg(qs, "speed") != "") speed = Text::ParseFloat(QArg(qs, "speed"));
    auto dfm = DataFileMgr(how);
    if (dfm is null) return "{\"error\":\"no DataFileMgr (" + how + ")\"}";
    auto app = GetApp();
    // the GhostMgr comes up a few seconds after the playground (the mode script's
    // start): wait for it
    uint tg = Time::Now;
    CGameGhostMgrScript@ gm = null;
    uint nullApp = 0;
    while (gm is null && Time::Now - tg < 40000) {
        // the MODE's ghost manager first (CSmArenaRulesMode.GhostMgr — the one
        // the ghost-loader plugins use; the client ManiaApp's stayed null for
        // 40 s in TM_PlayMap_Local and in the map's own mode, 2026-09-25)
        auto rules = cast<CSmArenaRulesMode>(app.PlaygroundScript);
        if (rules !is null && rules.GhostMgr !is null) {
            @gm = rules.GhostMgr;
        } else if (app.Network !is null && app.Network.ClientManiaAppPlayground !is null) {
            @gm = app.Network.ClientManiaAppPlayground.GhostMgr;
        } else {
            nullApp++;
        }
        if (gm is null) yield();
    }
    if (gm is null) return "{\"error\":\"no GhostMgr after " + (Time::Now - tg) + " ms (playground app null on " + nullApp + " frames)\",\"ctx\":" + CurrentCtx() + "}";
    string path = PathArg();
    if (path == "") return "{\"error\":\"arg.txt is empty\"}";
    // the MODE's DataFileMgr when there is one (the menu app's answered
    // "Unable to load file" for a Replays\… ghost in play, 2026-09-25); an
    // absolute path first, then the same path relative to the user folder
    auto rules2 = cast<CSmArenaRulesMode>(app.PlaygroundScript);
    if (rules2 !is null && rules2.DataFileMgr !is null) { @dfm = rules2.DataFileMgr; how = "mode"; }
    string tried = "";
    CWebServicesTaskResult_GhostListScript@ task = null;
    for (uint attempt = 0; attempt < 2 && (task is null || task.HasFailed); attempt++) {
        string p2 = path;
        if (attempt == 1) {
            // relative to the user game folder: strip everything up to "/Trackmania/"
            int k = path.IndexOf("/Trackmania/");
            if (k < 0) break;
            p2 = path.SubStr(k + 12).Replace("/", "\\");
        }
        @task = dfm.Replay_Load(p2);
        if (task is null) return "{\"error\":\"Replay_Load returned null\"}";
        uint t1 = Time::Now;
        while (task.IsProcessing && Time::Now - t1 < 30000) yield();
        tried += (attempt > 0 ? " | " : "") + p2 + " -> " + (task.HasSucceeded ? "ok" : string(task.ErrorDescription));
    }
    // a bare .Ghost.Gbx is not a replay: Ghost_Download with a file:// URL
    CGameGhostScript@ dl = null;
    if (task is null || task.HasFailed) {
        // our own HTTP server hands the file back (a file:// URL got "No packdesc")
        string url = "http://127.0.0.1:29800/file?p=" + Net::UrlEncode(path);
        auto gt = dfm.Ghost_Download("mk64cpu" + Time::Now + ".Ghost.Gbx", url);
        if (gt !is null) {
            uint t2 = Time::Now;
            while (gt.IsProcessing && Time::Now - t2 < 30000) yield();
            tried += " | " + url + " -> " + (gt.HasSucceeded ? "ok" : string(gt.ErrorDescription));
            if (gt.HasSucceeded && gt.Ghost !is null) @dl = gt.Ghost;
        }
    }
    if (dl !is null) {
        MwId id;
        if (phys) {
            id = gm.Ghost_AddPhysicalized(dl, offset, speed, CGameGhostMgrScript::EGhostPhyMode::SoftCollisions, false);
        } else {
            // ?layer=0: a plain opponent ghost (the mode's own PB ghost is one);
            // default the ghost layer
            id = gm.Ghost_Add(dl, QArg(qs, "layer") != "0", offset);
        }
        // offset=start: every managed ghost's clock zero = the player's race
        // start (the mode's own call for its PB ghost). With TimeOffset 0 a
        // ghost's zero was the PLAYGROUND's (~3 s before the countdown ended;
        // measured 2026-09-25 with /vis: ghost time = race time + 3 s).
        if (so == "start") {
            auto rules3 = cast<CSmArenaRulesMode>(app.PlaygroundScript);
            auto sp3 = ScriptPlayer();
            if (rules3 !is null && sp3 !is null) rules3.Ghosts_SetStartTime(sp3.StartTime);
        }
        return "{\"path\":\"" + path + "\",\"via\":\"" + how + " [" + tried + "]\",\"ok\":1,\"ghosts\":1,\"offset\":" + offset + ",\"phys\":" + (phys ? 1 : 0) + ",\"added\":[{\"instance\":" + id.Value + ",\"nickname\":\"" + string(dl.Nickname) + "\"}]}";
    }
    uint t0 = Time::Now;
    how += " [" + tried + "]";
    string r = "{\"path\":\"" + path + "\",\"via\":\"" + how + "\",\"ms\":" + (Time::Now - t0)
             + ",\"processing\":" + (task.IsProcessing ? "1" : "0")
             + ",\"ok\":" + (task.HasSucceeded ? "1" : "0")
             + ",\"failed\":" + (task.HasFailed ? "1" : "0")
             + ",\"error\":\"" + task.ErrorType + "/" + task.ErrorCode + " " + string(task.ErrorDescription) + "\"";
    if (task.HasSucceeded) {
        r += ",\"ghosts\":" + task.Ghosts.Length + ",\"added\":[";
        for (uint i = 0; i < task.Ghosts.Length; i++) {
            auto g = task.Ghosts[i];
            MwId id = gm.Ghost_Add(g);
            if (i > 0) r += ",";
            r += "{\"instance\":" + id.Value + ",\"nickname\":\"" + string(g.Nickname) + "\",\"idname\":\"" + g.IdName + "\",\"trigram\":\"" + g.Trigram + "\"}";
        }
        r += "]";
    }
    // the task (and the ghosts it owns) is kept alive on purpose: a probe
    return r + "}";
}

string PgGhostRm() {
    auto app = GetApp();
    if (app.Network is null || app.Network.ClientManiaAppPlayground is null) return "{\"error\":\"not in a playground\"}";
    CGameGhostMgrScript@ gm = null;
    auto rules = cast<CSmArenaRulesMode>(app.PlaygroundScript);
    if (rules !is null) @gm = rules.GhostMgr;
    if (gm is null && app.Network !is null && app.Network.ClientManiaAppPlayground !is null) @gm = app.Network.ClientManiaAppPlayground.GhostMgr;
    if (gm is null) return "{\"error\":\"no GhostMgr\"}";
    gm.Ghost_RemoveAll();
    return "{\"removed\":true}";
}

// The bytes of a file under the user's Trackmania folder (or ProgramData),
// as an octet-stream — the loopback the game downloads a ghost from.
HttpResponse@ ServeFile(const string &in p) {
    string path = Net::UrlDecode(p).Replace("\\", "/");
    if (path == "" || !IO::FileExists(path)) return HttpResponse(404, "no such file: " + path);
    IO::File f(path, IO::FileMode::Read);
    MemoryBuffer@ buf = f.Read(f.Size());
    f.Close();
    return HttpResponse(200, buf);
}

// /pgghosts  how many ghosts the mode's manager holds right now
string PgGhostCount() {
    auto app = GetApp();
    auto rules = cast<CSmArenaRulesMode>(app.PlaygroundScript);
    if (rules is null || rules.GhostMgr is null) return "{\"error\":\"no mode GhostMgr\"}";
    auto sp = ScriptPlayer();
    return "{\"ghosts\":" + rules.Ghosts.Length + ",\"race_ms\":" + (sp is null ? -1 : sp.CurrentRaceTime) + ",\"start_ms\":" + (sp is null ? -1 : sp.StartTime) + ",\"now\":" + rules.Now + "}";
}

// /vis  every vehicle visual in the scene (the player's car AND the ghosts'),
// position + speed — the ground truth of whether an added ghost is being
// played, and where (VehicleState::GetAllVis)
string VisList() {
    auto scene = GetApp().GameScene;
    if (scene is null) return "{\"error\":\"no GameScene\"}";
    auto all = VehicleState::GetAllVis(scene);
    string s = "{\"n\":" + all.Length + ",\"cars\":[";
    for (uint i = 0; i < all.Length; i++) {
        auto v = all[i];
        if (v is null || v.AsyncState is null) continue;
        vec3 p = v.AsyncState.Position;
        if (i > 0) s += ",";
        s += "[" + Text::Format("%.1f", p.x) + "," + Text::Format("%.1f", p.y) + "," + Text::Format("%.1f", p.z) + "," + Text::Format("%.1f", v.AsyncState.WorldVel.Length() * 3.6) + "]";
    }
    return s + "]}";
}
