// Fid Probe
// ---------
// Read-only look at the game's virtual file system (the Fids drives: Game,
// User, Fake, ProgramData, Resource) and at what a map's items resolved to,
// for the 2026-10-17 question from AR: can an item that lives in a map's
// archive (or in the user's Items folder) reach a PACK prefab through its
// GBX reference table? Commands arrive in the storage folder's cmd.txt
// (`<seq> <verb> [arg]`), the report goes to out.txt, state.json carries the
// heartbeat and the last command's first line.
//
//   tree                     every drive's root: parent chain, first-level folders;
//                            the User Items folder and the Game prefab folder chains
//   ls <drive> <path> [d]    a folder's sub-folders and files, d levels deep (default 1)
//   find <drive> <path> <needle> [d]   files whose name contains needle, d levels deep
//   fid <drive> <path>       one file: size, container, Preload -> nod class; for an item
//                            its EntityModel / EntityModelEdition
//   map                      the current map: CopperPrice, every anchored object with its
//                            model's EntityModel state; the Fake drive's embedded files
//
// Nothing here writes game memory; Preload only asks the loader for a nod.

[Setting name="Show window"]
bool S_ShowWindow = false;

uint g_Heartbeat = 0;
int g_CmdSeq = -1;
bool g_CmdSeqLoaded = false;
string g_CmdResult = "";
uint64 g_LastIo = 0;
uint g_Budget = 0;   // lines left for the current listing / search

string StoragePath(const string &in name) { return IO::FromStorageFolder(name); }

string ReadSmallFile(const string &in path) {
    if (!IO::FileExists(path)) return "";
    try {
        IO::File f(path, IO::FileMode::Read);
        string t = f.ReadToEnd();
        f.Close();
        return t.Trim();
    } catch {
        return "";
    }
}

void WriteFile(const string &in path, const string &in text) {
    try {
        IO::File f(path, IO::FileMode::Write);
        f.Write(text);
        f.Close();
    } catch {}
}

string ClassOf(CMwNod@ nod) {
    if (nod is null) return "null";
    auto ti = Reflection::TypeOf(nod);
    if (ti is null) return "?";
    return ti.Name;
}

// Drives ----------------------------------------------------------------------

CSystemFidsFolder@ DriveFolder(const string &in drive, const string &in path) {
    string d = drive.ToLower();
    if (d == "user") return Fids::GetUserFolder(path);
    if (d == "game") return Fids::GetGameFolder(path);
    if (d == "fake") return Fids::GetFakeFolder(path);
    if (d == "programdata" || d == "pd") return Fids::GetProgramDataFolder(path);
    if (d == "resource" || d == "res") return Fids::GetResourceFolder(path);
    return null;
}

CSystemFidFile@ DriveFile(const string &in drive, const string &in path) {
    string d = drive.ToLower();
    if (d == "user") return Fids::GetUser(path);
    if (d == "game") return Fids::GetGame(path);
    if (d == "fake") return Fids::GetFake(path);
    if (d == "programdata" || d == "pd") return Fids::GetProgramData(path);
    if (d == "resource" || d == "res") return Fids::GetResource(path);
    return null;
}

string Chain(CSystemFidsFolder@ f) {
    string s = "";
    int guard = 0;
    while (f !is null && guard < 40) {
        s += "    [" + guard + "] DirName='" + string(f.DirName) + "' FullDirName='" + string(f.FullDirName) + "' trees=" + f.Trees.Length + " leaves=" + f.Leaves.Length + "\n";
        @f = f.ParentFolder;
        guard++;
    }
    if (f !is null) s += "    ... (chain cut at 40)\n";
    return s;
}

