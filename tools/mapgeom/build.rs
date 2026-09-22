//! `MAPGEOM_BUILD_ID`: the converter build the bake cache keys on
//! (`bake_cache::BUILD_ID`). `<git commit>-<source hash>`: the short hash of
//! HEAD (so a cache entry says which commit made it), and an FNV-1a hash of
//! every source file of the converter — `mapgeom`, `tmmaps` and `gbx`, plus
//! the lock file — so an UNCOMMITTED edit misses the cache too (a commit hash
//! alone would serve yesterday's bake for today's dirty tree). Without git the
//! commit part is the package version.

use std::path::Path;

fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= *b as u64;
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

fn hash_tree(dir: &Path, h: &mut u64) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            hash_tree(&p, h);
        } else if let Ok(bytes) = std::fs::read(&p) {
            fnv(h, p.to_string_lossy().as_bytes());
            fnv(h, &bytes);
        }
    }
}

fn main() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let git = std::process::Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .current_dir(&manifest)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("v{}", std::env::var("CARGO_PKG_VERSION").unwrap_or_default()));
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for rel in ["src", "../tmmaps/src", "../gbx/src", "../Cargo.lock"] {
        let p = manifest.join(rel);
        if p.is_dir() {
            hash_tree(&p, &mut h);
        } else if let Ok(bytes) = std::fs::read(&p) {
            fnv(&mut h, &bytes);
        }
        println!("cargo:rerun-if-changed={}", p.display());
    }
    // a new commit (HEAD moves) changes the id even when the sources did not
    for rel in ["../../.git/HEAD", "../../.git/refs/heads"] {
        let p = manifest.join(rel);
        if p.exists() {
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }
    println!("cargo:rustc-env=MAPGEOM_BUILD_ID={git}-{h:016x}");
}
