//! THE MAP'S MOD (E7, 2026-09-30) — `CGameCtnChallenge::ModPackDesc`: the header's `<desc mod="…">` names a texture mod whose
//! zip the map depends on (`<dep file="Skins\<Collection>\Mod\<mod>.zip" url="…"/>`). The game mounts the zip's `Image/<stem>.dds`
//! files over the collection's `Media\Texture\Image\` tree by FILENAME (the collection's `ModFid` overlay — RE 17, NOTES 00:12Z
//! 09-30), and every bitmap of that collection then loads the modded file: there is no lightmapper-specific texture path, so the
//! editor's bake emits the MOD's colours from every LM emitter that binds a replaced image — captured on g23 (frame 537, 09:25Z
//! 09-30: PS 15391's 568 TrackBorders*InWorld draws bind texture 6889 = `mods/NationsNORWAY/Image/TrackBordersInWorld_D.dds` bit
//! for bit on 12 of 13 mip levels through `re16_mipcmp`'s vertical flip; the pak's texture differs on 73 % of the mip-0 blocks).
//!
//! The port: `mount_for_map` reads the header, locates the zip (or its extracted `Image/` folder), and mounts its images as
//! `mapgeom::store::ModMount` — `<Collection>\Media\Texture\Image\<stem>.dds` for every `Image/<stem>.dds`, the collection being
//! the one the dep path names (`Skins\Stadium\Mod\…` → Stadium: a Stadium mod replaces Stadium-named images wherever a Stadium
//! material is bound — on the island maps through `Modifier\StadiumOnTerrain\*`). The sRGB / raw VIEW of a replaced texture stays
//! the shader binding's (RE 17 16:25Z (C)): the mount changes the bytes, not the decode. The `_R` / `_N` maps ride along unused
//! (the LM never reads them). Default ON whenever the header names a mod; `LMTOOL_MOD=0` off; `LMTOOL_MOD=<zip or dir>` mounts
//! that file instead (a study/override). A named mod whose file is nowhere = a setup WARNING (fatal under `--strict`): a bake
//! without the mod is not the editor's bake.
//!
//! Where the zip is looked for, in order: `LMTOOL_MOD_DIR` (colon-separated bank directories: `<dir>/<short>/Image/`,
//! `<dir>/<short>/<short>.zip`, `<dir>/<short>.zip`, `<dir>/<mangled>.zip`); beside the map (the user's Trackmania folder: the
//! ancestor holding `Maps`, then `Skins\<Collection>\Mod\<mangled>.zip`; the map's own folder too); the store's
//! `tiny/lightmap-re/mods/<short>/` (RE 17's bank of the five Nations mods). `<short>` = the url's file stem (`NationsNORWAY`),
//! `<mangled>` = the header's name (`_nadeo-download-cdn-ubi-com_trackmania_assets_2026_Mod_NationsNORWAY`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What the map header says about its mod.
#[derive(Debug, Clone, PartialEq)]
pub struct ModRef {
    /// `<desc mod="…">` as spelled (the CDN-mangled name for Nadeo's mods).
    pub mangled: String,
    /// The dep's `file` (`Skins\Stadium\Mod\<mangled>.zip`), when the header lists it.
    pub dep_file: Option<String>,
    /// The dep's `url`, when listed.
    pub url: Option<String>,
    /// The collection whose image tree the mod overlays: the dep path's `Skins\<Collection>\Mod\` segment; the map's own
    /// environment when the header carries no dep line.
    pub collection: String,
    /// The url's file stem (`NationsNORWAY`), else the mangled name's tail after `_Mod_`, else the mangled name.
    pub short: String,
}

/// The header's mod reference, `Ok(None)` for `mod=""` / no attribute.
pub fn header_mod(map_path: &str) -> Result<Option<ModRef>, String> {
    let g = tmmaps::gbx::Gbx::load(Path::new(map_path)).map_err(|e| format!("{map_path}: {e}"))?;
    let chunks = tmmaps::header::user_chunks(&g.user_data).unwrap_or_default();
    let xml = tmmaps::header::header_xml(&chunks).unwrap_or_default();
    Ok(mod_ref_from_xml(&xml))
}

