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

string PgGhostAdd() {
    string how;
    auto dfm = DataFileMgr(how);
    if (dfm is null) return "{\"error\":\"no DataFileMgr (" + how + ")\"}";
    auto app = GetApp();
    if (app.Network is null || app.Network.ClientManiaAppPlayground is null) {
        return "{\"error\":\"no ClientManiaAppPlayground (not in a playground)\"}";
    }
    auto gm = app.Network.ClientManiaAppPlayground.GhostMgr;
    if (gm is null) return "{\"error\":\"no GhostMgr\"}";
    string path = PathArg();
    if (path == "") return "{\"error\":\"arg.txt is empty\"}";
    auto task = dfm.Replay_Load(path);
    if (task is null) return "{\"error\":\"Replay_Load returned null\"}";
    uint t0 = Time::Now;
    while (task.IsProcessing && Time::Now - t0 < 30000) yield();
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
    auto gm = app.Network.ClientManiaAppPlayground.GhostMgr;
    if (gm is null) return "{\"error\":\"no GhostMgr\"}";
    gm.Ghost_RemoveAll();
    return "{\"removed\":true}";
}
