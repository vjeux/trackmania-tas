// Kine.as -- the RUNTIME side of a kinematic (moving) item, and the knobs a
// plugin can turn on it. The question this answers (Yannex, 2026-09-24): can
// an Openplanet plugin move a PHYSICAL obstacle -- does the car's collision
// follow a moving part whose pose the plugin forces, or only the picture?
//
// WHAT THE GAME HAS (Openplanet 1.29.14 reflection, OpenplanetNext.json):
//   CScene.ScenePhy (IScenePhy) .Dyna (NSceneDyna_SMgr)
//       .KinematicConstraints        MwFastBuffer<NSceneDyna_SKinematicConstraint>   (64 B each, opaque)
//       .KinematicSharedSignals      MwSArray<NSceneDyna_SKinematicSharedSignal@>    { Model, Phase, cRef }
//       .Items.States                MwFastBuffer<NSceneDyna_SItemState>             { Vel, AngularVel }
//   the Model of a shared signal is the item's NPlugDyna_SKinematicConstraint
//   (the file's KinematicConstraint: TransAxis, TransMin, TransMax, RotAxis,
//   AngleMinDeg, AngleMaxDeg, the anim funcs), shared by every instance of the
//   item model in the map; a signal is one (model, phase) pair -- the phase is
//   the map's per-placement AnimPhaseOffset in eighths, as a float.
//   The playground's physics scene is CSmArenaClient.Arena.ArenaPhysics.ScenePhy.
//
// ROUTES
//   /kine[?name=SUBSTR][&all=1]  (read)
//       the dyna manager of every scene we can reach (playground arena physics,
//       GameScene.HackScene, the editor's grid scene): counts, then one row per
//       shared signal
//         sig  i  model=<ident>  taxis  tmin  tmax  raxis  amin  amax  phase  cref
//       then the scene mobils whose model/solid name contains SUBSTR (default
//       "Pusher"), with the visual world transform CSceneMobil.Item.Corpus.Location
//       (CHmsZoneElem: iso4 rotation rows + translation, and Vel), and the map's
//       anchored objects matching SUBSTR (position, AnimPhaseOffset).
//   /kineset?i=N&tmin=F&tmax=F[&amin=F&amax=F][&phase=F]   (lock token)
//       write the fields of shared signal N's MODEL (and/or the signal's Phase);
//       prints the row before and after. tmin=tmax=X holds the part at X along
//       its axis whatever the clock says -- a door "opened" by a plugin.
//   /kinemove?i=MOBIL&dx=F&dy=F&dz=F   (lock token)
//       add (dx,dy,dz) to mobil MOBIL's Corpus.Location translation: the
//       VISUAL scene object moved directly, to see whether the collision comes
//       along (it is expected not to -- the physics body lives in NSceneDyna).
//
// Everything here goes through reflected members -- no raw pointer reads --
// so a null on the way is a message, not a crash.

string F3(float f) { return Text::Format("%.3f", f); }

string Iso4Row(const iso4 &in m) {
    return "t=(" + F3(m.tx) + "," + F3(m.ty) + "," + F3(m.tz) + ")"
        + " x=(" + F3(m.xx) + "," + F3(m.xy) + "," + F3(m.xz) + ")"
        + " y=(" + F3(m.yx) + "," + F3(m.yy) + "," + F3(m.yz) + ")"
        + " z=(" + F3(m.zx) + "," + F3(m.zy) + "," + F3(m.zz) + ")";
}

// The physics scene of the playground the car drives in, or null.
IScenePhy@ ArenaPhy() {
    auto app = cast<CTrackMania>(GetApp());
    if (app is null) return null;
    auto pg = cast<CSmArenaClient>(app.CurrentPlayground);
    if (pg is null || pg.Arena is null || pg.Arena.ArenaPhysics is null) return null;
    return pg.Arena.ArenaPhysics.ScenePhy;
}

string SigRow(NSceneDyna_SMgr@ dyna, uint i) {
    auto sig = dyna.KinematicSharedSignals[i];
    if (sig is null) return "sig " + i + " null\n";
    NPlugDyna_SKinematicConstraint@ mdl = sig.Model;
    string s = "sig " + i + " model=";
    if (mdl is null) {
        s += "<null>";
    } else {
        string id = mdl.IdName;
        s += (id == "" ? Reflection::TypeOf(mdl).Name : id)
            + " taxis=" + int(mdl.TransAxis) + " tmin=" + F3(mdl.TransMin) + " tmax=" + F3(mdl.TransMax)
            + " raxis=" + int(mdl.RotAxis) + " amin=" + F3(mdl.AngleMinDeg) + " amax=" + F3(mdl.AngleMaxDeg)
            + " tc=" + int(mdl.ShaderTcType);
    }
    s += " phase=" + F3(sig.Phase) + " cref=" + sig.cRef + "\n";
    return s;
}

