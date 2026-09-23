//! The block browser as the pack defines it. Every `CGameCtnBlockInfo` (and
//! every `CGameItemModel`) is a `CGameCtnCollector`, and its header chunk
//! `0x2E001003` carries what the editor's browser shows: the PAGE the block is
//! filed under (`RoadTech/Main/`, forward slashes, trailing slash), its
//! display name, the catalog position and the production state; the sibling
//! header chunk `0x2E001004` is the 64×64 WEBP icon the browser draws.
//!
//! `mapgeom blockinfo-catalog [SUBSTRING] [--out TSV] [--collection Stadium]`
//! lists them all — the yardstick for an item set whose folders must mirror
//! the block browser (`item_set.rs`).

use crate::store::DataStore;
use tmmaps::gbx::Reader;

/// Header chunk `0x2E001003` (`CGameCtnCollector`, version 8).
#[derive(Clone, Debug, PartialEq)]
pub struct CollectorDesc {
    pub ident: String,
    pub collection: u32,
    pub author: String,
    pub version: u32,
    /// The browser page, e.g. `RoadTech/Main/`; empty on the blocks the
    /// browser never shows (fillers, clips, generated pieces).
    pub page: String,
    pub parent: Option<String>,
    pub flags: i32,
    pub catalog_position: i16,
    pub name: String,
    pub prod_state: u8,
}

/// Header chunk `0x2E001004`: `u16 width, u16 height` with the high bit set
/// on both when a WEBP follows (`u16 version, u32 len, bytes`), else raw RGBA.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    pub width: u16,
    pub height: u16,
    pub webp: bool,
    /// The chunk payload verbatim (what an item header takes as is).
    pub payload: Vec<u8>,
}

fn lookback_id(r: &mut Reader, table: &mut Vec<String>) -> Result<Option<String>, String> {
    let w = r.u32();
    if w == 0xFFFF_FFFF {
        return Ok(None);
    }
    if w & 0xC000_0000 == 0 {
        // a plain number (a collection id): rendered as its decimal
        return Ok(Some(w.to_string()));
    }
    let idx = w & 0x3FFF_FFFF;
    if idx == 0 {
        let s = r.string();
        table.push(s.clone());
        return Ok(Some(s));
    }
    table.get(idx as usize - 1).cloned().map(Some).ok_or_else(|| format!("lookback index {idx} past the table ({} strings)", table.len()))
}

impl CollectorDesc {
    /// Parse the payload of header chunk `0x2E001003`.
    pub fn parse(payload: &[u8]) -> Result<CollectorDesc, String> {
        let mut r = Reader::new(payload);
        let mut table: Vec<String> = Vec::new();
        let lbver = r.u32();
        if lbver != 3 {
            return Err(format!("collector desc: lookback version {lbver}, expected 3"));
        }
        let ident = lookback_id(&mut r, &mut table)?.unwrap_or_default();
        let collection = r.u32();
        let author = lookback_id(&mut r, &mut table)?.unwrap_or_default();
        let version = r.u32();
        if version < 7 {
            return Err(format!("collector desc: version {version} not modelled (needs >= 7)"));
        }
        let page = r.string();
        let parent = lookback_id(&mut r, &mut table)?;
        let flags = r.i32();
        let catalog_position = r.u16() as i16;
        let name = r.string();
        let prod_state = r.u8();
        Ok(CollectorDesc { ident, collection, author, version, page, parent, flags, catalog_position, name, prod_state })
    }

    /// The desc and the icon out of a `.Gbx` file's header.
    pub fn from_file(bytes: &[u8]) -> Result<(CollectorDesc, Option<Icon>), String> {
        let g = tmmaps::gbx::Gbx::parse(bytes);
        let chunks = crate::static_item::file::parse_header_chunks(&g.user_data)?;
        let desc = chunks.iter().find(|c| c.id == 0x2E001003).ok_or("no collector description header chunk (0x2E001003)")?;
        let desc = CollectorDesc::parse(&desc.payload)?;
        let icon = chunks.iter().find(|c| c.id == 0x2E001004).map(|c| Icon::parse(&c.payload));
        Ok((desc, icon))
    }

