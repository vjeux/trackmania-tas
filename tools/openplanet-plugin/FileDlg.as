// FileDlg.as -- the ghost-import file dialog, driven from the object graph.
//
// `/importghosts` raises FrameDialogSaveAs, the game's generic file dialog. Its
// state lives on CGameDialogs: DialogSaveAs_Path (offset 176, writable memory),
// DialogSaveAs_Files (a list of CGameFid) and the OnValidate / OnCancel /
// OnRefresh handlers.
//
// WHY NOT THE SELECTION FLAG: CGameFid::Selected reads offset 65535, i.e. it is
// a computed script-side property with no memory behind it, so the row-select
// the UI performs cannot be reproduced that way. What IS writable is the path
// the dialog is pointed at -- and a file dialog given a full path to a file
// does not need a selection at all.

string FidName(CGameFid@ fid) {
    if (fid is null) return "";
    string n = fid.Name;
    return n;
}

string SaveAsState() {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "{\"err\":\"no BasicDialogs\"}";
    string j = "{\"path\":\"" + dlg.DialogSaveAs_Path + "\"";
    j += ",\"display\":\"" + dlg.DialogSaveAs_PathToDisplay + "\"";
    j += ",\"files\":[";
    for (uint i = 0; i < dlg.DialogSaveAs_Files.Length; i++) {
        auto fid = dlg.DialogSaveAs_Files[i];
        if (i > 0) j += ",";
        j += "{\"i\":" + i + ",\"name\":\"" + FidName(fid) + "\"}";
    }
    j += "]}";
    return j;
}

// Point the dialog straight at a file (or a folder) and report what it then
// holds. The offset is resolved BY NAME at runtime -- never hardcoded, because
// a game update moves it and a stale constant would write into whatever field
// now lives there.
string SaveAsSetPath(const string &in path) {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "no BasicDialogs";
    if (path == "") return "empty path (write it to arg.txt first)";
    uint16 off = MemberOffset("CGameDialogs", "DialogSaveAs_Path");
    if (off == 65535) return "could not resolve DialogSaveAs_Path";
    Dev::SetOffset(dlg, off, path);
    return "path now \"" + dlg.DialogSaveAs_Path + "\"";
}

string SaveAsRefresh() {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "no BasicDialogs";
    dlg.DialogSaveAs_OnRefresh();
    return "refreshed, " + dlg.DialogSaveAs_Files.Length + " entries";
}

string SaveAsValidate() {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "no BasicDialogs";
    dlg.DialogSaveAs_OnValidate();
    return "validated";
}

// The whole import, in one call: point the dialog at the file, validate, and
// report the clip's ghost-block count so the caller can see it took.
string ImportGhostFile(const string &in path) {
    auto api = MTApi();
    if (api is null) return "not in the MediaTracker";
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "no BasicDialogs";
    uint before = GhostBlockCount();

    api.ImportGhosts();
    uint16 off = MemberOffset("CGameDialogs", "DialogSaveAs_Path");
    if (off == 65535) return "could not resolve DialogSaveAs_Path";
    Dev::SetOffset(dlg, off, path);
    dlg.DialogSaveAs_OnValidate();

    return "{\"before\":" + before + ",\"path\":\"" + dlg.DialogSaveAs_Path + "\"}";
}

uint GhostBlockCount() {
    auto api = MTApi();
    if (api is null) return 0;
    auto clip = api.Clip;
    if (clip is null) return 0;
    uint n = 0;
    for (uint i = 0; i < clip.Tracks.Length; i++)
        for (uint b = 0; b < clip.Tracks[i].Blocks.Length; b++)
            if (cast<CGameCtnMediaBlockEntity>(clip.Tracks[i].Blocks[b]) !is null) n++;
    return n;
}

// THE IMPORT, in one call, verified.
//
// The working sequence, found by trying the alternatives and reading the clip
// after each: ImportGhosts (raise the dialog) -> write DialogSaveAs_Path ->
// DialogSaveAs_OnValidate (accept the file) -> ImportGhosts_OnOk (perform the
// import). Leave out the last step and the dialog closes having done nothing --
// which is what "no ghost imported" looked like for two hours.
//
// THE PATH MUST BE RELATIVE TO THE REPLAYS FOLDER: "_shoot/1_TAS.Ghost.Gbx".
// A full "C:/..." path is accepted by the field, closes the dialog, and imports
// nothing at all.
//
// Returns the ghost-block count before and after, so the caller never has to
// infer success -- if after == before, it did not happen.
string ImportGhostRel(const string &in relpath) {
    auto api = MTApi();
    if (api is null) return "{\"err\":\"not in the MediaTracker\"}";
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return "{\"err\":\"no BasicDialogs\"}";
    if (relpath == "") return "{\"err\":\"empty path\"}";
    uint16 off = MemberOffset("CGameDialogs", "DialogSaveAs_Path");
    if (off == 65535) return "{\"err\":\"could not resolve DialogSaveAs_Path\"}";

    uint before = GhostBlockCount();
    api.ImportGhosts();
    Dev::SetOffset(dlg, off, relpath);
    dlg.DialogSaveAs_OnValidate();
    api.ImportGhosts_OnOk();

    return "{\"before\":" + before + ",\"after\":" + GhostBlockCount()
         + ",\"path\":\"" + relpath + "\"}";
}
