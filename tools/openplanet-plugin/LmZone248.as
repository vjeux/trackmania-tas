// /lmzone248 — the zone's SPECIAL STATIC MESHES (zone+0x248, RE 7's hops from RenderLighting_Frames l.1150–1300 +
// FUN_140264e10 / FUN_140263380), a probe-box source of the lightmapper after the item records:
//   obj = *(zone+0x248); obj+0x30 u32 (the pose path is the == 1 branch)
//   groups: count *(u32*)(obj+0x68), array *(ptr*)(obj+0x60), entry stride 0x78; group = *(ptr*)(entry+0x10) (NULL → skip)
//   per group: nGeoms *(u32*)(group+0x160), geoms *(ptr*)(group+0x158) stride 0x10 {u32 visualIdx, u32 materialIdx, …};
//     visuals = *(ptr*)(group+0xa8) (pointer array) → visual = visuals[visualIdx]; BOX = 6 f32 at visual+0x88 (centre, half);
//     materials = *(ptr*)(group+0x208) (pointer array), nMaterials *(u32*)(group+0x210)
//   poses (obj+0x30 == 1): idx = *(u32*)(*(ptr*)(obj+0x1d8) + 4·g); 0x7fffffff = none; (int)idx < 0 → pose at
//     *(ptr*)(obj+0x1e8) + (idx & 0x7fffffff)·0x20, flag *(u32*)(*(ptr*)(obj+0x208) + 4·(idx & 0x7fffffff)); else pose at
//     *(ptr*)(obj+0xc0) + idx·0x20, flag *(u32*)(*(ptr*)(obj+0xe0) + 4·idx); pose = {f32 quat[4], f32 pos[3], u32 pad}
// Output: lmzone248-<stamp>-groups.tsv (g, group, poseIdx, quat, pos, flag, nGeoms, nMaterials) and -geoms.tsv (g, k,
// visualIdx, materialIdx, material ptr, box centre/half %.9g). Every array is probed at both ends before the plain reads.

