//! DXBC (Direct3D 11 shader bytecode) reader: the container, the `RDEF`
//! reflection chunk (constant buffers, their variables, the bound resources)
//! and a disassembler for the `SHEX`/`SHDR` shader-model 4/5 token stream.
//!
//! The game ships every shader it uses precompiled in
//! `Packs/GpuCache_D3D11_SM5.zip` (one `*.hlsl.GpuCache.Gbx` per entry point,
//! the DXBC blob LZO-packed in the Gbx body). The lightmapper is a GPU
//! program -- `Lightmap/*.hlsl` -- so its light model is written in these
//! blobs, not in the exe. Reading them needs exactly what this module does:
//! the token format is the public `d3d11TokenizedProgramFormat.hpp` layout,
//! printed in fxc's own notation (`mad r0.xyz, r1.xyzx, l(0.5, 0.5, 0.5, 0),
//! r2.xyzx`) so any D3D reference reads the output directly.
//!
//! What is NOT here: hull/domain/geometry-specific declarations beyond the
//! generic operand walk (printed as raw dwords), SM5.1 register spaces (the
//! cache is SM5.0), and the `STAT`/`SFI0` chunks (nothing in them decides
//! anything).

use std::fmt::Write as _;

fn u32at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn u16at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn cstr(b: &[u8], o: usize) -> String {
    let end = b[o..].iter().position(|&c| c == 0).map(|p| o + p).unwrap_or(b.len());
    String::from_utf8_lossy(&b[o..end]).into_owned()
}

/// Every `DXBC` blob inside `data` (a decompressed GpuCache body may carry
/// more than one), as (offset, length).
pub fn find_blobs(data: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 32 <= data.len() {
        if &data[i..i + 4] == b"DXBC" {
            let total = u32at(data, i + 24) as usize;
            if total >= 32 && i + total <= data.len() {
                out.push((i, total));
                i += total;
                continue;
            }
        }
        i += 1;
    }
    out
}

pub struct Chunk<'a> {
    pub fourcc: String,
    pub data: &'a [u8],
}

pub fn chunks(blob: &[u8]) -> Vec<Chunk<'_>> {
    let n = u32at(blob, 28) as usize;
    let mut out = Vec::new();
    for k in 0..n {
        let off = u32at(blob, 32 + 4 * k) as usize;
        if off + 8 > blob.len() {
            break;
        }
        let fourcc = String::from_utf8_lossy(&blob[off..off + 4]).into_owned();
        let len = u32at(blob, off + 4) as usize;
        let end = (off + 8 + len).min(blob.len());
        out.push(Chunk { fourcc, data: &blob[off + 8..end] });
    }
    out
}

// ------------------------------------------------------------------ RDEF

fn var_type_name(class: u16, ty: u16, rows: u16, cols: u16) -> String {
    let base = match ty {
        0 => "void",
        1 => "bool",
        2 => "int",
        3 => "float",
        4 => "string",
        5 => "texture",
        6 => "texture1d",
        7 => "texture2d",
        8 => "texture3d",
        9 => "texturecube",
        10 => "sampler",
        19 => "uint",
        20 => "uint8",
        29 => "double",
        _ => "type?",
    };
    match class {
        0 => base.to_string(),                             // scalar
        1 => format!("{}{}", base, cols),                  // vector
        2 | 3 => format!("{}{}x{}", base, rows, cols),     // matrix rows/cols
        5 => "struct".to_string(),
        _ => format!("{}[class {}]", base, class),
    }
}

