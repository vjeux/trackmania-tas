//! The NO-GHOST GATE: the oracle driver refuses to load any input file that is
//! not chain-rooted at a container this system synthesized.
//!
//! # What "chain-rooted" means, and why it is a walk and not a flag
//!
//! Every tape we write carries a [`Prov`] record naming its producer and its
//! parent. A tape is **chain-rooted** iff following those parents terminates at
//! a record whose producer is [`Producer::Synthesizer`] and which has no
//! parent. Nothing is trusted because it says it is trustworthy: a record
//! claiming `producer = synthesizer` while also naming a parent is refused, and
//! a record naming a parent we have never seen is refused.
//!
//! A human's `.Ghost.Gbx` has no `PROV` record at all, so it is refused by the
//! first condition, which is the point.
//!
//! # The two properties this file is built around
//!
//! **It fails closed.** Every path out of [`Gate::admit`] that is not a
//! successful walk to a root is a refusal. There is no `.ok()?` in here and
//! there is no default-allow: an unreadable ledger, a malformed record, a
//! cycle, a depth overrun and an absent record are all *refusals*, each with
//! its own reason. `.ok()?` is `2>/dev/null` with a nicer spelling, and an
//! instrument that fails toward *clean* produces nothing to be suspicious of.
//!
//! **It reads both operands from the world.** The subject is the file's own
//! bytes: the gate hashes the file it was handed and looks that hash up. It
//! never takes a hash, a producer or a verdict from its caller. A check that
//! compares two things the command line supplied has not run.
//!
//! # Its control is two-sided, and neither half is optional
//!
//! A detector that never says no is decoration; so is one that never says yes.
//! `tmauto gate selftest` hands the gate a real human recording from the
//! quarantine directory (must **refuse**) and one of our own chain-rooted tapes
//! (must **accept**), in the same batch, and fails if either half is missing.

use crate::sha::sha256_file;
use crate::tape::{Producer, Prov};
use crate::verdict::TapeHash;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Why the gate refused. Every one of these is a refusal; none is a warning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The file is not in the provenance ledger at all. This is what a human
    /// recording looks like.
    NoProvenanceRecord { file_sha: String },
    /// The file is registered, but the tape it names has no record.
    UnknownTape { tape: TapeHash },
    /// A parent in the chain has no record — the chain is broken, so it
    /// terminates nowhere.
    BrokenChain { at: TapeHash, missing_parent: TapeHash },
    /// The chain terminates at a producer that does not root chains.
    NotRootedAtSynthesizer { at: TapeHash, producer: Producer },
    /// A record claims to be a root but names a parent, or claims a parent but
    /// is a synthesizer record. Self-contradictory: refused rather than
    /// interpreted.
    ContradictoryRecord { at: TapeHash },
    /// The walk revisited a tape. A cycle terminates at nothing.
    Cycle { at: TapeHash },
    /// The walk ran past the depth limit without reaching a root.
    TooDeep { depth: usize },
    /// The ledger itself could not be read. **Refused, not skipped** — an
    /// instrument that cannot see is not an instrument that saw nothing wrong.
    LedgerUnreadable { path: String, err: String },
    /// The file could not be hashed.
    FileUnreadable { path: String, err: String },
    /// The file is outside the clean workspace.
    OutsideCleanWorkspace { path: String },
}

impl Refusal {
    pub fn reason(&self) -> String {
        match self {
            Refusal::NoProvenanceRecord { file_sha } => format!(
                "no PROV record for this file (sha256 {}). A file this system did not \
                 write has no provenance and is never loaded.",
                &file_sha[..16]
            ),
            Refusal::UnknownTape { tape } => {
                format!("the file names tape {} and no PROV record exists for it", tape)
            }
            Refusal::BrokenChain { at, missing_parent } => format!(
                "tape {} names parent {} and no PROV record exists for that parent",
                at, missing_parent
            ),
            Refusal::NotRootedAtSynthesizer { at, producer } => format!(
                "the chain terminates at tape {} produced by '{}', which does not root a \
                 chain; only a synthesized container does",
                at,
                producer.as_str()
            ),
            Refusal::ContradictoryRecord { at } => format!(
                "the PROV record for tape {} is self-contradictory (a synthesizer record \
                 with a parent, or a non-synthesizer record without one)",
                at
            ),
            Refusal::Cycle { at } => format!("the provenance chain revisits tape {}", at),
            Refusal::TooDeep { depth } => {
                format!("the provenance chain is deeper than {} without reaching a root", depth)
            }
            Refusal::LedgerUnreadable { path, err } => {
                format!("the provenance ledger {} could not be read: {}", path, err)
            }
            Refusal::FileUnreadable { path, err } => {
                format!("{} could not be read: {}", path, err)
            }
            Refusal::OutsideCleanWorkspace { path } => {
                format!("{} is outside the clean workspace", path)
            }
        }
    }
}

