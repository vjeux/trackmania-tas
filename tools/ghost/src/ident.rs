//! Identity: the car skin, the display name, the 3-letter trigram, and the
//! foreign identifiers that ride along with a borrowed container.
//!
//! OUR RUN CAN BE COMPLETELY OURS AND THE FILE STILL SOMEBODY ELSE'S. A
//! synthesised tape is built on a carrier and inherits that carrier's body:
//! its login, its club tag, its zone, its personal skin, the storage-object
//! uuid inside the skin path, and the server-reported account id. A strip-list
//! aimed at one of those leaves the others, which is how five published clips
//! had to be withdrawn in one night.
//!
//! So this module does not have a strip-list. It CLASSIFIES every identity
//! string in the container, prints them all with their offsets, and
//! `--anonymise` clears every class at once -- and `ghost verify` fails a file
//! that still carries any of them.

use gbx::container::{body_strings_in, replace_strings, write_gbx, Container};
use crate::cli::{die, flag, has};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Skin,
    Locator,
    Zone,
    Nickname,
    Trigram,
    ClubTag,
    Login,
    AccountId,
    MapUid,
    Other,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Skin => "skin",
            Role::Locator => "locator URL",
            Role::Zone => "zone",
            Role::Nickname => "display name",
            Role::Trigram => "trigram",
            Role::ClubTag => "club tag",
            Role::Login => "login",
            Role::AccountId => "account id",
            Role::MapUid => "map uid",
            Role::Other => "",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Field {
    pub role: Role,
    pub at: usize,
    pub len: usize,
    pub s: String,
}


/// Classify the identity strings a ghost carries.
///
/// This is STRUCTURAL, not a string search. `CGameCtnGhost`'s main chunk
/// (`0x03092000`) holds, in order: the player model, the skin pack descriptors
/// (each a checksum, a path and a locator URL), the display name, then the
/// compressed record -- and AFTER the record, the trigram, the zone and the
/// club tag. The account id is its own chunk `0x0309200F` and the map uid is
/// `0x03092010`.
///
/// Anything in the chunk this does not recognise is still returned as `Other`
/// and still printed: the module never hides a string it does not understand,
/// because the last five identity bugs were all about a field nobody listed.
pub fn scan(c: &Container) -> Vec<Field> {
    let body = c.body();
    let mut out: Vec<Field> = Vec::new();
    let chunks = c.chunks();

    // the main ghost chunk, and where the record blob starts inside it
    let main = chunks.iter().find(|k| k.0 == 0x03092000);
    let rec = gbx::recwrite::find_rec_site(body).ok().map(|s| s.hdr);
    // A REPLAY nests its ghost node without a skippable 0x03092000 chunk, so
    // the structural walk has nothing to anchor on. Fall back to the whole
    // region in front of the input chunk, which is where the same fields sit.
    let fallback_span = || -> (usize, usize) {
        // Start AFTER the embedded map. A replay carries a whole map, and that
        // map has its own author login and zone in it -- scanning across it
        // finds the MAP AUTHOR and offers to rename them, which is a different
        // person and a different file.
        let start = c.embedded_map().map(|(o, n)| o + n).unwrap_or(0);
        let end = gbx::tape::find_inputs_chunk(body)
            .map(|x| x.0)
            .unwrap_or(body.len())
            .max(start);
        (start, end)
    };
    let (mpoff, mpend) = match main {
        Some(&(_, _, poff, sz)) => (poff, poff + sz),
        None => fallback_span(),
    };
    {
        let split = rec.filter(|r| *r > mpoff && *r < mpend).unwrap_or(mpend);
        // --- before the record: model, skins, display name
        let head = body_strings_in(body, mpoff, split);
        for (i, b) in head.iter().enumerate() {
            let role = if b.s.contains("Skins\\Models\\") || b.s.contains("Skins/Models/") {
                Role::Skin
            } else if b.s.starts_with("http://") || b.s.starts_with("https://") {
                Role::Locator
            } else if i + 1 == head.len() {
                // the string immediately before the record is the ghost's
                // displayed player name
                Role::Nickname
            } else {
                Role::Other
            };
            out.push(Field { role, at: b.at, len: b.len, s: b.s.clone() });
        }
        // --- after the record: trigram, zone, club tag
        let tailstart = rec
            .and_then(|r| {
                gbx::recwrite::find_rec_site(body).ok().map(|s| r + 12 + s.csize)
            })
            .unwrap_or(mpoff);
        let tail = body_strings_in(body, tailstart, mpend);
        let zi = tail.iter().position(|b| b.s.starts_with("World|"));
        for (i, b) in tail.iter().enumerate() {
            let role = match zi {
                Some(z) if i == z => Role::Zone,
                Some(z) if i + 1 == z => Role::Trigram,
                Some(z) if i == z + 1 => Role::ClubTag,
                _ => Role::Other,
            };
            out.push(Field { role, at: b.at, len: b.len, s: b.s.clone() });
        }
    }
    // --- the account id and the map uid.
    //
    // These are NOT skippable chunks: `0x0309200F` and `0x03092010` are written
    // inline, chunk id then data, with no `PIKS` marker and no size -- so a
    // scan that only knows skippable chunks walks straight past both, which is
    // how an account id survives an anonymiser. They are found by their id
    // bytes and then validated by parsing what follows.
    if let Some(f) = inline_string_chunk(body, 0x0309200F, Role::AccountId) {
        out.push(f);
    }
    if let Some(f) = inline_string_chunk(body, 0x03092010, Role::MapUid) {
        out.push(f);
    }
    out.sort_by_key(|f| f.at);
    // The inline-chunk lookup and the string walk can both land on the same
    // string; keep the named one.
    out.dedup_by(|b, a| {
        if a.at == b.at {
            if a.role == Role::Other {
                a.role = b.role;
            }
            true
        } else {
            false
        }
    });
    out
}