/// Human-readable dump of the reflection chunk: resource bindings, then each
/// constant buffer with its variables (name, byte offset, size, type).
pub fn rdef_dump(d: &[u8]) -> String {
    let mut s = String::new();
    if d.len() < 28 {
        return "  (RDEF too short)\n".into();
    }
    let n_cb = u32at(d, 0) as usize;
    let cb_off = u32at(d, 4) as usize;
    let n_res = u32at(d, 8) as usize;
    let res_off = u32at(d, 12) as usize;
    let minor = d[16];
    let major = d[17];
    let ptype = u16at(d, 18);
    let creator = u32at(d, 24) as usize;
    let rd11 = d.len() >= 32 && &d[28..32] == b"RD11";
    let kind = match ptype {
        0xFFFF => "ps",
        0xFFFE => "vs",
        0x4753 => "gs",
        0x4853 => "hs",
        0x4453 => "ds",
        0x4353 => "cs",
        _ => "??",
    };
    let _ = writeln!(s, "  {}_{}_{}  creator \"{}\"", kind, major, minor, if creator < d.len() { cstr(d, creator) } else { String::new() });
    // RD11 (fxc for D3D11) follows the header with its record sizes:
    // +32 header size, +36 cbuffer desc, +40 binding desc, +44 variable
    // desc, +48 type desc, +52 member desc. SM5.0 bindings are 32 bytes;
    // SM5.1 grows them to 40 (space, id) and this reads that size.
    let (res_stride, vstride) = if rd11 && d.len() >= 56 {
        (u32at(d, 40) as usize, u32at(d, 44) as usize)
    } else {
        (32, 24)
    };
    for i in 0..n_res {
        let o = res_off + i * res_stride;
        if o + 32 > d.len() {
            break;
        }
        let name = cstr(d, u32at(d, o) as usize);
        let ty = u32at(d, o + 4);
        let ret = u32at(d, o + 8);
        let dim = u32at(d, o + 12);
        let samples = u32at(d, o + 16);
        let bind = u32at(d, o + 20);
        let count = u32at(d, o + 24);
        let flags = u32at(d, o + 28);
        let tyname = match ty {
            0 => "cbuffer",
            1 => "tbuffer",
            2 => "texture",
            3 => "sampler",
            4 => "uav_rwtyped",
            5 => "structured",
            6 => "uav_rwstructured",
            7 => "byteaddress",
            8 => "uav_rwbyteaddress",
            9 => "uav_append",
            10 => "uav_consume",
            11 => "uav_rwstructured_counter",
            _ => "?",
        };
        let dimname = match dim {
            0 => "",
            1 => "buffer",
            2 => "1d",
            3 => "1darray",
            4 => "2d",
            5 => "2darray",
            6 => "2dms",
            7 => "2dmsarray",
            8 => "3d",
            9 => "cube",
            10 => "cubearray",
            11 => "bufferex",
            _ => "dim?",
        };
        let retname = match ret {
            0 => "",
            1 => "unorm",
            2 => "snorm",
            3 => "sint",
            4 => "uint",
            5 => "float",
            6 => "mixed",
            7 => "double",
            8 => "continued",
            _ => "ret?",
        };
        let reg = match ty {
            0 | 1 => "cb",
            3 => "s",
            4 | 6 | 8 | 9 | 10 | 11 => "u",
            _ => "t",
        };
        let _ = writeln!(
            s,
            "  {:<14} {}{:<3} {:<22} {} {} {}{}{}",
            tyname,
            reg,
            bind,
            name,
            dimname,
            retname,
            if count != 1 { format!("count={} ", count) } else { String::new() },
            if samples != 0 && samples != 0xFFFFFFFF { format!("samples={} ", samples) } else { String::new() },
            if flags != 0 { format!("flags=0x{:x}", flags) } else { String::new() }
        );
    }
    // constant buffers
    for i in 0..n_cb {
        let o = cb_off + i * 24;
        if o + 24 > d.len() {
            break;
        }
        let name = cstr(d, u32at(d, o) as usize);
        let nvar = u32at(d, o + 4) as usize;
        let var_off = u32at(d, o + 8) as usize;
        let size = u32at(d, o + 12);
        let cbtype = u32at(d, o + 20);
        let _ = writeln!(s, "  cbuffer {} ({} bytes{})", name, size, match cbtype {
            0 => "",
            1 => ", tbuffer",
            2 => ", interface pointers",
            3 => ", resource bind info",
            _ => ", type?",
        });
        for v in 0..nvar {
            let vo = var_off + v * vstride;
            if vo + 24 > d.len() {
                break;
            }
            let vname = cstr(d, u32at(d, vo) as usize);
            let start = u32at(d, vo + 4);
            let vsize = u32at(d, vo + 8);
            let vflags = u32at(d, vo + 12);
            let tyoff = u32at(d, vo + 16) as usize;
            let mut tyname = String::from("?");
            if tyoff + 12 <= d.len() {
                let class = u16at(d, tyoff);
                let ty = u16at(d, tyoff + 2);
                let rows = u16at(d, tyoff + 4);
                let cols = u16at(d, tyoff + 6);
                let elems = u16at(d, tyoff + 8);
                tyname = var_type_name(class, ty, rows, cols);
                if elems > 1 {
                    let _ = write!(tyname, "[{}]", elems);
                }
                if class == 5 && rd11 && tyoff + 36 <= d.len() {
                    let nm = u32at(d, tyoff + 32) as usize;
                    if nm < d.len() {
                        tyname = format!("struct {}", cstr(d, nm));
                    }
                }
            }
            let _ = writeln!(
                s,
                "    c[{:>4}] +{:<4} {:<26} {}{}",
                start / 16,
                start % 16,
                vname,
                tyname,
                if vflags & 2 != 0 { format!("  ({} bytes, used)", vsize) } else { format!("  ({} bytes, unused)", vsize) }
            );
            if tyoff + 12 <= d.len() && u16at(d, tyoff) == 5 {
                struct_members(d, tyoff, start, 6, &mut s, rd11);
            }
        }
    }
    s
}

