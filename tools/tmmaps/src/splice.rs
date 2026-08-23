//! splice.rs — write an edited map by **patching the stock file's own
//! compressed stream**, instead of re-emitting the file.
//!
//! ## Why this module exists
//!
//! `map.rs` already patches the *body*: a position-only move rewrites a
//! handful of bytes and memcpy's the rest. But the file on disk is
//! LZO-compressed, and the writer used to throw the stock file's compressed
//! stream away and run `lzo1x_1_compress` over the whole body. **LZO
//! recompression is not bit-reproducible** — the game ships maps compressed by
//! a stronger LZO variant, so a rebuilt 173691 comes back 29 763 bytes longer
//! and shares essentially no byte after the header with the file the game
//! downloaded. The dedicated server accepts that. Whether the game CLIENT does
//! is the question this module exists to stop having to ask: a file that
//! differs from a file the client already loads in only the bytes of the edit
//! cannot be rejected *for being rebuilt*.
//!
//! This is the same principle `ghost map set` follows for the embedded map —
//! splice the bytes in, never re-serialise — one level down, inside the LZO
//! stream.
//!
//! ## The two methods, and how one is chosen
//!
//! | method | what the output file is | when |
//! |---|---|---|
//! | `Literal` | the stock file, byte for byte, with the edited bytes overwritten **in place inside the compressed stream**. Same length. | every edited byte was emitted by the stock stream as a *literal*, and no later match copies from one |
//! | `Tail` | the stock stream verbatim up to an instruction boundary at or before the first edited byte, then a freshly compressed tail | otherwise (an edited byte sits inside a match) |
//! | `Reemit` | full `lzo1x_1_compress` of the new body | only when the body's LENGTH changed — a rename or an item-model swap. Nothing else can produce one |
//!
//! Every method is **verified before the bytes are returned**: the produced
//! stream is decompressed with liblzo2 and required to equal the intended body
//! exactly, over its whole length. A splicer that silently drops an edit, or
//! that lets a later match copy a patched byte somewhere it should not, fails
//! that check — it cannot fail toward "clean", because the check is an
//! equality on the thing the game will read, not a checksum of what we meant.
//!
//! ## The LZO1X stream, as the decoder reads it
//!
//! Ported from `lzo1x_d.ch` (LZO 2.10), and it must stay a faithful mirror:
//! this module decides *which stream bytes are literals*, and a decoder that
//! disagrees with liblzo2 about that would patch the wrong bytes. The
//! agreement is checked on real maps in `selftest` (`splice.scan_agrees`),
//! which requires the scan's own reconstruction of the output to equal
//! liblzo2's.
//!
//! ```text
//!   first byte > 17           a starting literal run of (b - 17)
//!   t < 16   (top of loop)    a literal run of t + 3 (t == 0: long form)
//!   t < 16   (after literals) a 3-byte match
//!   t in 16..31               a long-distance match; distance 0 is END OF STREAM
//!   t in 32..63               a match, length in t & 31 (long form when 0)
//!   t >= 64                   a short match, length (t >> 5) + 1
//!   ...and the low 2 bits of the last distance byte are the number of
//!   literals that follow the match with no opcode of their own.
//! ```

use crate::gbx::{lzo_compress, lzo_decompress};

/// How the compressed stream of the written file was produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// The stock stream with the edited bytes overwritten in place.
    Literal,
    /// The stock stream with one short stretch replaced: everything before the
    /// edit and everything after it is the stock stream's own bytes.
    Middle,
    /// The stock stream verbatim to a cut, then a recompressed tail.
    Tail,
    /// The whole body compressed from scratch because the source file had no
    /// compressed stream of its own — a `'U'` body, which older writers in this
    /// project produced and the dedicated server refuses ("Can't load map").
    /// Nothing is lost by compressing it; there was simply nothing to splice.
    Compress,
    /// The whole body recompressed: the only method that changes bytes the
    /// edit did not ask to change.
    Reemit,
}

