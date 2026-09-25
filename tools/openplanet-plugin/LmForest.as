// /lmforest — the zone's placed TREES as the game holds them (NHmsForestVis = zone+0x260), for the lightmapper's
// probe-box list (RE 7, 2026-09-25: RenderLighting_Frames adds GetAllTreesBBox — every placed tree's species box
// through its pose — after the item records). Layout (FUN_14026e590 / FUN_14026db50 / FUN_14026d2b0):
//   forest+0x188 → the tree array, stride 0x20 = { f32 quat[4] (stored order, printed raw), f32 pos[3], u32 pad }
//   forest+0x190   u32 tree count
//   forest+0x198 → u8 species per tree (0xff = none → the game skips it)
//   forest+0x20  → the species table, stride 0x3a8: +0x7c f32 centre[3], +0x88 f32 half[3]
//   forest+0x8     u32 species count;  forest+0x0 → CPlugVegetTreeModel* [i] (its +0x18 = the fid)
// Every read is SafeRead-probed once per array (the ends), then plain reads inside; the output is a TSV in
// PluginStorage (lmforest-<stamp>-trees.tsv / -species.tsv) and a short summary.
// Routes: /lmforest?info=1 (the pointers + counts) | /lmforest?dump=1 (the two TSVs) — no lock needed, read-only.

const uint64 FOREST_TREE_STRIDE = 0x20;
const uint64 FOREST_SPECIES_STRIDE = 0x3a8;

class LmForest {
    bool ok = false;
    string info;
    uint64 forest = 0;
    uint64 trees = 0;
    uint nTrees = 0;
    uint64 species = 0;
    uint64 speciesTable = 0;
    uint64 models = 0;
    uint nSpecies = 0;
}

LmForest LmForestFind() {
    LmForest f;
    auto app = GetApp();
    CHmsZone@ zone = null;
    string root = "";
    auto ed = cast<CGameCtnEditorCommon>(app.Editor);
    if (ed !is null && ed.Grid !is null && ed.Grid.Scene !is null && ed.Grid.Scene.Sector !is null) { @zone = ed.Grid.Scene.Sector.Zone; root = "editor.Grid.Scene"; }
    if (zone is null && app.GameScene !is null && app.GameScene.HackScene !is null && app.GameScene.HackScene.Sector !is null) { @zone = app.GameScene.HackScene.Sector.Zone; root = "GameScene.HackScene"; }
    if (zone is null) { f.info = "no zone (no editor grid, no GameScene)"; return f; }
    f.forest = Dev::GetOffsetUint64(zone, 0x260);
    f.info = "root=" + root + " forest(zone+0x260)=" + Hex64(f.forest) + "\n";
    // the neighbourhood, for the case the forest sits elsewhere or fills lazily: every plausible pointer word in
    // zone+0x200..0x2f8 with its would-be counts (+0x8, +0x190) and its vtable RVA
    for (uint off = 0x200; off < 0x300; off += 8) {
        uint64 q = Dev::GetOffsetUint64(zone, off);
        if (!PlausiblePtr(q)) continue;
        try {
            uint64 vt = LmVtRva(q);
            uint c8 = Dev::SafeReadUInt32(q + 0x8);
            uint c190 = Dev::SafeReadUInt32(q + 0x190);
            f.info += "  zone+" + Text::Format("%x", off) + " → " + Hex64(q) + " vt.rva=" + Hex64(vt) + " [+8]=" + c8 + " [+190]=" + c190 + "\n";
        } catch {
            f.info += "  zone+" + Text::Format("%x", off) + " → " + Hex64(q) + " (unreadable)\n";
        }
    }
    if (!PlausiblePtr(f.forest)) { f.info += "forest pointer not plausible\n"; return f; }
    try {
        Dev::SafeReadUInt64(f.forest + 0x198);
        f.trees = Dev::SafeReadUInt64(f.forest + 0x188);
        f.nTrees = Dev::SafeReadUInt32(f.forest + 0x190);
        f.species = Dev::SafeReadUInt64(f.forest + 0x198);
        f.speciesTable = Dev::SafeReadUInt64(f.forest + 0x20);
        f.models = Dev::SafeReadUInt64(f.forest + 0x0);
        f.nSpecies = Dev::SafeReadUInt32(f.forest + 0x8);
    } catch {
        f.info += "forest fields unreadable: " + getExceptionInfo() + "\n";
        return f;
    }
    f.info += "trees=" + Hex64(f.trees) + " count=" + f.nTrees + " species bytes=" + Hex64(f.species) + " speciesTable=" + Hex64(f.speciesTable) + " models=" + Hex64(f.models) + " nSpecies=" + f.nSpecies + "\n";
    if (f.nTrees > 2000000 || f.nSpecies > 4096) { f.info += "counts not plausible\n"; return f; }
    f.ok = true;
    return f;
}