/// The members of a struct type record, each at its absolute byte offset in
/// the constant buffer (`base` + member offset), nested structs recursed.
fn struct_members(d: &[u8], tyoff: usize, base: u32, indent: usize, s: &mut String, rd11: bool) {
    let nmem = u16at(d, tyoff + 10) as usize;
    let moff = u32at(d, tyoff + 12) as usize;
    let elems = u16at(d, tyoff + 8).max(1) as u32;
    let _ = elems;
    for m in 0..nmem {
        let o = moff + m * 12;
        if o + 12 > d.len() {
            break;
        }
        let name = cstr(d, u32at(d, o) as usize);
        let mty = u32at(d, o + 4) as usize;
        let off = u32at(d, o + 8);
        let mut tyname = String::from("?");
        let mut is_struct = false;
        if mty + 12 <= d.len() {
            let class = u16at(d, mty);
            let ty = u16at(d, mty + 2);
            let rows = u16at(d, mty + 4);
            let cols = u16at(d, mty + 6);
            let el = u16at(d, mty + 8);
            tyname = var_type_name(class, ty, rows, cols);
            if el > 1 {
                let _ = write!(tyname, "[{}]", el);
            }
            is_struct = class == 5;
            if is_struct && rd11 && mty + 36 <= d.len() {
                let nm = u32at(d, mty + 32) as usize;
                if nm < d.len() {
                    tyname = format!("struct {}", cstr(d, nm));
                }
            }
        }
        let abs = base + off;
        let _ = writeln!(s, "{}c[{:>4}].{} +{:<3} {:<28} {}", " ".repeat(indent), abs / 16, ["x", "y", "z", "w"][((abs % 16) / 4) as usize], abs % 16, name, tyname);
        if is_struct && indent < 30 {
            struct_members(d, mty, abs, indent + 2, s, rd11);
        }
    }
}

// ------------------------------------------------------------- signatures

pub fn signature_dump(d: &[u8], name: &str) -> String {
    let mut s = String::new();
    if d.len() < 8 {
        return s;
    }
    let n = u32at(d, 0) as usize;
    let stride = if name == "OSG5" || name == "ISG1" || name == "OSG1" || name == "PSG1" { 32 } else { 24 };
    let base = 8;
    for i in 0..n {
        let o = base + i * stride;
        if o + 24 > d.len() {
            break;
        }
        let (nm_off, sem_idx, sysval, comp, reg, mask, rw) = if stride == 32 {
            (u32at(d, o + 4), u32at(d, o + 8), u32at(d, o + 12), u32at(d, o + 16), u32at(d, o + 20), d[o + 24], d[o + 25])
        } else {
            (u32at(d, o), u32at(d, o + 4), u32at(d, o + 8), u32at(d, o + 12), u32at(d, o + 16), d[o + 20], d[o + 21])
        };
        let nm = if (nm_off as usize) < d.len() { cstr(d, nm_off as usize) } else { "?".into() };
        let comps = |m: u8| -> String {
            let mut t = String::new();
            for (k, c) in "xyzw".chars().enumerate() {
                if m & (1 << k) != 0 {
                    t.push(c);
                }
            }
            t
        };
        let _ = writeln!(
            s,
            "  {:<14} {}  {}{}  reg {}  mask {:<4} used {:<4}{}",
            nm,
            sem_idx,
            ["float", "uint", "int", "float", "?", "?"].get(comp as usize).unwrap_or(&"?"),
            "",
            reg,
            comps(mask),
            comps(rw),
            if sysval != 0 { format!(" sysval {}", sysval) } else { String::new() }
        );
    }
    s
}

// -------------------------------------------------------------- SHEX/SHDR

