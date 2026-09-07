//! tminput -- the INPUT-SEMANTICS arm's editing tool for the packet STATE WORD.
//!
//! `ghost tape poke` edits steer/accel/brake over a tick range; nothing edited
//! the state word (mode nibble + 22 flag bits) except by hand in a `.gtape`.
//! These commands do that one edit, and only that one, so a minimal pair is a
//! minimal pair: the tape is re-encoded VERBATIM (every same-bit kept) and the
//! only packets whose coding changes are the first tick of the window (an
//! explicit literal carrying the new word) and the first tick after it (an
//! explicit literal restoring the word that was there).
//!
//!   tminput setword IN OUT --ticks A..B --flags HEX [--word0 HEX]
//!   tminput strip   IN OUT [--ticks A..B]          every non-plain word -> plain
//!   tminput words   FILE                            the runs of non-plain words
//!
//! Every write is read back and the tape compared field by field.

use gbx::container::{self, Container};
use gbx::tape::{literal_for, unpack_word_pub, Encoding, Packet, StateEnc, Tape};

fn die<T>(m: impl std::fmt::Display) -> T {
    eprintln!("tminput: {m}");
    std::process::exit(2)
}

fn flag<'a>(a: &'a [String], k: &str) -> Option<&'a String> {
    a.iter().position(|s| s == k).and_then(|i| a.get(i + 1))
}

fn need<'a>(a: &'a [String], k: &str) -> &'a String {
    flag(a, k).unwrap_or_else(|| die(format!("missing {k}")))
}

fn hex(s: &str) -> u32 {
    let t = s.trim_start_matches("0x").trim_start_matches("0X");
    u32::from_str_radix(t, 16).unwrap_or_else(|_| die(format!("not hex: {s}")))
}

fn range(s: &str) -> (usize, usize) {
    let mut it = s.splitn(2, "..");
    let a: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or_else(|| die("--ticks A..B"));
    let b: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or_else(|| die("--ticks A..B"));
    if b <= a {
        die::<()>("--ticks A..B needs B > A");
    }
    (a, b)
}

fn is_plain(p: &Packet) -> bool {
    let w0 = p.word0 & !0x20;
    p.flags == 0 && w0 == (w0 & 0xF)
}

/// Set (word0, flags) on ticks [a, b) and make the coding carry it: an explicit
/// literal at `a`, `prev` inside, and at `b` an explicit literal of whatever
/// word tick `b` had (unless it already carries one).
fn set_word(t: &mut Tape, a: usize, b: usize, word0: u32, flags: u32) {
    let ar = t.archives.first_mut().unwrap_or_else(|| die("no archive"));
    let n = ar.packets.len();
    if b > n {
        die::<()>(format!("--ticks {a}..{b} but the tape has {n} ticks"));
    }
    let lit = literal_for(word0, flags);
    let (w0, fl) = unpack_word_pub(lit);
    if fl != flags || (w0 & 0xF) != (word0 & 0xF) {
        die::<()>(format!("literal 0x{lit:x} does not round-trip word0 0x{word0:x} flags 0x{flags:x} (got 0x{w0:x}/0x{fl:x})"));
    }
    // what tick b must be restored to
    let restore = if b < n {
        let q = &ar.packets[b];
        match q.state {
            StateEnc::Lit(_) => None,
            _ => Some((q.word0, q.flags)),
        }
    } else {
        None
    };
    for i in a..b {
        let p = &mut ar.packets[i];
        p.word0 = (p.word0 & 0x20) | (w0 & !0x20); // keep the respawn bit
        p.flags = flags;
        p.mode = p.word0 & 0xF;
        p.state = if i == a { StateEnc::Lit(lit) } else { StateEnc::Prev };
    }
    if let Some((rw, rf)) = restore {
        let l = literal_for(rw, rf);
        ar.packets[b].state = StateEnc::Lit(l);
    }
}