    /// The page as folder names (`RoadTech/Main/` -> `["RoadTech", "Main"]`).
    pub fn page_folders(&self) -> Vec<String> {
        self.page.split(['/', '\\']).filter(|s| !s.is_empty()).map(String::from).collect()
    }
}

impl Icon {
    pub fn parse(payload: &[u8]) -> Icon {
        let mut r = Reader::new(payload);
        let width = r.u16();
        let height = r.u16();
        let webp = width & 0x8000 != 0 && height & 0x8000 != 0;
        Icon { width: width & 0x7FFF, height: height & 0x7FFF, webp, payload: payload.to_vec() }
    }
}

/// One block browser entry: the pack path of the block info and its desc.
#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub path: String,
    pub class_id: u32,
    pub desc: CollectorDesc,
    pub icon: Option<Icon>,
}

/// Every block info of `collection` (the pack folder name, `Stadium`) whose
/// desc parses, in pack order. `filter` narrows by path substring.
pub fn block_infos(store: &mut DataStore, collection: &str, filter: Option<&str>) -> Vec<Result<CatalogEntry, (String, String)>> {
    let prefix = format!("{collection}\\GameCtnBlockInfo\\");
    let paths: Vec<(String, u32)> = store
        .entries()
        .filter(|e| {
            let p = e.path();
            p.starts_with(&prefix) && p.ends_with(".Gbx") && crate::blockinfo::is_block_info_class(e.class_id) && filter.map(|f| p.contains(f)).unwrap_or(true)
        })
        .map(|e| (e.path(), e.class_id))
        .collect();
    let mut out = Vec::with_capacity(paths.len());
    for (path, class_id) in paths {
        match store.read(&path) {
            Ok(bytes) => match CollectorDesc::from_file(&bytes) {
                Ok((desc, icon)) => out.push(Ok(CatalogEntry { path, class_id, desc, icon })),
                Err(e) => out.push(Err((path, e))),
            },
            Err(e) => out.push(Err((path, e))),
        }
    }
    out
}

/// `mapgeom blockinfo-catalog`: the TSV of every block info's browser facts.
pub fn cmd(store: &mut DataStore, rest: &[String]) {
    let flag = |name: &str| rest.iter().position(|a| a == name).and_then(|i| rest.get(i + 1).cloned());
    let collection = flag("--collection").unwrap_or_else(|| "Stadium".to_string());
    let out = flag("--out");
    let filter: Option<String> = rest.iter().skip(1).filter(|a| !a.starts_with("--")).find(|a| Some(a.as_str()) != flag("--collection").as_deref() && Some(a.as_str()) != out.as_deref()).cloned();
    let entries = block_infos(store, &collection, filter.as_deref());
    let mut tsv = String::from("path\tclass\tident\tname\tpage\tflags\tcatalog_position\tprod_state\ticon\n");
    let (mut ok, mut failed, mut paged) = (0usize, 0usize, 0usize);
    for e in &entries {
        match e {
            Ok(e) => {
                ok += 1;
                if !e.desc.page.is_empty() {
                    paged += 1;
                }
                let icon = match &e.icon {
                    Some(i) => format!("{}x{}{}", i.width, i.height, if i.webp { " webp" } else { " rgba" }),
                    None => "-".to_string(),
                };
                tsv.push_str(&format!("{}\t{:08X}\t{}\t{}\t{}\t0x{:X}\t{}\t{}\t{}\n", e.path, e.class_id, e.desc.ident, e.desc.name, e.desc.page, e.desc.flags, e.desc.catalog_position, e.desc.prod_state, icon));
            }
            Err((p, err)) => {
                failed += 1;
                eprintln!("{p}: {err}");
            }
        }
    }
    match out {
        Some(p) => {
            std::fs::write(&p, tsv).expect("write --out");
            eprintln!("{ok} block infos ({paged} on a browser page), {failed} unreadable -> {p}");
        }
        None => {
            print!("{tsv}");
            eprintln!("{ok} block infos ({paged} on a browser page), {failed} unreadable");
        }
    }
}

