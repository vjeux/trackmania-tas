// Reactor Contact
// ---------------
// Reads, read-only, whether the car is touching a reactor-granting surface
// RIGHT NOW -- pad, ring or gate -- straight from the physics object the game
// itself updates, for AR's tm_Reactor-Duration plugin. REACTOR.md next to this
// file has the reverse engineering; the summary:
//
//   Client build 2026-01-28 (git 128130, GameVersion 3.3.0). Once per 10 ms
//   physics tick the per-car gameplay step (Trackmania.exe+0x43df50, inside
//   PhysicsStep_TM) calls the reactor update (+0x43d5b0). It asks the contact
//   solver whether the car touches gameplay material 12 (ReactorBoost), 14
//   (ReactorBoost2), 18 (ReactorBoost_Oriented) or 19 (ReactorBoost2_Oriented)
//   and, while any does, writes on the car physics object:
//
//     car+0x13A8  u32  type   1 up / 2 down
//     car+0x13AC  u32  level  1 (12/18) / 2 (14/19)
//     car+0x13B0  u32  GameTime of this tick     <- refreshed EVERY tick in contact
//     car+0x13B4  u32  GameTime of the activation (only when no boost was active)
//     car+0x13BC  u32  duration = tuning.ReactorDuration (6000) / coef
//
//   Past car+0x13B0 + car+0x13BC the type/level are zeroed; the three times
//   and the 6000 stay (AR's "it is not a countdown").
//
// How the car is reached from Openplanet (fail-closed, see Resolve()):
//
//   CSmPlayer + Score.Offset + 0xB0        four vehicle slots, one qword each
//   CSmPlayer + Score.Offset - 0x228       pointer INTO the active car (car+0x12C8)
//   [car+0x88] -> physics tuning           +0x2F4/+0x2F8/+0x2FC = 100/0.3/10000
//                                          and +0x3290 = 6000 (ReactorDuration)
//   [car+0x12F0] vec3                      the car position == ScriptAPI.Position
//
// `Score` is the CSmPlayer's highest-offset reflected member (0x1068 here), so
// the two anchors ride along with the object when Nadeo inserts fields before
// it, the same way XertroV's 2023 `GetOffset(player, "Score") - (0xE60 - 0xC80)`
// did. Every read is checked before it is trusted; when anything is off the
// plugin reports "not resolved" and every export returns false / 0.

const uint64 R_TYPE = 0x13A8;
const uint64 R_LVL = 0x13AC;
const uint64 R_CONTACT = 0x13B0;
const uint64 R_ACTIVATION = 0x13B4;
const uint64 R_DURATION = 0x13BC;
const uint64 CAR_TUNING = 0x88;
const uint64 CAR_POS = 0x12F0;
const uint64 TUNING_SIG_1 = 0x2F4;
const uint64 TUNING_SIG_2 = 0x2F8;
const uint64 TUNING_SIG_3 = 0x2FC;
const uint SIG_1_BITS = 0x42C80000;   // 100.0
const uint SIG_2_BITS = 0x3E99999A;   // 0.3
const uint SIG_3_BITS = 0x461C4000;   // 10000.0
const uint64 TUNING_REACTOR_DURATION = 0x3290;
const uint REACTOR_DURATION_DEFAULT = 6000;
const int SLOTS_FROM_SCORE = 0xB0;
const int STATE_FROM_SCORE = -0x228;
const uint64 STATE_INTO_CAR = 0x12C8;
const uint SLOT_COUNT = 4;
// A render frame sees the last completed tick; GameTime runs ahead of the
// tick's stamp by 0..9 ms (measured), so two ticks of slack say "touching".
const int TOUCH_WINDOW_MS = 20;
const uint64 REVALIDATE_MS = 1000;

[Setting name="Show the contact window"]
bool S_ShowWindow = true;

// One resolved car per CSmPlayer, re-checked every second.
class Resolved {
    uint playerId;
    uint64 car;
    uint64 tuning;
    uint64 checkedAt;
    string status;
    bool ok;
}
array<Resolved@> g_Cache;
string g_LocalStatus = "no playground";
bool g_AutomationMarker = false;
uint64 g_LastStateWrite = 0;
uint g_Heartbeat = 0;

bool LooksLikePointer(uint64 p) { return p > 0x10000 && p < 0x00007FFFFFFFFFFF; }

int GameTime() {
    auto app = cast<CTrackMania>(GetApp());
    if (app is null || app.Network is null || app.Network.PlaygroundClientScriptAPI is null) return 0;
    return app.Network.PlaygroundClientScriptAPI.GameTime;
}

