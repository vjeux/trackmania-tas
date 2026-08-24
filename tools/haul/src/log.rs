//! Append-only, writer-sharded logs.
//!
//! Two properties matter and they are both about surviving months rather than
//! hours:
//!
//! 1. **Append-only.** A record is never rewritten, so a half-finished write
//!    can lose the tail of a file but can never turn an earlier true statement
//!    into a false one.
//! 2. **Sharded by writer.** Each box×process writes `<node>-<start>.rec` and
//!    nothing else. Two boxes committing at the same time touch disjoint
//!    files, so the git merge is a directory union and never a conflict. The
//!    logical log is the concatenation of every shard, sorted by timestamp.
//!
//! The same shape is used for the journal (what happened), the ledger (what we
//! tried and what it produced), and the alarm log (what fired).

use crate::rec::Rec;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Log {
    path: PathBuf,
}

impl Log {
    /// Open (creating) this process's shard in `dir`.
    pub fn shard(dir: &Path, node: &str, start: i64) -> std::io::Result<Log> {
        std::fs::create_dir_all(dir)?;
        Ok(Log { path: dir.join(format!("{node}-{start}.rec")) })
    }

    pub fn at(path: impl Into<PathBuf>) -> Log {
        Log { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, r: &Rec) -> std::io::Result<()> {
        if let Some(p) = self.path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        writeln!(f, "{}", r.render())?;
        f.flush()?;
        Ok(())
    }
}

/// Read every shard in `dir`, in timestamp order.
///
/// A shard that fails to parse is an error, loudly: this project's signature
/// failure is an instrument that returns an empty, clean-looking answer when
/// something is actually broken, and "the journal is empty" must never be one
/// of the ways a corrupt file can present.
pub fn read_all(dir: &Path) -> Result<Vec<Rec>, String> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("read_dir {dir:?}: {e}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "rec").unwrap_or(false))
        .collect();
    entries.sort();
    for p in entries {
        let text = std::fs::read_to_string(&p).map_err(|e| format!("read {p:?}: {e}"))?;
        let recs = Rec::parse_all(&text).map_err(|e| format!("{}: {e}", p.display()))?;
        out.extend(recs);
    }
    out.sort_by_key(|r| r.ts);
    Ok(out)
}

/// Records of one kind, oldest first.
pub fn of_kind<'a>(recs: &'a [Rec], kind: &str) -> Vec<&'a Rec> {
    recs.iter().filter(|r| r.kind == kind).collect()
}

pub fn last_of_kind<'a>(recs: &'a [Rec], kind: &str) -> Option<&'a Rec> {
    recs.iter().rev().find(|r| r.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("haul-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn two_writers_do_not_share_a_file_and_the_union_is_ordered() {
        let d = tmpdir("shards");
        let a = Log::shard(&d, "boxA", 100).unwrap();
        let b = Log::shard(&d, "boxB", 100).unwrap();
        assert_ne!(a.path(), b.path(), "two boxes must never append to one file");
        a.append(&Rec::at(30, "sample").f("who", "a")).unwrap();
        b.append(&Rec::at(10, "sample").f("who", "b")).unwrap();
        a.append(&Rec::at(20, "sample").f("who", "a")).unwrap();
        let all = read_all(&d).unwrap();
        assert_eq!(
            all.iter().map(|r| r.ts).collect::<Vec<_>>(),
            vec![10, 20, 30],
            "the logical log is every shard in timestamp order"
        );
    }

    #[test]
    fn a_corrupt_shard_is_an_error_not_an_empty_read() {
        let d = tmpdir("corrupt");
        Log::shard(&d, "boxA", 1).unwrap().append(&Rec::at(1, "ok")).unwrap();
        std::fs::write(d.join("boxB-1.rec"), "this is not a record\n").unwrap();
        let e = read_all(&d).unwrap_err();
        assert!(e.contains("boxB-1.rec"), "the error must name the bad shard: {e}");
    }

    #[test]
    fn a_missing_directory_reads_as_empty_and_that_is_fine() {
        // Distinct from the case above on purpose: nothing written yet is a
        // legitimate empty; a file that will not parse is not.
        let d = tmpdir("absent").join("never-created");
        assert!(read_all(&d).unwrap().is_empty());
    }
}
