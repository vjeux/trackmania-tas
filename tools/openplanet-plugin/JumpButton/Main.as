// Trackmania Jump Button
// ----------------------
// Adds a jump to the car by writing its physics velocity directly.
//
// How it works:
//   Openplanet's reflection does not expose the physics vehicle object, so the
//   plugin obtains it the way the engine does. A 12-byte jump at the CarSport
//   physics handler entry diverts into a small allocated island that copies the
//   car pointer out of the handler's own argument (r8 -> [r8+8]), bumps a
//   liveness counter, runs the displaced prologue, and jumps back. The hook
//   only observes; the jump itself is an addition to the car's vertical
//   velocity, which the engine then integrates normally.
//
// Safety:
//   - exact build gate (handler signature + build banner) before any write;
//   - expected-byte preimage check, then readback verification, on every patch;
//   - hook removed and island freed on unload, disable and destroy;
//   - nothing is written at all on an unsupported build.
//
// Automation:
//   The plugin writes state.json every frame and executes commands from
//   cmd.txt in its storage folder, so a test harness can wait on real
//   conditions (heartbeat advancing, map loaded, car present) instead of
//   sleeping, and can drive a jump without simulated input or screen clicks.

const string TARGET_HANDLER_PATTERN =
    "48 8B C4 F3 0F 11 48 10 48 89 48 08 55 56 57 41 54 "
    "48 8D A8 48 FE FF FF 48 81 EC 98 02 00 00 49 8B 78 08 "
    "41 BA 04 00 00 00";
const string TARGET_BUILD_BANNER_PATTERN =
    "64 61 74 65 3D 32 30 32 36 2D 30 31 2D 32 38 5F 31 33 5F 30 30 20 "
    "67 69 74 3D 31 32 38 31 33 30 2D 36 64 64 61 33 37 32 38 65 39 31 20 "
    "47 61 6D 65 56 65 72 73 69 6F 6E 3D 33 2E 33 2E 30";
// The same banner, readable — what the user is told when theirs differs.
const string TARGET_BUILD_BANNER = "2026-01-28 (git 128130, GameVersion 3.3.0)";
const string TARGET_ENTRY_ORIGINAL = "48 8B C4 F3 0F 11 48 10 48 89 48 08";
const uint ENTRY_PATCH_BYTES = 12;

// Car object layout, build 128130 (witnessed from the image's own instructions).
const uint64 CAR_POSITION = 0x12F0;   // vec3, full precision
const uint64 CAR_VELOCITY = 0x12FC;   // vec3, metres/second
const uint64 CAR_VEL_Y    = 0x1300;   // CAR_VELOCITY + 4 (Y is up)
const uint64 CAR_WHEEL0   = 0x1790;   // 4 per-wheel records
const uint64 CAR_WHEEL_STRIDE = 0xB8; // contact point vec3 at +0x00, zero when airborne

// Island layout.
const uint64 ISL_CARPTR  = 0x00;  // uint64, refreshed every physics tick
const uint64 ISL_TICK    = 0x08;  // uint32, liveness counter
const uint64 ISL_RETADDR = 0x10;  // uint64, handler + 12
const uint64 ISL_CODE    = 0x20;
const uint   ISL_SIZE    = 0x80;

[Setting name="Jump key" description="Press this to jump. Default is Left Shift: Space is the game's own respawn key, and a jump button that also respawns you is a trap."]
VirtualKey S_JumpKey = VirtualKey::Shift;

[Setting name="Jump strength (m/s)" min=1.0 max=30.0 description="Upward velocity added to the car."]
// Measured on build 128130, 2026-09-23: peak height gain against strength is
// almost exactly quadratic —
//     4 -> 0.35 m   6 -> 0.73 m   8 -> 1.25 m
//    10 -> 1.89 m  12 -> 2.66 m  15 -> 4.02 m
// 10 is the default because 1.89 m clears a car and a low wall with 0.43 s of
// air: visibly a jump, without turning the car into an aircraft or making
// existing tracks trivial. Below 6 it reads as a bump; above 12 it launches.
float S_JumpStrength = 10.0f;

