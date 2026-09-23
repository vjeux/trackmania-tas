//! clientre -- instruments for reading the game client's own implementation
//! of the things we re-implement (first: the lightmapper).
//!
//!   clientre gpucache FILE.GpuCache.Gbx [--out DIR]
//!       LZO-unpack a shader cache entry, find the DXBC blob(s), print the
//!       reflection (constant buffers, resources, signatures) and the SM5
//!       disassembly. `--out DIR` also writes each raw blob as DIR/<name>.dxbc.
//!   clientre gpucache-all DIR_OF_GBX OUT_DIR
//!       the same for every *.GpuCache.Gbx under a directory, one .txt each.
//!   clientre dxbc FILE.dxbc
//!       dump a raw DXBC blob.
//!
//! The shader cache lives in `Packs/GpuCache_D3D11_SM5.zip` next to the exe;
//! the lightmapper's programs are the `Lightmap/*.hlsl.GpuCache.Gbx` entries.
//! Nothing here touches the game or a map.

mod dxbc;
mod lmimages;

fn die(msg: String) -> ! {
    eprintln!("clientre: {}", msg);
    std::process::exit(2);
}

fn unpack(path: &str) -> Vec<u8> {
    let data = std::fs::read(path).unwrap_or_else(|e| die(format!("{}: {}", path, e)));
    if data.len() >= 3 && &data[0..3] == b"GBX" {
        gbx::container::Gbx::parse(&data).body
    } else {
        data
    }
}

fn ascii_strings(b: &[u8], min: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for &c in b {
        if (0x20..0x7f).contains(&c) {
            cur.push(c as char);
        } else {
            if cur.len() >= min {
                out.push(cur.clone());
            }
            cur.clear();
        }
    }
    if cur.len() >= min {
        out.push(cur);
    }
    out
}

fn dump_one(path: &str, out_dir: Option<&str>) -> String {
    let body = unpack(path);
    let blobs = dxbc::find_blobs(&body);
    let mut s = String::new();
    s.push_str(&format!("{}: body {} bytes, {} DXBC blob(s)\n", path, body.len(), blobs.len()));
    let stem = std::path::Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| "blob".into());
    for (k, &(off, len)) in blobs.iter().enumerate() {
        let blob = &body[off..off + len];
        // The Nadeo wrapper between blobs names the entry point and the
        // permutation's defines in the clear; print those strings.
        let pre_start = if k == 0 { 0 } else { blobs[k - 1].0 + blobs[k - 1].1 };
        let pre: Vec<String> = ascii_strings(&body[pre_start..off], 4);
        if !pre.is_empty() {
            s.push_str(&format!("-- wrapper: {}\n", pre.join(" | ")));
        }
        if let Some(dir) = out_dir {
            let _ = std::fs::create_dir_all(dir);
            let name = format!("{}/{}{}.dxbc", dir, stem, if blobs.len() > 1 { format!(".{}", k) } else { String::new() });
            std::fs::write(&name, blob).unwrap_or_else(|e| die(format!("{}: {}", name, e)));
        }
        s.push_str(&format!("== blob {} at +0x{:x}\n", k, off));
        s.push_str(&dxbc::dump_blob(blob));
    }
    s
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(|s| s.as_str()) {
        Some("gpucache") => {
            let path = a.get(1).unwrap_or_else(|| die("gpucache FILE [--out DIR]".into()));
            let out = a.iter().position(|x| x == "--out").and_then(|p| a.get(p + 1)).map(|s| s.as_str());
            print!("{}", dump_one(path, out));
        }
        Some("gpucache-all") => {
            let dir = a.get(1).unwrap_or_else(|| die("gpucache-all DIR OUT_DIR".into()));
            let out = a.get(2).unwrap_or_else(|| die("gpucache-all DIR OUT_DIR".into()));
            std::fs::create_dir_all(out).unwrap_or_else(|e| die(e.to_string()));
            let mut entries: Vec<_> = std::fs::read_dir(dir)
                .unwrap_or_else(|e| die(format!("{}: {}", dir, e)))
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.to_string_lossy().ends_with(".GpuCache.Gbx"))
                .collect();
            entries.sort();
            for p in entries {
                let ps = p.to_string_lossy().into_owned();
                let stem = p.file_name().unwrap().to_string_lossy().replace(".GpuCache.Gbx", "");
                let text = dump_one(&ps, None);
                let outp = format!("{}/{}.txt", out, stem);
                std::fs::write(&outp, text).unwrap_or_else(|e| die(format!("{}: {}", outp, e)));
                println!("{}", outp);
            }
        }
        Some("lmimages") => {
            let path = a.get(1).unwrap_or_else(|| die("lmimages MAP.Gbx [OUTDIR]".into()));
            let out = a.get(2).map(|s| s.as_str());
            print!("{}", lmimages::run(path, out).unwrap_or_else(|e| die(e)));
        }
        Some("dxbc") => {
            let path = a.get(1).unwrap_or_else(|| die("dxbc FILE".into()));
            let data = std::fs::read(path).unwrap_or_else(|e| die(format!("{}: {}", path, e)));
            print!("{}", dxbc::dump_blob(&data));
        }
        _ => {
            eprintln!("clientre gpucache FILE [--out DIR] | gpucache-all DIR OUT_DIR | dxbc FILE | lmimages MAP.Gbx [OUTDIR]");
            std::process::exit(2);
        }
    }
}