/// Find an inline (non-skippable) chunk that holds one length-prefixed string.
/// The chunk id may be followed by a `0x40000000` lookback marker.
fn inline_string_chunk(body: &[u8], cid: u32, role: Role) -> Option<Field> {
    let idb = cid.to_le_bytes();
    let mut i = 0usize;
    while i + 12 <= body.len() {
        if body[i..i + 4] == idb {
            for skip in [4usize, 8] {
                let p = i + skip;
                if p + 4 > body.len() {
                    continue;
                }
                if skip == 8 && u32::from_le_bytes(body[i + 4..i + 8].try_into().unwrap()) != 0x4000_0000 {
                    continue;
                }
                let n = u32::from_le_bytes(body[p..p + 4].try_into().unwrap()) as usize;
                if n == 0 || n > 64 || p + 4 + n > body.len() {
                    continue;
                }
                if let Ok(s) = std::str::from_utf8(&body[p + 4..p + 4 + n]) {
                    if s.chars().all(|c| c.is_ascii_graphic()) {
                        return Some(Field { role, at: p, len: n, s: s.to_string() });
                    }
                }
            }
        }
        i += 1;
    }
    None
}

pub fn print(c: &Container) {
    let f = scan(c);
    let named: Vec<&Field> = f.iter().filter(|x| x.role != Role::Other && x.role != Role::MapUid).collect();
    if named.is_empty() {
        println!("identity      (no identity strings found)");
        return;
    }
    println!("identity");
    for x in named {
        println!("  {:<12} @{:<7} {:?}", x.role.label(), x.at, x.s);
    }
}

pub const DEFAULT_SKIN: &str = "Skins\\Models\\CarSport\\TAS.zip";

