//! A minimal DXBC (Direct3D shader bytecode) reader: the SHEX/SHDR token stream walked instruction by
//! instruction, the IMMEDIATE32 operands collected with their exact f32 bits. The disassembly RenderDoc prints
//! rounds literals to six decimals (`0.093506`), which leaves ~100 f32 candidates; the token stream has the bits.
//! Used by `lmtool dxbc-literals FILE` and the H-basis kernel's constants (lmaccum.rs).

/// One instruction's literals: (instruction index, opcode, the f32 immediates in operand order).
#[derive(Clone, Debug)]
pub struct InstLiterals {
    pub index: usize,
    pub opcode: u32,
    pub values: Vec<u32>,
}

/// The chunk `fourcc`'s payload of a DXBC container.
pub fn chunk<'a>(dxbc: &'a [u8], fourcc: &[u8; 4]) -> Option<&'a [u8]> {
    if dxbc.len() < 32 || &dxbc[..4] != b"DXBC" {
        return None;
    }
    let u = |o: usize| u32::from_le_bytes(dxbc[o..o + 4].try_into().unwrap()) as usize;
    let n = u(28);
    for i in 0..n {
        let off = u(32 + 4 * i);
        if off + 8 > dxbc.len() {
            return None;
        }
        if &dxbc[off..off + 4] == fourcc {
            let size = u(off + 4);
            return dxbc.get(off + 8..off + 8 + size);
        }
    }
    None
}

/// The token stream of the shader (SHEX for SM5, SHDR for SM4): (program type, major, minor, tokens after the
/// two header dwords).
pub fn tokens(dxbc: &[u8]) -> Option<(u32, u32, u32, Vec<u32>)> {
    let sh = chunk(dxbc, b"SHEX").or_else(|| chunk(dxbc, b"SHDR"))?;
    let t: Vec<u32> = sh.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    if t.len() < 2 {
        return None;
    }
    let v = t[0];
    let (minor, major, ptype) = (v & 0xf, (v >> 4) & 0xf, v >> 16);
    let len = t[1] as usize;
    Some((ptype, major, minor, t[2..len.min(t.len())].to_vec()))
}

/// The length in dwords of the operand at `t[p..]` (the operand token, its extended tokens, indices and
/// immediates), and the immediate32 values it carries.
fn operand(t: &[u32], p: usize) -> (usize, Vec<u32>) {
    let tok = t[p];
    let mut len = 1;
    // extended operand tokens (bit 31 chains)
    let mut ext = tok;
    while ext & 0x8000_0000 != 0 && p + len < t.len() {
        ext = t[p + len];
        len += 1;
    }
    let num_comp = tok & 3;
    let otype = (tok >> 12) & 0xff;
    let mut vals = Vec::new();
    if otype == 4 || otype == 5 {
        // IMMEDIATE32 / IMMEDIATE64: 1 or 4 components
        let n = match num_comp { 1 => 1, 2 => 4, _ => 0 } * if otype == 5 { 2 } else { 1 };
        for i in 0..n {
            if let Some(v) = t.get(p + len + i) { vals.push(*v); }
        }
        len += n;
        return (len, vals);
    }
    let dim = (tok >> 20) & 3;
    for i in 0..dim as usize {
        let rep = (tok >> (22 + 3 * i)) & 7;
        match rep {
            0 => len += 1,                                            // immediate32 index
            1 => len += 2,                                            // immediate64
            2 => { let (l, _) = operand(t, p + len); len += l; }      // relative
            3 => { len += 1; let (l, _) = operand(t, p + len); len += l; }
            4 => { len += 2; let (l, _) = operand(t, p + len); len += l; }
            _ => {}
        }
    }
    (len, vals)
}