fn write_back(c: &Container, t: &Tape, out: &str, enc: Encoding) {
    let body = t.splice_into(c.body(), enc).unwrap_or_else(|e| die(e));
    container::write_gbx(&c.gbx, body, out).unwrap_or_else(|e| die(e));
    let back = Tape::from_file(out).unwrap_or_else(|e| die(e));
    let mut bad = 0;
    for (na, ba) in t.archives.iter().zip(back.archives.iter()) {
        if na.packets.len() != ba.packets.len() {
            die::<()>("read-back tick count differs");
        }
        for (i, (p, q)) in na.packets.iter().zip(ba.packets.iter()).enumerate() {
            if p.steer != q.steer
                || p.accel != q.accel
                || p.brake != q.brake
                || p.respawn() != q.respawn()
                || p.mode != q.mode
                || p.mouse != q.mouse
                || p.flags != q.flags
                || (p.word0 & !0x20) != (q.word0 & !0x20)
            {
                if bad < 5 {
                    eprintln!("  tick {i}: read back differently: {:?} vs {:?}", p, q);
                }
                bad += 1;
            }
        }
    }
    if bad > 0 {
        die::<()>(format!("{bad} ticks read back differently; {out} is NOT what was asked for"));
    }
}

fn runs(t: &Tape) -> Vec<(usize, usize, u32, u32)> {
    let mut out = Vec::new();
    let ar = match t.archives.first() {
        Some(a) => a,
        None => return out,
    };
    let n = ar.packets.len();
    let mut i = 0;
    while i < n {
        let p = &ar.packets[i];
        if is_plain(p) {
            i += 1;
            continue;
        }
        let key = (p.word0 & !0x20, p.flags);
        let mut j = i;
        while j < n && (ar.packets[j].word0 & !0x20, ar.packets[j].flags) == key {
            j += 1;
        }
        out.push((i, j, key.0, key.1));
        i = j;
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let op = a.first().map(|s| s.as_str()).unwrap_or_else(|| die("tminput <setword|strip|words> ..."));
    let rest = &a[1..];
    match op {
        "words" => {
            let f = rest.first().unwrap_or_else(|| die("tminput words FILE"));
            let t = Tape::from_file(f).unwrap_or_else(|e| die(e));
            let so = t.archives[0].start_offset_ms as i64;
            println!("tick_from\ttick_to\trace_s_from\trace_s_to\tword0\tflags\tsteer_on\tsteer_last");
            for (i, j, w0, fl) in runs(&t) {
                let p = &t.archives[0].packets;
                println!(
                    "{i}\t{j}\t{:.3}\t{:.3}\t0x{w0:x}\t0x{fl:x}\t{}\t{}",
                    (so + 10 * i as i64) as f64 / 1000.0,
                    (so + 10 * j as i64) as f64 / 1000.0,
                    p[i].steer_i8(),
                    p[j - 1].steer_i8()
                );
            }
        }
        "setword" | "strip" => {
            let inp = rest.first().unwrap_or_else(|| die("tminput {op} IN OUT ..."));
            let out = rest.get(1).unwrap_or_else(|| die("tminput {op} IN OUT ..."));
            let c = Container::load(inp).unwrap_or_else(|e| die(e));
            let mut t = Tape::from_file(inp).unwrap_or_else(|e| die(e));
            t.verbatim_is_identity().unwrap_or_else(|e| die(format!("codec identity failed on {inp}: {e}")));
            let enc = if rest.iter().any(|s| s == "--explicit") { Encoding::Explicit } else { Encoding::Verbatim };
            if op == "setword" {
                let (a0, b0) = range(need(rest, "--ticks"));
                let flags = hex(need(rest, "--flags"));
                let word0 = flag(rest, "--word0").map(|s| hex(s)).unwrap_or(t.archives[0].packets[a0].word0 & 0xF);
                set_word(&mut t, a0, b0, word0, flags);
                println!("set ticks {a0}..{b0} to word0 0x{word0:x} flags 0x{flags:x}");
            } else {
                let (lo, hi) = flag(rest, "--ticks").map(|s| range(s)).unwrap_or((0, t.n()));
                let rs = runs(&t);
                let mut k = 0;
                for (i, j, w0, fl) in rs {
                    if i >= lo && j <= hi {
                        set_word(&mut t, i, j, w0 & 0xF, 0);
                        println!("stripped ticks {i}..{j} (was word0 0x{w0:x} flags 0x{fl:x})");
                        k += 1;
                    }
                }
                println!("{k} runs stripped");
            }
            write_back(&c, &t, out, enc);
            println!("wrote {out}");
        }
        o => die(format!("unknown op {o}")),
    }
}
