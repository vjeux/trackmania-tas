use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

const TARGET_EXE_SHA256: &str = "3fc7d8cda542beda131c44306b123f4004d07d7e22f512b46b762afc29f6edda";
const PAYLOAD_SHA256: &str = "a1d5cdcd21ed4b152ae18b9f94dd8fa4f3eb4375d0035a83c20923a251bccd9a";
const RELATIVE_TARGET: &str = "GameData/Vehicles/Items/Cars/CarRally.Item.Gbx";
const PAYLOAD: &[u8] = include_bytes!("../payloads/rally-release/CarRally.Item.Gbx");

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
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(majority);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
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

fn digest(bytes: &[u8]) -> String {
    sha256(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn target_path(game_root: &Path) -> PathBuf {
    game_root.join(RELATIVE_TARGET)
}

fn backup_path(target: &Path) -> PathBuf {
    let mut value = target.as_os_str().to_os_string();
    value.push(".hpl-backup");
    PathBuf::from(value)
}

fn temporary_path(target: &Path) -> PathBuf {
    let mut value = target.as_os_str().to_os_string();
    value.push(format!(".hpl-tmp-{}", std::process::id()));
    PathBuf::from(value)
}

fn validate_payload() -> Result<(), String> {
    if PAYLOAD.len() != 3_056 || &PAYLOAD[..3] != b"GBX" {
        return Err("embedded Rally item has the wrong size or magic".into());
    }
    let class_id = u32::from_le_bytes(PAYLOAD[9..13].try_into().unwrap());
    if class_id != 0x2E00_2000 {
        return Err(format!("embedded Rally item has class 0x{class_id:08X}"));
    }
    let observed = digest(PAYLOAD);
    if observed != PAYLOAD_SHA256 {
        return Err(format!("embedded Rally item checksum mismatch: {observed}"));
    }
    Ok(())
}

fn validate_target(game_root: &Path) -> Result<(), String> {
    validate_payload()?;
    let exe_path = game_root.join("Trackmania.exe");
    let exe =
        fs::read(&exe_path).map_err(|error| format!("read {}: {error}", exe_path.display()))?;
    let observed = digest(&exe);
    if observed != TARGET_EXE_SHA256 {
        return Err(format!("unsupported Trackmania.exe SHA-256 {observed}"));
    }
    Ok(())
}

fn install(game_root: &Path) -> Result<(), String> {
    validate_target(game_root)?;
    let target = target_path(game_root);
    let backup = backup_path(&target);
    if target.exists() {
        let existing =
            fs::read(&target).map_err(|error| format!("read existing override: {error}"))?;
        if digest(&existing) == PAYLOAD_SHA256 {
            println!("release Rally item override already installed");
            return Ok(());
        }
        if backup.exists() {
            return Err(format!(
                "refusing to overwrite both {} and {}",
                target.display(),
                backup.display()
            ));
        }
        fs::rename(&target, &backup)
            .map_err(|error| format!("back up existing loose Rally item: {error}"))?;
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create override directory: {error}"))?;
    }
    let temporary = temporary_path(&target);
    let result = (|| -> Result<(), String> {
        let mut file = fs::File::create(&temporary)
            .map_err(|error| format!("create temporary Rally item: {error}"))?;
        file.write_all(PAYLOAD)
            .map_err(|error| format!("write temporary Rally item: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("sync temporary Rally item: {error}"))?;
        let written = fs::read(&temporary)
            .map_err(|error| format!("read back temporary Rally item: {error}"))?;
        if digest(&written) != PAYLOAD_SHA256 {
            return Err("temporary Rally item checksum mismatch".into());
        }
        fs::rename(&temporary, &target)
            .map_err(|error| format!("activate historical Rally item: {error}"))?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        if backup.exists() && !target.exists() {
            let _ = fs::rename(&backup, &target);
        }
        return Err(error);
    }
    let installed =
        fs::read(&target).map_err(|error| format!("read installed Rally item: {error}"))?;
    if digest(&installed) != PAYLOAD_SHA256 {
        let _ = fs::remove_file(&target);
        if backup.exists() {
            let _ = fs::rename(&backup, &target);
        }
        return Err("installed Rally item readback failed; prior file restored".into());
    }
    println!("installed release Rally item at {}", target.display());
    println!("restart Trackmania before selecting Rally release");
    Ok(())
}

fn restore(game_root: &Path) -> Result<(), String> {
    // Recovery must remain available after Trackmania updates. Ownership is
    // proven by the payload hash below; the exact-executable gate is install-only.
    validate_payload()?;
    let target = target_path(game_root);
    let backup = backup_path(&target);
    if target.exists() {
        let existing =
            fs::read(&target).map_err(|error| format!("read loose Rally item: {error}"))?;
        if digest(&existing) != PAYLOAD_SHA256 {
            return Err(format!(
                "refusing to remove unowned file {}",
                target.display()
            ));
        }
        fs::remove_file(&target)
            .map_err(|error| format!("remove historical Rally item: {error}"))?;
    }
    if backup.exists() {
        fs::rename(&backup, &target)
            .map_err(|error| format!("restore prior loose Rally item: {error}"))?;
    }
    println!("restored installed Rally item; restart Trackmania");
    Ok(())
}

fn status(game_root: &Path) -> Result<(), String> {
    validate_payload()?;
    let target = target_path(game_root);
    let backup = backup_path(&target);
    if !target.exists() {
        println!("current: no loose Rally item override");
    } else {
        let bytes = fs::read(&target).map_err(|error| format!("read loose Rally item: {error}"))?;
        let hash = digest(&bytes);
        if hash == PAYLOAD_SHA256 {
            println!("historical: certified release Rally item override installed");
        } else {
            println!("unknown: unowned loose Rally item SHA-256 {hash}");
        }
    }
    println!("backup_present={}", backup.exists());
    Ok(())
}

fn main() {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let game_root = args.next().unwrap_or_default();
    if game_root.is_empty() || !matches!(command.as_str(), "install" | "restore" | "status") {
        eprintln!("usage: rally_item_overlay <install|restore|status> <Trackmania-directory>");
        std::process::exit(2);
    }
    let result = match command.as_str() {
        "install" => install(Path::new(&game_root)),
        "restore" => restore(Path::new(&game_root)),
        "status" => status(Path::new(&game_root)),
        _ => unreachable!(),
    };
    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