/// The deepest provenance chain the gate will walk. A search that appends one
/// tape per macro could legitimately build a long lineage; this bound exists so
/// a malformed ledger cannot spin, not to limit real work.
pub const MAX_CHAIN_DEPTH: usize = 1_000_000;

/// The provenance ledger: append-only, one record per line, human-greppable.
///
/// Two tables in one file, distinguished by their leading tag:
///
/// ```text
/// PROV  1  <producer> <parent|-> <seed> <ts> <map_uid>      # keyed by tape hash
/// TAPE  1  <tape_hash>  <prov-line...>
/// FILE  1  <file_sha256>  <tape_hash>  <path>
/// ```
pub struct Ledger {
    path: PathBuf,
    /// tape hash -> its record
    tapes: HashMap<TapeHash, Prov>,
    /// file sha256 -> the tape it holds
    files: HashMap<String, TapeHash>,
}

impl Ledger {
    pub fn path_in(clean_root: &Path) -> PathBuf {
        clean_root.join("prov").join("ledger.tsv")
    }

    /// Load the ledger. A missing ledger is an EMPTY ledger, which admits
    /// nothing — that is a fail-closed state, not an error. A ledger that
    /// exists and cannot be parsed is an error and every admit refuses.
    pub fn load(path: &Path) -> Result<Ledger, Refusal> {
        let mut l = Ledger { path: path.to_path_buf(), tapes: HashMap::new(), files: HashMap::new() };
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(l),
            Err(e) => {
                return Err(Refusal::LedgerUnreadable {
                    path: path.display().to_string(),
                    err: e.to_string(),
                })
            }
        };
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            match f.first().copied() {
                Some("TAPE") => {
                    // TAPE \t 1 \t <hash> \t PROV \t 1 \t ...
                    let hash = f
                        .get(2)
                        .and_then(|h| TapeHash::from_hex(h))
                        .ok_or_else(|| Ledger::bad(path, n, "unparseable tape hash"))?;
                    let prov = Prov::parse_line(&f[3..].join("\t"))
                        .ok_or_else(|| Ledger::bad(path, n, "unparseable PROV record"))?;
                    l.tapes.insert(hash, prov);
                }
                Some("FILE") => {
                    let sha = f
                        .get(2)
                        .ok_or_else(|| Ledger::bad(path, n, "FILE row has no sha"))?
                        .to_string();
                    let hash = f
                        .get(3)
                        .and_then(|h| TapeHash::from_hex(h))
                        .ok_or_else(|| Ledger::bad(path, n, "FILE row has no tape hash"))?;
                    l.files.insert(sha, hash);
                }
                _ => return Err(Ledger::bad(path, n, "unknown row tag")),
            }
        }
        Ok(l)
    }

    fn bad(path: &Path, line: usize, what: &str) -> Refusal {
        Refusal::LedgerUnreadable {
            path: path.display().to_string(),
            err: format!("line {}: {}", line + 1, what),
        }
    }

    pub fn tape_count(&self) -> usize {
        self.tapes.len()
    }
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Record a tape's provenance.
    pub fn record_tape(&mut self, hash: TapeHash, prov: &Prov) -> std::io::Result<()> {
        self.append(&format!("TAPE\t1\t{}\t{}", hash.hex(), prov.to_line()))?;
        self.tapes.insert(hash, prov.clone());
        Ok(())
    }

    /// Record that a written container file holds a tape.
    pub fn record_file(&mut self, file_sha: &str, tape: TapeHash, path: &Path) -> std::io::Result<()> {
        self.append(&format!("FILE\t1\t{}\t{}\t{}", file_sha, tape.hex(), path.display()))?;
        self.files.insert(file_sha.to_string(), tape);
        Ok(())
    }

    fn append(&self, line: &str) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(p) = self.path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        writeln!(f, "{}", line)
    }
}

/// The gate itself.
pub struct Gate {
    pub clean_root: PathBuf,
    pub ledger: Ledger,
    /// Where refusals are logged. A refusal that is not logged did not happen
    /// as far as an audit is concerned.
    pub refusal_log: PathBuf,
}

/// What the gate decided, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Chain-rooted at a synthesized container, `depth` links back.
    Admit { tape: TapeHash, depth: usize },
    Refuse(Refusal),
}

impl Decision {
    pub fn is_admit(&self) -> bool {
        matches!(self, Decision::Admit { .. })
    }
}

impl Gate {
    pub fn open(clean_root: &Path) -> Result<Gate, Refusal> {
        let ledger = Ledger::load(&Ledger::path_in(clean_root))?;
        Ok(Gate {
            clean_root: clean_root.to_path_buf(),
            ledger,
            refusal_log: clean_root.join("prov").join("refusals.tsv"),
        })
    }