string ListFolder(CSystemFidsFolder@ f, int depth, const string &in indent) {
    if (f is null) return indent + "(null folder)\n";
    string s = "";
    for (uint i = 0; i < f.Trees.Length && g_Budget > 0; i++) {
        auto t = f.Trees[i];
        s += indent + "+ " + string(t.DirName) + "  (" + t.Trees.Length + " dirs, " + t.Leaves.Length + " files)\n";
        g_Budget--;
        if (depth > 1) s += ListFolder(t, depth - 1, indent + "  ");
    }
    for (uint i = 0; i < f.Leaves.Length && g_Budget > 0; i++) {
        auto l = f.Leaves[i];
        s += indent + "- " + string(l.FileName) + "  " + l.ByteSize + " B" + (l.Nod is null ? "" : "  nod=" + ClassOf(l.Nod)) + (l.Container is null ? "" : "  in=" + string(l.Container.FileName)) + "\n";
        g_Budget--;
    }
    return s;
}

void FindFiles(CSystemFidsFolder@ f, const string &in needle, int depth, array<CSystemFidFile@>@ hits) {
    if (f is null || g_Budget == 0) return;
    for (uint i = 0; i < f.Leaves.Length; i++) {
        auto l = f.Leaves[i];
        if (string(l.FileName).ToLower().Contains(needle.ToLower())) {
            hits.InsertLast(l);
            g_Budget--;
            if (g_Budget == 0) return;
        }
    }
    if (depth <= 1) return;
    for (uint i = 0; i < f.Trees.Length; i++) FindFiles(f.Trees[i], needle, depth - 1, hits);
}

string DescribeItem(CMwNod@ nod) {
    auto item = cast<CGameItemModel>(nod);
    if (item is null) return "";
    string s = " item: EntityModel=" + ClassOf(item.EntityModel) + " EntityModelEdition=" + ClassOf(item.EntityModelEdition);
    s += " IdName='" + item.IdName + "'";
    return s;
}

string DescribeFid(CSystemFidFile@ fid, bool preload) {
    if (fid is null) return "(no such fid)\n";
    string s = "  FullFileName='" + string(fid.FullFileName) + "' size=" + fid.ByteSize + " compressed=" + (fid.Compressed ? "y" : "n");
    s += " container=" + (fid.Container is null ? "none" : "'" + string(fid.Container.FullFileName) + "'");
    s += " folder='" + (fid.ParentFolder is null ? "" : string(fid.ParentFolder.FullDirName)) + "'";
    s += " nod(before)=" + ClassOf(fid.Nod) + "\n";
    if (preload) {
        CMwNod@ nod = null;
        try {
            @nod = Fids::Preload(fid);
        } catch {
            s += "  Preload THREW: " + getExceptionInfo() + "\n";
        }
        s += "  Preload -> " + ClassOf(nod) + DescribeItem(nod) + "\n";
    } else if (fid.Nod !is null) {
        s += "  nod: " + ClassOf(fid.Nod) + DescribeItem(fid.Nod) + "\n";
    }
    return s;
}

// Commands --------------------------------------------------------------------

string CmdTree() {
    string s = "FID DRIVES at Time::Now=" + Time::Now + "\n";
    array<string> drives = { "game", "user", "fake", "programdata", "resource" };
    for (uint i = 0; i < drives.Length; i++) {
        auto f = DriveFolder(drives[i], "");
        s += "drive " + drives[i] + ":\n";
        if (f is null) { s += "    (null)\n"; continue; }
        s += "  parent chain from the root:\n" + Chain(f);
        g_Budget = 60;
        s += "  contents (1 level):\n" + ListFolder(f, 1, "    ");
    }
    s += "User Items chain:\n" + Chain(Fids::GetUserFolder("Items"));
    s += "Game Stadium/Media/Prefab/Items/ObstaclePusher chain:\n" + Chain(Fids::GetGameFolder("Stadium/Media/Prefab/Items/ObstaclePusher"));
    s += "Game Stadium/Items chain:\n" + Chain(Fids::GetGameFolder("Stadium/Items"));
    g_Budget = 400;
    s += "Fake drive, 4 levels:\n" + ListFolder(Fids::GetFakeFolder(""), 4, "  ");
    return s;
}

