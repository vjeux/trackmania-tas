// Trackmania Speed Cap
// --------------------
// Raises, or removes, the CarSport's 1000 km/h speed limit -- the "999" the
// speedometer pins at on boost and downhill sections.
//
// Where the limit lives (client build 2026-01-28, git 128130, GameVersion
// 3.3.0, read off the image with tools/asmdig; SPEEDCAP.md has the evidence):
//
//   NSceneVehiclePhy::ComputeForces (Trackmania.exe+0x4427d0) ends every
//   physics tick with
//       tuning = car->Tuning;                         // car+0x88
//       if (dot(v, v) > tuning->MaxSpeed^2 && tuning->MaxSpeed > eps)
//           v *= tuning->MaxSpeed / |v|;              // tuning+0x2f0
//   `car->Tuning` is set at spawn (Trackmania.exe+0x3cbc9f) to
//       tunings.Data[tunings.ActiveIndex]
//   -- the ACTIVE entry of the vehicle physics model's tunings container,
//   the same 28-entry container of dated physics eras HPLTuneDump walked.
//   Each entry is a 0x3778-byte tuning object whose constructor
//   (Trackmania.exe+0x20196b) writes MaxSpeed = 0x438AE38E = 277.7778 m/s =
//   exactly 1000 km/h, beside three neighbours that identify the slot:
//   +0x2f4 = 100, +0x2f8 = 0.3, +0x2fc = 10000.
//
// So the limit is a PARAMETER of the active physics tuning, not a constant in
// the code, and removing it is one float write per tuning object. No code
// patch, no hook, nothing allocated: this plugin only ever writes that one
// field, and only in objects whose three neighbours match.
//
// How the tunings are reached: GlobalCatalog -> chapter "Vehicles" -> article
// "CarSport" -> LoadedNod (CGameItemModel) +0x288 -> vehicle model +0x28 ->
// physics model (class 0x090EA000) +0x18 -> tunings container (class
// 0x090EB000 at +0x28: data +0x18, count +0x20, active index +0x2c).
// HPLTuneDump used exactly this walk on this build.
//
// Every entry that carries the signature is written, not just the active
// one, so a mode that selects another era still sees the new limit. The game
// rebuilds these objects between maps and they come back with the stock
// value, so the field is re-checked a few times a second and re-applied when
// it reverts. The original value is restored on disable and unload.
//
// Automation: state.json (4 Hz) and cmd.txt in the plugin storage folder, the
// same protocol as the Jump Button plugin, so `jumprig captest` can prove the
// cap is gone against the game's own physics state.

const uint16 ITEM_ENTITY_MODEL_OFFSET = 0x288;  // CGameItemModel -> vehicle model
const uint16 VEHICLE_PHY_MODEL_OFFSET = 0x28;   // vehicle model -> physics model
const uint64 TUNINGS_DATA = 0x18;               // container: pointer array
const uint64 TUNINGS_COUNT = 0x20;
const uint64 TUNINGS_CLASS = 0x28;
const uint64 TUNINGS_INDEX = 0x2C;              // the entry the cars use
const uint TUNING_CLASS_ID = 0x090EB000;
const uint64 TUNING_NAME = 0x18;                // MwId of the era
const uint64 TUNING_MAXSPEED = 0x2F0;           // float, metres/second
const uint64 TUNING_SIG_1 = 0x2F4;              // 100.0
const uint64 TUNING_SIG_2 = 0x2F8;              // 0.3
const uint64 TUNING_SIG_3 = 0x2FC;              // 10000.0
const uint SIG_1_BITS = 0x42C80000;
const uint SIG_2_BITS = 0x3E99999A;
const uint SIG_3_BITS = 0x461C4000;
const uint STOCK_BITS = 0x438AE38E;             // 277.7778 m/s = 1000 km/h
const uint64 CAR_TUNING = 0x88;                 // car -> tuning, read by ComputeForces
const uint MAX_TUNINGS = 64;                    // sanity bound on the container count

