use std::{env, fs};

fn between<'a>(s: &'a str, a: &str, b: &str) -> &'a str {
    let p = s.find(a).expect("start marker") + a.len();
    let q = s[p..].find(b).expect("end marker") + p;
    &s[p..q]
}
fn bytes(s: &str) -> Vec<u8> {
    between(s, "PROFILE_FALL2022_ISLAND_BYTES = \"", "\";")
        .split_whitespace()
        .map(|x| u8::from_str_radix(x, 16).expect("hex byte"))
        .collect()
}
fn at(bytes: &[u8], off: usize, expected: &[u8], name: &str) {
    assert_eq!(
        &bytes[off..off + expected.len()],
        expected,
        "{name} at 0x{off:04x}"
    );
}
fn uint_array(s: &str, name: &str) -> Vec<u64> {
    let marker = format!("{name} = {{");
    between(s, &marker, "};")
        .split(',')
        .filter(|x| !x.trim().is_empty())
        .map(|x| {
            let x = x.trim();
            x.strip_prefix("0x")
                .map(|h| u64::from_str_radix(h, 16).unwrap())
                .unwrap_or_else(|| x.parse().unwrap())
        })
        .collect()
}

fn main() {
    let path = env::args().nth(1).expect("Profile_Fall2022.as");
    let source = fs::read_to_string(path).expect("read profile");
    let island = bytes(&source);
    assert_eq!(island.len(), 9916);

    let rewritten: &[(usize, &[u8], &str)] = &[
        (0x0119, &[0x49, 0x81, 0xc1, 0x98, 0x12, 0, 0], "12e0->1298"),
        (0x01b1, &[0xba, 0x40, 0x0d, 0, 0], "d30->d40"),
        (0x01b6, &[0xb9, 0xf0, 0x0c, 0, 0], "ce0->cf0"),
        (0x02da, &[0x48, 0x81, 0xc2, 0xa8, 0x15, 0, 0], "15f0->15a8"),
        (0x03c6, &[0x48, 0x81, 0xc1, 0x38, 0x06, 0, 0], "630->638"),
        (0x0880, &[0x48, 0x81, 0xc1, 0x90, 0x1e, 0, 0], "1ed8->1e90"),
        (0x0ec2, &[0x48, 0x81, 0xc1, 0x08, 0x1d, 0, 0], "1d50->1d08"),
        (0x16f2, &[0x48, 0x81, 0xc1, 0x98, 0x2d, 0, 0], "30e0->2d98"),
        (0x0352, &[0x83, 0xbf, 0x8c, 0x13, 0, 0, 0], "149c->138c"),
        (
            0x03d5,
            &[0xf3, 0x0f, 0x10, 0x8f, 0xa4, 0x13, 0, 0],
            "14b4->13a4 read",
        ),
        (
            0x03f5,
            &[0xf3, 0x0f, 0x11, 0x8f, 0xa4, 0x13, 0, 0],
            "14b4->13a4 write",
        ),
        (
            0x076f,
            &[0xf3, 0x0f, 0x59, 0xbf, 0x7c, 0x14, 0, 0],
            "158c->147c",
        ),
        (
            0x0850,
            &[0xf3, 0x45, 0x0f, 0x59, 0x86, 0xf0, 0x1c, 0, 0],
            "1d38->1cf0 tuning",
        ),
        (0x0f7a, &[0x48, 0x8d, 0x9f, 0x2c, 0x1c, 0, 0], "1d1c->1c2c"),
        (
            0x1076,
            &[0xf3, 0x41, 0x0f, 0x59, 0xa4, 0x24, 0x5c, 0x0a, 0, 0],
            "a14->a5c",
        ),
        (
            0x11a4,
            &[0xf3, 0x0f, 0x59, 0x99, 0x5c, 0x0a, 0, 0],
            "a14->a5c",
        ),
        (
            0x127d,
            &[0xf3, 0x0f, 0x59, 0xbf, 0x30, 0x14, 0, 0],
            "1540->1430",
        ),
        (0x13be, &[0x44, 0x3b, 0xa7, 0x0c, 0x14, 0, 0], "151c->140c"),
        (
            0x05da,
            &[0x4c, 0x8b, 0xac, 0xc8, 0xb8, 0x06, 0, 0],
            "670->6b8",
        ),
        (
            0x0605,
            &[0xf3, 0x41, 0x0f, 0x10, 0x47, 0x18],
            "contact 10->18",
        ),
        (
            0x0633,
            &[0xf3, 0x41, 0x0f, 0x10, 0x47, 0x48],
            "contact 18->48",
        ),
        (
            0x077b,
            &[0xf3, 0x44, 0x0f, 0x59, 0x9f, 0x80, 0x14, 0, 0],
            "147c->1480",
        ),
        (
            0x0784,
            &[0xf3, 0x0f, 0x59, 0xb7, 0x84, 0x14, 0, 0],
            "1594->1484",
        ),
        (
            0x09e6,
            &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0x28, 0x19, 0, 0],
            "1970->1928",
        ),
        (
            0x09f1,
            &[0xf3, 0x45, 0x0f, 0x10, 0x86, 0xf8, 0x1c, 0, 0],
            "1d40->1cf8",
        ),
        (
            0x10a6,
            &[0xf3, 0x0f, 0x59, 0x97, 0xdc, 0x14, 0, 0],
            "15ec->14dc",
        ),
        (
            0x1116,
            &[0xf3, 0x41, 0x0f, 0x10, 0x8e, 0xf4, 0x19, 0, 0],
            "1a3c->19f4",
        ),
        (
            0x1203,
            &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0xa4, 0x0d, 0, 0],
            "d94->da4",
        ),
        (
            0x120e,
            &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0xa0, 0x0d, 0, 0],
            "d90->da0",
        ),
        (
            0x1235,
            &[0xf3, 0x45, 0x0f, 0x10, 0x86, 0x7c, 0x18, 0, 0],
            "18c4->187c",
        ),
        (
            0x12d3,
            &[0xf3, 0x41, 0x0f, 0x10, 0x84, 0x24, 0x50, 0x0a, 0, 0],
            "a08->a50",
        ),
        (
            0x12dd,
            &[0xf3, 0x0f, 0x59, 0x87, 0x54, 0x17, 0, 0],
            "1874->1754",
        ),
        (
            0x13da,
            &[0xf3, 0x45, 0x0f, 0x5e, 0xbe, 0x8c, 0x1c, 0, 0],
            "1cd4->1c8c",
        ),
        (
            0x1508,
            &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0x88, 0x1e, 0, 0],
            "1ed0->1e88",
        ),
        (
            0x155c,
            &[0xf3, 0x45, 0x0f, 0x10, 0x9e, 0xe4, 0x1e, 0, 0],
            "1f2c->1ee4",
        ),
        (
            0x1581,
            &[0xf3, 0x45, 0x0f, 0x59, 0xb6, 0xf0, 0x1e, 0, 0],
            "1f38->1ef0",
        ),
        (
            0x1710,
            &[0xf3, 0x0f, 0x10, 0x8f, 0xd8, 0x13, 0, 0],
            "14e8->13d8",
        ),
        (
            0x1741,
            &[0xf3, 0x41, 0x0f, 0x10, 0x84, 0x24, 0x4c, 0x0a, 0, 0],
            "a04->a4c",
        ),
        (
            0x174b,
            &[0xf3, 0x0f, 0x59, 0x87, 0x50, 0x17, 0, 0],
            "1870->1750",
        ),
        (
            0x18c6,
            &[0xf3, 0x0f, 0x10, 0x87, 0xd8, 0x13, 0, 0],
            "14e8->13d8",
        ),
        (
            0x1bff,
            &[0xf3, 0x0f, 0x10, 0x82, 0xfc, 0x1c, 0, 0],
            "1d44->1cfc",
        ),
        (
            0x1c09,
            &[0xf3, 0x0f, 0x10, 0x82, 0x00, 0x1d, 0, 0],
            "1d48->1d00",
        ),
        (
            0x2259,
            &[0x49, 0x8d, 0x90, 0xc0, 0x17, 0, 0],
            "helper 16c0->17c0",
        ),
    ];
    for (off, expected, name) in rewritten {
        at(&island, *off, expected, name);
    }

    let unchanged_immediate: &[(usize, &[u8], &str)] = &[
        (0x442, &[0x48, 0x81, 0xc1, 0xb8, 0, 0, 0], "wheel stride"),
        (0xbed, &[0x48, 0x81, 0xc3, 0xb8, 0, 0, 0], "wheel stride"),
        (0xf81, &[0x49, 0x83, 0xc5, 0x70], "wheel member"),
        (0x117c, &[0x3c, 0x4a], "surface id"),
        (0x134f, &[0x49, 0x81, 0xc5, 0xb8, 0, 0, 0], "wheel stride"),
        (0x1bab, &[0x48, 0x81, 0xc1, 0xb8, 0, 0, 0], "wheel stride"),
    ];
    let unchanged_modrm: &[(usize, &[u8], &str)] = &[
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
    for (off, expected, name) in unchanged_immediate.iter().chain(unchanged_modrm.iter()) {
        at(&island, *off, expected, name);
    }

    at(
        &island,
        0x239c,
        &[0xeb, 0x66, 0x90, 0x90],
        "helper-field branch",
    );
    at(
        &island,
        0x2404,
        &[
            0x48, 0x81, 0xc1, 0x88, 0, 0, 0, 0xe9, 0x90, 0xff, 0xff, 0xff,
        ],
        "helper +0x88 cave",
    );
    at(
        &island,
        0x0d5,
        &[0xe9, 0x36, 0x23, 0, 0, 0x90, 0x90, 0x90, 0x90],
        "pointer-store branch",
    );
    at(
        &island,
        0x1aba,
        &[0xe9, 0x62, 0x09, 0, 0, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
        "pointer-load branch",
    );
    at(
        &island,
        0x20a4,
        &[
            0x48, 0x8d, 0x8f, 0x80, 0x12, 0, 0, 0xe9, 0x15, 0xfa, 0xff, 0xff,
        ],
        "displaced instruction cave",
    );
    at(
        &island,
        0x2410,
        &[
            0x48, 0x89, 0x85, 0xa0, 0, 0, 0, 0xe8, 0x44, 0, 0, 0, 0xe9, 0xbd, 0xdc, 0xff, 0xff,
        ],
        "pointer-store cave",
    );
    at(
        &island,
        0x2421,
        &[
            0x48, 0x8b, 0x85, 0xa0, 0, 0, 0, 0xe9, 0x77, 0xfc, 0xff, 0xff,
        ],
        "pointer-load cave",
    );
    at(
        &island,
        0x2430,
        &[0x48, 0x8d, 0x45, 0xd0, 0x90, 0x90, 0x90],
        "helper output scratch adapter",
    );
    at(
        &island,
        0x1fef,
        &[0xc7, 0x87, 0xb4, 0x14, 0, 0, 0, 0, 0, 0],
        "current live-field write +0x14b4",
    );

    assert_eq!(rewritten.len(), 43);
    assert_eq!(unchanged_immediate.len(), 6);
    assert_eq!(unchanged_modrm.len(), 9);
    for required in [
        "PROFILE_FALL2022_FIELD_REMAP_COUNT = 44",
        "PROFILE_FALL2022_DIRECT_FIELD_REMAP_COUNT = 43",
        "PROFILE_FALL2022_HELPER_FIELD_REMAP_COUNT = 1",
        "PROFILE_FALL2022_ABI_ADAPTER_COUNT = 2",
        "PROFILE_FALL2022_RELOCATED_CALL_COUNT = 155",
        "PROFILE_FALL2022_ABS64_THUNK_COUNT = 40",
        "PROFILE_FALL2022_BEHAVIOR_CERTIFIED = false",
    ] {
        assert!(source.contains(required), "missing {required}");
    }

    let reloc_offsets = uint_array(&source, "PROFILE_FALL2022_RELOC_OFFSETS");
    let reloc_targets = uint_array(&source, "PROFILE_FALL2022_RELOC_TARGET_RVAS");
    let abs_offsets = uint_array(&source, "PROFILE_FALL2022_ABS64_OFFSETS");
    let abs_targets = uint_array(&source, "PROFILE_FALL2022_ABS64_TARGET_RVAS");
    assert_eq!(reloc_offsets.len(), 155);
    assert_eq!(reloc_targets.len(), 155);
    assert_eq!(abs_offsets.len(), 40);
    assert_eq!(abs_targets.len(), 40);
    assert_eq!(
        reloc_offsets[2], 9240,
        "moved call relocation must point at cave +0x2418"
    );
    assert!(
        !reloc_offsets.contains(&218),
        "old call relocation at +0xda must be removed"
    );

    println!("verified Fall payload: 44 field remaps, 15 proven unchanged carriers, 2 ABI adapters, 155 rel32, 40 abs64, bytes={}", island.len());
}
