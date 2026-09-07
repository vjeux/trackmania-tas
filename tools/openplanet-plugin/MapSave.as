// MapSave.as -- re-save, from the track editor, the map that is open in it.
//
// # Why this exists
//
// TMX 19298 "sportyxD-12" (a 2020 map) loads and instantiates PERFECTLY in the
// game client -- you can open it in the editor and render on it -- but it does
// NOT build in the 2026 dedicated server we use as a headless physics oracle:
// the car spawns on the void floor (y~9) instead of the start platform (y=74),
// so every ghost, including the official world record, validates as DNF cp0.
// The server reports "Can't load: 0%", i.e. it parses the file and then builds
// no track.
//
// The one thing known to produce a buildable copy is a re-save by the CURRENT
// editor: a community member's re-saved copy of this same map validates fine.
// This route performs that re-save headlessly, so the map can be fixed without
// a human at the keyboard.
//
//   /mapvalidate                              -- editor-side validation pass
//   /mapsave?path=Maps/_shoot/out.Map.Gbx     -- write it
//
// `SaveMap` takes a path relative to the user's Trackmania directory, NOT an
// absolute Windows path.

string MapSave(const string &in path) {
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return "not in an editor -- /editmap3 first";
    auto pmt = ed.PluginMapType;
    if (pmt is null) return "no PluginMapType on this editor";
    if (path == "") return "no ?path= given";
    pmt.SaveMap(path);
    return "SaveMap(\"" + path + "\") issued";
}

// The editor's own validation pass. A map the editor has not validated may be
// missing computed data a fresh save is expected to carry, so this is worth
// running before the save and worth being able to run alone.
string MapValidate() {
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return "not in an editor -- /editmap3 first";
    auto pmt = ed.PluginMapType;
    if (pmt is null) return "no PluginMapType on this editor";
    pmt.Validate();
    return "validate issued";
}

// What the editor thinks it has open: enough to tell a loaded map from an
// empty editor before blaming a save that never had anything to write.
string MapEditorState() {
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return "{\"editor\":null}";
    auto pmt = ed.PluginMapType;
    string name = "";
    uint blocks = 0;
    auto ch = ed.Challenge;
    if (ch !is null) {
        name = ch.MapName;
        blocks = ch.Blocks.Length;
    }
    return "{\"editor\":\"CGameCtnEditorFree\",\"pluginMapType\":"
        + (pmt is null ? "false" : "true")
        + ",\"map\":\"" + name + "\",\"blocks\":" + blocks + "}";
}

// Leave the track editor without going through its menus, so a fresh map can
// be opened. `/back` does not exit the editor; EditMap3 then refuses with
// "already in an editor". QuickQuit discards unsaved changes, which is what we
// want when a probe has dirtied the in-editor map.
string MapQuit() {
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return "not in an editor";
    auto pmt = ed.PluginMapType;
    if (pmt is null) return "no PluginMapType";
    pmt.QuickQuit();
    return "quickquit issued";
}

// Block inventory straight from the GAME's own parse of the map.
//
// Our offline GBX parser reads 46 block names on the real 19298 as
// `<bad id N>` -- its lookback-table decode diverges on that file -- so every
// count taken from it is suspect, including the one that matters:
// `GateSpecialNoSteering` 17x on the real map vs 1x on the editor re-save.
// If that difference is real, the re-save is not a faithful repair; if it is a
// parser artifact, it is. The client is the authority, so ask the client.
//
//   /mapblocks            -> {"total":N,"names":{"<BlockModel.Name>":count,...}}
//
// Counts only; the editor's Blocks array on this map is ~2400 entries and
// emitting one line each would be a large response for no extra information.
string MapBlocks() {
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return "not in an editor -- /editmap3 first";
    auto ch = ed.Challenge;
    if (ch is null) return "no Challenge on this editor";

    array<string> names;
    array<uint> counts;
    uint total = ch.Blocks.Length;
    for (uint i = 0; i < total; i++) {
        auto b = ch.Blocks[i];
        if (b is null) continue;
        string n = "<null model>";
        if (b.BlockModel !is null) n = b.BlockModel.Name;
        int at = names.Find(n);
        if (at < 0) { names.InsertLast(n); counts.InsertLast(1); }
        else { counts[at] = counts[at] + 1; }
    }

    string js = "{\"total\":" + total + ",\"names\":{";
    for (uint i = 0; i < names.Length; i++) {
        if (i > 0) js += ",";
        js += "\"" + names[i] + "\":" + counts[i];
    }
    return js + "}}";
}

