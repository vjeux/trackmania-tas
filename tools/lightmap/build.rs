//! Links libwebp (the game writes its atlases with libwebp 1.6.0, preset DEFAULT; the closest we
//! can build offline is 1.4.0 from fbsource/third-party/webp). The static library is expected at
//! `vendor/libwebp140.a` (built by `scripts/build-libwebp.sh`); without it the `webp` cfg is off
//! and the baker falls back to its own VP8 encoder.
fn main() {
    export_target_cpu();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor");
    let lib = dir.join("libwebp140.a");
    println!("cargo:rerun-if-changed={}", lib.display());
    println!("cargo:rustc-check-cfg=cfg(have_libwebp)");
    if lib.exists() {
        println!("cargo:rustc-link-search=native={}", dir.display());
        println!("cargo:rustc-link-lib=static=webp140");
        println!("cargo:rustc-link-lib=dylib=m");
        println!("cargo:rustc-link-lib=dylib=pthread");
        println!("cargo:rustc-cfg=have_libwebp");
    }
}

/// The `-C target-cpu=` of this build, for the host guard (lightmap::hostcpu): cargo hands the build
/// script the encoded rustflags (`CARGO_ENCODED_RUSTFLAGS`, 0x1f-separated); the last target-cpu wins.
/// Exported as the compile-time env LMTOOL_TARGET_CPU ("generic" when none is set).
fn export_target_cpu() {
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    let flags = std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    let parts: Vec<&str> = flags.split('\x1f').collect();
    let mut cpu = "generic".to_string();
    let mut i = 0;
    while i < parts.len() {
        let p = parts[i];
        if let Some(v) = p.strip_prefix("-Ctarget-cpu=").or_else(|| p.strip_prefix("-C target-cpu=")) {
            cpu = v.to_string();
        } else if p == "-C" && i + 1 < parts.len() {
            if let Some(v) = parts[i + 1].strip_prefix("target-cpu=") {
                cpu = v.to_string();
            }
            i += 1;
        }
        i += 1;
    }
    println!("cargo:rustc-env=LMTOOL_TARGET_CPU={cpu}");
}