impl Method {
    pub fn label(self) -> &'static str {
        match self {
            Method::Literal => "literal-patch (stock stream, edited bytes overwritten in place)",
            Method::Middle => "middle-splice (stock stream either side of one recompressed stretch)",
            Method::Tail => "tail-splice (stock stream verbatim to the cut, tail recompressed)",
            Method::Compress => {
                "compress (the source body was stored UNCOMPRESSED — there was no stream to splice)"
            }
            Method::Reemit => "re-emit (whole body recompressed — the body's length changed)",
        }
    }
}

/// What a splice produced, and how.
pub struct Spliced {
    pub stream: Vec<u8>,
    pub method: Method,
    /// Bytes of the stock compressed stream that are unchanged in the output,
    /// counted from the front.
    pub shared_prefix: usize,
    /// Bytes of the stock compressed stream carried through verbatim at the
    /// end (the tail the splice did not have to touch).
    pub shared_suffix: usize,
    /// How many bytes of the body the edit changed.
    pub changed_bytes: usize,
    /// The stock stream's length, so a report can say what FRACTION survived.
    pub stock_len: usize,
}

impl Spliced {
    /// One line for a human: how this file was written and how much of the
    /// stock file it still is.
    pub fn summary(&self) -> String {
        if self.method == Method::Reemit {
            return format!(
                "written by {} — no part of the stock compressed stream survives",
                self.method.label()
            );
        }
        let carried = (self.shared_prefix + self.shared_suffix).min(self.stock_len);
        format!(
            "written by {} — {} body byte(s) changed, {} of {} stock stream bytes carried \
             verbatim ({:.2} %)",
            self.method.label(),
            self.changed_bytes,
            carried,
            self.stock_len,
            100.0 * carried as f64 / self.stock_len.max(1) as f64
        )
    }
}

// ------------------------------------------------------------------ scanning

/// A literal run: `len` bytes copied from `stream` at `stream_off` to the
/// output at `out_off`.
#[derive(Clone, Copy, Debug)]
pub struct LitRun {
    pub out_off: usize,
    pub stream_off: usize,
    pub len: usize,
}

/// The result of walking an LZO1X stream: where its literals come from, and
/// every point at which the stream can be cut and continued.
pub struct Scan {
    pub lits: Vec<LitRun>,
    /// `(stream_off, out_off)` at the top of the decoder's main loop — the
    /// only points where no literals are owed by a previous instruction, so
    /// the only points where a fresh stream may be appended.
    pub cuts: Vec<(usize, usize)>,
    pub out_len: usize,
    pub stream_len: usize,
}

impl Scan {
    /// The stream offset holding the byte the output has at `out`, when that
    /// byte is a literal.
    pub fn literal_at(&self, out: usize) -> Option<usize> {
        let i = match self.lits.binary_search_by_key(&out, |r| r.out_off) {
            Ok(i) => i,
            Err(0) => return None,
            Err(i) => i - 1,
        };
        let r = self.lits[i];
        if out < r.out_off + r.len {
            Some(r.stream_off + (out - r.out_off))
        } else {
            None
        }
    }

    /// Reconstruct the output from the scan alone. This is the scan's own
    /// control: it uses only the literal runs and match arithmetic the scan
    /// recorded, so it agrees with liblzo2 only if the walk is faithful.
    pub fn replay(&self, src: &[u8], matches: &[(usize, usize, usize)]) -> Vec<u8> {
        let mut out = vec![0u8; self.out_len];
        let mut lit = self.lits.iter().peekable();
        let mut m = matches.iter().peekable();
        let mut op = 0usize;
        while op < self.out_len {
            if let Some(r) = lit.peek() {
                if r.out_off == op {
                    out[op..op + r.len].copy_from_slice(&src[r.stream_off..r.stream_off + r.len]);
                    op += r.len;
                    lit.next();
                    continue;
                }
            }
            let &&(mo, dist, len) = match m.peek() {
                Some(x) => x,
                None => break,
            };
            assert_eq!(mo, op, "scan: match/literal ordering");
            m.next();
            for i in 0..len {
                out[op + i] = out[op + i - dist];
            }
            op += len;
        }
        out
    }
}