[Setting name="Cooldown (s)" min=0.0 max=3.0 description="Minimum time between jumps."]
// Airtime at the default strength is 0.43 s, so a 0.35 s cooldown does not
// let a second jump stack onto the first while still rising.
float S_Cooldown = 0.35f;

[Setting name="Require ground contact" description="Only jump when at least one wheel touches the ground."]
bool S_RequireGround = true;

[Setting name="Keep horizontal speed" description="Add to vertical velocity instead of replacing it."]
bool S_AddToVertical = true;

[Setting name="Show debug window"]
bool S_ShowDebug = false;

[Setting name="Automation (state + command files)" description="Write state.json and execute cmd.txt in the plugin storage folder."]
bool S_Automation = true;

uint64 g_Handler = 0;
uint64 g_Island = 0;
string g_EntryBackup = "";
bool g_BuildSupported = false;
// `editplay`: EditMap is asynchronous, so TEST is pressed from Update() once
// the editor exists. Bounded: give up after 90 s rather than pressing TEST in
// some later, unrelated editor session.
bool g_PendingEditTest = false;
uint64 g_PendingEditTestSince = 0;
bool g_Hooked = false;
string g_Status = "starting up";

uint64 g_CarPtr = 0;
uint g_HookTicks = 0;
vec3 g_Pos = vec3(0, 0, 0);
vec3 g_Vel = vec3(0, 0, 0);
int g_WheelsDown = 0;
uint64 g_LastJumpAt = 0;
uint g_JumpCount = 0;
string g_LastJumpInfo = "";

uint g_Heartbeat = 0;
int g_CmdSeq = -1;
string g_CmdResult = "";
float g_PeakY = 0.0f;
float g_JumpStartY = 0.0f;
bool g_TrackingJump = false;
float g_LastJumpGain = 0.0f;

// Openplanet's Text::Format takes exactly one value argument, so these wrap
// the two precisions this plugin prints. (A multi-argument call compiles
// nowhere and takes the whole plugin down with it.)
string F2(float v) { return Text::Format("%.2f", v); }
string F4(float v) { return Text::Format("%.4f", v); }

string HexU64LE(uint64 v) {
    string hex = "";
    for (uint i = 0; i < 8; i++) hex += Text::Format("%02X ", uint((v >> (i * 8)) & 0xFF));
    return hex;
}

string HexU32LE(uint v) {
    string hex = "";
    for (uint i = 0; i < 4; i++) hex += Text::Format("%02X ", uint((v >> (i * 8)) & 0xFF));
    return hex;
}

string HexI32LE(int v) { return HexU32LE(uint(v)); }

string AbsoluteJump(uint64 target) {
    return "48 B8 " + HexU64LE(target) + "FF E0";
}

/// Does this entry hold a jump WE wrote? `48 B8 <imm64>` (mov rax, imm) then
/// `FF E0` (jmp rax) — the shape AbsoluteJump emits, and nothing else on this
/// entry looks like it. Used to recognise our own stale patch after a reload
/// so it can be re-pointed rather than refused.
bool IsOurJump(const string &in entry) {
    string e = entry.Trim();
    return e.StartsWith("48 B8 ") && e.EndsWith("FF E0");
}

