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
