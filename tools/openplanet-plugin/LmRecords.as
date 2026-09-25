// LmRecords.as -- THE LIGHTMAPPER'S RECORD ARRAY, read off the live CHmsLightMap
// during an editor bake (RE 7's request, 2026-09-25: the editor packs ~10 % more
// layout area than the file's mapping holds on tiny 16 -- records present at
// UpdateMapping, gone at cache creation?). Read-only; every hop SafeRead-probed;
// no scans; nothing dereferenced unverified.
//
//   /lmrecords?info=1        -> the pointer chain, counts, s, Σ (debug the walk)
//   /lmrecords[?src=H|L]     -> TSV of the records (H = the holder's array, L = the
//                               compute object's COPY taken at UpdateMapping)
//   /lmrecords?arm=1         -> arm the WATCHER: a coroutine polls the two data
//                               conditions every frame and writes the dumps to the
//                               plugin storage (lmrecords-A-*.tsv after the
//                               allocation, lmrecords-B-*.tsv after the cache write);
//                               the arm flag persists (lmrecords-arm.txt) so a game
//                               started later arms itself at plugin load
//   /lmrecords?disarm=1      -> disarm + remove the flag
//   /lmrecords?status=1      -> the watcher's state
//
// POINTER CHAIN [RE 7, DISASSEMBLY]: CHmsZone (the map scene's Sector.Zone, as
// /treeinst) -> zone+0x438 = the zone's lightmap OWNER (vtable RVA 0x1b68940)
// -> owner+0x110 = the CHmsLightMap (class 0x06021000, vtable RVA 0x1b683e0)
// -> +0x18 = H, the record HOLDER (NHmsLightMap::SPImp, 0x4f8 B, no vtable);
// +0x20 = L, the compute object (0x970 B; L+0x218 == H, L+0x18 == zone).
// H+0xa8/+0xb0 = {records ptr, u32 count}, stride 0x58; L+0xd8/+0xe0 = the COPY
// taken at UpdateMapping. Record: +0 model ptr, +8 mobil ptr (kind 0) / u32 pool
// index (kind 2), +0x10 tree ptr (kind 0) / u64 bind key (kinds 1/2), +0x18
// lm-data ptr (may be NULL), +0x20 PreLightGen ptr (+0 f32 MeterByUv, +4..+0x10
// uv0 u0 v0 u1 v1, +0x14..+0x20 uv1, +0x24/+0x28 i32 sprite w h, +0x40 uvGroups
// ptr, +0x48 u32 count, +0x50 u8 u01), +0x28 chart record ptr (+0 u32 pos
// x|y<<16, +4 u32 size w|h<<16, +8..+0x14 f32 ST ×4, +0x18 i32 group), +0x30
// fid ptr (not dereferenced), +0x38..+0x40 f32 centre, +0x44..+0x4c f32 half,
// +0x50 f32 quality, +0x54 u32 flags (bits 0-1 kind: 0 mobil, 1 instanced/dyna,
// 2 static; 4 material class; 8 no PLG; 0x10 uv-set-1 ok; 0x20 has PLG).
// MOMENTS: A = *(f32*)(H+0x488) (s) non-zero AND *(CHmsLightMap+0x10) non-null
// (the alloc result: +0xc f32 Σarea, +0x14 f32 s); B = *(H+8) becomes a NEW
// CHmsLightMapCache (class 0x06022000, vtable RVA 0x1b6aea8) with a non-null
// +0x70 pointer (-> [0] f32 Σ, the file's 0x0602200B); cache+0x88 = the
// mapping: +0x98 z0 count, +0xa8 keys count, +0xb8 pos count, +0xc8 size count.

const uint64 LM_VT_OWNER = 0x1b68940;
const uint64 LM_VT_LIGHTMAP = 0x1b683e0;
const uint64 LM_VT_CACHE = 0x1b6aea8;
const uint64 LM_REC_STRIDE = 0x58;
string g_lmStep = "";
bool g_lmForestDumped = false;