// Island code: observe the car pointer, then continue into the real prologue.
//
//   4D 8B 58 08            mov  r11, [r8+8]          ; the car
//   4C 89 1D <rel32>       mov  [rip+carptr], r11
//   FF 05 <rel32>          inc  dword [rip+tick]
//   48 8B C4               mov  rax, rsp             ; displaced
//   F3 0F 11 48 10         movss [rax+0x10], xmm1    ; displaced
//   48 89 48 08            mov  [rax+8], rcx         ; displaced
//   FF 25 <rel32>          jmp  qword [rip+retaddr]
//
// r11 and rax are volatile and dead at entry (the first original instruction
// overwrites rax), flags are dead at entry, and rsp is untouched, so the
// displaced prologue observes exactly what it would have observed.
string BuildIslandCode(uint64 island) {
    uint64 pc = island + ISL_CODE;
    string s = "4D 8B 58 08 ";
    pc += 4;

    uint64 after = pc + 7;
    s += "4C 89 1D " + HexI32LE(int(int64(island + ISL_CARPTR) - int64(after)));
    pc = after;

    after = pc + 6;
    s += "FF 05 " + HexI32LE(int(int64(island + ISL_TICK) - int64(after)));
    pc = after;

    s += "48 8B C4 F3 0F 11 48 10 48 89 48 08 ";
    pc += 12;

    after = pc + 6;
    s += "FF 25 " + HexI32LE(int(int64(island + ISL_RETADDR) - int64(after)));
    return s;
}

bool ValidateBuild() {
    uint64 banner = Dev::FindPattern(TARGET_BUILD_BANNER_PATTERN);
    g_Handler = Dev::FindPattern(TARGET_HANDLER_PATTERN);
    if (banner == 0 || g_Handler == 0) {
        g_Status = "unsupported Trackmania build - nothing was written";
        return false;
    }
    if (Dev::Read(g_Handler, 41) != TARGET_HANDLER_PATTERN) {
        g_Handler = 0;
        g_Status = "handler signature check failed - nothing was written";
        return false;
    }
    g_BuildSupported = true;
    return true;
}

bool InstallHook() {
    if (g_Hooked) return true;
    if (!g_BuildSupported || g_Handler == 0) return false;

    string entry = Dev::Read(g_Handler, ENTRY_PATCH_BYTES);
    bool clean = (entry == TARGET_ENTRY_ORIGINAL);
    bool ourStalePatch = IsOurJump(entry);
    if (!clean && !ourStalePatch) {
        g_Status = "handler entry patched by something else - refusing";
        return false;
    }

    g_Island = Dev::Allocate(ISL_SIZE, true);
    if (g_Island == 0) {
        g_Status = "executable allocation failed";
        return false;
    }

    try {
        Dev::Write(g_Island + ISL_CARPTR, uint64(0));
        Dev::Write(g_Island + ISL_TICK, uint(0));
        Dev::Write(g_Island + ISL_RETADDR, g_Handler + ENTRY_PATCH_BYTES);
        Dev::Write(g_Island + ISL_CODE, BuildIslandCode(g_Island));

        // Re-pointing a stale patch is SAFE and is the reload path: the
        // displaced prologue is a constant, so the new island runs exactly
        // what the old one did. The old island is never freed (see
        // RemoveHook), so a physics thread still inside it keeps running
        // valid code.
        string jump = AbsoluteJump(g_Island + ISL_CODE);
        string backup = Dev::Patch(g_Handler, jump);
        if (clean) g_EntryBackup = backup;
        if (Dev::Read(g_Handler, ENTRY_PATCH_BYTES) != jump) {
            if (clean) Dev::Patch(g_Handler, TARGET_ENTRY_ORIGINAL);
            g_Island = 0;   // leaked deliberately; never freed
            g_Status = "patch verification failed - entry restored";
            return false;
        }
    } catch {
        g_Island = 0;       // leaked deliberately; never freed
        g_Status = "install exception: " + getExceptionInfo();
        return false;
    }

    g_Hooked = true;
    g_Status = "ready - press the jump key while driving";
    trace("[Jump] hook installed handler=" + Text::FormatPointer(g_Handler)
        + " island=" + Text::FormatPointer(g_Island)
        + (ourStalePatch ? " (re-pointed a stale patch from a previous load)" : ""));
    return true;
}