/// Every instruction's immediate32 literals, in order (declarations included; custom-data blocks skipped).
pub fn literals(t: &[u32]) -> Vec<InstLiterals> {
    let mut out = Vec::new();
    let mut p = 0usize;
    let mut idx = 0usize;
    while p < t.len() {
        let tok = t[p];
        let opcode = tok & 0x7ff;
        if opcode == 0x2a {
            // CUSTOMDATA: the length is the next dword (in dwords, including both)
            let l = t.get(p + 1).copied().unwrap_or(2) as usize;
            p += l.max(2);
            continue;
        }
        let mut len = ((tok >> 24) & 0x7f) as usize;
        if len == 0 { len = 1; }
        let end = (p + len).min(t.len());
        // opcode token + extended opcode tokens
        let mut q = p + 1;
        let mut ext = tok;
        while ext & 0x8000_0000 != 0 && q < end {
            ext = t[q];
            q += 1;
        }
        // declarations carry payloads that are not operands (names, layouts): only walk the arithmetic range
        let is_decl = (0x59..=0x6f).contains(&opcode) || (0x9c..=0xb2).contains(&opcode) || opcode == 0x35 || opcode == 0x68;
        let mut vals = Vec::new();
        if !is_decl {
            while q < end {
                let (l, v) = operand(t, q);
                vals.extend(v);
                q += l.max(1);
            }
        }
        out.push(InstLiterals { index: idx, opcode, values: vals });
        idx += 1;
        p = end;
    }
    out
}

/// The input/output signature (ISGN / OSGN / PCSG chunks): (semantic name, semantic index, register, mask).
pub fn signature(dxbc: &[u8], fourcc: &[u8; 4]) -> Vec<(String, u32, u32, u32)> {
    let Some(c) = chunk(dxbc, fourcc).or_else(|| if fourcc == b"ISGN" { chunk(dxbc, b"ISG1") } else { None }) else { return Vec::new() };
    let u = |o: usize| u32::from_le_bytes(c[o..o + 4].try_into().unwrap());
    if c.len() < 8 { return Vec::new(); }
    let n = u(0) as usize;
    let stride = if fourcc == b"ISG1" || fourcc == b"OSG1" { 32 } else { 24 };
    let mut out = Vec::new();
    for i in 0..n {
        let e = 8 + i * stride;
        if e + stride > c.len() { break; }
        let (name_off, sem_idx, reg, mask) = (u(e) as usize, u(e + 4), u(e + 16), u(e + 20) & 0xff);
        let name: String = c.get(name_off..).map(|s| s.iter().take_while(|b| **b != 0).map(|b| *b as char).collect()).unwrap_or_default();
        out.push((name, sem_idx, reg, mask));
    }
    out
}

/// The literal f32s of a shader in instruction order, with their bits.
pub fn literal_f32s(dxbc: &[u8]) -> Vec<(usize, u32, f32, u32)> {
    let Some((_, _, _, t)) = tokens(dxbc) else { return Vec::new() };
    let mut out = Vec::new();
    for il in literals(&t) {
        for v in il.values {
            out.push((il.index, il.opcode, f32::from_bits(v), v));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_immediate_operand_is_read_with_its_components() {
        // a 4-component immediate32 operand: token type 4 (bits 12-19), num_comp 2, then four dwords
        let tok = (4u32 << 12) | 2;
        let t = [tok, 1.0f32.to_bits(), 2.0f32.to_bits(), 0, 0];
        let (len, vals) = operand(&t, 0);
        assert_eq!(len, 5);
        assert_eq!(vals, vec![1.0f32.to_bits(), 2.0f32.to_bits(), 0, 0]);
        // a 1-component one
        let tok1 = (4u32 << 12) | 1;
        let (len, vals) = operand(&[tok1, 0.5f32.to_bits()], 0);
        assert_eq!((len, vals), (2, vec![0.5f32.to_bits()]));
        // a temp register r0.xyzw with one immediate index: type 0, dim 1, rep 0
        let tokr = (1u32 << 20) | 2;
        assert_eq!(operand(&[tokr, 0], 0).0, 2);
    }
}
