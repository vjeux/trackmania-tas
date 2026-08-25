//! `tmmaps header` — what a `.Map.Gbx` DECLARES about itself, before any block
//! is read.
//!
//! Written for one question: *this map loads and simulates in the dedicated
//! server and never opens in the in-game editor — is there anything structural
//! that only this map has?* (146612 "Spaghetti Nights 2", `CANNOT-OPEN.md`.)
//! The engine reads the body; the **editor** additionally needs everything the
//! header declares — the title, the exe build, the external references, the
//! embedded object zip. Those are exactly the fields nothing else here printed.
//!
//! Every number is read off the file. Nothing is inferred, and a field the file
//! does not carry prints as `-` rather than as a default that reads like a
//! measurement.
//!
//! ```text
//! tmmaps header MAP [MAP ...]        one block per map
//! tmmaps header MAP ... --tsv        one ROW per map: the corpus comparison
//! tmmaps header MAP ... --names      uid / name / author: the identity audit
//! tmmaps header MAP --xml            the community XML chunk, verbatim
//! ```
//!
//! A difference only one map has is a lead; a difference several maps share is
//! not — which is why `--tsv` exists and why the summary counts distinct
//! values per column when given more than one map.

use crate::gbx::Gbx;

const HEAVY: u32 = 0x8000_0000;

/// One header (`user_data`) chunk: id, heavy flag, bytes.
pub struct HChunk {
    pub id: u32,
    pub heavy: bool,
    pub data: Vec<u8>,
}