string LmForestDump(LmForest &in f) {
    string tag = "lmforest-" + Time::Stamp;
    string summary = f.info;
    // the species table
    string sb = "i\tmodel\tfidName\tcx\tcy\tcz\thx\thy\thz\tcx2\tcy2\tcz2\thalfLen\n";
    if (PlausiblePtr(f.speciesTable) && f.nSpecies > 0) {
        Dev::SafeReadUInt64(f.speciesTable);
        Dev::SafeReadUInt64(f.speciesTable + uint64(f.nSpecies) * FOREST_SPECIES_STRIDE - 8);
        for (uint i = 0; i < f.nSpecies; i++) {
            uint64 r = f.speciesTable + uint64(i) * FOREST_SPECIES_STRIDE;
            uint64 model = 0;
            string fidName = "";
            // the model pointer only (a raw pointer has no nod handle here; the fid name is RE 7's to map by index)
            if (PlausiblePtr(f.models)) {
                try { model = Dev::SafeReadUInt64(f.models + 8 * uint64(i)); } catch { fidName = "(unreadable)"; }
            }
            sb += i + "\t" + Hex64(model) + "\t" + fidName;
            for (uint k = 0; k < 6; k++) sb += "\t" + Text::Format("%.9g", Dev::ReadFloat(r + 0x7c + 4 * k));
            for (uint k = 0; k < 4; k++) sb += "\t" + Text::Format("%.9g", Dev::ReadFloat(r + 0x94 + 4 * k));
            sb += "\n";
        }
    }
    LmWriteFile(tag + "-species.tsv", sb);
    // the trees, in slices with a yield between (the script runtime's per-slice budget)
    string name = tag + "-trees.tsv";
    LmWriteFile(name, "i\tspecies\tq0\tq1\tq2\tq3\tpx\tpy\tpz\tpad\n");
    uint written = 0, skipped = 0;
    if (PlausiblePtr(f.trees) && f.nTrees > 0) {
        Dev::SafeReadUInt64(f.trees);
        Dev::SafeReadUInt64(f.trees + uint64(f.nTrees) * FOREST_TREE_STRIDE - 8);
        bool haveSpecies = PlausiblePtr(f.species);
        if (haveSpecies) { Dev::SafeReadUInt8(f.species); Dev::SafeReadUInt8(f.species + uint64(f.nTrees) - 1); }
        string tb = "";
        for (uint i = 0; i < f.nTrees; i++) {
            uint64 r = f.trees + uint64(i) * FOREST_TREE_STRIDE;
            uint sp = haveSpecies ? Dev::ReadUInt8(f.species + uint64(i)) : 255;
            if (sp == 255) skipped++;
            tb += i + "\t" + sp;
            for (uint k = 0; k < 7; k++) tb += "\t" + Text::Format("%.9g", Dev::ReadFloat(r + 4 * k));
            tb += "\t" + Text::Format("%08x", Dev::ReadUInt32(r + 0x1c)) + "\n";
            written++;
            if ((i % 400) == 399) { LmAppend(name, tb); tb = ""; yield(); }
        }
        if (tb.Length > 0) LmAppend(name, tb);
    }
    summary += "wrote " + tag + "-species.tsv (" + f.nSpecies + " species) and " + name + " (" + written + " trees, " + skipped + " without a species)\n";
    return summary;
}

string LmForestRoute(const string &in qs) {
    LmForest f = LmForestFind();
    if (QArg(qs, "info") == "1" || !f.ok) return f.info;
    if (QArg(qs, "dump") == "1") {
        try {
            return LmForestDump(f);
        } catch {
            return f.info + "dump threw: " + getExceptionInfo() + "\n";
        }
    }
    return f.info + "(?info=1 | ?dump=1)\n";
}
