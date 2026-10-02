// Reactor Probe
// -------------
// Read-only probe of the REACTOR state that Trackmania keeps on the car's
// physics object, for AR's tm_Reactor-Duration plugin (Discord, 2026-09-25).
// Nothing here writes game memory. REACTOR.md next to this file carries the
// reverse engineering; the short version, client build 2026-01-28 (git 128130,
// GameVersion 3.3.0):
//
//   PhysicsStep_TM (Trackmania.exe+0x1101800) -> per-car gameplay step
//   (+0x43df50) -> reactor update (+0x43d5b0), once per 10 ms physics tick:
//
//     touching = contact with gameplay material 12 (ReactorBoost), 14
//                (ReactorBoost2), 18 (ReactorBoost_Oriented) or 19
//                (ReactorBoost2_Oriented) -- pad, ring or gate alike
//     if (touching) {
//         car+0x13A8 = type   (1 up / 2 down, from the surface normal)
//         car+0x13AC = level  (1 for 12/18, 2 for 14/19)
//         car+0x13B0 = now    <- REFRESHED EVERY TICK WHILE TOUCHING
//         if (old type == 0) car+0x13B4 = now     (activation)
//         car+0x13BC = tuning.ReactorDuration / coef   (6000 ms -> 0x1770)
//     }
//     end = car+0x13B0 + car+0x13BC; now > end -> type/level/+0x13B8 zeroed,
//     +0x13B0/+0x13B4/+0x13BC KEPT (so 6000 stays after the boost ends).
//
//   `now` is the physics step's own millisecond clock, the playground's
//   GameTime. car+0x13B8 is the wheel-force ramp start (zeroed whenever the
//   force stops), the value XertroV saw "become 0". The vis state the game
//   builds for Openplanet (+0x3d1380) derives ReactorBoostLvl / Type /
//   ReactorFinalTimer from exactly these fields, at CSceneVehicleVisState
//   +0x174 / +0x178 / +0x17C.
//
// What this probe does:
//   * given the car pointer (the Jump Button hook's, `car <hex>`), logs the
//     six reactor dwords every frame next to the vis state's reactor enums,
//     the race time and the position -> reactor.log (TSV) in the storage
//     folder, and scans the CSmPlayer / CSmScriptPlayer / CSceneVehicleVis
//     objects for the pointers that lead to the car (discovery.txt);
//   * `scan` finds the car WITHOUT the hook: every qword of the CSmPlayer
//     that points at an object whose +0x88 carries the physics tuning
//     signature (SpeedCap's MaxSpeed neighbours) and whose tuning has
//     ReactorDuration = 6000 at +0x3290.
//
// Automation protocol: state.json (4 Hz) + cmd.txt / cmd.seq, as SpeedCap.

const uint64 R_TYPE = 0x13A8;        // u32: 0 none, 1 up, 2 down
const uint64 R_LVL = 0x13AC;         // u32: 0 none, 1 boost, 2 boost2
const uint64 R_CONTACT = 0x13B0;     // u32 ms: last tick in contact (refreshed every tick)
const uint64 R_ACTIVATION = 0x13B4;  // u32 ms: first contact of this boost
const uint64 R_START = 0x13B8;       // u32 ms: wheel-force ramp start, 0 while no force
const uint64 R_DURATION = 0x13BC;    // u32 ms: 6000 / coef
const uint64 R_COEF = 0x1AFC;        // f32: the coef, 1.0 normally
const uint64 R_SHAPE = 0x1C60;       // u64: the touched shape's handle, 0 when off
const uint64 CAR_TUNING = 0x88;      // car -> active physics tuning
const uint64 TUNING_MAXSPEED = 0x2F0;
const uint64 TUNING_SIG_1 = 0x2F4;   // 100.0
const uint64 TUNING_SIG_2 = 0x2F8;   // 0.3
const uint64 TUNING_SIG_3 = 0x2FC;   // 10000.0
const uint SIG_1_BITS = 0x42C80000;
const uint SIG_2_BITS = 0x3E99999A;
const uint SIG_3_BITS = 0x461C4000;
const uint64 TUNING_REACTOR_ENABLED = 0x3278;   // u32, the step skips the reactor when 0
const uint64 TUNING_REACTOR_DURATION = 0x3290;  // u32, 6000
const uint REACTOR_DURATION_DEFAULT = 6000;
const uint CAR_SCAN_BYTES = 0x1D00;   // the object is at least 0x1C98 bytes (fields read by the step)
const uint VIS_MANAGER_INDEX = 13;    // NSceneVehicleVis_SMgr in the scene's manager table (VehicleState)
const uint VIS_VEHICLES_OFFSET = 0x210;