string CmdLs(const string &in arg) {
    array<string> parts = arg.Split(" ");
    if (parts.Length < 2) return "ls <drive> <path> [depth]";
    int depth = parts.Length > 2 ? Text::ParseInt(parts[2]) : 1;
    auto f = DriveFolder(parts[0], parts[1]);
    if (f is null) return "ls: no folder '" + parts[1] + "' on drive " + parts[0];
    g_Budget = 2000;
    return "folder '" + string(f.FullDirName) + "' (" + f.Trees.Length + " dirs, " + f.Leaves.Length + " files)\n" + ListFolder(f, depth, "  ");
}

string CmdFind(const string &in arg) {
    array<string> parts = arg.Split(" ");
    if (parts.Length < 3) return "find <drive> <path> <needle> [depth]";
    int depth = parts.Length > 3 ? Text::ParseInt(parts[3]) : 6;
    auto f = DriveFolder(parts[0], parts[1]);
    if (f is null) return "find: no folder '" + parts[1] + "' on drive " + parts[0];
    array<CSystemFidFile@> hits;
    g_Budget = 200;
    FindFiles(f, parts[2], depth, hits);
    string s = hits.Length + " hit(s) for '" + parts[2] + "' under '" + string(f.FullDirName) + "'\n";
    for (uint i = 0; i < hits.Length; i++) s += DescribeFid(hits[i], false);
    return s;
}

string CmdProbe(const string &in arg) {
    array<string> parts = arg.Split(" ");
    if (parts.Length < 2) return "probe <drive> <folder> [needle]";
    string needle = parts.Length > 2 ? parts[2] : ".Item.Gbx";
    auto f = DriveFolder(parts[0], parts[1]);
    if (f is null) return "probe: no folder '" + parts[1] + "' on drive " + parts[0];
    string s = "probe '" + string(f.FullDirName) + "' for '" + needle + "' (" + f.Leaves.Length + " files)\n";
    uint n = 0;
    for (uint i = 0; i < f.Leaves.Length; i++) {
        auto l = f.Leaves[i];
        if (!string(l.FileName).ToLower().Contains(needle.ToLower())) continue;
        s += "- " + string(l.FileName) + "\n" + DescribeFid(l, true);
        n++;
    }
    s += n + " file(s) probed\n";
    return s;
}

string CmdFid(const string &in arg) {
    array<string> parts = arg.Split(" ");
    if (parts.Length < 2) return "fid <drive> <path>";
    auto fid = DriveFile(parts[0], parts[1]);
    if (fid is null) return "fid: no file '" + parts[1] + "' on drive " + parts[0];
    return "fid " + parts[0] + ":" + parts[1] + "\n" + DescribeFid(fid, true);
}

string CmdMap() {
    auto app = cast<CTrackMania>(GetApp());
    CGameCtnChallenge@ map = app.RootMap;
    string where = "RootMap";
    if (map is null && app.Editor !is null) {
        auto ed = cast<CGameCtnEditorFree>(app.Editor);
        if (ed !is null) { @map = ed.Challenge; where = "Editor.Challenge"; }
    }
    if (map is null) return "map: no map loaded";
    string s = where + " '" + map.MapName + "' uid " + map.IdName + " CopperPrice=" + map.CopperPrice + " AnchoredObjects=" + map.AnchoredObjects.Length + "\n";
    for (uint i = 0; i < map.AnchoredObjects.Length; i++) {
        auto o = map.AnchoredObjects[i];
        if (o is null) { s += "  [" + i + "] null\n"; continue; }
        string model = "null";
        string em = "-";
        string eme = "-";
        if (o.ItemModel !is null) {
            model = o.ItemModel.IdName;
            em = ClassOf(o.ItemModel.EntityModel);
            eme = ClassOf(o.ItemModel.EntityModelEdition);
        }
        vec3 p = o.AbsolutePositionInMap;
        s += "  [" + i + "] " + model + " pos=" + Text::Format("%.1f", p.x) + "," + Text::Format("%.1f", p.y) + "," + Text::Format("%.1f", p.z) + " EntityModel=" + em + " Edition=" + eme + "\n";
    }
    // the embedded files, wherever the game put them
    array<CSystemFidFile@> hits;
    g_Budget = 200;
    FindFiles(Fids::GetFakeFolder(""), ".Item.Gbx", 8, hits);
    s += "Fake drive .Item.Gbx files: " + hits.Length + "\n";
    for (uint i = 0; i < hits.Length; i++) s += DescribeFid(hits[i], false);
    array<CSystemFidFile@> hits2;
    g_Budget = 200;
    FindFiles(Fids::GetFakeFolder(""), ".Prefab.Gbx", 8, hits2);
    s += "Fake drive .Prefab.Gbx files: " + hits2.Length + "\n";
    for (uint i = 0; i < hits2.Length; i++) s += DescribeFid(hits2[i], false);
    array<CSystemFidFile@> hits3;
    g_Budget = 200;
    FindFiles(Fids::GetUserFolder(""), ".Item.Gbx", 2, hits3);
    s += "User drive .Item.Gbx files (2 levels): " + hits3.Length + "\n";
    for (uint i = 0; i < hits3.Length && i < 60; i++) s += DescribeFid(hits3[i], false);
    return s;
}

