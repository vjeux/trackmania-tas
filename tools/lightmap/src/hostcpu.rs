//! The host-CPU guard for a build compiled for a specific CPU (`-C target-cpu=znver4` is the shipped
//! x86-64 Linux default in tools/.cargo/config.toml: the render boxes are EPYC Genoa, and the Zen 4
//! schedule + AVX-512 is worth 3–6 % of a bake, bit-identical — Rust never contracts a·b+c into an FMA
//! or reassociates a sum without explicit fast-math, and the harness confirms it).
//!
//! Such a binary faults with SIGILL on a CPU without the features (the Skylake ODs: a GFNI instruction in the
//! first Gbx decompression). `guard()` runs FIRST
//! in `main` (std's runtime start-up is precompiled for baseline x86-64, so nothing before it can hit an
//! AVX-512 instruction): when the build's target CPU needs features the host lacks it looks for a
//! fallback binary beside the executable — `<exe>-x86-64-v3`, then `<exe>-v3` — and exec's it with the
//! same arguments and environment (`LMTOOL_CPU_FALLBACK=PATH` names another); without one it prints
//! what to build and exits 3 instead of dying on an illegal instruction.
//!
//! The target CPU the build was made for comes from build.rs (`LMTOOL_TARGET_CPU`, read off the encoded
//! rustflags); `lmtool --version`-style tooling can print it through `built_for()`.

/// The `-C target-cpu=` the binary was compiled with ("generic" when none was given).
pub fn built_for() -> &'static str {
    option_env!("LMTOOL_TARGET_CPU").unwrap_or("generic")
}

/// The features a `-C target-cpu=CPU` build may emit that a lesser host lacks, as `is_x86_feature_detected!`
/// names them. Only the CPUs we build for are listed; an unknown CPU name is treated as needing nothing
/// beyond x86-64-v3 (which every devserver and render box has).
fn required_features(cpu: &str) -> &'static [&'static str] {
    match cpu {
        // Zen 4: AVX-512 (F/BW/DQ/VL/CD/IFMA/VBMI/VBMI2/VNNI/BITALG/VPOPCNTDQ/BF16), GFNI, VAES, VPCLMULQDQ
        "znver4" | "znver5" => &["avx512f", "avx512bw", "avx512dq", "avx512vl", "avx512cd", "avx512ifma", "avx512vbmi", "avx512vbmi2", "avx512vnni", "avx512bitalg", "avx512vpopcntdq", "avx512bf16", "gfni", "vaes", "vpclmulqdq", "avx2", "fma", "bmi2"],
        "x86-64-v4" | "skylake-avx512" | "icelake-server" | "sapphirerapids" => &["avx512f", "avx512bw", "avx512dq", "avx512vl", "avx2", "fma", "bmi2"],
        "x86-64-v3" | "haswell" | "znver1" | "znver2" | "znver3" => &["avx2", "fma", "bmi2", "bmi1", "movbe", "f16c"],
        _ => &[],
    }
}

/// The features of `cpu` this host lacks.
#[cfg(target_arch = "x86_64")]
pub fn missing_features(cpu: &str) -> Vec<&'static str> {
    required_features(cpu).iter().copied().filter(|f| !has(f)).collect()
}

#[cfg(not(target_arch = "x86_64"))]
pub fn missing_features(_cpu: &str) -> Vec<&'static str> {
    Vec::new()
}

/// The host's feature bits read from CPUID and XGETBV directly — NOT `is_x86_feature_detected!`: that macro
/// folds to `true` at compile time for every feature the build itself enables (`-C target-cpu=znver4` enables
/// them all), so a guard written with it is dead code in exactly the binary that needs it (found by the audit:
/// the znver4 lmtool died on a Skylake with SIGILL in miniz_oxide's Huffman init, a GFNI `vgf2p8affineqb`,
/// with the guard compiled out). Integer operations only: this runs before anything else in `main`.
#[cfg(target_arch = "x86_64")]
struct HostBits {
    l1_ecx: u32,
    l7_ebx: u32,
    l7_ecx: u32,
    l7_1_eax: u32,
    /// XCR0: the register state the OS saves — bit 1 XMM, 2 YMM, 5 opmask, 6 ZMM_Hi256, 7 Hi16_ZMM.
    xcr0: u64,
}

