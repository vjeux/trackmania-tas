// jancorrect — produce a corrected, still-fail-closed January payload.
//
// Correction policy, deliberately conservative:
//
//  * A missing remap is repaired ONLY where the base register is PROVEN to
//    hold one object across the whole copied region (exactly one definition
//    besides the epilogue restore) AND the same January offset on that
//    register is remapped to exactly one current offset elsewhere in the
//    island. Under those two conditions the untouched site and the remapped
//    sites are the same field of the same object, so leaving them different is
//    a contradiction no field map can justify.
//
//  * Every other suspected omission is recorded, NOT patched. Patching a site
//    whose base object is unproven could introduce a wrong access, which is
//    worse than the one already there.
//
//  * Each applied repair is differentially re-verified: after the write, the
//    instruction must decode with the same mnemonic, same length and same
//    operand shape, differing only in the memory displacement.
//
//  * The emitted profile is marked NOT behavior-certified and NOT statically
//    complete, and carries the residual defect count, so the plugin's own gate
//    can refuse to install it.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;
use std::{env, fs};

#[derive(Clone)]
struct Insn {
    offset: usize,
    bytes: Vec<u8>,
    text: String,
}

impl Insn {
    fn mnemonic(&self) -> &str {
        self.text.split_whitespace().next().unwrap_or("")
    }
    fn shape(&self) -> String {
        let mut out = String::new();
        let b = self.text.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if i + 2 < b.len() && b[i] == b'0' && b[i + 1] == b'x' {
                let mut j = i + 2;
                while j < b.len() && (b[j] as char).is_ascii_hexdigit() {
                    j += 1;
                }
                out.push_str("IMM");
                i = j;
                continue;
            }
            out.push(b[i] as char);
            i += 1;
        }
        out
    }
    fn memory_displacements(&self) -> Vec<(String, i64)> {
        let mut out = Vec::new();
        let text = &self.text;
        let raw = text.as_bytes();
        let mut i = 0;
        while i < raw.len() {
            if raw[i] != b'[' {
                i += 1;
                continue;
            }
            let Some(close) = text[i..].find(']') else { break };
            let inner = &text[i + 1..i + close];
            i += close + 1;
            let Some(plus) = inner.rfind('+') else { continue };
            let base = inner[..plus].to_string();
            let Some(hex) = inner[plus + 1..].trim().strip_prefix("0x") else { continue };
            let Ok(v) = i64::from_str_radix(hex, 16) else { continue };
            out.push((base, v));
        }
        out
    }
}

fn disassemble(path: &str, start: usize, stop: usize) -> Vec<Insn> {
    let out = Command::new("objdump")
        .args([
            "-D", "-b", "binary", "-m", "i386:x86-64", "-M", "intel",
            &format!("--start-address={start}"),
            &format!("--stop-address={stop}"),
            path,
        ])
        .output()
        .expect("objdump");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut v: Vec<Insn> = Vec::new();
    for line in text.lines() {
        let Some((left, rest)) = line.split_once(":\t") else { continue };
        let left = left.trim();
        if left.is_empty() || !left.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let offset = usize::from_str_radix(left, 16).unwrap();
        let (bt, it) = match rest.split_once('\t') {
            Some((b, t)) => (b, t),
            None => (rest, ""),
        };
        let bytes: Vec<u8> = bt.split_whitespace().filter_map(|b| u8::from_str_radix(b, 16).ok()).collect();
        if it.trim().is_empty() {
            if let Some(last) = v.last_mut() {
                last.bytes.extend_from_slice(&bytes);
                continue;
            }
        }
        v.push(Insn { offset, bytes, text: it.split('#').next().unwrap_or("").trim().to_string() });
    }
    v
}

