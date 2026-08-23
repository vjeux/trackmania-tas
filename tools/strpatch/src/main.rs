// strpatch -- replace a byte string inside a GBX file's DECOMPRESSED body.
//
// Built to answer one question: does the dedicated server switch physics on the
// build stamp a ghost carries? The stamp lives inside the LZO-compressed body,
// so `sed` cannot reach it. Answer, from this tool: no -- the server echoes the
// patched stamp back in its `GameBuild` field and simulates exactly the same
// run (OLDBUILD.md §7).
//
//   strpatch --in A.Ghost.Gbx --out B.Ghost.Gbx --old '<text>' --new '<text>'
//
// Old and new must be the same length: this rewrites bytes in place and does
// not touch any length field that might describe them.
use gbx::{container::write_gbx, Gbx};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (mut inp, mut out, mut old, mut new) =
        (String::new(), String::new(), String::new(), String::new());
    let mut i = 0;
    while i < a.len() {
        match a[i].as_str() {
            "--in" => {
                inp = a[i + 1].clone();
                i += 2;
            }
            "--out" => {
                out = a[i + 1].clone();
                i += 2;
            }
            "--old" => {
                old = a[i + 1].clone();
                i += 2;
            }
            "--new" => {
                new = a[i + 1].clone();
                i += 2;
            }
            _ => {
                eprintln!("usage: strpatch --in A --out B --old TEXT --new TEXT");
                std::process::exit(2);
            }
        }
    }
    if inp.is_empty() || out.is_empty() || old.is_empty() {
        eprintln!("usage: strpatch --in A --out B --old TEXT --new TEXT");
        std::process::exit(2);
    }
    if old.len() != new.len() {
        eprintln!(
            "strpatch: --old is {} bytes and --new is {} bytes; they must match",
            old.len(),
            new.len()
        );
        std::process::exit(2);
    }
    let g = Gbx::parse(&std::fs::read(&inp).unwrap_or_else(|e| {
        eprintln!("strpatch: {inp}: {e}");
        std::process::exit(1)
    }));
    let mut body = g.body.clone();
    let (ob, nb) = (old.as_bytes(), new.as_bytes());
    let mut hits = 0;
    let mut k = 0;
    while k + ob.len() <= body.len() {
        if &body[k..k + ob.len()] == ob {
            body[k..k + ob.len()].copy_from_slice(nb);
            hits += 1;
            k += ob.len();
        } else {
            k += 1;
        }
    }
    println!("{hits} occurrence(s) replaced");
    if hits == 0 {
        eprintln!("strpatch: nothing written");
        std::process::exit(1);
    }
    if let Err(e) = write_gbx(&g, body, &out) {
        eprintln!("strpatch: {e}");
        std::process::exit(1);
    }
    println!("wrote {out}");
}
