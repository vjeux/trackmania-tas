use std::{env, fs, path::Path};

const TARGET_SHA256: &str = "3fc7d8cda542beda131c44306b123f4004d07d7e22f512b46b762afc29f6edda";

#[derive(Clone)]
struct Site {
    name: &'static str,
    rva: u32,
    current: &'static [u8],
    historical: &'static [u8],
}

const SITES: &[Site] = &[
    Site {
        name: "pre-May Xi smooth-steering snap",
        rva: 0x2C360E,
        current: &[0xF3, 0x0F, 0x11, 0x64, 0x8D, 0x74],
        historical: &[0x90; 6],
    },
    Site {
        name: "pre-Feb action-key route",
        rva: 0x2B8C4C,
        current: &[0x74, 0x18],
        historical: &[0x90, 0x90],
    },
    Site {
        name: "release delayed adherence",
        rva: 0x1342927,
        current: &[0xE8, 0x24, 0xFB, 0xFF, 0xFF],
        historical: &[0x31, 0xC0, 0x90, 0x90, 0x90],
    },
    Site {
        name: "release delayed acceleration",
        rva: 0x1342AB7,
        current: &[0xE8, 0x94, 0xF9, 0xFF, 0xFF],
        historical: &[0x31, 0xC0, 0x90, 0x90, 0x90],
    },
    Site {
        name: "release delayed control",
        rva: 0x1342C47,
        current: &[0xE8, 0x04, 0xF8, 0xFF, 0xFF],
        historical: &[0x31, 0xC0, 0x90, 0x90, 0x90],
    },
];

fn u16le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}
fn u32le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn rva_offset(image: &[u8], rva: u32) -> usize {
    let pe = u32le(image, 0x3c) as usize;
    assert_eq!(&image[pe..pe + 4], b"PE\0\0", "PE signature");
    let section_count = u16le(image, pe + 6) as usize;
    let optional_size = u16le(image, pe + 20) as usize;
    let section_table = pe + 24 + optional_size;
    for index in 0..section_count {
        let section = section_table + index * 40;
        let virtual_size = u32le(image, section + 8);
        let virtual_address = u32le(image, section + 12);
        let raw_size = u32le(image, section + 16);
        let raw_offset = u32le(image, section + 20);
        let span = virtual_size.max(raw_size);
        if rva >= virtual_address && rva < virtual_address + span {
            return (raw_offset + (rva - virtual_address)) as usize;
        }
    }
    panic!("RVA 0x{rva:X} is outside PE sections");
}

fn at_rva<'a>(image: &'a [u8], rva: u32, length: usize) -> &'a [u8] {
    let offset = rva_offset(image, rva);
    &image[offset..offset + length]
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let bit_len = (input.len() as u64) * 8;
    let mut message = input.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());
    let mut h = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    for chunk in message.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (index, word) in chunk.chunks_exact(4).enumerate() {
            w[index] = u32::from_be_bytes(word.try_into().unwrap());
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (state, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *state = state.wrapping_add(value);
        }
    }
    let mut output = [0u8; 32];
    for (index, word) in h.iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    output
}

fn must(haystack: &str, needle: &str, what: &str) {
    assert!(haystack.contains(needle), "missing {what}: {needle}");
}

fn simulate_transaction(
    desired: &[bool],
    fail_at: Option<usize>,
) -> Result<Vec<Vec<u8>>, Vec<Vec<u8>>> {
    let mut state = SITES
        .iter()
        .map(|site| site.current.to_vec())
        .collect::<Vec<_>>();
    let before = state.clone();
    let mut changed = Vec::new();
    for (index, wanted) in desired.iter().copied().enumerate() {
        if !wanted {
            continue;
        }
        state[index] = SITES[index].historical.to_vec();
        changed.push(index);
        if fail_at == Some(index) {
            for changed_index in changed.into_iter().rev() {
                state[changed_index] = before[changed_index].clone();
            }
            return Err(state);
        }
    }
    Ok(state)
}