// ---------------------------------------------------------------------------
// The browser trees: `Stadium\GameCtnBlockInfo\<hash>` JSON files
// (`CGameBlockInfoTreeRoot`, `CGameItemModelTreeRoot`) — folders and leaves
// exactly as the editor's block / item browser shows them (the collector
// header's page name is NOT what the browser uses: 785 unrelated blocks share
// `PlatformTech/Main/` there).
// ---------------------------------------------------------------------------

/// A minimal JSON value — Nadeo's tree files are JSON with trailing commas
/// (`{ "Name" : "RoadTechStraight", }`), which a strict parser rejects.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(kv) => kv.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

struct JsonCur<'a> {
    s: &'a [u8],
    o: usize,
}

impl<'a> JsonCur<'a> {
    fn ws(&mut self) {
        while self.o < self.s.len() && (self.s[self.o] as char).is_ascii_whitespace() {
            self.o += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.o).copied()
    }
    fn expect(&mut self, c: u8) -> Result<(), String> {
        if self.peek() == Some(c) {
            self.o += 1;
            Ok(())
        } else {
            Err(format!("json: expected '{}' at {}", c as char, self.o))
        }
    }
    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = Vec::new();
        loop {
            let c = *self.s.get(self.o).ok_or("json: unterminated string")?;
            self.o += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = *self.s.get(self.o).ok_or("json: bad escape")?;
                    self.o += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'u' => {
                            let hex = std::str::from_utf8(self.s.get(self.o..self.o + 4).ok_or("json: bad \\u")?).map_err(|e| e.to_string())?;
                            self.o += 4;
                            let cp = u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(char::from_u32(cp).unwrap_or('?').encode_utf8(&mut buf).as_bytes());
                        }
                        other => out.push(other),
                    }
                }
                other => out.push(other),
            }
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }
    fn value(&mut self) -> Result<Json, String> {
        match self.peek().ok_or("json: unexpected end")? {
            b'{' => {
                self.o += 1;
                let mut kv = Vec::new();
                loop {
                    match self.peek().ok_or("json: unterminated object")? {
                        b'}' => {
                            self.o += 1;
                            break;
                        }
                        b',' => {
                            self.o += 1;
                        }
                        _ => {
                            let k = self.string()?;
                            self.expect(b':')?;
                            let v = self.value()?;
                            kv.push((k, v));
                        }
                    }
                }
                Ok(Json::Obj(kv))
            }
            b'[' => {
                self.o += 1;
                let mut a = Vec::new();
                loop {
                    match self.peek().ok_or("json: unterminated array")? {
                        b']' => {
                            self.o += 1;
                            break;
                        }
                        b',' => {
                            self.o += 1;
                        }
                        _ => a.push(self.value()?),
                    }
                }
                Ok(Json::Arr(a))
            }
            b'"' => Ok(Json::Str(self.string()?)),
            b't' if self.s[self.o..].starts_with(b"true") => {
                self.o += 4;
                Ok(Json::Bool(true))
            }
            b'f' if self.s[self.o..].starts_with(b"false") => {
                self.o += 5;
                Ok(Json::Bool(false))
            }
            b'n' if self.s[self.o..].starts_with(b"null") => {
                self.o += 4;
                Ok(Json::Null)
            }
            _ => {
                let start = self.o;
                while self.o < self.s.len() && matches!(self.s[self.o], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E') {
                    self.o += 1;
                }
                let t = std::str::from_utf8(&self.s[start..self.o]).map_err(|e| e.to_string())?;
                t.parse::<f64>().map(Json::Num).map_err(|_| format!("json: bad token at {start}: {t:?}"))
            }
        }
    }
}

pub fn parse_json(text: &[u8]) -> Result<Json, String> {
    // a UTF-8 BOM, if any
    let s = text.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(text);
    let mut c = JsonCur { s, o: 0 };
    let v = c.value()?;
    c.ws();
    if c.o != s.len() {
        return Err(format!("json: {} trailing bytes", s.len() - c.o));
    }
    Ok(v)
}