[Setting name="Show debug window"]
bool S_ShowDebug = true;

[Setting name="Log every frame while a car is known"]
bool S_Log = true;

// Runtime state -----------------------------------------------------------

uint64 g_Car = 0;
string g_CarSource = "";
uint64 g_Tuning = 0;
uint g_Heartbeat = 0;
string g_Status = "starting up";
bool g_InMap = false;
uint g_LogCount = 0;
uint g_ContactFrames = 0;
array<string> g_Pending;
uint64 g_LastFlush = 0;
uint64 g_LastTickMs = 0;
string g_Discovery = "";
string g_Report = "";
int g_CmdSeq = -1;
bool g_CmdSeqLoaded = false;
string g_CmdResult = "";
bool g_LogHeaderWritten = false;

// last sampled values, for the state file and the debug window
uint g_Type = 0, g_Lvl = 0, g_Contact = 0, g_Activation = 0, g_Start = 0, g_Duration = 0;
int g_GameTime = 0;
int g_VisLvl = 0, g_VisType = 0;
float g_FinalTimer = 0.0f;

string Hex(uint64 v) {
    if (v == 0) return "0";
    string digits = "0123456789ABCDEF";
    string s = "";
    while (v != 0) {
        s = digits.SubStr(int(v & 0xF), 1) + s;
        v >>= 4;
    }
    return s;
}
string Hex32(uint v) { return Text::Format("%08X", v); }
string F3(float v) { return Text::Format("%.3f", v); }
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

void WriteSmallFile(const string &in path, const string &in text) {
    try {
        IO::File f(path, IO::FileMode::Write);
        f.Write(text);
        f.Close();
    } catch {}
}

void AppendFile(const string &in path, const string &in text) {
    try {
        IO::File f(path, IO::FileMode::Append);
        f.Write(text);
        f.Close();
    } catch {}
}

bool LooksLikePointer(uint64 p) { return p > 0x10000 && p < 0x00007FFFFFFFFFFF; }

// The game objects -----------------------------------------------------------

CSmArenaClient@ Playground() {
    auto app = cast<CTrackMania>(GetApp());
    if (app is null) return null;
    return cast<CSmArenaClient>(app.CurrentPlayground);
}

CGameTerminal@ LocalTerminal() {
    auto pg = Playground();
    if (pg is null || pg.GameTerminals.Length == 0) return null;
    return pg.GameTerminals[0];
}

CSmPlayer@ LocalPlayer() {
    auto term = LocalTerminal();
    if (term is null) return null;
    CSmPlayer@ pl = cast<CSmPlayer>(term.ControlledPlayer);
    if (pl is null) @pl = cast<CSmPlayer>(term.GUIPlayer);
    return pl;
}

int GameTime() {
    auto app = cast<CTrackMania>(GetApp());
    if (app is null || app.Network is null || app.Network.PlaygroundClientScriptAPI is null) return 0;
    return app.Network.PlaygroundClientScriptAPI.GameTime;
}

/// The byte offset of a reflected member, or -1.
int MemberOffset(CMwNod@ nod, const string &in member) {
    if (nod is null) return -1;
    auto ti = Reflection::TypeOf(nod);
    if (ti is null) return -1;
    auto m = ti.GetMember(member);
    if (m is null) return -1;
    return int(m.Offset);
}

/// The raw address a reflected pointer member holds.
uint64 RawMember(CMwNod@ holder, const string &in member) {
    int off = MemberOffset(holder, member);
    if (off < 0) return 0;
    return Dev::GetOffsetUint64(holder, uint16(off));
}

// Openplanet's MwClassInfo carries no size, so the object sizes come from the
// game's own reflection dump (OpenplanetNext.json, build 128130).
const uint SIZE_CSMPLAYER = 48800;       // 0xBEA0
const uint SIZE_CSMSCRIPTPLAYER = 0x100; // reflected 88 bytes; a thin wrapper, over-read a little
const uint SIZE_CSCENEVEHICLEVIS = 4264; // 0x10A8

uint ObjectSize(CMwNod@ nod, uint fallback) {
    auto ti = Reflection::TypeOf(nod);
    if (ti is null) return fallback;
    if (ti.Name == "CSmPlayer") return SIZE_CSMPLAYER;
    if (ti.Name == "CSmScriptPlayer") return SIZE_CSMSCRIPTPLAYER;
    if (ti.Name == "CSceneVehicleVis") return SIZE_CSCENEVEHICLEVIS;
    return fallback;
}