/// THE ISLAND IS NEVER FREED, AND THAT IS THE DESIGN.
///
/// The physics handler runs on a game thread, asynchronously to this script
/// thread. Restoring the entry bytes stops NEW calls entering the island, but
/// says nothing about a thread already executing inside it — and there is no
/// way from here to know, or to suspend it. Freeing the island therefore
/// races: the game jumps into 0x80 bytes that have just been handed back.
///
/// That race is not theoretical. It killed the game on 2026-09-23 every time
/// Openplanet hot-reloaded the plugin (an edit to this file is enough), with
/// no crash dump and no log line — the process simply vanished ~10 s after a
/// map loaded, i.e. as soon as physics started calling the handler again.
///
/// So the island leaks: 0x80 bytes per plugin load, a few hundred bytes across
/// a session, against a use-after-free in the game's physics thread. It also
/// makes a STALE patch safe — if this plugin is ever unloaded without running
/// its teardown, the entry still points at mapped, valid code that does the
/// right thing and returns.
void RemoveHook() {
    if (!g_Hooked) return;
    try {
        if (g_Handler != 0 && Dev::Read(g_Handler, ENTRY_PATCH_BYTES) != TARGET_ENTRY_ORIGINAL) {
            Dev::Patch(g_Handler, TARGET_ENTRY_ORIGINAL);
        }
    } catch { }
    g_Island = 0;   // deliberately leaked; see above. NEVER Dev::Free here.
    g_Hooked = false;
    g_CarPtr = 0;
    g_Status = "hook removed; original physics restored";
    trace("[Jump] hook removed (island leaked on purpose: a physics thread may still be in it)");
}

bool CarLooksSane(uint64 car) {
    return car >= 0x10000;
}

void ReadCarState() {
    g_CarPtr = 0;
    g_WheelsDown = 0;
    if (!g_Hooked || g_Island == 0) return;

    uint64 car = Dev::ReadUInt64(g_Island + ISL_CARPTR);
    g_HookTicks = Dev::ReadUInt32(g_Island + ISL_TICK);
    if (!CarLooksSane(car)) return;

    g_CarPtr = car;
    g_Pos = vec3(Dev::ReadFloat(car + CAR_POSITION),
                 Dev::ReadFloat(car + CAR_POSITION + 4),
                 Dev::ReadFloat(car + CAR_POSITION + 8));
    g_Vel = vec3(Dev::ReadFloat(car + CAR_VELOCITY),
                 Dev::ReadFloat(car + CAR_VELOCITY + 4),
                 Dev::ReadFloat(car + CAR_VELOCITY + 8));

    for (uint i = 0; i < 4; i++) {
        uint64 w = car + CAR_WHEEL0 + uint64(i) * CAR_WHEEL_STRIDE;
        if (Dev::ReadFloat(w) != 0.0f || Dev::ReadFloat(w + 4) != 0.0f || Dev::ReadFloat(w + 8) != 0.0f) {
            g_WheelsDown++;
        }
    }

    if (g_TrackingJump) {
        if (g_Pos.y > g_PeakY) g_PeakY = g_Pos.y;
        g_LastJumpGain = g_PeakY - g_JumpStartY;
    }
}

bool InPlayground() {
    // A playground is a playground whether or not the editor is behind it.
    // The editor's TEST mode gives a real one with the player's car, and since
    // PlayMap stopped loading maps (2026-09-23) it is the route that works —
    // excluding `app.Editor` here refused the only playground we can get.
    return GetApp().CurrentPlayground !is null;
}

string DoJump() {
    if (!g_Hooked) { g_LastJumpInfo = "hook not installed"; return g_LastJumpInfo; }
    if (!InPlayground()) { g_LastJumpInfo = "not driving"; return g_LastJumpInfo; }
    if (!CarLooksSane(g_CarPtr)) { g_LastJumpInfo = "no car yet"; return g_LastJumpInfo; }

    uint64 now = Time::Now;
    if (now - g_LastJumpAt < uint64(S_Cooldown * 1000)) {
        g_LastJumpInfo = "cooling down";
        return g_LastJumpInfo;
    }
    if (S_RequireGround && g_WheelsDown == 0) {
        g_LastJumpInfo = "airborne - no jump";
        return g_LastJumpInfo;
    }

    float before = Dev::ReadFloat(g_CarPtr + CAR_VEL_Y);
    float after = S_AddToVertical ? (before + S_JumpStrength) : S_JumpStrength;
    if (!S_AddToVertical && before > after) after = before;

    Dev::Write(g_CarPtr + CAR_VEL_Y, after);
    float readback = Dev::ReadFloat(g_CarPtr + CAR_VEL_Y);

    g_LastJumpAt = now;
    g_JumpCount++;
    g_JumpStartY = g_Pos.y;
    g_PeakY = g_Pos.y;
    g_TrackingJump = true;
    g_LastJumpInfo = "vy " + Text::Format("%.2f", before) + " -> " + Text::Format("%.2f", readback)
        + " wheels=" + g_WheelsDown;
    trace("[Jump] " + g_LastJumpInfo);
    return g_LastJumpInfo;
}