fn verify_collision_payloads(root: &Path) {
    const PRE_HASH: &str = "82a0822220468e50f78b372c840fe2c01fee9cc017a712b3f926548668841661";
    const POST_HASH: &str = "ef0ebee29e98faec02c5e563c99688fb664b9b76314eee20c77aef8c9c048d9d";
    const CURRENT_HASH: &str = "7ea1385e37ecaa3005939bd1d38608f5a36e276da4f81d3efc9079cfa13a68cb";
    const ROOT_HASH: &str = "1f7b1bc03a67d7cfde6917857f81f6e4ef9046385cbe3dc9bccf093a0e65e64c";

    let pre = fs::read(root.join("payloads/snow/pre-feb/SnowCar.Shape.Gbx")).unwrap();
    let post = fs::read(root.join("evidence/snow-assets/CarSnow-Surface-post-Feb.Gbx")).unwrap();
    let current = fs::read(root.join("evidence/snow-assets/SnowCar.Shape.Gbx"))
        .or_else(|_| fs::read(root.join("evidence/snow-live/current-all/SnowCar.Shape.Gbx")))
        .unwrap();
    assert_eq!(pre.len(), 1_123);
    assert_eq!(post.len(), 1_147);
    assert_eq!(current.len(), 1_151);
    assert_eq!(hex(&sha256(&pre)), PRE_HASH);
    assert_eq!(hex(&sha256(&post)), POST_HASH);
    assert_eq!(hex(&sha256(&current)), CURRENT_HASH);
    assert_eq!(&pre[..3], b"GBX");
    assert_eq!(u32le(&pre, 9), 0x0900_C000);
    assert_eq!(u32le(&pre, 25), 0x0900_C003);
    assert_eq!(u32le(&pre, 29), 4);
    assert_eq!(u32le(&pre, 33), 2);
    assert_eq!(u32le(&pre, 37), 13);
    assert_eq!(u32le(&pre, 41), 7);

    let mut offset = 45usize;
    for expected_radius in [0x3F99_03C9, 0x3F78_2DE0, 0x3F8F_4A23] {
        assert_eq!(u32le(&pre, offset), 0, "pre-Feb body primitive is a sphere");
        assert_eq!(u32le(&pre, offset + 4), expected_radius);
        assert_eq!(u16le(&pre, offset + 8), 0);
        assert_eq!(
            &pre[offset + 10..offset + 22],
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0x3F]
        );
        offset += 22;
    }
    for _ in 0..4 {
        assert_eq!(u32le(&pre, offset), 0, "wheel primitive is a sphere");
        assert_eq!(u32le(&pre, offset + 4), 0x3EF0_A3D7);
        offset += 22;
    }
    assert_eq!(offset, 0xC7);

    let pre_root = fs::read(root.join("evidence/snow-assets/CarSnow-pre-Feb.Item.Gbx")).unwrap();
    let post_root = fs::read(root.join("evidence/snow-assets/CarSnow-post-Feb.Item.Gbx")).unwrap();
    assert_eq!(
        pre_root, post_root,
        "root CarSnow item is the negative control"
    );
    assert_eq!(pre_root.len(), 1_900);
    assert_eq!(hex(&sha256(&pre_root)), ROOT_HASH);
}

