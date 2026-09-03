use std::{collections::BTreeSet, env, fs};

#[derive(Clone, Copy)]
struct Patch {
    off: usize,
    old: &'static [u8],
    new: &'static [u8],
    class: &'static str,
    evidence: &'static str,
}

const DIRECT_PATCHES: &[Patch] = &[
    Patch {
        off: 0x0119,
        old: &[0x49, 0x81, 0xc1, 0xe0, 0x12, 0, 0],
        new: &[0x49, 0x81, 0xc1, 0x98, 0x12, 0, 0],
        class: "field_immediate",
        evidence: "current 0x852048",
    },
    Patch {
        off: 0x01b1,
        old: &[0xba, 0x30, 0x0d, 0, 0],
        new: &[0xba, 0x40, 0x0d, 0, 0],
        class: "field_immediate",
        evidence: "current 0x8520cb",
    },
    Patch {
        off: 0x01b6,
        old: &[0xb9, 0xe0, 0x0c, 0, 0],
        new: &[0xb9, 0xf0, 0x0c, 0, 0],
        class: "field_immediate",
        evidence: "current 0x8520d0; first native fault",
    },
    Patch {
        off: 0x02da,
        old: &[0x48, 0x81, 0xc2, 0xf0, 0x15, 0, 0],
        new: &[0x48, 0x81, 0xc2, 0xa8, 0x15, 0, 0],
        class: "field_immediate",
        evidence: "current 0x85220e",
    },
    Patch {
        off: 0x03c6,
        old: &[0x48, 0x81, 0xc1, 0x30, 0x06, 0, 0],
        new: &[0x48, 0x81, 0xc1, 0x38, 0x06, 0, 0],
        class: "field_immediate",
        evidence: "current object band",
    },
    Patch {
        off: 0x0880,
        old: &[0x48, 0x81, 0xc1, 0xd8, 0x1e, 0, 0],
        new: &[0x48, 0x81, 0xc1, 0x90, 0x1e, 0, 0],
        class: "field_immediate",
        evidence: "current 0x85272c",
    },
    Patch {
        off: 0x0ec2,
        old: &[0x48, 0x81, 0xc1, 0x50, 0x1d, 0, 0],
        new: &[0x48, 0x81, 0xc1, 0x08, 0x1d, 0, 0],
        class: "field_immediate",
        evidence: "current 0x852d6e",
    },
    Patch {
        off: 0x16f2,
        old: &[0x48, 0x81, 0xc1, 0xe0, 0x30, 0, 0],
        new: &[0x48, 0x81, 0xc1, 0x98, 0x2d, 0, 0],
        class: "field_immediate",
        evidence: "current 0x8537b9",
    },
    Patch {
        off: 0x0352,
        old: &[0x83, 0xbf, 0x9c, 0x14, 0, 0, 0],
        new: &[0x83, 0xbf, 0x8c, 0x13, 0, 0, 0],
        class: "field_modrm",
        evidence: "current 0x85393c",
    },
    Patch {
        off: 0x03d5,
        old: &[0xf3, 0x0f, 0x10, 0x8f, 0xb4, 0x14, 0, 0],
        new: &[0xf3, 0x0f, 0x10, 0x8f, 0xa4, 0x13, 0, 0],
        class: "field_modrm",
        evidence: "current 0x852824",
    },
    Patch {
        off: 0x03f5,
        old: &[0xf3, 0x0f, 0x11, 0x8f, 0xb4, 0x14, 0, 0],
        new: &[0xf3, 0x0f, 0x11, 0x8f, 0xa4, 0x13, 0, 0],
        class: "field_modrm",
        evidence: "current state-object band",
    },
    Patch {
        off: 0x076f,
        old: &[0xf3, 0x0f, 0x59, 0xbf, 0x8c, 0x15, 0, 0],
        new: &[0xf3, 0x0f, 0x59, 0xbf, 0x7c, 0x14, 0, 0],
        class: "field_modrm",
        evidence: "current 0x852623",
    },
    Patch {
        off: 0x0850,
        old: &[0xf3, 0x45, 0x0f, 0x59, 0x86, 0x38, 0x1d, 0, 0],
        new: &[0xf3, 0x45, 0x0f, 0x59, 0x86, 0xf0, 0x1c, 0, 0],
        class: "field_modrm",
        evidence: "current 0x852c80; tuning-object band",
    },
    Patch {
        off: 0x0f7a,
        old: &[0x48, 0x8d, 0x9f, 0x1c, 0x1d, 0, 0],
        new: &[0x48, 0x8d, 0x9f, 0x2c, 0x1c, 0, 0],
        class: "field_modrm",
        evidence: "current 0x852e09",
    },
    Patch {
        off: 0x1076,
        old: &[0xf3, 0x41, 0x0f, 0x59, 0xa4, 0x24, 0x14, 0x0a, 0, 0],
        new: &[0xf3, 0x41, 0x0f, 0x59, 0xa4, 0x24, 0x5c, 0x0a, 0, 0],
        class: "field_modrm",
        evidence: "current 0x852f29",
    },
    Patch {
        off: 0x11a4,
        old: &[0xf3, 0x0f, 0x59, 0x99, 0x14, 0x0a, 0, 0],
        new: &[0xf3, 0x0f, 0x59, 0x99, 0x5c, 0x0a, 0, 0],
        class: "field_modrm",
        evidence: "current 0x853045",
    },
    Patch {
        off: 0x127d,
        old: &[0xf3, 0x0f, 0x59, 0xbf, 0x40, 0x15, 0, 0],
        new: &[0xf3, 0x0f, 0x59, 0xbf, 0x30, 0x14, 0, 0],
        class: "field_modrm",
        evidence: "current 0x853121",
    },
    Patch {
        off: 0x13be,
        old: &[0x44, 0x3b, 0xa7, 0x1c, 0x15, 0, 0],
        new: &[0x44, 0x3b, 0xa7, 0x0c, 0x14, 0, 0],
        class: "field_modrm",
        evidence: "current 0x8534b9",
    },
    Patch {
        off: 0x05da,
        old: &[0x4c, 0x8b, 0xac, 0xc8, 0x70, 0x06, 0, 0],
        new: &[0x4c, 0x8b, 0xac, 0xc8, 0xb8, 0x06, 0, 0],
        class: "field_modrm",
        evidence: "current 0x852479 material table",
    },
    Patch {
        off: 0x0605,
        old: &[0xf3, 0x41, 0x0f, 0x10, 0x47, 0x10],
        new: &[0xf3, 0x41, 0x0f, 0x10, 0x47, 0x18],
        class: "field_modrm",
        evidence: "current 0x8524da contact struct",
    },
    Patch {
        off: 0x0633,
        old: &[0xf3, 0x41, 0x0f, 0x10, 0x47, 0x18],
        new: &[0xf3, 0x41, 0x0f, 0x10, 0x47, 0x48],
        class: "field_modrm",
        evidence: "current 0x8524eb contact struct",
    },
    Patch {
        off: 0x077b,
        old: &[0xf3, 0x44, 0x0f, 0x59, 0x9f, 0x7c, 0x14, 0, 0],
        new: &[0xf3, 0x44, 0x0f, 0x59, 0x9f, 0x80, 0x14, 0, 0],
        class: "field_modrm",
        evidence: "current 0x85262f",
    },
    Patch {
        off: 0x0784,
        old: &[0xf3, 0x0f, 0x59, 0xb7, 0x94, 0x15, 0, 0],
        new: &[0xf3, 0x0f, 0x59, 0xb7, 0x84, 0x14, 0, 0],
        class: "field_modrm",
        evidence: "current 0x852637",
    },
    Patch {
        off: 0x09e6,
        old: &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0x70, 0x19, 0, 0],
        new: &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0x28, 0x19, 0, 0],
        class: "field_modrm",
        evidence: "current 0x852891 tuning-object band",
    },
    Patch {
        off: 0x09f1,
        old: &[0xf3, 0x45, 0x0f, 0x10, 0x86, 0x40, 0x1d, 0, 0],
        new: &[0xf3, 0x45, 0x0f, 0x10, 0x86, 0xf8, 0x1c, 0, 0],
        class: "field_modrm",
        evidence: "current 0x85289a tuning-object band",
    },
    Patch {
        off: 0x10a6,
        old: &[0xf3, 0x0f, 0x59, 0x97, 0xec, 0x15, 0, 0],
        new: &[0xf3, 0x0f, 0x59, 0x97, 0xdc, 0x14, 0, 0],
        class: "field_modrm",
        evidence: "current 0x14085068a",
    },
    Patch {
        off: 0x1116,
        old: &[0xf3, 0x41, 0x0f, 0x10, 0x8e, 0x3c, 0x1a, 0, 0],
        new: &[0xf3, 0x41, 0x0f, 0x10, 0x8e, 0xf4, 0x19, 0, 0],
        class: "field_modrm",
        evidence: "current 0x852fb8",
    },
    Patch {
        off: 0x1203,
        old: &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0x94, 0x0d, 0, 0],
        new: &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0xa4, 0x0d, 0, 0],
        class: "field_modrm",
        evidence: "current 0x8530a5",
    },
    Patch {
        off: 0x120e,
        old: &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0x90, 0x0d, 0, 0],
        new: &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0xa0, 0x0d, 0, 0],
        class: "field_modrm",
        evidence: "current 0x8530b0",
    },
    Patch {
        off: 0x1235,
        old: &[0xf3, 0x45, 0x0f, 0x10, 0x86, 0xc4, 0x18, 0, 0],
        new: &[0xf3, 0x45, 0x0f, 0x10, 0x86, 0x7c, 0x18, 0, 0],
        class: "field_modrm",
        evidence: "current 0x8530d8",
    },
    Patch {
        off: 0x12d3,
        old: &[0xf3, 0x41, 0x0f, 0x10, 0x84, 0x24, 0x08, 0x0a, 0, 0],
        new: &[0xf3, 0x41, 0x0f, 0x10, 0x84, 0x24, 0x50, 0x0a, 0, 0],
        class: "field_modrm",
        evidence: "current 0x852f29",
    },
    Patch {
        off: 0x12dd,
        old: &[0xf3, 0x0f, 0x59, 0x87, 0x74, 0x18, 0, 0],
        new: &[0xf3, 0x0f, 0x59, 0x87, 0x54, 0x17, 0, 0],
        class: "field_modrm",
        evidence: "current 0x853178",
    },
    Patch {
        off: 0x13da,
        old: &[0xf3, 0x45, 0x0f, 0x5e, 0xbe, 0xd4, 0x1c, 0, 0],
        new: &[0xf3, 0x45, 0x0f, 0x5e, 0xbe, 0x8c, 0x1c, 0, 0],
        class: "field_modrm",
        evidence: "current 0x8534d6",
    },
    Patch {
        off: 0x1508,
        old: &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0xd0, 0x1e, 0, 0],
        new: &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0x88, 0x1e, 0, 0],
        class: "field_modrm",
        evidence: "current 0x853603",
    },
    Patch {
        off: 0x155c,
        old: &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0x2c, 0x1f, 0, 0],
        new: &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0xe4, 0x1e, 0, 0],
        class: "field_modrm",
        evidence: "current 0x853657",
    },
    Patch {
        off: 0x1581,
        old: &[0xf3, 0x45, 0x0f, 0x59, 0xb6, 0x38, 0x1f, 0, 0],
        new: &[0xf3, 0x45, 0x0f, 0x59, 0xb6, 0xf0, 0x1e, 0, 0],
        class: "field_modrm",
        evidence: "current 0x85367c",
    },
    Patch {
        off: 0x1710,
        old: &[0xf3, 0x0f, 0x10, 0x8f, 0xe8, 0x14, 0, 0],
        new: &[0xf3, 0x0f, 0x10, 0x8f, 0xd8, 0x13, 0, 0],
        class: "field_modrm",
        evidence: "current 0x8537d7",
    },
    Patch {
        off: 0x1741,
        old: &[0xf3, 0x41, 0x0f, 0x10, 0x84, 0x24, 0x04, 0x0a, 0, 0],
        new: &[0xf3, 0x41, 0x0f, 0x10, 0x84, 0x24, 0x4c, 0x0a, 0, 0],
        class: "field_modrm",
        evidence: "current 0x853809",
    },
    Patch {
        off: 0x174b,
        old: &[0xf3, 0x0f, 0x59, 0x87, 0x70, 0x18, 0, 0],
        new: &[0xf3, 0x0f, 0x59, 0x87, 0x50, 0x17, 0, 0],
        class: "field_modrm",
        evidence: "current 0x853813",
    },
    Patch {
        off: 0x18c6,
        old: &[0xf3, 0x0f, 0x10, 0x87, 0xe8, 0x14, 0, 0],
        new: &[0xf3, 0x0f, 0x10, 0x87, 0xd8, 0x13, 0, 0],
        class: "field_modrm",
        evidence: "current 0x853974",
    },
    Patch {
        off: 0x1bff,
        old: &[0xf3, 0x0f, 0x10, 0x82, 0x44, 0x1d, 0, 0],
        new: &[0xf3, 0x0f, 0x10, 0x82, 0xfc, 0x1c, 0, 0],
        class: "field_modrm",
        evidence: "current 0x853cb4",
    },
    Patch {
        off: 0x1c09,
        old: &[0xf3, 0x0f, 0x10, 0x82, 0x48, 0x1d, 0, 0],
        new: &[0xf3, 0x0f, 0x10, 0x82, 0x00, 0x1d, 0, 0],
        class: "field_modrm",
        evidence: "current 0x853cbe",
    },
    Patch {
        off: 0x2259,
        old: &[0x49, 0x8d, 0x90, 0xc0, 0x16, 0, 0],
        new: &[0x49, 0x8d, 0x90, 0xc0, 0x17, 0, 0],
        class: "field_modrm",
        evidence: "current 0x8417ed helper wheel array",
    },
];