/// The player's vehicle vis: its nod and its raw address (from the vis
/// manager's array, since Openplanet has no address-of for a nod).
CSceneVehicleVis@ PlayerVis(CSmPlayer@ player, uint64 &out raw) {
    raw = 0;
    auto app = GetApp();
    if (app.GameScene is null) return null;
    uint managers = Dev::GetOffsetUint32(app.GameScene, 0x8);
    if (VIS_MANAGER_INDEX > managers) return null;
    auto mgr = Dev::GetOffsetNod(app.GameScene, 0x10 + VIS_MANAGER_INDEX * 0x8);
    if (mgr is null) return null;
    uint64 arr = Dev::GetOffsetUint64(mgr, VIS_VEHICLES_OFFSET);
    uint count = Dev::GetOffsetUint32(mgr, VIS_VEHICLES_OFFSET + 0x8);
    if ((arr & 0xF) != 0 || count > 1000) return null;
    auto vehicles = Dev::GetOffsetNod(mgr, VIS_VEHICLES_OFFSET);
    if (vehicles is null) return null;
    uint wanted = player.GetCurrentEntityID();
    for (uint i = 0; i < count; i++) {
        auto nod = Dev::GetOffsetNod(vehicles, i * 0x8);
        if (nod is null) continue;
        uint id = Dev::GetOffsetUint32(nod, 0);
        if (wanted != 0 && id != wanted) continue;
        if (wanted == 0 && (id & 0x02000000) == 0) continue;
        raw = Dev::GetOffsetUint64(vehicles, i * 0x8);
        return Dev::ForceCast<CSceneVehicleVis@>(nod).Get();
    }
    return null;
}

// The car -----------------------------------------------------------------

bool TuningSignatureAt(uint64 tuning) {
    try {
        return Dev::SafeReadUInt32(tuning + TUNING_SIG_1) == SIG_1_BITS
            && Dev::SafeReadUInt32(tuning + TUNING_SIG_2) == SIG_2_BITS
            && Dev::SafeReadUInt32(tuning + TUNING_SIG_3) == SIG_3_BITS;
    } catch {
        return false;
    }
}

/// True when `car` looks like the physics car: +0x88 is a tuning with the
/// MaxSpeed signature whose reactor duration is the 6000 default.
bool CarSignature(uint64 car, uint64 &out tuning, string &out why) {
    tuning = 0;
    if (!LooksLikePointer(car) || (car & 0x7) != 0) { why = "not an aligned pointer"; return false; }
    uint64 t = 0;
    try {
        t = Dev::SafeReadUInt64(car + CAR_TUNING);
    } catch {
        why = "car+0x88 unreadable";
        return false;
    }
    if (!LooksLikePointer(t)) { why = "car+0x88 is not a pointer"; return false; }
    if (!TuningSignatureAt(t)) { why = "tuning at car+0x88 lacks the MaxSpeed signature"; return false; }
    uint dur = 0;
    try {
        dur = Dev::SafeReadUInt32(t + TUNING_REACTOR_DURATION);
    } catch {
        why = "tuning+0x3290 unreadable";
        return false;
    }
    if (dur != REACTOR_DURATION_DEFAULT) { why = "tuning+0x3290 is " + dur + ", not 6000"; return false; }
    tuning = t;
    why = "ok";
    return true;
}

void SetCar(uint64 car, const string &in source) {
    uint64 tuning = 0;
    string why;
    if (!CarSignature(car, tuning, why)) {
        g_Status = "car " + Text::FormatPointer(car) + " refused: " + why;
        warn("[ReactorProbe] " + g_Status);
        return;
    }
    g_Car = car;
    g_Tuning = tuning;
    g_CarSource = source;
    g_Status = "car " + Text::FormatPointer(car) + " (" + source + "), tuning " + Text::FormatPointer(tuning);
    trace("[ReactorProbe] " + g_Status);
}

void DropCar(const string &in why) {
    if (g_Car == 0) return;
    trace("[ReactorProbe] dropping car " + Text::FormatPointer(g_Car) + ": " + why);
    g_Car = 0;
    g_Tuning = 0;
    g_CarSource = "";
    g_Status = "no car: " + why;
}

// Discovery -----------------------------------------------------------------