/// `header_mod` on the header XML alone.
pub fn mod_ref_from_xml(xml: &str) -> Option<ModRef> {
    let mangled = tmmaps::header::attr_pub(xml, "desc", "mod").filter(|m| !m.is_empty())?;
    let envir = tmmaps::header::attr_pub(xml, "desc", "envir").unwrap_or_default();
    // the dep line naming the mod's zip: `<dep file="Skins\<Coll>\Mod\<mangled>.zip" url="…"/>`
    let mut dep_file = None;
    let mut url = None;
    let mut i = 0usize;
    while let Some(p) = xml[i..].find("<dep ") {
        let s = i + p;
        let Some(e) = xml[s..].find('>') else { break };
        let seg = &xml[s..s + e];
        let file = seg_attr(seg, "file");
        if let Some(f) = &file {
            let upper = f.to_ascii_uppercase();
            let stem = f.rsplit(['\\', '/']).next().unwrap_or(f);
            let stem = stem.strip_suffix(".zip").or_else(|| stem.strip_suffix(".ZIP")).unwrap_or(stem);
            if upper.contains("\\MOD\\") && stem.eq_ignore_ascii_case(&mangled) {
                dep_file = file.clone();
                url = seg_attr(seg, "url");
                break;
            }
        }
        i = s + e;
    }
    let collection = dep_file
        .as_deref()
        .and_then(|f| {
            let parts: Vec<&str> = f.split(['\\', '/']).collect();
            parts.iter().position(|p| p.eq_ignore_ascii_case("Skins")).and_then(|k| parts.get(k + 1)).map(|s| s.to_string())
        })
        .filter(|c| !c.is_empty() && !c.eq_ignore_ascii_case("Any"))
        .unwrap_or_else(|| envir.clone());
    let short = url
        .as_deref()
        .and_then(|u| u.rsplit('/').next())
        .map(|s| s.strip_suffix(".zip").unwrap_or(s).to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| mangled.rsplit("_Mod_").next().filter(|s| *s != mangled.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| mangled.clone());
    Some(ModRef { mangled, dep_file, url, collection, short })
}

fn seg_attr(seg: &str, name: &str) -> Option<String> {
    let k = format!("{name}=\"");
    let a = seg.find(&k)? + k.len();
    let b = seg[a..].find('"')? + a;
    Some(gbx::name::unescape_xml(&seg[a..b]))
}

/// A located mod: its image files (stem without `.dds` → bytes) and where they came from.
pub struct ModFiles {
    pub source: String,
    pub images: Vec<(String, Arc<Vec<u8>>)>,
}

/// The `Image/<stem>.dds` files of a mod zip (central directory driven: stored or deflated entries; a data-descriptor entry's
/// sizes are the central directory's). Other entries (Icon, `Image/*.tga`, nested folders) are skipped.
pub fn zip_images(zip: &[u8]) -> Result<Vec<(String, Arc<Vec<u8>>)>, String> {
    // the end-of-central-directory record: the last "PK\x05\x06" within the trailing 64 KiB + 22
    let tail = zip.len().saturating_sub(65_536 + 22);
    let eocd = zip[tail..].windows(4).rposition(|w| w == b"PK\x05\x06").map(|p| p + tail).ok_or("no end-of-central-directory record")?;
    if eocd + 22 > zip.len() { return Err("truncated end-of-central-directory record".into()); }
    let n = u16::from_le_bytes([zip[eocd + 10], zip[eocd + 11]]) as usize;
    let cd_size = u32::from_le_bytes(zip[eocd + 12..eocd + 16].try_into().unwrap()) as usize;
    let cd_off = u32::from_le_bytes(zip[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
    if cd_off + cd_size > zip.len() { return Err(format!("central directory [{cd_off}, {}) beyond the file ({} B)", cd_off + cd_size, zip.len())); }
    let mut out = Vec::new();
    let mut i = cd_off;
    for _ in 0..n {
        if i + 46 > zip.len() || &zip[i..i + 4] != b"PK\x01\x02" { return Err(format!("central directory entry at {i} is not PK\\x01\\x02")); }
        let method = u16::from_le_bytes([zip[i + 10], zip[i + 11]]);
        let csize = u32::from_le_bytes(zip[i + 20..i + 24].try_into().unwrap()) as usize;
        let usize_ = u32::from_le_bytes(zip[i + 24..i + 28].try_into().unwrap()) as usize;
        let nlen = u16::from_le_bytes([zip[i + 28], zip[i + 29]]) as usize;
        let elen = u16::from_le_bytes([zip[i + 30], zip[i + 31]]) as usize;
        let clen = u16::from_le_bytes([zip[i + 32], zip[i + 33]]) as usize;
        let lho = u32::from_le_bytes(zip[i + 42..i + 46].try_into().unwrap()) as usize;
        let name = String::from_utf8_lossy(&zip[i + 46..(i + 46 + nlen).min(zip.len())]).into_owned();
        i += 46 + nlen + elen + clen;
        let norm = name.replace('\\', "/");
        let lower = norm.to_ascii_lowercase();
        let Some(rest) = lower.strip_prefix("image/") else { continue };
        if !rest.ends_with(".dds") || rest.contains('/') { continue }
        let stem = norm["Image/".len()..norm.len() - ".dds".len()].to_string();
        // the local header (its own name/extra lengths locate the data)
        if lho + 30 > zip.len() || &zip[lho..lho + 4] != b"PK\x03\x04" { return Err(format!("{name}: local header at {lho} is not PK\\x03\\x04")); }
        let lnlen = u16::from_le_bytes([zip[lho + 26], zip[lho + 27]]) as usize;
        let lelen = u16::from_le_bytes([zip[lho + 28], zip[lho + 29]]) as usize;
        let start = lho + 30 + lnlen + lelen;
        if start + csize > zip.len() { return Err(format!("{name}: data [{start}, {}) beyond the file", start + csize)); }
        let data = match method {
            0 => zip[start..start + csize].to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec(&zip[start..start + csize]).map_err(|e| format!("{name}: deflate: {e:?}"))?,
            m => return Err(format!("{name}: zip method {m} (only stored / deflate)")),
        };
        if data.len() != usize_ { return Err(format!("{name}: inflated {} B, the directory says {usize_}", data.len())); }
        out.push((stem, Arc::new(data)));
    }
    out.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));
    Ok(out)
}

/// The images of a mod given as a zip file or as a directory holding `Image/*.dds` (or the `Image` directory itself).
pub fn load_files(path: &Path) -> Result<ModFiles, String> {
    if path.is_file() {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let images = zip_images(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        return Ok(ModFiles { source: path.display().to_string(), images });
    }
    let img_dir = if path.file_name().map(|n| n.eq_ignore_ascii_case("Image")).unwrap_or(false) { path.to_path_buf() } else { path.join("Image") };
    if !img_dir.is_dir() { return Err(format!("{}: neither a zip file nor a directory with Image/", path.display())); }
    let mut images = Vec::new();
    for e in std::fs::read_dir(&img_dir).map_err(|e| format!("{}: {e}", img_dir.display()))? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".dds").or_else(|| name.strip_suffix(".DDS")) else { continue };
        let bytes = std::fs::read(e.path()).map_err(|er| format!("{}: {er}", e.path().display()))?;
        images.push((stem.to_string(), Arc::new(bytes)));
    }
    images.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));
    Ok(ModFiles { source: img_dir.display().to_string(), images })
}