/// Walk an LZO1X stream. Returns the scan and the match list
/// `(out_off, distance, len)` in output order.
///
/// A faithful mirror of `lzo1x_decompress_safe`; the `goto`s of the C are the
/// `Label` state machine here, in the same order and with the same names.
pub fn scan(src: &[u8]) -> Result<(Scan, Vec<(usize, usize, usize)>), String> {
    #[derive(Clone, Copy)]
    enum Label {
        Loop,
        FirstLiteralRun,
        Match,
        MatchNext(usize),
    }
    let mut lits: Vec<LitRun> = Vec::new();
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    let mut matches: Vec<(usize, usize, usize)> = Vec::new();
    let mut ip = 0usize;
    let mut op = 0usize;
    let mut t: usize;

    macro_rules! need {
        ($n:expr) => {
            if ip + $n > src.len() {
                return Err(format!("lzo scan: stream ends inside an instruction at {}", ip));
            }
        };
    }
    macro_rules! lit {
        ($n:expr) => {{
            let n: usize = $n;
            need!(n);
            lits.push(LitRun { out_off: op, stream_off: ip, len: n });
            ip += n;
            op += n;
        }};
    }

    let mut label = Label::Loop;
    need!(1);
    if src[0] > 17 {
        t = src[0] as usize - 17;
        ip = 1;
        if t < 4 {
            label = Label::MatchNext(t);
        } else {
            lit!(t);
            label = Label::FirstLiteralRun;
        }
    }
    t = 0;
    loop {
        match label {
            Label::Loop => {
                cuts.push((ip, op));
                need!(1);
                t = src[ip] as usize;
                ip += 1;
                if t >= 16 {
                    label = Label::Match;
                    continue;
                }
                if t == 0 {
                    loop {
                        need!(1);
                        if src[ip] != 0 {
                            break;
                        }
                        t += 255;
                        ip += 1;
                    }
                    t += 15 + src[ip] as usize;
                    ip += 1;
                }
                lit!(t + 3);
                label = Label::FirstLiteralRun;
            }
            Label::FirstLiteralRun => {
                need!(1);
                t = src[ip] as usize;
                ip += 1;
                if t >= 16 {
                    label = Label::Match;
                    continue;
                }
                need!(1);
                let dist = 1 + 0x0800 + (t >> 2) + ((src[ip] as usize) << 2);
                ip += 1;
                matches.push((op, dist, 3));
                op += 3;
                label = match_done(src, ip);
            }
            Label::MatchNext(n) => {
                lit!(n);
                need!(1);
                t = src[ip] as usize;
                ip += 1;
                label = Label::Match;
            }
            Label::Match => {
                let dist;
                let len;
                if t >= 64 {
                    need!(1);
                    dist = 1 + ((t >> 2) & 7) + ((src[ip] as usize) << 3);
                    ip += 1;
                    len = (t >> 5) + 1;
                } else if t >= 32 {
                    let mut l = t & 31;
                    if l == 0 {
                        loop {
                            need!(1);
                            if src[ip] != 0 {
                                break;
                            }
                            l += 255;
                            ip += 1;
                        }
                        l += 31 + src[ip] as usize;
                        ip += 1;
                    }
                    need!(2);
                    let w = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
                    ip += 2;
                    dist = 1 + (w >> 2);
                    len = l + 2;
                } else if t >= 16 {
                    let hi = (t & 8) << 11;
                    let mut l = t & 7;
                    if l == 0 {
                        loop {
                            need!(1);
                            if src[ip] != 0 {
                                break;
                            }
                            l += 255;
                            ip += 1;
                        }
                        l += 7 + src[ip] as usize;
                        ip += 1;
                    }
                    need!(2);
                    let w = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
                    ip += 2;
                    if hi == 0 && (w >> 2) == 0 {
                        // END OF STREAM
                        return Ok((
                            Scan { lits, cuts, out_len: op, stream_len: ip },
                            matches,
                        ));
                    }
                    dist = 0x4000 + hi + (w >> 2);
                    len = l + 2;
                } else {
                    need!(1);
                    dist = 1 + (t >> 2) + ((src[ip] as usize) << 2);
                    ip += 1;
                    len = 2;
                }
                if dist > op {
                    return Err(format!(
                        "lzo scan: match at output {} reaches {} bytes back, before the start",
                        op, dist
                    ));
                }
                matches.push((op, dist, len));
                op += len;
                label = match_done(src, ip);
            }
        }
    }

    fn match_done(src: &[u8], ip: usize) -> Label {
        let s = (src[ip - 2] & 3) as usize;
        if s == 0 {
            Label::Loop
        } else {
            Label::MatchNext(s)
        }
    }
}

