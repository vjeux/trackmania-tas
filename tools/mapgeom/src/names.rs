//! Resolving a pack's hashed file names.
//!
//! Most of `dedicated_TMStadium.pak` is stored under real paths, but three
//! classes of asset are not — `CPlugPrefab`, `CPlugSurface` and `CPlugSolid`
//! live under 34 hex characters, e.g.
//! `Stadium\Media\Prefab\42F328BF947AC905A1D4FECB9A40E4C6F7`. Those files hold
//! the meshes, so without this module the geometry is unreachable by name.
//!
//! The hash is 17 bytes rendered as hex: **the byte length of the (lowercased,
//! UTF-8) path, then MD5 of it** — and rendered **low nibble first**, which is
//! not how anybody writes hex. Both details matter and both are easy to miss:
//!
//! * An earlier attempt in this project wrote the leading byte as `0x00`
//!   (following an older `MD5.Compute136`, whose 136-bit output is
//!   `0x00 ++ md5`). Every hash it computed began `00`, no pack entry does, and
//!   the conclusion drawn was that the naming scheme was unresolved. It was the
//!   length byte, and the length byte is why the real names begin with wildly
//!   different pairs.
//! * The nibble swap turns a correct MD5 into a string that *looks* like a
//!   plausible wrong hash, so it fails the same way a wrong algorithm does.
//!
//! What is hashed is a SUFFIX of the path, and the entry lives under the
//! remaining prefix: `A\B\C\name` may be stored as `A\B\C\<h(name)>`,
//! `A\B\<h(C\name)>`, `A\<h(B\C\name)>` or `<h(A\B\C\name)>`. So resolution
//! walks the split points, which is cheap and exact.

use crate::md5::md5;

/// `MD5.Compute136` as TM2020 spells it: length byte ++ md5, hex with the low
/// nibble of each byte written first.
pub fn compute136(text: &str) -> String {
    let lowered = text.to_lowercase();
    let bytes = lowered.as_bytes();
    let mut h = [0u8; 17];
    h[0] = bytes.len() as u8;
    h[1..].copy_from_slice(&md5(bytes));
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut s = String::with_capacity(34);
    for b in h {
        s.push(HEX[(b % 16) as usize] as char);
        s.push(HEX[(b / 16) as usize] as char);
    }
    s
}

/// Every pack path a logical path could be stored under, cheapest first.
///
/// Split points only; the trailing component is always part of the hashed
/// suffix, because a directory is never hashed on its own.
pub fn candidates(path: &str) -> Vec<String> {
    let clean = normalise(path);
    let parts: Vec<&str> = clean.split('\\').collect();
    let mut out = Vec::with_capacity(parts.len() + 1);
    out.push(clean.clone());
    for cut in (0..parts.len()).rev() {
        let dir = parts[..cut].join("\\");
        let tail = parts[cut..].join("\\");
        let h = compute136(&tail);
        out.push(if dir.is_empty() { h } else { format!("{}\\{}", dir, h) });
    }
    out
}

/// Collapse `..` and `.`, and normalise separators, the way the game resolves a
/// reference-table path against the referring file's folder.
pub fn normalise(path: &str) -> String {
    let unified = path.replace('/', "\\");
    let mut stack: Vec<&str> = Vec::new();
    for part in unified.split('\\') {
        match part {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            p => stack.push(p),
        }
    }
    stack.join("\\")
}

/// Join a referring file's folder with a reference-table path.
pub fn join(folder: &str, rel: &str) -> String {
    if folder.is_empty() {
        normalise(rel)
    } else {
        normalise(&format!("{}\\{}", folder, rel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two properties the hash has to have, pinned against the shape of a
    /// real pack entry rather than against our own output: 34 hex characters,
    /// and a first byte that is the path's LENGTH — so two paths of different
    /// lengths cannot share a leading pair, and no hash may begin `00`.
    #[test]
    fn compute136_shape() {
        let h = compute136("TiltCurve3_Air.Prefab.Gbx");
        assert_eq!(h.len(), 34);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        // 25 bytes = 0x19, written low nibble first -> "91".
        assert_eq!(&h[..2], "91");
        // Case folds: the game lowercases before hashing.
        assert_eq!(compute136("TILTCURVE3_AIR.PREFAB.GBX"), h);
    }

    /// The nibble swap is the detail that silently produces a wrong-looking
    /// right answer, so pin it directly: MD5("") is d41d8cd98f00b204e9800998
    /// ecf8427e, and an empty path has length 0.
    #[test]
    fn nibble_order_is_low_first() {
        let h = compute136("");
        assert_eq!(h, "004DD1C89DF8002B409E089089CE8F24E7");
    }

    #[test]
    fn candidates_walk_the_split_points() {
        let c = candidates("A\\B\\name.Gbx");
        assert_eq!(c[0], "A\\B\\name.Gbx");
        assert_eq!(c[1], format!("A\\B\\{}", compute136("name.Gbx")));
        assert_eq!(c[2], format!("A\\{}", compute136("B\\name.Gbx")));
        assert_eq!(c[3], compute136("A\\B\\name.Gbx"));
        assert_eq!(c.len(), 4);
    }

    #[test]
    fn normalise_collapses_ancestors() {
        assert_eq!(normalise("Stadium\\X\\..\\Y\\a.Gbx"), "Stadium\\Y\\a.Gbx");
        assert_eq!(join("Stadium\\GameCtnBlockInfo", "..\\Media\\P.Gbx"), "Stadium\\Media\\P.Gbx");
    }
}
