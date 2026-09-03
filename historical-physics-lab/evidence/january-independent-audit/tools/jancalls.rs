// jancalls — helper identity by reachability.
//
// The generator redirects exactly ONE root call site per copied helper. If a
// January helper was called from more sites than the hand-written table lists,
// the extra sites fall through to the generic aligner and end up calling a
// CURRENT function instead of the copied January code. A copied helper that
// nothing calls is the same bug seen from the other end.

use std::collections::BTreeMap;
use std::fs;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let profile = fs::read_to_string(&args[0]).expect("profile");
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
    let scalar = |name: &str| -> u64 {
        let start = format!("{name} = ");
        let b = profile.find(&start).map(|p| p + start.len()).unwrap();
        let e = profile[b..].find(';').unwrap() + b;
        profile[b..e].trim().parse().unwrap()
    };

    let call_offsets = list("PROFILE_JAN2022_CALL_RELOC_OFFSETS");
    let call_targets = list("PROFILE_JAN2022_CALL_TARGET_ISLAND_OFFSETS");
    let abs_offsets = list("PROFILE_JAN2022_ABS64_OFFSETS");
    let region_starts = list("PROFILE_JAN2022_SOURCE_REGION_START_VAS");
    let region_ends = list("PROFILE_JAN2022_SOURCE_REGION_END_VAS");
    let region_offsets = list("PROFILE_JAN2022_ISLAND_REGION_OFFSETS");
    let region_lengths = list("PROFILE_JAN2022_ISLAND_REGION_LENGTHS");
    let adapter = scalar("PROFILE_JAN2022_INTERPOLATION_ADAPTER_OFFSET");

    let thunk_entries: Vec<u64> = abs_offsets.iter().map(|o| o - 2).collect();
    let mut hits: BTreeMap<u64, usize> = BTreeMap::new();
    for t in &call_targets {
        *hits.entry(*t).or_insert(0) += 1;
    }

    println!("== call-target distribution ==");
    let to_thunk: usize = call_targets.iter().filter(|t| thunk_entries.contains(t)).count();
    let to_adapter: usize = call_targets.iter().filter(|t| **t == adapter).count();
    let region_entry: Vec<u64> = region_offsets.clone();
    let to_region: usize = call_targets.iter().filter(|t| region_entry.contains(t)).count();
    println!("total call relocations: {}", call_targets.len());
    println!("  -> absolute thunk (external, current image): {to_thunk}");
    println!("  -> copied region entry (in-island January code): {to_region}");
    println!("  -> interpolation adapter: {to_adapter}");
    let other = call_targets.len() - to_thunk - to_region - to_adapter;
    println!("  -> anything else: {other}");

    println!("\n== reachability of every copied region ==");
    for i in 0..region_starts.len() {
        let entry = region_offsets[i];
        let callers = hits.get(&entry).copied().unwrap_or(0);
        let role = if i == 0 { "root handler (entered by the patched CarSport entry)" } else { "copied helper" };
        let flag = if i > 0 && callers == 0 {
            "  <-- DEAD COPY: nothing in the island calls it"
        } else if i > 0 && callers > 1 {
            "  <-- multiple call sites redirected here"
        } else {
            ""
        };
        println!(
            "region {i:2}: source {:#x}..{:#x} island {entry:5} len {:4}  callers={callers}  {role}{flag}",
            region_starts[i], region_ends[i], region_lengths[i]
        );
    }

    println!("\n== thunk fan-in ==");
    let mut thunk_hits = 0;
    for t in &thunk_entries {
        let c = hits.get(t).copied().unwrap_or(0);
        if c == 0 {
            println!("thunk at {t}: UNUSED");
        } else {
            thunk_hits += c;
        }
    }
    println!("{thunk_hits} call sites leave through {} thunks", thunk_entries.len());
}
