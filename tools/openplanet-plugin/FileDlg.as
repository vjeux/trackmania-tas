// FileDlg.as -- the ghost-import file dialog, from the object graph.
//
// `/importghosts` raises FrameDialogSaveAs, the game's generic file dialog. Its
// state lives on CGameDialogs: DialogSaveAs_Path, DialogSaveAs_Files (a list of
// CGameFid) and DialogSaveAs_OnValidate / _OnCancel / _OnRefresh. Each CGameFid
// carries Name, Path and -- the one that matters -- Selected.
//
// That is the list the old pipeline was clicking rows in, computing page and
// row indices from `ls | sort` and getting them wrong whenever the folder
// changed underneath it.

string FidName(CGameFid@ fid) {
    if (fid is null) return "";
    string n = fid.Name;
    return n;
}

string SaveAsState() {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "{\"err\":\"no BasicDialogs\"}";
    string j = "{\"display\":\"" + dlg.DialogSaveAs_PathToDisplay + "\"";
    j += ",\"files\":[";
    for (uint i = 0; i < dlg.DialogSaveAs_Files.Length; i++) {
        auto fid = dlg.DialogSaveAs_Files[i];
        if (i > 0) j += ",";
        j += "{\"i\":" + i + ",\"name\":\"" + FidName(fid) + "\"";
        j += ",\"sel\":" + ((fid !is null && fid.Selected) ? "true" : "false") + "}";
    }
    j += "]}";
    return j;
}

// Select every entry whose name contains `want` (empty = all), and report what
// was selected. Selection by NAME, never by row: the row index is a property of
// the folder listing and changes when anything else lands in the folder.
string SaveAsSelect(const string &in want) {  // read-only probe for now
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "no BasicDialogs";
    string hits = "";
    int n = 0;
    for (uint i = 0; i < dlg.DialogSaveAs_Files.Length; i++) {
        auto fid = dlg.DialogSaveAs_Files[i];
        if (fid is null) continue;
        string nm = FidName(fid);
        if (want != "" && nm.IndexOf(want) < 0) continue;
        // (Selected has no setter; selection goes through the list control)
        hits += " " + nm;
        n++;
    }
    return "selected " + n + ":" + hits;
}

string SaveAsValidate() {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "no BasicDialogs";
    dlg.DialogSaveAs_OnValidate();
    return "validated";
}

string SaveAsRefresh() {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "no BasicDialogs";
    dlg.DialogSaveAs_OnRefresh();
    return "refreshed, " + dlg.DialogSaveAs_Files.Length + " entries";
}