const OPCODES: &[&str] = &[
    "add", "and", "break", "breakc", "call", "callc", "case", "continue", "continuec", "cut", "default",
    "deriv_rtx", "deriv_rty", "discard", "div", "dp2", "dp3", "dp4", "else", "emit", "emitthencut", "endif",
    "endloop", "endswitch", "eq", "exp", "frc", "ftoi", "ftou", "ge", "iadd", "if", "ieq", "ige", "ilt", "imad",
    "imax", "imin", "imul", "ine", "ineg", "ishl", "ishr", "itof", "label", "ld", "ld_ms", "log", "loop", "lt",
    "mad", "min", "max", "customdata", "mov", "movc", "mul", "ne", "nop", "not", "or", "resinfo", "ret", "retc",
    "round_ne", "round_ni", "round_pi", "round_z", "rsq", "sample", "sample_c", "sample_c_lz", "sample_l",
    "sample_d", "sample_b", "sqrt", "switch", "sincos", "udiv", "ult", "uge", "umul", "umad", "umax", "umin",
    "ushr", "utof", "xor", "dcl_resource", "dcl_constantbuffer", "dcl_sampler", "dcl_indexrange",
    "dcl_outputtopology", "dcl_inputprimitive", "dcl_maxout", "dcl_input", "dcl_input_sgv", "dcl_input_siv",
    "dcl_input_ps", "dcl_input_ps_sgv", "dcl_input_ps_siv", "dcl_output", "dcl_output_sgv", "dcl_output_siv",
    "dcl_temps", "dcl_indexableTemp", "dcl_globalFlags", "reserved0", "lod", "gather4", "samplepos",
    "sampleinfo", "reserved1", "hs_decls", "hs_control_point_phase", "hs_fork_phase", "hs_join_phase",
    "emit_stream", "cut_stream", "emitthencut_stream", "fcall", "bufinfo", "deriv_rtx_coarse",
    "deriv_rtx_fine", "deriv_rty_coarse", "deriv_rty_fine", "gather4_c", "gather4_po", "gather4_po_c", "rcp",
    "f32tof16", "f16tof32", "uaddc", "usubb", "countbits", "firstbit_hi", "firstbit_lo", "firstbit_shi",
    "ubfe", "ibfe", "bfi", "bfrev", "swapc", "dcl_stream", "dcl_function_body", "dcl_function_table",
    "dcl_interface", "dcl_input_control_point_count", "dcl_output_control_point_count", "dcl_tessellator_domain",
    "dcl_tessellator_partitioning", "dcl_tessellator_output_primitive", "dcl_hs_max_tessfactor",
    "dcl_hs_fork_phase_instance_count", "dcl_hs_join_phase_instance_count", "dcl_thread_group",
    "dcl_uav_typed", "dcl_uav_raw", "dcl_uav_structured", "dcl_tgsm_raw", "dcl_tgsm_structured",
    "dcl_resource_raw", "dcl_resource_structured", "ld_uav_typed", "store_uav_typed", "ld_raw", "store_raw",
    "ld_structured", "store_structured", "atomic_and", "atomic_or", "atomic_xor", "atomic_cmp_store",
    "atomic_iadd", "atomic_imax", "atomic_imin", "atomic_umax", "atomic_umin", "imm_atomic_alloc",
    "imm_atomic_consume", "imm_atomic_iadd", "imm_atomic_and", "imm_atomic_or", "imm_atomic_xor",
    "imm_atomic_exch", "imm_atomic_cmp_exch", "imm_atomic_imax", "imm_atomic_imin", "imm_atomic_umax",
    "imm_atomic_umin", "sync", "dadd", "dmax", "dmin", "dmul", "deq", "dge", "dlt", "dne", "dmov", "dmovc",
    "dtof", "ftod", "eval_snapped", "eval_sample_index", "eval_centroid", "dcl_gs_instance_count", "abort",
    "debug_break", "reserved2", "ddiv", "dfma", "drcp", "msad", "dtoi", "dtou", "itod", "utod",
];

/// Opcodes whose immediates are integers (printed as ints, not floats).
fn int_typed(op: u32) -> bool {
    matches!(
        op,
        1 | 30 | 32 | 33 | 34 | 35 | 36 | 37 | 38 | 39 | 40 | 41 | 42 | 43 | 45 | 46 | 59 | 60 | 61 | 78 | 79 | 80 | 81 | 82 | 83
            | 84 | 85 | 86 | 87 | 108 | 111 | 121 | 132 | 133 | 134 | 135 | 136 | 137 | 138 | 139 | 140 | 141 | 163 | 164 | 165
            | 166 | 167 | 168
    ) || (169..=189).contains(&op)
}

struct Operand {
    text: String,
    len: usize,
}

fn swizzle_name(i: u32) -> char {
    ['x', 'y', 'z', 'w'][(i & 3) as usize]
}

fn reg_prefix(ty: u32) -> &'static str {
    match ty {
        0 => "r",
        1 => "v",
        2 => "o",
        3 => "x",
        4 => "l",
        5 => "d",
        6 => "s",
        7 => "t",
        8 => "cb",
        9 => "icb",
        10 => "label",
        11 => "vPrim",
        12 => "oDepth",
        13 => "null",
        14 => "rasterizer",
        15 => "oMask",
        16 => "m",
        17 => "fb",
        18 => "ft",
        19 => "fp",
        20 => "fi",
        21 => "fo",
        22 => "vOutputControlPointID",
        23 => "vForkInstanceID",
        24 => "vJoinInstanceID",
        25 => "vicp",
        26 => "vocp",
        27 => "vpc",
        28 => "vDomain",
        29 => "this",
        30 => "u",
        31 => "g",
        32 => "vThreadID",
        33 => "vThreadGroupID",
        34 => "vThreadIDInGroup",
        35 => "vCoverage",
        36 => "vThreadIDInGroupFlattened",
        37 => "vGSInstanceID",
        38 => "oDepthGE",
        39 => "oDepthLE",
        40 => "vCycleCounter",
        41 => "oStencilRef",
        42 => "vInnerCoverage",
        _ => "op?",
    }
}