#[cfg(target_arch = "x86_64")]
#[inline(never)]
fn host_bits() -> HostBits {
    use std::arch::x86_64::{__cpuid, __cpuid_count};
    // SAFETY: CPUID exists on every x86-64 CPU; XGETBV is executed only when CPUID reports OSXSAVE
    unsafe {
        let max = __cpuid(0).eax;
        let l1 = __cpuid(1);
        let (l7, l7_1) = if max >= 7 {
            let l7 = __cpuid_count(7, 0);
            let l7_1 = if l7.eax >= 1 { __cpuid_count(7, 1).eax } else { 0 };
            (l7, l7_1)
        } else {
            (__cpuid_count(0, 0), 0)
        };
        let l7_valid = max >= 7;
        let osxsave = l1.ecx & (1 << 27) != 0;
        let xcr0 = if osxsave {
            let (lo, hi): (u32, u32);
            std::arch::asm!("xgetbv", in("ecx") 0u32, out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags));
            (hi as u64) << 32 | lo as u64
        } else {
            0
        };
        HostBits { l1_ecx: l1.ecx, l7_ebx: if l7_valid { l7.ebx } else { 0 }, l7_ecx: if l7_valid { l7.ecx } else { 0 }, l7_1_eax: l7_1, xcr0 }
    }
}

#[cfg(target_arch = "x86_64")]
fn has(f: &str) -> bool {
    let b = host_bits();
    let avx_state = b.xcr0 & 0x6 == 0x6;
    let avx512_state = avx_state && b.xcr0 & 0xe0 == 0xe0;
    match f {
        "avx2" => avx_state && b.l7_ebx & (1 << 5) != 0,
        "fma" => avx_state && b.l1_ecx & (1 << 12) != 0,
        "bmi1" => b.l7_ebx & (1 << 3) != 0,
        "bmi2" => b.l7_ebx & (1 << 8) != 0,
        "movbe" => b.l1_ecx & (1 << 22) != 0,
        "f16c" => avx_state && b.l1_ecx & (1 << 29) != 0,
        "avx512f" => avx512_state && b.l7_ebx & (1 << 16) != 0,
        "avx512dq" => avx512_state && b.l7_ebx & (1 << 17) != 0,
        "avx512ifma" => avx512_state && b.l7_ebx & (1 << 21) != 0,
        "avx512cd" => avx512_state && b.l7_ebx & (1 << 28) != 0,
        "avx512bw" => avx512_state && b.l7_ebx & (1 << 30) != 0,
        "avx512vl" => avx512_state && b.l7_ebx & (1 << 31) != 0,
        "avx512vbmi" => avx512_state && b.l7_ecx & (1 << 1) != 0,
        "avx512vbmi2" => avx512_state && b.l7_ecx & (1 << 6) != 0,
        "gfni" => b.l7_ecx & (1 << 8) != 0,
        "vaes" => avx_state && b.l7_ecx & (1 << 9) != 0,
        "vpclmulqdq" => avx_state && b.l7_ecx & (1 << 10) != 0,
        "avx512vnni" => avx512_state && b.l7_ecx & (1 << 11) != 0,
        "avx512bitalg" => avx512_state && b.l7_ecx & (1 << 12) != 0,
        "avx512vpopcntdq" => avx512_state && b.l7_ecx & (1 << 14) != 0,
        "avx512bf16" => avx512_state && b.l7_1_eax & (1 << 5) != 0,
        _ => true,
    }
}

/// The host's CPU model name (/proc/cpuinfo), for the messages.
pub fn host_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|s| s.lines().find(|l| l.starts_with("model name")).map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())).unwrap_or_else(|| "unknown CPU".into())
}