const UNCHANGED_IMMEDIATES: &[(usize, &[u8], &str)] = &[
    (0x442, &[0x48, 0x81, 0xc1, 0xb8, 0, 0, 0], "wheel stride"),
    (0xbed, &[0x48, 0x81, 0xc3, 0xb8, 0, 0, 0], "wheel stride"),
    (0xf81, &[0x49, 0x83, 0xc5, 0x70], "wheel member"),
    (0x117c, &[0x3c, 0x4a], "surface id"),
    (0x134f, &[0x49, 0x81, 0xc5, 0xb8, 0, 0, 0], "wheel stride"),
    (0x1bab, &[0x48, 0x81, 0xc1, 0xb8, 0, 0, 0], "wheel stride"),
];
const UNCHANGED_MODRM: &[(usize, &[u8], &str)] = &[
    (0x1eb, &[0xf3, 0x0f, 0x10, 0x5e, 0x18], "current 0x852101"),
    (0x290, &[0xf3, 0x0f, 0x10, 0x76, 0x18], "current 0x8521c8"),
    (
        0xa2e,
        &[0xf3, 0x41, 0x0f, 0x59, 0x4d, 0x24],
        "current 0x8528d9",
    ),
    (
        0x1a3a,
        &[0xf3, 0x41, 0x0f, 0x59, 0x4b, 0x18],
        "current 0x853ae7",
    ),
    (
        0x1ae3,
        &[0xf3, 0x41, 0x0f, 0x10, 0x43, 0x18],
        "current 0x853b98",
    ),
    (
        0x1c11,
        &[0xf3, 0x45, 0x0f, 0x59, 0x63, 0x18],
        "current 0x853ccb",
    ),
    (0x1c65, &[0x41, 0x0f, 0x2f, 0x43, 0x18], "current 0x853d13"),
    (0x1e07, &[0xf3, 0x0f, 0x59, 0x4e, 0x24], "current 0x853ea7"),
    (0x1f87, &[0xf3, 0x0f, 0x10, 0x46, 0x18], "current 0x854026"),
];

