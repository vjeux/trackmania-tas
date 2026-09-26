//! A small XML tokenizer/tree builder, enough for XML-RPC payloads and for the
//! dedicated server's config files. No namespaces, no DTDs, no CDATA beyond
//! the `<![CDATA[...]]>` form, entities limited to the five predefined ones plus
//! numeric references -- which is all the server ever emits.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug)]
pub struct ParseError {
    pub pos: usize,
    pub msg: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "xml parse error at byte {}: {}", self.pos, self.msg)
    }
}

impl std::error::Error for ParseError {}

impl Element {
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find_map(|n| match n {
            Node::Element(e) if e.name == name => Some(e),
            _ => None,
        })
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter_map(move |n| match n {
            Node::Element(e) if e.name == name => Some(e),
            _ => None,
        })
    }

    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    /// Concatenated text of the direct text children.
    pub fn text(&self) -> String {
        let mut s = String::new();
        for n in &self.children {
            if let Node::Text(t) = n {
                s.push_str(t);
            }
        }
        s
    }

    /// Find the first element with this name anywhere below.
    pub fn find(&self, name: &str) -> Option<&Element> {
        for e in self.elements() {
            if e.name == name {
                return Some(e);
            }
            if let Some(found) = e.find(name) {
                return Some(found);
            }
        }
        None
    }
}

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';') else {
            out.push_str(rest);
            return out;
        };
        let ent = &rest[1..end];
        match ent {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ if ent.starts_with("#x") => match u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32) {
                Some(c) => out.push(c),
                None => out.push_str(&rest[..=end]),
            },
            _ if ent.starts_with('#') => match ent[1..].parse::<u32>().ok().and_then(char::from_u32) {
                Some(c) => out.push(c),
                None => out.push_str(&rest[..=end]),
            },
            _ => out.push_str(&rest[..=end]),
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn err<T>(&self, msg: &str) -> Result<T, ParseError> {
        Err(ParseError { pos: self.pos, msg: msg.to_string() })
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn starts_with(&self, p: &str) -> bool {
        self.s[self.pos..].starts_with(p.as_bytes())
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.pos += 1;
        }
    }

    fn skip_until(&mut self, p: &str) -> Result<(), ParseError> {
        match find(&self.s[self.pos..], p.as_bytes()) {
            Some(i) => {
                self.pos += i + p.len();
                Ok(())
            }
            None => self.err(&format!("unterminated construct, expected {p:?}")),
        }
    }

    fn name(&mut self) -> Result<String, ParseError> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b':') {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return self.err("expected a name");
        }
        Ok(String::from_utf8_lossy(&self.s[start..self.pos]).into_owned())
    }

    fn quoted(&mut self) -> Result<String, ParseError> {
        let q = match self.peek() {
            Some(q @ (b'"' | b'\'')) => q,
            _ => return self.err("expected a quoted attribute value"),
        };
        self.pos += 1;
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == q {
                let v = String::from_utf8_lossy(&self.s[start..self.pos]).into_owned();
                self.pos += 1;
                return Ok(unescape(&v));
            }
            self.pos += 1;
        }
        self.err("unterminated attribute value")
    }

    /// Skip the prolog, comments and processing instructions before the root.
    fn prolog(&mut self) -> Result<(), ParseError> {
        // A UTF-8 BOM (the server's own config files carry one).
        if self.s[self.pos..].starts_with(&[0xEF, 0xBB, 0xBF]) {
            self.pos += 3;
        }
        loop {
            self.skip_ws();
            if self.starts_with("<?") {
                self.skip_until("?>")?;
            } else if self.starts_with("<!--") {
                self.skip_until("-->")?;
            } else if self.starts_with("<!") {
                self.skip_until(">")?;
            } else {
                return Ok(());
            }
        }
    }

    fn element(&mut self) -> Result<Element, ParseError> {
        if self.peek() != Some(b'<') {
            return self.err("expected '<'");
        }
        self.pos += 1;
        let name = self.name()?;
        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'/') => {
                    self.pos += 1;
                    if self.peek() != Some(b'>') {
                        return self.err("expected '>' after '/'");
                    }
                    self.pos += 1;
                    return Ok(Element { name, attrs, children: Vec::new() });
                }
                Some(b'>') => {
                    self.pos += 1;
                    break;
                }
                Some(_) => {
                    let k = self.name()?;
                    self.skip_ws();
                    if self.peek() != Some(b'=') {
                        return self.err("expected '=' in attribute");
                    }
                    self.pos += 1;
                    self.skip_ws();
                    let v = self.quoted()?;
                    attrs.push((k, v));
                }
                None => return self.err("unexpected end inside a tag"),
            }
        }
        let mut children = Vec::new();
        loop {
            if self.starts_with("</") {
                self.pos += 2;
                let close = self.name()?;
                if close != name {
                    return self.err(&format!("mismatched close tag: <{name}> ... </{close}>"));
                }
                self.skip_ws();
                if self.peek() != Some(b'>') {
                    return self.err("expected '>' after close tag name");
                }
                self.pos += 1;
                return Ok(Element { name, attrs, children });
            } else if self.starts_with("<!--") {
                self.skip_until("-->")?;
            } else if self.starts_with("<![CDATA[") {
                self.pos += 9;
                let start = self.pos;
                self.skip_until("]]>")?;
                let text = String::from_utf8_lossy(&self.s[start..self.pos - 3]).into_owned();
                children.push(Node::Text(text));
            } else if self.starts_with("<?") {
                self.skip_until("?>")?;
            } else if self.peek() == Some(b'<') {
                children.push(Node::Element(self.element()?));
            } else if self.peek().is_none() {
                return self.err(&format!("unexpected end of input inside <{name}>"));
            } else {
                let start = self.pos;
                while let Some(c) = self.peek() {
                    if c == b'<' {
                        break;
                    }
                    self.pos += 1;
                }
                let raw = String::from_utf8_lossy(&self.s[start..self.pos]).into_owned();
                children.push(Node::Text(unescape(&raw)));
            }
        }
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Parse one document and return its root element.
pub fn parse(input: &[u8]) -> Result<Element, ParseError> {
    let mut p = Parser { s: input, pos: 0 };
    p.prolog()?;
    let root = p.element()?;
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_with_attrs_and_entities() {
        let doc = br#"<?xml version="1.0"?><!-- c --><a x="1" y='two'><b>hi &amp; &lt;bye&gt;</b><c/><d><![CDATA[<raw>]]></d></a>"#;
        let root = parse(doc).unwrap();
        assert_eq!(root.name, "a");
        assert_eq!(root.attrs, vec![("x".to_string(), "1".to_string()), ("y".to_string(), "two".to_string())]);
        assert_eq!(root.child("b").unwrap().text(), "hi & <bye>");
        assert!(root.child("c").unwrap().children.is_empty());
        assert_eq!(root.child("d").unwrap().text(), "<raw>");
    }

    #[test]
    fn bom_and_comments_inside() {
        let doc = b"\xEF\xBB\xBF<r>\n\t<k>v</k> <!-- note --> <k>w</k></r>";
        let root = parse(doc).unwrap();
        let ks: Vec<String> = root.children_named("k").map(|e| e.text()).collect();
        assert_eq!(ks, vec!["v", "w"]);
    }

    #[test]
    fn escape_roundtrip() {
        let s = "a<b>&\"c\"'d'";
        assert_eq!(unescape(&escape(s)), s);
    }

    #[test]
    fn mismatched_close_is_an_error() {
        assert!(parse(b"<a><b></a>").is_err());
    }
}