string LmZone248Dump(const string &in tagBase) {
    auto app = GetApp();
    CHmsZone@ zone = null;
    string root = "";
    auto ed = cast<CGameCtnEditorCommon>(app.Editor);
    if (ed !is null && ed.Grid !is null && ed.Grid.Scene !is null && ed.Grid.Scene.Sector !is null) { @zone = ed.Grid.Scene.Sector.Zone; root = "editor.Grid.Scene"; }
    if (zone is null && app.GameScene !is null && app.GameScene.HackScene !is null && app.GameScene.HackScene.Sector !is null) { @zone = app.GameScene.HackScene.Sector.Zone; root = "GameScene.HackScene"; }
    if (zone is null) return "no zone (no editor grid, no GameScene)\n";
    uint64 obj = Dev::GetOffsetUint64(zone, 0x248);
    string info = "root=" + root + " obj(zone+0x248)=" + Hex64(obj) + "\n";
    if (!PlausiblePtr(obj)) return info + "obj not plausible\n";
    Dev::SafeReadUInt64(obj + 0x208);
    uint mode = Dev::SafeReadUInt32(obj + 0x30);
    uint nGroups = Dev::SafeReadUInt32(obj + 0x68);
    uint64 groups = Dev::SafeReadUInt64(obj + 0x60);
    info += "obj+0x30=" + mode + " groups=" + Hex64(groups) + " count=" + nGroups + "\n";
    if (nGroups > 100000) return info + "group count not plausible\n";
    string tag = tagBase + "-" + Time::Stamp;
    string gb = "g\tentry\tgroup\tposeIdx\tq0\tq1\tq2\tq3\tpx\tpy\tpz\tflag\tnGeoms\tnMaterials\tvisuals\tmaterials\n";
    string kb = "g\tk\tvisualIdx\tmaterialIdx\tvisual\tmaterial\tcx\tcy\tcz\thx\thy\thz\n";
    uint nGeomsAll = 0, nKept = 0;
    if (PlausiblePtr(groups) && nGroups > 0) {
        Dev::SafeReadUInt64(groups);
        Dev::SafeReadUInt64(groups + uint64(nGroups) * 0x78 - 8);
        for (uint g = 0; g < nGroups; g++) {
            uint64 entry = groups + uint64(g) * 0x78;
            uint64 group = Dev::ReadUInt64(entry + 0x10);
            string poseStr = "\t\t\t\t\t\t\t\t";
            if (mode == 1) {
                try {
                    uint64 idxArr = Dev::SafeReadUInt64(obj + 0x1d8);
                    uint idx = Dev::SafeReadUInt32(idxArr + 4 * uint64(g));
                    if (idx == 0x7fffffff) {
                        poseStr = "\tnone\t\t\t\t\t\t\t\t";
                    } else {
                        uint64 pose; uint flag;
                        if ((idx & 0x80000000) != 0) {
                            uint j = idx & 0x7fffffff;
                            pose = Dev::SafeReadUInt64(obj + 0x1e8) + uint64(j) * 0x20;
                            flag = Dev::SafeReadUInt32(Dev::SafeReadUInt64(obj + 0x208) + 4 * uint64(j));
                        } else {
                            pose = Dev::SafeReadUInt64(obj + 0xc0) + uint64(idx) * 0x20;
                            flag = Dev::SafeReadUInt32(Dev::SafeReadUInt64(obj + 0xe0) + 4 * uint64(idx));
                        }
                        Dev::SafeReadUInt64(pose + 0x18);
                        poseStr = "\t" + Text::Format("%08x", idx);
                        for (uint c = 0; c < 7; c++) poseStr += "\t" + Text::Format("%.9g", Dev::ReadFloat(pose + 4 * c));
                        poseStr += "\t" + Text::Format("%08x", flag);
                    }
                } catch {
                    poseStr = "\t(pose unreadable: " + getExceptionInfo() + ")\t\t\t\t\t\t\t\t";
                }
            }
            if (!PlausiblePtr(group)) { gb += g + "\t" + Hex64(entry) + "\t" + Hex64(group) + poseStr + "\t0\t0\t\t\n"; continue; }
            uint nGeoms = 0, nMat = 0; uint64 geoms = 0, visuals = 0, materials = 0;
            try {
                Dev::SafeReadUInt64(group + 0x210);
                nGeoms = Dev::SafeReadUInt32(group + 0x160);
                geoms = Dev::SafeReadUInt64(group + 0x158);
                visuals = Dev::SafeReadUInt64(group + 0xa8);
                materials = Dev::SafeReadUInt64(group + 0x208);
                nMat = Dev::SafeReadUInt32(group + 0x210);
            } catch {
                gb += g + "\t" + Hex64(entry) + "\t" + Hex64(group) + poseStr + "\t(group fields unreadable)\t\t\t\n";
                continue;
            }
            gb += g + "\t" + Hex64(entry) + "\t" + Hex64(group) + poseStr + "\t" + nGeoms + "\t" + nMat + "\t" + Hex64(visuals) + "\t" + Hex64(materials) + "\n";
            if (nGeoms > 100000 || !PlausiblePtr(geoms)) continue;
            Dev::SafeReadUInt64(geoms);
            Dev::SafeReadUInt64(geoms + uint64(nGeoms) * 0x10 - 8);
            for (uint k = 0; k < nGeoms; k++) {
                nGeomsAll++;
                uint vi = Dev::ReadUInt32(geoms + uint64(k) * 0x10);
                uint mi = Dev::ReadUInt32(geoms + uint64(k) * 0x10 + 4);
                uint64 visual = 0, material = 0;
                string box = "";
                try {
                    if (PlausiblePtr(visuals)) visual = Dev::SafeReadUInt64(visuals + 8 * uint64(vi));
                    if (PlausiblePtr(materials) && mi < nMat) material = Dev::SafeReadUInt64(materials + 8 * uint64(mi));
                    if (PlausiblePtr(visual)) {
                        Dev::SafeReadUInt64(visual + 0x88 + 16);
                        for (uint c = 0; c < 6; c++) box += "\t" + Text::Format("%.9g", Dev::ReadFloat(visual + 0x88 + 4 * c));
                        nKept++;
                    } else {
                        box = "\t\t\t\t\t\t";
                    }
                } catch {
                    box = "\t(unreadable)\t\t\t\t\t";
                }
                kb += g + "\t" + k + "\t" + vi + "\t" + mi + "\t" + Hex64(visual) + "\t" + Hex64(material) + box + "\n";
            }
            if ((g % 64) == 63) yield();
        }
    }
    LmWriteFile(tag + "-groups.tsv", gb);
    LmWriteFile(tag + "-geoms.tsv", kb);
    info += "wrote " + tag + "-groups.tsv (" + nGroups + " groups) and -geoms.tsv (" + nGeomsAll + " geoms, " + nKept + " with a visual box)\n";
    return info;
}

string LmZone248Route(const string &in qs) {
    try {
        return LmZone248Dump("lmzone248");
    } catch {
        return "zone248 threw: " + getExceptionInfo() + "\n";
    }
}
