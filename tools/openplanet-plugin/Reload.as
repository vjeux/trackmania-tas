// Reload.as -- the plugin reloads ITSELF, over HTTP.
//
// WHY: every change to this plugin used to cost a full game restart (~35 s,
// plus re-entering the map and the MediaTracker), which is why the pipeline
// kept getting patched in the shell instead of here -- the shell was the only
// layer with a fast edit loop. `Meta::ReloadPlugin` exists (found by dumping
// the strings in Openplanet.dll: "void ReloadPlugin(Plugin@ plugin)"), so the
// loop is now: write the file, curl /reload, test. About one second.
//
// The reload cannot happen inside the request handler -- that would tear down
// the coroutine mid-response and the caller gets a dropped connection instead
// of an answer. So the request only ARMS it, and Update() performs it on the
// next frame, after the response has gone out.

bool g_reloadArmed = false;

string ArmReload() {
    g_reloadArmed = true;
    return "reloading on the next frame";
}

// Called from Main.as's Update().
void ReloadTick() {
    if (!g_reloadArmed) return;
    g_reloadArmed = false;
    auto p = Meta::ExecutingPlugin();
    if (p is null) {
        trace("[GhostShooter] reload: no executing plugin");
        return;
    }
    trace("[GhostShooter] reloading " + p.Name + " on request");
    Meta::ReloadPlugin(p);
}