/// The physics object test: +0x88 is a tuning with the MaxSpeed neighbours and
/// the 6000 ms reactor duration. False on any unreadable or odd byte.
bool CarSignature(uint64 car, uint64 &out tuning, string &out why) {
    tuning = 0;
    if (!LooksLikePointer(car) || (car & 0x7) != 0) { why = "not an aligned pointer"; return false; }
    try {
        uint64 t = Dev::SafeReadUInt64(car + CAR_TUNING);
        if (!LooksLikePointer(t)) { why = "car+0x88 is not a pointer"; return false; }
        if (Dev::SafeReadUInt32(t + TUNING_SIG_1) != SIG_1_BITS
            || Dev::SafeReadUInt32(t + TUNING_SIG_2) != SIG_2_BITS
            || Dev::SafeReadUInt32(t + TUNING_SIG_3) != SIG_3_BITS) { why = "tuning lacks the MaxSpeed signature"; return false; }
        uint dur = Dev::SafeReadUInt32(t + TUNING_REACTOR_DURATION);
        if (dur != REACTOR_DURATION_DEFAULT) { why = "tuning ReactorDuration is " + dur; return false; }
        tuning = t;
    } catch {
        why = "unreadable memory";
        return false;
    }
    why = "ok";
    return true;
}

/// Both anchors, checked against each other and against the player's position.
bool Resolve(CSmPlayer@ player, uint64 &out car, uint64 &out tuning, string &out status) {
    car = 0;
    tuning = 0;
    auto ti = Reflection::TypeOf(player);
    if (ti is null) { status = "no type info for the player"; return false; }
    auto scoreMember = ti.GetMember("Score");
    if (scoreMember is null) { status = "CSmPlayer has no Score member"; return false; }
    int score = int(scoreMember.Offset);
    if (score < 0x400 || score + SLOTS_FROM_SCORE + int(SLOT_COUNT) * 8 > 0xBEA0) { status = "Score offset 0x" + Text::Format("%X", score) + " is not where this build keeps it"; return false; }

    // the state pointer names the active car
    uint64 sp = Dev::GetOffsetUint64(player, uint16(score + STATE_FROM_SCORE));
    if (!LooksLikePointer(sp) || sp < STATE_INTO_CAR) { status = "state pointer (Score-0x228) is empty"; return false; }
    uint64 fromState = sp - STATE_INTO_CAR;

    // ...and must be one of the four vehicle slots
    int slot = -1;
    for (uint k = 0; k < SLOT_COUNT; k++) {
        uint64 q = Dev::GetOffsetUint64(player, uint16(score + SLOTS_FROM_SCORE + int(k) * 8));
        if (q == fromState) slot = int(k);
    }
    if (slot < 0) { status = "state pointer does not name one of the vehicle slots"; return false; }

    string why;
    if (!CarSignature(fromState, tuning, why)) { status = "slot " + slot + ": " + why; return false; }

    // ...and sit where the script API says the player is (when it says anything)
    auto script = cast<CSmScriptPlayer>(player.ScriptAPI);
    if (script !is null) {
        vec3 p = script.Position;
        if (p.LengthSquared() > 1.0f) {
            vec3 c;
            try {
                c = Dev::SafeReadVec3(fromState + CAR_POS);
            } catch {
                status = "car position unreadable";
                return false;
            }
            if ((c - p).LengthSquared() > 4.0f) { status = "slot " + slot + " is not where the player is (" + Text::Format("%.1f", (c - p).Length()) + " m off)"; return false; }
        }
    }
    car = fromState;
    status = "slot " + slot + " " + Text::FormatPointer(car) + " (Score at +0x" + Text::Format("%X", score) + ")";
    return true;
}

Resolved@ Lookup(CSmPlayer@ player) {
    if (player is null) return null;
    uint id = player.Id.Value;
    Resolved@ r = null;
    for (uint i = 0; i < g_Cache.Length; i++) if (g_Cache[i].playerId == id) @r = g_Cache[i];
    if (r is null) {
        @r = Resolved();
        r.playerId = id;
        r.checkedAt = 0;
        g_Cache.InsertLast(r);
    }
    uint64 now = Time::Now;
    if (now - r.checkedAt >= REVALIDATE_MS || (!r.ok && now - r.checkedAt >= 250)) {
        r.checkedAt = now;
        uint64 car, tuning;
        string status;
        bool ok = Resolve(player, car, tuning, status);
        if (ok != r.ok || car != r.car) trace("[ReactorContact] " + (ok ? "resolved " : "not resolved: ") + status);
        r.ok = ok;
        r.car = ok ? car : 0;
        r.tuning = ok ? tuning : 0;
        r.status = status;
    }
    return r;
}

uint Field(CSmPlayer@ player, uint64 off) {
    auto r = Lookup(player);
    if (r is null || !r.ok) return 0;
    return Dev::ReadUInt32(r.car + off);
}

// Exports ---------------------------------------------------------------------

