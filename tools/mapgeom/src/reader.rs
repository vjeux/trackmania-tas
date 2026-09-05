//! The byte-level reader every GBX body walk uses: little-endian scalars, GBX
//! strings, and the two stateful encodings a GBX body carries — the lookback
//! string table and the node graph's node references.
//!
//! The state is the whole point. A GBX body is not a sequence of independent
//! records: a lookback string can be a back-reference to the *n*-th string
//! written so far, and a node reference can be either "node 7, which you have
//! already read" or "node 7, whose bytes start right here". Read either one
//! with the wrong state and the stream desynchronises silently — you get
//! plausible floats out of the middle of somebody else's chunk. So the reader
//! owns that state and nothing else may fabricate it.

/// The 122 collection ids a lookback string may name instead of a string.
/// Only the ones this project has ever seen in Stadium data are named; an
/// unnamed id renders as `U<n>` rather than being silently dropped, because a
/// collection id in an unexpected place is evidence the walk desynchronised.
fn collection_id(idx: u32) -> String {
    match idx {
        0 => "Desert".into(),
        1 => "Snow".into(),
        2 => "Rally".into(),
        3 => "Island".into(),
        4 => "Bay".into(),
        5 => "Coast".into(),
        6 => "Stadium".into(),
        11 => "Valley".into(),
        12 => "Canyon".into(),
        13 => "Lagoon".into(),
        25 => "Stadium256".into(),
        26 => "Stadium".into(),
        10003 => "Common".into(),
        _ => format!("U{}", idx),
    }
}

pub struct Reader<'a> {
    pub b: &'a [u8],
    pub o: usize,
    /// The lookback table, in write order. Index 1 is the first string.
    pub lb: Vec<String>,
    /// Byte offsets of position-like floats met during the walk, as
    /// (offset, float count): what an in-place rescale must multiply. Filled
    /// only at the reads `classes.rs` marks; a scan that recovers past an
    /// unknown layout adds nothing here, which is why the rescaler refuses
    /// any recovery it has not vetted.
    pub marks: Vec<(usize, usize)>,
    /// Whether the one-per-body lookback version word has been consumed.
    lb_ver: bool,
}

#[derive(Debug)]
pub struct Overrun {
    pub want: usize,
    pub at: usize,
    pub len: usize,
}

impl std::fmt::Display for Overrun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "read {} bytes at 0x{:x} but the buffer is 0x{:x} long",
            self.want, self.at, self.len
        )
    }
}

