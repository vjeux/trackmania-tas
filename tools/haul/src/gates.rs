//! **The acceptance gates.** What a worker must hand over before the harness
//! will bank a claimed result as a result.
//!
//! Set 2026-08-24 by the lead, after an explorer produced five `confirmed`
//! tapes that could not be reproduced: their run began at the wrong position
//! (or the state locator tracked the wrong object), and the replay frame
//! needed to re-derive them was not recorded. Nothing here is about that
//! explorer — these are general, and they apply to every worker this harness
//! will ever supervise, including ones nobody has written yet.
//!
//! | gate | what it requires | why |
//! |---|---|---|
//! | **G1 reproducible** | frame/prefix, and hashes of the map and the template | a tape is not a result if nobody can re-derive it. The 153-tick frame error is a real defect this project has already paid for: a search's tick 0 is the fork's resume boundary, not the file's |
//! | **G2 live tick 0** | a driving worker reports the car's state at tick 0 from the LIVE run | telemetry in a container may be the template's; only the running engine knows where the car actually was |
//! | **G3 fail closed** | no start-position control ⇒ **refused**, never accepted | the difference between "checked and fine" and "not checked" must never collapse into a pass |
//! | **G4 transcript** | the oracle's own raw output, banked beside the claim | a number in a report is a claim; the engine's bytes are evidence, and they let a later reader re-judge without re-running |
//!
//! Every gate is a pure function over a `Claim`, so its test needs no engine —
//! and every gate has a test that REFUSES and a test that ACCEPTS. A gate
//! nobody has watched refuse is decoration; a gate that refuses everything is
//! worse, because it gets switched off.

use crate::rec::Rec;

/// What a worker hands the harness when it says it has produced something.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Claim {
    /// What is being claimed, for the human: "cps 3 on Summer 2026 - 01".
    pub what: String,
    /// The written tape's own hash. The artifact the claim is about.
    pub tape_md5: String,

    // ---- G1: could somebody else re-derive this?
    /// The tick the tape's frame starts at, in the run's own numbering.
    /// `None` is a refusal: a search's tick 0 is routinely NOT the file's.
    pub frame_start_tick: Option<i64>,
    /// The hash of the prefix this run was branched from, if it was. `None`
    /// means "from the root", which is a legitimate answer and must be stated
    /// rather than left blank.
    pub prefix: Option<Prefix>,
    /// The map file this ran on, by hash — matched against the registry.
    pub map_md5: String,
    /// The container the tape was written into, by hash.
    pub template_md5: String,

    // ---- G2/G3: where did the car actually start?
    /// Tick-0 position read from the LIVE run, not from a container's
    /// telemetry. `None` fails G2 for a driving worker and G3 always.
    pub live_tick0: Option<Tick0>,
    /// Does this worker drive? A sweep over written tapes does not.
    pub drives: bool,

    // ---- G4: the engine's own words
    /// The oracle's raw output for this claim, verbatim.
    pub oracle_transcript: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tick0 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Metres from the map's declared start line, horizontally. `None` means
    /// the comparison could not be made — which G3 refuses.
    pub dev_from_spawn_m: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Prefix {
    /// The hash of the tape this branched from.
    pub tape_md5: String,
    /// The tick the branch was taken at.
    pub at_tick: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// Metres of horizontal deviation from the declared spawn that still count
    /// as "started at the start line".
    pub start_dev_max_m: i64,
    /// Shortest oracle transcript that could plausibly be one. A transcript
    /// field holding `""` or `"ok"` satisfies "is it present?" and nothing
    /// else, which is the decoration this whole module exists to prevent.
    pub min_transcript_bytes: usize,
}

