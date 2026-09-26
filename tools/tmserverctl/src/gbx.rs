//! GbxRemote 2 -- the dedicated server's XML-RPC-over-TCP protocol.
//!
//! Wire format (little-endian): the server greets with `u32 len` + `"GbxRemote 2"`.
//! Then every message is `u32 len`, `u32 handle`, `len` bytes of XML. Client
//! requests carry handles with the top bit set (0x8000_0000 upwards); the
//! server answers with the same handle, and pushes callbacks (methodCall
//! documents) with handles below 0x8000_0000.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::xml::{self, Element};

/// One XML-RPC value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
    Double(f64),
    Base64(String),
    Array(Vec<Value>),
    Struct(BTreeMap<String, Value>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Base64(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Double(d) => Some(*d as i64),
            Value::Bool(b) => Some(*b as i64),
            Value::Str(s) => s.trim().parse().ok(),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Int(i) => Some(*i != 0),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Struct(m) => m.get(key),
            _ => None,
        }
    }
    /// `struct.key` as text, or "" -- for printing server structs.
    pub fn field_str(&self, key: &str) -> String {
        self.get(key).map(|v| v.to_plain()).unwrap_or_default()
    }
    /// Human form: strings bare, scalars as text, containers as JSON-ish.
    pub fn to_plain(&self) -> String {
        match self {
            Value::Str(s) | Value::Base64(s) => s.clone(),
            other => other.to_string(),
        }
    }

    fn write_xml(&self, out: &mut String) {
        out.push_str("<value>");
        match self {
            Value::Int(i) => {
                out.push_str("<int>");
                out.push_str(&i.to_string());
                out.push_str("</int>");
            }
            Value::Bool(b) => {
                out.push_str("<boolean>");
                out.push(if *b { '1' } else { '0' });
                out.push_str("</boolean>");
            }
            Value::Str(s) => {
                out.push_str("<string>");
                out.push_str(&xml::escape(s));
                out.push_str("</string>");
            }
            Value::Double(d) => {
                out.push_str("<double>");
                out.push_str(&format!("{d}"));
                out.push_str("</double>");
            }
            Value::Base64(s) => {
                out.push_str("<base64>");
                out.push_str(s);
                out.push_str("</base64>");
            }
            Value::Array(items) => {
                out.push_str("<array><data>");
                for v in items {
                    v.write_xml(out);
                }
                out.push_str("</data></array>");
            }
            Value::Struct(map) => {
                out.push_str("<struct>");
                for (k, v) in map {
                    out.push_str("<member><name>");
                    out.push_str(&xml::escape(k));
                    out.push_str("</name>");
                    v.write_xml(out);
                    out.push_str("</member>");
                }
                out.push_str("</struct>");
            }
        }
        out.push_str("</value>");
    }

    fn from_xml(value: &Element) -> Result<Value> {
        // <value>text</value> without a type tag is a string.
        let typed = value.elements().next();
        let Some(t) = typed else {
            return Ok(Value::Str(value.text()));
        };
        let text = t.text();
        Ok(match t.name.as_str() {
            "i4" | "int" | "i8" => Value::Int(text.trim().parse().map_err(|_| Error::Protocol(format!("bad int {text:?}")))?),
            "boolean" => Value::Bool(matches!(text.trim(), "1" | "true" | "True")),
            "string" => Value::Str(text),
            "double" => Value::Double(text.trim().parse().map_err(|_| Error::Protocol(format!("bad double {text:?}")))?),
            "base64" => Value::Base64(text.trim().to_string()),
            "dateTime.iso8601" => Value::Str(text.trim().to_string()),
            "array" => {
                let data = t.child("data").ok_or_else(|| Error::Protocol("array without <data>".into()))?;
                let mut items = Vec::new();
                for v in data.children_named("value") {
                    items.push(Value::from_xml(v)?);
                }
                Value::Array(items)
            }
            "struct" => {
                let mut map = BTreeMap::new();
                for m in t.children_named("member") {
                    let name = m.child("name").map(|n| n.text()).unwrap_or_default();
                    let v = m.child("value").ok_or_else(|| Error::Protocol("member without <value>".into()))?;
                    map.insert(name, Value::from_xml(v)?);
                }
                Value::Struct(map)
            }
            other => return Err(Error::Protocol(format!("unknown value type <{other}>"))),
        })
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(i) => write!(f, "{i}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::Double(d) => write!(f, "{d}"),
            Value::Base64(s) => write!(f, "base64({} bytes)", s.len()),
            Value::Array(a) => {
                write!(f, "[")?;
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::Struct(m) => {
                write!(f, "{{")?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {v}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}
impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Int(i)
    }
}
impl From<i32> for Value {
    fn from(i: i32) -> Self {
        Value::Int(i as i64)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}
impl From<Vec<String>> for Value {
    fn from(v: Vec<String>) -> Self {
        Value::Array(v.into_iter().map(Value::Str).collect())
    }
}

/// Guess a value from a command-line token: `123` int, `true`/`false` bool,
/// `1.5` double, anything else a string. A leading `s:` forces a string
/// (`s:123`), `[a,b,c]` is an array of strings.
pub fn value_from_cli(tok: &str) -> Value {
    if let Some(rest) = tok.strip_prefix("s:") {
        return Value::Str(rest.to_string());
    }
    if tok.starts_with('[') && tok.ends_with(']') {
        let inner = &tok[1..tok.len() - 1];
        if inner.trim().is_empty() {
            return Value::Array(Vec::new());
        }
        return Value::Array(inner.split(',').map(|s| Value::Str(s.trim().to_string())).collect());
    }
    match tok {
        "true" | "True" => return Value::Bool(true),
        "false" | "False" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(i) = tok.parse::<i64>() {
        return Value::Int(i);
    }
    if tok.contains('.') {
        if let Ok(d) = tok.parse::<f64>() {
            return Value::Double(d);
        }
    }
    Value::Str(tok.to_string())
}

/// A callback pushed by the server.
#[derive(Debug, Clone, PartialEq)]
pub struct Callback {
    pub method: String,
    pub params: Vec<Value>,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Protocol(String),
    /// The server answered with `<fault>`: (faultCode, faultString).
    Fault(i64, String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Protocol(m) => write!(f, "protocol: {m}"),
            Error::Fault(code, msg) => write!(f, "server fault {code}: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<xml::ParseError> for Error {
    fn from(e: xml::ParseError) -> Self {
        Error::Protocol(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

const HANDLE_BASE: u32 = 0x8000_0000;
const MAX_MESSAGE: u32 = 64 * 1024 * 1024;

pub struct Client {
    stream: TcpStream,
    next_handle: u32,
    /// Callbacks that arrived while waiting for a response.
    pending: Vec<Callback>,
}

fn method_call(method: &str, params: &[Value]) -> String {
    let mut out = String::with_capacity(256);
    out.push_str("<?xml version=\"1.0\"?><methodCall><methodName>");
    out.push_str(&xml::escape(method));
    out.push_str("</methodName><params>");
    for p in params {
        out.push_str("<param>");
        p.write_xml(&mut out);
        out.push_str("</param>");
    }
    out.push_str("</params></methodCall>");
    out
}

impl Client {
    /// Connect and read the greeting. `timeout` bounds the connect and every read.
    pub fn connect<A: ToSocketAddrs>(addr: A, timeout: Duration) -> Result<Client> {
        let addrs: Vec<_> = addr.to_socket_addrs()?.collect();
        let mut last = io::Error::new(io::ErrorKind::NotFound, "no address to connect to");
        for a in addrs {
            match TcpStream::connect_timeout(&a, timeout) {
                Ok(stream) => {
                    stream.set_nodelay(true)?;
                    stream.set_read_timeout(Some(timeout))?;
                    stream.set_write_timeout(Some(timeout))?;
                    let mut c = Client { stream, next_handle: HANDLE_BASE, pending: Vec::new() };
                    c.greeting()?;
                    return Ok(c);
                }
                Err(e) => last = e,
            }
        }
        Err(Error::Io(last))
    }

    fn greeting(&mut self) -> Result<()> {
        let mut len = [0u8; 4];
        self.stream.read_exact(&mut len)?;
        let len = u32::from_le_bytes(len);
        if len == 0 || len > 64 {
            return Err(Error::Protocol(format!("greeting length {len} is not a GbxRemote greeting")));
        }
        let mut buf = vec![0u8; len as usize];
        self.stream.read_exact(&mut buf)?;
        let s = String::from_utf8_lossy(&buf);
        if !s.starts_with("GBXRemote 2") {
            return Err(Error::Protocol(format!("unexpected greeting {s:?}")));
        }
        Ok(())
    }

    pub fn set_read_timeout(&mut self, t: Option<Duration>) -> Result<()> {
        self.stream.set_read_timeout(t)?;
        Ok(())
    }

    fn read_frame(&mut self) -> Result<(u32, Vec<u8>)> {
        let mut head = [0u8; 8];
        self.stream.read_exact(&mut head)?;
        let len = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
        let handle = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
        if len > MAX_MESSAGE {
            return Err(Error::Protocol(format!("message of {len} bytes refused")));
        }
        let mut body = vec![0u8; len as usize];
        self.stream.read_exact(&mut body)?;
        Ok((handle, body))
    }

    fn parse_callback(body: &[u8]) -> Result<Callback> {
        let root = xml::parse(body)?;
        if root.name != "methodCall" {
            return Err(Error::Protocol(format!("callback root is <{}>", root.name)));
        }
        let method = root.child("methodName").map(|m| m.text().trim().to_string()).unwrap_or_default();
        let mut params = Vec::new();
        if let Some(ps) = root.child("params") {
            for p in ps.children_named("param") {
                if let Some(v) = p.child("value") {
                    params.push(Value::from_xml(v)?);
                }
            }
        }
        Ok(Callback { method, params })
    }

    fn parse_response(body: &[u8]) -> Result<Value> {
        let root = xml::parse(body)?;
        if root.name != "methodResponse" {
            return Err(Error::Protocol(format!("response root is <{}>", root.name)));
        }
        if let Some(fault) = root.child("fault") {
            let v = fault.child("value").map(Value::from_xml).transpose()?;
            let code = v.as_ref().and_then(|v| v.get("faultCode")).and_then(|c| c.as_i64()).unwrap_or(-1);
            let msg = v.as_ref().map(|v| v.field_str("faultString")).unwrap_or_default();
            return Err(Error::Fault(code, msg));
        }
        let value = root
            .child("params")
            .and_then(|ps| ps.child("param"))
            .and_then(|p| p.child("value"))
            .map(Value::from_xml)
            .transpose()?;
        Ok(value.unwrap_or(Value::Bool(true)))
    }

    /// Call a method and wait for its answer; callbacks that arrive meanwhile are
    /// queued for `take_callbacks`.
    pub fn call(&mut self, method: &str, params: &[Value]) -> Result<Value> {
        let handle = self.next_handle;
        self.next_handle = if self.next_handle == u32::MAX { HANDLE_BASE } else { self.next_handle + 1 };
        let xml = method_call(method, params);
        let body = xml.as_bytes();
        let mut frame = Vec::with_capacity(body.len() + 8);
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(&handle.to_le_bytes());
        frame.extend_from_slice(body);
        self.stream.write_all(&frame)?;
        let started = Instant::now();
        loop {
            let (h, body) = match self.read_frame() {
                Ok(f) => f,
                Err(Error::Io(e)) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                    if started.elapsed() > Duration::from_secs(30) {
                        return Err(Error::Io(e));
                    }
                    continue;
                }
                Err(e) => return Err(e),
            };
            if h == handle {
                return Self::parse_response(&body);
            }
            if h < HANDLE_BASE {
                self.pending.push(Self::parse_callback(&body)?);
            }
            // An answer to another handle can only mean a lost request; ignore it.
        }
    }

    /// Wait for the next callback (up to the read timeout). Returns `Ok(None)` on
    /// a timeout, so a caller can poll a stop flag between waits.
    pub fn next_callback(&mut self) -> Result<Option<Callback>> {
        if !self.pending.is_empty() {
            return Ok(Some(self.pending.remove(0)));
        }
        loop {
            match self.read_frame() {
                Ok((h, body)) => {
                    if h < HANDLE_BASE {
                        return Ok(Some(Self::parse_callback(&body)?));
                    }
                    // A stray response; nothing is waiting for it.
                }
                Err(Error::Io(e)) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => return Ok(None),
                Err(e) => return Err(e),
            }
        }
    }

    pub fn take_callbacks(&mut self) -> Vec<Callback> {
        std::mem::take(&mut self.pending)
    }

    /// `Authenticate` as one of the three levels of dedicated_cfg.txt.
    pub fn authenticate(&mut self, level: &str, password: &str) -> Result<()> {
        let ok = self.call("Authenticate", &[level.into(), password.into()])?;
        if ok.as_bool() == Some(true) {
            Ok(())
        } else {
            Err(Error::Protocol(format!("Authenticate({level}) refused")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_a_call() {
        let s = method_call("TriggerModeScriptEventArray", &["LowG.Cmd".into(), Value::from(vec!["me".to_string(), "gravity".to_string(), "0.5".to_string()])]);
        assert!(s.contains("<methodName>TriggerModeScriptEventArray</methodName>"));
        assert!(s.contains("<array><data><value><string>me</string></value><value><string>gravity</string></value><value><string>0.5</string></value></data></array>"));
    }

    #[test]
    fn parses_a_struct_response() {
        let body = br#"<?xml version="1.0"?><methodResponse><params><param><value><struct><member><name>Code</name><value><i4>4</i4></value></member><member><name>Name</name><value><string>Running - Play</string></value></member></struct></value></param></params></methodResponse>"#;
        let v = Client::parse_response(body).unwrap();
        assert_eq!(v.get("Code").unwrap().as_i64(), Some(4));
        assert_eq!(v.field_str("Name"), "Running - Play");
    }

    #[test]
    fn parses_a_fault() {
        let body = br#"<?xml version="1.0"?><methodResponse><fault><value><struct><member><name>faultCode</name><value><int>-1000</int></value></member><member><name>faultString</name><value><string>Not allowed.</string></value></member></struct></value></fault></methodResponse>"#;
        match Client::parse_response(body) {
            Err(Error::Fault(-1000, m)) => assert_eq!(m, "Not allowed."),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_a_chat_callback() {
        let body = br#"<?xml version="1.0"?><methodCall><methodName>ManiaPlanet.PlayerChat</methodName><params><param><value><i4>255</i4></value></param><param><value><string>yannex</string></value></param><param><value><string>/gravity 0.3</string></value></param><param><value><boolean>1</boolean></value></param></params></methodCall>"#;
        let cb = Client::parse_callback(body).unwrap();
        assert_eq!(cb.method, "ManiaPlanet.PlayerChat");
        assert_eq!(cb.params[1].as_str(), Some("yannex"));
        assert_eq!(cb.params[2].as_str(), Some("/gravity 0.3"));
        assert_eq!(cb.params[3].as_bool(), Some(true));
    }

    #[test]
    fn cli_values() {
        assert_eq!(value_from_cli("12"), Value::Int(12));
        assert_eq!(value_from_cli("true"), Value::Bool(true));
        assert_eq!(value_from_cli("0.5"), Value::Double(0.5));
        assert_eq!(value_from_cli("s:12"), Value::Str("12".into()));
        assert_eq!(value_from_cli("[a, b]"), Value::Array(vec!["a".into(), "b".into()]));
        assert_eq!(value_from_cli("hello"), Value::Str("hello".into()));
    }
}