// "Unlimited" is a big finite number, not infinity: ComputeForces squares it
// every tick, and any other reader of the field should see a sane float.
// 1e6 m/s is 3.6 million km/h; the car cannot get anywhere near it.
const float UNLIMITED_MPS = 1000000.0f;

// OFF by default, deliberately: the render box is shared, and a physics
// change that is silently on for every other driver's recording is a trap.
// Turn it on in the plugin's settings (or `jumprig capcmd unlimited`).
[Setting name="Remove the speed limit" description="On = no limit. Off = the stock 1000 km/h game, or the limit below if it is not 1000."]
bool S_RemoveCap = false;

[Setting name="Speed limit (km/h)" min=1000 max=20000 description="The limit when the switch above is off. 1000 is the stock game."]
float S_LimitKmh = 1000.0f;

[Setting name="Show debug window"]
bool S_ShowDebug = false;

[Setting name="Automation (state + command files)" description="Write state.json and execute cmd.txt in the plugin storage folder."]
bool S_Automation = true;

// Runtime state -----------------------------------------------------------

uint64 g_PhyModelPtr = 0;       // class 0x090EA000 nod, from the catalog
uint64 g_ContainerPtr = 0;      // its tunings container (class 0x090EB000)
uint g_TuningsCount = 0;
uint g_ActiveIndex = 0;
uint64 g_ActivePtr = 0;         // tunings.Data[ActiveIndex]: what the cars read
string g_ActiveName = "";
array<uint64> g_Signed;         // every entry whose +0x2f4..+0x2fc match
bool g_ActiveSigned = false;    // the active entry is among them
bool g_OriginalKnown = false;
float g_Original = 0.0f;        // the value the game had before we touched it
float g_LastWritten = 0.0f;
bool g_WroteSomething = false;
uint g_Applied = 0;             // writes because the setting asked for it
uint g_Reapplied = 0;           // writes because an entry reverted / was rebuilt
uint g_ModelChanges = 0;        // the active tuning object moved
uint64 g_LastPreloadAt = 0;
uint g_Heartbeat = 0;
string g_Status = "starting up";
string g_LastWrite = "";

// Command protocol (see the Jump Button plugin for the reasoning behind the
// persisted watermark: a reload must not re-run the last command).
int g_CmdSeq = -1;
bool g_CmdSeqLoaded = false;
string g_CmdResult = "";

string F1(float v) { return Text::Format("%.1f", v); }
string F4(float v) { return Text::Format("%.4f", v); }
string Hex32(uint v) { return "0x" + Text::Format("%08X", v); }

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

// The original value outlives a plugin reload on disk, so a reload that finds
// OUR value in the field (nothing restored it) still knows what stock was.
void PersistOriginal() { WriteSmallFile(StoragePath("original.txt"), F4(g_Original)); }
bool LoadPersistedOriginal(float &out v) {
    string t = ReadSmallFile(StoragePath("original.txt"));
    if (t.Length == 0) return false;
    v = Text::ParseFloat(t);
    return v > 1.0f;
}

// Resolving the tunings -----------------------------------------------------