impl Default for Policy {
    fn default() -> Self {
        Policy { start_dev_max_m: 32, min_transcript_bytes: 40 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub gate: &'static str,
    pub why: String,
}

pub const GATES: &[&str] = &["G1-reproducible", "G2-live-tick0", "G3-start-control", "G4-transcript"];

/// G1 — could somebody else re-derive this artifact?
pub fn g1_reproducible(c: &Claim) -> Option<Refusal> {
    let mut missing = Vec::new();
    if c.frame_start_tick.is_none() {
        missing.push(
            "frame_start_tick (a search's tick 0 is the fork's resume boundary, not the file's — \
             this project has already shipped a tape 153 ticks out of frame)",
        );
    }
    if c.map_md5.len() != 32 {
        missing.push("map_md5 (which map was this measured on?)");
    }
    if c.template_md5.len() != 32 {
        missing.push("template_md5 (which container carries it?)");
    }
    if c.tape_md5.len() != 32 {
        missing.push("tape_md5 (what artifact is the claim about?)");
    }
    if missing.is_empty() {
        return None;
    }
    Some(Refusal {
        gate: "G1-reproducible",
        why: format!("a claim nobody can re-derive is not a result — missing: {}", missing.join("; ")),
    })
}

/// G2 — a driving worker reports tick 0 from the live run.
pub fn g2_live_tick0(c: &Claim) -> Option<Refusal> {
    if !c.drives {
        return None;
    }
    if c.live_tick0.is_some() {
        return None;
    }
    Some(Refusal {
        gate: "G2-live-tick0",
        why: "a driving worker must report the car's tick-0 state from the LIVE run. A \
              container's telemetry is not a substitute: a synthesised tape carries its \
              template's telemetry, so reading the file can describe a run nobody made"
            .to_string(),
    })
}

/// G3 — no start-position control means refused, never accepted.
///
/// The distinction this enforces: *checked and fine* and *not checked* must
/// never produce the same outcome. A worker that cannot supply the control
/// gets a refusal it can read, not a silent pass.
pub fn g3_start_control(c: &Claim, p: &Policy) -> Option<Refusal> {
    let dev = match c.live_tick0.and_then(|t| t.dev_from_spawn_m) {
        Some(d) => d,
        None => {
            return Some(Refusal {
                gate: "G3-start-control",
                why: "no start-position control: nothing here can tell whether this run drove \
                      the map from its start line, and 'not checked' is refused rather than \
                      folded in with 'fine'"
                    .to_string(),
            })
        }
    };
    if dev <= p.start_dev_max_m as f64 {
        return None;
    }
    Some(Refusal {
        gate: "G3-start-control",
        why: format!(
            "the car started {dev:.1} m from the map's declared start line (tolerance {} m) — \
             whatever this run is, it is not a run of the map from the beginning",
            p.start_dev_max_m
        ),
    })
}

/// G4 — the oracle's own transcript is banked beside the claim.
pub fn g4_transcript(c: &Claim, p: &Policy) -> Option<Refusal> {
    match &c.oracle_transcript {
        Some(t) if t.len() >= p.min_transcript_bytes => None,
        Some(t) => Some(Refusal {
            gate: "G4-transcript",
            why: format!(
                "the oracle transcript is {} bytes, which cannot be one. A present-but-empty \
                 field satisfies 'is it there?' and nothing else",
                t.len()
            ),
        }),
        None => Some(Refusal {
            gate: "G4-transcript",
            why: "no oracle transcript: a number in a report is a claim, the engine's own \
                  output is evidence, and only the second lets a later reader re-judge this \
                  without re-running it"
                .to_string(),
        }),
    }
}

/// Run every gate. An empty result means the claim is bankable.
pub fn evaluate(c: &Claim, p: &Policy) -> Vec<Refusal> {
    [g1_reproducible(c), g2_live_tick0(c), g3_start_control(c, p), g4_transcript(c, p)]
        .into_iter()
        .flatten()
        .collect()
}

pub fn accepted(c: &Claim, p: &Policy) -> bool {
    evaluate(c, p).is_empty()
}

/// The record banked for an accepted claim — everything a later reader needs
/// in order to re-derive it without asking anybody.
pub fn to_rec(c: &Claim) -> Rec {
    let mut r = Rec::new("claim")
        .f("what", &c.what)
        .f("tape_md5", &c.tape_md5)
        .f("map_md5", &c.map_md5)
        .f("template_md5", &c.template_md5)
        .f("drives", if c.drives { 1 } else { 0 });
    match c.frame_start_tick {
        Some(t) => r.set("frame_start_tick", t),
        None => r.set("frame_start_tick", "MISSING"),
    }
    match &c.prefix {
        Some(p) => {
            r.set("prefix_tape_md5", &p.tape_md5);
            r.set("prefix_at_tick", p.at_tick);
        }
        None => r.set("prefix", "root"),
    }
    if let Some(t) = c.live_tick0 {
        r.set("tick0_x", t.x);
        r.set("tick0_y", t.y);
        r.set("tick0_z", t.z);
        match t.dev_from_spawn_m {
            Some(d) => r.set("start_dev_m", d),
            None => r.set("start_dev_m", "UNKNOWN"),
        }
    }
    if let Some(t) = &c.oracle_transcript {
        r.set("transcript_md5", crate::md5::md5_hex(t.as_bytes()));
        r.set("transcript_bytes", t.len());
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A claim that satisfies every gate. Each refusal test below breaks
    /// exactly one field of THIS, so a test can only pass by exercising the
    /// gate it names.
    fn good() -> Claim {
        Claim {
            what: "cps 3 on Summer 2026 - 01".into(),
            tape_md5: "a".repeat(32),
            frame_start_tick: Some(0),
            prefix: None,
            map_md5: "b".repeat(32),
            template_md5: "c".repeat(32),
            live_tick0: Some(Tick0 { x: 1584.2, y: 16.0, z: 783.4, dev_from_spawn_m: Some(0.9) }),
            drives: true,
            oracle_transcript: Some(
                "TAS_23144.Ghost.Gbx  IsValid=1  Time=23144  Checkpoints=3  Respawns=0".into(),
            ),
        }
    }

    // ---- the control that makes every refusal below mean something

    #[test]
    fn a_complete_claim_is_accepted() {
        assert_eq!(evaluate(&good(), &Policy::default()), Vec::new());
        assert!(accepted(&good(), &Policy::default()));
    }

    // ---- G1

    #[test]
    fn g1_refuses_a_claim_with_no_frame() {
        let c = Claim { frame_start_tick: None, ..good() };
        let r = g1_reproducible(&c).unwrap();
        assert_eq!(r.gate, "G1-reproducible");
        assert!(r.why.contains("frame_start_tick"), "{}", r.why);
    }

    #[test]
    fn g1_refuses_a_claim_that_does_not_say_which_map() {
        let c = Claim { map_md5: String::new(), ..good() };
        assert!(g1_reproducible(&c).unwrap().why.contains("map_md5"));
    }

    #[test]
    fn g1_refuses_a_claim_that_does_not_say_which_container() {
        let c = Claim { template_md5: "short".into(), ..good() };
        assert!(g1_reproducible(&c).unwrap().why.contains("template_md5"));
    }

    #[test]
    fn a_branch_records_what_it_branched_from_and_root_is_a_valid_answer() {
        // Both must be expressible. "From the root" is a fact; a blank field
        // is an omission, and the record renders them differently.
        let rooted = to_rec(&good());
        assert!(rooted.render().contains("prefix=root"), "{}", rooted.render());

        let branched = Claim {
            prefix: Some(Prefix { tape_md5: "d".repeat(32), at_tick: 1_500 }),
            ..good()
        };
        assert!(accepted(&branched, &Policy::default()));
        let r = to_rec(&branched).render();
        assert!(r.contains("prefix_at_tick=1500"), "{r}");
    }

    // ---- G2

    #[test]
    fn g2_refuses_a_driving_worker_that_reports_no_live_tick0() {
        let c = Claim { live_tick0: None, ..good() };
        let r = g2_live_tick0(&c).unwrap();
        assert!(r.why.contains("template's telemetry"), "{}", r.why);
    }

    #[test]
    fn g2_does_not_apply_to_a_worker_that_does_not_drive() {
        // The sweep reads tapes somebody else wrote; it has no tick 0 of its
        // own and must not be asked for one.
        let c = Claim { drives: false, live_tick0: None, ..good() };
        assert!(g2_live_tick0(&c).is_none());
    }

    #[test]
    fn but_a_non_driving_worker_still_faces_every_other_gate() {
        // Otherwise `drives = false` becomes a way to switch the gates off.
        let c = Claim { drives: false, live_tick0: None, oracle_transcript: None, ..good() };
        let gates: Vec<&str> = evaluate(&c, &Policy::default()).iter().map(|r| r.gate).collect();
        assert!(gates.contains(&"G3-start-control"), "{gates:?}");
        assert!(gates.contains(&"G4-transcript"), "{gates:?}");
    }

    // ---- G3

    #[test]
    fn g3_refuses_an_absent_control_rather_than_passing_it() {
        // The whole point: "not checked" must not read as "checked and fine".
        let c = Claim {
            live_tick0: Some(Tick0 { x: 0.0, y: 0.0, z: 0.0, dev_from_spawn_m: None }),
            ..good()
        };
        let r = g3_start_control(&c, &Policy::default()).unwrap();
        assert!(r.why.contains("not checked"), "{}", r.why);
    }

    #[test]
    fn g3_refuses_a_car_that_started_at_a_checkpoint() {
        // The real case, in its real numbers.
        let c = Claim {
            live_tick0: Some(Tick0 {
                x: 1359.5,
                y: 10.0,
                z: 1103.0,
                dev_from_spawn_m: Some(390.0),
            }),
            ..good()
        };
        let r = g3_start_control(&c, &Policy::default()).unwrap();
        assert!(r.why.contains("390.0 m"), "{}", r.why);
    }

    #[test]
    fn g3_accepts_a_car_on_the_start_line() {
        assert!(g3_start_control(&good(), &Policy::default()).is_none());
    }

    // ---- G4

    #[test]
    fn g4_refuses_a_claim_with_no_transcript() {
        let c = Claim { oracle_transcript: None, ..good() };
        assert!(g4_transcript(&c, &Policy::default()).unwrap().why.contains("evidence"));
    }

    #[test]
    fn g4_refuses_a_transcript_that_is_present_but_empty() {
        // "Is the field set?" is a test any outcome satisfies.
        let c = Claim { oracle_transcript: Some(String::new()), ..good() };
        assert!(g4_transcript(&c, &Policy::default()).is_some());
        let c = Claim { oracle_transcript: Some("ok".into()), ..good() };
        assert!(g4_transcript(&c, &Policy::default()).is_some());
    }

    #[test]
    fn the_banked_record_carries_everything_needed_to_re_derive() {
        let r = to_rec(&good()).render();
        for needle in ["tape_md5=", "map_md5=", "template_md5=", "frame_start_tick=0", "transcript_md5="] {
            assert!(r.contains(needle), "{needle} missing from {r}");
        }
    }

    #[test]
    fn a_missing_frame_is_recorded_as_missing_not_as_zero() {
        // Tick 0 and "nobody said" must not render identically: the whole
        // frame defect is that a run's tick 0 is often not the file's.
        let c = Claim { frame_start_tick: None, ..good() };
        assert!(to_rec(&c).render().contains("frame_start_tick=MISSING"));
    }

    #[test]
    fn every_gate_has_a_refusal_and_the_names_match_the_list() {
        // A gate added without a way to see it refuse would be decoration.
        let broken = Claim {
            frame_start_tick: None,
            live_tick0: None,
            oracle_transcript: None,
            ..good()
        };
        let mut fired: Vec<&str> = evaluate(&broken, &Policy::default()).iter().map(|r| r.gate).collect();
        fired.sort();
        fired.dedup();
        assert_eq!(fired, GATES.to_vec());
    }
}