/// Every qword of `nod` that points at (or near) one of the known objects.
void ScanNod(const string &in label, CMwNod@ nod, uint size, uint64 car, uint64 playerRaw, uint64 scriptRaw, uint64 visRaw, uint64 tuning) {
    if (nod is null) { g_Report += label + ": null\n"; return; }
    if (size > 0xFFF0) size = 0xFFF0;
    uint hits = 0;
    for (uint off = 0; off + 8 <= size; off += 8) {
        uint64 q = Dev::GetOffsetUint64(nod, uint16(off));
        if (q == 0) continue;
        if (car != 0) {
            int64 d = int64(q) - int64(car);
            if (d > -0x4000 && d < 0x4000) {
                g_Report += "  " + label + "+0x" + Hex(off) + " -> car" + (d >= 0 ? "+0x" + Hex(uint64(d)) : "-0x" + Hex(uint64(-d))) + "\n";
                hits++;
            }
        }
        if (q == playerRaw) { g_Report += "  " + label + "+0x" + Hex(off) + " -> CSmPlayer\n"; hits++; }
        if (q == scriptRaw) { g_Report += "  " + label + "+0x" + Hex(off) + " -> CSmScriptPlayer\n"; hits++; }
        if (q == visRaw) { g_Report += "  " + label + "+0x" + Hex(off) + " -> CSceneVehicleVis\n"; hits++; }
        if (tuning != 0 && q == tuning) { g_Report += "  " + label + "+0x" + Hex(off) + " -> tuning\n"; hits++; }
    }
    g_Report += label + ": " + hits + " hit(s) in 0x" + Hex(size) + " bytes\n";
}

/// The car object itself: pointers back to the known objects, the vis
/// state's position, the reactor block.
void ScanCar(uint64 car, uint64 playerRaw, uint64 scriptRaw, uint64 visRaw, uint64 tuning, vec3 pos) {
    uint hits = 0;
    uint readable = 0;
    for (uint off = 0; off + 8 <= CAR_SCAN_BYTES; off += 8) {
        uint64 q = 0;
        try {
            q = Dev::SafeReadUInt64(car + off);
        } catch {
            g_Report += "  car+0x" + Hex(off) + ": unreadable, scan stops\n";
            break;
        }
        readable = off + 8;
        if (q == playerRaw && q != 0) { g_Report += "  car+0x" + Hex(off) + " -> CSmPlayer\n"; hits++; }
        if (q == scriptRaw && q != 0) { g_Report += "  car+0x" + Hex(off) + " -> CSmScriptPlayer\n"; hits++; }
        if (q == visRaw && q != 0) { g_Report += "  car+0x" + Hex(off) + " -> CSceneVehicleVis\n"; hits++; }
        if (q == tuning && q != 0) { g_Report += "  car+0x" + Hex(off) + " -> tuning\n"; hits++; }
    }
    // the vis state copies inside the car: three floats equal to the vis position
    for (uint off = 0; off + 12 <= readable; off += 4) {
        vec3 v;
        try {
            v = Dev::SafeReadVec3(car + off);
        } catch {
            break;
        }
        if (Math::Abs(v.x - pos.x) < 0.02f && Math::Abs(v.y - pos.y) < 0.02f && Math::Abs(v.z - pos.z) < 0.02f) {
            g_Report += "  car+0x" + Hex(off) + " = vis Position (" + F3(v.x) + " " + F3(v.y) + " " + F3(v.z) + ")\n";
            hits++;
        }
    }
    g_Report += "car: " + hits + " hit(s) in 0x" + Hex(readable) + " readable bytes\n";
    g_Report += "car reactor block: type=" + Dev::ReadUInt32(car + R_TYPE) + " lvl=" + Dev::ReadUInt32(car + R_LVL)
        + " contact=" + Dev::ReadUInt32(car + R_CONTACT) + " activation=" + Dev::ReadUInt32(car + R_ACTIVATION)
        + " start=" + Dev::ReadUInt32(car + R_START) + " duration=" + Dev::ReadUInt32(car + R_DURATION)
        + " coef=" + F3(Dev::ReadFloat(car + R_COEF)) + " shape=" + Text::FormatPointer(Dev::ReadUInt64(car + R_SHAPE)) + "\n";
}

string MembersOf(CMwNod@ nod, const string &in label) {
    if (nod is null) return label + ": null\n";
    auto ti = Reflection::TypeOf(nod);
    if (ti is null) return label + ": no type info\n";
    string sb = label + " is " + ti.Name + " (" + ti.Members.Length + " members)\n";
    for (uint i = 0; i < ti.Members.Length; i++) {
        sb += "  +0x" + Hex(ti.Members[i].Offset) + " " + ti.Members[i].Name + "\n";
    }
    return sb;
}

string MembersOfType(const string &in tname) {
    auto ti = Reflection::GetType(tname);
    if (ti is null) return tname + ": no such type\n";
    string sb = tname + " (" + ti.Members.Length + " members)\n";
    for (uint i = 0; i < ti.Members.Length; i++) {
        sb += "  +0x" + Hex(ti.Members[i].Offset) + " " + ti.Members[i].Name + "\n";
    }
    return sb;
}