// ------------------------------------------------------------------ encoding

/// Encode a literal run of `n` bytes in the form the decoder expects **inside**
/// a stream (not at its start). `None` when `n` has no such encoding: a
/// mid-stream run of 1 or 2 bytes is carried in the previous match's state
/// bits and one of exactly 3 cannot be spelled at all (opcode 0 means "long
/// form", so `t = n - 3 = 0` is unreachable).
fn encode_literal_run(n: usize) -> Option<Vec<u8>> {
    if n < 4 {
        return None;
    }
    let x = n - 3;
    if x < 16 {
        return Some(vec![x as u8]);
    }
    let mut out = vec![0u8];
    let mut rest = x - 15;
    while rest > 255 {
        out.push(0);
        rest -= 255;
    }
    out.push(rest as u8);
    Some(out)
}

/// Re-spell a freshly compressed stream's FIRST instruction so it can be
/// appended to another stream. `lzo1x_1_compress` may open with the
/// start-of-stream form (`byte > 17` = a literal run of `byte - 17`), which the
/// decoder only recognises at offset 0.
fn make_appendable(tail: &[u8]) -> Option<Vec<u8>> {
    if tail.is_empty() {
        return None;
    }
    if tail[0] <= 17 {
        return Some(tail.to_vec());
    }
    let n = tail[0] as usize - 17;
    let head = encode_literal_run(n)?;
    if 1 + n > tail.len() {
        return None;
    }
    let mut out = head;
    out.extend_from_slice(&tail[1..]);
    Some(out)
}

// ------------------------------------------------------------------ splicing