class LmChain {
    bool ok = false;
    string info;
    uint64 zoneVt = 0;
    uint64 owner = 0;
    uint64 lm = 0;
    uint64 H = 0;
    uint64 L = 0;
}

// the vtable RVA of the object at `p` (0 when unreadable or implausible); SafeRead throws on an unmapped page
uint64 LmVtRva(uint64 p) {
    if (!PlausiblePtr(p)) return 0;
    uint64 vt = Dev::SafeReadUInt64(p);
    uint64 base = Dev::BaseAddress();
    if (vt < base) return 0;
    return vt - base;
}

LmChain LmFind() {
    LmChain c;
    auto app = GetApp();
    CScene@ scene = null;
    string root = "";
    auto ed = cast<CGameCtnEditorCommon>(app.Editor);
    if (ed !is null && ed.Grid !is null && ed.Grid.Scene !is null) { @scene = ed.Grid.Scene; root = "editor.Grid.Scene"; }
    if (scene is null && app.GameScene !is null) { @scene = app.GameScene.HackScene; root = "GameScene.HackScene"; }
    if (scene is null) { c.info = "no scene (no editor grid, no GameScene)"; return c; }
    if (scene.Sector is null || scene.Sector.Zone is null) { c.info = "root=" + root + " no Sector.Zone"; return c; }
    CHmsZone@ zone = scene.Sector.Zone;
    c.zoneVt = Dev::GetOffsetUint64(zone, 0);
    c.info = "root=" + root + " zoneVt=" + Hex64(c.zoneVt) + "\n";
    // zone+0x438: the zone's lightmap owner (read INSIDE the zone object)
    c.owner = Dev::GetOffsetUint64(zone, 0x438);
    uint64 rva = LmVtRva(c.owner);
    c.info += "owner=" + Hex64(c.owner) + " vt.rva=" + Hex64(rva) + (rva == LM_VT_OWNER ? " OK" : " MISMATCH (expected " + Hex64(LM_VT_OWNER) + ")") + "\n";
    if (rva != LM_VT_OWNER) return c;
    c.lm = Dev::SafeReadUInt64(c.owner + 0x110);
    rva = LmVtRva(c.lm);
    c.info += "lightmap=" + Hex64(c.lm) + " vt.rva=" + Hex64(rva) + (rva == LM_VT_LIGHTMAP ? " OK" : " MISMATCH (expected " + Hex64(LM_VT_LIGHTMAP) + ")") + "\n";
    if (rva != LM_VT_LIGHTMAP) return c;
    c.H = Dev::SafeReadUInt64(c.lm + 0x18);
    c.L = Dev::SafeReadUInt64(c.lm + 0x20);
    c.info += "H=" + Hex64(c.H) + " L=" + Hex64(c.L) + "\n";
    if (!PlausiblePtr(c.H)) { c.info += "H not plausible\n"; return c; }
    // self-checks (each read probed): L+0x218 == H, L+0x18 == the zone (compared by vtable), H+0xe0 == L
    Dev::SafeReadUInt64(c.H + 0x4f0);
    if (PlausiblePtr(c.L)) {
        Dev::SafeReadUInt64(c.L + 0x968);
        uint64 l218 = Dev::SafeReadUInt64(c.L + 0x218);
        uint64 l18 = Dev::SafeReadUInt64(c.L + 0x18);
        uint64 he0 = Dev::SafeReadUInt64(c.H + 0xe0);
        uint64 zvt = PlausiblePtr(l18) ? Dev::SafeReadUInt64(l18) : 0;
        c.info += "checks: L+0x218==H " + (l218 == c.H ? "OK" : "NO(" + Hex64(l218) + ")") + "; L+0x18 vt==zone vt " + (zvt == c.zoneVt ? "OK" : "NO(" + Hex64(zvt) + ")") + "; H+0xe0==L " + (he0 == c.L ? "OK" : "NO(" + Hex64(he0) + ")") + "\n";
    } else {
        c.info += "L not plausible (no compute object yet)\n";
    }
    c.ok = true;
    return c;
}

