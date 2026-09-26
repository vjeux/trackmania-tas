//! The host-CPU guard for a build compiled for a specific CPU (`-C target-cpu=znver4` is the shipped
//! x86-64 Linux default in tools/.cargo/config.toml: the render boxes are EPYC Genoa, and the Zen 4
//! schedule + AVX-512 is worth 3–6 % of a bake, bit-identical — Rust never contracts a·b+c into an FMA
//! or reassociates a sum without explicit fast-math, and the harness confirms it).
//!
//! Such a binary faults with SIGILL on a CPU without the features (the Skylake ODs). `guard()` runs FIRST
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

#[cfg(target_arch = "x86_64")]
fn has(f: &str) -> bool {
    match f {
        "avx2" => std::arch::is_x86_feature_detected!("avx2"),
        "fma" => std::arch::is_x86_feature_detected!("fma"),
        "bmi1" => std::arch::is_x86_feature_detected!("bmi1"),
        "bmi2" => std::arch::is_x86_feature_detected!("bmi2"),
        "movbe" => std::arch::is_x86_feature_detected!("movbe"),
        "f16c" => std::arch::is_x86_feature_detected!("f16c"),
        "avx512f" => std::arch::is_x86_feature_detected!("avx512f"),
        "avx512bw" => std::arch::is_x86_feature_detected!("avx512bw"),
        "avx512dq" => std::arch::is_x86_feature_detected!("avx512dq"),
        "avx512vl" => std::arch::is_x86_feature_detected!("avx512vl"),
        "avx512cd" => std::arch::is_x86_feature_detected!("avx512cd"),
        "avx512ifma" => std::arch::is_x86_feature_detected!("avx512ifma"),
        "avx512vbmi" => std::arch::is_x86_feature_detected!("avx512vbmi"),
        "avx512vbmi2" => std::arch::is_x86_feature_detected!("avx512vbmi2"),
        "avx512vnni" => std::arch::is_x86_feature_detected!("avx512vnni"),
        "avx512bitalg" => std::arch::is_x86_feature_detected!("avx512bitalg"),
        "avx512vpopcntdq" => std::arch::is_x86_feature_detected!("avx512vpopcntdq"),
        "avx512bf16" => std::arch::is_x86_feature_detected!("avx512bf16"),
        "gfni" => std::arch::is_x86_feature_detected!("gfni"),
        "vaes" => std::arch::is_x86_feature_detected!("vaes"),
        "vpclmulqdq" => std::arch::is_x86_feature_detected!("vpclmulqdq"),
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