/// Produce the compressed stream for `new_body`, changing as little of
/// `stock_stream` as the edit allows.
///
/// `stock_stream` is the stock file's compressed payload and `stock_body` what
/// it decompresses to. The result is **verified**: whatever method is chosen,
/// the returned stream decompresses to exactly `new_body`.
pub fn splice(stock_stream: &[u8], stock_body: &[u8], new_body: &[u8]) -> Spliced {
    // The body's length changed (a rename, an item-model swap): no part of the
    // stock stream survives a shift, so this is the one case that re-emits.
    if stock_body.len() != new_body.len() {
        let stream = lzo_compress(new_body);
        verify(&stream, new_body, Method::Reemit);
        return Spliced {
            stream,
            method: Method::Reemit,
            shared_prefix: 0,
            shared_suffix: 0,
            stock_len: stock_stream.len(),
            changed_bytes: 0,
        };
    }

    let changed: Vec<usize> =
        (0..new_body.len()).filter(|&i| stock_body[i] != new_body[i]).collect();

    if changed.is_empty() {
        // Nothing to do: the stock stream IS the answer, bit for bit.
        verify(stock_stream, new_body, Method::Literal);
        return Spliced {
            stream: stock_stream.to_vec(),
            method: Method::Literal,
            shared_prefix: stock_stream.len(),
            shared_suffix: stock_stream.len(),
            stock_len: stock_stream.len(),
            changed_bytes: 0,
        };
    }

    let (sc, _matches) = match scan(stock_stream) {
        Ok(x) => x,
        Err(e) => panic!("cannot read the stock map's own compressed stream: {}", e),
    };
    assert_eq!(
        sc.out_len,
        stock_body.len(),
        "the stream walk and liblzo2 disagree about the body's length"
    );

    // --- method 1: overwrite the literals the stock stream already carries.
    // The whole file keeps its length and every other byte.
    let mut patched = stock_stream.to_vec();
    let mut all_literal = true;
    for &off in &changed {
        match sc.literal_at(off) {
            Some(s) => patched[s] = new_body[off],
            None => {
                all_literal = false;
                break;
            }
        }
    }
    if all_literal && lzo_decompress(&patched, new_body.len()) == new_body {
        return Spliced {
            shared_prefix: shared_prefix(stock_stream, &patched),
            shared_suffix: shared_suffix(stock_stream, &patched),
            stock_len: stock_stream.len(),
            stream: patched,
            method: Method::Literal,
            changed_bytes: changed.len(),
        };
    }

    // --- method 2: replace one stretch of the stream and keep the rest.
    //
    // An edited byte that the stock stream produces with a MATCH cannot be
    // overwritten — the byte is not in the stream at all. So recompress the
    // stretch that carries the edit and splice it between the stock stream's
    // own bytes.
    //
    // Resuming the stock stream afterwards is sound because an LZO match names
    // a DISTANCE, not an address: the output either side of the stretch is at
    // its original offsets and holds its original bytes, so every later
    // instruction means what it meant. The two conditions are that the resumed
    // opcode is a MATCH — the one opcode class that reads the same whether the
    // decoder arrives from a literal run or from the top of its loop — and that
    // no later match reaches back into the edited bytes, which is what the
    // verification at the end of each attempt is for.
    let first = *changed.first().unwrap();
    let last = *changed.last().unwrap();
    let mut cut = match sc.cuts.binary_search_by_key(&first, |c| c.1) {
        Ok(i) => i,
        Err(0) => panic!("no cut point at or before the first edited byte"),
        Err(i) => i - 1,
    };
    // Resume candidates: cut points past the last edited byte whose opcode is a
    // match. The nearest few are tried first and are almost always right; the
    // last candidate is unconditional — **no LZO1X match can reach further
    // back than 0xBFFF bytes**, so a resume that far past the edit cannot copy
    // it, whatever the stock stream does in between.
    let near = sc
        .cuts
        .iter()
        .filter(|(s, o)| *o > last && stock_stream[*s] >= 16)
        .take(RESUME_TRIES);
    let far = sc
        .cuts
        .iter()
        .find(|(s, o)| *o > last + LZO_MAX_DISTANCE && stock_stream[*s] >= 16);
    let resumes: Vec<(usize, usize)> = near.chain(far).cloned().collect();
    loop {
        let (cs, co) = sc.cuts[cut];
        for &(rs, ro) in &resumes {
            let mid = lzo_compress(&new_body[co..ro]);
            // A freshly compressed stream ends with the end-of-stream marker,
            // which must not appear in the middle of one.
            assert!(
                mid.len() >= 3 && mid[mid.len() - 3..] == [0x11, 0x00, 0x00],
                "lzo1x_1_compress did not end with the end-of-stream marker"
            );
            let mid = match make_appendable(&mid[..mid.len() - 3]) {
                Some(m) => m,
                None => continue,
            };
            let mut stream = stock_stream[..cs].to_vec();
            stream.extend_from_slice(&mid);
            stream.extend_from_slice(&stock_stream[rs..]);
            if lzo_decompress(&stream, new_body.len()) == new_body {
                return Spliced {
                    shared_prefix: shared_prefix(stock_stream, &stream),
                    shared_suffix: shared_suffix(stock_stream, &stream),
                    stock_len: stock_stream.len(),
                    stream,
                    method: Method::Middle,
                    changed_bytes: changed.len(),
                };
            }
        }
        // --- method 3: no resume point worked (an edit at the very end of the
        // body has none at all). Keep the stream to the cut and compress the
        // rest of the body.
        if let Some(tail) = make_appendable(&lzo_compress(&new_body[co..])) {
            let mut stream = stock_stream[..cs].to_vec();
            stream.extend_from_slice(&tail);
            if lzo_decompress(&stream, new_body.len()) == new_body {
                return Spliced {
                    shared_prefix: shared_prefix(stock_stream, &stream),
                    shared_suffix: shared_suffix(stock_stream, &stream),
                    stock_len: stock_stream.len(),
                    stream,
                    method: Method::Tail,
                    changed_bytes: changed.len(),
                };
            }
        }
        // An unspellable opening instruction (a 3-byte literal run cannot be
        // written mid-stream) or a stretch that did not verify: step the cut
        // back one instruction and try again.
        if cut == 0 {
            panic!("no cut point produced a stream that decompresses to the intended body");
        }
        cut -= 1;
    }
}