pub type R<T> = Result<T, String>;

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Reader<'a> {
        Reader { b, o: 0, lb: Vec::new(), lb_ver: false, marks: Vec::new() }
    }

    /// A reader over a sub-slice that SHARES this reader's lookback table.
    /// A skippable chunk's payload is length-prefixed but its strings still
    /// index the body-wide table, so a nested walk that starts a fresh table
    /// reads the wrong strings.
    pub fn sub<T>(&mut self, n: usize, f: impl FnOnce(&mut Reader) -> R<T>) -> R<T> {
        let end = self.o.checked_add(n).ok_or("sub length overflow")?;
        if end > self.b.len() {
            return Err(format!("sub-chunk of {} bytes past end of body", n));
        }
        let mut r = Reader { b: &self.b[self.o..end], o: 0, lb: std::mem::take(&mut self.lb), lb_ver: self.lb_ver, marks: Vec::new() };
        let base = self.o;
        let out = f(&mut r);
        self.lb = std::mem::take(&mut r.lb);
        self.lb_ver = r.lb_ver;
        self.marks.extend(r.marks.into_iter().map(|(o, n)| (o + base, n)));
        self.o = end;
        out
    }

    /// Mark the next `n` floats as positions to rescale.
    pub fn mark(&mut self, n: usize) {
        self.marks.push((self.o, n));
    }

    pub fn left(&self) -> usize {
        self.b.len().saturating_sub(self.o)
    }
    pub fn eof(&self) -> bool {
        self.o >= self.b.len()
    }
    pub fn take(&mut self, n: usize) -> R<&'a [u8]> {
        let end = self.o.checked_add(n).ok_or("length overflow")?;
        if end > self.b.len() {
            return Err(Overrun { want: n, at: self.o, len: self.b.len() }.to_string());
        }
        let s = &self.b[self.o..end];
        self.o = end;
        Ok(s)
    }
    pub fn u8(&mut self) -> R<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> R<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> R<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> R<i32> {
        Ok(self.u32()? as i32)
    }
    pub fn f32(&mut self) -> R<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    pub fn vec3(&mut self) -> R<[f32; 3]> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
    pub fn vec2(&mut self) -> R<[f32; 2]> {
        Ok([self.f32()?, self.f32()?])
    }
    /// A GBX quaternion is stored x, y, z, w.
    pub fn quat(&mut self) -> R<[f32; 4]> {
        Ok([self.f32()?, self.f32()?, self.f32()?, self.f32()?])
    }
    pub fn iso4(&mut self) -> R<[f32; 12]> {
        let mut m = [0f32; 12];
        for v in m.iter_mut() {
            *v = self.f32()?;
        }
        Ok(m)
    }
    pub fn boxf(&mut self) -> R<[f32; 6]> {
        let mut m = [0f32; 6];
        for v in m.iter_mut() {
            *v = self.f32()?;
        }
        Ok(m)
    }
    pub fn bool32(&mut self) -> R<bool> {
        Ok(self.u32()? != 0)
    }
    pub fn bool8(&mut self) -> R<bool> {
        Ok(self.u8()? != 0)
    }
    pub fn peek_u32(&self) -> Option<u32> {
        if self.o + 4 > self.b.len() {
            return None;
        }
        Some(u32::from_le_bytes(self.b[self.o..self.o + 4].try_into().unwrap()))
    }
    pub fn string(&mut self) -> R<String> {
        let n = self.u32()? as usize;
        if n > 1 << 22 {
            return Err(format!("absurd string length {} at 0x{:x}", n, self.o - 4));
        }
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    /// A length-prefixed byte block.
    pub fn bytes_pfx(&mut self) -> R<&'a [u8]> {
        let n = self.u32()? as usize;
        self.take(n)
    }

    /// A GBX array with a u32 count, each element read by `f`.
    pub fn array<T>(&mut self, mut f: impl FnMut(&mut Self) -> R<T>) -> R<Vec<T>> {
        let n = self.u32()? as usize;
        // An array count is the single most load-bearing number in the walk: a
        // desynchronised stream shows up here first, as a count of millions.
        if n > 40_000_000 {
            return Err(format!("absurd array count {} at 0x{:x}", n, self.o - 4));
        }
        let mut v = Vec::with_capacity(n.min(1 << 16));
        for _ in 0..n {
            v.push(f(self)?);
        }
        Ok(v)
    }

    /// The GBX "lookback" string: either a literal (which joins the table), a
    /// back-reference into the table, or a collection id.
    pub fn lookback(&mut self) -> R<String> {
        if !self.lb_ver {
            let v = self.u32()?;
            if v != 3 {
                return Err(format!("lookback version {} (expected 3) at 0x{:x}", v, self.o - 4));
            }
            self.lb_ver = true;
        }
        let raw = self.u32()?;
        let flags = raw >> 30;
        let idx = raw & 0x3FFF_FFFF;
        if idx == 0x3FFF_FFFF {
            return Ok(match flags {
                2 => "Unassigned".into(),
                _ => String::new(),
            });
        }
        if flags == 0 || flags == 3 {
            return Ok(collection_id(idx));
        }
        if idx == 0 {
            let s = self.string()?;
            self.lb.push(s.clone());
            return Ok(s);
        }
        match self.lb.get(idx as usize - 1) {
            Some(s) => Ok(s.clone()),
            None => Err(format!(
                "lookback index {} but only {} strings written (at 0x{:x})",
                idx,
                self.lb.len(),
                self.o - 4
            )),
        }
    }

    /// An `Ident`: id, collection, author — three lookback strings.
    pub fn meta(&mut self) -> R<(String, String, String)> {
        Ok((self.lookback()?, self.lookback()?, self.lookback()?))
    }
}

/// A step of a body walk, printed when `MAPGEOM_TRACE=1` is set.
///
/// Writing a reader for a class nobody has decoded is a matter of watching the
/// cursor and the byte you are about to consume. This is that watch, and it is
/// the reason a new chunk can be added in minutes rather than by bisection.
pub fn trace(f: impl FnOnce() -> String) {
    use std::sync::OnceLock;
    static ON: OnceLock<bool> = OnceLock::new();
    if *ON.get_or_init(|| std::env::var("MAPGEOM_TRACE").is_ok_and(|v| v != "0")) {
        eprintln!("{}", f());
    }
}
