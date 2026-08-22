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

string FindShootDialog() {
    return "unavailable: no typed member holds CGameDialogShootParams@, and a "
         "blind memory scan for it crashes the game (see the note above)";
}

CGameDialogShootParams@ ShootDialog() {
    // No safe route to the instance yet. Left here so the routes compile and
    // report honestly rather than pretending to work.
    return null;
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
    auto dlg = GetApp().BasicDialogs;
    if (dlg !is null && dlg.Dialogs !is null && dlg.Dialogs.CurrentFrame !is null)
        j += ",\"dialog\":\"" + dlg.Dialogs.CurrentFrame.IdName + "\"";
    else j += ",\"dialog\":null";
    j += "}";
    return j;
}
