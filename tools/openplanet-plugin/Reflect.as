// Reflect.as -- runtime introspection, so the pipeline is built against the API
// the game ACTUALLY has rather than against a guess.
//
// Every brittle part of the old pipeline is a synthetic click standing in for
// an API nobody looked for. Before replacing one, dump the class:
//
//   /members?t=CGameEditorMediaTrackerPluginAPI
//   /members?t=CGameCtnMediaBlockCameraGame
//
// MwMemberInfo carries Name and Offset (it does NOT carry a Type -- that cost a
// compile cycle to learn, and the compile error in Openplanet.log names the
// line and column, which is the fastest feedback loop available here).

string DumpMembers(const string &in tname) {
    if (tname == "") return "usage: /members?t=CClassName";
    auto ti = Reflection::GetType(tname);
    if (ti is null) return "no such type: " + tname;
    string sb = tname + "  (" + ti.Members.Length + " members)\n";
    for (uint i = 0; i < ti.Members.Length; i++) {
        auto m = ti.Members[i];
        sb += "  " + m.Name + "  @" + m.Offset + "\n";
    }
    return sb;
}

// The type of a LIVE object, plus its members. Answers "what is this thing
// really?" for anything reachable in the object graph.
string DumpInstance(CMwNod@ nod, const string &in label) {
    if (nod is null) return label + ": null\n";
    auto ti = Reflection::TypeOf(nod);
    if (ti is null) return label + ": (no type info)\n";
    string sb = label + ": " + ti.Name + "  (" + ti.Members.Length + " members)\n";
    for (uint i = 0; i < ti.Members.Length; i++) {
        sb += "    " + ti.Members[i].Name + "  @" + ti.Members[i].Offset + "\n";
    }
    return sb;
}
// touch 1787415714
