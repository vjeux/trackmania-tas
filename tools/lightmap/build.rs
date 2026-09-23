//! Links libwebp (the game writes its atlases with libwebp 1.6.0, preset DEFAULT; the closest we
//! can build offline is 1.4.0 from fbsource/third-party/webp). The static library is expected at
//! `vendor/libwebp140.a` (built by `scripts/build-libwebp.sh`); without it the `webp` cfg is off
//! and the baker falls back to its own VP8 encoder.
fn main() {
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
