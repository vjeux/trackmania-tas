//! The GBX reference table as a value: parse it, rewrite it, print it.
//!
//! A `.Gbx` file names the files it needs in a table after the header:
//!
//! ```text
//! u32 numExternalNodes
//! if numExternalNodes > 0:
//!   u32 ancestorLevel            how many folders UP from the file's own folder the tree hangs
//!   folder tree                  u32 count, then per folder: string name + its own sub-tree
//!   per external node:
//!     u32 flags                  bit 2 set = a resource index instead of a file name
//!     string fileName | u32 resourceIndex
//!     u32 nodeIndex              the node slot in the body this file fills
//!     u32 useFile                (container version >= 5)
//!     u32 folderIndex            (file names only) 0 = the ancestor folder itself,
//!                                the tree below it numbered depth-first from 1
//! ```
//!
//! The game resolves an entry by walking `ancestorLevel` folders up from the
//! file, then down the named folders. A pack item (`Stadium\Items\X.Item.Gbx`,
//! ancestor 1, `Media\Prefab\…`) reaches `Stadium\Media\Prefab\…` that way.
//! `mapgeom reref` rewrites the table of a copy of such a file so the same
//! entries are spelled from another root, at another ancestor level, with
//! other flags — the instrument of the 2026-10-17 question "can an item that
//! lives in a map's archive, or in the user's Items folder, reach a PACK
//! prefab through its reference table?" (AR's ask; REFS.md next to the probe
//! plugin has the answer).

use std::collections::BTreeMap;
use tmmaps::gbx::{Gbx, Reader};