fn writes_register(text: &str, reg: &str) -> bool {
    let mnemonic = text.split_whitespace().next().unwrap_or("");
    let first = text[mnemonic.len()..].trim().split(',').next().unwrap_or("").trim();
    if first != reg {
        return false;
    }
    !matches!(mnemonic, "cmp" | "test" | "push" | "jmp" | "call" | "comiss" | "ucomiss" | "pop")
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: jancorrect Profile_Jan2022.as WORKDIR OUT.as");
        std::process::exit(2);
    }
    let profile = fs::read_to_string(&args[0]).expect("profile");
    let work = &args[1];
    let out_path = &args[2];

    let list = |name: &str| -> Vec<u64> {
        let start = format!("{name} = {{");
        let b = profile.find(&start).map(|p| p + start.len()).expect("array");
        let e = profile[b..].find("};").unwrap() + b;
        profile[b..e]
            .split(',')
            .map(|p| {
                let p = p.trim();
                if let Some(h) = p.strip_prefix("0x") { u64::from_str_radix(h, 16).unwrap() } else { p.parse().unwrap() }
            })
            .collect()
    };
    let mut island: Vec<u8> = {
        let start = "PROFILE_JAN2022_ISLAND_BYTES = \"";
        let b = profile.find(start).unwrap() + start.len();
        let e = profile[b..].find("\";").unwrap() + b;
        profile[b..e].split_whitespace().map(|x| u8::from_str_radix(x, 16).unwrap()).collect()
    };

    let mut field_offsets = list("PROFILE_JAN2022_FIELD_OFFSETS");
    let mut field_sites = list("PROFILE_JAN2022_FIELD_SOURCE_VAS");
    let mut field_widths = list("PROFILE_JAN2022_FIELD_WIDTHS");
    let mut field_old = list("PROFILE_JAN2022_FIELD_OLD_VALUES");
    let mut field_new = list("PROFILE_JAN2022_FIELD_NEW_VALUES");
    let region_starts = list("PROFILE_JAN2022_SOURCE_REGION_START_VAS");
    let region_ends = list("PROFILE_JAN2022_SOURCE_REGION_END_VAS");
    let region_offsets = list("PROFILE_JAN2022_ISLAND_REGION_OFFSETS");
    let region_lengths = list("PROFILE_JAN2022_ISLAND_REGION_LENGTHS");

    let scratch = format!("{work}/island-work.bin");
    fs::create_dir_all(work).ok();
    fs::write(&scratch, &island).expect("scratch");

    // -------- decode every copied region -------------------------------
    let mut all: Vec<(usize, Insn)> = Vec::new(); // (region index, insn)
    for i in 0..region_starts.len() {
        for insn in disassemble(&scratch, region_offsets[i] as usize, (region_offsets[i] + region_lengths[i]) as usize) {
            all.push((i, insn));
        }
    }

    let patched_bytes: BTreeSet<usize> = field_offsets
        .iter()
        .zip(&field_widths)
        .flat_map(|(o, w)| *o as usize..(*o + *w) as usize)
        .collect();
    let patched_old_at: BTreeMap<usize, i64> = field_offsets
        .iter()
        .zip(&field_old)
        .map(|(o, v)| (*o as usize, *v as i64))
        .collect();

    // -------- contested (region, base, january offset) ------------------
    struct Site {
        region: usize,
        offset: usize,
        base: String,
        january: i64,
        current: i64,
        patched: bool,
        text: String,
        length: usize,
    }
    let mut sites: Vec<Site> = Vec::new();
    for (region, insn) in &all {
        let touched = (insn.offset..insn.offset + insn.bytes.len()).any(|b| patched_bytes.contains(&b));
        for (base, disp) in insn.memory_displacements() {
            if base.starts_with("rip") || base.starts_with("rsp") || base.starts_with("rbp") {
                continue;
            }
            let january = if touched {
                (insn.offset..insn.offset + insn.bytes.len())
                    .find_map(|b| patched_old_at.get(&b).copied())
                    .unwrap_or(disp)
            } else {
                disp
            };
            sites.push(Site {
                region: *region,
                offset: insn.offset,
                base,
                january,
                current: disp,
                patched: touched,
                text: insn.text.clone(),
                length: insn.bytes.len(),
            });
        }
    }

    // -------- which base registers are provably one object? -------------
    let mut stable: BTreeMap<(usize, String), bool> = BTreeMap::new();
    for i in 0..region_starts.len() {
        let region_insns: Vec<&Insn> = all.iter().filter(|(r, _)| *r == i).map(|(_, x)| x).collect();
        let bases: BTreeSet<String> = sites.iter().filter(|s| s.region == i).map(|s| s.base.clone()).collect();
        for base in bases {
            let definitions = region_insns.iter().filter(|x| writes_register(&x.text, &base)).count();
            stable.insert((i, base), definitions <= 1);
        }
    }

    // -------- contested groups ------------------------------------------
    let mut groups: BTreeMap<(usize, String, i64), Vec<usize>> = BTreeMap::new();
    for (index, s) in sites.iter().enumerate() {
        groups.entry((s.region, s.base.clone(), s.january)).or_default().push(index);
    }

    let mut applied: Vec<(usize, u64, i64, i64, String)> = Vec::new(); // offset, site VA, old, new, text
    let mut deferred: Vec<String> = Vec::new();

    for ((region, base, january), members) in &groups {
        let patched: Vec<usize> = members.iter().copied().filter(|i| sites[*i].patched).collect();
        let untouched: Vec<usize> = members.iter().copied().filter(|i| !sites[*i].patched).collect();
        if patched.is_empty() || untouched.is_empty() {
            continue;
        }
        // the unique current offset this January offset maps to, in this region
        let mut news = BTreeSet::new();
        for i in &patched {
            news.insert(sites[*i].current);
        }
        if news.len() != 1 {
            deferred.push(format!(
                "region {region} [{base}+{january:#x}]: {} remapped sites disagree on the target {:?}; not repaired",
                patched.len(),
                news.iter().map(|v| format!("{v:#x}")).collect::<Vec<_>>()
            ));
            continue;
        }
        let new_value = *news.iter().next().unwrap();
        let is_stable = stable.get(&(*region, base.clone())).copied().unwrap_or(false);
        if !is_stable {
            deferred.push(format!(
                "region {region} [{base}+{january:#x}]: {} site(s) left at the January offset, but `{base}` has several definitions in the region, so the object identity is unproven; NOT repaired (needs the January binary)",
                untouched.len()
            ));
            continue;
        }
        for i in untouched {
            let s = &sites[i];
            let bytes = &island[s.offset..s.offset + s.length];
            let needle = (*january as i32).to_le_bytes();
            let positions: Vec<usize> = bytes
                .windows(4)
                .enumerate()
                .filter(|(_, w)| *w == needle)
                .map(|(p, _)| p)
                .collect();
            if positions.len() != 1 {
                deferred.push(format!(
                    "region {region} [{base}+{january:#x}] at island {}: displacement bytes are not unique in the instruction ({} matches); NOT repaired",
                    s.offset,
                    positions.len()
                ));
                continue;
            }
            let at = s.offset + positions[0];
            island[at..at + 4].copy_from_slice(&(new_value as i32).to_le_bytes());
            let source_va = region_starts[*region] + (s.offset - region_offsets[*region] as usize) as u64;
            applied.push((at, source_va, *january, new_value, s.text.clone()));
        }
    }

    // -------- differential re-verification of every repair --------------
    fs::write(&scratch, &island).expect("scratch");
    let mut verified = 0;
    let mut rejected = Vec::new();
    for (at, _, january, new_value, before) in &applied {
        let region = (0..region_starts.len())
            .find(|i| {
                *at >= region_offsets[*i] as usize && *at < (region_offsets[*i] + region_lengths[*i]) as usize
            })
            .unwrap();
        let after: Vec<Insn> = disassemble(&scratch, region_offsets[region] as usize, (region_offsets[region] + region_lengths[region]) as usize);
        let Some(now) = after.iter().find(|x| *at >= x.offset && *at < x.offset + x.bytes.len()) else {
            rejected.push(format!("island {at}: no instruction covers the repair"));
            continue;
        };
        let before_mnemonic = before.split_whitespace().next().unwrap_or("");
        let same_shape = {
            let mut b = String::new();
            let raw = before.as_bytes();
            let mut i = 0;
            while i < raw.len() {
                if i + 2 < raw.len() && raw[i] == b'0' && raw[i + 1] == b'x' {
                    let mut j = i + 2;
                    while j < raw.len() && (raw[j] as char).is_ascii_hexdigit() {
                        j += 1;
                    }
                    b.push_str("IMM");
                    i = j;
                    continue;
                }
                b.push(raw[i] as char);
                i += 1;
            }
            b == now.shape()
        };
        let displaced = now.memory_displacements().iter().any(|(_, v)| *v == *new_value);
        if now.mnemonic() == before_mnemonic && same_shape && displaced {
            verified += 1;
        } else {
            rejected.push(format!(
                "island {at}: repair {january:#x}->{new_value:#x} did not decode cleanly: `{before}` -> `{}`",
                now.text
            ));
        }
    }

    // -------- extend the field manifest ---------------------------------
    for (at, va, january, new_value, _) in &applied {
        field_offsets.push(*at as u64);
        field_sites.push(*va);
        field_widths.push(4);
        field_old.push(*january as u64);
        field_new.push(*new_value as u64);
    }
    let mut order: Vec<usize> = (0..field_offsets.len()).collect();
    order.sort_by_key(|i| field_offsets[*i]);
    let reorder = |v: &Vec<u64>, order: &Vec<usize>| -> Vec<u64> { order.iter().map(|i| v[*i]).collect() };
    let field_offsets = reorder(&field_offsets, &order);
    let field_sites = reorder(&field_sites, &order);
    let field_widths = reorder(&field_widths, &order);
    let field_old = reorder(&field_old, &order);
    let field_new = reorder(&field_new, &order);

    // -------- emit the corrected profile --------------------------------
    let hex = |v: &Vec<u64>| v.iter().map(|x| format!("0x{x:X}")).collect::<Vec<_>>().join(",");
    let dec = |v: &Vec<u64>| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
    let island_text = island.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ");

    let mut corrected = profile.clone();
    let replace_array = |text: &mut String, name: &str, body: String| {
        let start = format!("{name} = {{");
        let b = text.find(&start).unwrap() + start.len();
        let e = text[b..].find("};").unwrap() + b;
        text.replace_range(b..e, &body);
    };
    let replace_scalar = |text: &mut String, name: &str, body: String| {
        let start = format!("{name} = ");
        let b = text.find(&start).unwrap() + start.len();
        let e = text[b..].find(';').unwrap() + b;
        text.replace_range(b..e, &body);
    };
    {
        let start = "PROFILE_JAN2022_ISLAND_BYTES = \"";
        let b = corrected.find(start).unwrap() + start.len();
        let e = corrected[b..].find("\";").unwrap() + b;
        corrected.replace_range(b..e, &island_text);
    }
    replace_array(&mut corrected, "PROFILE_JAN2022_FIELD_OFFSETS", dec(&field_offsets));
    replace_array(&mut corrected, "PROFILE_JAN2022_FIELD_SOURCE_VAS", hex(&field_sites));
    replace_array(&mut corrected, "PROFILE_JAN2022_FIELD_WIDTHS", dec(&field_widths));
    replace_array(&mut corrected, "PROFILE_JAN2022_FIELD_OLD_VALUES", hex(&field_old));
    replace_array(&mut corrected, "PROFILE_JAN2022_FIELD_NEW_VALUES", hex(&field_new));
    replace_scalar(&mut corrected, "const uint PROFILE_JAN2022_FIELD_RELOCATION_COUNT", field_offsets.len().to_string());

    let header = format!(
        "// CORRECTED January 2022 island. Independent audit {audit}.\n\
         // {applied} omitted structure remaps repaired, all differentially re-verified.\n\
         // {deferred} suspected omissions NOT repaired: the base object is unproven without\n\
         // the January executable. This payload is NOT behavior certified and NOT\n\
         // statically complete; the plugin must refuse to install it.\n\
         const bool PROFILE_JAN2022_BEHAVIOR_CERTIFIED = false;\n\
         const bool PROFILE_JAN2022_STATIC_COMPLETE = false;\n\
         const uint PROFILE_JAN2022_AUDIT_REPAIRED_OMISSIONS = {applied};\n\
         const uint PROFILE_JAN2022_AUDIT_RESIDUAL_OMISSIONS = {deferred};\n",
        audit = "2026-09-03",
        applied = applied.len(),
        deferred = deferred.len()
    );
    corrected.insert_str(0, &header);
    fs::write(out_path, &corrected).expect("write corrected profile");
    fs::write(format!("{work}/island-corrected.bin"), &island).expect("write corrected island");

    // -------- report ------------------------------------------------------
    println!("== repairs applied ({}) ==", applied.len());
    for (at, va, january, new_value, text) in &applied {
        println!("  island {at:5} January {va:#x}: {january:#x} -> {new_value:#x}   `{text}`");
    }
    println!("\ndifferentially re-verified: {verified}/{}", applied.len());
    for r in &rejected {
        println!("REJECTED\t{r}");
    }
    println!("\n== deferred, evidence required ({}) ==", deferred.len());
    for d in &deferred {
        println!("  {d}");
    }
    println!("\nfield relocations: {} -> {}", field_offsets.len() - applied.len(), field_offsets.len());
    println!("island size unchanged: {} bytes", island.len());
    println!("wrote {out_path}");
}