string LmState(LmChain@ c) {
    string s = "";
    g_lmStep = "state:H";
    uint W = Dev::SafeReadUInt32(c.H + 0x478);
    uint Hh = Dev::SafeReadUInt32(c.H + 0x47c);
    float sc = Dev::SafeReadFloat(c.H + 0x488);
    uint nH = Dev::SafeReadUInt32(c.H + 0xb0);
    uint64 aH = Dev::SafeReadUInt64(c.H + 0xa8);
    uint noPlg = Dev::SafeReadUInt32(c.H + 0x84);
    uint hasPlg = Dev::SafeReadUInt32(c.H + 0x88);
    s += "H: W,H=" + W + "," + Hh + " s=" + Text::Format("%.9g", sc) + " records=" + Hex64(aH) + " count=" + nH + " noPLG=" + noPlg + " hasPLG=" + hasPlg + "\n";
    g_lmStep = "state:alloc";
    uint64 alloc = Dev::SafeReadUInt64(c.lm + 0x10);
    if (PlausiblePtr(alloc)) {
        s += "alloc(lm+0x10)=" + Hex64(alloc) + " Σarea=" + Text::Format("%.9g", Dev::SafeReadFloat(alloc + 0xc)) + " s=" + Text::Format("%.9g", Dev::SafeReadFloat(alloc + 0x14)) + "\n";
    } else {
        s += "alloc(lm+0x10)=" + Hex64(alloc) + "\n";
    }
    if (PlausiblePtr(c.L)) {
        g_lmStep = "state:L";
        uint64 aL = Dev::SafeReadUInt64(c.L + 0xd8);
        uint nL = Dev::SafeReadUInt32(c.L + 0xe0);
        s += "L: copy=" + Hex64(aL) + " count=" + nL + "\n";
    }
    g_lmStep = "state:cache";
    uint64 cache = Dev::SafeReadUInt64(c.H + 8);
    s += "cache(H+8)=" + Hex64(cache);
    if (PlausiblePtr(cache)) {
        uint64 rva = LmVtRva(cache);
        s += " vt.rva=" + Hex64(rva) + (rva == LM_VT_CACHE ? " OK" : " (not the cache kind)");
        g_lmStep = "state:cache+0x70";
        uint64 p70 = Dev::SafeReadUInt64(cache + 0x70);
        if (PlausiblePtr(p70)) s += " Σ(+0x70->[0])=" + Text::Format("%.9g", Dev::SafeReadFloat(p70));
        g_lmStep = "state:cache+0x88";
        // the mapping's counts (RE 7's offsets) — guarded: a wrong offset must not abort the dump
        try {
            uint64 mp = Dev::SafeReadUInt64(cache + 0x88);
            if (PlausiblePtr(mp)) {
                g_lmStep = "state:mapping counts";
                s += " mapping=" + Hex64(mp) + " z0=" + Dev::SafeReadUInt32(mp + 0x98) + " keys=" + Dev::SafeReadUInt32(mp + 0xa8) + " pos=" + Dev::SafeReadUInt32(mp + 0xb8) + " size=" + Dev::SafeReadUInt32(mp + 0xc8);
            } else {
                s += " mapping(+0x88)=" + Hex64(mp);
            }
        } catch {
            s += " mapping: unreadable (" + g_lmStep + ")";
        }
        // the cache's first qwords, for RE 7 to place the mapping pointer
        try {
            g_lmStep = "state:cache qwords";
            s += " cache.q=[";
            for (uint k = 0; k < 0x100; k += 8) s += Hex64(Dev::SafeReadUInt64(cache + k)) + (k + 8 < 0x100 ? " " : "");
            s += "]";
        } catch {
            s += " cache qwords unreadable";
        }
    }
    s += "\n";
    return s;
}

