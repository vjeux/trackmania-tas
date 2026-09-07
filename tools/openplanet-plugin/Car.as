// Car.as -- the LIVE vehicle, sampled per frame.
//
// Why this exists: the retail client's /validatepath is the only Rally-physics
// oracle anyone has on Haystack 6, and it will not carry a car through this
// map's vehicle gate -- same item, same point, model CPBoxFW finishes at 4.660
// and GateGameplayRally4m does not finish at any declared time, on the client
// and on the dedicated server alike. The PLAYGROUND does give a CarRally at
// this map's spawn. So the Rally numbers have to be read out of a running game
// instead of out of a validated replay, and this is that readout.

CSmScriptPlayer@ ScriptPlayer() {
    auto app = cast<CTrackMania>(GetApp());
    if (app is null) return null;
    auto pg = cast<CSmArenaClient>(app.CurrentPlayground);
    if (pg is null) return null;
    if (pg.GameTerminals.Length == 0) return null;
    auto term = pg.GameTerminals[0];
    CSmPlayer@ pl = cast<CSmPlayer>(term.GUIPlayer);
    if (pl is null) @pl = cast<CSmPlayer>(term.ControlledPlayer);
    if (pl is null) return null;
    return cast<CSmScriptPlayer>(pl.ScriptAPI);
}

string CarRow(CSmScriptPlayer@ s) {
    return "" + Time::Now + "\t" + s.CurrentRaceTime
        + "\t" + s.Position.x + "\t" + s.Position.y + "\t" + s.Position.z
        + "\t" + s.Velocity.x + "\t" + s.Velocity.y + "\t" + s.Velocity.z
        + "\t" + s.Speed + "\t" + s.RaceWaypointTimes.Length;
}

string CarState() {
    auto s = ScriptPlayer();
    if (s is null) return "err: no script player";
    return CarRow(s) + "\n";
}

// One HTTP call, one whole trajectory. The bridge to this box costs a round
// trip per dispatch, so sampling from outside at 20 Hz is not a thing that can
// be done; yield() hands the frame back to the game exactly as Await does.
string CarLog(int ms) {
    if (ms <= 0) ms = 3000;
    if (ms > 30000) ms = 30000;
    uint t0 = Time::Now;
    string sb = "wall_ms\tt_ms\tx\ty\tz\tvx\tvy\tvz\tspeed\tcp\n";
    while (int(Time::Now - t0) < ms) {
        auto p = ScriptPlayer();
        if (p is null) {
            sb += "# no script player\n";
            break;
        }
        sb += CarRow(p) + "\n";
        yield();
    }
    return sb;
}

// The LIVE camera per frame, next to the car: where the MediaTracker put it.
// This is the readout behind the tiny campaign's camera check — the tiny
// map's intro camera at time t must be the original's at t through the
// items' transform — and behind the in-game trigger test (the camera jumps
// when the car enters a trigger). Same shape as CarLog: one HTTP call, one
// whole trajectory, a row per frame. A null player (the intro, the loading
// screen) leaves the car columns empty rather than ending the log.
//
// The camera is the viewport's first CHmsCamera (`NextLocation` iso4, `Fov`);
// there is no `Camera::` namespace in this Openplanet (1.29.14) — a first
// version referenced Camera::GetCurrentPosition, failed to compile, and took
// the HTTP server down for every driver on the box for five minutes
// (2026-09-07 11:41). Members here are all in OpenplanetNext.json, which is
// what the linter checks.
string CamLog(int ms) {
    if (ms <= 0) ms = 3000;
    if (ms > 30000) ms = 30000;
    uint t0 = Time::Now;
    auto app = GetApp();
    string sb = "wall_ms\tt_ms\tpx\tpy\tpz\tcx\tcy\tcz\tfov\n";
    while (int(Time::Now - t0) < ms) {
        string cam = "\t\t\t";
        auto vp = app.Viewport;
        if (vp !is null && vp.Cameras.Length > 0) {
            auto c = vp.Cameras[0];
            if (c !is null) cam = "" + c.NextLocation.tx + "\t" + c.NextLocation.ty + "\t" + c.NextLocation.tz + "\t" + c.Fov;
        }
        auto p = ScriptPlayer();
        string car = (p is null) ? "\t\t\t" : ("" + p.CurrentRaceTime + "\t" + p.Position.x + "\t" + p.Position.y + "\t" + p.Position.z);
        sb += "" + Time::Now + "\t" + car + "\t" + cam + "\n";
        yield();
    }
    return sb;
}