/// Parse one operand at `toks[i..]`; `as_int` decides how immediates print.
fn parse_operand(toks: &[u32], i: usize, as_int: bool) -> Operand {
    let t0 = toks[i];
    let mut n = 1;
    let ncomp_code = t0 & 3;
    let sel_mode = (t0 >> 2) & 3;
    let ty = (t0 >> 12) & 0xFF;
    let idx_dim = (t0 >> 20) & 3;
    let mut ext = t0 >> 31 != 0;
    let mut modifier = 0u32;
    let mut minprec = 0u32;
    while ext {
        let e = toks[i + n];
        n += 1;
        if e & 0x3F == 1 {
            modifier = (e >> 6) & 0xFF;
            minprec = (e >> 14) & 7;
        }
        ext = e >> 31 != 0;
    }
    let mut text = String::new();
    if ty == 4 || ty == 5 {
        // immediates
        let count = match ncomp_code {
            1 => 1,
            2 => 4,
            _ => 0,
        };
        let vals: Vec<u32> = toks[i + n..i + n + count].to_vec();
        n += count;
        if ty == 5 {
            let mut parts = Vec::new();
            for k in (0..count).step_by(2) {
                let bits = ((vals.get(k + 1).copied().unwrap_or(0) as u64) << 32) | vals[k] as u64;
                parts.push(format!("{}", f64::from_bits(bits)));
            }
            text = format!("d({})", parts.join(", "));
        } else {
            let parts: Vec<String> = vals.iter().map(|&v| imm_text(v, as_int)).collect();
            text = format!("l({})", parts.join(", "));
        }
        return Operand { text: wrap_modifier(text, modifier), len: n };
    }
    // register with indices
    let mut idx_text = Vec::new();
    for d in 0..idx_dim {
        let rep = (t0 >> (22 + 3 * d)) & 7;
        match rep {
            0 => {
                idx_text.push(format!("{}", toks[i + n]));
                n += 1;
            }
            1 => {
                let v = ((toks[i + n + 1] as u64) << 32) | toks[i + n] as u64;
                idx_text.push(format!("{}", v));
                n += 2;
            }
            2 => {
                let sub = parse_operand(toks, i + n, true);
                n += sub.len;
                idx_text.push(sub.text);
            }
            3 => {
                let imm = toks[i + n];
                n += 1;
                let sub = parse_operand(toks, i + n, true);
                n += sub.len;
                idx_text.push(format!("{} + {}", sub.text, imm));
            }
            4 => {
                let imm = ((toks[i + n + 1] as u64) << 32) | toks[i + n] as u64;
                n += 2;
                let sub = parse_operand(toks, i + n, true);
                n += sub.len;
                idx_text.push(format!("{} + {}", sub.text, imm));
            }
            _ => {}
        }
    }
    let _ = write!(text, "{}", reg_prefix(ty));
    match idx_dim {
        0 => {}
        1 => {
            let _ = write!(text, "{}", idx_text[0]);
        }
        2 => {
            let _ = write!(text, "{}[{}]", idx_text[0], idx_text[1]);
        }
        _ => {
            let _ = write!(text, "{}[{}][{}]", idx_text[0], idx_text[1], idx_text[2]);
        }
    }
    // component selection
    if ncomp_code == 2 {
        match sel_mode {
            0 => {
                let mask = (t0 >> 4) & 0xF;
                if mask != 0xF {
                    text.push('.');
                    for k in 0..4 {
                        if mask & (1 << k) != 0 {
                            text.push(swizzle_name(k));
                        }
                    }
                }
            }
            1 => {
                let sw = (t0 >> 4) & 0xFF;
                let s: String = (0..4).map(|k| swizzle_name((sw >> (2 * k)) & 3)).collect();
                if s != "xyzw" {
                    text.push('.');
                    text.push_str(&s);
                }
            }
            2 => {
                let c = (t0 >> 4) & 3;
                text.push('.');
                text.push(swizzle_name(c));
            }
            _ => {}
        }
    }
    let _ = minprec;
    Operand { text: wrap_modifier(text, modifier), len: n }
}

fn wrap_modifier(t: String, m: u32) -> String {
    match m {
        1 => format!("-{}", t),
        2 => format!("|{}|", t),
        3 => format!("-|{}|", t),
        _ => t,
    }
}

