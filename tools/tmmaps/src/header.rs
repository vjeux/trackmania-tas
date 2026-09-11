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

/// Rebuild `user_data` from a chunk list (the inverse of `user_chunks`).
pub fn user_data_from_chunks(chunks: &[HChunk]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
    for c in chunks {
        out.extend_from_slice(&c.id.to_le_bytes());
        let sz = c.data.len() as u32 | if c.heavy { HEAVY } else { 0 };
        out.extend_from_slice(&sz.to_le_bytes());
    }
    for c in chunks {
        out.extend_from_slice(&c.data);
    }
    out
}

/// Replace the value of `attr` inside the first `<tag ...>` element of the
/// header XML (any value length). Returns None when the element or the
/// attribute is absent.
pub fn xml_set_attr(xml: &str, tag: &str, attr: &str, value: &str) -> Option<String> {
    let open = format!("<{tag} ");
    let start = xml.find(&open)?;
    let end = start + xml[start..].find('>')?;
    let elem = &xml[start..end];
    let key = format!("{attr}=\"");
    let k = elem.find(&key)?;
    let vstart = start + k + key.len();
    let vend = vstart + xml[vstart..].find('"')?;
    Some(format!("{}{}{}", &xml[..vstart], value, &xml[vend..]))
}

/// Medal times (ms) and the validated flag, written everywhere the header
/// spells them: chunk 0x03043002 (version 13: u32 version, u8 flag, then
/// bronze / silver / gold / author as u32) and the XML `<times .../>` and
/// `validated` attribute. `author_score` follows the author time.
pub fn set_times(user_data: &[u8], bronze: u32, silver: u32, gold: u32, author: u32, validated: bool) -> Option<Vec<u8>> {
    let mut chunks = user_chunks(user_data)?;
    for c in chunks.iter_mut() {
        if c.id == 0x0304_3002 {
            let version = u32::from_le_bytes(c.data[0..4].try_into().ok()?);
            if version < 5 || c.data.len() < 21 {
                return None;
            }
            for (i, v) in [bronze, silver, gold, author].iter().enumerate() {
                c.data[5 + 4 * i..9 + 4 * i].copy_from_slice(&v.to_le_bytes());
            }
        }
    }
    let xml = header_xml(&chunks)?;
    let mut xml = xml_set_attr(&xml, "times", "bronze", &bronze.to_string())?;
    xml = xml_set_attr(&xml, "times", "silver", &silver.to_string())?;
    xml = xml_set_attr(&xml, "times", "gold", &gold.to_string())?;
    xml = xml_set_attr(&xml, "times", "authortime", &author.to_string())?;
    xml = xml_set_attr(&xml, "times", "authorscore", &author.to_string()).unwrap_or(xml);
    xml = xml_set_attr(&xml, "desc", "validated", if validated { "1" } else { "0" })?;
    for c in chunks.iter_mut() {
        if c.id == 0x0304_3005 {
            let mut d = Vec::with_capacity(4 + xml.len());
            d.extend_from_slice(&(xml.len() as u32).to_le_bytes());
            d.extend_from_slice(xml.as_bytes());
            c.data = d;
        }
    }
    Some(user_data_from_chunks(&chunks))
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

/// The (ident, author) an item file declares in its header chunk 0x2E001003
/// (collector ident: lookback id version, id string, collection, author).
/// Read as the first two lookback strings of the user data after the ids
/// version word -- enough for the items we embed (fresh lookback table).
pub fn item_ident_author(bytes: &[u8]) -> Option<(String, String)> {
    let g = Gbx::parse(bytes);
    let ud = &g.user_data;
    let n = u32::from_le_bytes(ud[0..4].try_into().ok()?) as usize;
    let mut data = 4 + n * 8;
    for i in 0..n {
        let o = 4 + i * 8;
        let id = u32::from_le_bytes(ud[o..o + 4].try_into().ok()?);
        let size = (u32::from_le_bytes(ud[o + 4..o + 8].try_into().ok()?) & 0x7fff_ffff) as usize;
        if id == 0x2E00_1003 {
            let mut r = data;
            let _ver = u32::from_le_bytes(ud[r..r + 4].try_into().ok()?); r += 4;
            // ident / author are lookback ids: 0x40000000 + inline string
            // pushes to the table, 0x40000001+ back-refs it. The old code
            // only accepted inline strings, so our own files (author ==
            // ident, written as back-ref 0x40000001 by Wr::id) came back as
            // the literal "#40000001" -- which the catalog then wrote into
            // the manifest/placement author, and the game instantiates such
            // items but never renders them (quads, grafts, T1-T4, 2026-09-05).
            // Resolve back-refs through the table like Rd::id does.
            let mut table: Vec<String> = Vec::new();
            let mut strings = Vec::new();
            // ident: id (lookback), collection (u32 id), author (lookback)
            for k in 0..3 {
                let w = u32::from_le_bytes(ud[r..r + 4].try_into().ok()?); r += 4;
                if k == 1 { continue; } // collection: plain id
                if w == 0xFFFF_FFFF {
                    strings.push(String::new());
                } else if (w >> 30) == 0 {
                    strings.push(format!("#{w:x}"));
                } else {
                    let idx = (w & 0x3FFF_FFFF) as usize;
                    if idx == 0 {
                        let l = u32::from_le_bytes(ud[r..r + 4].try_into().ok()?) as usize; r += 4;
                        let s = String::from_utf8_lossy(&ud[r..r + l]).to_string(); r += l;
                        table.push(s.clone());
                        strings.push(s);
                    } else if idx == 0x3FFF_FFFF {
                        strings.push(format!("#{w:x}"));
                    } else if let Some(s) = table.get(idx - 1) {
                        strings.push(s.clone());
                    } else {
                        strings.push(format!("#{w:x}"));
                    }
                }
            }
            return Some((strings[0].clone(), strings[1].clone()));
        }
        data += size;
    }
    None
}

/// A stored (uncompressed) zip with one more file appended.
pub fn zip_add(zip: &[u8], name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    // parse local headers
    let mut i = 0usize;
    while i + 30 <= zip.len() && &zip[i..i + 4] == b"PK\x03\x04" {
        let method = u16::from_le_bytes(zip[i + 8..i + 10].try_into().unwrap());
        let csize = u32::from_le_bytes(zip[i + 18..i + 22].try_into().unwrap()) as usize;
        let nlen = u16::from_le_bytes(zip[i + 26..i + 28].try_into().unwrap()) as usize;
        let xlen = u16::from_le_bytes(zip[i + 28..i + 30].try_into().unwrap()) as usize;
        let fname = String::from_utf8_lossy(&zip[i + 30..i + 30 + nlen]).to_string();
        let start = i + 30 + nlen + xlen;
        let data = match method {
            0 => zip[start..start + csize].to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec(&zip[start..start + csize]).expect("inflate"),
            _ => panic!("zip method {method}"),
        };
        files.push((fname, data));
        i = start + csize;
    }
    files.push((name.to_string(), bytes.to_vec()));
    let map: std::collections::BTreeMap<String, Vec<u8>> = files.into_iter().collect();
    deflated_zip(&map)
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// A zip with every entry deflated (method 8), as the game writes its own
/// embedded-item archives.
pub fn deflated_zip(files: &std::collections::BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let off = out.len() as u32;
        let crc = crc32(data);
        let n = name.as_bytes();
        let comp = miniz_oxide::deflate::compress_to_vec(data, 6);
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&[20, 0, 0, 0, 8, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(comp.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(n.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(n);
        out.extend_from_slice(&comp);
        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 8, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(comp.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(n.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0u8; 8]);
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&off.to_le_bytes());
        central.extend_from_slice(n);
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

pub fn stored_zip(files: &std::collections::BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let off = out.len() as u32;
        let crc = crc32(data);
        let n = name.as_bytes();
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(n.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(n);
        out.extend_from_slice(data);
        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(n.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0u8; 8]);
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&off.to_le_bytes());
        central.extend_from_slice(n);
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// Rewrite an item's collection id in BOTH ident chunks (header 0x2E001003
/// and body 0x2E00100B). A map silently drops an embedded item whose ident
/// collection differs from its own (BlueBay maps are 0x1C, Stadium 0x1A):
/// "found but dropped", no dialog. Every item embedded into a map must go
/// through this with the map's collection.
pub fn set_ident_collection(bytes: &[u8], collection: u32) -> Vec<u8> {
    use crate::gbx::Reader;
    let mut g = Gbx::parse(bytes);
    let ud = g.user_data.clone();
    let n = u32::from_le_bytes(ud[0..4].try_into().unwrap()) as usize;
    let mut off = 4 + n * 8;
    let mut new_ud = ud.clone();
    for i in 0..n {
        let id = u32::from_le_bytes(ud[4 + i * 8..8 + i * 8].try_into().unwrap());
        let size = (u32::from_le_bytes(ud[8 + i * 8..12 + i * 8].try_into().unwrap()) & 0x7FFF_FFFF) as usize;
        if id == 0x2E001003 {
            let mut r = Reader::new(&ud[off..off + size]);
            r.u32(); // lookback version
            let w = r.u32();
            if (w & 0x3FFF_FFFF) == 0 && w != 0xFFFF_FFFF {
                r.string();
            }
            let at = off + r.o;
            new_ud[at..at + 4].copy_from_slice(&collection.to_le_bytes());
        }
        off += size;
    }
    g.user_data = new_ud;
    let mut body = g.body.clone();
    let pos = body.windows(4).position(|w| w == 0x2E00100Bu32.to_le_bytes()).expect("body ident chunk");
    let mut o = pos + 4;
    let w = u32::from_le_bytes(body[o..o + 4].try_into().unwrap());
    o += 4;
    if (w & 0x3FFF_FFFF) == 0 && w != 0xFFFF_FFFF {
        let l = u32::from_le_bytes(body[o..o + 4].try_into().unwrap()) as usize;
        o += 4 + l;
    }
    body[o..o + 4].copy_from_slice(&collection.to_le_bytes());
    g.body = body.clone();
    g.write_body_recompressed(&body)
}

// ---------------------------------------------------------------- game skins

/// The class id of `CPlugGameSkin`, and the id of the HEADER chunk a skinnable
/// model carries (items AND block infos: `RaceScreen6x1.Item.Gbx` declares
/// `Any\Advertisement6x1\`, the `TechnicsScreen155Straight` blockinfo
/// `Any\Advertisement16x9\`). This header chunk is the ONLY place the
/// declaration lives — the body's `SkinDirectory` fields (0x2E001010,
/// 0x2E00201E) stay empty in every Nadeo screen item — and it is what makes
/// the game feed the model's `Image` texture with the current in-game
/// advertisement (the campaign artwork) or with a placement's own skin file.
/// A model without it draws the material's default texture: the yellow
/// `RaceAd6x1.dds` "NADEO / TRACKMANIA" panel every tiny screen showed.
pub const GAME_SKIN_CHUNK: u32 = 0x090F_4000;

/// One texture slot of a game skin: `*Image` → `Stadium\Media\Texture\Image\
/// RaceAd6x1.dds` on the 6x1 screen (`*` = every material's `Image` sampler,
/// the file = the default texture the skin replaces).
#[derive(Clone, Debug, PartialEq)]
pub struct SkinFid {
    pub class: u32,
    pub name: String,
    pub file: String,
    pub flag: u32,
}

/// Header chunk 0x090F4000, read as: `u8 version` (8), `string dir` (the skin
/// folder under `Skins\`), `string parent`, `string u03`, `u8 count`, `count`
/// × `SkinFid`, then 16 trailing bytes (four words; items `0,0,0,1`, the
/// screen blockinfo `0,1,0,1`). Measured on every skinned Stadium item and
/// blockinfo (`mapgeom skins`), which all decode to the exact byte count.
#[derive(Clone, Debug, PartialEq)]
pub struct GameSkin {
    pub version: u8,
    pub dir: String,
    pub parent: String,
    pub u03: String,
    pub fids: Vec<SkinFid>,
    pub tail: Vec<u8>,
}

impl GameSkin {
    pub fn decode(payload: &[u8]) -> Option<GameSkin> {
        if payload.len() < 1 + 4 + 4 + 4 + 1 {
            return None;
        }
        let mut r = crate::gbx::Reader::new(payload);
        let version = r.u8();
        let mut string = |r: &mut crate::gbx::Reader| -> Option<String> {
            if r.o + 4 > payload.len() {
                return None;
            }
            let n = r.u32() as usize;
            if r.o + n > payload.len() {
                return None;
            }
            Some(String::from_utf8_lossy(r.bytes(n)).into_owned())
        };
        let dir = string(&mut r)?;
        let parent = string(&mut r)?;
        let u03 = string(&mut r)?;
        if r.o >= payload.len() {
            return None;
        }
        let count = r.u8() as usize;
        let mut fids = Vec::with_capacity(count);
        for _ in 0..count {
            if r.o + 4 > payload.len() {
                return None;
            }
            let class = r.u32();
            let name = string(&mut r)?;
            let file = string(&mut r)?;
            if r.o + 4 > payload.len() {
                return None;
            }
            let flag = r.u32();
            fids.push(SkinFid { class, name, file, flag });
        }
        let tail = payload[r.o..].to_vec();
        Some(GameSkin { version, dir, parent, u03, fids, tail })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut b = vec![self.version];
        let put = |b: &mut Vec<u8>, s: &str| {
            b.extend_from_slice(&(s.len() as u32).to_le_bytes());
            b.extend_from_slice(s.as_bytes());
        };
        put(&mut b, &self.dir);
        put(&mut b, &self.parent);
        put(&mut b, &self.u03);
        b.push(self.fids.len() as u8);
        for f in &self.fids {
            b.extend_from_slice(&f.class.to_le_bytes());
            put(&mut b, &f.name);
            put(&mut b, &f.file);
            b.extend_from_slice(&f.flag.to_le_bytes());
        }
        b.extend_from_slice(&self.tail);
        b
    }

    /// One line: `dir | fids | tail`.
    pub fn summary(&self) -> String {
        let fids: Vec<String> = self.fids.iter().map(|f| format!("{:08X}:{}={}", f.class, f.name, f.file)).collect();
        let tail: Vec<String> = self.tail.chunks(4).map(|c| c.iter().map(|b| format!("{b:02x}")).collect::<String>()).collect();
        format!("v{} dir={} parent={:?} u03={:?} fids=[{}] tail={}", self.version, self.dir, self.parent, self.u03, fids.join(", "), tail.join(" "))
    }
}

/// The raw 0x090F4000 header chunk of any GBX file (item, blockinfo), if it
/// has one.
pub fn game_skin_chunk(bytes: &[u8]) -> Option<Vec<u8>> {
    let g = Gbx::parse(bytes);
    let chunks = user_chunks(&g.user_data)?;
    chunks.into_iter().find(|c| c.id == GAME_SKIN_CHUNK).map(|c| c.data)
}

/// The decoded skin declaration of a GBX file, if it has one.
pub fn game_skin(bytes: &[u8]) -> Option<GameSkin> {
    GameSkin::decode(&game_skin_chunk(bytes)?)
}

/// The file with its 0x090F4000 header chunk replaced (or inserted right
/// after the collector-description chunk 0x2E001003 and the icon 0x2E001004,
/// where Nadeo's items carry it). The body is untouched; the header table is
/// rebuilt. Uncompressed and compressed bodies both come back as they were.
pub fn set_game_skin_chunk(bytes: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut g = Gbx::parse(bytes);
    let mut chunks = user_chunks(&g.user_data).expect("header chunk table");
    if let Some(c) = chunks.iter_mut().find(|c| c.id == GAME_SKIN_CHUNK) {
        c.data = payload.to_vec();
    } else {
        let at = chunks
            .iter()
            .rposition(|c| c.id == 0x2E00_1003 || c.id == 0x2E00_1004)
            .map(|i| i + 1)
            .unwrap_or(0);
        chunks.insert(at, HChunk { id: GAME_SKIN_CHUNK, heavy: false, data: payload.to_vec() });
    }
    let mut ud = Vec::new();
    ud.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
    for c in &chunks {
        ud.extend_from_slice(&c.id.to_le_bytes());
        ud.extend_from_slice(&((c.data.len() as u32) | if c.heavy { HEAVY } else { 0 }).to_le_bytes());
    }
    for c in &chunks {
        ud.extend_from_slice(&c.data);
    }
    g.user_data = ud;
    let body = g.body.clone();
    if g.comp.is_some() {
        g.write_body_recompressed(&body)
    } else {
        g.write_body_uncompressed(&body)
    }
}

// ---------------------------------------------------------------- file refs

/// A `FileRef` (a placement's skin `packDesc`, flags bit 2 of the v8 anchored
/// object tail): `u8 version`, then (v ≥ 3) a 32-byte checksum, `string path`
/// (`Skins\Stadium\LightColors\WhiteCold.dds` on Summer 15's 462 skinned
/// lights), `string url` (empty unless the skin was downloaded).
#[derive(Clone, Debug, PartialEq)]
pub struct FileRef {
    pub version: u8,
    pub checksum: [u8; 32],
    pub path: String,
    pub url: String,
}

impl FileRef {
    /// Decode at the start of `b`; returns the value and the bytes consumed.
    pub fn decode(b: &[u8]) -> Option<(FileRef, usize)> {
        let mut r = crate::gbx::Reader::new(b);
        if b.is_empty() {
            return None;
        }
        let version = r.u8();
        let mut checksum = [0u8; 32];
        if version >= 3 {
            if r.o + 32 > b.len() {
                return None;
            }
            checksum.copy_from_slice(r.bytes(32));
        }
        let mut string = |r: &mut crate::gbx::Reader| -> Option<String> {
            if r.o + 4 > b.len() {
                return None;
            }
            let n = r.u32() as usize;
            if r.o + n > b.len() {
                return None;
            }
            Some(String::from_utf8_lossy(r.bytes(n)).into_owned())
        };
        let path = string(&mut r)?;
        let url = if version >= 3 || !path.is_empty() { string(&mut r)? } else { String::new() };
        Some((FileRef { version, checksum, path, url }, r.o))
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut b = vec![self.version];
        if self.version >= 3 {
            b.extend_from_slice(&self.checksum);
        }
        b.extend_from_slice(&(self.path.len() as u32).to_le_bytes());
        b.extend_from_slice(self.path.as_bytes());
        b.extend_from_slice(&(self.url.len() as u32).to_le_bytes());
        b.extend_from_slice(self.url.as_bytes());
        b
    }
}