// WHAT THE PHYSICS FEELS UNDER EACH WHEEL, per frame. The surface question
// of the tiny campaign (2026-09-07): a road item baked from a Nadeo prefab
// carries the prefab's own collision ids — but does the ENGINE read them the
// same way? The only honest readout is the live vehicle state: its four
// `*GroundContactMaterial` fields are the EPlugSurfaceMaterialId the physics
// resolved for each wheel this frame (the enum is the pack's table: 16
// Asphalt, 9 Rubber, 2 Grass, 6 Dirt …), next to the speed and the pedals, so
// an acceleration trace on the original and on the tiny build can be laid
// side by side with the surface each wheel was on.
//
// `VehicleState::ViewingPlayerState()` is the bundled VehicleState plugin's
// export (Openplanet/Plugins/VehicleState/Export.as; info.toml lists the
// dependency). It is null before the vehicle exists (intro, loading), which
// leaves a comment row rather than ending the log.
string WheelRow(CSceneVehicleVisState@ vs, CSmScriptPlayer@ s) {
    string race = (s is null) ? "" : ("" + s.CurrentRaceTime);
    return "" + Time::Now + "\t" + race
        + "\t" + vs.Position.x + "\t" + vs.Position.y + "\t" + vs.Position.z
        + "\t" + vs.WorldVel.x + "\t" + vs.WorldVel.y + "\t" + vs.WorldVel.z
        + "\t" + vs.FrontSpeed
        + "\t" + vs.InputGasPedal + "\t" + vs.InputBrakePedal + "\t" + vs.InputSteer
        + "\t" + (vs.IsGroundContact ? 1 : 0)
        + "\t" + int(vs.FLGroundContactMaterial) + "\t" + int(vs.FRGroundContactMaterial)
        + "\t" + int(vs.RLGroundContactMaterial) + "\t" + int(vs.RRGroundContactMaterial)
        + "\t" + vs.FLSlipCoef + "\t" + vs.FRSlipCoef + "\t" + vs.RLSlipCoef + "\t" + vs.RRSlipCoef
        + "\t" + vs.FLDamperLen + "\t" + vs.FRDamperLen + "\t" + vs.RLDamperLen + "\t" + vs.RRDamperLen
        + "\t" + vs.CurGear + "\t" + vs.GroundDist;
}

string WheelHeader() {
    return "wall_ms\tt_ms\tx\ty\tz\tvx\tvy\tvz\tfrontspeed\tgas\tbrake\tsteer\tground\tfl\tfr\trl\trr\tflslip\tfrslip\trlslip\trrslip\tfldamp\tfrdamp\trldamp\trrdamp\tgear\tgrounddist\n";
}

string WheelState() {
    CSceneVehicleVisState@ vs = VehicleState::ViewingPlayerState();
    if (vs is null) return "err: no vehicle state";
    return WheelHeader() + WheelRow(vs, ScriptPlayer()) + "\n";
}

string WheelLog(int ms) {
    if (ms <= 0) ms = 3000;
    if (ms > 30000) ms = 30000;
    uint t0 = Time::Now;
    string sb = WheelHeader();
    while (int(Time::Now - t0) < ms) {
        CSceneVehicleVisState@ vs = VehicleState::ViewingPlayerState();
        if (vs is null) {
            sb += "# no vehicle state\n";
        } else {
            sb += WheelRow(vs, ScriptPlayer()) + "\n";
        }
        yield();
    }
    return sb;
}