/// Split `user_data` into its chunks. `None` when the table does not account
/// for every byte — a partial parse would be indistinguishable from a whole one.
pub fn user_chunks(ud: &[u8]) -> Option<Vec<HChunk>> {
    if ud.len() < 4 {
        return None;
    }
    let n = u32::from_le_bytes(ud[0..4].try_into().ok()?) as usize;
    if n == 0 || n > 64 || ud.len() < 4 + 8 * n {
        return None;
    }
    let mut spec = Vec::with_capacity(n);
    let mut total = 0usize;
    for i in 0..n {
        let o = 4 + 8 * i;
        let id = u32::from_le_bytes(ud[o..o + 4].try_into().ok()?);
        let sz = u32::from_le_bytes(ud[o + 4..o + 8].try_into().ok()?);
        total += (sz & !HEAVY) as usize;
        spec.push((id, sz & HEAVY != 0, (sz & !HEAVY) as usize));
    }
    if 4 + 8 * n + total != ud.len() {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    let mut o = 4 + 8 * n;
    for (id, heavy, size) in spec {
        out.push(HChunk { id, heavy, data: ud[o..o + size].to_vec() });
        o += size;
    }
    Some(out)
}

/// The community XML chunk (`0x03043005`), which is a single GBX string.
pub fn header_xml(chunks: &[HChunk]) -> Option<String> {
    let c = chunks.iter().find(|c| c.id == 0x0304_3005)?;
    if c.data.len() < 4 {
        return None;
    }
    let n = u32::from_le_bytes(c.data[0..4].try_into().ok()?) as usize;
    if 4 + n > c.data.len() {
        return None;
    }
    Some(String::from_utf8_lossy(&c.data[4..4 + n]).into_owned())
}


/// Strip ManiaPlanet markup from a name. The decoder lives in the format
/// crate — see `gbx::name` for why there is exactly one of it.
pub use gbx::name::strip_fmt;

/// An attribute out of the header XML: `name="value"` inside `<tag ...>`.
///
/// The value is XML-UNESCAPED here, at the one place it is read. 208024's
/// header holds `Miru&apos;s Hell 2` and 285268's `Pain ft Mango &amp;
/// Teuflum`; handing those out raw made both look like names this repo had got
/// wrong in the 2026-08-25 audit, which is the false positive that buries the
/// real ones.
fn attr(xml: &str, tag: &str, name: &str) -> Option<String> {
    let t = xml.find(&format!("<{tag} "))?;
    let end = xml[t..].find('>')? + t;
    let seg = &xml[t..end];
    let k = format!("{name}=\"");
    let a = seg.find(&k)? + k.len();
    let b = seg[a..].find('"')? + a;
    Some(gbx::name::unescape_xml(&seg[a..b]))
}

/// Every `<dep file="…"/>` in the XML: the external files the map declares it
/// needs. A missing dependency is invisible to a simulation and is exactly the
/// kind of thing an editor open would have to resolve.
fn deps(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(p) = xml[i..].find("<dep ") {
        let s = i + p;
        let Some(e) = xml[s..].find('>') else { break };
        let seg = &xml[s..s + e];
        if let Some(a) = seg.find("file=\"") {
            let a = a + 6;
            if let Some(b) = seg[a..].find('"') {
                out.push(seg[a..a + b].to_string());
            }
        }
        i = s + e;
    }
    out
}

/// The reference table's EXTERNAL entries, by name. These are files outside the
/// map that the container itself points at.
pub fn ref_entries(g: &Gbx) -> (u32, Vec<String>) {
    let b = &g.ref_table;
    if b.len() < 4 {
        return (0, Vec::new());
    }
    let mut r = crate::gbx::Reader::new(b);
    let n = r.u32();
    if n == 0 {
        return (0, Vec::new());
    }
    r.u32(); // ancestorLevel
    let nfolders = r.u32();
    fn folders(r: &mut crate::gbx::Reader, cnt: u32) {
        for _ in 0..cnt {
            r.string();
            let sub = r.u32();
            folders(r, sub);
        }
    }
    folders(&mut r, nfolders);
    let mut names = Vec::new();
    for _ in 0..n {
        let flags = r.u32();
        if flags & 4 == 0 {
            names.push(r.string());
        } else {
            names.push(format!("resource#{}", r.u32()));
        }
        r.u32(); // nodeIndex
        if g.version >= 5 {
            r.u32(); // useFile
        }
        if flags & 4 == 0 {
            r.u32(); // folderIndex
        }
    }
    (n, names)
}

/// The names inside a zip blob, read from its central directory.
fn zip_names(z: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 46 <= z.len() {
        if &z[i..i + 4] == b"PK\x01\x02" {
            let nlen = u16::from_le_bytes([z[i + 28], z[i + 29]]) as usize;
            let elen = u16::from_le_bytes([z[i + 30], z[i + 31]]) as usize;
            let clen = u16::from_le_bytes([z[i + 32], z[i + 33]]) as usize;
            if i + 46 + nlen <= z.len() {
                out.push(String::from_utf8_lossy(&z[i + 46..i + 46 + nlen]).into_owned());
            }
            i += 46 + nlen + elen + clen;
        } else {
            i += 1;
        }
    }
    out
}

/// The embedded-objects chunk (`0x03043054`): the zip of custom items a map
/// carries inside itself. Returns (zip bytes, entry names).
pub fn embedded_zip(body: &[u8]) -> Option<(usize, Vec<String>)> {
    let (_, _, payload, size) = crate::gbx::all_skip_chunks(body)
        .into_iter()
        .find(|(cid, _, _, _)| *cid == 0x0304_3054)?;
    let seg = &body[payload..(payload + size).min(body.len())];
    let z = seg.windows(4).position(|w| w == b"PK\x03\x04")?;
    Some((seg.len() - z, zip_names(&seg[z..])))
}

/// One map's declared facts, in the order they are read out of the file.
pub struct MapHeader {
    pub path: String,
    pub bytes: u64,
    pub gbxver: u16,
    pub class: u32,
    pub nodes: u32,
    pub extrefs: u32,
    pub ref_names: Vec<String>,
    pub chunks: Vec<(u32, usize)>,
    pub uid: String,
    /// `authortime` from the header XML, in MILLISECONDS. The author time is a
    /// number IN THE MAP FILE — which is what makes it a legitimate yardstick
    /// for a project that may not consult a human ghost for anything. Print it
    /// as seconds with a decimal.
    pub authortime: String,
    pub gold: String,
    pub silver: String,
    pub bronze: String,
    pub title: String,
    pub exever: String,
    pub exebuild: String,
    pub author: String,
    pub name: String,
    pub envir: String,
    pub mood: String,
    pub maptype: String,
    pub mapstyle: String,
    pub validated: String,
    pub nblocks_declared: String,
    /// `lightmap="N"` — the baked-lighting version the CLIENT must load. The
    /// dedicated server never reads it, so it is exactly the kind of field that
    /// can separate a map that simulates from a map that will not open.
    pub lightmap: String,
    /// `hasghostblocks="1"` — drive-through blocks.
    pub ghostblocks: String,
    pub displaycost: String,
    pub deps: Vec<String>,
    pub blocks_u: usize,
    pub blocks_b: usize,
    pub items: usize,
    pub models: usize,
    pub zip_bytes: usize,
    pub zip_entries: Vec<String>,
    pub thumb_bytes: usize,
}

impl MapHeader {
    /// How many entries of the embedded zip are custom **BLOCKS** rather than
    /// items. A `.Block.Gbx` is a different thing from a `.Item.Gbx` to the
    /// editor, which is why it is counted separately.
    pub fn zip_blocks(&self) -> usize {
        self.zip_entries.iter().filter(|z| z.ends_with(".Block.Gbx")).count()
    }
}

/// `attr`, for the self-test: the selftest asserts WHICH TAG the name comes
/// off, which is the thing the audit's bug was in.
pub fn attr_pub(xml: &str, tag: &str, name: &str) -> Option<String> {
    attr(xml, tag, name)
}

pub fn read(path: &str) -> Result<MapHeader, String> {
    let g = Gbx::load(std::path::Path::new(path)).map_err(|e| format!("{path}: {e}"))?;
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let chunks = user_chunks(&g.user_data).unwrap_or_default();
    let xml = header_xml(&chunks).unwrap_or_default();
    let (extrefs, ref_names) = ref_entries(&g);
    let m = crate::map::MapFile::load(std::path::Path::new(path));
    let mut models: Vec<&str> = m.items.iter().map(|i| i.model.as_str()).collect();
    models.sort_unstable();
    models.dedup();
    let (zip_bytes, zip_entries) = embedded_zip(&g.body).unwrap_or((0, Vec::new()));
    let thumb = chunks.iter().find(|c| c.id == 0x0304_3007).map(|c| c.data.len()).unwrap_or(0);
    let get = |t: &str, a: &str| attr(&xml, t, a).unwrap_or_else(|| "-".into());
    Ok(MapHeader {
        path: path.to_string(),
        bytes,
        gbxver: g.version,
        class: g.class_id,
        nodes: g.num_nodes,
        extrefs,
        ref_names,
        chunks: chunks.iter().map(|c| (c.id, c.data.len())).collect(),
        uid: get("ident", "uid"),
        authortime: get("times", "authortime"),
        gold: get("times", "gold"),
        silver: get("times", "silver"),
        bronze: get("times", "bronze"),
        title: get("header", "title"),
        exever: get("header", "exever"),
        exebuild: get("header", "exebuild"),
        author: get("ident", "author"),
        // The map's own declared NAME lives on `<ident>`, beside the uid and
        // the author — NOT on `<desc>`. This read said `desc` until the
        // 2026-08-25 name audit, so every map printed `name -` and nothing in
        // this repo was ever checked against the name the file declares. That
        // is how "The Magnet Trial" — a title we invented from 186935's skin
        // dependencies — got published for a map whose header says
        // `[object Object]`.
        name: get("ident", "name"),
        envir: get("desc", "envir"),
        mood: get("desc", "mood"),
        maptype: get("desc", "maptype"),
        mapstyle: get("desc", "mapstyle"),
        validated: get("desc", "validated"),
        nblocks_declared: get("desc", "nblocks"),
        lightmap: get("header", "lightmap"),
        ghostblocks: get("desc", "hasghostblocks"),
        displaycost: get("desc", "displaycost"),
        deps: deps(&xml),
        blocks_u: m.blocks.len(),
        blocks_b: m.baked.len(),
        items: m.items.len(),
        models: models.len(),
        zip_bytes,
        zip_entries,
        thumb_bytes: thumb,
    })
}

pub fn cmd(args: &[String]) {
    let mut paths: Vec<String> = Vec::new();
    let mut tsv = false;
    let mut want_xml = false;
    let mut names = false;
    for a in &args[2..] {
        match a.as_str() {
            "--tsv" => tsv = true,
            "--xml" => want_xml = true,
            "--names" => names = true,
            s if s.starts_with("--") => {
                eprintln!("tmmaps header: unknown option `{s}`");
                std::process::exit(2);
            }
            s => paths.push(s.to_string()),
        }
    }
    if paths.is_empty() {
        eprintln!("usage: tmmaps header MAP [MAP ...] [--tsv] [--xml] [--names]");
        std::process::exit(2);
    }

    if want_xml {
        for p in &paths {
            let g = match Gbx::load(std::path::Path::new(p)) {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("{p}: {e}");
                    std::process::exit(1);
                }
            };
            let chunks = user_chunks(&g.user_data).unwrap_or_default();
            println!("=== {p}");
            println!("{}", header_xml(&chunks).unwrap_or_else(|| "(no XML chunk)".into()));
        }
        return;
    }

    if names {
        // The identity audit view: what the FILE says it is. One row per map,
        // uid first so it can be joined against trackmania.io — which is the
        // only independent check on a name, since every other document in this
        // repo is one we wrote ourselves.
        //
        // A map that fails to parse is a LOUD row here, not a skipped one: an
        // absent artefact must never read as agreement.
        println!("path\tuid\tname\trawname\tauthorid\tauthortime");
        let mut bad = 0;
        for p in &paths {
            match read(p) {
                Ok(h) => println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    h.path, h.uid, strip_fmt(&h.name), h.name, h.author,
                    crate::secs::secs_str(&h.authortime)
                ),
                Err(e) => {
                    bad += 1;
                    println!("{p}\tERROR\tERROR\tERROR\tERROR\t-");
                    eprintln!("{e}");
                }
            }
        }
        if bad > 0 {
            std::process::exit(1);
        }
        return;
    }

    let mut hs = Vec::new();
    for p in &paths {
        match read(p) {
            Ok(h) => hs.push(h),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }

    if tsv {
        // Label each row by its PARENT directory as well as its file name: the
        // corpus stores most maps as `<id>/map.Map.Gbx`, so a bare base name
        // collapses every row to the same string.
        let label = |p: &str| -> String {
            let mut it = p.rsplit('/');
            let base = it.next().unwrap_or(p);
            match it.next() {
                Some(dir) => format!("{dir}/{base}"),
                None => base.to_string(),
            }
        };
        println!(
            "map\tbytes\tauthortime\tgbxver\tnodes\textrefs\thdrchunks\ttitle\texever\texebuild\tenvir\tmood\tmaptype\tmapstyle\tvalidated\tlightmap\tghostblocks\tdisplaycost\tblocks_u\tblocks_b\titems\tmodels\tdeps\tzip_bytes\tzip_files\tzip_blocks\tthumb"
        );
        for h in &hs {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                label(&h.path), h.bytes, crate::secs::secs_str(&h.authortime), h.gbxver, h.nodes, h.extrefs, h.chunks.len(),
                h.title, h.exever, h.exebuild, h.envir, h.mood, h.maptype, h.mapstyle,
                h.validated, h.lightmap, h.ghostblocks, h.displaycost, h.blocks_u, h.blocks_b, h.items, h.models,
                h.deps.len(), h.zip_bytes, h.zip_entries.len(), h.zip_blocks(), h.thumb_bytes
            );
        }
        if hs.len() > 1 {
            // A difference only one map has is a lead; one several share is not.
            let col = |f: &dyn Fn(&MapHeader) -> String| {
                let mut v: Vec<String> = hs.iter().map(f).collect();
                v.sort();
                v.dedup();
                v
            };
            println!();
            println!("# distinct values across {} maps", hs.len());
            for (name, f) in [
                ("gbxver", &(|h: &MapHeader| h.gbxver.to_string()) as &dyn Fn(&MapHeader) -> String),
                ("extrefs", &|h: &MapHeader| h.extrefs.to_string()),
                ("hdrchunks", &|h: &MapHeader| h.chunks.len().to_string()),
                ("title", &|h: &MapHeader| h.title.clone()),
                ("exever", &|h: &MapHeader| h.exever.clone()),
                ("exebuild", &|h: &MapHeader| h.exebuild.clone()),
                ("envir", &|h: &MapHeader| h.envir.clone()),
                ("maptype", &|h: &MapHeader| h.maptype.clone()),
                ("mapstyle", &|h: &MapHeader| h.mapstyle.clone()),
                ("validated", &|h: &MapHeader| h.validated.clone()),
                ("lightmap", &|h: &MapHeader| h.lightmap.clone()),
                ("ghostblocks", &|h: &MapHeader| h.ghostblocks.clone()),
                ("deps", &|h: &MapHeader| h.deps.len().to_string()),
                ("zip_files", &|h: &MapHeader| h.zip_entries.len().to_string()),
                ("zip_blocks", &|h: &MapHeader| h.zip_blocks().to_string()),
            ] {
                let v = col(f);
                println!("{name}\t{}", v.join(" | "));
            }
        }
        return;
    }

    for h in &hs {
        println!("=== {}  {} bytes", h.path, h.bytes);
        println!(
            "  container   gbx v{}  class 0x{:08X}  nodes {}  extrefs {}",
            h.gbxver, h.class, h.nodes, h.extrefs
        );
        for n in &h.ref_names {
            println!("              extref: {n}");
        }
        print!("  hdr chunks ");
        for (id, sz) in &h.chunks {
            print!(" 0x{id:08X}:{sz}");
        }
        println!();
        let plain = strip_fmt(&h.name);
        println!("  ident       uid {}  author {}  name {}", h.uid, h.author, plain);
        if plain != h.name {
            println!("              name (raw, with markup) {}", h.name);
        }
        println!(
            "  times       author {}  gold {}  silver {}  bronze {}",
            crate::secs::secs_str(&h.authortime),
            crate::secs::secs_str(&h.gold),
            crate::secs::secs_str(&h.silver),
            crate::secs::secs_str(&h.bronze)
        );
        println!(
            "  title       {}  exever {}  exebuild {}",
            h.title, h.exever, h.exebuild
        );
        println!(
            "  desc        envir {}  mood {}  maptype {}  mapstyle {}  validated {}  nblocks(declared) {}",
            h.envir, h.mood, h.maptype, h.mapstyle, h.validated, h.nblocks_declared
        );
        println!(
            "  body        blocks {} unbaked + {} baked  items {} ({} distinct models)",
            h.blocks_u, h.blocks_b, h.items, h.models
        );
        println!(
            "  embedded    zip {} bytes, {} file(s)   thumbnail {} bytes",
            h.zip_bytes,
            h.zip_entries.len(),
            h.thumb_bytes
        );
        for z in h.zip_entries.iter().take(40) {
            println!("              {z}");
        }
        if h.zip_entries.len() > 40 {
            println!("              … {} more", h.zip_entries.len() - 40);
        }
        if h.deps.is_empty() {
            println!("  deps        none declared");
        } else {
            println!("  deps        {}", h.deps.len());
            for d in &h.deps {
                println!("              {d}");
            }
        }
    }
}
