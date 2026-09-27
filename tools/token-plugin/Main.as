// AgentTokenMint — the ONE thing the publishing tools need from a running game: the two
// Nadeo web-service tokens (NadeoServices + NadeoLiveServices) written to this plugin's
// storage folder, once, at load. No HTTP server, no routes, no input, no camera, nothing
// else. It exists so that a token mint never depends on an agent plugin being installed in
// a game vjeux plays (2026-09-27: all agent plugins quarantined from the box on his order);
// `tinyctl` loads it ONLY into a game the tool launched itself, in a free window, and the
// game is closed again by pid when the tokens are on disk.
//
// Output: OpenplanetNext/PluginStorage/AgentTokenMint/token-NadeoServices.txt and
// token-NadeoLiveServices.txt, each the full `Authorization` header value.

void WriteToken(const string &in aud) {
    NadeoServices::AddAudience(aud);
    for (uint i = 0; i < 600 && !NadeoServices::IsAuthenticated(aud); i++) yield();
    if (!NadeoServices::IsAuthenticated(aud)) {
        trace("AgentTokenMint: not authenticated for " + aud);
        return;
    }
    auto req = NadeoServices::Request(aud);
    string h = string(req.Headers["Authorization"]);
    IO::File f(IO::FromStorageFolder("token-" + aud + ".txt"), IO::FileMode::Write);
    f.Write(h);
    f.Close();
    trace("AgentTokenMint: wrote token-" + aud + ".txt (" + h.Length + " chars)");
}

void Main() {
    // the storage folder is created on first write; a stale pair is removed first so a
    // reader never mistakes yesterday's tokens for today's
    // (IO::Delete throws on a missing file — first run, 2026-09-27 15:32)
    if (IO::FileExists(IO::FromStorageFolder("token-NadeoServices.txt"))) IO::Delete(IO::FromStorageFolder("token-NadeoServices.txt"));
    if (IO::FileExists(IO::FromStorageFolder("token-NadeoLiveServices.txt"))) IO::Delete(IO::FromStorageFolder("token-NadeoLiveServices.txt"));
    WriteToken("NadeoServices");
    WriteToken("NadeoLiveServices");
    IO::File done(IO::FromStorageFolder("minted.txt"), IO::FileMode::Write);
    done.Write(Time::FormatString("%Y-%m-%dT%H:%M:%S"));
    done.Close();
}