/// One leaf of a browser tree: the folder path from the root and the leaf's
/// name (a block info name, or an item / macroblock name).
#[derive(Clone, Debug, PartialEq)]
pub struct TreeLeaf {
    pub folders: Vec<String>,
    pub name: String,
}

fn flatten(node: &Json, folders: &mut Vec<String>, out: &mut Vec<TreeLeaf>) {
    let name = node.get("Name").and_then(|n| n.as_str()).unwrap_or("").to_string();
    if node.get("IsFolder").and_then(|b| b.as_bool()).unwrap_or(false) {
        folders.push(name);
        for c in node.get("Childs").and_then(|c| c.as_arr()).unwrap_or(&[]) {
            flatten(c, folders, out);
        }
        folders.pop();
    } else if !name.is_empty() {
        out.push(TreeLeaf { folders: folders.clone(), name });
    }
}

/// The leaves of a tree file's `RootChilds`, depth first, in file order.
pub fn tree_leaves(text: &[u8]) -> Result<(String, Vec<TreeLeaf>), String> {
    let j = parse_json(text)?;
    let class = j.get("ClassId").and_then(|c| c.as_str()).unwrap_or("").to_string();
    let mut out = Vec::new();
    let mut folders = Vec::new();
    for c in j.get("RootChilds").and_then(|c| c.as_arr()).unwrap_or(&[]) {
        flatten(c, &mut folders, &mut out);
    }
    Ok((class, out))
}

/// The tree files of a collection: every `<Collection>\GameCtnBlockInfo\<hash>`
/// entry whose bytes start with `{` — (class id, leaves) each.
pub fn browser_trees(store: &mut DataStore, collection: &str) -> Vec<(String, String, Vec<TreeLeaf>)> {
    let prefix = format!("{collection}\\GameCtnBlockInfo\\");
    let paths: Vec<String> = store.entries().map(|e| e.path()).filter(|p| p.starts_with(&prefix) && !p[prefix.len()..].contains('\\') && !p.ends_with(".Gbx")).collect();
    let mut out = Vec::new();
    for p in paths {
        let Ok(bytes) = store.read(&p) else { continue };
        let trimmed = bytes.iter().position(|b| !(*b as char).is_ascii_whitespace()).map(|i| &bytes[i..]).unwrap_or(&bytes[..]);
        if !trimmed.starts_with(b"{") {
            continue;
        }
        match tree_leaves(&bytes) {
            Ok((class, leaves)) => out.push((p, class, leaves)),
            Err(e) => eprintln!("{p}: {e}"),
        }
    }
    out
}

/// The block browser of a collection: its `CGameBlockInfoTreeRoot` leaves,
/// without the `DEV` root folder the shipped editor hides.
pub fn block_browser(store: &mut DataStore, collection: &str) -> Result<Vec<TreeLeaf>, String> {
    let trees = browser_trees(store, collection);
    let (_, _, leaves) = trees.into_iter().find(|(_, class, _)| class == "CGameBlockInfoTreeRoot").ok_or_else(|| format!("{collection}: no CGameBlockInfoTreeRoot tree file in the packs"))?;
    Ok(leaves.into_iter().filter(|l| l.folders.first().map(|f| f != "DEV").unwrap_or(true)).collect())
}

/// `mapgeom browser-tree [--collection Stadium] [--class CGameBlockInfoTreeRoot]`:
/// every leaf as `folder/path<TAB>name`.
pub fn tree_cmd(store: &mut DataStore, rest: &[String]) {
    let flag = |name: &str| rest.iter().position(|a| a == name).and_then(|i| rest.get(i + 1).cloned());
    let collection = flag("--collection").unwrap_or_else(|| "Stadium".to_string());
    let class = flag("--class");
    for (path, cls, leaves) in browser_trees(store, &collection) {
        if class.as_deref().map(|c| c != cls).unwrap_or(false) {
            continue;
        }
        eprintln!("{path}: {cls}, {} leaves", leaves.len());
        for l in &leaves {
            println!("{cls}\t{}\t{}", l.folders.join("/"), l.name);
        }
    }
}