// one TSV of `cnt` records at `arr`, APPENDED to the storage file `name` in slices of 300 records with a yield
// between slices (the script runtime aborts any single slice over ~1 s; 12 214 records took longer)
void LmDumpTo(const string &in name, uint64 arr, uint cnt) {
    if (!PlausiblePtr(arr) || cnt == 0 || cnt > 400000) { LmWriteFile(name, "# records array not plausible (" + Hex64(arr) + ", " + cnt + ")\n"); return; }
    g_lmStep = "dump:array ends " + Hex64(arr) + " n=" + cnt;
    Dev::SafeReadUInt64(arr);
    Dev::SafeReadUInt64(arr + uint64(cnt) * LM_REC_STRIDE - 8);
    LmWriteFile(name, "i\tkind\tflags\tquality\tmodel\tp8\tkey\tlmdata\tplg\tmeterByUv\tu0\tv0\tu1\tv1\tu0b\tv0b\tu1b\tv1b\tspriteW\tspriteH\tuvGroups\tu01\tchart\tcx\tcy\tcw\tch\tst0\tst1\tst2\tst3\tgroup\tcenterX\tcenterY\tcenterZ\thalfX\thalfY\thalfZ\n");
    string sb = "";
    for (uint i = 0; i < cnt; i++) {
        uint64 r = arr + uint64(i) * LM_REC_STRIDE;
        g_lmStep = "dump:record " + i;
        uint64 model = Dev::ReadUInt64(r + 0x0);
        uint64 p8 = Dev::ReadUInt64(r + 0x8);
        uint64 key = Dev::ReadUInt64(r + 0x10);
        uint64 lmdata = Dev::ReadUInt64(r + 0x18);
        uint64 plg = Dev::ReadUInt64(r + 0x20);
        uint64 chart = Dev::ReadUInt64(r + 0x28);
        float cx = Dev::ReadFloat(r + 0x38), cy = Dev::ReadFloat(r + 0x3c), cz = Dev::ReadFloat(r + 0x40);
        float hx = Dev::ReadFloat(r + 0x44), hy = Dev::ReadFloat(r + 0x48), hz = Dev::ReadFloat(r + 0x4c);
        float q = Dev::ReadFloat(r + 0x50);
        uint flags = Dev::ReadUInt32(r + 0x54);
        sb += i + "\t" + (flags & 3) + "\t" + Text::Format("%08x", flags) + "\t" + Text::Format("%.9g", q) + "\t" + Hex64(model) + "\t" + Hex64(p8) + "\t" + Hex64(key) + "\t" + Hex64(lmdata) + "\t" + Hex64(plg);
        if (PlausiblePtr(plg)) {
            g_lmStep = "dump:record " + i + " plg " + Hex64(plg);
            Dev::SafeReadUInt64(plg + 0x50);
            sb += "\t" + Text::Format("%.9g", Dev::ReadFloat(plg + 0x0));
            for (uint k = 0; k < 8; k++) sb += "\t" + Text::Format("%.9g", Dev::ReadFloat(plg + 4 + 4 * k));
            sb += "\t" + Dev::ReadInt32(plg + 0x24) + "\t" + Dev::ReadInt32(plg + 0x28) + "\t" + Dev::ReadUInt32(plg + 0x48) + "\t" + Dev::ReadUInt8(plg + 0x50);
        } else {
            for (uint k = 0; k < 13; k++) sb += "\t";
        }
        sb += "\t" + Hex64(chart);
        if (PlausiblePtr(chart)) {
            g_lmStep = "dump:record " + i + " chart " + Hex64(chart);
            Dev::SafeReadUInt64(chart + 0x18);
            uint pos = Dev::ReadUInt32(chart + 0);
            uint size = Dev::ReadUInt32(chart + 4);
            sb += "\t" + (pos & 0xffff) + "\t" + (pos >> 16) + "\t" + (size & 0xffff) + "\t" + (size >> 16);
            for (uint k = 0; k < 4; k++) sb += "\t" + Text::Format("%.9g", Dev::ReadFloat(chart + 8 + 4 * k));
            sb += "\t" + Dev::ReadInt32(chart + 0x18);
        } else {
            for (uint k = 0; k < 9; k++) sb += "\t";
        }
        sb += "\t" + Text::Format("%.9g", cx) + "\t" + Text::Format("%.9g", cy) + "\t" + Text::Format("%.9g", cz) + "\t" + Text::Format("%.9g", hx) + "\t" + Text::Format("%.9g", hy) + "\t" + Text::Format("%.9g", hz) + "\n";
        if ((i % 300) == 299) {
            LmAppend(name, sb);
            sb = "";
            yield();
        }
    }
    if (sb.Length > 0) LmAppend(name, sb);
}