/// The CarSport physics model nod, or null with g_Status saying why not.
CMwNod@ ResolvePhyModel() {
    auto catalog = GetApp().GlobalCatalog;
    if (catalog is null) { g_Status = "no GlobalCatalog yet"; return null; }
    uint vehicleChapters = 0;
    string seen = "";
    for (uint i = 0; i < catalog.Chapters.Length; i++) {
        auto chapter = catalog.Chapters[i];
        if (chapter is null) continue;
        // More than one chapter can carry the vehicles name; CarSport may sit
        // in any of them, so every one is scanned before giving up.
        if (!(chapter.IdName == "Vehicles" || chapter.IdName == "#10003")) continue;
        vehicleChapters++;
        for (uint j = 0; j < chapter.Articles.Length; j++) {
            auto article = chapter.Articles[j];
            if (article is null) continue;
            if (string(article.Name) != "CarSport") {
                if (seen.Length < 200) seen += string(article.Name) + " ";
                continue;
            }
            if (article.LoadedNod is null) {
                // Ask once every few seconds, not every tick: Preload is a
                // request to the loader, and the nod appears a little later.
                if (Time::Now - g_LastPreloadAt > 3000) {
                    article.Preload();
                    g_LastPreloadAt = Time::Now;
                }
                g_Status = "CarSport not loaded yet - preload requested";
                return null;
            }
            auto entity = Dev::GetOffsetNod(article.LoadedNod, ITEM_ENTITY_MODEL_OFFSET);
            if (entity is null) { g_Status = "CarSport item has no entity model at +0x288"; return null; }
            auto phy = Dev::GetOffsetNod(entity, VEHICLE_PHY_MODEL_OFFSET);
            if (phy is null) { g_Status = "vehicle model has no physics model at +0x28"; return null; }
            g_PhyModelPtr = Dev::GetOffsetUint64(entity, VEHICLE_PHY_MODEL_OFFSET);
            return phy;
        }
    }
    if (vehicleChapters == 0) g_Status = "no Vehicles chapter in the catalog (yet)";
    else g_Status = "no CarSport article in " + vehicleChapters + " vehicle chapter(s); saw: " + seen;
    return null;
}

bool LooksLikePointer(uint64 p) { return p > 0x10000 && p < 0x00007FFFFFFFFFFF; }

/// The three neighbours of MaxSpeed in one tuning object, bit for bit. This
/// is what stands between a moved field on a new build and a write into
/// somebody else's float.
bool SignatureAt(uint64 tuning) {
    return Dev::ReadUInt32(tuning + TUNING_SIG_1) == SIG_1_BITS
        && Dev::ReadUInt32(tuning + TUNING_SIG_2) == SIG_2_BITS
        && Dev::ReadUInt32(tuning + TUNING_SIG_3) == SIG_3_BITS;
}

/// Find the tunings container and the active entry; fill g_Signed. False,
/// with g_Status, when any link is missing.
bool ResolveTunings() {
    CMwNod@ phy = ResolvePhyModel();
    if (phy is null) return false;

    // The container sits at +0x18 on this build and at +0x20 on the Sep. 30
    // one; the class id at its +0x28 is the check, not the offset.
    uint64 container = 0;
    array<uint16> cands = { 0x18, 0x20 };
    for (uint k = 0; k < cands.Length; k++) {
        uint64 c = Dev::GetOffsetUint64(phy, cands[k]);
        if (!LooksLikePointer(c)) continue;
        if (Dev::ReadUInt32(c + TUNINGS_CLASS) == TUNING_CLASS_ID) { container = c; break; }
    }
    if (container == 0) { g_Status = "physics model has no tunings container of class 0x090EB000 at +0x18/+0x20"; return false; }
    g_ContainerPtr = container;

    uint count = Dev::ReadUInt32(container + TUNINGS_COUNT);
    uint index = Dev::ReadUInt32(container + TUNINGS_INDEX);
    uint64 data = Dev::ReadUInt64(container + TUNINGS_DATA);
    if (count == 0 || count > MAX_TUNINGS || !LooksLikePointer(data)) {
        g_Status = "tunings container looks wrong: count=" + count + " data=" + Text::FormatPointer(data);
        return false;
    }
    if (index >= count) { g_Status = "tunings active index " + index + " out of " + count; return false; }
    g_TuningsCount = count;
    g_ActiveIndex = index;

    uint64 active = Dev::ReadUInt64(data + uint64(index) * 8);
    if (!LooksLikePointer(active)) { g_Status = "active tuning entry is not a pointer"; return false; }
    if (active != g_ActivePtr) {
        if (g_ActivePtr != 0) {
            g_ModelChanges++;
            trace("[SpeedCap] active tuning moved " + Text::FormatPointer(g_ActivePtr) + " -> " + Text::FormatPointer(active));
        }
        g_ActivePtr = active;
        g_ActiveName = MwId(Dev::ReadUInt32(active + TUNING_NAME)).GetName();
    }

    g_Signed.RemoveRange(0, g_Signed.Length);
    for (uint i = 0; i < count; i++) {
        uint64 t = Dev::ReadUInt64(data + uint64(i) * 8);
        if (LooksLikePointer(t) && SignatureAt(t)) g_Signed.InsertLast(t);
    }
    g_ActiveSigned = g_Signed.Find(active) >= 0;
    if (!g_ActiveSigned) {
        string dump = "";
        for (uint64 o = 0x2E0; o < 0x320; o += 4) dump += Text::Format("%08X ", Dev::ReadUInt32(active + o));
        g_Status = "active tuning '" + g_ActiveName + "' (" + index + "/" + count + ") lacks the MaxSpeed signature - nothing written; dwords +0x2e0..: " + dump;
        return false;
    }
    return true;
}