/// Every place the mod's files may be, in lookup order (see the module doc); the first that exists wins.
pub fn candidate_paths(r: &ModRef, map_path: &str) -> Vec<PathBuf> {
    fn bank(out: &mut Vec<PathBuf>, r: &ModRef, dir: &Path) {
        for name in [r.short.as_str(), r.mangled.as_str()] {
            out.push(dir.join(name).join("Image"));
            out.push(dir.join(name).join(format!("{name}.zip")));
            out.push(dir.join(name).join(format!("{}.zip", r.short)));
            out.push(dir.join(format!("{name}.zip")));
        }
    }
    let mut out: Vec<PathBuf> = Vec::new();
    if let Ok(v) = std::env::var("LMTOOL_MOD_DIR") {
        for d in v.split(':').filter(|d| !d.is_empty()) { bank(&mut out, r, Path::new(d)); }
    }
    // beside the map: the user's Trackmania folder (the ancestor holding Maps) → Skins\<Coll>\Mod\<mangled>.zip; the map's own folder
    let mp = Path::new(map_path);
    let dep_rel: Option<PathBuf> = r.dep_file.as_deref().map(|f| f.split(['\\', '/']).collect::<PathBuf>());
    let default_rel: PathBuf = ["Skins", r.collection.as_str(), "Mod", &format!("{}.zip", r.mangled)].iter().collect();
    let mut anc = mp.parent();
    while let Some(d) = anc {
        if d.join("Maps").is_dir() || d.join("Skins").is_dir() {
            if let Some(rel) = &dep_rel { out.push(d.join(rel)); }
            out.push(d.join(&default_rel));
        }
        anc = d.parent();
    }
    if let Some(d) = mp.parent() {
        bank(&mut out, r, d);
    }
    // the store's bank of fetched mods (RE 17: tiny/lightmap-re/mods/<short>/{<short>.zip, Image/})
    if let Ok(home) = std::env::var("HOME") {
        bank(&mut out, r, &Path::new(&home).join("persistent/private-30d/tm-player/tiny/lightmap-re/mods"));
    }
    out.dedup();
    out
}