/// With a known car: where do the Openplanet-reachable objects point at it?
string Discover() {
    auto term = LocalTerminal();
    auto player = LocalPlayer();
    if (term is null || player is null) return "discover: no local player";
    uint64 playerRaw = RawMember(term, "ControlledPlayer");
    if (playerRaw == 0) playerRaw = RawMember(term, "GUIPlayer");
    uint64 scriptRaw = RawMember(player, "ScriptAPI");
    CSmScriptPlayer@ script = cast<CSmScriptPlayer>(player.ScriptAPI);
    uint64 visRaw = 0;
    CSceneVehicleVis@ vis = PlayerVis(player, visRaw);
    vec3 pos = vec3(0, 0, 0);
    CMwNod@ visNod = null;
    if (vis !is null) {
        pos = vis.AsyncState.Position;
        @visNod = Dev::ForceCast<CMwNod@>(vis).Get();
    }

    string r = "discovery at Time::Now=" + Time::Now + " GameTime=" + GameTime() + "\n";
    r += "car=" + Text::FormatPointer(g_Car) + " (" + g_CarSource + ") tuning=" + Text::FormatPointer(g_Tuning) + "\n";
    r += "CSmPlayer=" + Text::FormatPointer(playerRaw) + " CSmScriptPlayer=" + Text::FormatPointer(scriptRaw)
        + " CSceneVehicleVis=" + Text::FormatPointer(visRaw) + " entity=" + Hex32(player.GetCurrentEntityID())
        + " visPos=" + F3(pos.x) + " " + F3(pos.y) + " " + F3(pos.z) + "\n";
    r += "CSmPlayer.Score offset=0x" + Hex(uint64(MemberOffset(player, "Score")))
        + " ScriptAPI offset=0x" + Hex(uint64(MemberOffset(player, "ScriptAPI")))
        + " SpawnIndex offset=0x" + Hex(uint64(MemberOffset(player, "SpawnIndex"))) + "\n";
    g_Report = "";
    ScanNod("CSmPlayer", player, ObjectSize(player, 0x2000), g_Car, playerRaw, scriptRaw, visRaw, g_Tuning);
    if (script !is null) ScanNod("CSmScriptPlayer", script, ObjectSize(script, 0x100), g_Car, playerRaw, scriptRaw, visRaw, g_Tuning);
    if (visNod !is null) ScanNod("CSceneVehicleVis", visNod, SIZE_CSCENEVEHICLEVIS, g_Car, playerRaw, scriptRaw, visRaw, g_Tuning);
    if (g_Car != 0) ScanCar(g_Car, playerRaw, scriptRaw, visRaw, g_Tuning, pos);
    r += g_Report;
    r += MembersOf(player, "player");
    if (script !is null) r += MembersOf(script, "script");
    // never Reflection::TypeOf(vis): a CSceneVehicleVis is not a CMwNod (its first
    // qword is the entity id, no vtable) and TypeOf dereferences it -- crash 13:40.
    r += MembersOfType("CSceneVehicleVis");
    r += MembersOfType("CSceneVehicleVisState");
    g_Discovery = r;
    WriteSmallFile(StoragePath("discovery.txt"), r);
    return "discovery written (" + r.Length + " chars)";
}

// The car's own links back to the Openplanet world (build 128130, measured
// 2026-09-25 with the hook's pointer as ground truth, see discovery-1.txt):
const uint64 CAR_VIS = 0x1A0;          // car -> its CSceneVehicleVis
const uint64 CAR_POS = 0x12F0;         // vec3, the per-tick copy of the body position (== vis Position)
const uint64 CAR_STATE = 0x1280;       // the physics state block the reactor fields live in
// ...and where the CSmPlayer keeps the car (relative to its reflected `Score`
// member, 0x1068 on this build, the highest-offset named member it has):
const int PLAYER_SLOTS_FROM_SCORE = 0xB0;     // Score+0xB0: 4 vehicle objects, one qword each (0x1118)
const int PLAYER_STATE_FROM_SCORE = -0x228;   // Score-0x228: pointer to car+0x12C8 of the ACTIVE car (0xE40)
const uint64 PLAYER_STATE_TARGET = 0x12C8;
const uint PLAYER_SLOT_COUNT = 4;

/// Does `car` belong to `visRaw` / `pos`? "vis" when car+0x1A0 is the vis
/// nod, "pos" when its copy-out position is the vis position, "" otherwise.
string CarMatchesVis(uint64 car, uint64 visRaw, vec3 pos, bool havePos) {
    string m = "";
    try {
        if (visRaw != 0 && Dev::SafeReadUInt64(car + CAR_VIS) == visRaw) m += "vis";
        if (havePos) {
            vec3 p = Dev::SafeReadVec3(car + CAR_POS);
            if (Math::Abs(p.x - pos.x) < 0.05f && Math::Abs(p.y - pos.y) < 0.05f && Math::Abs(p.z - pos.z) < 0.05f) m += (m.Length > 0 ? "+" : "") + "pos";
        }
    } catch {}
    return m;
}