float DesiredMps() {
    if (S_RemoveCap) return UNLIMITED_MPS;
    // 1000 km/h means "the stock game": hand back the exact original bits
    // rather than 1000/3.6 recomputed in float.
    if (Math::Abs(S_LimitKmh - 1000.0f) < 0.01f && g_OriginalKnown) return g_Original;
    return S_LimitKmh / 3.6f;
}

string Describe(float mps) {
    if (mps >= UNLIMITED_MPS) return "unlimited";
    if (g_OriginalKnown && mps == g_Original) return F1(mps * 3.6f) + " km/h (stock)";
    return F1(mps * 3.6f) + " km/h";
}

float ActiveMaxSpeed() {
    if (g_ActivePtr == 0 || !g_ActiveSigned) return 0.0f;
    return Dev::ReadFloat(g_ActivePtr + TUNING_MAXSPEED);
}

/// Look at the tunings, learn the original, write the desired value into
/// every signed entry that does not hold it. A few times a second and on
/// every setting change; idempotent.
void Tick(bool becauseSettingChanged) {
    if (!ResolveTunings()) return;

    uint bits = Dev::ReadUInt32(g_ActivePtr + TUNING_MAXSPEED);
    float current = Dev::ReadFloat(g_ActivePtr + TUNING_MAXSPEED);
    if (!g_OriginalKnown) {
        float persisted;
        if (bits == STOCK_BITS) {
            g_Original = current;
        } else if (LoadPersistedOriginal(persisted)) {
            // Our own value from a previous load, nobody restored it.
            g_Original = persisted;
        } else {
            // Not the constructor default and no record of ours: the game's
            // data set it. Whatever it is, that is what "stock" means here.
            g_Original = current;
            warn("[SpeedCap] MaxSpeed is not the constructor default (" + F4(current)
                + " m/s, " + Hex32(bits) + "); treating it as the original");
        }
        g_OriginalKnown = true;
        PersistOriginal();
    }

    float desired = DesiredMps();
    uint written = 0;
    for (uint i = 0; i < g_Signed.Length; i++) {
        uint64 t = g_Signed[i];
        if (Dev::ReadFloat(t + TUNING_MAXSPEED) == desired) continue;
        Dev::Write(t + TUNING_MAXSPEED, desired);
        if (Dev::ReadFloat(t + TUNING_MAXSPEED) != desired) {
            g_Status = "write did not stick in tuning " + i + ": wanted " + F4(desired);
            warn("[SpeedCap] " + g_Status);
            return;
        }
        written++;
    }
    if (written > 0) {
        g_WroteSomething = true;
        g_LastWritten = desired;
        if (becauseSettingChanged || g_Applied == 0) g_Applied++; else g_Reapplied++;
        g_LastWrite = F4(current) + " -> " + F4(desired) + " m/s in " + written + " of " + g_Signed.Length + " tunings";
        trace("[SpeedCap] MaxSpeed " + g_LastWrite + " (" + (becauseSettingChanged ? "setting" : "re-applied") + ")");
    }
    g_Status = "limit is " + Describe(desired) + " (active tuning '" + g_ActiveName + "', "
        + g_Signed.Length + "/" + g_TuningsCount + " tunings carry the field)";
}