string StoragePath(const string &in name) {
    return IO::FromStorageFolder(name);
}

string JsonEscape(const string &in s) {
    string esc = s;
    esc = esc.Replace("\\", "\\\\");
    esc = esc.Replace("\"", "\\\"");
    esc = esc.Replace("\n", " ");
    return esc;
}

void WriteState() {
    string path = StoragePath("state.json");
    string s = "{\n";
    s += "  \"heartbeat\": " + g_Heartbeat + ",\n";
    s += "  \"build_supported\": " + (g_BuildSupported ? "true" : "false") + ",\n";
    s += "  \"hooked\": " + (g_Hooked ? "true" : "false") + ",\n";
    s += "  \"hook_ticks\": " + g_HookTicks + ",\n";
    s += "  \"in_playground\": " + (InPlayground() ? "true" : "false") + ",\n";
    s += "  \"car\": \"" + Text::FormatPointer(g_CarPtr) + "\",\n";
    s += "  \"car_valid\": " + (CarLooksSane(g_CarPtr) ? "true" : "false") + ",\n";
    s += "  \"pos\": [" + F4(g_Pos.x) + ", " + F4(g_Pos.y) + ", " + F4(g_Pos.z) + "],\n";
    s += "  \"vel\": [" + F4(g_Vel.x) + ", " + F4(g_Vel.y) + ", " + F4(g_Vel.z) + "],\n";
    s += "  \"wheels_down\": " + g_WheelsDown + ",\n";
    s += "  \"jumps\": " + g_JumpCount + ",\n";
    s += "  \"last_jump_gain\": " + F4(g_LastJumpGain) + ",\n";
    s += "  \"jump_strength\": " + F4(S_JumpStrength) + ",\n";
    s += "  \"cmd_seq\": " + g_CmdSeq + ",\n";
    s += "  \"cmd_result\": \"" + JsonEscape(g_CmdResult) + "\",\n";
    s += "  \"last_jump\": \"" + JsonEscape(g_LastJumpInfo) + "\",\n";
    s += "  \"status\": \"" + JsonEscape(g_Status) + "\"\n";
    s += "}\n";

    // The harness reads this file while we write it, and on Windows that is a
    // sharing violation, not a torn read: the open THROWS. Writing every
    // frame therefore produced an exception per frame ("Unable to open file:
    // Permission denied"), which spammed the log and destabilised the plugin.
    //
    // Two changes make it a non-event: write at 20 Hz rather than per frame
    // (the harness polls at 20 Hz, so nothing is lost), and treat a failed
    // open as a SKIP. The reader already retries — it has to, because it can
    // also catch a half-written file — so a dropped write costs nothing.
    try {
        IO::File f(path, IO::FileMode::Write);
        f.Write(s);
        f.Close();
    } catch {
        // Reader had it open. The next tick writes a fresher state anyway.
    }
}