const ORIGINAL_HEADER: &str = "// Generated Fall 2022 native island: exact Sep. 30 routine, build-128130 field offsets and native targets.\nconst uint PROFILE_FALL2022_ISLAND_SIZE = 9916;\nconst uint PROFILE_FALL2022_UNRESOLVED_CALLS = 0;\nconst uint PROFILE_FALL2022_UNRESOLVED_RIP = 0;";
const FINAL_HEADER: &str = "// Generated Fall 2022 native island: exact Sep. 30 routine, exhaustively adapted for build 128130.\nconst uint PROFILE_FALL2022_ISLAND_SIZE = 9916;\nconst uint PROFILE_FALL2022_UNRESOLVED_CALLS = 0;\nconst uint PROFILE_FALL2022_UNRESOLVED_RIP = 0;\nconst uint PROFILE_FALL2022_FIELD_REMAP_COUNT = 44;\nconst uint PROFILE_FALL2022_DIRECT_FIELD_REMAP_COUNT = 43;\nconst uint PROFILE_FALL2022_HELPER_FIELD_REMAP_COUNT = 1;\nconst uint PROFILE_FALL2022_AUDITED_IMMEDIATE_COUNT = 15;\nconst uint PROFILE_FALL2022_REWRITTEN_IMMEDIATE_COUNT = 9;\nconst uint PROFILE_FALL2022_PROVEN_UNCHANGED_IMMEDIATE_COUNT = 6;\nconst uint PROFILE_FALL2022_PROVEN_UNCHANGED_MODRM_COUNT = 9;\nconst uint PROFILE_FALL2022_ABI_ADAPTER_COUNT = 2;\nconst uint PROFILE_FALL2022_RELOCATED_CALL_COUNT = 155;\nconst uint PROFILE_FALL2022_ABS64_THUNK_COUNT = 40;\nconst bool PROFILE_FALL2022_BEHAVIOR_CERTIFIED = false;";

