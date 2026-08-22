// Await.as -- WAITING, DONE PROPERLY.
//
// Every wait in this pipeline used to be a poll loop in the driver: ask, sleep
// 200 ms, ask again. That is a sleep with extra steps -- it burns a round trip
// per tick, it reports the condition up to a tick late, and the interval is a
// number somebody guessed.
//
// It does not have to be. HttpServer calls RequestHandler from inside a
// coroutine (RunClient, started with startnew), so `yield()` in a handler
// returns control to the GAME and resumes on the next frame. The request simply
// does not answer until the condition holds. The driver makes one HTTP call and
// blocks on the socket -- no sleeps, no interval, and the answer arrives on the
// frame the thing actually happened.
//
// Every condition here is a fact from the object graph. Deadlines are in FRAMES
// as well as milliseconds, because a game that has stopped rendering will never
// satisfy any of them and should fail rather than hang forever.

// SYNTAX NOTE: conditions use a colon, never "=" or ">=". QArg is a crude
// query splitter with no URL decoding, so "ctx%3D0" arrives literally and
// silently matches nothing -- it times out looking correct, which is the worst
// possible failure. A colon needs no encoding.
//
// ctx:N          the context number: 0 menu, 1 track editor, 2 mediatracker, 3 race
// ready          ManiaTitleControlScriptAPI::IsReady -- EditMap on a not-ready
//                title silently does nothing, which is the failure that looks
//                like "the map did not open"
// nodialog       no modal frame in the basic dialogs
// ghosts:N       at least N ghost blocks in the MediaTracker clip
// shootdlg       the CGameDialogShootParams nod exists
// noshootdlg     ...and is gone again (i.e. the dialog was accepted)
// tracks:N       at least N tracks in the clip
bool CondMet(const string &in cond) {
    auto app = GetApp();

    if (cond == "ready") {
        auto mp = MP();
        if (mp is null) return false;
        auto tc = mp.ManiaTitleControlScriptAPI;
        return tc !is null && tc.IsReady;
    }
    if (cond == "nodialog") {
        auto dlg = app.BasicDialogs;
        return dlg is null || dlg.Dialogs is null || dlg.Dialogs.CurrentFrame is null;
    }
    if (cond == "shootdlg")   return ShootDialogNod() !is null;
    if (cond == "noshootdlg") return ShootDialogNod() is null;

    if (cond.StartsWith("ctx:")) {
        int want = Text::ParseInt(cond.SubStr(4));
        return CurrentCtx() == want;
    }
    if (cond.StartsWith("ghosts:")) {
        int want = Text::ParseInt(cond.SubStr(7));
        return int(GhostBlockCount()) >= want;
    }
    if (cond.StartsWith("tracks:")) {
        int want = Text::ParseInt(cond.SubStr(7));
        auto api = MTApi();
        if (api is null || api.Clip is null) return false;
        return int(api.Clip.Tracks.Length) >= want;
    }
    return false;
}

// A CONDITION NOBODY IMPLEMENTS MUST NOT LOOK LIKE A CONDITION THAT IS FALSE.
// A typo would otherwise wait out the whole deadline and report ok:false, which
// reads exactly like "the game did not get there" and sends you debugging the
// game. Rejected up front instead.
bool CondKnown(const string &in cond) {
    return cond == "ready" || cond == "nodialog"
        || cond == "shootdlg" || cond == "noshootdlg"
        || cond.StartsWith("ctx:") || cond.StartsWith("ghosts:")
        || cond.StartsWith("tracks:");
}

// The context number, shared with /ctx so the two can never disagree.
int CurrentCtx() {
    auto app = GetApp();
    if (app.Editor is null) return (app.CurrentPlayground is null) ? 0 : 3;
    if (cast<CGameEditorMediaTracker>(app.Editor) !is null) return 2;
    if (cast<CGameCtnEditorFree>(app.Editor) !is null) return 1;
    return 9;
}