// Gate positions straight from the GAME's parse.
//
// Our offline parser mis-decodes the real 19298 (46 `<bad id N>` block names),
// so the hoop cell coordinates taken from it -- and every world position and
// gate objective derived from them -- are suspect. `/mapblocks` already showed
// one of its counts was wrong. The game is the authority; ask it for the
// coordinates too.
//
//   /mapgates -> [{"name":..,"x":..,"y":..,"z":..,"dir":..}, ...]
//
// Emits every block whose model name contains "Gate" or "Start" (checkpoints,
// finishes, boosters, the spawn) -- the handful that define the route, not the
// 2400-block scenery. Coord is the editor's integer block cell.
string MapGates() {
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return "not in an editor -- /editmap3 first";
    auto ch = ed.Challenge;
    if (ch is null) return "no Challenge on this editor";

    string js = "[";
    bool first = true;
    for (uint i = 0; i < ch.Blocks.Length; i++) {
        auto b = ch.Blocks[i];
        if (b is null || b.BlockModel is null) continue;
        string n = b.BlockModel.Name;
        if (n.IndexOf("Gate") < 0 && n.IndexOf("Start") < 0) continue;
        if (!first) js += ",";
        first = false;
        js += "{\"name\":\"" + n + "\",\"x\":" + b.Coord.x
            + ",\"y\":" + b.Coord.y + ",\"z\":" + b.Coord.z
            + ",\"dir\":" + int(b.Direction) + "}";
    }
    return js + "]";
}


// Delete only JxshTM's three mock-only FREE checkpoint blocks.
//
// Offline model renaming left each block's waypoint-special-property attached
// to a StructurePillar model; the dedicated server then failed to initialise a
// car. The editor owns all correlated metadata, so remove through its safe API
// and let SaveMap rebuild the file consistently.
//
// Match is intentionally strict:
//   model == GateCheckpoint, free block coord (uint max), exactly 3 removals.
// The three real hoop checkpoints have normal cells and cannot match. The
// extra GateExpandableFinish is intentionally KEPT: its presence is what makes
// the old map initialise a driven car in the 2026 server. With the easy ground
// checkpoints gone, that finish cannot fire until the three real hoops have.
string StripGroundCheckpoints() {
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return "not in an editor";
    auto pmt = ed.PluginMapType;
    auto ch = ed.Challenge;
    if (pmt is null || ch is null) return "no PluginMapType/Challenge";

    array<CGameCtnBlock@> doomed;
    for (uint i = 0; i < ch.Blocks.Length; i++) {
        auto b = ch.Blocks[i];
        if (b is null || b.BlockModel is null) continue;
        if (b.BlockModel.Name != "GateCheckpoint") continue;
        if (b.Coord.x < 1000000 || b.Coord.z < 1000000) continue;
        doomed.InsertLast(b);
    }
    if (doomed.Length != 3) {
        return "REFUSED: expected exactly 3 free GateCheckpoint blocks, found " + doomed.Length;
    }
    uint ok = 0;
    for (uint i = 0; i < doomed.Length; i++) {
        auto b = doomed[i];
        auto dir = CGameEditorPluginMap::ECardinalDirections(int(b.Direction));
        // Free blocks expose (-1,0,-1) in Block.Coord, which is a sentinel and
        // cannot identify the placed instance. Their first block unit carries
        // the actual snapped cell the editor APIs expect.
        nat3 c = b.Coord;
        if (b.BlockUnits.Length > 0) c = b.BlockUnits[0].AbsoluteOffset;
        if (pmt.RemoveBlock(int3(int(c.x), int(c.y), int(c.z)))) ok++;
    }
    return "removed " + ok + "/" + doomed.Length + " ground checkpoints; blocks now " + ch.Blocks.Length;
}