    /// May the oracle driver load this file?
    ///
    /// The subject is the file's BYTES. Nothing the caller says about the file
    /// is consulted, so a caller cannot talk its way past the gate.
    pub fn admit(&self, path: &Path) -> Decision {
        let d = self.decide(path);
        if let Decision::Refuse(r) = &d {
            self.log_refusal(path, r);
        }
        d
    }

    fn decide(&self, path: &Path) -> Decision {
        let sha = match sha256_file(path) {
            Ok(h) => h.iter().map(|b| format!("{:02x}", b)).collect::<String>(),
            Err(e) => {
                return Decision::Refuse(Refusal::FileUnreadable {
                    path: path.display().to_string(),
                    err: e.to_string(),
                })
            }
        };
        let tape = match self.ledger.files.get(&sha) {
            Some(t) => *t,
            None => return Decision::Refuse(Refusal::NoProvenanceRecord { file_sha: sha }),
        };
        match self.walk_to_root(tape) {
            Ok(depth) => Decision::Admit { tape, depth },
            Err(r) => Decision::Refuse(r),
        }
    }

    /// Follow parents to a root. Returns the number of links walked.
    pub fn walk_to_root(&self, start: TapeHash) -> Result<usize, Refusal> {
        let mut seen = std::collections::HashSet::new();
        let mut cur = start;
        let mut depth = 0usize;
        loop {
            if !seen.insert(cur) {
                return Err(Refusal::Cycle { at: cur });
            }
            if depth > MAX_CHAIN_DEPTH {
                return Err(Refusal::TooDeep { depth });
            }
            let rec = match self.ledger.tapes.get(&cur) {
                Some(r) => r,
                None if depth == 0 => return Err(Refusal::UnknownTape { tape: cur }),
                None => {
                    // unreachable in practice: the parent link is checked below
                    return Err(Refusal::UnknownTape { tape: cur });
                }
            };
            // A record must be internally consistent before it is followed.
            if rec.producer.is_root() != rec.parent.is_none() {
                return Err(Refusal::ContradictoryRecord { at: cur });
            }
            match rec.parent {
                None => {
                    return if rec.producer.is_root() {
                        Ok(depth)
                    } else {
                        Err(Refusal::NotRootedAtSynthesizer { at: cur, producer: rec.producer })
                    }
                }
                Some(p) => {
                    if !self.ledger.tapes.contains_key(&p) {
                        return Err(Refusal::BrokenChain { at: cur, missing_parent: p });
                    }
                    cur = p;
                    depth += 1;
                }
            }
        }
    }