/// One mounted image, for the census line.
#[derive(Debug, Clone)]
pub struct MountedImage {
    pub stem: String,
    pub logical: String,
    /// The mod file's (width, height, fourcc / format tag) from its DDS header.
    pub dims: Option<(u32, u32, String)>,
    pub bytes: usize,
}

/// The DDS header's (width, height, fourcc-or-format) for the census.
pub fn dds_dims(b: &[u8]) -> Option<(u32, u32, String)> {
    if b.len() < 128 || &b[0..4] != b"DDS " { return None; }
    let h = u32::from_le_bytes(b[12..16].try_into().ok()?);
    let w = u32::from_le_bytes(b[16..20].try_into().ok()?);
    let fourcc = &b[84..88];
    let tag = if fourcc == b"DX10" && b.len() >= 148 { format!("DXGI {}", u32::from_le_bytes(b[128..132].try_into().ok()?)) } else if fourcc.iter().all(|c| c.is_ascii_graphic()) { String::from_utf8_lossy(fourcc).into_owned() } else { format!("rgb{}", u32::from_le_bytes(b[88..92].try_into().ok()?)) };
    Some((w, h, tag))
}

/// The result of `mount_for_map`: what was mounted, or why nothing was.
pub struct MountReport {
    pub mod_ref: Option<ModRef>,
    pub source: Option<String>,
    pub mounted: Vec<MountedImage>,
    /// A setup WARNING (the header names a mod and no file was found; a file that would not read) — the bake is not the editor's.
    pub warning: Option<String>,
    pub note: String,
}

