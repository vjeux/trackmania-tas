//! A reader for the decomp's C DATA files — `Type name[] = { ... };` blocks of
//! nested braces, macro calls and integer expressions — just enough to lift
//! Mario Kart 64's course tables out of `courses/<course>/*.c` without a C
//! compiler. Comments and preprocessor lines are dropped first; every array
//! becomes a tree of [`Node`]s.

use std::collections::HashMap;

/// One parsed element of an initializer.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// A brace initializer `{ a, b, ... }`.
    List(Vec<Node>),
    /// A macro or function call `name(args...)`.
    Call(String, Vec<Node>),
    /// Anything else: an expression's raw text (`-139`, `0x2000`,
    /// `G_TX_NOMIRROR | G_TX_CLAMP`, a symbol).
    Expr(String),
}

impl Node {
    pub fn list(&self) -> Option<&[Node]> {
        match self {
            Node::List(v) => Some(v),
            _ => None,
        }
    }
    pub fn call(&self) -> Option<(&str, &[Node])> {
        match self {
            Node::Call(n, a) => Some((n.as_str(), a)),
            _ => None,
        }
    }
    pub fn text(&self) -> Option<&str> {
        match self {
            Node::Expr(s) => Some(s.as_str()),
            _ => None,
        }
    }
    /// The element as an integer, evaluating `|`, `&`, `<<`, `>>`, `+`, `-`,
    /// `*`, parentheses and the names in `consts`.
    pub fn int(&self, consts: &Consts) -> Option<i64> {
        match self {
            Node::Expr(s) => eval(s, consts),
            _ => None,
        }
    }
    /// The element as a bare identifier (a symbol reference).
    pub fn ident(&self) -> Option<&str> {
        let s = self.text()?.trim();
        if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !s.chars().next().unwrap().is_ascii_digit() {
            Some(s)
        } else {
            None
        }
    }
}

/// Named integer constants for expression evaluation.
pub type Consts = HashMap<&'static str, i64>;

/// `Type name[] = { ... }` (or `Type name = { ... }`) blocks of a file, by name.
/// The type is kept for callers that want to filter (`Gfx`, `TrackSections`).
#[derive(Debug)]
pub struct Array {
    pub ty: String,
    pub name: String,
    pub items: Vec<Node>,
}

/// Strip `//` and `/* */` comments and preprocessor lines (an `#include` of a
/// generated `.inc.c` inside an initializer just leaves an empty array).
pub fn strip(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let b = src.as_bytes();
    let mut i = 0;
    let mut line_start = true;
    while i < b.len() {
        let c = b[i];
        if line_start && c == b'#' {
            // `#include "x.inc.c"` inside an initializer is the array's whole
            // content (a texture symbol aliasing an asset): keep it as a call
            if b[i..].starts_with(b"#include \"") {
                let start = i + "#include ".len();
                let mut j = start + 1;
                while j < b.len() && b[j] != b'"' {
                    j += 1;
                }
                out.push_str("__include(");
                out.push_str(std::str::from_utf8(&b[start..=j.min(b.len() - 1)]).unwrap_or(""));
                out.push_str("),");
                i = j + 1;
                continue;
            }
            while i < b.len() && b[i] != b'\n' {
                // a preprocessor line may continue with a backslash
                if b[i] == b'\\' && i + 1 < b.len() && b[i + 1] == b'\n' {
                    i += 2;
                    continue;
                }
                i += 1;
            }
            continue;
        }
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            out.push(' ');
            continue;
        }
        if c == b'"' {
            // string literal: copy verbatim
            out.push('"');
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' && i + 1 < b.len() {
                    out.push(b[i] as char);
                    i += 1;
                }
                out.push(b[i] as char);
                i += 1;
            }
            out.push('"');
            i += 1;
            line_start = false;
            continue;
        }
        out.push(c as char);
        line_start = c == b'\n' || (line_start && (c == b' ' || c == b'\t'));
        i += 1;
    }
    out
}

/// Every top-level initializer of a (stripped) source: `... name[] = { ... };`
/// and `... name = { ... };`. Scalar definitions (`f32 x = 1.0f;`) are skipped.
pub fn arrays(src: &str) -> Vec<Array> {
    let mut out = Vec::new();
    let b = src.as_bytes();
    let mut i = 0;
    while let Some(eq) = find_from(b, i, b'=') {
        // the initializer must start with `{`
        let mut j = eq + 1;
        while j < b.len() && (b[j] == b' ' || b[j] == b'\n' || b[j] == b'\t' || b[j] == b'\r') {
            j += 1;
        }
        if j >= b.len() || b[j] != b'{' {
            i = eq + 1;
            continue;
        }
        // the declaration is the text since the previous `;` (or start)
        let decl_start = rfind_before(b, eq, b';').map(|p| p + 1).unwrap_or(0);
        let decl = std::str::from_utf8(&b[decl_start..eq]).unwrap_or("").trim();
        let (ty, name) = split_decl(decl);
        let (items, end) = parse_list(b, j);
        if !name.is_empty() {
            out.push(Array { ty, name, items });
        }
        i = end;
    }
    out
}

