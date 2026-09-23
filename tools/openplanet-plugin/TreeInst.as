// TreeInst.as -- the FOREST INSTANCES the game made of the loaded map's
// vegetation items, read off the live NHmsForestVis manager: for every placed
// tree its quaternion, position and uniform scale after the per-instance
// variation (Trackmania.exe 0x14026b4f0: MurmurHash2 of the 28-byte pose seeds
// an LCG that draws the scale, a random world-Y yaw and two ±AngleMax tilts).
// This is the RUNTIME side of `mapgeom veget-instances MAP` (RE child 4,
// 2026-09-23): the two must agree bit for bit.
//
//   /treeinst[?from=A&n=B]    -> header + one TSV row per record A..A+B (default 0..4000):
//        i  model  flag  qw qx qy qz  x y z  scale     (%.9g floats)
//   /treeinst?raw=1           -> the same plus the raw hex words of each row
//   /treeinst?info=1          -> only the chain: scene, zone, forest mgr,
//                                arrays, counts (to debug the walk)
//   /treeinst?zone=1          -> the CHmsZone qwords +0x200..+0x2f8
//
// POINTER CHAIN [DISASSEMBLY + RUNTIME]: the map scene is the editor grid's
// CSceneObject.Scene (== app.GameScene.HackScene); its CScene.Sector.Zone is
// the CHmsZone of the world; the item spawner 0x141081910 reads the forest
// manager at zone+0x260 (zone = the model's bound zone at model+0x288 ==
// record+0x120; the block-scene builder 0x140dbf2a0 reads the same field);
// the forest manager (NHmsForestVis::SMgr)
// keeps the instance records at +0x188 (stride 0x20: quat w,x,y,z at +0,
// pos x,y,z at +0x10, scale at +0x1c -- 0x14026df20), the per-instance model
// index bytes at +0x198, the variation flag bytes at +0x1a8, the record count
// at +0x190, the live count at +0x1cc and the model pointer array at +0.
// Read-only.

string Hex64(uint64 v) {
    return Text::Format("%08x", uint(v >> 32)) + Text::Format("%08x", uint(v & 0xffffffff));
}

bool PlausiblePtr(uint64 p) {
    // a user-mode heap or module address on Windows x64
    return p > 0x10000 && p < 0x00007fffffffffff && (p & 7) == 0;
}