/// Put the game's own value back, in every entry that still holds what WE
/// wrote: a value we do not recognise belongs to someone else.
void Restore() {
    if (!g_WroteSomething || !g_OriginalKnown) return;
    if (!ResolveTunings()) return;
    uint restored = 0;
    for (uint i = 0; i < g_Signed.Length; i++) {
        uint64 t = g_Signed[i];
        if (Dev::ReadFloat(t + TUNING_MAXSPEED) != g_LastWritten) continue;
        Dev::Write(t + TUNING_MAXSPEED, g_Original);
        restored++;
    }
    g_WroteSomething = false;
    g_Status = "restored the original limit (" + F1(g_Original * 3.6f) + " km/h) in " + restored + " tunings";
    trace("[SpeedCap] " + g_Status);
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

/// Does the car the physics is integrating point at the tuning we write?
/// ComputeForces reads its params through car+0x88; the Jump Button plugin
/// publishes the car pointer, and `jumprig captest` hands it to us here.
string CarCheck(const string &in arg) {
    uint64 car = ParseHex64(arg);
    if (car < 0x10000) return "carcheck: bad car pointer '" + arg + "'";
    if (g_ActivePtr == 0) return "carcheck: tunings not resolved yet - " + g_Status;
    uint64 params = Dev::ReadUInt64(car + CAR_TUNING);
    if (!LooksLikePointer(params)) return "carcheck: car+0x88 is not a pointer (" + Text::FormatPointer(params) + ")";
    float carMax = Dev::ReadFloat(params + TUNING_MAXSPEED);
    int slot = g_Signed.Find(params);
    return "carcheck: car->tuning=" + Text::FormatPointer(params) + " active=" + Text::FormatPointer(g_ActivePtr)
        + (params == g_ActivePtr ? " SAME" : (slot >= 0 ? " DIFFERENT (but a signed entry)" : " DIFFERENT"))
        + " tuning.MaxSpeed=" + F4(carMax) + " m/s (" + F1(carMax * 3.6f) + " km/h)";
}

/// Raw look at what the physics reads: the dwords around the MaxSpeed slot
/// of car->tuning, and where that object sits in the tunings array.
string Probe(const string &in arg) {
    uint64 car = ParseHex64(arg);
    if (car < 0x10000) return "probe: bad car pointer '" + arg + "'";
    uint64 params = Dev::ReadUInt64(car + CAR_TUNING);
    if (!LooksLikePointer(params)) return "probe: car+0x88 is not a pointer (" + Text::FormatPointer(params) + ")";
    string dump = "";
    for (uint64 o = 0x2E0; o < 0x320; o += 4) dump += Text::Format("%08X ", Dev::ReadUInt32(params + o));
    uint bits = Dev::ReadUInt32(params + TUNING_MAXSPEED);
    string where = "not in the tunings array";
    if (g_ContainerPtr != 0) {
        uint64 data = Dev::ReadUInt64(g_ContainerPtr + TUNINGS_DATA);
        for (uint i = 0; i < g_TuningsCount; i++) {
            if (Dev::ReadUInt64(data + uint64(i) * 8) == params) { where = "tunings[" + i + "]" + (i == g_ActiveIndex ? " = the active entry" : ""); break; }
        }
    }
    return "probe: car->tuning=" + Text::FormatPointer(params) + " " + where + " name='"
        + MwId(Dev::ReadUInt32(params + TUNING_NAME)).GetName() + "' +0x2f0=" + Hex32(bits)
        + (bits == STOCK_BITS ? " (STOCK 1000 km/h)" : "") + " dwords +0x2e0..: " + dump;
}

string RunCommand(const string &in verb, const string &in arg) {
    if (verb == "unlimited") { S_RemoveCap = true; Tick(true); return g_Status; }
    if (verb == "stock") { S_RemoveCap = false; S_LimitKmh = 1000.0f; Tick(true); return g_Status; }
    if (verb == "cap") {
        float kmh = Text::ParseFloat(arg);
        if (kmh < 1.0f) return "cap: bad value '" + arg + "'";
        S_RemoveCap = false;
        S_LimitKmh = kmh;
        Tick(true);
        return g_Status;
    }
    if (verb == "apply") { Tick(true); return g_Status; }
    if (verb == "status") { Tick(false); return g_Status; }
    if (verb == "carcheck") return CarCheck(arg);
    if (verb == "probe") return Probe(arg);
    if (verb == "restore") { Restore(); return g_Status; }
    if (verb == "reload") { g_ReloadArmed = true; return "reloading on the next frame"; }
    return "unknown command: " + verb;
}

// The developer-mode mtime watcher did not pick up files written from the
// WSL side (2026-09-24), so the plugin reloads ITSELF on request -- armed
// here, performed from Update() on the next frame so the command's result
// is written first. Meta::ReloadPlugin tears this script down, and
// OnDestroyed restores the game's value on the way out.
bool g_ReloadArmed = false;
void ReloadTick() {
    if (!g_ReloadArmed) return;
    g_ReloadArmed = false;
    auto p = Meta::ExecutingPlugin();
    if (p is null) return;
    trace("[SpeedCap] reloading on request");
    Meta::ReloadPlugin(p);
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
    trace("[SpeedCap] cmd #" + seq + " " + parts[1] + " -> " + g_CmdResult);
}

string JsonEscape(const string &in s) {
    string esc = s;
    esc = esc.Replace("\\", "\\\\");
    esc = esc.Replace("\"", "\\\"");
    esc = esc.Replace("\n", " ");
    return esc;
}

void WriteState() {
    float current = ActiveMaxSpeed();
    string s = "{\n";
    s += "  \"heartbeat\": " + g_Heartbeat + ",\n";
    s += "  \"model\": \"" + Text::FormatPointer(g_ActivePtr) + "\",\n";
    s += "  \"model_found\": " + (g_ActivePtr != 0 ? "true" : "false") + ",\n";
    s += "  \"signature_ok\": " + (g_ActiveSigned ? "true" : "false") + ",\n";
    s += "  \"phy_model\": \"" + Text::FormatPointer(g_PhyModelPtr) + "\",\n";
    s += "  \"tunings_count\": " + g_TuningsCount + ",\n";
    s += "  \"tunings_signed\": " + g_Signed.Length + ",\n";
    s += "  \"active_index\": " + g_ActiveIndex + ",\n";
    s += "  \"active_name\": \"" + JsonEscape(g_ActiveName) + "\",\n";
    s += "  \"original_known\": " + (g_OriginalKnown ? "true" : "false") + ",\n";
    s += "  \"original_mps\": " + F4(g_Original) + ",\n";
    s += "  \"current_mps\": " + F4(current) + ",\n";
    s += "  \"current_kmh\": " + F1(current * 3.6f) + ",\n";
    s += "  \"desired_mps\": " + F4(DesiredMps()) + ",\n";
    s += "  \"unlimited\": " + (S_RemoveCap ? "true" : "false") + ",\n";
    s += "  \"limit_kmh\": " + F1(S_LimitKmh) + ",\n";
    s += "  \"applied\": " + g_Applied + ",\n";
    s += "  \"reapplied\": " + g_Reapplied + ",\n";
    s += "  \"model_changes\": " + g_ModelChanges + ",\n";
    s += "  \"cmd_seq\": " + g_CmdSeq + ",\n";
    s += "  \"cmd_result\": \"" + JsonEscape(g_CmdResult) + "\",\n";
    s += "  \"last_write\": \"" + JsonEscape(g_LastWrite) + "\",\n";
    s += "  \"status\": \"" + JsonEscape(g_Status) + "\"\n";
    s += "}\n";
    // A reader holding the file open makes this open throw on Windows; the
    // next write is 250 ms away, so a skipped one costs nothing.
    WriteSmallFile(StoragePath("state.json"), s);
}

// Frame loop ----------------------------------------------------------------

uint64 g_LastTickMs = 0;
const uint64 TICK_PERIOD_MS = 250;   // 4 Hz: a rebuilt tuning gets its value back within a quarter second

void Update(float dt) {
    g_Heartbeat++;
    uint64 now = Time::Now;
    if (now - g_LastTickMs < TICK_PERIOD_MS) return;
    g_LastTickMs = now;
    Tick(false);
    if (!S_Automation) return;
    PollCommand();
    WriteState();
    ReloadTick();
}

void OnSettingsChanged() { Tick(true); }

void RenderMenu() {
    if (UI::MenuItem("\\$f80" + Icons::Rocket + "\\$z Speed Cap", "", S_ShowDebug)) {
        S_ShowDebug = !S_ShowDebug;
    }
}

void RenderInterface() {
    if (!S_ShowDebug) return;
    UI::SetNextWindowSize(460, 280, UI::Cond::FirstUseEver);
    if (UI::Begin("Speed Cap", S_ShowDebug)) {
        UI::Text("active tuning: " + (g_ActivePtr == 0 ? "-" : (g_ActiveName + " " + Text::FormatPointer(g_ActivePtr)))
            + (g_ActiveSigned ? "  \\$8f0signature ok" : "  \\$f40no signature"));
        UI::Text("tunings: " + g_Signed.Length + " of " + g_TuningsCount + " carry the field");
        if (g_OriginalKnown) UI::Text("original limit: " + F1(g_Original * 3.6f) + " km/h (" + F4(g_Original) + " m/s)");
        float current = ActiveMaxSpeed();
        UI::Text("current limit:  " + Describe(current) + " (" + F4(current) + " m/s)");
        UI::Text("writes: " + g_Applied + " asked, " + g_Reapplied + " re-applied, tuning moved " + g_ModelChanges + "x");
        UI::Separator();
        UI::TextWrapped(g_Status);
        UI::Separator();
        if (UI::Button("Unlimited")) { S_RemoveCap = true; Tick(true); }
        UI::SameLine();
        if (UI::Button("Stock 1000 km/h")) { S_RemoveCap = false; S_LimitKmh = 1000.0f; Tick(true); }
        UI::SameLine();
        if (UI::Button("2000 km/h")) { S_RemoveCap = false; S_LimitKmh = 2000.0f; Tick(true); }
    }
    UI::End();
}

void Main() {
    Tick(true);
    if (g_ActivePtr != 0 && !g_ActiveSigned) {
        warn("[SpeedCap] " + g_Status);
        UI::ShowNotification("Speed Cap", "Disabled: this build's physics tuning does not look like the one this plugin knows. Nothing was written.",
            vec4(0.9, 0.5, 0.1, 1), 15000);
        return;
    }
    UI::ShowNotification("Speed Cap", S_RemoveCap ? "Speed limit removed."
        : (Math::Abs(S_LimitKmh - 1000.0f) < 0.01f ? "Stock speed limit (1000 km/h). Enable the plugin's setting to remove it."
                                                    : ("Speed limit set to " + F1(S_LimitKmh) + " km/h.")),
        vec4(0.2, 0.7, 0.3, 1), 6000);
}

void OnDisabled() { Restore(); }
void OnDestroyed() { Restore(); }
