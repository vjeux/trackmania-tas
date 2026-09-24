// TokenGate.as — the game refuses commands from a driver that does not hold
// the lock.
//
// WHY THIS IS IN THE GAME
//
// The Rust side (`tmdrive::GameLock`) makes it impossible to *call* a game
// operation without the lock, and a workspace contract test stops a crate from
// shelling out around it. Neither can stop a hand-run curl, a stale script, or
// a tool nobody has converted yet. Those all arrive here, at the plugin's HTTP
// server — so this is the last place the rule can be enforced, and the only
// place that does not depend on the caller cooperating.
//
// HOW
//
// `tmdrive` writes the current holder's token to
//   OpenplanetNext/PluginStorage/TmDriveLock/token.txt
// when it takes the lock, and removes it on release. Every mutating request
// must carry `?token=<that value>`. A request without it, or with a stale one,
// is answered 409 `token-refused` and changes nothing.
//
// READS STAY OPEN. `/ctx`, `/ping`, `/ready`, `/build` and friends are
// side-effect free: gating them would mean every observer needed the lock,
// which in practice means everyone holds it and the lock stops meaning
// anything. Only commands that CHANGE the game are gated.

namespace TokenGate {
    // Routes that change the game. Everything not listed here is read-only and
    // needs no token. Add to this list, never remove without thinking about
    // what the route does.
    const array<string> GUARDED = {
        "/playmap", "/editmap", "/shoot", "/reload", "/quit", "/setup",
        "/camera", "/cursor", "/input", "/key", "/click", "/save", "/saveas",
        "/inventory", "/place", "/delete", "/move", "/nadeotoken", "/publish",
        "/thumb", "/mapsave", "/treeinst", "/meshflags", "/mobils"
    };

    string TokenPath() {
        // The lock record itself, not a mirror of it. tmdrive writes the lock
        // under PluginStorage precisely so this plugin reads the SAME bytes:
        // an earlier design mirrored a token file here, the mirror outlived
        // its lock, and a stale token still opened the game.
        return IO::FromDataFolder("PluginStorage/TmDriveLock/token");
    }

    // The token of whoever currently holds the game lock, or "" if the box is
    // free. Read per request rather than cached: the holder changes while this
    // plugin keeps running, and a cached token would let a released driver
    // carry on driving.
    string CurrentToken() {
        string p = TokenPath();
        if (!IO::FileExists(p)) return "";
        IO::File f(p, IO::FileMode::Read);
        string t = f.ReadToEnd().Trim();
        f.Close();
        return t;
    }

    bool IsGuarded(const string &in route) {
        string path = route;
        int q = path.IndexOf("?");
        if (q >= 0) path = path.SubStr(0, q);
        path = path.ToLower();
        for (uint i = 0; i < GUARDED.Length; i++) {
            if (path == GUARDED[i]) return true;
        }
        return false;
    }

    string QueryToken(const string &in route) {
        int q = route.IndexOf("?");
        if (q < 0) return "";
        auto parts = route.SubStr(q + 1).Split("&");
        for (uint i = 0; i < parts.Length; i++) {
            auto kv = parts[i].Split("=", 2);
            if (kv.Length == 2 && kv[0] == "token") return kv[1].Trim();
        }
        return "";
    }

    // Returns "" to allow, or the reason to refuse.
    string Refuse(const string &in route) {
        if (!IsGuarded(route)) return "";
        string held = CurrentToken();
        if (held.Length == 0) {
            return "token-refused: no session holds the game lock. "
                 + "Take it with `tmdrive` (one game, one driver).";
        }
        string given = QueryToken(route);
        if (given.Length == 0) {
            return "token-refused: this command carries no lock token. "
                 + "Drive the game through tmdrive, which stamps it.";
        }
        if (given != held) {
            // Name the holder's session so the caller can go and ask for it.
            string holder = held;
            int c = holder.IndexOf(":");
            if (c > 0) holder = holder.SubStr(0, c);
            return "token-refused: the game is held by session " + holder
                 + ". Ask for it, or wait for the lock.";
        }
        return "";
    }
}