/// Without the hook: the car-shaped qwords of the CSmPlayer, the named-anchor
/// path (Score+0xB0 slots, Score-0x228 state pointer), and which of them is
/// the player's vehicle right now (its vis / position).
string SignatureScan() {
    auto player = LocalPlayer();
    if (player is null) return "scan: no local player";
    uint64 modBegin = Dev::BaseAddress();
    uint64 modEnd = Dev::BaseAddressEnd();
    uint size = ObjectSize(player, 0x2000);
    if (size > 0xFFF0) size = 0xFFF0;
    uint64 visRaw = 0;
    CSceneVehicleVis@ vis = PlayerVis(player, visRaw);
    vec3 pos = vec3(0, 0, 0);
    bool havePos = false;
    if (vis !is null) { pos = vis.AsyncState.Position; havePos = true; }
    string r = "signature scan of CSmPlayer (0x" + Hex(size) + " bytes), vis " + Text::FormatPointer(visRaw) + "\n";
    array<uint64> found;
    array<uint> where;
    uint64 pick = 0;
    string pickWhy = "";
    for (uint off = 0; off + 8 <= size; off += 8) {
        uint64 q = Dev::GetOffsetUint64(player, uint16(off));
        // the vehicle objects are 8-aligned (the first run's 16-byte test skipped the real car)
        if (!LooksLikePointer(q) || (q & 0x7) != 0) continue;
        if (q >= modBegin && q < modEnd) continue;
        uint64 tuning = 0;
        string why;
        if (!CarSignature(q, tuning, why)) continue;
        string m = CarMatchesVis(q, visRaw, pos, havePos);
        r += "  CSmPlayer+0x" + Hex(off) + " -> " + Text::FormatPointer(q) + " (tuning " + Text::FormatPointer(tuning) + ")" + (m.Length > 0 ? " = the player's vehicle [" + m + "]" : "") + "\n";
        found.InsertLast(q);
        where.InsertLast(off);
        if (m.Length > 0 && pick == 0) { pick = q; pickWhy = "CSmPlayer+0x" + Hex(off) + " [" + m + "]"; }
    }
    r += found.Length + " candidate(s)\n";
    // the named-anchor path
    int score = MemberOffset(player, "Score");
    if (score >= 0) {
        r += "Score at CSmPlayer+0x" + Hex(uint64(score)) + "\n";
        for (uint k = 0; k < PLAYER_SLOT_COUNT; k++) {
            int off = score + PLAYER_SLOTS_FROM_SCORE + int(k) * 8;
            uint64 q = Dev::GetOffsetUint64(player, uint16(off));
            uint64 tuning = 0;
            string why;
            bool sig = CarSignature(q, tuning, why);
            string m = sig ? CarMatchesVis(q, visRaw, pos, havePos) : "";
            r += "  slot " + k + " (Score+0x" + Hex(uint64(PLAYER_SLOTS_FROM_SCORE + int(k) * 8)) + ") = " + Text::FormatPointer(q) + (sig ? " car-shaped" : " (" + why + ")") + (m.Length > 0 ? " = the player's vehicle [" + m + "]" : "") + "\n";
        }
        int soff = score + PLAYER_STATE_FROM_SCORE;
        uint64 sp = Dev::GetOffsetUint64(player, uint16(soff));
        uint64 carFromState = sp - PLAYER_STATE_TARGET;
        uint64 tuning = 0;
        string why;
        bool sig = LooksLikePointer(sp) && CarSignature(carFromState, tuning, why);
        r += "  state pointer (Score-0x" + Hex(uint64(-PLAYER_STATE_FROM_SCORE)) + ") = " + Text::FormatPointer(sp) + " -> car " + Text::FormatPointer(carFromState) + (sig ? " car-shaped" : " (" + why + ")") + (sig ? " [" + CarMatchesVis(carFromState, visRaw, pos, havePos) + "]" : "") + "\n";
        if (sig && pick != 0) r += (carFromState == pick ? "  state pointer AGREES with the vis-matched slot\n" : "  state pointer DISAGREES with the vis-matched slot\n");
    }
    if (pick != 0) {
        if (g_Car == 0) SetCar(pick, "scan: " + pickWhy);
        else if (g_Car != pick) r += "  vis-matched car DIFFERS from the current car " + Text::FormatPointer(g_Car) + "\n";
        else r += "  vis-matched car = the current car\n";
    } else if (found.Length > 0) {
        r += "  no candidate matches the player's vis; nothing chosen\n";
    }
    AppendFile(StoragePath("discovery.txt"), r);
    trace("[ReactorProbe] " + r);
    return r;
}

