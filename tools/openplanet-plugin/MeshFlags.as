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
    uint vflags = 0;
    if (nvis > 0) {
        // +0xa8 is the visuals ARRAY pointer (not a nod): read the first entry
        uint64 arr = Dev::GetOffsetUint64(mesh, 0xa8);
        if (arr != 0) {
            uint64 v0 = Dev::ReadUInt64(arr);
            if (v0 != 0) { frames = Dev::ReadUInt32(v0 + 0x118); vflags = Dev::ReadUInt32(v0 + 0x24); }
        }
    }
    // the material lists OnNodLoaded consults before it marks a visual as a
    // tween (bit 27 of CPlugVisual+0x24): file materials (+0xb8/+0xc0), user
    // insts (+0xc8/+0xd0), the resolved runtime list (+0x1f8/+0x200)
    uint nmat = Dev::GetOffsetUint32(mesh, 0xc0);
    uint nuser = Dev::GetOffsetUint32(mesh, 0xd0);
    uint nres = Dev::GetOffsetUint32(mesh, 0x200);
    string s = item + " | " + kind + " | " + Reflection::TypeOf(mesh).Name + " | vct=" + vct + " | nvis=" + nvis
        + " | frames0=" + frames + " | vflags0=" + Text::Format("0x%x", vflags) + " | mats=" + nmat + "/" + nuser + "/" + nres + " | flags=" + Text::Format("0x%x", flags) + " | owner=0x" + Text::Format("%08x", uint(owner >> 32)) + Text::Format("%08x", uint(owner & 0xffffffff));
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

// ---- where in the Fid tree things live -------------------------------------
// /fids?name=SUBSTR -> per matching item: the item's own fid (full name) and
// its folder chain up to the root (depth counted), then for every entity mesh
// its owner fid (+0x338) and the fids of its plain `Materials` list (+0xc8/+0xd0,
// the list CPlugSolid2Model::OnNodLoaded consults for the tween bit). The
// answer to "what ancestor level and folder chain does a reference table in an
// EMBEDDED item need to name a game material the way the pack meshes do".

string FolderChain(CSystemFidsFolder@ f) {
    string s = "";
    int depth = 0;
    while (f !is null && depth < 16) {
        s += (depth == 0 ? "" : " <- ") + f.DirName;
        @f = f.ParentFolder;
        depth++;
    }
    return s + " [" + depth + " folders]";
}

string FidLine(const string &in label, CMwNod@ nod) {
    if (nod is null) return label + ": <null nod>\n";
    auto fid = GetFidFromNod(nod);
    if (fid is null) return label + ": no fid (" + Reflection::TypeOf(nod).Name + ")\n";
    return label + ": " + fid.FullFileName + " | " + FolderChain(fid.ParentFolder) + "\n";
}

string MeshFids(const string &in item, CMwNod@ em, int depth) {
    if (em is null || depth > 4) return "";
    string res = "";
    auto vl = cast<NPlugItem_SVariantList>(em);
    if (vl !is null) {
        for (uint i = 0; i < vl.Variants.Length; i++) res += MeshFids(item + "/v" + i, vl.Variants[i].EntityModel, depth + 1);
        return res;
    }
    auto pf = cast<CPlugPrefab>(em);
    if (pf !is null) {
        for (uint i = 0; i < pf.Ents.Length; i++) res += MeshFids(item + "/e" + i, pf.Ents[i].Model, depth + 1);
        return res;
    }
    CMwNod@ mesh = null;
    auto dyna = cast<CPlugDynaObjectModel>(em);
    if (dyna !is null) @mesh = dyna.Mesh;
    auto st = cast<CPlugStaticObjectModel>(em);
    if (st !is null) @mesh = st.Mesh;
    auto cm = cast<CGameCommonItemEntityModel>(em);
    if (cm !is null) return MeshFids(item + "/common", cm.StaticObject, depth + 1);
    if (mesh is null) return item + " | " + Reflection::TypeOf(em).Name + " | (no mesh)\n";
    res += FidLine(item + " mesh", mesh);
    uint64 owner = Dev::GetOffsetUint64(mesh, 0x338);
    if (owner != 0) res += FidLine(item + " mesh owner(+0x338)", Dev::ReadNod(owner));
    uint nmat = Dev::GetOffsetUint32(mesh, 0xd0);
    uint64 arr = Dev::GetOffsetUint64(mesh, 0xc8);
    for (uint k = 0; k < nmat && k < 8 && arr != 0; k++) {
        uint64 p = Dev::ReadUInt64(arr + 8 * k);
        if (p == 0) {
            res += item + " material[" + k + "]: <null>\n";
            continue;
        }
        CMwNod@ mn = Dev::ReadNod(p);
        res += FidLine(item + " material[" + k + "]", mn);
    }
    return res;
}

string ItemFids(const string &in qs) {
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return "not in an editor -- /editmap3 first";
    auto ch = ed.Challenge;
    if (ch is null) return "no Challenge on this editor";
    string needle = QArg(qs, "name");
    if (needle == "") return "fids needs name=SUBSTR";
    dictionary seen;
    string res = "";
    for (uint i = 0; i < ch.AnchoredObjects.Length; i++) {
        auto o = ch.AnchoredObjects[i];
        if (o is null || o.ItemModel is null) continue;
        string n = o.ItemModel.IdName;
        if (n.IndexOf(needle) < 0 || seen.Exists(n)) continue;
        seen.Set(n, 1);
        res += FidLine(n + " item", o.ItemModel);
        res += MeshFids(n, o.ItemModel.EntityModel, 0);
    }
    // the game's own folders, for the depth arithmetic
    auto game = Fids::GetGameFolder("");
    if (game !is null) res += "GameData root: " + game.FullDirName + " | " + FolderChain(game) + "\n";
    auto user = Fids::GetUserFolder("");
    if (user !is null) res += "UserData root: " + user.FullDirName + " | " + FolderChain(user) + "\n";
    if (res == "") res = "no item matches";
    return res;
}
