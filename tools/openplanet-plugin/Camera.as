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

// ---------------------------------------------------------------------------
// THE SCENE ENDS WHEN THE FIRST CAR FINISHES
// ---------------------------------------------------------------------------
//
// The MediaTracker renders the whole clip, and the clip is as long as its
// longest block. Both of those are the game's, and neither is what we want to
// publish: filming a 218.812 s run against a 441.002 s human record produced
// 441 s of video, of which the last 222 s is a camera bolted to a car that
// finished four minutes earlier -- and when the TARGET car's own block ends,
// the camera has nothing to follow and drifts off to the top of the map. One
// scene-length bug, seen as two symptoms.
//
// So the driver computes the end from the ghosts it staged and sets it here.
// Blocks' Start/End are read-only to AngelScript for the same reason ClipEntId
// is, so the same Dev::SetOffset-by-name mechanism applies -- offsets resolved
// at runtime, never a constant, and read back through the normal property
// afterwards, because a write that is not read back is a claim.
//
// CGameCtnMediaBlock is the base of every block type, so one offset lookup
// covers camera blocks, entity blocks and anything else in the clip.

// The argument is MILLISECONDS -- an integer, because `Text::ParseInt` is what
// this plugin's query parser has and a float spelling would be one more thing
// to get wrong at the wire. Block times are seconds, so it converts here.
// EXTEND the camera to cover the car.
//
// ClipEnd below shortens, and its comment says why extending is dangerous: a
// camera block that outlives its target's entity block follows a car that is
// not there and drifts off the map. That is true, and this does not do it.
//
// The opposite defect is what this fixes. `/mktrack` writes a camera block of
// about 3 s regardless of the run's length, so on a 21.99 s lap the camera
// stops tracking at 3 s and the game falls back to its own view -- the car
// flies the remaining 19 s unwatched and the finish is never on screen. Three
// separate clips were published tonight before the cause was found, because
// the symptom (an empty final frame) looks exactly like a bad camera angle.
//
// The safe bound is the car's own data: extend camera blocks to the longest
// ENTITY block, never past it. So the invariant is unchanged -- a camera never
// outlives its target -- and the gap in the other direction is closed.
//
// MemberOffset is resolved against the CONCRETE class. The abstract base
// CGameCtnMediaBlock has no reflected `End` and returns 65535 ("could not
// resolve"), which is what defeated the first attempt at this.
string ClipCam(int endMs) {
    auto api = MTApi();
    if (api is null) return "not MT";
    auto clip = api.Clip;
    if (clip is null) return "no clip";

    // The cap comes from the entity blocks, not from the argument: the caller
    // asks for a length, the CAR decides what is honest.
    float carEnd = 0;
    for (uint i = 0; i < clip.Tracks.Length; i++) {
        for (uint b = 0; b < clip.Tracks[i].Blocks.Length; b++) {
            auto blk = cast<CGameCtnMediaBlockEntity>(clip.Tracks[i].Blocks[b]);
            if (blk is null) continue;
            if (blk.End > carEnd) carEnd = blk.End;
        }
    }
    if (carEnd <= 0) return "no entity block to bound the camera by";

    float want = (endMs > 0) ? (float(endMs) / 1000.0f) : carEnd;
    if (want > carEnd) want = carEnd;   // never outlive the car

    uint16 offEnd = MemberOffset("CGameCtnMediaBlockCameraGame", "End");
    if (offEnd == 65535) return "could not resolve CGameCtnMediaBlockCameraGame::End";

    int n = 0;
    float before = 0;
    for (uint i = 0; i < clip.Tracks.Length; i++) {
        for (uint b = 0; b < clip.Tracks[i].Blocks.Length; b++) {
            auto cam = cast<CGameCtnMediaBlockCameraGame>(clip.Tracks[i].Blocks[b]);
            if (cam is null) continue;
            if (cam.End > before) before = cam.End;
            if (cam.End < want) { Dev::SetOffset(cam, offEnd, want); n++; }
        }
    }

    // Read back through the normal property: a write that is not read back is
    // a claim, not a result.
    float after = 0;
    for (uint i = 0; i < clip.Tracks.Length; i++) {
        for (uint b = 0; b < clip.Tracks[i].Blocks.Length; b++) {
            auto cam = cast<CGameCtnMediaBlockCameraGame>(clip.Tracks[i].Blocks[b]);
            if (cam is null) continue;
            if (cam.End > after) after = cam.End;
        }
    }
    return "{\"carEnd\":" + carEnd + ",\"want\":" + want + ",\"extended\":" + n
         + ",\"camBefore\":" + before + ",\"camAfter\":" + after + "}";
}

string ClipEnd(int endMs) {
    auto api = MTApi();
    if (api is null) return "not MT";
    auto clip = api.Clip;
    if (clip is null) return "no clip";
    if (endMs <= 0) return "refusing a non-positive clip end";
    float endSecs = float(endMs) / 1000.0f;

    uint16 offEnd = MemberOffset("CGameCtnMediaBlock", "End");
    if (offEnd == 65535) return "could not resolve CGameCtnMediaBlock::End";

    int n = 0;
    float longest = 0;
    for (uint i = 0; i < clip.Tracks.Length; i++) {
        for (uint b = 0; b < clip.Tracks[i].Blocks.Length; b++) {
            auto blk = clip.Tracks[i].Blocks[b];
            if (blk is null) continue;
            if (blk.End > longest) longest = blk.End;
            // SHORTEN ONLY. Extending a block past its own data is what
            // produces a camera following a car that is not there any more,
            // which is the defect this exists to end -- doing it deliberately
            // here would be the same bug with our name on it.
            if (blk.End > endSecs) {
                Dev::SetOffset(blk, offEnd, endSecs);
                n++;
            }
        }
    }
    string got = "";
    float after = 0;
    for (uint i = 0; i < clip.Tracks.Length; i++) {
        for (uint b = 0; b < clip.Tracks[i].Blocks.Length; b++) {
            auto blk = clip.Tracks[i].Blocks[b];
            if (blk is null) continue;
            if (blk.End > after) after = blk.End;
        }
    }
    got = "{\"askedMs\":" + endMs + ",\"asked\":" + endSecs + ",\"shortened\":" + n
        + ",\"longestBefore\":" + longest + ",\"longestAfter\":" + after + "}";
    return got;
}
