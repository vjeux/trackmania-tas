//! `tinyctl upload FILE… [--record uploads.txt] [--meta BIN]`: put JPG/PNG
//! comparison sheets into the agentcloud attachment store (the only image
//! route the chat and the artifact panel render) and print one
//! `NAME<TAB>FILE_ID` row per file — the same rows `/tmp/tiny3/uploads.txt`
//! carries for maps 01–05, so a new session can rebuild the artifact.
//!
//! The upload is the intern GraphQL mutation `xfb_metamate_nest_bulk_file_upload`
//! run through the Meta CLI (`meta graphql.mutation execute … -V file:///…json`);
//! the variables file holds the image as base64. Each file goes in its own
//! mutation so one failure names one file.

use std::path::Path;

const MUTATION: &str = "mutation Up($inputs: [XFBMetamateNestFileUploadInput!]!) { xfb_metamate_nest_bulk_file_upload(inputs: $inputs) { uploaded_file_ids } }";

pub fn cmd(args: &[String]) -> Result<(), String> {
    let record = tmmaps::cli::flag(args, "--record").map(str::to_string);
    let meta = tmmaps::cli::flag(args, "--meta").unwrap_or("meta").to_string();
    let mut files: Vec<&String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--record" | "--meta" => i += 2,
            a if a.starts_with("--") => return Err(format!("unknown flag {a}")),
            _ => {
                files.push(&args[i]);
                i += 1;
            }
        }
    }
    if files.is_empty() {
        return Err("no files: tinyctl upload FILE… [--record uploads.txt]".into());
    }
    let mut rows = String::new();
    for f in files {
        let p = Path::new(f);
        let id = upload_one(&meta, p)?;
        let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or(f);
        println!("{name}\t{id}");
        rows.push_str(&format!("{name} {id}\n"));
    }
    if let Some(r) = record {
        use std::io::Write;
        let mut fh = std::fs::OpenOptions::new().append(true).create(true).open(&r).map_err(|e| format!("{r}: {e}"))?;
        fh.write_all(rows.as_bytes()).map_err(|e| format!("{r}: {e}"))?;
    }
    Ok(())
}

fn upload_one(meta: &str, p: &Path) -> Result<String, String> {
    let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let name = p.file_name().and_then(|s| s.to_str()).ok_or_else(|| format!("{}: not a file name", p.display()))?;
    let mime = match p.extension().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        other => return Err(format!("{}: unsupported extension {other:?}", p.display())),
    };
    let vars = format!("{{\"inputs\":[{{\"file_content\":\"{}\",\"file_content_type\":\"{mime}\",\"file_name\":\"{}\"}}]}}", base64(&bytes), json_escape(name));
    let tmp = std::env::temp_dir().join(format!("tinyctl-upload-{}-{}.json", std::process::id(), sanitize(name)));
    std::fs::write(&tmp, vars).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let out = std::process::Command::new(meta)
        .args(["graphql.mutation", "execute", "--schema=intern", "-q", MUTATION, "-V", &format!("file://{}", tmp.display()), "-o", "json"])
        .output()
        .map_err(|e| format!("{meta}: {e}"))?;
    let _ = std::fs::remove_file(&tmp);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    match file_id(&stdout) {
        Some(id) if out.status.success() => Ok(id),
        _ => Err(format!("{}: upload failed (exit {:?})\n{stdout}\n{stderr}", p.display(), out.status.code())),
    }
}

/// The first id after `"uploaded_file_ids"` in the CLI's JSON — a digit
/// string, quoted or bare.
fn file_id(json: &str) -> Option<String> {
    let at = json.find("uploaded_file_ids")?;
    let rest = &json[at + "uploaded_file_ids".len()..];
    let digits: String = rest.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    (!digits.is_empty()).then_some(digits)
}

fn sanitize(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect()
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Standard base64 with padding (RFC 4648), the alphabet the mutation reads.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_rfc4648() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foo"), "Zm9v");
        assert_eq!(super::base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn file_id_from_cli_json() {
        assert_eq!(super::file_id(r#"{"data":{"xfb_metamate_nest_bulk_file_upload":{"uploaded_file_ids":["1043183215220487"]}}}"#).as_deref(), Some("1043183215220487"));
        assert_eq!(super::file_id(r#"{"uploaded_file_ids": [2300609174103119]}"#).as_deref(), Some("2300609174103119"));
        assert_eq!(super::file_id(r#"{"errors":[]}"#), None);
    }
}
