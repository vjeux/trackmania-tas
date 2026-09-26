//! Small encoding helpers. All platforms; unit-tested on Linux.

/// Decode lowercase/uppercase hex (no 0x prefix, no whitespace except ends).
pub fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err("odd hex length".to_string());
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len() / 2);
    let mut i = 0;
    while i < b.len() {
        let hi = hexval(b[i])?;
        let lo = hexval(b[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hexval(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("bad hex char 0x{c:02x}")),
    }
}

pub fn hex_encode(b: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for &x in b {
        s.push(H[(x >> 4) as usize] as char);
        s.push(H[(x & 15) as usize] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let v = vec![0x00, 0x01, 0xfe, 0xff, 0x41, 0x42];
        assert_eq!(hex_decode(&hex_encode(&v)).unwrap(), v);
    }

    #[test]
    fn upper_and_mixed() {
        assert_eq!(hex_decode("Ab09").unwrap(), vec![0xab, 0x09]);
    }

    #[test]
    fn rejects_odd_and_bad() {
        assert!(hex_decode("abc").is_err());
        assert!(hex_decode("zz").is_err());
        assert!(hex_decode("").unwrap().is_empty());
    }
}