// the request-time form (no yields inside a request): the first `cnt` records as one string
string LmDump(uint64 arr, uint cnt) {
    if (cnt > 300) cnt = 300;
    string tmp = "lmrecords-req.tsv";
    LmDumpTo(tmp, arr, cnt);
    IO::File f(IO::FromStorageFolder(tmp), IO::FileMode::Read);
    string txt = f.ReadToEnd();
    f.Close();
    return txt;
}

void LmWriteFile(const string &in name, const string &in text) {
    IO::File f(IO::FromStorageFolder(name), IO::FileMode::Write);
    f.Write(text);
    f.Close();
}

// the watcher
bool g_lmArmed = false;
bool g_lmRunning = false;
string g_lmStatus = "idle";
int g_lmStage = 0; // 0 waiting for the chain, 1 waiting for A, 2 waiting for B, 3 done

string g_lmRunTag = "";

string LmDumpBoth(LmChain@ c, const string &in tag) {
    g_lmStep = "dumpboth:state";
    string st = c.info + LmState(c);
    uint64 aH = Dev::SafeReadUInt64(c.H + 0xa8);
    uint nH = Dev::SafeReadUInt32(c.H + 0xb0);
    string pre = "lmrecords-" + g_lmRunTag + tag;
    LmWriteFile(pre + "-info.txt", st);
    LmDumpTo(pre + "-H.tsv", aH, nH);
    if (PlausiblePtr(c.L)) {
        uint64 aL = Dev::SafeReadUInt64(c.L + 0xd8);
        uint nL = Dev::SafeReadUInt32(c.L + 0xe0);
        if (aL != aH) LmDumpTo(pre + "-L.tsv", aL, nL);
        else LmWriteFile(pre + "-L.txt", "L's array == H's array (" + Hex64(aH) + ", " + nL + " records)\n");
    }
    return st;
}

void LmAppend(const string &in name, const string &in text) {
    IO::File f(IO::FromStorageFolder(name), IO::FileMode::Append);
    f.Write(text);
    f.Close();
}