fn imm_text(v: u32, as_int: bool) -> String {
    if as_int {
        return format!("{}", v as i32);
    }
    let f = f32::from_bits(v);
    // a value that only makes sense as an integer bit pattern
    if f.is_nan() || (f != 0.0 && (f.abs() < 1e-30 || f.abs() > 1e30)) {
        if v < 0x10000 || (v as i32) > -0x10000 {
            return format!("{}", v as i32);
        }
        return format!("0x{:08x}", v);
    }
    let mut s = format!("{}", f);
    if !s.contains('.') && !s.contains('e') && !s.contains("inf") {
        s.push_str(".0");
    }
    s
}

fn sysval_name(v: u32) -> &'static str {
    match v {
        0 => "undefined",
        1 => "position",
        2 => "clip_distance",
        3 => "cull_distance",
        4 => "rendertarget_array_index",
        5 => "viewport_array_index",
        6 => "vertex_id",
        7 => "primitive_id",
        8 => "instance_id",
        9 => "is_front_face",
        10 => "sampleIndex",
        11..=22 => "tessfactor",
        _ => "sysval?",
    }
}

fn resource_dim(v: u32) -> &'static str {
    match v {
        0 => "unknown",
        1 => "buffer",
        2 => "texture1d",
        3 => "texture2d",
        4 => "texture2dms",
        5 => "texture3d",
        6 => "texturecube",
        7 => "texture1darray",
        8 => "texture2darray",
        9 => "texture2dmsarray",
        10 => "texturecubearray",
        11 => "raw_buffer",
        12 => "structured_buffer",
        _ => "dim?",
    }
}

fn ret_type(v: u32) -> &'static str {
    match v {
        1 => "unorm",
        2 => "snorm",
        3 => "sint",
        4 => "uint",
        5 => "float",
        6 => "mixed",
        7 => "double",
        8 => "continued",
        9 => "unused",
        _ => "?",
    }
}

fn ret_types4(v: u32) -> String {
    format!("({},{},{},{})", ret_type(v & 0xF), ret_type((v >> 4) & 0xF), ret_type((v >> 8) & 0xF), ret_type((v >> 12) & 0xF))
}