/// Mount the map's mod for this process (see the module doc). `LMTOOL_MOD=0` leaves the packs as they are;
/// `LMTOOL_MOD=<zip|dir>` mounts that file for the header's collection (the map's environment when the header names no mod).
pub fn mount_for_map(map_path: &str) -> MountReport {
    let knob = std::env::var("LMTOOL_MOD").ok();
    let mod_ref = match header_mod(map_path) {
        Ok(r) => r,
        Err(e) => return MountReport { mod_ref: None, source: None, mounted: Vec::new(), warning: None, note: format!("mod: header unreadable ({e}); no mod mounted") },
    };
    if knob.as_deref() == Some("0") {
        let _ = mapgeom::store::unmount_mod();
        return MountReport { note: format!("mod: LMTOOL_MOD=0 — {} (the packs' textures are used)", match &mod_ref { Some(r) => format!("the header's mod {} is NOT mounted", r.mangled), None => "the header names no mod".into() }), mod_ref, source: None, mounted: Vec::new(), warning: None };
    }
    let forced: Option<PathBuf> = knob.as_deref().filter(|k| !k.is_empty() && *k != "1").map(PathBuf::from);
    let (r, files) = match (&mod_ref, &forced) {
        (None, None) => {
            let _ = mapgeom::store::unmount_mod();
            return MountReport { mod_ref: None, source: None, mounted: Vec::new(), warning: None, note: "mod: none (the header's mod=\"\"; the packs' textures are the game's)".into() };
        }
        (_, Some(p)) => {
            let envir = tmmaps::header::read(map_path).map(|h| h.envir).unwrap_or_default();
            let r = mod_ref.clone().unwrap_or(ModRef { mangled: p.display().to_string(), dep_file: None, url: None, collection: envir, short: p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default() });
            match load_files(p) {
                Ok(f) => (r, f),
                Err(e) => return MountReport { mod_ref: Some(r), source: None, mounted: Vec::new(), warning: Some(format!("LMTOOL_MOD={}: {e} — no mod mounted", p.display())), note: String::new() },
            }
        }
        (Some(r), None) => {
            let cands = candidate_paths(r, map_path);
            let mut found = None;
            let mut errs: Vec<String> = Vec::new();
            for c in &cands {
                if !c.exists() { continue }
                match load_files(c) {
                    Ok(f) if !f.images.is_empty() => { found = Some(f); break; }
                    Ok(_) => errs.push(format!("{}: no Image/*.dds", c.display())),
                    Err(e) => errs.push(e),
                }
            }
            match found {
                Some(f) => (r.clone(), f),
                None => {
                    let _ = mapgeom::store::unmount_mod();
                    let w = format!(
                        "the header names the mod {} ({}) and its zip is nowhere (looked at {} places{}) — the editor bakes WITH it: every emitter binding a replaced texture emits the wrong colour in this bake; bank the zip under LMTOOL_MOD_DIR or beside the map (Skins\\{}\\Mod\\) or set LMTOOL_MOD=<zip>",
                        r.mangled,
                        r.url.clone().unwrap_or_else(|| "no url in the header".into()),
                        cands.len(),
                        if errs.is_empty() { String::new() } else { format!("; {}", errs.join("; ")) },
                        r.collection
                    );
                    return MountReport { mod_ref: Some(r.clone()), source: None, mounted: Vec::new(), warning: Some(w), note: String::new() };
                }
            }
        }
    };
    let mut map: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
    let mut mounted = Vec::new();
    for (stem, bytes) in &files.images {
        let logical = format!("{}\\Media\\Texture\\Image\\{stem}.dds", r.collection);
        map.insert(logical.to_uppercase(), Arc::clone(bytes));
        mounted.push(MountedImage { stem: stem.clone(), logical, dims: dds_dims(bytes), bytes: bytes.len() });
    }
    let _ = mapgeom::store::mount_mod(mapgeom::store::ModMount { name: r.mangled.clone(), files: map });
    let note = format!(
        "mod: {} MOUNTED over {}\\Media\\Texture\\Image\\ from {}: {} images ({})",
        r.mangled,
        r.collection,
        files.source,
        mounted.len(),
        mounted.iter().map(|m| match &m.dims { Some((w, h, t)) => format!("{} {w}×{h} {t}", m.stem), None => format!("{} ({} B, not a DDS)", m.stem, m.bytes) }).collect::<Vec<_>>().join(", ")
    );
    MountReport { mod_ref: Some(r), source: Some(files.source), mounted, warning: None, note }
}