fn between<'a>(s: &'a str, a: &str, b: &str) -> (usize, usize, &'a str) {
    let p = s.find(a).expect("start marker") + a.len();
    let q = s[p..].find(b).expect("end marker") + p;
    (p, q, &s[p..q])
}
fn parse_profile(s: &str) -> Vec<u8> {
    between(s, "PROFILE_FALL2022_ISLAND_BYTES = \"", "\";")
        .2
        .split_whitespace()
        .map(|x| u8::from_str_radix(x, 16).expect("hex byte"))
        .collect()
}
fn fmt_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
fn fmt_slice(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
fn apply(bytes: &mut [u8], patch: Patch, used: &mut BTreeSet<usize>) {
    assert_eq!(
        &bytes[patch.off..patch.off + patch.old.len()],
        patch.old,
        "preimage mismatch at 0x{:04x}: {}",
        patch.off,
        patch.evidence
    );
    assert_eq!(
        patch.old.len(),
        patch.new.len(),
        "length at 0x{:04x}",
        patch.off
    );
    for i in patch.off..patch.off + patch.old.len() {
        assert!(used.insert(i), "overlapping patch byte 0x{i:04x}");
    }
    bytes[patch.off..patch.off + patch.new.len()].copy_from_slice(patch.new);
}
fn apply_raw(
    bytes: &mut [u8],
    off: usize,
    old: &[u8],
    new: &[u8],
    used: &mut BTreeSet<usize>,
    name: &str,
) {
    apply(
        bytes,
        Patch {
            off,
            old: Box::leak(old.to_vec().into_boxed_slice()),
            new: Box::leak(new.to_vec().into_boxed_slice()),
            class: "abi_adapter",
            evidence: Box::leak(name.to_string().into_boxed_str()),
        },
        used,
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "usage: remap_fall_island INPUT.as OUTPUT.as AUDIT.tsv"
    );
    let source = fs::read_to_string(&args[1]).expect("read input profile");
    assert_eq!(
        source.matches(ORIGINAL_HEADER).count(),
        1,
        "untouched header preimage"
    );
    let mut bytes = parse_profile(&source);
    assert_eq!(bytes.len(), 9916, "island size");
    let mut used = BTreeSet::new();
    for patch in DIRECT_PATCHES {
        apply(&mut bytes, *patch, &mut used);
    }

    apply_raw(
        &mut bytes,
        0x239c,
        &[0x48, 0x83, 0xc1, 0x78],
        &[0xeb, 0x66, 0x90, 0x90],
        &mut used,
        "helper +0x78 branch",
    );
    apply_raw(
        &mut bytes,
        0x2404,
        &[0xcc; 12],
        &[
            0x48, 0x81, 0xc1, 0x88, 0, 0, 0, 0xe9, 0x90, 0xff, 0xff, 0xff,
        ],
        &mut used,
        "helper +0x88 cave",
    );
    apply_raw(
        &mut bytes,
        0x0d5,
        &[0x48, 0x89, 0x45, 0x60, 0xe8, 0x62, 0xcf, 0xfe, 0xff],
        &[0xe9, 0x36, 0x23, 0, 0, 0x90, 0x90, 0x90, 0x90],
        &mut used,
        "relocate persistent pointer store",
    );
    apply_raw(
        &mut bytes,
        0x1aba,
        &[0x48, 0x8b, 0x45, 0x60, 0x48, 0x8d, 0x8f, 0x80, 0x12, 0, 0],
        &[0xe9, 0x62, 0x09, 0, 0, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
        &mut used,
        "relocate persistent pointer load",
    );
    apply_raw(
        &mut bytes,
        0x2410,
        &[0; 17],
        &[
            0x48, 0x89, 0x85, 0xa0, 0, 0, 0, 0xe8, 0x44, 0, 0, 0, 0xe9, 0xbd, 0xdc, 0xff, 0xff,
        ],
        &mut used,
        "persistent pointer store cave",
    );
    apply_raw(
        &mut bytes,
        0x2421,
        &[0; 12],
        &[
            0x48, 0x8b, 0x85, 0xa0, 0, 0, 0, 0xe9, 0x77, 0xfc, 0xff, 0xff,
        ],
        &mut used,
        "persistent pointer load cave",
    );
    apply_raw(
        &mut bytes,
        0x20a4,
        &[0xcc; 12],
        &[
            0x48, 0x8d, 0x8f, 0x80, 0x12, 0, 0, 0xe9, 0x15, 0xfa, 0xff, 0xff,
        ],
        &mut used,
        "restore displaced pointer-load instruction",
    );
    apply_raw(
        &mut bytes,
        0x2430,
        &[0x48, 0x8d, 0x05, 0xd9, 0xff, 0xff, 0xff],
        &[0x48, 0x8d, 0x45, 0xd0, 0x90, 0x90, 0x90],
        &mut used,
        "current helper output scratch",
    );

    for (off, expected, name) in UNCHANGED_IMMEDIATES.iter().chain(UNCHANGED_MODRM.iter()) {
        assert_eq!(
            &bytes[*off..*off + expected.len()],
            *expected,
            "unchanged {name} at 0x{off:04x}"
        );
    }

    let (p, q, _) = between(&source, "PROFILE_FALL2022_ISLAND_BYTES = \"", "\";");
    let mut output = String::with_capacity(source.len() + 512);
    output.push_str(&source[..p]);
    output.push_str(&fmt_bytes(&bytes));
    output.push_str(&source[q..]);
    output = output.replacen(ORIGINAL_HEADER, FINAL_HEADER, 1);
    let old_reloc = "{169,202,218,301,";
    let new_reloc = "{169,202,9240,301,";
    assert_eq!(
        output.matches(old_reloc).count(),
        1,
        "relocation-offset preimage"
    );
    output = output.replacen(old_reloc, new_reloc, 1);
    fs::write(&args[2], output).expect("write output profile");

    let mut audit = String::from("island_offset\tclass\told_bytes\tnew_bytes\tevidence\n");
    for patch in DIRECT_PATCHES {
        audit.push_str(&format!(
            "0x{:04x}\t{}\t{}\t{}\t{}\n",
            patch.off,
            patch.class,
            fmt_slice(patch.old),
            fmt_slice(patch.new),
            patch.evidence
        ));
    }
    audit.push_str("0x239c\thelper_field\t48 83 C1 78\t48 81 C1 88 (via +0x2404 cave)\tcurrent material-curve layout at 0x140845a44\n");
    for (off, expected, evidence) in UNCHANGED_IMMEDIATES {
        audit.push_str(&format!(
            "0x{off:04x}\tproven_unchanged_immediate\t{}\t{}\t{evidence}\n",
            fmt_slice(expected),
            fmt_slice(expected)
        ));
    }
    for (off, expected, evidence) in UNCHANGED_MODRM {
        audit.push_str(&format!(
            "0x{off:04x}\tproven_unchanged_modrm\t{}\t{}\t{evidence}\n",
            fmt_slice(expected),
            fmt_slice(expected)
        ));
    }
    audit.push_str("0x00d5\tabi_adapter\tpointer at [rbp+0x60]\tpointer at [rbp+0xa0]\tcurrent 0x1408456b0 output struct overwrites old +0x60 slot\n");
    audit.push_str("0x2430\tabi_adapter\toutput in island tail\toutput at [rbp-0x30]\tcurrent helper gained an output-pointer argument\n");
    fs::write(&args[3], audit).expect("write audit manifest");
    println!("generated Fall island: 43 direct field remaps, 1 helper field remap, 2 ABI adapters, {} bytes", bytes.len());
}
