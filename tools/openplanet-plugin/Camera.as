// Camera.as -- set the camera by VALUE.
//
// CGameCtnMediaBlockCameraGame's GameCam and ClipEntId are exposed READ-ONLY to
// AngelScript ("The property has no set accessor"), which is why the old
// pipeline cycled a button and re-read the value up to twelve times per field.
// Dev::SetOffset writes them directly.
//
// THE OFFSETS ARE LOOKED UP AT RUNTIME, never hardcoded: a game update moves
// them, and a stale constant here would write into some neighbouring field and
// corrupt the block silently. Reflection gives the offset by NAME, so the write
// is as safe as the name is stable.

uint16 MemberOffset(const string &in tname, const string &in member) {
    auto ti = Reflection::GetType(tname);
    if (ti is null) return 65535;
    for (uint i = 0; i < ti.Members.Length; i++)
        if (ti.Members[i].Name == member) return ti.Members[i].Offset;
    return 65535;
}

string CameraSet(int ent, int cam) {
    auto api = MTApi();
    if (api is null) return "not MT";
    auto clip = api.Clip;
    if (clip is null) return "no clip";

    uint16 offEnt = MemberOffset("CGameCtnMediaBlockCameraGame", "ClipEntId");
    uint16 offCam = MemberOffset("CGameCtnMediaBlockCameraGame", "GameCam");
    if (offEnt == 65535 || offCam == 65535)
        return "could not resolve offsets (ClipEntId=" + offEnt + " GameCam=" + offCam + ")";

    int n = 0;
    for (uint i = 0; i < clip.Tracks.Length; i++) {
        for (uint b = 0; b < clip.Tracks[i].Blocks.Length; b++) {
            auto cg = cast<CGameCtnMediaBlockCameraGame>(clip.Tracks[i].Blocks[b]);
            if (cg is null) continue;
            Dev::SetOffset(cg, offEnt, uint(ent));
            Dev::SetOffset(cg, offCam, uint(cam));
            n++;
        }
    }
    // Read it BACK through the normal property and report that -- a write that
    // is not read back is a claim, not a result.
    string got = "";
    for (uint i = 0; i < clip.Tracks.Length; i++) {
        for (uint b = 0; b < clip.Tracks[i].Blocks.Length; b++) {
            auto cg = cast<CGameCtnMediaBlockCameraGame>(clip.Tracks[i].Blocks[b]);
            if (cg is null) continue;
            got += " [ent=" + cg.ClipEntId + " cam=" + int(cg.GameCam) + "]";
        }
    }
    return "wrote " + n + " camera block(s);" + got;
}