/// Which of the mounted images shadow a texture the packs hold (the live overlays), as `(stem, pak dims)` — a mod image with no
/// pak counterpart replaces nothing (the game would still mount it; no material can bind it).
pub fn shadowed(store: &mapgeom::store::DataStore, mounted: &[MountedImage]) -> Vec<(String, Option<(u32, u32, String)>)> {
    let mut out = Vec::new();
    for m in mounted {
        if store.resolve(&m.logical).is_some() {
            out.push((m.stem.clone(), store.read_pack(&m.logical).ok().and_then(|b| dds_dims(&b))));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const G23_XML: &str = r#"<header type="map" exever="3.3.0" exebuild="2026-06-04_16_08" title="TMStadium" lightmap="0"><ident uid="x" name="g23" author="a" authorzone="World"/><desc envir="WhiteShore" mood="Day" type="Race" maptype="TrackMania\TM_Race" mapstyle="" validated="0" nblaps="0" displaycost="271451" mod="_nadeo-download-cdn-ubi-com_trackmania_assets_2026_Mod_NationsNORWAY" hasghostblocks="0" /><playermodel id=""/><times bronze="-1" silver="-1" gold="-1" authortime="-1" authorscore="0"/><deps><dep file="Skins\Stadium\LightColors\Off.dds"/><dep file="Skins\Stadium\ItemFlag\_nadeo-download-cdn-ubi-com_trackmania_assets_2026_ItemFlag_NationsNORWAY.dds" url="https://nadeo-download.cdn.ubi.com/trackmania/assets/2026/ItemFlag/NationsNORWAY.dds"/><dep file="Skins\Stadium\Mod\_nadeo-download-cdn-ubi-com_trackmania_assets_2026_Mod_NationsNORWAY.zip" url="https://nadeo-download.cdn.ubi.com/trackmania/assets/2026/Mod/NationsNORWAY.zip"/></deps></header>"#;

    #[test]
    fn g23_header_names_the_norway_mod_of_the_stadium_tree() {
        let r = mod_ref_from_xml(G23_XML).expect("a mod");
        assert_eq!(r.mangled, "_nadeo-download-cdn-ubi-com_trackmania_assets_2026_Mod_NationsNORWAY");
        assert_eq!(r.collection, "Stadium", "the dep path Skins\\Stadium\\Mod\\ names the tree the mod overlays, not the map's WhiteShore");
        assert_eq!(r.short, "NationsNORWAY");
        assert_eq!(r.url.as_deref(), Some("https://nadeo-download.cdn.ubi.com/trackmania/assets/2026/Mod/NationsNORWAY.zip"));
        assert!(r.dep_file.as_deref().unwrap().ends_with("_Mod_NationsNORWAY.zip"));
    }

    #[test]
    fn an_unmodded_header_has_no_mod() {
        assert_eq!(mod_ref_from_xml(r#"<header><desc envir="Stadium" mood="Day" mod="" /></header>"#), None);
        assert_eq!(mod_ref_from_xml(r#"<header><desc envir="Stadium" mood="Day" /></header>"#), None);
    }

    #[test]
    fn a_mod_without_a_dep_line_overlays_the_maps_own_collection() {
        let r = mod_ref_from_xml(r#"<header><desc envir="BlueBay" mood="Day" mod="MyMod" /><deps/></header>"#).unwrap();
        assert_eq!(r.collection, "BlueBay");
        assert_eq!(r.short, "MyMod");
    }

    /// A stored zip built by tmmaps' writer reads back through the central-directory walk: only Image/*.dds entries, stems kept.
    #[test]
    fn zip_images_reads_stored_and_deflated_entries() {
        let mut files = std::collections::BTreeMap::new();
        files.insert("Image/RoadTech_D.dds".to_string(), b"DDS fake".to_vec());
        files.insert("Image/RoadTech_D_HueMask.dds".to_string(), vec![7u8; 3000]);
        files.insert("Icon.dds".to_string(), b"not an image".to_vec());
        files.insert("Image/Sub/Deep.dds".to_string(), b"nested".to_vec());
        files.insert("Image/Readme.txt".to_string(), b"txt".to_vec());
        let stored = tmmaps::header::stored_zip(&files);
        let got = zip_images(&stored).unwrap();
        assert_eq!(got.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>(), vec!["RoadTech_D", "RoadTech_D_HueMask"]);
        assert_eq!(got[0].1.as_slice(), b"DDS fake");
        assert_eq!(got[1].1.len(), 3000);
        let deflated = tmmaps::header::deflated_zip(&files);
        let got2 = zip_images(&deflated).unwrap();
        assert_eq!(got2.len(), 2);
        assert_eq!(got2[1].1.as_slice(), &vec![7u8; 3000][..]);
    }

    /// The mount shadows a store's read for the mounted logical path and leaves every other path to the packs.
    #[test]
    fn a_mounted_image_is_what_a_store_reads() {
        let mut files = HashMap::new();
        files.insert("TESTMODCOLL\\MEDIA\\TEXTURE\\IMAGE\\FOO_D.DDS".to_string(), Arc::new(b"modded".to_vec()));
        let prev = mapgeom::store::mount_mod(mapgeom::store::ModMount { name: "test".into(), files });
        let mut st = mapgeom::store::DataStore::empty();
        assert_eq!(st.read("TestModColl\\Media\\Texture\\Image\\Foo_D.dds").unwrap().as_slice(), b"modded");
        assert!(st.read("TestModColl\\Media\\Texture\\Image\\Bar_D.dds").is_err());
        match prev { Some(m) => { let _ = mapgeom::store::mount_mod(mapgeom::store::ModMount { name: m.name.clone(), files: m.files.clone() }); } None => { let _ = mapgeom::store::unmount_mod(); } }
    }

    #[test]
    fn dds_dims_reads_the_header() {
        let mut b = vec![0u8; 128];
        b[0..4].copy_from_slice(b"DDS ");
        b[12..16].copy_from_slice(&1024u32.to_le_bytes());
        b[16..20].copy_from_slice(&4096u32.to_le_bytes());
        b[84..88].copy_from_slice(b"DXT5");
        assert_eq!(dds_dims(&b), Some((4096, 1024, "DXT5".into())));
    }
}
