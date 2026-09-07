//! A small JSON reader (the repo carries no serde; `gates.json` and the
//! route files are read with this and written by hand).

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

impl Json {
    pub fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.get(k),
            _ => None,
        }
    }
    pub fn f64(&self) -> Option<f64> {
        match self {
            Json::Num(x) => Some(*x),
            _ => None,
        }
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn arr(&self) -> Option<&Vec<Json>> {
        match self {
            Json::Arr(v) => Some(v),
            _ => None,
        }
    }
    pub fn vec3(&self) -> Option<[f64; 3]> {
        let a = self.arr()?;
        if a.len() != 3 {
            return None;
        }
        Some([a[0].f64()?, a[1].f64()?, a[2].f64()?])
    }
}

pub fn parse(s: &str) -> Result<Json, String> {
    let b = s.as_bytes();
    let mut i = 0;
    let v = value(b, &mut i)?;
    ws(b, &mut i);
    if i != b.len() {
        return Err(format!("trailing bytes at {}", i));
    }
    Ok(v)
}

fn ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\n' | b'\r' | b'\t') {
        *i += 1;
    }
}

fn value(b: &[u8], i: &mut usize) -> Result<Json, String> {
    ws(b, i);
    if *i >= b.len() {
        return Err("unexpected end".into());
    }
    match b[*i] {
        b'{' => {
            *i += 1;
            let mut m = BTreeMap::new();
            ws(b, i);
            if *i < b.len() && b[*i] == b'}' {
                *i += 1;
                return Ok(Json::Obj(m));
            }
            loop {
                ws(b, i);
                let k = match value(b, i)? {
                    Json::Str(s) => s,
                    _ => return Err(format!("object key is not a string at {}", i)),
                };
                ws(b, i);
                if *i >= b.len() || b[*i] != b':' {
                    return Err(format!("expected ':' at {}", i));
                }
                *i += 1;
                let v = value(b, i)?;
                m.insert(k, v);
                ws(b, i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(b'}') => {
                        *i += 1;
                        return Ok(Json::Obj(m));
                    }
                    _ => return Err(format!("expected ',' or '}}' at {}", i)),
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut v = Vec::new();
            ws(b, i);
            if *i < b.len() && b[*i] == b']' {
                *i += 1;
                return Ok(Json::Arr(v));
            }
            loop {
                v.push(value(b, i)?);
                ws(b, i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(b']') => {
                        *i += 1;
                        return Ok(Json::Arr(v));
                    }
                    _ => return Err(format!("expected ',' or ']' at {}", i)),
                }
            }
        }
        b'"' => {
            *i += 1;
            let mut s = String::new();
            loop {
                let c = *b.get(*i).ok_or("unterminated string")?;
                *i += 1;
                match c {
                    b'"' => return Ok(Json::Str(s)),
                    b'\\' => {
                        let e = *b.get(*i).ok_or("bad escape")?;
                        *i += 1;
                        match e {
                            b'n' => s.push('\n'),
                            b't' => s.push('\t'),
                            b'r' => s.push('\r'),
                            b'"' => s.push('"'),
                            b'\\' => s.push('\\'),
                            b'/' => s.push('/'),
                            b'u' => {
                                let h = std::str::from_utf8(b.get(*i..*i + 4).ok_or("bad \\u")?).map_err(|e| e.to_string())?;
                                *i += 4;
                                let cp = u32::from_str_radix(h, 16).map_err(|e| e.to_string())?;
                                s.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                            }
                            _ => return Err(format!("bad escape \\{}", e as char)),
                        }
                    }
                    _ => {
                        // copy a UTF-8 run
                        let start = *i - 1;
                        let mut end = *i;
                        while end < b.len() && b[end] != b'"' && b[end] != b'\\' {
                            end += 1;
                        }
                        s.push_str(std::str::from_utf8(&b[start..end]).map_err(|e| e.to_string())?);
                        *i = end;
                    }
                }
            }
        }
        b't' if b[*i..].starts_with(b"true") => {
            *i += 4;
            Ok(Json::Bool(true))
        }
        b'f' if b[*i..].starts_with(b"false") => {
            *i += 5;
            Ok(Json::Bool(false))
        }
        b'n' if b[*i..].starts_with(b"null") => {
            *i += 4;
            Ok(Json::Null)
        }
        _ => {
            let start = *i;
            while *i < b.len() && matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                *i += 1;
            }
            let t = std::str::from_utf8(&b[start..*i]).map_err(|e| e.to_string())?;
            t.parse::<f64>().map(Json::Num).map_err(|_| format!("bad number {:?} at {}", t, start))
        }
    }
}

/// Escape a string for JSON output.
pub fn quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_small() {
        let j = parse(r#"{"a": [1, 2.5, -3e2], "b": "x\"y", "c": null, "d": true}"#).unwrap();
        assert_eq!(j.get("a").unwrap().arr().unwrap()[2].f64(), Some(-300.0));
        assert_eq!(j.get("b").unwrap().str(), Some("x\"y"));
        assert_eq!(j.get("d"), Some(&Json::Bool(true)));
    }
}