string RunCommand(const string &in verb, const string &in arg) {
    if (verb == "tree") return CmdTree();
    if (verb == "ls") return CmdLs(arg);
    if (verb == "find") return CmdFind(arg);
    if (verb == "fid") return CmdFid(arg);
    if (verb == "probe") return CmdProbe(arg);
    if (verb == "map") return CmdMap();
    if (verb == "ping") return "pong " + Time::Now;
    return "unknown command: " + verb;
}

void LoadCmdSeq() {
    g_CmdSeqLoaded = true;
    string t = ReadSmallFile(StoragePath("cmd.seq"));
    if (t.Length > 0) g_CmdSeq = Text::ParseInt(t);
}

void PollCommand() {
    if (!g_CmdSeqLoaded) LoadCmdSeq();
    string line = ReadSmallFile(StoragePath("cmd.txt"));
    if (line.Length == 0) return;
    array<string> parts = line.Split(" ", 3);
    if (parts.Length < 2) return;
    int seq = Text::ParseInt(parts[0]);
    if (seq <= g_CmdSeq) return;
    g_CmdSeq = seq;
    WriteFile(StoragePath("cmd.seq"), "" + g_CmdSeq);
    string report;
    try {
        report = RunCommand(parts[1], parts.Length > 2 ? parts[2] : "");
    } catch {
        report = "EXCEPTION: " + getExceptionInfo();
    }
    WriteFile(StoragePath("out.txt"), report);
    g_CmdResult = report.Split("\n")[0];
    trace("[FidProbe] cmd #" + seq + " " + parts[1] + " -> " + report.Length + " chars");
}

string JsonEscape(const string &in s) {
    string esc = s;
    esc = esc.Replace("\\", "\\\\");
    esc = esc.Replace("\"", "\\\"");
    esc = esc.Replace("\n", " ");
    return esc;
}

void WriteState() {
    string s = "{\n";
    s += "  \"heartbeat\": " + g_Heartbeat + ",\n";
    s += "  \"cmd_seq\": " + g_CmdSeq + ",\n";
    s += "  \"cmd_result\": \"" + JsonEscape(g_CmdResult) + "\"\n";
    s += "}\n";
    WriteFile(StoragePath("state.json"), s);
}

void Update(float dt) {
    g_Heartbeat++;
    uint64 now = Time::Now;
    if (now - g_LastIo < 200) return;
    g_LastIo = now;
    PollCommand();
    WriteState();
}

void RenderMenu() {
    if (UI::MenuItem("\\$8af" + Icons::FolderOpen + "\\$z Fid Probe", "", S_ShowWindow)) S_ShowWindow = !S_ShowWindow;
}

void RenderInterface() {
    if (!S_ShowWindow) return;
    if (UI::Begin("Fid Probe", S_ShowWindow)) {
        UI::Text("heartbeat " + g_Heartbeat + "  last cmd #" + g_CmdSeq);
        UI::TextWrapped(g_CmdResult);
    }
    UI::End();
}