#[derive(Debug, Clone, PartialEq)]
pub struct RefEntry {
    pub flags: u32,
    /// `Ok(folder path + file name)` for a named file, `Err(resource index)` otherwise.
    pub target: Result<String, u32>,
    pub node_index: u32,
    pub use_file: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RefTable {
    pub ancestor_level: u32,
    pub entries: Vec<RefEntry>,
}

#[derive(Default)]
struct Folder {
    name: String,
    subs: Vec<Folder>,
}

// `prefix` is None at the root (a folder named "" — a leading backslash — keys as
// "" and its children as "\\child")
fn read_folders(r: &mut Reader, out: &mut Vec<String>, prefix: Option<&str>) {
    let n = r.u32();
    for _ in 0..n {
        let name = r.string();
        let path = match prefix {
            None => name.clone(),
            Some(p) => format!("{p}\\{name}"),
        };
        out.push(path.clone());
        read_folders(r, out, Some(&path));
    }
}

/// Parse the table bytes of a container (`Gbx::ref_table`), `has_use_file` =
/// container version >= 5.
pub fn parse(bytes: &[u8], has_use_file: bool) -> RefTable {
    if bytes.len() < 4 {
        return RefTable::default();
    }
    let mut r = Reader::new(bytes);
    let n = r.u32();
    if n == 0 {
        return RefTable::default();
    }
    let ancestor_level = r.u32();
    // folder index 0 is the ancestor folder; the tree is numbered depth-first from 1
    let mut folders: Vec<String> = vec![String::new()];
    read_folders(&mut r, &mut folders, None);
    let mut entries = Vec::new();
    for _ in 0..n {
        let flags = r.u32();
        let name = if flags & 4 == 0 { Ok(r.string()) } else { Err(r.u32()) };
        let node_index = r.u32();
        let use_file = if has_use_file { r.u32() } else { 0 };
        let target = match name {
            Ok(file) => {
                let fi = r.u32() as usize;
                let dir = folders.get(fi).cloned().unwrap_or_else(|| format!("<folder#{fi}>"));
                Ok(if dir.is_empty() { file } else { format!("{dir}\\{file}") })
            }
            Err(res) => Err(res),
        };
        entries.push(RefEntry { flags, target, node_index, use_file });
    }
    RefTable { ancestor_level, entries }
}

/// Serialise a table; the folder tree is rebuilt from the entries' paths and
/// numbered the game's way.
pub fn serialize(t: &RefTable, has_use_file: bool) -> Vec<u8> {
    fn insert(f: &mut Folder, parts: &[&str]) {
        if parts.is_empty() {
            return;
        }
        let pos = match f.subs.iter().position(|s| s.name == parts[0]) {
            Some(p) => p,
            None => {
                f.subs.push(Folder { name: parts[0].to_string(), subs: Vec::new() });
                f.subs.len() - 1
            }
        };
        insert(&mut f.subs[pos], &parts[1..]);
    }
    // `prefix` is None at the root: a folder named "" (a path with a leading
    // backslash) must still key as "" and its children as "\\child"
    fn number(f: &Folder, prefix: Option<&str>, next: &mut u32, out: &mut BTreeMap<String, u32>) {
        for sub in &f.subs {
            let path = match prefix {
                None => sub.name.clone(),
                Some(p) => format!("{p}\\{}", sub.name),
            };
            out.insert(path.clone(), *next);
            *next += 1;
            number(sub, Some(&path), next, out);
        }
    }
    fn write(f: &Folder, v: &mut Vec<u8>) {
        v.extend_from_slice(&(f.subs.len() as u32).to_le_bytes());
        for sub in &f.subs {
            put_string(v, &sub.name);
            write(sub, v);
        }
    }
    let mut v = Vec::new();
    v.extend_from_slice(&(t.entries.len() as u32).to_le_bytes());
    if t.entries.is_empty() {
        return v;
    }
    let mut root = Folder::default();
    let mut split: Vec<(String, String)> = Vec::new();
    for e in &t.entries {
        if let Ok(p) = &e.target {
            let p = p.replace('/', "\\");
            let (dir, file) = match p.rfind('\\') {
                Some(i) => (p[..i].to_string(), p[i + 1..].to_string()),
                None => (String::new(), p.clone()),
            };
            if !dir.is_empty() {
                insert(&mut root, &dir.split('\\').collect::<Vec<_>>());
            }
            split.push((dir, file));
        } else {
            split.push((String::new(), String::new()));
        }
    }
    let mut index = BTreeMap::new();
    let mut next = 1u32;
    number(&root, None, &mut next, &mut index);
    v.extend_from_slice(&t.ancestor_level.to_le_bytes());
    write(&root, &mut v);
    for (e, (dir, file)) in t.entries.iter().zip(split) {
        v.extend_from_slice(&e.flags.to_le_bytes());
        match &e.target {
            Ok(_) => put_string(&mut v, &file),
            Err(res) => v.extend_from_slice(&res.to_le_bytes()),
        }
        v.extend_from_slice(&e.node_index.to_le_bytes());
        if has_use_file {
            v.extend_from_slice(&e.use_file.to_le_bytes());
        }
        if e.target.is_ok() {
            let fi = if dir.is_empty() { 0 } else { index[&dir] };
            v.extend_from_slice(&fi.to_le_bytes());
        }
    }
    v
}

fn put_string(v: &mut Vec<u8>, s: &str) {
    v.extend_from_slice(&(s.len() as u32).to_le_bytes());
    v.extend_from_slice(s.as_bytes());
}

pub fn describe(t: &RefTable) -> String {
    let mut s = format!("ancestor {}  {} external node(s)\n", t.ancestor_level, t.entries.len());
    for e in &t.entries {
        match &e.target {
            Ok(p) => s += &format!("  node {:>3}  flags {}  useFile {}  {}\n", e.node_index, e.flags, e.use_file, p),
            Err(r) => s += &format!("  node {:>3}  flags {}  useFile {}  resource #{}\n", e.node_index, e.flags, e.use_file, r),
        }
    }
    s
}

/// One rewrite of a file's table: every named entry gets `prefix\` in front
/// of its path (after `strip` leading folders are removed), the ancestor
/// level, flags and useFile are replaced when given, and `entry` overrides
/// name individual entries (`node=full\path\file`).
#[derive(Debug, Clone, Default)]
pub struct Rewrite {
    pub ancestor: Option<u32>,
    pub prefix: String,
    pub strip: usize,
    pub flags: Option<u32>,
    pub use_file: Option<u32>,
    pub entry: Vec<(u32, String)>,
}

pub fn rewrite(t: &RefTable, rw: &Rewrite) -> RefTable {
    let mut out = t.clone();
    if let Some(a) = rw.ancestor {
        out.ancestor_level = a;
    }
    for e in &mut out.entries {
        if let Ok(p) = &e.target {
            let mut parts: Vec<&str> = p.split('\\').collect();
            let file = parts.pop().unwrap_or("");
            let dirs: Vec<&str> = parts.into_iter().skip(rw.strip).collect();
            let mut path = String::new();
            if !rw.prefix.is_empty() {
                path.push_str(rw.prefix.trim_end_matches('\\'));
            }
            for d in dirs {
                if !path.is_empty() {
                    path.push('\\');
                }
                path.push_str(d);
            }
            if !path.is_empty() {
                path.push('\\');
            }
            path.push_str(file);
            e.target = Ok(path);
        }
        if let Some(f) = rw.flags {
            e.flags = f;
        }
        if let Some(u) = rw.use_file {
            e.use_file = u;
        }
        for (node, p) in &rw.entry {
            if *node == e.node_index {
                e.target = Ok(p.replace('/', "\\"));
            }
        }
    }
    out
}

/// The file with its table replaced; the body is kept byte for byte (the
/// container's own compressed stream is reused when it has one).
pub fn with_table(bytes: &[u8], t: &RefTable) -> Vec<u8> {
    let mut g = Gbx::parse(bytes);
    g.ref_table = serialize(t, g.version >= 5);
    let body = g.body.clone();
    g.write_body_recompressed(&body)
}

pub fn table_of(bytes: &[u8]) -> RefTable {
    let g = Gbx::parse(bytes);
    parse(&g.ref_table, g.version >= 5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_numbers_folders_depth_first() {
        let t = RefTable {
            ancestor_level: 1,
            entries: vec![
                RefEntry { flags: 1, target: Ok("Media\\Prefab\\Items\\ObstaclePusher\\ObstaclePusher16m.Prefab.Gbx".into()), node_index: 1, use_file: 0 },
                RefEntry { flags: 1, target: Ok("Media\\Modifier\\ItemObstacleOff.Gbx".into()), node_index: 2, use_file: 0 },
                RefEntry { flags: 1, target: Ok("Media\\PlaceParam\\ObstaclePusher16m_H2V2.PlaceParam.Gbx".into()), node_index: 3, use_file: 0 },
            ],
        };
        let bytes = serialize(&t, true);
        assert_eq!(parse(&bytes, true), t);
        // folder indices: Media 1, Prefab 2, Items 3, ObstaclePusher 4, Modifier 5, PlaceParam 6
        let mut r = Reader::new(&bytes);
        assert_eq!(r.u32(), 3);
        assert_eq!(r.u32(), 1);
        let mut names = Vec::new();
        read_folders(&mut r, &mut names, None);
        assert_eq!(names, vec!["Media", "Media\\Prefab", "Media\\Prefab\\Items", "Media\\Prefab\\Items\\ObstaclePusher", "Media\\Modifier", "Media\\PlaceParam"]);
        r.u32();
        r.string();
        r.u32();
        r.u32();
        assert_eq!(r.u32(), 4);
    }

    #[test]
    fn leading_backslash_and_drive_spellings_round_trip() {
        for p in ["\\GameData\\Stadium\\Media\\X.Gbx", "C:\\Program Files (x86)\\Trackmania\\GameData\\Stadium\\Media\\X.Gbx", "X.Gbx"] {
            let t = RefTable { ancestor_level: 2, entries: vec![RefEntry { flags: 1, target: Ok(p.to_string()), node_index: 1, use_file: 0 }] };
            assert_eq!(parse(&serialize(&t, true), true), t, "{p}");
        }
    }

    #[test]
    fn rewrite_prefix_and_strip() {
        let t = RefTable { ancestor_level: 1, entries: vec![RefEntry { flags: 1, target: Ok("Media\\Prefab\\X.Prefab.Gbx".into()), node_index: 1, use_file: 0 }] };
        let rw = Rewrite { ancestor: Some(3), prefix: "GameData\\Stadium".into(), strip: 0, flags: Some(0), use_file: Some(1), entry: vec![] };
        let o = rewrite(&t, &rw);
        assert_eq!(o.ancestor_level, 3);
        assert_eq!(o.entries[0].target, Ok("GameData\\Stadium\\Media\\Prefab\\X.Prefab.Gbx".to_string()));
        assert_eq!(o.entries[0].flags, 0);
        assert_eq!(o.entries[0].use_file, 1);
        let rw2 = Rewrite { strip: 1, ..Default::default() };
        assert_eq!(rewrite(&t, &rw2).entries[0].target, Ok("Prefab\\X.Prefab.Gbx".to_string()));
    }
}