/// The fallback binaries looked for beside the executable, in order.
fn fallback_candidates() -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    if let Some(p) = std::env::var_os("LMTOOL_CPU_FALLBACK") {
        v.push(std::path::PathBuf::from(p));
    }
    if let Ok(exe) = std::env::current_exe() {
        let name = exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "lmtool".into());
        for suffix in ["-x86-64-v3", "-v3", "-generic"] {
            v.push(exe.with_file_name(format!("{name}{suffix}")));
        }
    }
    v
}

/// Exits or execs the fallback when this build cannot run here; returns when it can. Must be the first
/// thing `main` does.
pub fn guard() {
    let cpu = built_for();
    let missing = missing_features(cpu);
    if missing.is_empty() {
        return;
    }
    let host = host_model();
    // a fallback binary that is not itself this file
    for cand in fallback_candidates() {
        let same = std::env::current_exe().ok().and_then(|e| std::fs::canonicalize(e).ok()) == std::fs::canonicalize(&cand).ok();
        if cand.is_file() && !same {
            eprintln!("lmtool: built for {cpu}, this host ({host}) lacks {} — running {} instead", missing.join(", "), cand.display());
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                let err = std::process::Command::new(&cand).args(std::env::args_os().skip(1)).exec();
                eprintln!("lmtool: exec {} failed: {err}", cand.display());
            }
        }
    }
    eprintln!(
        "lmtool: this binary was built for -C target-cpu={cpu} and this host ({host}) lacks {}: it would die with an illegal instruction.\n\
         Build for this host with  RUSTFLAGS='-C target-cpu=x86-64-v3' cargo build --release -p lightmap --target-dir target-v3\n\
         (or put such a build beside this one as lmtool-x86-64-v3, or name one with LMTOOL_CPU_FALLBACK=PATH).",
        missing.join(", ")
    );
    std::process::exit(3);
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;

    /// The CPUID reading agrees with the runtime detection on the features the host has — and, the point of
    /// the guard, it is NOT folded: on a build for a CPU the host lacks, `has` must say so (this test can only
    /// check agreement where the two can both be trusted, i.e. where the build does not enable the feature).
    #[test]
    fn cpuid_bits_agree_with_the_runtime_detection() {
        let pairs: [(&str, bool); 8] = [
            ("avx2", std::arch::is_x86_feature_detected!("avx2")),
            ("fma", std::arch::is_x86_feature_detected!("fma")),
            ("bmi2", std::arch::is_x86_feature_detected!("bmi2")),
            ("avx512f", std::arch::is_x86_feature_detected!("avx512f")),
            ("avx512bw", std::arch::is_x86_feature_detected!("avx512bw")),
            ("avx512vbmi", std::arch::is_x86_feature_detected!("avx512vbmi")),
            ("gfni", std::arch::is_x86_feature_detected!("gfni")),
            ("avx512bf16", std::arch::is_x86_feature_detected!("avx512bf16")),
        ];
        // where the build enables a feature the macro is a compile-time `true` regardless of the host: only
        // features the build does not assume are a real comparison
        let assumed = |f: &str| -> bool {
            match f {
                "avx2" => cfg!(target_feature = "avx2"),
                "fma" => cfg!(target_feature = "fma"),
                "bmi2" => cfg!(target_feature = "bmi2"),
                "avx512f" => cfg!(target_feature = "avx512f"),
                "avx512bw" => cfg!(target_feature = "avx512bw"),
                "avx512vbmi" => cfg!(target_feature = "avx512vbmi"),
                "gfni" => cfg!(target_feature = "gfni"),
                "avx512bf16" => cfg!(target_feature = "avx512bf16"),
                _ => false,
            }
        };
        for (f, detected) in pairs {
            if assumed(f) {
                // the macro is folded here; CPUID must still report a real bit (this host runs the test)
                assert!(has(f), "{f}: the build assumes it and the host runs this test, so CPUID must report it");
            } else {
                assert_eq!(has(f), detected, "{f}");
            }
        }
        // the missing-feature list of a foreign CPU is computed, not folded: an impossible CPU name needs nothing
        assert!(missing_features("not-a-cpu").is_empty());
    }
}