// Block until the condition holds. Returns how long it took, in ms and frames,
// so the caller gets a measurement rather than a reassurance.
string Await(const string &in cond, int timeoutMs) {
    if (cond == "") return "{\"err\":\"no condition\"}";
    if (!CondKnown(cond)) return "{\"err\":\"unknown condition: " + cond + "\"}";
    uint t0 = Time::Now;
    int frames = 0;
    int budget = (timeoutMs > 0) ? timeoutMs : 60000;
    while (true) {
        if (CondMet(cond))
            return "{\"ok\":true,\"cond\":\"" + cond + "\",\"ms\":" + (Time::Now - t0)
                 + ",\"frames\":" + frames + "}";
        if (int(Time::Now - t0) > budget)
            return "{\"ok\":false,\"cond\":\"" + cond + "\",\"ms\":" + (Time::Now - t0)
                 + ",\"frames\":" + frames + ",\"ctx\":" + CurrentCtx() + "}";
        // THE WHOLE TRICK: hand the frame back to the game and pick up here on
        // the next one. Nothing sleeps; the game is not held up.
        yield();
        frames++;
    }
    return "{\"err\":\"unreachable\"}";
}

// ---------------------------------------------------------------------------
// IS THE GAME STILL WRITING THIS FILE?
//
// The last wait in the pipeline that was not a condition. Nothing in the object
// graph moves during a shoot -- measured across a whole 53-second render: no
// Operation_InProgress, no IsPlaying, no timer, no progress bar. But the
// ENCODER holds its output open, and a read-open that also asks to deny writers
// fails while it does. That is the writer's own release, and it is exact.
//
// The driver used to do this over PowerShell, which meant spawning a process
// four times a second FOR THE WHOLE RENDER, competing with the encoder for the
// machine. In here it costs one failed CreateFile per frame and the answer
// arrives on the frame it happens.
//
// FileMode::Read never truncates -- the obvious Write mode would DESTROY the
// render it is waiting for.
bool FileFree(const string &in path) {
    if (path == "") return false;
    if (!IO::FileExists(path)) return false;
    try {
        IO::File f(path, IO::FileMode::Read, IO::ShareMode::DenyWrite);
        f.Close();
        return true;
    } catch {
        return false;
    }
    return false;
}

// filefree -- the path comes from arg.txt, never the query string: it carries
// backslashes and spaces, and QArg does no URL decoding.
string AwaitFileFree(const string &in path, int timeoutMs) {
    if (path == "") return "{\"err\":\"no path\"}";
    uint t0 = Time::Now;
    int frames = 0;
    int budget = (timeoutMs > 0) ? timeoutMs : 3600000;
    bool everExisted = false;
    while (true) {
        if (IO::FileExists(path)) everExisted = true;
        if (everExisted && FileFree(path))
            return "{\"ok\":true,\"path\":\"" + path + "\",\"size\":" + IO::FileSize(path)
                 + ",\"ms\":" + (Time::Now - t0) + ",\"frames\":" + frames + "}";
        if (int(Time::Now - t0) > budget)
            return "{\"ok\":false,\"path\":\"" + path + "\",\"existed\":"
                 + (everExisted ? "true" : "false")
                 + ",\"ms\":" + (Time::Now - t0) + ",\"frames\":" + frames + "}";
        yield();
        frames++;
    }
    return "{\"err\":\"unreachable\"}";
}

// ---------------------------------------------------------------------------
// WHICH MAP IS LOADED, by IDENTITY.
//
// Loading a map into the editor is the single most expensive step in the
// pipeline -- 11.5 s for the 1.9 MB Underwater map, of which only ~3.5 s is
// entering the editor at all (measured against a 115 KB map, which took 3.8 s).
// The rest is the game building the scene, and it is real work.
//
// So the way to not pay it is to not do it twice. CGameCtnChallengeInfo::MapUid
// is the map's identity, and it is also readable straight out of the .Map.Gbx
// header, so the driver can tell whether the map it wants is ALREADY open
// before deciding to reload it. Comparing names would not do: two files can
// carry the same MapName, and our own edited copies do.
string LoadedMap() {
    auto app = GetApp();
    auto map = app.RootMap;
    if (map is null) return "{\"loaded\":false}";
    string uid = "";
    if (map.MapInfo !is null) uid = map.MapInfo.MapUid;
    return "{\"loaded\":true,\"uid\":\"" + uid + "\""
         + ",\"name\":\"" + map.MapName + "\""
         + ",\"ctx\":" + CurrentCtx() + "}";
}
