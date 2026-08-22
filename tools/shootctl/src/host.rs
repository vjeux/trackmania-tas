// Where the plugin's HTTP server lives, as seen FROM THIS PROCESS.
//
// THE WSL TRAP, which cost an afternoon in disguise: the game and its plugin
// run on Windows, so the server is on the WINDOWS loopback. The old pipeline
// reached it with `curl.exe`, a Windows binary, for which 127.0.0.1 is the
// right address. A Linux binary running in WSL has its OWN loopback, and
// 127.0.0.1 there is a different machine -- connection refused, while the shell
// one line earlier reported the plugin healthy.
//
// Candidates, in order, first one that answers wins:
//   $SHOOT_HOST          explicit override
//   127.0.0.1            native Windows, or WSL with mirrored networking
//   the resolv.conf nameserver   WSL2's default NAT host address
//   the default gateway          the same host under some WSL configs
pub fn plugin_addrs() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(h) = std::env::var("SHOOT_HOST") {
        if !h.trim().is_empty() {
            out.push(format!("{}:29800", h.trim()));
        }
    }
    out.push("127.0.0.1:29800".to_string());
    if let Ok(rc) = std::fs::read_to_string("/etc/resolv.conf") {
        for line in rc.lines() {
            if let Some(ip) = line.strip_prefix("nameserver ") {
                out.push(format!("{}:29800", ip.trim()));
            }
        }
    }
    if let Ok(rt) = std::fs::read_to_string("/proc/net/route") {
        for line in rt.lines().skip(1) {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() > 2 && f[1] == "00000000" {
                if let Ok(v) = u32::from_str_radix(f[2], 16) {
                    let b = v.to_le_bytes();
                    // /proc/net/route stores the gateway as a little-endian hex u32, so the
                    // bytes are already in network order: 0x010012AC -> 172.18.0.1.
                    // Reversing them yielded 1.0.18.172 and a connection refused that
                    // looked exactly like "the plugin is down".
                    out.push(format!("{}.{}.{}.{}:29800", b[0], b[1], b[2], b[3]));
                }
            }
        }
    }
    out
}