string TreeInst(const string &in qs) {
    auto app = GetApp();
    // the MAP scene: in the editor, app.GameScene.HackScene is a 20-mobil overlay
    // scene; the world the blocks and items live in is the scene of the editor's
    // grid mobil (CSceneObject.Scene)
    CScene@ scene = null;
    string root = "";
    auto ed = cast<CGameCtnEditorCommon>(app.Editor);
    if (ed !is null && ed.Grid !is null && ed.Grid.Scene !is null) { @scene = ed.Grid.Scene; root = "editor.Grid.Scene"; }
    if (scene is null && app.GameScene !is null) { @scene = app.GameScene.HackScene; root = "GameScene.HackScene"; }
    if (scene is null) return "no scene (no editor grid, no GameScene)";
    uint64 base = Dev::BaseAddress();
    string info = "root=" + root + " mobils=" + scene.Mobils.Length + " scene=" + Hex64(Dev::GetOffsetUint64(scene, 0)) + " (vtbl) base=" + Hex64(base) + "\n";
    // THE FOREST MANAGER HANGS OFF THE CHmsZone (CScene.Sector.Zone, class
    // 0x06004000, 0x468 B): the item spawner 0x141081910 reads it at zone+0x260
    // (zone = the model's bound zone, model+0x288 == record+0x120) and the
    // block-scene builder 0x140dbf2a0 l.1450 reads the same +0x260 of its zone.
    // (The scene-vis manager table at CScene+0x28 -> [+0x10 + idx*8] has slot
    // 27 = NSceneItem EMPTY in the editor scene; and probing +0x260 of every
    // table entry crashed the game once, 2026-09-23 15:53 -- never scan.)
    if (scene.Sector is null || scene.Sector.Zone is null) return info + "no Sector.Zone";
    CHmsZone@ zone = scene.Sector.Zone;
    info += "zone=" + Hex64(Dev::GetOffsetUint64(zone, 0)) + " (vtbl) " + Reflection::TypeOf(zone).Name + "\n";
    if (QArg(qs, "zone") == "1") {
        // the zone's qwords around +0x260, read INSIDE the zone object only
        for (uint k = 0x200; k < 0x300; k += 8) info += Text::Format("+0x%03x ", k) + Hex64(Dev::GetOffsetUint64(zone, k)) + "\n";
        return info;
    }
    uint64 forest = Dev::GetOffsetUint64(zone, 0x260);
    info += "forest=" + Hex64(forest) + "\n";
    if (!PlausiblePtr(forest)) return info + "no forest manager";
    uint64 recs = Dev::ReadUInt64(forest + 0x188);
    uint nrec = Dev::ReadUInt32(forest + 0x190);
    uint64 midx = Dev::ReadUInt64(forest + 0x198);
    uint64 flg = Dev::ReadUInt64(forest + 0x1a8);
    uint live = Dev::ReadUInt32(forest + 0x1cc);
    uint64 models = Dev::ReadUInt64(forest + 0);
    uint nmodels = Dev::ReadUInt32(forest + 8);
    info += "records=" + Hex64(recs) + " count=" + nrec + " live=" + live + " modelIdx=" + Hex64(midx) + " flags=" + Hex64(flg) + " models=" + Hex64(models) + " nmodels=" + nmodels + "\n";
    if (QArg(qs, "info") == "1") return info;
    if (!PlausiblePtr(recs) || nrec > 200000) return info + "records array not plausible";
    // probe the first and last record through the throwing reader first: an
    // unmapped page aborts this request instead of killing the game
    Dev::SafeReadUInt64(recs);
    Dev::SafeReadUInt64(recs + uint64(nrec) * 0x20 - 8);
    bool raw = QArg(qs, "raw") == "1";
    // paging: ?from=A&n=B (default the first 4000 records)
    uint from = QArg(qs, "from") == "" ? 0 : uint(Text::ParseInt(QArg(qs, "from")));
    uint cnt = QArg(qs, "n") == "" ? 4000 : uint(Text::ParseInt(QArg(qs, "n")));
    uint to = from + cnt < nrec ? from + cnt : nrec;
    string sb = info + "i\tmodel\tflag\tqw\tqx\tqy\tqz\tx\ty\tz\tscale\n";
    for (uint i = from; i < to; i++) {
        uint64 r = recs + uint64(i) * 0x20;
        float qw = Dev::ReadFloat(r + 0x0);
        float qx = Dev::ReadFloat(r + 0x4);
        float qy = Dev::ReadFloat(r + 0x8);
        float qz = Dev::ReadFloat(r + 0xc);
        float x = Dev::ReadFloat(r + 0x10);
        float y = Dev::ReadFloat(r + 0x14);
        float z = Dev::ReadFloat(r + 0x18);
        float s = Dev::ReadFloat(r + 0x1c);
        // an unused slot of the preallocated array is all zero (no unit quaternion)
        if (qw == 0 && qx == 0 && qy == 0 && qz == 0) continue;
        uint m = PlausiblePtr(midx) ? Dev::ReadUInt8(midx + i) : 255;
        uint f = PlausiblePtr(flg) ? Dev::ReadUInt8(flg + i) : 255;
        sb += i + "\t" + m + "\t" + f + "\t"
            + Text::Format("%.9g", qw) + "\t" + Text::Format("%.9g", qx) + "\t" + Text::Format("%.9g", qy) + "\t" + Text::Format("%.9g", qz) + "\t"
            + Text::Format("%.9g", x) + "\t" + Text::Format("%.9g", y) + "\t" + Text::Format("%.9g", z) + "\t" + Text::Format("%.9g", s);
        if (raw) {
            sb += "\t";
            for (uint k = 0; k < 8; k++) sb += Text::Format("%08x", Dev::ReadUInt32(r + 4 * k)) + (k < 7 ? " " : "");
        }
        sb += "\n";
    }
    return sb;
}
