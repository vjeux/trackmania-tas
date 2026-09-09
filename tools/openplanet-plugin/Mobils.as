// Mobils.as -- the scene's mobils: what the engine actually instantiated for
// the loaded map, with the per-mobil visibility bit. The map file (and the
// editor's BakedBlocks list, /mapblocks2) keeps every generated clip record;
// which of them the game DRAWS is decided elsewhere, and this is where the
// answer is readable: every CSceneMobil is one placed model, and
// CSceneMobil.IsVisible is the engine's own verdict.
//
//   /mobils                      -> {"total":N,"visible":V,"hidden":H}
//   /mobils?vis=0[&limit=N]      -> the hidden mobils: index, model, type
//   /mobils?vis=1&name=SUBSTR    -> visible mobils whose model name contains SUBSTR
//   /mobils?i=N&raw=1            -> one mobil: model, type, visibility, and the raw
//                                   words of the mobil (64 B) and its Item (128 B)
//                                   as float / u32 / hex -- to find the location
//                                   (an iso4) by eye against a known block's cell
//   /mobils?i=N&off=OFF          -> the 12 floats at Item+OFF (an iso4 candidate)
//
// Read-only. No editor needed: works in the editor and in a playground alike.

string ModelName(CSceneMobil@ m) {
    if (m is null) return "<null>";
    string s = "";
    if (m.Model !is null) {
        s = m.Model.IdName;
        if (s == "") s = Reflection::TypeOf(m.Model).Name;
    }
    if (m.Item !is null && m.Item.Solid !is null) {
        string sn = m.Item.Solid.IdName;
        if (sn != "") s += (s == "" ? "" : " solid=") + sn;
    }
    if (s == "") s = Reflection::TypeOf(m).Name;
    return s;
}

string FloatsAt(CMwNod@ nod, uint off, uint count) {
    string s = "";
    for (uint k = 0; k < count; k++) {
        float f = Dev::GetOffsetFloat(nod, off + 4 * k);
        if (k > 0) s += ",";
        s += Text::Format("%.3f", f);
    }
    return s;
}

string RawWords(CMwNod@ nod, uint bytes) {
    string s = "";
    for (uint off = 0; off < bytes; off += 4) {
        uint u = Dev::GetOffsetUint32(nod, off);
        float f = Dev::GetOffsetFloat(nod, off);
        s += "  +" + Text::Format("0x%02x", off) + "  " + Text::Format("%08x", u) + "  " + Text::Format("%.3f", f) + "\n";
    }
    return s;
}

string SceneMobils(const string &in qs) {
    auto app = GetApp();
    if (app.GameScene is null) return "no GameScene";
    CScene@ scene = app.GameScene.HackScene;
    if (scene is null) return "no HackScene";
    uint total = scene.Mobils.Length;
    string si = QArg(qs, "i");
    if (si != "") {
        uint i = uint(Text::ParseInt(si));
        if (i >= total) return "index " + i + " past " + total;
        CSceneMobil@ m = scene.Mobils[i];
        if (m is null) return "mobil " + i + " is null";
        string sb = "mobil " + i + "  model=" + ModelName(m) + "  type=" + Reflection::TypeOf(m).Name + "  visible=" + (m.IsVisible ? "1" : "0") + "\n";
        string soff = QArg(qs, "off");
        if (soff != "" && m.Item !is null) {
            return sb + "item+" + soff + ": " + FloatsAt(m.Item, uint(Text::ParseInt(soff)), 12) + "\n";
        }
        if (QArg(qs, "raw") == "1") {
            sb += "-- mobil (64 B) --\n" + RawWords(m, 64);
            if (m.Item !is null) {
                sb += "-- item " + Reflection::TypeOf(m.Item).Name + " (128 B) --\n" + RawWords(m.Item, 128);
                if (m.Item.Solid !is null) sb += "-- solid " + Reflection::TypeOf(m.Item.Solid).Name + " (192 B) --\n" + RawWords(m.Item.Solid, 192);
            }
        }
        return sb;
    }
    string svis = QArg(qs, "vis");
    string needle = QArg(qs, "name");
    if (svis == "" && needle == "") {
        uint vis = 0;
        for (uint i = 0; i < total; i++) {
            CSceneMobil@ m = scene.Mobils[i];
            if (m !is null && m.IsVisible) vis++;
        }
        return "{\"total\":" + total + ",\"visible\":" + vis + ",\"hidden\":" + (total - vis) + "}";
    }
    int limit = 400;
    if (QArg(qs, "limit") != "") limit = Text::ParseInt(QArg(qs, "limit"));
    bool wantVis = svis != "0";
    string sb = "";
    int n = 0;
    for (uint i = 0; i < total; i++) {
        CSceneMobil@ m = scene.Mobils[i];
        if (m is null) continue;
        if (svis != "" && m.IsVisible != wantVis) continue;
        string mn = ModelName(m);
        if (needle != "" && mn.IndexOf(needle) < 0) continue;
        n++;
        if (n <= limit) sb += i + "\t" + (m.IsVisible ? "1" : "0") + "\t" + mn + "\n";
    }
    return "matched " + n + " of " + total + "\n" + sb;
}
