// MtProbe.as -- the MediaTracker editor's camera/trigger switches, read and set.
//
// WHY (2026-09-09, video session 2): on Summer 01/02/03/05/12 the SHOOT renders
// the whole clip from the map's THUMBNAIL camera (chunk 0x03043036 position,
// fov 70) with no car in frame, while live playback of the same clip follows
// the car with the chase camera; 11/14/15/16/17/24 shoot fine. Everything the
// setup reports is identical, so these routes expose what the editor itself
// thinks about the clip camera (UseClipCamWhenAvailable / CanUseClipCam), the
// triggers mode and the ghost-ref mode, and let a probe flip them.
string MtFlags() {
    auto api = MTApi();
    if (api is null) return "not MT";
    string sb = "{";
    sb += "\"useClipCam\":" + (api.UseClipCamWhenAvailable ? "1" : "0");
    sb += ",\"canUseClipCam\":" + (api.CanUseClipCam ? "1" : "0");
    sb += ",\"orbitalNotFree\":" + (api.UseOrbitalInsteadOfFreeCam ? "1" : "0");
    sb += ",\"triggersMode\":" + (api.IsTriggersModeOn() ? "1" : "0");
    sb += ",\"recordGhostMode\":" + (api.IsRecordGhostModeOn() ? "1" : "0");
    sb += ",\"playing\":" + (api.IsPlaying() ? "1" : "0");
    sb += ",\"timer\":" + api.CurrentTimer;
    sb += ",\"editMode\":" + tostring(api.EditMode);
    sb += ",\"clipConditionValue\":" + api.ClipConditionValue;
    sb += "}";
    return sb;
}

string MtSet(const string &in what, const string &in val) {
    auto api = MTApi();
    if (api is null) return "not MT";
    bool on = (val == "1" || val == "true");
    if (what == "clipcam") { api.UseClipCamWhenAvailable = on; return MtFlags(); }
    if (what == "orbital") { api.UseOrbitalInsteadOfFreeCam = on; return MtFlags(); }
    if (what == "resettrig") { api.ResetTriggerZone(); return "reset trigger zone; " + MtFlags(); }
    if (what == "trigmode") { api.ToggleTriggersMode(); return MtFlags(); }
    if (what == "ghostref") { api.ToggleGhostRef(); return MtFlags(); }
    if (what == "stopghostref") { api.StopGhostRefPreview(); return MtFlags(); }
    if (what == "clipcond") { api.ToggleClipCondition(); return MtFlags(); }
    return "usage: /mtset?what=clipcam|orbital|resettrig|trigmode|ghostref|stopghostref|clipcond&val=1|0";
}

// A clip's trigger-mode bools, written by member offset (the properties are
// read-only to AngelScript, like CameraGame's ClipEntId): /mtclip1?what=
// beforestart|stoprespawn|stopleave&val=1|0 on the CURRENT clip, read back.
string MtClipFlag(const string &in what, const string &in val) {
    auto api = MTApi();
    if (api is null) return "not MT";
    auto clip = api.Clip;
    if (clip is null) return "no clip";
    string member = "";
    if (what == "beforestart") member = "TriggersBeforeRaceStart";
    if (what == "stoprespawn") member = "StopWhenRespawn";
    if (what == "stopleave") member = "StopWhenLeave";
    if (member == "") return "usage: /mtclip1?what=beforestart|stoprespawn|stopleave&val=1|0";
    uint16 off = MemberOffset("CGameCtnMediaClip", member);
    if (off == 65535) return "could not resolve CGameCtnMediaClip::" + member;
    uint8 v = (val == "1" || val == "true") ? 1 : 0;
    Dev::SetOffset(clip, off, v);
    return "{\"clip\":\"" + clip.Name + "\",\"" + member + "\":" + (what == "beforestart" ? (clip.TriggersBeforeRaceStart ? "1" : "0") : (what == "stoprespawn" ? (clip.StopWhenRespawn ? "1" : "0") : (clip.StopWhenLeave ? "1" : "0"))) + ",\"offset\":" + off + "}";
}

// Hide / show the MediaTracker editor's interface for a live-playback screen capture
// (the shoot's fallback on Summer 01/02/03/05/12 leaves live playback as the only
// camera that follows the car): /mtui?hide=1|0
string MtUi(const string &in hide) {
    auto api = MTApi();
    if (api is null) return "not MT";
    bool h = (hide == "1" || hide == "true");
    string r = "{";
    try { api.ToolBarSetVisible(!h); r += "\"toolbar\":\"" + (h ? "hidden" : "shown") + "\""; } catch { r += "\"toolbar\":\"" + getExceptionInfo() + "\""; }
    try { api.SetTempHidePropertyList(h); r += ",\"propertyList\":\"" + (h ? "hidden" : "shown") + "\""; } catch { r += ",\"propertyList\":\"" + getExceptionInfo() + "\""; }
    if (h) { try { api.InformInterfaceIsHidden(); r += ",\"informHidden\":1"; } catch { r += ",\"informHidden\":\"" + getExceptionInfo() + "\""; } }
    r += "}";
    return r;
}

// The loaded map's author/validation ghost as the game holds it (the MT's "Ref. Ghost:
// Author ghost"), and — /authghost?clear=1 — the pointer nulled. On Summer 01/02/03/05/10/12
// the editor holds one although the FILE carries no ghost chunk; those are exactly the maps
// whose shoot renders the thumbnail camera with no car.
string AuthGhost(const string &in clear) {
    auto app = GetApp();
    auto map = app.RootMap;
    if (map is null) return "no RootMap";
    auto p = map.ChallengeParameters;
    if (p is null) return "no ChallengeParameters";
    auto g = p.RaceValidateGhost;
    string r = "{\"raceValidateGhost\":";
    if (g is null) r += "null";
    else r += "{\"time\":" + g.RaceTime + ",\"nick\":\"" + string(g.GhostNickname) + "\"}";
    r += ",\"authorTime\":" + p.AuthorTime;
    if (clear == "1" && g !is null) {
        uint16 off = MemberOffset("CGameCtnChallengeParameters", "RaceValidateGhost");
        if (off == 65535) {
            r += ",\"cleared\":\"no offset for RaceValidateGhost\"";
        } else {
            Dev::SetOffset(p, off, uint64(0));
            r += ",\"cleared\":" + (p.RaceValidateGhost is null ? "1" : "0") + ",\"offset\":" + off;
        }
    }
    return r + "}";
}