/// How many NEARBY resume points to try before jumping to the one that cannot
/// fail. Each costs a compression of the stretch plus a verification; the
/// first is usually the answer, and a near resume keeps the recompressed
/// stretch short.
const RESUME_TRIES: usize = 8;

/// The furthest back an LZO1X match can reach: `0x4000 + 0x4000 + 0x3FFF`.
/// Past this the stock stream physically cannot refer to the edited bytes.
const LZO_MAX_DISTANCE: usize = 0xBFFF;

fn shared_prefix(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

fn shared_suffix(a: &[u8], b: &[u8]) -> usize {
    a.iter().rev().zip(b.iter().rev()).take_while(|(x, y)| x == y).count()
}

fn verify(stream: &[u8], want: &[u8], m: Method) {
    let got = lzo_decompress(stream, want.len());
    assert!(
        got == want,
        "{}: the written stream does not decompress to the intended body",
        m.label()
    );
}

// ------------------------------------------------------------------ commands

/// Every fixed-size field in a map that this tool can write, as a labelled
/// body range. Used to say WHICH placement a differing byte belongs to, so a
/// body diff reads as "block #4649's cell" and not as "offset 118 344".
fn field_map(m: &crate::map::MapFile) -> Vec<(usize, usize, String)> {
    let mut v = Vec::new();
    for b in &m.blocks {
        v.push((b.coord_off, 3, format!("block#{} {} cell", b.index, b.name)));
        if let Some(o) = b.free_off {
            v.push((o, 12, format!("block#{} {} free position", b.index, b.name)));
            v.push((o + 12, 12, format!("block#{} {} free rotation", b.index, b.name)));
        }
    }
    for b in &m.baked {
        if let Some(o) = b.free_off {
            v.push((o, 12, format!("b{} {} free position", b.index, b.name)));
            v.push((o + 12, 12, format!("b{} {} free rotation", b.index, b.name)));
        }
    }
    for it in &m.items {
        v.push((it.pos_off, 12, format!("item#{} {} position", it.index, it.model)));
        v.push((it.yaw_off, 4, format!("item#{} {} yaw", it.index, it.model)));
        v.push((it.coord_off, 3, format!("item#{} {} cell", it.index, it.model)));
    }
    v.sort_by_key(|x| x.0);
    v
}

/// `tmmaps bodydiff A B` — what an edit actually changed, on the DECOMPRESSED
/// bodies, plus how much of the file itself the two share.
///
/// This is the control for the whole splice path, and it is the instrument that
/// says whether a map has been *edited* or *rebuilt*: a spliced edit differs
/// from its stock file in the bytes of the edit and nowhere else, and its
/// compressed stream shares a prefix with the stock stream. A re-emitted map
/// shares the header and then nothing.
pub fn cmd_bodydiff(args: &[String]) {
    let pa = std::path::Path::new(&args[2]);
    let pb = std::path::Path::new(&args[3]);
    let fa = std::fs::read(pa).unwrap_or_else(|e| panic!("{}: {}", pa.display(), e));
    let fb = std::fs::read(pb).unwrap_or_else(|e| panic!("{}: {}", pb.display(), e));
    let ma = crate::map::MapFile::load(pa);
    let mb = crate::map::MapFile::load(pb);
    let (a, b) = (&ma.gbx.body, &mb.gbx.body);
    println!("file      {:>10} -> {:>10} bytes", fa.len(), fb.len());
    let fshare = fa.iter().zip(&fb).take_while(|(x, y)| x == y).count();
    println!("           shared prefix {} bytes ({:.1} % of the stock file)",
             fshare, 100.0 * fshare as f64 / fa.len() as f64);
    // The file-level prefix stops at the compressed-size word whenever the
    // stream's length changed at all, so it says almost nothing on its own.
    // The streams are the honest comparison.
    if let (Some(ca), Some(cb)) = (&ma.gbx.comp, &mb.gbx.comp) {
        let p = ca.iter().zip(cb).take_while(|(x, y)| x == y).count();
        let s = ca.iter().rev().zip(cb.iter().rev()).take_while(|(x, y)| x == y).count();
        println!(
            "stream    {:>10} -> {:>10} bytes\n           shared {} at the front + {} at the back \
             = {:.2} % of the stock stream carried through verbatim",
            ca.len(),
            cb.len(),
            p,
            s,
            100.0 * (p + s).min(ca.len()) as f64 / ca.len() as f64
        );
    }
    println!("body      {:>10} -> {:>10} bytes", a.len(), b.len());
    if a.len() != b.len() {
        // A length change means something was RE-SERIALISED: a rename, an item
        // model swap, or a writer that rebuilt a chunk. Say where, because
        // "12 bytes shorter" is the whole diagnosis and the offset names the
        // record that shrank.
        let pre = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
        let suf = a
            .iter()
            .rev()
            .zip(b.iter().rev())
            .take_while(|(x, y)| x == y)
            .count()
            .min(a.len().min(b.len()) - pre);
        let who = fields_who(&field_map(&ma), pre, a.len() - suf);
        println!(
            "  THE BODIES ARE DIFFERENT LENGTHS: {} bytes. This edit RE-SERIALISED something —\n  \
             identical to {} and again from {} (A) / {} (B).\n  the changed span is {} bytes of A \
             against {} of B, starting in: {}",
            b.len() as i64 - a.len() as i64,
            pre,
            a.len() - suf,
            b.len() - suf,
            a.len() - suf - pre,
            b.len() - suf - pre,
            who
        );
        std::process::exit(1);
    }
    let fields = field_map(&ma);
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0usize;
    while i < a.len() {
        if a[i] != b[i] {
            let s = i;
            while i < a.len() && a[i] != b[i] {
                i += 1;
            }
            runs.push((s, i));
        }
        i += 1;
    }
    let total: usize = runs.iter().map(|(s, e)| e - s).sum();
    println!("body differs in {} bytes, in {} run(s):", total, runs.len());
    for (s, e) in &runs {
        println!("  @{:<9} {:>3} bytes   {}", s, e - s, fields_who(&fields, *s, *e));
    }
}

/// Which placement a body span belongs to, or a named refusal to guess.
fn fields_who(fields: &[(usize, usize, String)], s: usize, e: usize) -> String {
    fields
        .iter()
        .find(|(o, l, _)| *o < e && s < o + l)
        .map(|(_, _, n)| n.clone())
        .unwrap_or_else(|| "UNATTRIBUTED — not a fixed-size field this tool writes".to_string())
}

/// `tmmaps rewrite MAP --out F [--reemit]` — write a map back out with **no
/// edit at all**, and say by which method.
///
/// It exists to isolate one question that nothing else here can ask: *does the
/// game client mind the WRITER?* The default output is the stock file byte for
/// byte (the splice has nothing to change); `--reemit` is the same body with
/// its compressed stream rebuilt, which is what every map this project wrote
/// before the splice path existed. Load both in the client and the answer is
/// attributable to recompression alone — neither file has an edit in it.
pub fn cmd_rewrite(args: &[String]) {
    let src = std::path::Path::new(&args[2]);
    let out = crate::cli::flag(args, "--out").expect("--out FILE");
    let m = crate::map::MapFile::load(src);
    let body = m.patched_body();
    let (bytes, sp) = if crate::cli::has(args, "--reemit") {
        let stream = lzo_compress(&body);
        verify(&stream, &body, Method::Reemit);
        (
            m.gbx.file_with_stream(&body, &stream),
            Spliced { stream, method: Method::Reemit, shared_prefix: 0, shared_suffix: 0, changed_bytes: 0, stock_len: 0 },
        )
    } else {
        m.gbx.write_body(&body)
    };
    std::fs::write(out, &bytes).unwrap_or_else(|e| panic!("{}: {}", out, e));
    let stock = std::fs::read(src).unwrap();
    println!(
        "{} -> {}\n  method   {}\n  file     {} -> {} bytes\n  stream   {} shared at the front + \
         {} at the back\n  body     {} bytes, {} changed",
        src.display(),
        out,
        sp.method.label(),
        stock.len(),
        bytes.len(),
        sp.shared_prefix,
        sp.shared_suffix,
        body.len(),
        sp.changed_bytes,
    );
}