fn find_from(b: &[u8], from: usize, c: u8) -> Option<usize> {
    b[from..].iter().position(|&x| x == c).map(|p| p + from)
}

fn rfind_before(b: &[u8], before: usize, c: u8) -> Option<usize> {
    b[..before].iter().rposition(|&x| x == c)
}

/// `TrackSections d_course_x_addr[]` → ("TrackSections", "d_course_x_addr");
/// `struct ActorSpawnData d_x[]` → ("struct ActorSpawnData", "d_x").
fn split_decl(decl: &str) -> (String, String) {
    let decl = decl.trim().trim_start_matches("static ").trim_start_matches("const ");
    let mut words: Vec<&str> = decl.split_whitespace().collect();
    if words.is_empty() {
        return (String::new(), String::new());
    }
    let last = words.pop().unwrap();
    let name = last.split('[').next().unwrap_or(last).trim_start_matches('*').to_string();
    (words.join(" "), name)
}

/// Parse a `{ ... }` starting at `b[open] == b'{'`; returns the elements and
/// the index just past the closing brace.
fn parse_list(b: &[u8], open: usize) -> (Vec<Node>, usize) {
    debug_assert_eq!(b[open], b'{');
    let mut items = Vec::new();
    let mut i = open + 1;
    loop {
        // skip whitespace and stray commas
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b',') {
            i += 1;
        }
        if i >= b.len() {
            return (items, i);
        }
        if b[i] == b'}' {
            return (items, i + 1);
        }
        let (node, next) = parse_element(b, i);
        items.push(node);
        i = next;
    }
}

/// One element up to the next `,` or closing bracket at depth 0.
fn parse_element(b: &[u8], start: usize) -> (Node, usize) {
    if b[start] == b'{' {
        let (items, end) = parse_list(b, start);
        return (Node::List(items), end);
    }
    // scan the raw text of the element, tracking parentheses
    let mut depth = 0i32;
    let mut i = start;
    let mut first_paren: Option<usize> = None;
    while i < b.len() {
        let c = b[i];
        if c == b'(' {
            if depth == 0 && first_paren.is_none() {
                first_paren = Some(i);
            }
            depth += 1;
        } else if c == b')' {
            depth -= 1;
            if depth < 0 {
                break;
            }
        } else if depth == 0 && (c == b',' || c == b'}') {
            break;
        } else if c == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                i += 1;
            }
        }
        i += 1;
    }
    let text = std::str::from_utf8(&b[start..i]).unwrap_or("").trim();
    // a call: `name(` ... `)` with nothing after the closing paren
    if let Some(p) = first_paren {
        let name = std::str::from_utf8(&b[start..p]).unwrap_or("").trim();
        let is_ident = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if is_ident && text.ends_with(')') {
            let inner_start = p + 1;
            let inner_end = i - 1; // index of the closing paren
            let args = parse_args(&b[inner_start..inner_end.max(inner_start)]);
            return (Node::Call(name.to_string(), args), i);
        }
    }
    (Node::Expr(text.to_string()), i)
}

/// Comma-separated arguments (depth-aware), each parsed as an element.
fn parse_args(b: &[u8]) -> Vec<Node> {
    let mut args = Vec::new();
    let mut i = 0;
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let (node, next) = parse_element(b, i);
        args.push(node);
        i = next;
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b',') {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
    }
    args
}

