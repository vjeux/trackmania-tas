//! A small strict JSON reader.
//!
//! Strict on purpose: it rejects the bare `NaN` / `Infinity` tokens CPython's
//! `json.dump` happily emits and browsers' `JSON.parse` refuses. Anything this
//! parser accepts, `JSON.parse` accepts, so it doubles as the payload validator
//! used by `tmsite stats`.

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    /// `is_int` records that the literal had no `.`/`e`, so it can be echoed
    /// back as an integer.
    Num { f: f64, is_int: bool },
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(kv) => kv.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Num { f, .. } => Some(*f),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Num { f, .. } => Some(*f as i64),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[Value]> {
        match self {
            Value::Arr(a) => Some(a),
            _ => None,
        }
    }
}

pub fn parse(s: &str) -> Result<Value, String> {
    let b = s.as_bytes();
    let mut p = P { b, i: 0 };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i != b.len() {
        return Err(format!("trailing data at byte {}", p.i));
    }
    Ok(v)
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn err<T>(&self, msg: &str) -> Result<T, String> {
        let ctx: String = String::from_utf8_lossy(
            &self.b[self.i.saturating_sub(20)..(self.i + 20).min(self.b.len())],
        )
        .into_owned();
        Err(format!("{} at byte {} near {:?}", msg, self.i, ctx))
    }
    fn lit(&mut self, w: &str) -> bool {
        if self.b[self.i..].starts_with(w.as_bytes()) {
            self.i += w.len();
            true
        } else {
            false
        }
    }
    fn value(&mut self) -> Result<Value, String> {
        if self.i >= self.b.len() {
            return self.err("unexpected end");
        }
        match self.b[self.i] {
            b'{' => self.obj(),
            b'[' => self.arr(),
            b'"' => Ok(Value::Str(self.string()?)),
            b't' => {
                if self.lit("true") {
                    Ok(Value::Bool(true))
                } else {
                    self.err("bad token")
                }
            }
            b'f' => {
                if self.lit("false") {
                    Ok(Value::Bool(false))
                } else {
                    self.err("bad token")
                }
            }
            b'n' => {
                if self.lit("null") {
                    Ok(Value::Null)
                } else {
                    self.err("bad token")
                }
            }
            b'N' => self.err("bare NaN is not JSON"),
            b'I' => self.err("bare Infinity is not JSON"),
            b'-' if self.b[self.i..].starts_with(b"-Infinity") => {
                self.err("bare -Infinity is not JSON")
            }
            b'-' | b'0'..=b'9' => self.num(),
            _ => self.err("unexpected character"),
        }
    }
    fn obj(&mut self) -> Result<Value, String> {
        self.i += 1;
        let mut kv = Vec::new();
        self.ws();
        if self.i < self.b.len() && self.b[self.i] == b'}' {
            self.i += 1;
            return Ok(Value::Obj(kv));
        }
        loop {
            self.ws();
            let k = self.string()?;
            self.ws();
            if self.i >= self.b.len() || self.b[self.i] != b':' {
                return self.err("expected ':'");
            }
            self.i += 1;
            self.ws();
            let v = self.value()?;
            kv.push((k, v));
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Obj(kv));
                }
                _ => return self.err("expected ',' or '}'"),
            }
        }
    }
    fn arr(&mut self) -> Result<Value, String> {
        self.i += 1;
        let mut a = Vec::new();
        self.ws();
        if self.i < self.b.len() && self.b[self.i] == b']' {
            self.i += 1;
            return Ok(Value::Arr(a));
        }
        loop {
            self.ws();
            a.push(self.value()?);
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::Arr(a));
                }
                _ => return self.err("expected ',' or ']'"),
            }
        }
    }
    fn string(&mut self) -> Result<String, String> {
        if self.b.get(self.i) != Some(&b'"') {
            return self.err("expected string");
        }
        self.i += 1;
        let mut out = String::new();
        loop {
            let c = match self.b.get(self.i) {
                Some(c) => *c,
                None => return self.err("unterminated string"),
            };
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = *self.b.get(self.i).ok_or("unterminated escape")?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let h = std::str::from_utf8(&self.b[self.i..self.i + 4])
                                .map_err(|e| e.to_string())?;
                            let cp = u32::from_str_radix(h, 16).map_err(|e| e.to_string())?;
                            self.i += 4;
                            out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                        }
                        _ => return self.err("bad escape"),
                    }
                }
                _ => {
                    // copy the raw utf-8 byte run
                    let start = self.i - 1;
                    while self.i < self.b.len()
                        && self.b[self.i] != b'"'
                        && self.b[self.i] != b'\\'
                    {
                        self.i += 1;
                    }
                    out.push_str(&String::from_utf8_lossy(&self.b[start..self.i]));
                }
            }
        }
    }
    fn num(&mut self) -> Result<Value, String> {
        let start = self.i;
        if self.b[self.i] == b'-' {
            self.i += 1;
        }
        let mut is_int = true;
        while self.i < self.b.len() {
            match self.b[self.i] {
                b'0'..=b'9' => self.i += 1,
                b'.' | b'e' | b'E' | b'+' | b'-' => {
                    is_int = false;
                    self.i += 1;
                }
                _ => break,
            }
        }
        let t = std::str::from_utf8(&self.b[start..self.i]).map_err(|e| e.to_string())?;
        let f: f64 = t.parse().map_err(|_| format!("bad number {:?}", t))?;
        Ok(Value::Num { f, is_int })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_python_nan_and_infinity() {
        assert!(parse("[1.0, NaN]").is_err());
        assert!(parse("[Infinity]").is_err());
        assert!(parse("[-Infinity]").is_err());
        assert!(parse("[1.0, 2]").is_ok());
    }

    #[test]
    fn parses_a_trajectory_shape() {
        let v = parse(r#"{"name":"a","time_ms":19538,"samples":[{"t":0,"x":1.5}]}"#).unwrap();
        assert_eq!(v.get("name").unwrap().as_str(), Some("a"));
        assert_eq!(v.get("time_ms").unwrap().as_i64(), Some(19538));
        assert_eq!(v.get("samples").unwrap().as_arr().unwrap().len(), 1);
    }
}