// the watcher: every other frame it walks the chain; while the chain exists it appends the state to
// lmrecords-trace.txt every ~2 s and dumps the arrays whenever the record counts / the cache pointer /
// the alloc pointer / s change (tags c0, c1, ...), plus the named moments A and B
void LmWatch() {
    g_lmRunning = true;
    g_lmStage = 0;
    uint64 cache0 = 0;
    bool haveCache0 = false;
    uint frames = 0;
    uint64 lastTrace = 0;
    uint lastNH = 0xffffffff, lastNL = 0xffffffff;
    uint64 lastCache = 0, lastAlloc = 0;
    bool lastSNonZero = false;
    float s0 = 0;
    uint nH0 = 0xffffffff;
    uint changeDumps = 0;
    while (g_lmArmed) {
        yield();
        frames++;
        if ((frames % 2) != 0) continue;
        LmChain@ c = null;
        try {
            @c = LmFind();
        } catch {
            g_lmStatus = "chain threw: " + getExceptionInfo();
            continue;
        }
        if (!c.ok) { g_lmStatus = "stage " + g_lmStage + ": no chain (" + c.info.Replace("\n", " | ") + ")"; continue; }
        try {
            uint nH = Dev::SafeReadUInt32(c.H + 0xb0);
            uint nL = PlausiblePtr(c.L) ? Dev::SafeReadUInt32(c.L + 0xe0) : 0;
            uint64 cache = Dev::SafeReadUInt64(c.H + 8);
            uint64 alloc = Dev::SafeReadUInt64(c.lm + 0x10);
            float sc = Dev::SafeReadFloat(c.H + 0x488);
            if (g_lmStage == 0) {
                cache0 = cache;
                haveCache0 = true;
                s0 = sc;
                nH0 = nH;
                g_lmStage = 1;
                g_lmRunTag = "r" + Time::Stamp + "-";
                g_lmStatus = "armed: chain found, cache0=" + Hex64(cache0) + " run " + g_lmRunTag;
                LmAppend("lmrecords-trace.txt", "== chain found " + Time::Stamp + " run " + g_lmRunTag + "\n" + c.info);
                // the LOAD-TIME dump (tag L): the map's record array as the editor built it on load — for a map that
                // already carries a lightmap the compute keeps the same set and the same s, so nothing else may trigger
                // before the editor leaves (hill4's 9-s q3 bake gave no dump at all, 2026-09-25)
                try {
                    string st0 = LmDumpBoth(c, "L");
                    LmAppend("lmrecords-trace.txt", "== load-time dump L\n" + st0);
                } catch {
                    LmAppend("lmrecords-trace.txt", "load-time dump threw at " + g_lmStep + ": " + getExceptionInfo() + "\n");
                }
            }
            // the zone's forest (NHmsForestVis at zone+0x260): its tree array fills during the compute — dump it once
            // when the count turns non-zero (RE 7's probe-box source for the zone trees)
            if (!g_lmForestDumped) {
                try {
                    LmForest fo = LmForestFind();
                    if (fo.ok && fo.nTrees > 0) {
                        string sf = LmForestDump(fo);
                        g_lmForestDumped = true;
                        LmAppend("lmrecords-trace.txt", "== forest dump at " + Time::Stamp + "\n" + sf);
                    }
                } catch {
                    LmAppend("lmrecords-trace.txt", "forest probe threw: " + getExceptionInfo() + "\n");
                }
            }
            uint64 now = Time::Stamp;
            if (now - lastTrace >= 2) {
                lastTrace = now;
                uint nForest = 0;
                try { LmForest fo2 = LmForestFind(); if (fo2.ok) nForest = fo2.nTrees; } catch {}
                LmAppend("lmrecords-trace.txt", "t=" + now + " stage=" + g_lmStage + " nH=" + nH + " nL=" + nL + " s=" + Text::Format("%.9g", sc) + " alloc=" + Hex64(alloc) + " cache=" + Hex64(cache) + " forestTrees=" + nForest + "\n");
            }
            bool changed = (nH != lastNH) || (nL != lastNL) || (cache != lastCache) || (PlausiblePtr(alloc) != PlausiblePtr(lastAlloc)) || ((sc != 0) != lastSNonZero);
            if (changed && lastNH != 0xffffffff && changeDumps < 12) {
                string tag = "c" + changeDumps;
                string st = LmDumpBoth(c, tag);
                LmAppend("lmrecords-trace.txt", "-- change dump " + tag + " at " + now + ": nH " + lastNH + "->" + nH + " nL " + lastNL + "->" + nL + " cache " + Hex64(lastCache) + "->" + Hex64(cache) + " alloc " + Hex64(lastAlloc) + "->" + Hex64(alloc) + " s=" + Text::Format("%.9g", sc) + "\n" + st);
                changeDumps++;
            }
            lastNH = nH; lastNL = nL; lastCache = cache; lastAlloc = alloc; lastSNonZero = (sc != 0);
            if (g_lmStage == 1) {
                // moment A: the allocation happened — s changed from the loaded map's value (lm+0x10 is a small
                // integer on this build, not the alloc pointer RE 7 expected) or the H count changed
                if ((sc != 0 && sc != s0) || (nH != nH0 && nH0 != 0xffffffff)) {
                    string st = LmDumpBoth(c, "A");
                    g_lmStage = 2;
                    g_lmStatus = "A dumped (s=" + Text::Format("%.9g", sc) + "): " + st.Replace("\n", " | ");
                    LmAppend("lmrecords-trace.txt", "== moment A at " + now + "\n" + st);
                }
            } else if (g_lmStage == 2) {
                if (haveCache0 && cache != cache0 && PlausiblePtr(cache)) {
                    uint64 rva = LmVtRva(cache);
                    uint64 p70 = Dev::SafeReadUInt64(cache + 0x70);
                    if (rva == LM_VT_CACHE && PlausiblePtr(p70)) {
                        string st = LmDumpBoth(c, "B");
                        g_lmStage = 3;
                        g_lmStatus = "B dumped: " + st.Replace("\n", " | ");
                        LmAppend("lmrecords-trace.txt", "== moment B at " + now + "\n" + st);
                    }
                }
            }
        } catch {
            g_lmStatus = "stage " + g_lmStage + " threw at " + g_lmStep + ": " + getExceptionInfo();
            LmAppend("lmrecords-trace.txt", "threw at stage " + g_lmStage + " step " + g_lmStep + ": " + getExceptionInfo() + "\n");
            // after a throw inside a change dump, count it so the loop does not re-throw every frame
            changeDumps++;
            lastNH = 0xfffffffe;
        }
    }
    g_lmRunning = false;
}

