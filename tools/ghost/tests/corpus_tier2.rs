//! Tier 2: the decode of every reference ghost is pinned by digest, and the
//! engine agrees with it.
//!
//! Two levels, and the second is the point.
//!
//! **The digest** catches a decode that changed without anyone noticing. This
//! is not hypothetical: earlier in the release work a file was produced whose
//! every coordinate was one field late while the coverage line read 100 %, and
//! nothing caught it until a digest was compared.
//!
//! **The oracle** catches a decode that changed *consistently* — where a
//! digest would happily pin the new, wrong answer. The engine re-simulates the
//! file and must reproduce the exact millisecond the run was recorded at, so
//! it is an independent judge rather than a record of our own output.
//!
//! Skips without `TM_SERVER`, like the rest of the engine-dependent suite. The
//! dedicated server is a public download:
//!
//! ```text
//! curl -sSL -o ts.zip http://files.v04.maniaplanet.com/server/TrackmaniaServer_Latest.zip
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

fn bin(name: &str) -> Option<PathBuf> {
    let mut p = std::env::current_exe().ok()?;
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    let b = p.join(name);
    b.exists().then_some(b)
}

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata")
}

/// The reference corpus: every ghost with a known recorded time, which is
/// encoded in its own filename (`p00001_19538` ran 19.538).
fn corpus() -> Vec<(PathBuf, i64)> {
    let dir = testdata().join("decoder-goldens/ghosts");
    let Ok(rd) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut v: Vec<(PathBuf, i64)> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".Ghost.Gbx"))
        .filter_map(|p| {
            let stem = p.file_name()?.to_string_lossy().replace(".Ghost.Gbx", "");
            let ms: i64 = stem.rsplit('_').next()?.parse().ok()?;
            Some((p, ms))
        })
        .collect();
    v.sort();
    v
}

fn server_dir() -> Option<String> {
    let d = std::env::var("TM_SERVER").ok()?;
    Path::new(&d).join("TrackmaniaServer").exists().then_some(d)
}

/// The map the reference corpus was driven on.
fn corpus_map() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tmmaps/testdata/map1.Map.Gbx");
    p.exists().then_some(p)
}

#[test]
fn every_reference_run_re_simulates_to_the_time_it_was_recorded_at() {
    let (Some(tmmaps), Some(server), Some(map)) = (bin("tmmaps"), server_dir(), corpus_map())
    else {
        // No server is a SKIP, not a pass: say so, so a green run without one
        // is not mistaken for this check having happened.
        eprintln!(
            "SKIP every_reference_run_re_simulates...: needs TM_SERVER (a dir holding \
             TrackmaniaServer) and tmmaps built"
        );
        return;
    };
    let runs = corpus();
    assert!(runs.len() >= 40, "expected the reference corpus, found {}", runs.len());

    let mut wrong = Vec::new();
    for (path, ms) in &runs {
        let out = Command::new(&tmmaps)
            .args(["oracle", "--map"])
            .arg(&map)
            .arg("--ghosts")
            .arg(path)
            .arg("--server")
            .arg(&server)
            .output()
            .expect("run the oracle");
        let text = String::from_utf8_lossy(&out.stdout);
        let got = text.lines().last().unwrap_or_default().split('\t').nth(2).unwrap_or("?").trim().to_string();
        // Times print as seconds with a decimal, the house style.
        let want = format!("{}.{:03}", ms / 1000, ms % 1000);
        if got != want {
            wrong.push(format!(
                "{}: oracle {got}, recorded {want}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "runs the engine does not reproduce ({} of {}):\n  {}",
        wrong.len(),
        runs.len(),
        wrong.join("\n  ")
    );
}

/// The cheap half: no server needed, and it catches a decode that moved.
#[test]
fn every_reference_run_still_decodes_to_the_same_digest() {
    let Some(ghost) = bin("ghost") else { return };
    let runs = corpus();
    if runs.is_empty() {
        return;
    }
    let golden = testdata().join("decoder-goldens/manifest.md5");
    let mut mine = String::new();
    for (path, _) in &runs {
        let out = Command::new(&ghost)
            .arg("manifest")
            .arg(path)
            .output()
            .expect("run ghost manifest");
        // The manifest carries the file's own path; mask it so the digest
        // pins the DECODE and not the directory it ran in.
        let text = String::from_utf8_lossy(&out.stdout)
            .replace(&path.to_string_lossy().to_string(), "$FIXTURE");
        let name = path.file_name().unwrap_or_default().to_string_lossy().replace(".Ghost.Gbx", "");
        mine.push_str(&format!("{:x}  {name}\n", md5(text.as_bytes())));
    }

    if std::env::var("GHOST_BLESS").is_ok() {
        std::fs::write(&golden, &mine).expect("write golden");
        eprintln!("blessed {} digests -> {}", runs.len(), golden.display());
        return;
    }
    let Ok(want) = std::fs::read_to_string(&golden) else {
        eprintln!(
            "SKIP: {} absent -- create it with GHOST_BLESS=1",
            golden.display()
        );
        return;
    };
    let moved: Vec<(&str, &str)> = want
        .lines()
        .zip(mine.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert!(
        moved.is_empty(),
        "{} of {} runs decode differently now:\n  was {}\n  now {}",
        moved.len(),
        runs.len(),
        moved.iter().map(|(a, _)| *a).collect::<Vec<_>>().join("\n  was "),
        moved.iter().map(|(_, b)| *b).collect::<Vec<_>>().join("\n  now "),
    );
}

/// Small MD5, so this test needs no dependency. (RFC 1321.)
fn md5(data: &[u8]) -> u128 {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let k: Vec<u32> = (0..64)
        .map(|i| ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32)
        .collect();
    let (mut a0, mut b0, mut c0, mut d0) = (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    let mut msg = data.to_vec();
    let bitlen = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_le_bytes());
    for chunk in msg.chunks(64) {
        let m: Vec<u32> = (0..16)
            .map(|i| u32::from_le_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap()))
            .collect();
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f2 = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f2.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = 0u128;
    for (i, v) in [a0, b0, c0, d0].iter().enumerate() {
        for (j, byte) in v.to_le_bytes().iter().enumerate() {
            out |= (*byte as u128) << (8 * (i * 4 + j));
        }
    }
    out
}