/// Integer expression evaluation: `|`, `&`, `^`, `<<`, `>>`, `+`, `-`, `*`,
/// `/`, unary minus, parentheses, decimal/hex literals (with C suffixes),
/// and named constants. Unknown names evaluate to None.
pub fn eval(expr: &str, consts: &Consts) -> Option<i64> {
    let toks = tokenize(expr)?;
    let mut p = Parser { toks, pos: 0, consts };
    let v = p.parse_or()?;
    if p.pos != p.toks.len() {
        return None;
    }
    Some(v)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(i64),
    Name(String),
    Op(&'static str),
}

fn tokenize(s: &str) -> Option<Vec<Tok>> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            let hex = c == b'0' && i + 1 < b.len() && (b[i + 1] == b'x' || b[i + 1] == b'X');
            if hex {
                i += 2;
                while i < b.len() && b[i].is_ascii_hexdigit() {
                    i += 1;
                }
                let v = i64::from_str_radix(std::str::from_utf8(&b[start + 2..i]).ok()?, 16).ok()?;
                out.push(Tok::Num(v));
            } else {
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                let v: i64 = std::str::from_utf8(&b[start..i]).ok()?.parse().ok()?;
                out.push(Tok::Num(v));
            }
            // C suffixes
            while i < b.len() && (b[i] == b'u' || b[i] == b'U' || b[i] == b'l' || b[i] == b'L') {
                i += 1;
            }
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(Tok::Name(std::str::from_utf8(&b[start..i]).ok()?.to_string()));
            continue;
        }
        let two = if i + 1 < b.len() { &b[i..i + 2] } else { &b[i..i + 1] };
        let op: &'static str = match two {
            b"<<" => "<<",
            b">>" => ">>",
            _ => match c {
                b'|' => "|",
                b'&' => "&",
                b'^' => "^",
                b'+' => "+",
                b'-' => "-",
                b'*' => "*",
                b'/' => "/",
                b'(' => "(",
                b')' => ")",
                b'~' => "~",
                _ => return None,
            },
        };
        i += op.len();
        out.push(Tok::Op(op));
    }
    Some(out)
}

struct Parser<'a> {
    toks: Vec<Tok>,
    pos: usize,
    consts: &'a Consts,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn eat(&mut self, op: &str) -> bool {
        if let Some(Tok::Op(o)) = self.peek() {
            if *o == op {
                self.pos += 1;
                return true;
            }
        }
        false
    }
    fn parse_or(&mut self) -> Option<i64> {
        let mut v = self.parse_xor()?;
        while self.eat("|") {
            v |= self.parse_xor()?;
        }
        Some(v)
    }
    fn parse_xor(&mut self) -> Option<i64> {
        let mut v = self.parse_and()?;
        while self.eat("^") {
            v ^= self.parse_and()?;
        }
        Some(v)
    }
    fn parse_and(&mut self) -> Option<i64> {
        let mut v = self.parse_shift()?;
        while self.eat("&") {
            v &= self.parse_shift()?;
        }
        Some(v)
    }
    fn parse_shift(&mut self) -> Option<i64> {
        let mut v = self.parse_add()?;
        loop {
            if self.eat("<<") {
                v <<= self.parse_add()?;
            } else if self.eat(">>") {
                v >>= self.parse_add()?;
            } else {
                return Some(v);
            }
        }
    }
    fn parse_add(&mut self) -> Option<i64> {
        let mut v = self.parse_mul()?;
        loop {
            if self.eat("+") {
                v += self.parse_mul()?;
            } else if self.eat("-") {
                v -= self.parse_mul()?;
            } else {
                return Some(v);
            }
        }
    }
    fn parse_mul(&mut self) -> Option<i64> {
        let mut v = self.parse_unary()?;
        loop {
            if self.eat("*") {
                v *= self.parse_unary()?;
            } else if self.eat("/") {
                let d = self.parse_unary()?;
                if d == 0 {
                    return None;
                }
                v /= d;
            } else {
                return Some(v);
            }
        }
    }
    fn parse_unary(&mut self) -> Option<i64> {
        if self.eat("-") {
            return Some(-self.parse_unary()?);
        }
        if self.eat("~") {
            return Some(!self.parse_unary()?);
        }
        if self.eat("+") {
            return self.parse_unary();
        }
        if self.eat("(") {
            let v = self.parse_or()?;
            if !self.eat(")") {
                return None;
            }
            return Some(v);
        }
        match self.peek().cloned() {
            Some(Tok::Num(n)) => {
                self.pos += 1;
                Some(n)
            }
            Some(Tok::Name(n)) => {
                self.pos += 1;
                self.consts.get(n.as_str()).copied()
            }
            _ => None,
        }
    }
}