// Per-frame logging -----------------------------------------------------------

const string LOG_HEADER = "now_ms\tgame_time\trace_time\tstart_time\ttype\tlvl\tcontact\tactivation\tstart\tduration\tvis_lvl\tvis_type\tfinal_timer\tpos_x\tpos_y\tpos_z\tspeed\n";

void Sample() {
    if (g_Car == 0) return;
    g_Type = Dev::ReadUInt32(g_Car + R_TYPE);
    g_Lvl = Dev::ReadUInt32(g_Car + R_LVL);
    uint contact = Dev::ReadUInt32(g_Car + R_CONTACT);
    if (contact != g_Contact) g_ContactFrames++;
    g_Contact = contact;
    g_Activation = Dev::ReadUInt32(g_Car + R_ACTIVATION);
    g_Start = Dev::ReadUInt32(g_Car + R_START);
    g_Duration = Dev::ReadUInt32(g_Car + R_DURATION);
    g_GameTime = GameTime();
    if (!S_Log) return;

    auto player = LocalPlayer();
    CSmScriptPlayer@ script = null;
    CSceneVehicleVis@ vis = null;
    uint64 visRaw = 0;
    if (player !is null) {
        @script = cast<CSmScriptPlayer>(player.ScriptAPI);
        @vis = PlayerVis(player, visRaw);
    }
    vec3 pos = vec3(0, 0, 0);
    float speed = 0.0f;
    g_VisLvl = -1;
    g_VisType = -1;
    g_FinalTimer = 0.0f;
    if (vis !is null) {
        CSceneVehicleVisState@ vs = vis.AsyncState;
        pos = vs.Position;
        speed = vs.FrontSpeed;
        g_VisLvl = int(vs.ReactorBoostLvl);
        g_VisType = int(vs.ReactorBoostType);
        g_FinalTimer = VehicleState::GetReactorFinalTimer(vs);
    }
    int raceTime = script is null ? 0 : script.CurrentRaceTime;
    int startTime = script is null ? 0 : script.StartTime;
    g_Pending.InsertLast("" + Time::Now + "\t" + g_GameTime + "\t" + raceTime + "\t" + startTime
        + "\t" + g_Type + "\t" + g_Lvl + "\t" + g_Contact + "\t" + g_Activation + "\t" + g_Start + "\t" + g_Duration
        + "\t" + g_VisLvl + "\t" + g_VisType + "\t" + F3(g_FinalTimer)
        + "\t" + F3(pos.x) + "\t" + F3(pos.y) + "\t" + F3(pos.z) + "\t" + F3(speed) + "\n");
    g_LogCount++;
}

void Flush() {
    if (g_Pending.Length == 0) return;
    string block = "";
    if (!g_LogHeaderWritten) {
        block += LOG_HEADER;
        g_LogHeaderWritten = true;
    }
    for (uint i = 0; i < g_Pending.Length; i++) block += g_Pending[i];
    g_Pending.RemoveRange(0, g_Pending.Length);
    AppendFile(StoragePath("reactor.log"), block);
}

// Commands ------------------------------------------------------------------

uint64 ParseHex64(const string &in text) {
    string t = text.Trim();
    if (t.StartsWith("0x") || t.StartsWith("0X")) t = t.SubStr(2);
    uint64 v = 0;
    for (uint i = 0; i < t.Length; i++) {
        int c = int(t[i]);
        uint64 d;
        if (c >= 48 && c <= 57) d = uint64(c - 48);
        else if (c >= 97 && c <= 102) d = uint64(c - 97 + 10);
        else if (c >= 65 && c <= 70) d = uint64(c - 65 + 10);
        else return 0;
        v = (v << 4) | d;
    }
    return v;
}