string DynaReport(const string &in label, IScenePhy@ phy) {
    if (phy is null) return label + ": no IScenePhy\n";
    auto dyna = phy.Dyna;
    if (dyna is null) return label + ": no Dyna manager\n";
    string s = label + ": constraints=" + dyna.KinematicConstraints.Length
        + " signals=" + dyna.KinematicSharedSignals.Length + "\n";
    for (uint i = 0; i < dyna.KinematicSharedSignals.Length; i++) s += "  " + SigRow(dyna, i);
    return s;
}

// The scene whose mobils are the world: the editor's grid scene in the editor,
// GameScene.HackScene otherwise (Mobils.as / TreeInst.as).
CScene@ WorldScene(string &out root) {
    auto app = GetApp();
    auto ed = cast<CGameCtnEditorCommon>(app.Editor);
    if (ed !is null && ed.Grid !is null && ed.Grid.Scene !is null) { root = "editor.Grid.Scene"; return ed.Grid.Scene; }
    if (app.GameScene !is null && app.GameScene.HackScene !is null) { root = "GameScene.HackScene"; return app.GameScene.HackScene; }
    root = "none";
    return null;
}

string MobilRows(CScene@ scene, const string &in needle, bool all) {
    if (scene is null) return "  no scene\n";
    string s = "";
    uint n = 0;
    for (uint i = 0; i < scene.Mobils.Length; i++) {
        CSceneMobil@ m = scene.Mobils[i];
        if (m is null) continue;
        string mn = ModelName(m);
        if (!all && needle != "" && mn.IndexOf(needle) < 0) continue;
        n++;
        if (n > 200) break;
        s += "  mobil " + i + " " + mn + " vis=" + (m.IsVisible ? "1" : "0");
        if (m.Item is null) { s += " item=null\n"; continue; }
        s += " static=" + (m.Item.IsStatic ? "1" : "0");
        auto c = m.Item.Corpus;
        if (c is null) { s += " corpus=null\n"; continue; }
        s += " " + Iso4Row(c.Location) + " vel=(" + F3(c.Vel.x) + "," + F3(c.Vel.y) + "," + F3(c.Vel.z) + ")\n";
    }
    return "  mobils matching '" + needle + "': " + n + " of " + scene.Mobils.Length + "\n" + s;
}

string AnchoredRows(const string &in needle) {
    auto map = GetApp().RootMap;
    if (map is null) return "  no RootMap\n";
    string s = "";
    uint n = 0;
    for (uint i = 0; i < map.AnchoredObjects.Length; i++) {
        auto o = map.AnchoredObjects[i];
        if (o is null || o.ItemModel is null) continue;
        string id = o.ItemModel.IdName;
        if (needle != "" && id.IndexOf(needle) < 0) continue;
        n++;
        if (n > 50) break;
        vec3 p = o.AbsolutePositionInMap;
        s += "  anchored " + i + " " + id + " pos=(" + F3(p.x) + "," + F3(p.y) + "," + F3(p.z) + ")"
            + " yaw=" + F3(o.Yaw) + " phase8=" + int(o.AnimPhaseOffset) + " scale=" + F3(o.Scale) + "\n";
    }
    return "  anchored objects matching '" + needle + "': " + n + " of " + map.AnchoredObjects.Length + "\n" + s;
}

string Kine(const string &in qs) {
    string needle = QArg(qs, "name");
    if (needle == "") needle = "Pusher";
    bool all = QArg(qs, "all") == "1";
    string s = "";
    s += DynaReport("arena.ArenaPhysics.ScenePhy", ArenaPhy());
    string root;
    CScene@ scene = WorldScene(root);
    if (scene !is null) {
        s += DynaReport(root + ".ScenePhy", scene.ScenePhy);
        if (scene.ServerScenePhy !is null) s += DynaReport(root + ".ServerScenePhy", scene.ServerScenePhy);
    } else {
        s += "no world scene\n";
    }
    s += MobilRows(scene, needle, all);
    s += AnchoredRows(needle);
    return s;
}