/// Disassemble a `SHEX`/`SHDR` chunk. Structured control flow is indented.
pub fn disassemble(d: &[u8]) -> String {
    let mut out = String::new();
    if d.len() < 8 {
        return out;
    }
    let ver = u32at(d, 0);
    let ptype = ver >> 16;
    let major = (ver >> 4) & 0xF;
    let minor = ver & 0xF;
    let kind = ["ps", "vs", "gs", "hs", "ds", "cs"].get(ptype as usize).copied().unwrap_or("??");
    let _ = writeln!(out, "{}_{}_{}", kind, major, minor);
    let ntok = u32at(d, 4) as usize;
    let toks: Vec<u32> = (0..ntok.min(d.len() / 4)).map(|k| u32at(d, 4 * k)).collect();
    let mut i = 2;
    let mut indent = 0usize;
    while i < toks.len() {
        let t0 = toks[i];
        let op = t0 & 0x7FF;
        if op == 53 {
            // custom data: [class in bits 31:11][length]
            let class = t0 >> 11;
            let len = toks.get(i + 1).copied().unwrap_or(2) as usize;
            let len = len.max(2);
            if class == 3 {
                let _ = writeln!(out, "{}dcl_immediateConstantBuffer {{", "  ".repeat(indent));
                let body = &toks[i + 2..(i + len).min(toks.len())];
                for row in body.chunks(4) {
                    let parts: Vec<String> = row.iter().map(|&v| format!("{}", imm_text(v, false))).collect();
                    let ints: Vec<String> = row.iter().map(|&v| format!("{}", v as i32)).collect();
                    let _ = writeln!(out, "{}  {{ {} }}   // int {{ {} }}", "  ".repeat(indent), parts.join(", "), ints.join(", "));
                }
                let _ = writeln!(out, "{}}}", "  ".repeat(indent));
            } else {
                let _ = writeln!(out, "{}customdata class {} ({} dwords)", "  ".repeat(indent), class, len);
            }
            i += len;
            continue;
        }
        let mut len = ((t0 >> 24) & 0x7F) as usize;
        if len == 0 {
            len = 1;
        }
        let end = (i + len).min(toks.len());
        let mut n = 1;
        let mut ext_notes = Vec::new();
        let mut ext = t0 >> 31 != 0;
        while ext && i + n < end {
            let e = toks[i + n];
            n += 1;
            match e & 0x3F {
                1 => {
                    let sx = |v: u32| -> i32 { ((v & 0xF) as i32) << 28 >> 28 };
                    ext_notes.push(format!("aoffimmi({},{},{})", sx(e >> 9), sx(e >> 13), sx(e >> 17)));
                }
                2 => ext_notes.push(format!("indexable({}, stride={})", resource_dim((e >> 6) & 0x1F), (e >> 11) & 0xFFF)),
                3 => ext_notes.push(ret_types4(e >> 6)),
                _ => ext_notes.push(format!("ext?{:#x}", e)),
            }
            ext = e >> 31 != 0;
        }
        let name = OPCODES.get(op as usize).copied().unwrap_or("op?");
        let mut mnem = name.to_string();
        let ctl = (t0 >> 11) & 0x1FFF;
        // per-opcode control bits
        let sat = (t0 >> 13) & 1 == 1 && !(88..=107).contains(&op) && !(143..=162).contains(&op) && op != 53;
        match op {
            3 | 5 | 8 | 13 | 31 | 63 => {
                // test boolean
                if (t0 >> 18) & 1 == 1 {
                    mnem.push_str("_nz");
                } else {
                    mnem.push_str("_z");
                }
            }
            61 => match ctl & 3 {
                1 => mnem.push_str("_rcpFloat"),
                2 => mnem.push_str("_uint"),
                _ => {}
            },
            190 => {
                let mut f = Vec::new();
                if ctl & 1 != 0 {
                    f.push("t");
                }
                if ctl & 2 != 0 {
                    f.push("g");
                }
                if ctl & 4 != 0 {
                    f.push("ugroup");
                }
                if ctl & 8 != 0 {
                    f.push("uglobal");
                }
                mnem = format!("sync_{}", f.join("_"));
            }
            _ => {}
        }
        if sat && op != 3 && op != 31 && op != 63 && op != 5 && op != 8 && op != 13 && op != 190 {
            mnem.push_str("_sat");
        }
        let as_int = int_typed(op);
        let pad = "  ".repeat(indent);
        let mut line = String::new();
        match op {
            // ---- declarations with trailing dwords
            88 => {
                let dim = ctl & 0x1F;
                let o = parse_operand(&toks[..end], i + n, true);
                n += o.len;
                let rt = toks.get(i + n).copied().unwrap_or(0);
                n += 1;
                let ms = (ctl >> 5) & 0x7F;
                let _ = write!(line, "dcl_resource_{}{} {} {}", resource_dim(dim), if ms > 0 { format!("({})", ms) } else { String::new() }, ret_types4(rt), o.text);
            }
            89 => {
                let o = parse_operand(&toks[..end], i + n, true);
                n += o.len;
                let _ = write!(line, "dcl_constantbuffer {}, {}", o.text, if ctl & 1 == 1 { "dynamicIndexed" } else { "immediateIndexed" });
            }
            90 => {
                let o = parse_operand(&toks[..end], i + n, true);
                n += o.len;
                let mode = ctl & 0xF;
                let _ = write!(line, "dcl_sampler {}, {}", o.text, match mode {
                    0 => "mode_default",
                    1 => "mode_comparison",
                    2 => "mode_mono",
                    _ => "mode?",
                });
            }
            91 => {
                let o = parse_operand(&toks[..end], i + n, true);
                n += o.len;
                let c = toks.get(i + n).copied().unwrap_or(0);
                n += 1;
                let _ = write!(line, "dcl_indexrange {} {}", o.text, c);
            }
            96 | 97 | 99 | 100 | 102 | 103 => {
                let o = parse_operand(&toks[..end], i + n, true);
                n += o.len;
                let sv = toks.get(i + n).copied().unwrap_or(0);
                n += 1;
                let interp = if op == 99 || op == 100 { format!("{} ", interp_name(ctl & 0xF)) } else { String::new() };
                let _ = write!(line, "{} {}{}, {}", name, interp, o.text, sysval_name(sv));
            }
            98 => {
                let o = parse_operand(&toks[..end], i + n, true);
                n += o.len;
                let _ = write!(line, "dcl_input_ps {} {}", interp_name(ctl & 0xF), o.text);
            }
            104 => {
                let c = toks.get(i + n).copied().unwrap_or(0);
                n += 1;
                let _ = write!(line, "dcl_temps {}", c);
            }
            105 => {
                let a = toks.get(i + n).copied().unwrap_or(0);
                let b = toks.get(i + n + 1).copied().unwrap_or(0);
                let c = toks.get(i + n + 2).copied().unwrap_or(0);
                n += 3;
                let _ = write!(line, "dcl_indexableTemp x{}[{}], {}", a, b, c);
            }
            106 => {
                let mut f = Vec::new();
                if ctl & 1 != 0 {
                    f.push("refactoringAllowed");
                }
                if ctl & 2 != 0 {
                    f.push("enableDoublePrecisionFloatOps");
                }
                if ctl & 4 != 0 {
                    f.push("forceEarlyDepthStencil");
                }
                if ctl & 8 != 0 {
                    f.push("enableRawAndStructuredBuffers");
                }
                if ctl & 16 != 0 {
                    f.push("skipOptimization");
                }
                if ctl & 32 != 0 {
                    f.push("enableMinimumPrecision");
                }
                let _ = write!(line, "dcl_globalFlags {}", f.join(" | "));
            }
            155 => {
                let x = toks.get(i + n).copied().unwrap_or(0);
                let y = toks.get(i + n + 1).copied().unwrap_or(0);
                let z = toks.get(i + n + 2).copied().unwrap_or(0);
                n += 3;
                let _ = write!(line, "dcl_thread_group {}, {}, {}", x, y, z);
            }
            156 => {
                let dim = ctl & 0x1F;
                let o = parse_operand(&toks[..end], i + n, true);
                n += o.len;
                let rt = toks.get(i + n).copied().unwrap_or(0);
                n += 1;
                let _ = write!(line, "dcl_uav_typed_{} {} {}{}", resource_dim(dim), ret_types4(rt), o.text, if (ctl >> 5) & 1 == 1 { ", globallyCoherent" } else { "" });
            }
            158 | 162 => {
                let o = parse_operand(&toks[..end], i + n, true);
                n += o.len;
                let stride = toks.get(i + n).copied().unwrap_or(0);
                n += 1;
                let _ = write!(line, "{} {}, stride {}", name, o.text, stride);
            }
            159 => {
                let o = parse_operand(&toks[..end], i + n, true);
                n += o.len;
                let bytes = toks.get(i + n).copied().unwrap_or(0);
                n += 1;
                let _ = write!(line, "dcl_tgsm_raw {}, {} bytes", o.text, bytes);
            }
            160 => {
                let o = parse_operand(&toks[..end], i + n, true);
                n += o.len;
                let stride = toks.get(i + n).copied().unwrap_or(0);
                let count = toks.get(i + n + 1).copied().unwrap_or(0);
                n += 2;
                let _ = write!(line, "dcl_tgsm_structured {}, stride {}, count {}", o.text, stride, count);
            }
            92 => {
                let _ = write!(line, "dcl_outputtopology {}", ctl & 0x7F);
            }
            93 => {
                let _ = write!(line, "dcl_inputprimitive {}", ctl & 0x3F);
            }
            94 => {
                let c = toks.get(i + n).copied().unwrap_or(0);
                n += 1;
                let _ = write!(line, "dcl_maxout {}", c);
            }
            _ => {
                // generic: operands until the instruction ends
                let mut ops = Vec::new();
                let sub = &toks[..end];
                while i + n < end {
                    let o = parse_operand(sub, i + n, as_int || (op == 55 && ops.is_empty()) || (op == 54 && false));
                    n += o.len;
                    ops.push(o.text);
                }
                // resinfo / sample etc. keep their names; control flow indents
                let _ = write!(line, "{} {}", mnem, ops.join(", "));
            }
        }
        if !ext_notes.is_empty() {
            let _ = write!(line, "   [{}]", ext_notes.join(" "));
        }
        // indentation for structured control flow
        match op {
            18 | 21 | 22 | 23 | 10 | 6 => {
                indent = indent.saturating_sub(1);
            }
            _ => {}
        }
        let _ = writeln!(out, "{}{}", "  ".repeat(indent), line.trim_end());
        match op {
            31 | 18 | 48 | 76 | 6 | 10 => indent += 1,
            _ => {}
        }
        let _ = pad;
        i += len.max(n);
    }
    out
}