string RunCommand(const string &in verb, const string &in arg) {
    if (verb == "car") {
        uint64 car = ParseHex64(arg);
        if (car < 0x10000) return "car: bad pointer '" + arg + "'";
        SetCar(car, "harness");
        if (g_Car != car) return g_Status;
        return g_Status + "; " + Discover();
    }
    if (verb == "scan") return SignatureScan();
    if (verb == "discover") return Discover();
    if (verb == "drop") { DropCar("command"); return g_Status; }
    if (verb == "log") { S_Log = (arg == "on"); return "log " + (S_Log ? "on" : "off"); }
    if (verb == "flush") { Flush(); return "flushed, " + g_LogCount + " lines so far"; }
    if (verb == "reset") {
        Flush();
        try { IO::Delete(StoragePath("reactor.log")); } catch {}
        g_LogHeaderWritten = false;
        g_LogCount = 0;
        g_ContactFrames = 0;
        return "log reset";
    }
    if (verb == "members") return MembersOfType(arg);
    if (verb == "status") return g_Status;
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
    WriteSmallFile(StoragePath("cmd.seq"), "" + g_CmdSeq);
    g_CmdResult = RunCommand(parts[1], parts.Length > 2 ? parts[2] : "");
    trace("[ReactorProbe] cmd #" + seq + " " + parts[1] + " -> " + g_CmdResult);
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
    s += "  \"in_map\": " + (g_InMap ? "true" : "false") + ",\n";
    s += "  \"car\": \"" + Text::FormatPointer(g_Car) + "\",\n";
    s += "  \"car_source\": \"" + JsonEscape(g_CarSource) + "\",\n";
    s += "  \"tuning\": \"" + Text::FormatPointer(g_Tuning) + "\",\n";
    s += "  \"game_time\": " + g_GameTime + ",\n";
    s += "  \"type\": " + g_Type + ",\n";
    s += "  \"lvl\": " + g_Lvl + ",\n";
    s += "  \"contact\": " + g_Contact + ",\n";
    s += "  \"activation\": " + g_Activation + ",\n";
    s += "  \"start\": " + g_Start + ",\n";
    s += "  \"duration\": " + g_Duration + ",\n";
    s += "  \"vis_lvl\": " + g_VisLvl + ",\n";
    s += "  \"vis_type\": " + g_VisType + ",\n";
    s += "  \"final_timer\": " + F3(g_FinalTimer) + ",\n";
    s += "  \"log_count\": " + g_LogCount + ",\n";
    s += "  \"contact_frames\": " + g_ContactFrames + ",\n";
    s += "  \"cmd_seq\": " + g_CmdSeq + ",\n";
    s += "  \"cmd_result\": \"" + JsonEscape(g_CmdResult) + "\",\n";
    s += "  \"status\": \"" + JsonEscape(g_Status) + "\"\n";
    s += "}\n";
    WriteSmallFile(StoragePath("state.json"), s);
}

// Main loop -----------------------------------------------------------------

void Update(float dt) {
    g_Heartbeat++;
    auto pg = Playground();
    bool inMap = pg !is null && pg.GameTerminals.Length > 0;
    if (!inMap && g_InMap) DropCar("left the playground");
    g_InMap = inMap;
    if (inMap) Sample();

    uint64 now = Time::Now;
    if (now - g_LastFlush >= 500) {
        g_LastFlush = now;
        Flush();
    }
    if (now - g_LastTickMs < 250) return;
    g_LastTickMs = now;
    if (g_Car != 0) {
        uint64 tuning = 0;
        string why;
        if (!CarSignature(g_Car, tuning, why)) DropCar(why);
    }
    PollCommand();
    WriteState();
}

void RenderMenu() {
    if (UI::MenuItem("\\$f80" + Icons::Bolt + "\\$z Reactor Probe", "", S_ShowDebug)) {
        S_ShowDebug = !S_ShowDebug;
    }
}

void RenderInterface() {
    if (!S_ShowDebug) return;
    UI::SetNextWindowSize(520, 260, UI::Cond::FirstUseEver);
    if (UI::Begin("Reactor Probe", S_ShowDebug)) {
        UI::Text("car: " + (g_Car == 0 ? "-" : Text::FormatPointer(g_Car) + " (" + g_CarSource + ")"));
        UI::Text("game time " + g_GameTime + "   type " + g_Type + "   lvl " + g_Lvl);
        UI::Text("contact " + g_Contact + "   activation " + g_Activation + "   start " + g_Start + "   duration " + g_Duration);
        int age = g_GameTime - int(g_Contact);
        UI::Text(g_Car != 0 && g_Contact != 0 && age >= 0 && age <= 20 ? "\\$8f0IN REACTOR CONTACT (" + age + " ms ago)" : "\\$888no contact (" + age + " ms since the last)");
        UI::Text("vis lvl " + g_VisLvl + "   vis type " + g_VisType + "   final timer " + F3(g_FinalTimer));
        UI::Text("log lines " + g_LogCount + "   contact-refresh frames " + g_ContactFrames);
        UI::Separator();
        UI::TextWrapped(g_Status);
        if (UI::Button("Signature scan")) g_CmdResult = SignatureScan();
        UI::SameLine();
        if (UI::Button("Discover")) g_CmdResult = Discover();
    }
    UI::End();
}

void Main() {
    g_Status = "waiting for a car (cmd `car <hex>` or `scan`)";
}

void OnDestroyed() { Flush(); }
void OnDisabled() { Flush(); }