    fn log_refusal(&self, path: &Path, r: &Refusal) {
        use std::io::Write;
        if let Some(p) = self.refusal_log.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        // A failure to LOG a refusal must be loud: the refusal still stands,
        // but an audit that cannot see it is an audit that reports clean.
        match std::fs::OpenOptions::new().create(true).append(true).open(&self.refusal_log) {
            Ok(mut f) => {
                let _ = writeln!(
                    f,
                    "{}\tREFUSED\t{}\t{}",
                    crate::tape::now_unix(),
                    path.display(),
                    r.reason()
                );
            }
            Err(e) => {
                eprintln!(
                    "tmauto gate: REFUSED {} ({}) AND could not write the refusal log {}: {}",
                    path.display(),
                    r.reason(),
                    self.refusal_log.display(),
                    e
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tape::{Input, Tape};

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "tmauto-gate-test-{}-{}-{}",
            std::process::id(),
            tag,
            crate::tape::now_unix()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Write `bytes` to a file and register it as holding `tape`.
    fn register(g: &mut Gate, name: &str, bytes: &[u8], tape: &Tape) -> PathBuf {
        let p = g.clean_root.join(name);
        std::fs::write(&p, bytes).unwrap();
        let sha = crate::sha::sha256_hex(bytes);
        g.ledger.record_tape(tape.hash(), &tape.prov).unwrap();
        g.ledger.record_file(&sha, tape.hash(), &p).unwrap();
        p
    }

    #[test]
    fn a_rooted_tape_is_admitted_and_an_unregistered_file_is_refused() {
        let d = tmpdir("both");
        let mut g = Gate::open(&d).unwrap();
        let root = Tape::new(vec![Input::FULL_GAS; 4], Prov::root(1, "M"));
        let p = register(&mut g, "ours.Ghost.Gbx", b"our synthesized bytes", &root);

        // POSITIVE half
        assert!(g.admit(&p).is_admit(), "our own chain-rooted tape must be admitted");

        // NEGATIVE half, in the same test: a file nobody registered.
        let stranger = d.join("stranger.Ghost.Gbx");
        std::fs::write(&stranger, b"somebody else's recording").unwrap();
        match g.admit(&stranger) {
            Decision::Refuse(Refusal::NoProvenanceRecord { .. }) => {}
            other => panic!("an unregistered file must be refused, got {:?}", other),
        }
    }

    #[test]
    fn a_chain_is_walked_to_its_root() {
        let d = tmpdir("chain");
        let mut g = Gate::open(&d).unwrap();
        let root = Tape::new(vec![Input::FULL_GAS; 4], Prov::root(1, "M"));
        g.ledger.record_tape(root.hash(), &root.prov).unwrap();
        let mut prev = root.hash();
        let mut last = None;
        for k in 1..=5u32 {
            let t = Tape::new(
                vec![Input::new(k as i8, true, false); 4 + k as usize],
                Prov::child(Producer::Explorer, prev, k as u64, "M"),
            );
            g.ledger.record_tape(t.hash(), &t.prov).unwrap();
            prev = t.hash();
            last = Some(t);
        }
        let t = last.unwrap();
        let p = g.clean_root.join("deep.Ghost.Gbx");
        std::fs::write(&p, b"deep").unwrap();
        g.ledger.record_file(&crate::sha::sha256_hex(b"deep"), t.hash(), &p).unwrap();
        assert_eq!(g.admit(&p), Decision::Admit { tape: t.hash(), depth: 5 });
    }

    /// A chain that terminates at a producer other than the synthesizer is
    /// refused. This is the case that matters most: a tape whose ancestry runs
    /// back to something we did not manufacture.
    #[test]
    fn a_chain_rooted_elsewhere_is_refused() {
        let d = tmpdir("badroot");
        let mut g = Gate::open(&d).unwrap();
        // a "root" that is not the synthesizer: no parent, wrong producer
        let orphan = Tape::new(
            vec![Input::FULL_GAS; 3],
            Prov {
                producer: Producer::Explorer,
                parent: None,
                seed: 0,
                timestamp_unix: 0,
                map_uid: "M".into(),
            },
        );
        let p = register(&mut g, "orphan.Ghost.Gbx", b"orphan", &orphan);
        match g.admit(&p) {
            Decision::Refuse(Refusal::ContradictoryRecord { .. }) => {}
            other => panic!("expected a refusal, got {:?}", other),
        }
    }

    #[test]
    fn a_broken_chain_is_refused() {
        let d = tmpdir("broken");
        let mut g = Gate::open(&d).unwrap();
        let child = Tape::new(
            vec![Input::FULL_GAS; 3],
            Prov::child(Producer::Explorer, TapeHash([0xEE; 32]), 0, "M"),
        );
        let p = register(&mut g, "orphaned.Ghost.Gbx", b"orphaned child", &child);
        match g.admit(&p) {
            Decision::Refuse(Refusal::BrokenChain { .. }) => {}
            other => panic!("expected a broken-chain refusal, got {:?}", other),
        }
    }

    /// Editing an admitted file by one byte must make it a stranger again —
    /// the gate's subject is the bytes, not the path.
    #[test]
    fn one_changed_byte_is_a_different_file() {
        let d = tmpdir("bytes");
        let mut g = Gate::open(&d).unwrap();
        let root = Tape::new(vec![Input::FULL_GAS; 4], Prov::root(1, "M"));
        let p = register(&mut g, "ours.Ghost.Gbx", b"our synthesized bytes", &root);
        assert!(g.admit(&p).is_admit());
        std::fs::write(&p, b"our synthesized byteS").unwrap();
        assert!(!g.admit(&p).is_admit(), "a modified file must lose its admission");
    }

    /// A refusal must reach the log. An unlogged refusal is invisible to the
    /// audit, which is how a gate quietly stops mattering.
    #[test]
    fn refusals_are_logged() {
        let d = tmpdir("log");
        let g = Gate::open(&d).unwrap();
        let stranger = d.join("x.Ghost.Gbx");
        std::fs::write(&stranger, b"stranger").unwrap();
        assert!(!g.admit(&stranger).is_admit());
        let log = std::fs::read_to_string(&g.refusal_log).expect("a refusal log must exist");
        assert!(log.contains("REFUSED"), "log was: {}", log);
        assert!(log.contains("x.Ghost.Gbx"));
    }

    /// An unreadable ledger refuses everything. The alternative — treating an
    /// unreadable ledger as an empty one and carrying on — is an instrument
    /// failing toward clean.
    #[test]
    fn a_corrupt_ledger_refuses_rather_than_admits() {
        let d = tmpdir("corrupt");
        std::fs::create_dir_all(d.join("prov")).unwrap();
        std::fs::write(Ledger::path_in(&d), "GARBAGE\tnot a row\n").unwrap();
        match Gate::open(&d) {
            Err(Refusal::LedgerUnreadable { .. }) => {}
            other => panic!("a corrupt ledger must refuse, got {:?}", other.map(|_| "opened")),
        }
    }
}