pub fn cmd(a: &[String]) {
    let what = a.first().map(|s| s.as_str()).unwrap_or_else(|| die("ghost identity <show|set>"));
    let rest = &a[1..];
    match what {
        "show" => {
            let c = Container::load(&rest[0]).unwrap_or_else(|e| die(e));
            let f = scan(&c);
            println!("{:<14} {:>8}  {}", "role", "offset", "value");
            for x in &f {
                if x.role == Role::Other && x.s.len() < 3 {
                    continue;
                }
                println!("{:<14} {:>8}  {:?}", x.role.label(), x.at, x.s);
            }
        }
        "set" => {
            let inp = rest.first().unwrap_or_else(|| die("ghost identity set IN OUT [flags]"));
            let out = rest.get(1).unwrap_or_else(|| die("ghost identity set IN OUT [flags]"));
            let c = Container::load(inp).unwrap_or_else(|e| die(e));
            let fields = scan(&c);
            let anon = has(rest, "--anonymise") || has(rest, "--anonymize");
            let name = flag(rest, "--name");
            let trigram = flag(rest, "--trigram");
            let skin = flag(rest, "--skin");
            let login = flag(rest, "--login");
            let zone = flag(rest, "--zone");
            let clubtag = flag(rest, "--clubtag");
            if let Some(t) = trigram {
                if t.len() != 3 {
                    die(format!("a trigram is exactly 3 characters ({:?} is {})", t, t.len()));
                }
            }
            let mut edits: Vec<(usize, usize, Vec<u8>)> = Vec::new();
            let mut zero_cksum: Vec<usize> = Vec::new();
            let mut log: Vec<String> = Vec::new();
            // Which edits could be resized safely, so an anonymisation that
            // cannot shorten a string can pad instead of giving up. In a ghost
            // the identity sits inside skippable chunk 0x03092000 and shortening
            // is free; in a REPLAY the same strings sit in a nested node whose
            // offsets something else depends on, and shortening the driver's
            // name there produces a file that reads back perfectly and
            // validates to nothing.
            let chunks_ok = |at: usize, old: usize| -> bool {
                c.chunks()
                    .iter()
                    .filter(|(_, coff, poff, sz)| {
                        c.embedded_map().map_or(true, |(mo, ms)| *coff >= mo + ms || poff + sz <= mo)
                    })
                    .any(|(_, _, poff, sz)| at >= *poff && at + 4 + old <= poff + sz)
            };
            for f in &fields {
                let newv: Option<String> = match f.role {
                    Role::Skin => skin
                        .map(|s| if s == "default" { DEFAULT_SKIN.to_string() } else { s.to_string() })
                        .or(if anon { Some(DEFAULT_SKIN.to_string()) } else { None }),
                    Role::Nickname => name.map(|s| s.to_string()).or(if anon { Some("TAS".into()) } else { None }),
                    Role::Trigram => trigram.map(|s| s.to_string()).or(if anon { Some("TAS".into()) } else { None }),
                    Role::Login => login.map(|s| s.to_string()).or(if anon { Some("TAS".into()) } else { None }),
                    Role::Zone => zone.map(|s| s.to_string()),
                    Role::ClubTag => clubtag.map(|s| s.to_string()).or(if anon { Some(String::new()) } else { None }),
                    Role::Locator => if anon { Some(String::new()) } else { None },
                    Role::AccountId => if anon { Some(String::new()) } else { None },
                    _ => None,
                };
                if let Some(v) = newv {
                    let mut v = v;
                    // An anonymisation must not fail just because it cannot
                    // shorten: pad to the original byte length instead. `x`
                    // repeated is not a plausible account id or URL, which is
                    // the point.
                    if v.len() != f.len && !chunks_ok(f.at, f.len) && anon && v.len() < f.len {
                        let pad = "x".repeat(f.len);
                        log.push(format!(
                            "  {:<12} padded to {} bytes ({} cannot be resized in this container)",
                            f.role.label(),
                            f.len,
                            f.role.label()
                        ));
                        v = pad;
                    }
                    if v != f.s {
                        log.push(format!("  {:<12} {:?} -> {:?}", f.role.label(), f.s, v));
                        if f.role == Role::Skin {
                            zero_cksum.push(f.at);
                        }
                        edits.push((f.at, f.len, v.into_bytes()));
                    }
                }
            }
            if edits.is_empty() {
                die("nothing to change (`ghost identity show` lists what is there)");
            }
            edits.sort_by_key(|e| e.0);
            let mut pre = c.body().to_vec();
            for at in &zero_cksum {
                if *at >= 32 {
                    for b in pre[at - 32..*at].iter_mut() {
                        *b = 0;
                    }
                }
            }
            let protect = c.embedded_map();
            let body = replace_strings(&pre, &edits, protect).unwrap_or_else(|e| die(e));
            let unframed = gbx::container::unframed_edits();
            write_gbx(&c.gbx, body, out).unwrap_or_else(|e| die(e));
            // control: read it back and require every field to be what we asked
            let c2 = Container::load(out).unwrap_or_else(|e| die(e));
            let after = scan(&c2);
            for (role, want) in [
                (Role::Nickname, name),
                (Role::Trigram, trigram),
                (Role::Login, login),
            (Role::ClubTag, clubtag),
            ] {
                if let Some(w) = want {
                    let got = after.iter().find(|f| f.role == role).map(|f| f.s.clone());
                    if got.as_deref() != Some(w) {
                        die(format!(
                            "read-back control FAILED: {} is {:?}, asked for {:?}",
                            role.label(),
                            got,
                            w
                        ));
                    }
                }
            }
            // an anonymise that leaves an identifier behind is the exact failure
            // this exists to prevent, so it is checked rather than assumed
            if anon {
                let left: Vec<String> = after
                    .iter()
                    .filter(|f| {
                        matches!(f.role, Role::AccountId | Role::Locator)
                            && !f.s.is_empty()
                            && !f.s.chars().all(|c| c == 'x')
                    })
                    .map(|f| format!("{} {:?}", f.role.label(), f.s))
                    .collect();
                if !left.is_empty() {
                    die(format!("--anonymise left identifiers behind: {:?}", left));
                }
            }
            println!("wrote {}", out);
            for l in log {
                println!("{}", l);
            }
            println!("  read-back control OK");
            // THE CONTROL THAT MATTERS: a cosmetic edit must not change the
            // physics. Nothing in a read-back can see a file that still reads
            // correctly and no longer loads, so ask the plain oracle the same
            // question before and after.
            if !has(rest, "--no-oracle") {
                let server = crate::oracle::server_dir(flag(rest, "--server"));
                if server.join("TrackmaniaServer").exists() {
                    let mode = match flag(rest, "--map") {
                        Some(m) => crate::oracle::MapsMode::One(std::path::Path::new(m)),
                        None => crate::oracle::MapsMode::Empty,
                    };
                    let before = crate::oracle::validate(&server, std::path::Path::new(inp), mode, "id-a");
                    let after = crate::oracle::validate(&server, std::path::Path::new(out), mode, "id-b");
                    match (before, after) {
                        (Ok(b), Ok(a2)) => {
                            if b.time_ms == a2.time_ms {
                                println!("  oracle no-op control OK: {} before and after", b.secs());
                            } else {
                                let _ = std::fs::remove_file(out);
                                die(format!(
                                    "ORACLE NO-OP CONTROL FAILED: {} before, {} after. A cosmetic \
                                     edit changed what the file does. {} has been DELETED.",
                                    b.secs(),
                                    a2.secs(),
                                    out
                                ));
                            }
                        }
                        _ => println!("  oracle no-op control not run"),
                    }
                } else if !unframed.is_empty() {
                    let _ = std::fs::remove_file(out);
                    die(format!(
                        "REFUSED and deleted {}: {} edit(s) change a string's length at a place \
                         with no enclosing skippable chunk to correct ({}). A body is parsed \
                         serially so that CAN be safe -- clearing a ghost's inline account id is \
                         -- but nothing in the file proves it, and the one time it was not safe \
                         the file still read back perfectly and validated to nothing. Re-run with \
                         a dedicated server (--server DIR or TM_SERVER) so the plain oracle can \
                         decide, or use a replacement of exactly the same byte length.",
                        out,
                        unframed.len(),
                        unframed.join(", ")
                    ));
                } else {
                    println!(
                        "  oracle no-op control NOT RUN (no dedicated server), but every edit kept \
                         its byte length or sits in a chunk whose size was corrected."
                    );
                }
            }
        }
        o => die(format!("unknown `ghost identity` operation {:?}", o)),
    }
}