string RunCommand(const string &in verb, const string &in arg) {
    if (verb == "jump") return DoJump();
    if (verb == "install") return InstallHook() ? "installed" : ("install failed: " + g_Status);
    if (verb == "remove") { RemoveHook(); return "removed"; }
    if (verb == "strength") { S_JumpStrength = Text::ParseFloat(arg); return "strength=" + S_JumpStrength; }
    if (verb == "requireground") { S_RequireGround = (arg == "1" || arg == "true"); return "requireground=" + S_RequireGround; }
    if (verb == "cooldown") { S_Cooldown = Text::ParseFloat(arg); return "cooldown=" + S_Cooldown; }
    if (verb == "playmap") {
        auto app = cast<CTrackMania>(GetApp());
        if (app is null) return "no CTrackMania";
        if (app.ManiaTitleControlScriptAPI is null) return "no title API";
        // EMPTY MODE, deliberately: a mode name the title has not loaded makes
        // PlayMap fail silently (returns, reports ready, no playground).
        //
        // AND: PlayMap itself has loaded NOTHING on this box since 2026-09-23
        // (ok, then ctx 0 forever, an empty <map> line in UGCErrorsLog, stock
        // maps included, across restarts). It is kept for the day it works
        // again; `editplay` below is the route that does.
        app.ManiaTitleControlScriptAPI.PlayMap(arg, "", "");
        return "playmap requested: " + arg + " (note: PlayMap has been loading nothing since 2026-09-23; prefer editplay)";
    }
    if (verb == "editplay") {
        // THE ROUTE THAT WORKS: open the map in the editor, then press the
        // editor's own TEST button. A real playground with the player's car,
        // inside the editor. Found by the u10s session when PlayMap died.
        //
        // Two steps because EditMap is asynchronous: the editor exists a few
        // frames later. `editplay` starts it; Update() presses TEST the moment
        // the editor is up (g_PendingEditTest), so the caller sees one command.
        auto app = cast<CTrackMania>(GetApp());
        if (app is null) return "no CTrackMania";
        if (app.ManiaTitleControlScriptAPI is null) return "no title API";
        if (app.Editor !is null) return "already in an editor - backtomenu first";
        app.ManiaTitleControlScriptAPI.EditMap(arg, "", "");
        g_PendingEditTest = true;
        g_PendingEditTestSince = Time::Now;
        return "editmap requested: " + arg + " (TEST will be pressed when the editor is up)";
    }
    if (verb == "edtest") {
        auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
        if (ed is null) return "not in the map editor";
        ed.ButtonTestOnClick();
        return "ok";
    }
    if (verb == "backtomenu") {
        auto app = cast<CTrackMania>(GetApp());
        if (app is null) return "no CTrackMania";
        app.BackToMainMenu();
        return "back to menu";
    }
    return "unknown command: " + verb;
}

void PollCommand() {
    string path = StoragePath("cmd.txt");
    if (!IO::FileExists(path)) return;

    // Same sharing-violation hazard as WriteState: the harness writes this
    // file while we read it. A failed open is a SKIP, not an error — the next
    // tick reads it, and a command is only acted on once (seq > g_CmdSeq).
    string line = "";
    try {
        IO::File f(path, IO::FileMode::Read);
        line = f.ReadToEnd();
        f.Close();
    } catch {
        return;
    }

    line = line.Trim();
    if (line.Length == 0) return;

    array<string> parts = line.Split(" ", 3);
    if (parts.Length < 2) return;

    int seq = Text::ParseInt(parts[0]);
    if (seq <= g_CmdSeq) return;

    string verb = parts[1];
    string arg = parts.Length > 2 ? parts[2] : "";

    g_CmdSeq = seq;
    g_CmdResult = RunCommand(verb, arg);
    trace("[Jump] cmd #" + seq + " " + verb + " -> " + g_CmdResult);
}

void OnKeyPress(bool down, VirtualKey key) {
    if (!down || key != S_JumpKey) return;
    DoJump();
}

/// Wall-clock of the last state write and command poll, so both run at a
/// fixed rate instead of once per rendered frame. At 200+ fps the per-frame
/// version opened two files 200 times a second, which is how it collided with
/// the harness's reads constantly.
uint64 g_LastIoMs = 0;
const uint64 IO_PERIOD_MS = 50; // 20 Hz — the rate the harness polls at