bool IsTouching(CSmPlayer@ player) {
    uint contact = Field(player, R_CONTACT);
    if (contact == 0) return false;
    int age = GameTime() - int(contact);
    return age >= 0 && age <= TOUCH_WINDOW_MS;
}
uint LastContactTime(CSmPlayer@ player) { return Field(player, R_CONTACT); }
uint ActivationTime(CSmPlayer@ player) { return Field(player, R_ACTIVATION); }
uint Duration(CSmPlayer@ player) { return Field(player, R_DURATION); }
uint Level(CSmPlayer@ player) { return Field(player, R_LVL); }
uint Type(CSmPlayer@ player) { return Field(player, R_TYPE); }
bool IsResolved(CSmPlayer@ player) {
    auto r = Lookup(player);
    return r !is null && r.ok;
}
string Status() { return g_LocalStatus; }

// The local player --------------------------------------------------------------

CSmPlayer@ LocalPlayer() {
    auto app = cast<CTrackMania>(GetApp());
    if (app is null) return null;
    auto pg = cast<CSmArenaClient>(app.CurrentPlayground);
    if (pg is null || pg.GameTerminals.Length == 0) return null;
    auto term = pg.GameTerminals[0];
    CSmPlayer@ pl = cast<CSmPlayer>(term.ControlledPlayer);
    if (pl is null) @pl = cast<CSmPlayer>(term.GUIPlayer);
    return pl;
}

string StoragePath(const string &in name) { return IO::FromStorageFolder(name); }

void WriteState(CSmPlayer@ player) {
    Resolved@ r = null;
    if (player !is null) @r = Lookup(player);
    bool ok = r !is null && r.ok;
    string s = "{\n";
    s += "  \"heartbeat\": " + g_Heartbeat + ",\n";
    s += "  \"resolved\": " + (ok ? "true" : "false") + ",\n";
    s += "  \"car\": \"" + Text::FormatPointer(ok ? r.car : 0) + "\",\n";
    s += "  \"game_time\": " + GameTime() + ",\n";
    s += "  \"touching\": " + (IsTouching(player) ? "true" : "false") + ",\n";
    s += "  \"contact\": " + LastContactTime(player) + ",\n";
    s += "  \"activation\": " + ActivationTime(player) + ",\n";
    s += "  \"duration\": " + Duration(player) + ",\n";
    s += "  \"level\": " + Level(player) + ",\n";
    s += "  \"type\": " + Type(player) + ",\n";
    s += "  \"status\": \"" + g_LocalStatus.Replace("\"", "'") + "\"\n";
    s += "}\n";
    try {
        IO::File f(StoragePath("state.json"), IO::FileMode::Write);
        f.Write(s);
        f.Close();
    } catch {}
}

void Update(float dt) {
    g_Heartbeat++;
    auto player = LocalPlayer();
    if (player is null) {
        g_LocalStatus = "no local player";
        if (g_Cache.Length > 0) g_Cache.RemoveRange(0, g_Cache.Length);
    } else {
        auto r = Lookup(player);
        g_LocalStatus = r.ok ? "resolved: " + r.status : "not resolved: " + r.status;
    }
    if (g_AutomationMarker && Time::Now - g_LastStateWrite >= 100) {
        g_LastStateWrite = Time::Now;
        WriteState(player);
    }
}

void RenderMenu() {
    if (UI::MenuItem("\\$fa0" + Icons::Bolt + "\\$z Reactor Contact", "", S_ShowWindow)) S_ShowWindow = !S_ShowWindow;
}

void RenderInterface() {
    if (!S_ShowWindow) return;
    auto player = LocalPlayer();
    UI::SetNextWindowSize(420, 170, UI::Cond::FirstUseEver);
    if (UI::Begin("Reactor Contact", S_ShowWindow)) {
        if (player is null) {
            UI::Text("\\$888no local player");
        } else if (!IsResolved(player)) {
            UI::TextWrapped("\\$f80not resolved\\$z " + g_LocalStatus);
        } else {
            int now = GameTime();
            uint contact = LastContactTime(player);
            uint lvl = Level(player);
            if (IsTouching(player)) UI::Text("\\$0f0" + Icons::Bolt + " IN REACTOR CONTACT\\$z  level " + lvl);
            else if (contact == 0) UI::Text("\\$888no reactor touched yet");
            else UI::Text("\\$888not touching\\$z  (last contact " + (now - int(contact)) + " ms ago)");
            if (lvl != 0) {
                int remaining = int(contact) + int(Duration(player)) - now;
                UI::Text("boost active: level " + lvl + ", type " + Type(player) + ", " + Text::Format("%.2f", Math::Max(0, remaining) / 1000.0f) + " s left");
            } else {
                UI::Text("boost: none");
            }
            UI::Text("last contact " + contact + "  activation " + ActivationTime(player) + "  duration " + Duration(player) + "  GameTime " + now);
            UI::Separator();
            UI::TextWrapped("\\$888" + g_LocalStatus);
        }
    }
    UI::End();
}

void Main() {
    g_AutomationMarker = IO::FileExists(StoragePath("automation.on"));
}