void LmArmFromStorage() {
    if (IO::FileExists(IO::FromStorageFolder("lmrecords-arm.txt")) && !g_lmRunning) {
        g_lmArmed = true;
        startnew(LmWatch);
    }
}

string LmRecords(const string &in qs) {
    if (QArg(qs, "arm") == "1") {
        LmWriteFile("lmrecords-arm.txt", "armed " + Time::Stamp);
        // re-arm: a running watcher restarts its stage machine (stage 0 → the chain and the baselines anew)
        g_lmArmed = true;
        g_lmStage = 0;
        g_lmForestDumped = false;
        if (!g_lmRunning) startnew(LmWatch);
        return "armed (stage " + g_lmStage + ", running " + g_lmRunning + ")";
    }
    if (QArg(qs, "disarm") == "1") {
        g_lmArmed = false;
        if (IO::FileExists(IO::FromStorageFolder("lmrecords-arm.txt"))) IO::Delete(IO::FromStorageFolder("lmrecords-arm.txt"));
        return "disarmed";
    }
    if (QArg(qs, "status") == "1") return "armed=" + g_lmArmed + " running=" + g_lmRunning + " stage=" + g_lmStage + "\n" + g_lmStatus + "\n";
    if (QArg(qs, "dump") != "") {
        LmChain@ cd = LmFind();
        if (!cd.ok) return cd.info;
        try {
            return LmDumpBoth(cd, QArg(qs, "dump"));
        } catch {
            return "dump threw at " + g_lmStep + ": " + getExceptionInfo();
        }
    }
    LmChain@ c = LmFind();
    if (!c.ok) return c.info;
    string info = c.info + LmState(c);
    if (QArg(qs, "info") == "1") return info;
    string src = QArg(qs, "src");
    if (src == "L") {
        if (!PlausiblePtr(c.L)) return info + "no compute object";
        return info + LmDump(Dev::SafeReadUInt64(c.L + 0xd8), Dev::SafeReadUInt32(c.L + 0xe0));
    }
    return info + LmDump(Dev::SafeReadUInt64(c.H + 0xa8), Dev::SafeReadUInt32(c.H + 0xb0));
}
