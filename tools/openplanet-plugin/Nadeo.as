// Base URLs: NadeoServices::BaseURLCore() / BaseURLLive().
// Nadeo.as -- authenticated Nadeo API access using the GAME's own session.
//
// WHY: the public mirrors (trackmania.io, TMX) are caches and can be blind to a
// record for hours. Nadeo's own services answer immediately but require
// authentication, and the logged-in game already holds it. Openplanet exposes
// that session through NadeoServices, so the game can ask on our behalf.
//
// Routes:
//   /nadeoauth?aud=NadeoLiveServices  -> ensure an audience, report readiness
//   /nadeoget?aud=..&url=<escaped>    -> authenticated GET, body returned raw
//
// The URL is read from arg.txt (PathArg) when url= is absent, for the same
// reason paths are: query strings mangle punctuation.

string NadeoAuth(const string &in audIn) {
    string aud = audIn == "" ? "NadeoLiveServices" : audIn;
    NadeoServices::AddAudience(aud);
    if (!NadeoServices::IsAuthenticated(aud)) return "pending " + aud;
    return "ready " + aud;
}

string NadeoGet(const string &in audIn, const string &in urlIn) {
    string aud = audIn == "" ? "NadeoLiveServices" : audIn;
    string url = urlIn == "" ? PathArg() : urlIn;
    if (url == "") return "ERR no url";
    NadeoServices::AddAudience(aud);
    // Bounded wait: the audience is granted asynchronously after login.
    for (uint i = 0; i < 200 && !NadeoServices::IsAuthenticated(aud); i++) yield();
    if (!NadeoServices::IsAuthenticated(aud)) return "ERR not authenticated for " + aud;

    auto req = NadeoServices::Get(aud, url);
    req.Start();
    while (!req.Finished()) yield();
    return "HTTP " + req.ResponseCode() + "\n" + req.String();
}

// Resolve a login (from the in-game record row) to an account id, and back.
string NadeoWho(const string &in login) {
    string l = login == "" ? PathArg() : login;
    if (l == "") return "ERR no login";
    NadeoServices::AddAudience("NadeoServices");
    for (uint i = 0; i < 200 && !NadeoServices::IsAuthenticated("NadeoServices"); i++) yield();
    string acc = NadeoServices::LoginToAccountId(l);
    string name = acc == "" ? "" : NadeoServices::GetDisplayNameAsync(acc);
    return "login=" + l + " account=" + acc + " name=" + name;
}

// Resolve a DISPLAY NAME to an account id via the core service, which is what
// the in-game leaderboard row shows. LoginToAccountId takes a LOGIN and throws
// on a display name, which is the error this replaces.
string NadeoName(const string &in nameIn) {
    string n = nameIn == "" ? PathArg() : nameIn;
    if (n == "") return "ERR no name";
    NadeoServices::AddAudience("NadeoServices");
    for (uint i = 0; i < 200 && !NadeoServices::IsAuthenticated("NadeoServices"); i++) yield();
    auto req = NadeoServices::Get("NadeoServices",
        NadeoServices::BaseURLCore() + "/accounts/displayNames/?displayNameList[]=" + n);
    req.Start();
    while (!req.Finished()) yield();
    return "HTTP " + req.ResponseCode() + "\n" + req.String();
}

// Authenticated POST/PUT/DELETE. arg.txt line 1 = url, rest = JSON body
// (query strings mangle punctuation, and a body never fits one anyway).
string NadeoPost(const string &in audIn, const string &in methodIn) {
    string aud = audIn == "" ? "NadeoLiveServices" : audIn;
    string all = PathArg();
    int nl = all.IndexOf("\n");
    string url = nl < 0 ? all : all.SubStr(0, nl);
    string body = nl < 0 ? "" : all.SubStr(nl + 1);
    if (url == "") return "ERR no url";
    NadeoServices::AddAudience(aud);
    for (uint i = 0; i < 200 && !NadeoServices::IsAuthenticated(aud); i++) yield();
    if (!NadeoServices::IsAuthenticated(aud)) return "ERR not authenticated for " + aud;
    Net::HttpRequest@ req;
    string m = methodIn == "" ? "post" : methodIn;
    if (m == "put") @req = NadeoServices::Put(aud, url, body);
    else if (m == "delete") @req = NadeoServices::Delete(aud, url);
    else @req = NadeoServices::Post(aud, url, body);
    req.Start();
    while (!req.Finished()) yield();
    return "HTTP " + req.ResponseCode() + "\n" + req.String();
}

// Write the audience token to PluginStorage/token-<aud>.txt so a shell on the
// box can drive multipart uploads with curl (Openplanet strings cannot carry a
// binary body safely). The token never goes over the HTTP reply.
string NadeoToken(const string &in audIn) {
    string aud = audIn == "" ? "NadeoServices" : audIn;
    NadeoServices::AddAudience(aud);
    for (uint i = 0; i < 200 && !NadeoServices::IsAuthenticated(aud); i++) yield();
    if (!NadeoServices::IsAuthenticated(aud)) return "ERR not authenticated for " + aud;
    auto req = NadeoServices::Request(aud);
    string h = string(req.Headers["Authorization"]);
    IO::File f(IO::FromStorageFolder("token-" + aud + ".txt"), IO::FileMode::Write);
    f.Write(h);
    f.Close();
    return "wrote token-" + aud + ".txt (" + h.Length + " chars)";
}
