// /lmlights — the scene's LIGHT LIST as the lightmapper sees it (RE 7, 2026-09-25 23:20Z: frame 1 of the mapping head is the
// LOCAL-LIGHT accumulation; what lights stpad's frame 1 is open). Hops: scene = *(zone+0x18); list = *(scene+0x238),
// count = *(u32*)(scene+0x228), entries 16 B {light ptr, owner ptr}; per light: the vtable RVA, *(u32*)(light+0x20) (kept
// iff bit 0), light+0x00..0xc0 raw as 48 dwords (hex) + as floats, the owner's vtable RVA; also *(u32*)(scene+0x240).
// Output: lmlights-<tag>-<stamp>.tsv in PluginStorage + a summary; every array probed at both ends.

string LmLightsDump(const string &in tagBase) {
    auto app = GetApp();
    CHmsZone@ zone = null;
    string root = "";
    auto ed = cast<CGameCtnEditorCommon>(app.Editor);
    if (ed !is null && ed.Grid !is null && ed.Grid.Scene !is null && ed.Grid.Scene.Sector !is null) { @zone = ed.Grid.Scene.Sector.Zone; root = "editor.Grid.Scene"; }
    if (zone is null && app.GameScene !is null && app.GameScene.HackScene !is null && app.GameScene.HackScene.Sector !is null) { @zone = app.GameScene.HackScene.Sector.Zone; root = "GameScene.HackScene"; }
    if (zone is null) return "no zone (no editor grid, no GameScene)\n";
    uint64 scene = Dev::GetOffsetUint64(zone, 0x18);
    string info = "root=" + root + " scene(zone+0x18)=" + Hex64(scene) + "\n";
    if (!PlausiblePtr(scene)) return info + "scene pointer not plausible\n";
    Dev::SafeReadUInt64(scene + 0x240);
    uint64 list = Dev::SafeReadUInt64(scene + 0x238);
    uint count = Dev::SafeReadUInt32(scene + 0x228);
    uint c240 = Dev::SafeReadUInt32(scene + 0x240);
    info += "scene vt.rva=" + Hex64(LmVtRva(scene)) + " lights=" + Hex64(list) + " count(+0x228)=" + count + " [+0x240]=" + c240 + "\n";
    // the count: +0x228 when plausible, else the +0x240 field (stpad/hill4 read +0x228 = 0xffffffff, +0x240 = 2)
    if (count > 100000) { count = c240; info += "count taken from +0x240 = " + count + "\n"; }
    if (count > 100000) return info + "count not plausible\n";
    // the scene's first 0x100 bytes as qwords, for RE 7 to place the fields
    try { string q = ""; for (uint k = 0; k < 0x280; k += 8) q += (k > 0 ? " " : "") + Text::Format("%x", k) + ":" + Hex64(Dev::SafeReadUInt64(scene + k)); info += "scene.q=[" + q + "]\n"; } catch {}
    string tag = tagBase + "-" + Time::Stamp;
    string sb = "i\tlight\tlight.vt.rva\tflags20\towner\towner.vt.rva\traw00..bc(hex)\traw00..bc(f32)\n";
    uint kept = 0;
    if (PlausiblePtr(list) && count > 0) {
        Dev::SafeReadUInt64(list);
        Dev::SafeReadUInt64(list + uint64(count) * 16 - 8);
        for (uint i = 0; i < count; i++) {
            uint64 light = Dev::ReadUInt64(list + uint64(i) * 16);
            uint64 owner = Dev::ReadUInt64(list + uint64(i) * 16 + 8);
            string row = i + "\t" + Hex64(light) + "\t";
            if (PlausiblePtr(light)) {
                try {
                    Dev::SafeReadUInt64(light + 0xb8);
                    uint fl = Dev::SafeReadUInt32(light + 0x20);
                    if ((fl & 1) != 0) kept++;
                    row += Hex64(LmVtRva(light)) + "\t" + Text::Format("%08x", fl) + "\t" + Hex64(owner) + "\t" + (PlausiblePtr(owner) ? Hex64(LmVtRva(owner)) : "") + "\t";
                    string hx = "", fx = "";
                    for (uint k = 0; k < 48; k++) {
                        uint w = Dev::ReadUInt32(light + 4 * k);
                        hx += (k > 0 ? " " : "") + Text::Format("%08x", w);
                        fx += (k > 0 ? " " : "") + Text::Format("%.9g", Dev::ReadFloat(light + 4 * k));
                    }
                    row += hx + "\t" + fx;
                } catch {
                    row += "(unreadable: " + getExceptionInfo() + ")";
                }
            } else {
                row += "(not a pointer)";
            }
            sb += row + "\n";
            if ((i % 200) == 199) yield();
        }
    }
    LmWriteFile(tag + ".tsv", sb);
    info += "wrote " + tag + ".tsv (" + count + " lights, " + kept + " with bit 0 of +0x20)\n";
    // THE LIGHT INSTANCE ARRAY (RE 7, 00:45Z): FUN_1401e6910(scene) = {ptr scene+0x28, count scene+0x30}, entries 0x50 B:
    // +0 the light object (GxLightBall/Spot), +8..+0x37 the Iso4 (3 rows then the position at +0x2c), +0x38 idx|flags,
    // +0x40 a flag word; per light object the fields RE 7 named (+0x20, +0x9c, +0xa0..+0xb3 radii, +0xb4/+0xb8, +0x88/+0x8c,
    // +0x90/+0x94, +0x100/+0x104/+0x108, +0x118) — written as lmlights-inst-<tag>.tsv
    try {
        uint64 inst = Dev::SafeReadUInt64(scene + 0x28);
        uint nInst = Dev::SafeReadUInt32(scene + 0x30);
        info += "instances(scene+0x28)=" + Hex64(inst) + " count(+0x30)=" + nInst + "\n";
        if (PlausiblePtr(inst) && nInst > 0 && nInst < 200000) {
            Dev::SafeReadUInt64(inst);
            Dev::SafeReadUInt64(inst + uint64(nInst) * 0x50 - 8);
            string ib = "i\tlight\tlight.vt.rva\tw38\tw40\tm0\tm1\tm2\tm3\tm4\tm5\tm6\tm7\tm8\tpx\tpy\tpz\tf20\tf9c\tr_a0\tr_a4\tr_a8\tr_ac\tr_b0\tf_b4\tf_b8\tatt88\tatt8c\thyp90\thyp94\tang100\tang104\tang108\tcos118\n";
            uint written = 0;
            for (uint i = 0; i < nInst; i++) {
                uint64 e = inst + uint64(i) * 0x50;
                uint64 light = Dev::ReadUInt64(e);
                string row = i + "\t" + Hex64(light) + "\t" + (PlausiblePtr(light) ? Hex64(LmVtRva(light)) : "") + "\t" + Text::Format("%08x", Dev::ReadUInt32(e + 0x38)) + "\t" + Text::Format("%08x", Dev::ReadUInt32(e + 0x40));
                for (uint k = 0; k < 12; k++) row += "\t" + Text::Format("%.9g", Dev::ReadFloat(e + 8 + 4 * k));
                if (PlausiblePtr(light)) {
                    try {
                        Dev::SafeReadUInt64(light + 0x118);
                        row += "\t" + Text::Format("%08x", Dev::SafeReadUInt32(light + 0x20)) + "\t" + Text::Format("%08x", Dev::SafeReadUInt32(light + 0x9c));
                        for (uint k = 0; k < 5; k++) row += "\t" + Text::Format("%.9g", Dev::ReadFloat(light + 0xa0 + 4 * k));
                        row += "\t" + Text::Format("%.9g", Dev::ReadFloat(light + 0xb4)) + "\t" + Text::Format("%.9g", Dev::ReadFloat(light + 0xb8));
                        row += "\t" + Text::Format("%.9g", Dev::ReadFloat(light + 0x88)) + "\t" + Text::Format("%.9g", Dev::ReadFloat(light + 0x8c));
                        row += "\t" + Text::Format("%.9g", Dev::ReadFloat(light + 0x90)) + "\t" + Text::Format("%.9g", Dev::ReadFloat(light + 0x94));
                        row += "\t" + Text::Format("%.9g", Dev::ReadFloat(light + 0x100)) + "\t" + Text::Format("%.9g", Dev::ReadFloat(light + 0x104)) + "\t" + Text::Format("%.9g", Dev::ReadFloat(light + 0x108));
                        row += "\t" + Text::Format("%.9g", Dev::ReadFloat(light + 0x118));
                    } catch {
                        row += "\t(light unreadable)";
                    }
                }
                ib += row + "\n";
                written++;
                if ((i % 200) == 199) { LmAppend(tag + "-inst.tsv", ib); ib = ""; yield(); }
            }
            if (written <= 199) LmWriteFile(tag + "-inst.tsv", ib); else if (ib.Length > 0) LmAppend(tag + "-inst.tsv", ib);
            info += "wrote " + tag + "-inst.tsv (" + written + " light instances)\n";
        }
    } catch {
        info += "light instances unreadable: " + getExceptionInfo() + "\n";
    }
    return info;
}

string LmLightsRoute(const string &in qs) {
    try {
        return LmLightsDump("lmlights");
    } catch {
        return "lights threw: " + getExceptionInfo() + "\n";
    }
}