fn main() {
    let mut args = env::args().skip(1);
    let first = args
        .next()
        .expect("package source directory or --collision-only");
    if first == "--collision-only" {
        let root = args.next().expect("package source directory");
        verify_collision_payloads(Path::new(&root));
        println!("Snow collision payload verifier passed: exact hashes, GBX class, sphere geometry, and root negative control");
        return;
    }
    let root = first;
    let target_path = args.next().expect("build-128130 Trackmania.exe");
    let evidence_root = args.next().expect("official-family evidence root");
    verify_collision_payloads(Path::new(&root));

    let target = fs::read(&target_path).expect("read target executable");
    assert_eq!(
        hex(&sha256(&target)),
        TARGET_SHA256,
        "target executable SHA-256"
    );
    for site in SITES {
        assert_eq!(
            at_rva(&target, site.rva, site.current.len()),
            site.current,
            "target preimage for {} at RVA 0x{:X}",
            site.name,
            site.rva
        );
    }

    let jan =
        fs::read(Path::new(&evidence_root).join("profiles/2024-01-10/Trackmania/Trackmania.exe"))
            .unwrap();
    let feb =
        fs::read(Path::new(&evidence_root).join("profiles/2024-02-26/Trackmania/Trackmania.exe"))
            .unwrap();
    let march =
        fs::read(Path::new(&evidence_root).join("profiles/2024-03-19/Trackmania/Trackmania.exe"))
            .unwrap();
    let april =
        fs::read(Path::new(&evidence_root).join("profiles/2024-04-30/Trackmania/Trackmania.exe"))
            .unwrap();

    // February: type 0x18 begins bypassing the first action queue. The target retains this branch.
    assert_eq!(
        at_rva(&jan, 0x269460, 11),
        &[0x41, 0x81, 0xBF, 0x94, 0x0D, 0, 0, 0xA0, 0, 0, 0]
    );
    assert_eq!(at_rva(&feb, 0x269949, 5), &[0x83, 0xF8, 0x18, 0x74, 0x18]);
    assert_eq!(
        at_rva(&target, 0x2B8C49, 5),
        &[0x83, 0xF8, 0x18, 0x74, 0x18]
    );

    // May: Xi input smoothing adds a snap-to-target store; pre-May branches directly to old flow.
    assert_eq!(
        at_rva(&march, 0x2742A3, 8),
        &[0x85, 0xC0, 0x0F, 0x85, 0x58, 0x01, 0, 0]
    );
    assert_eq!(
        at_rva(&april, 0x2747C6, 14),
        &[0x85, 0xC0, 0x0F, 0x84, 0x7C, 0, 0, 0, 0xF3, 0x0F, 0x11, 0x64, 0x8D, 0x74]
    );
    assert_eq!(
        at_rva(&target, 0x2C3606, 14),
        &[0x85, 0xC0, 0x0F, 0x84, 0x7C, 0, 0, 0, 0xF3, 0x0F, 0x11, 0x64, 0x8D, 0x74]
    );

    let snow_source = fs::read_to_string(Path::new(&root).join("SnowPatches.as")).unwrap();
    for token in [
        "SNOW_ACTION_KEY_BRANCH_RVA = 0x2B8C4C",
        "SNOW_SMOOTH_STEERING_STORE_RVA = 0x2C360E",
        "SNOW_DELAYED_ADHERENCE_CALL_RVA = 0x1342927",
        "SNOW_DELAYED_ACCEL_CALL_RVA = 0x1342AB7",
        "SNOW_DELAYED_CONTROL_CALL_RVA = 0x1342C47",
        "PreflightSnowCodeTransaction",
        "RollBackSnowCodeChanges",
        "SNOW_COLLISION_PRE_FEB_SIZE = 1123",
        "SNOW_COLLISION_CURRENT_SIZE = 1151",
        "ValidateSnowCollisionEpoch",
    ] {
        must(&snow_source, token, "Snow payload invariant");
    }

    let february = simulate_transaction(&[true, false, false, false, false], None).unwrap();
    assert_eq!(february[0], SITES[0].historical);
    assert_eq!(february[1], SITES[1].current);
    let january = simulate_transaction(&[true, true, false, false, false], None).unwrap();
    assert_eq!(january[0], SITES[0].historical);
    assert_eq!(january[1], SITES[1].historical);
    assert!(january[2..]
        .iter()
        .zip(&SITES[2..])
        .all(|(bytes, site)| bytes == site.current));
    let release = simulate_transaction(&[true, true, true, true, true], None).unwrap();
    assert!(release
        .iter()
        .zip(SITES)
        .all(|(bytes, site)| bytes == site.historical));
    let rolled_back = simulate_transaction(&[true, true, true, true, true], Some(3)).unwrap_err();
    assert!(rolled_back
        .iter()
        .zip(SITES)
        .all(|(bytes, site)| bytes == site.current));

    // Positive and negative controls for strict preimage validation.
    let mut corrupted = target.clone();
    let offset = rva_offset(&corrupted, SITES[0].rva);
    corrupted[offset] ^= 1;
    assert_ne!(
        at_rva(&corrupted, SITES[0].rva, SITES[0].current.len()),
        SITES[0].current
    );

    println!("Snow payload verifier passed: exact target hash, five preimages, historical collision shape and root negative control, February/May historical controls, transactional rollback, corrupted-preimage rejection");
}
