// MeshFlags.as -- the runtime word that decides whether a dyna item's mesh
// gets a vertex-tween animation: CPlugSolid2Model+0x1f0 bit 0 ("some visual has
// >= 2 sub-visual frames"), computed by CPlugSolid2Model::OnNodLoaded and read
// by the prefab spawner (Trackmania.exe 0x140b73ae1) before it hands the
// InstDyna2 instance its period/phase handle. The 2026-09-08 flag threads saw
// our embedded tween cloths never animate on their own while the pack flags
// do; this route asks the loaded game which meshes carry the bit.
//
//   /meshflags[?name=SUBSTR]          -> one line per (item model, entity) mesh:
//        item | kind | mesh class | vct=VisCstType | nvis=visuals | flags=+0x1f0 | owner=+0x338
//   /meshflags?set=1[&name=SUBSTR]    -> also OR bit 0 into +0x1f0 of every tween mesh
//        (a mesh whose first visual has >= 2 sub-visuals) -- the in-game proof
//        that the bit alone is what the spawner wants; instances already spawned
//        keep their handle, so respawn afterwards
//   /respawn?name=SUBSTR[&dx=8][&dz=0] -> (stub: no placement API bound yet) a NEW copy of the first anchored
//        object whose item name contains SUBSTR, dx/dz metres away (the editor's
//        PlaceAnchoredObject: a fresh spawn reads the mesh flags again)
//
// Read-mostly; `set` and `respawn` change the live scene only (nothing is saved).

string MeshLine(const string &in item, const string &in kind, CMwNod@ mesh, bool set) {
    if (mesh is null) return item + " | " + kind + " | <null mesh>\n";
    uint flags = Dev::GetOffsetUint32(mesh, 0x1f0);
    uint nvis = Dev::GetOffsetUint32(mesh, 0xb0);
    uint vct = Dev::GetOffsetUint32(mesh, 0x38);
    uint64 owner = Dev::GetOffsetUint64(mesh, 0x338);
    // the first visual's sub-visual (frame) count: CPlugVisual+0x118
    uint frames = 0;
    if (nvis > 0) {
        // +0xa8 is the visuals ARRAY pointer (not a nod): read the first entry
        uint64 arr = Dev::GetOffsetUint64(mesh, 0xa8);
        if (arr != 0) {
            uint64 v0 = Dev::ReadUInt64(arr);
            if (v0 != 0) frames = Dev::ReadUInt32(v0 + 0x118);
        }
    }
    string s = item + " | " + kind + " | " + Reflection::TypeOf(mesh).Name + " | vct=" + vct + " | nvis=" + nvis
        + " | frames0=" + frames + " | flags=" + Text::Format("0x%x", flags) + " | owner=0x" + Text::Format("%08x", uint(owner >> 32)) + Text::Format("%08x", uint(owner & 0xffffffff));
    if (set && frames >= 2 && (flags & 1) == 0) {
        Dev::SetOffset(mesh, 0x1f0, uint(flags | 1));
        s += " | SET bit0 -> " + Text::Format("0x%x", Dev::GetOffsetUint32(mesh, 0x1f0));
    }
    return s + "\n";
}

string WalkEntityModel(const string &in item, CMwNod@ em, int depth, bool set) {
    if (em is null) return item + " | <null entity model>\n";
    if (depth > 4) return "";
    string res = "";
    auto vl = cast<NPlugItem_SVariantList>(em);
    if (vl !is null) {
        for (uint i = 0; i < vl.Variants.Length; i++) res += WalkEntityModel(item + "/v" + i, vl.Variants[i].EntityModel, depth + 1, set);
        return res;
    }
    auto pf = cast<CPlugPrefab>(em);
    if (pf !is null) {
        for (uint i = 0; i < pf.Ents.Length; i++) res += WalkEntityModel(item + "/e" + i, pf.Ents[i].Model, depth + 1, set);
        return res;
    }
    auto dyna = cast<CPlugDynaObjectModel>(em);
    if (dyna !is null) return MeshLine(item, "dyna", dyna.Mesh, set);
    auto st = cast<CPlugStaticObjectModel>(em);
    if (st !is null) return MeshLine(item, "static", st.Mesh, set);
    auto cm = cast<CGameCommonItemEntityModel>(em);
    if (cm !is null) return WalkEntityModel(item + "/common", cm.StaticObject, depth + 1, set);
    return item + " | " + Reflection::TypeOf(em).Name + " | (not walked)\n";
}

string MeshFlags(const string &in qs) {
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return "not in an editor -- /editmap3 first";
    auto ch = ed.Challenge;
    if (ch is null) return "no Challenge on this editor";
    string needle = QArg(qs, "name");
    bool set = QArg(qs, "set") == "1";
    dictionary seen;
    string res = "";
    for (uint i = 0; i < ch.AnchoredObjects.Length; i++) {
        auto o = ch.AnchoredObjects[i];
        if (o is null || o.ItemModel is null) continue;
        string n = o.ItemModel.IdName;
        if (needle != "" && n.IndexOf(needle) < 0) continue;
        if (seen.Exists(n)) continue;
        seen.Set(n, 1);
        res += WalkEntityModel(n, o.ItemModel.EntityModel, 0, set);
    }
    if (res == "") res = "no item matches";
    return res;
}

string RespawnItem(const string &in qs) {
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return "not in an editor -- /editmap3 first";
    auto pmt = cast<CGameEditorPluginMap>(ed.PluginMapType);
    auto ch = ed.Challenge;
    if (pmt is null || ch is null) return "no PluginMapType/Challenge";
    string needle = QArg(qs, "name");
    if (needle == "") return "respawn needs name=SUBSTR";
    string sdx = QArg(qs, "dx"), sdz = QArg(qs, "dz");
    float dx = sdx == "" ? 8.0f : Text::ParseFloat(sdx);
    float dz = sdz == "" ? 0.0f : Text::ParseFloat(sdz);
    for (uint i = 0; i < ch.AnchoredObjects.Length; i++) {
        auto o = ch.AnchoredObjects[i];
        if (o is null || o.ItemModel is null) continue;
        if (o.ItemModel.IdName.IndexOf(needle) < 0) continue;
        vec3 p = o.AbsolutePositionInMap + vec3(dx, 0.0f, dz);
        // 2026-09-09: CGameEditorPluginMap::PlaceAnchoredObject is NOT bound in this
        // Openplanet build ("No matching symbol" at compile time, one game restart
        // learnt it). Until a bound placement call is found this route only
        // says what it would do.
        return "would place " + o.ItemModel.IdName + " at " + p.ToString() + " (copy of #" + i + ") -- no bound placement API (PlaceAnchoredObject missing); items " + ch.AnchoredObjects.Length;
    }
    return "no anchored object matches " + needle;
}