// Which dyna manager a write goes to: the arena's, else the world scene's.
NSceneDyna_SMgr@ WriteDyna(string &out where) {
    auto phy = ArenaPhy();
    where = "arena";
    if (phy is null) {
        string root;
        CScene@ scene = WorldScene(root);
        if (scene !is null) { @phy = scene.ScenePhy; where = root; }
    }
    if (phy is null) return null;
    return phy.Dyna;
}

string KineSet(const string &in qs) {
    string si = QArg(qs, "i");
    if (si == "") return "usage: /kineset?i=N&tmin=F&tmax=F[&amin=F&amax=F][&phase=F]";
    string where;
    auto dyna = WriteDyna(where);
    if (dyna is null) return "no dyna manager";
    uint i = uint(Text::ParseInt(si));
    if (i >= dyna.KinematicSharedSignals.Length) return "signal " + i + " past " + dyna.KinematicSharedSignals.Length;
    auto sig = dyna.KinematicSharedSignals[i];
    if (sig is null) return "signal " + i + " is null";
    string s = "[" + where + "] before: " + SigRow(dyna, i);
    NPlugDyna_SKinematicConstraint@ mdl = sig.Model;
    string tmin = QArg(qs, "tmin"), tmax = QArg(qs, "tmax"), amin = QArg(qs, "amin"), amax = QArg(qs, "amax"), phase = QArg(qs, "phase");
    if (mdl !is null) {
        if (tmin != "") mdl.TransMin = Text::ParseFloat(tmin);
        if (tmax != "") mdl.TransMax = Text::ParseFloat(tmax);
        if (amin != "") mdl.AngleMinDeg = Text::ParseFloat(amin);
        if (amax != "") mdl.AngleMaxDeg = Text::ParseFloat(amax);
    } else if (tmin != "" || tmax != "" || amin != "" || amax != "") {
        s += "model is null: range not written\n";
    }
    if (phase != "") sig.Phase = Text::ParseFloat(phase);
    s += "after:  " + SigRow(dyna, i);
    return s;
}

string KineMove(const string &in qs) {
    string si = QArg(qs, "i");
    if (si == "") return "usage: /kinemove?i=MOBIL&dx=F&dy=F&dz=F";
    string root;
    CScene@ scene = WorldScene(root);
    if (scene is null) return "no world scene";
    uint i = uint(Text::ParseInt(si));
    if (i >= scene.Mobils.Length) return "mobil " + i + " past " + scene.Mobils.Length;
    CSceneMobil@ m = scene.Mobils[i];
    if (m is null || m.Item is null || m.Item.Corpus is null) return "mobil " + i + " has no Item.Corpus";
    CHmsCorpus@ c = m.Item.Corpus;
    string s = "[" + root + "] mobil " + i + " " + ModelName(m) + "\nbefore: " + Iso4Row(c.Location) + "\n";
    // iso4 members are get-only properties in this Openplanet (a `loc.tx +=`
    // failed to compile and took the plugin down for every driver on the box,
    // 2026-09-24 16:52): the translation is written through the reflected
    // byte offset of CHmsZoneElem::Location instead -- inside the corpus
    // object itself, no pointer followed. The iso4 lies rotation first (xx xy
    // xz yx yy yz zx zy zz) then tx ty tz, as in the file format.
    // MemberOffset is Camera.as's (one namespace for the whole plugin; 65535 = not found)
    uint16 off = MemberOffset("CHmsZoneElem", "Location");
    if (off == 65535) return s + "no reflected CHmsZoneElem::Location -- not written\n";
    uint16 ox = off + 36;
    uint16 oy = off + 40;
    uint16 oz = off + 44;
    float tx = Dev::GetOffsetFloat(c, ox) + Text::ParseFloat(QArg(qs, "dx"));
    float ty = Dev::GetOffsetFloat(c, oy) + Text::ParseFloat(QArg(qs, "dy"));
    float tz = Dev::GetOffsetFloat(c, oz) + Text::ParseFloat(QArg(qs, "dz"));
    Dev::SetOffset(c, ox, tx);
    Dev::SetOffset(c, oy, ty);
    Dev::SetOffset(c, oz, tz);
    s += "after:  " + Iso4Row(c.Location) + " (Location @" + off + ")\n";
    return s;
}