fn interp_name(v: u32) -> &'static str {
    match v {
        0 => "undefined",
        1 => "constant",
        2 => "linear",
        3 => "linear centroid",
        4 => "linear noperspective",
        5 => "linear noperspective centroid",
        6 => "linear sample",
        7 => "linear noperspective sample",
        _ => "interp?",
    }
}

/// The whole story of one blob: chunks, reflection, signatures, code.
pub fn dump_blob(blob: &[u8]) -> String {
    let mut s = String::new();
    let cs = chunks(blob);
    let _ = writeln!(s, "DXBC {} bytes, chunks: {}", blob.len(), cs.iter().map(|c| format!("{}({})", c.fourcc, c.data.len())).collect::<Vec<_>>().join(" "));
    for c in &cs {
        match c.fourcc.as_str() {
            "RDEF" => {
                let _ = writeln!(s, "-- RDEF");
                s.push_str(&rdef_dump(c.data));
            }
            "ISGN" | "ISG1" | "OSGN" | "OSG5" | "OSG1" | "PCSG" | "PSG1" => {
                let _ = writeln!(s, "-- {}", c.fourcc);
                s.push_str(&signature_dump(c.data, &c.fourcc));
            }
            _ => {}
        }
    }
    for c in &cs {
        if c.fourcc == "SHEX" || c.fourcc == "SHDR" {
            let _ = writeln!(s, "-- {}", c.fourcc);
            s.push_str(&disassemble(c.data));
        }
    }
    s
}