/// The `gbi.h` / decomp constants the course data uses.
pub fn gbi_consts() -> Consts {
    let mut c = Consts::new();
    for (k, v) in [
        ("G_IM_FMT_RGBA", 0),
        ("G_IM_FMT_YUV", 1),
        ("G_IM_FMT_CI", 2),
        ("G_IM_FMT_IA", 3),
        ("G_IM_FMT_I", 4),
        ("G_IM_SIZ_4b", 0),
        ("G_IM_SIZ_8b", 1),
        ("G_IM_SIZ_16b", 2),
        ("G_IM_SIZ_32b", 3),
        ("G_TX_LOADTILE", 7),
        ("G_TX_RENDERTILE", 0),
        ("G_TX_NOMIRROR", 0),
        ("G_TX_WRAP", 0),
        ("G_TX_MIRROR", 1),
        ("G_TX_CLAMP", 2),
        ("G_TX_NOMASK", 0),
        ("G_TX_NOLOD", 0),
        ("G_ON", 1),
        ("G_OFF", 0),
        // geometry mode (F3DEX)
        ("G_ZBUFFER", 0x1),
        ("G_SHADE", 0x4),
        ("G_SHADING_SMOOTH", 0x200),
        ("G_CULL_FRONT", 0x1000),
        ("G_CULL_BACK", 0x2000),
        ("G_CULL_BOTH", 0x3000),
        ("G_FOG", 0x10000),
        ("G_LIGHTING", 0x20000),
        ("G_TEXTURE_GEN", 0x40000),
        ("G_TEXTURE_GEN_LINEAR", 0x80000),
        ("G_LOD", 0x100000),
        ("G_CLIPPING", 0x800000),
        ("G_TT_NONE", 0),
        ("G_TT_RGBA16", 0x8000),
        ("G_TT_IA16", 0xC000),
        ("NUMLIGHTS_0", 1),
        ("NUMLIGHTS_1", 1),
        ("NUMLIGHTS_2", 2),
    ] {
        c.insert(k, v);
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_initializers_and_calls() {
        let src = r#"
// comment
#include <x.h>
CourseVtx d_v[] = {
    { { 175, 107, -448 }, { 0, 74 }, { MACRO_COLOR_FLAG(0xfc, 0xfc, 0xfc, 4), 0x00 } }, /* c */
    { { -1, 2, 3 }, { 4, 5 }, { MACRO_COLOR_FLAG(0x10, 0x20, 0x30, 0), 0x00 } },
};
Gfx d_dl[] = {
    gsDPSetTile(G_IM_FMT_RGBA, G_IM_SIZ_16b, 8, 0x0000, G_TX_RENDERTILE, 0, G_TX_NOMIRROR | G_TX_CLAMP, 5, G_TX_NOLOD,
                G_TX_NOMIRROR | G_TX_WRAP, 5, G_TX_NOLOD),
    gsSPVertex(0x04000000, 8, 0),
    gsSP2Triangles(0, 1, 2, 0, 0, 2, 3, 0),
    gsSPEndDisplayList(),
};
TrackSections d_addr[] = {
    { d_dl, ASPHALT, 1, 0x2000 },
};
f32 scalar = 1.0f;
"#;
        let arrs = arrays(&strip(src));
        assert_eq!(arrs.len(), 3);
        assert_eq!(arrs[0].name, "d_v");
        assert_eq!(arrs[0].ty, "CourseVtx");
        assert_eq!(arrs[0].items.len(), 2);
        let v0 = arrs[0].items[0].list().unwrap();
        let c = gbi_consts();
        assert_eq!(v0[0].list().unwrap()[2].int(&c), Some(-448));
        let (name, args) = v0[2].list().unwrap()[0].call().unwrap();
        assert_eq!(name, "MACRO_COLOR_FLAG");
        assert_eq!(args[3].int(&c), Some(4));
        let dl = &arrs[1];
        assert_eq!(dl.ty, "Gfx");
        let (n, a) = dl.items[0].call().unwrap();
        assert_eq!(n, "gsDPSetTile");
        assert_eq!(a.len(), 12);
        assert_eq!(a[6].int(&c), Some(2));
        assert_eq!(a[9].int(&c), Some(0));
        let (n, a) = dl.items[1].call().unwrap();
        assert_eq!(n, "gsSPVertex");
        assert_eq!(a[0].int(&c), Some(0x04000000));
        assert_eq!(dl.items[3].call().unwrap().0, "gsSPEndDisplayList");
        let sec = arrs[2].items[0].list().unwrap();
        assert_eq!(sec[0].ident(), Some("d_dl"));
        assert_eq!(sec[3].int(&c), Some(0x2000));
    }

    #[test]
    fn evaluates_expressions() {
        let c = gbi_consts();
        assert_eq!(eval("G_CULL_BACK | G_LIGHTING", &c), Some(0x22000));
        assert_eq!(eval("(1 << 4) + 3", &c), Some(19));
        assert_eq!(eval("-0x10", &c), Some(-16));
        assert_eq!(eval("UNKNOWN_NAME", &c), None);
    }
}