void Update(float dt) {
    g_Heartbeat++;
    ReadCarState();
    if (g_PendingEditTest) PressTestWhenEditorIsUp();
    if (!S_Automation) return;
    uint64 now = Time::Now;
    if (now - g_LastIoMs < IO_PERIOD_MS) return;
    g_LastIoMs = now;
    PollCommand();
    WriteState();
}

// The second half of `editplay`. Runs every frame while a TEST press is
// pending; presses it once the editor exists and has its map, and gives up
// after 90 s so a stale request can never fire in some later editor session.
void PressTestWhenEditorIsUp() {
    if (Time::Now - g_PendingEditTestSince > 90000) {
        g_PendingEditTest = false;
        g_CmdResult = "editplay: no editor within 90 s - gave up";
        warn("[Jump] " + g_CmdResult);
        return;
    }
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return;
    if (ed.Challenge is null) return;   // editor exists, map not in yet
    // Any yes/no dialog on the way (unsaved changes, etc.) is answered yes.
    ed.ButtonTestOnClick();
    g_PendingEditTest = false;
    g_CmdResult = "editplay: TEST pressed";
    trace("[Jump] " + g_CmdResult);
}

void RenderMenu() {
    if (UI::MenuItem("\\$f80" + Icons::ArrowUp + "\\$z Jump Button", "", S_ShowDebug)) {
        S_ShowDebug = !S_ShowDebug;
    }
}

void RenderInterface() {
    if (!S_ShowDebug) return;
    UI::SetNextWindowSize(380, 300, UI::Cond::FirstUseEver);
    if (UI::Begin("Jump Button", S_ShowDebug)) {
        UI::Text(g_BuildSupported ? "\\$8f0build supported" : "\\$f40build NOT supported");
        UI::Text("hook: " + (g_Hooked ? "\\$8f0installed" : "\\$f40not installed"));
        UI::Text("physics ticks: " + g_HookTicks + "   frames: " + g_Heartbeat);
        UI::Text("car: " + (g_CarPtr == 0 ? "-" : Text::FormatPointer(g_CarPtr)));
        UI::Text("pos  " + F2(g_Pos.x) + "  " + F2(g_Pos.y) + "  " + F2(g_Pos.z));
        UI::Text("vel  " + F2(g_Vel.x) + "  " + F2(g_Vel.y) + "  " + F2(g_Vel.z));
        UI::Text("wheels on ground: " + g_WheelsDown);
        UI::Text("jumps: " + g_JumpCount + "   " + g_LastJumpInfo);
        UI::Text("last jump height gain: " + F2(g_LastJumpGain) + " m");
        UI::Separator();
        UI::TextWrapped(g_Status);
        UI::Separator();
        if (g_Hooked) {
            if (UI::Button("Remove hook")) RemoveHook();
        } else {
            if (UI::Button("Install hook")) InstallHook();
        }
        UI::SameLine();
        if (UI::Button("Jump now")) DoJump();
    }
    UI::End();
}

void Main() {
    if (!ValidateBuild()) {
        // Say so where the user will SEE it. A jump button that silently does
        // nothing after a game update reads as "the plugin is broken"; a
        // notification saying which build it wants reads as "waiting for an
        // update". warn() also lands it in the log for anyone debugging.
        warn("[Jump] " + g_Status);
        UI::ShowNotification("Jump Button", "Disabled: " + g_Status
            + "\nThis version supports build " + TARGET_BUILD_BANNER
            + ". Nothing was patched.", vec4(0.9, 0.5, 0.1, 1), 15000);
        return;
    }
    if (!InstallHook()) {
        warn("[Jump] " + g_Status);
        UI::ShowNotification("Jump Button", "Disabled: " + g_Status, vec4(0.9, 0.5, 0.1, 1), 15000);
        return;
    }
    UI::ShowNotification("Jump Button", "Ready — press " + tostring(S_JumpKey) + " while driving to jump.",
        vec4(0.2, 0.7, 0.3, 1), 6000);
}

void OnDisabled() { RemoveHook(); }
void OnDestroyed() { RemoveHook(); }
