//! The committed input prefixes, shared.
//!
//! An archive of 100 000 entries, each holding its own `Vec<Input>` of up to
//! 4500 ticks, is 450 M inputs. It is also mostly the same inputs over and
//! over: two entries that diverged at tick 3000 agree on the first 3000.
//!
//! So a node is `(parent, the one macro that extends it)` and a prefix is
//! recovered by walking parents. An entry costs 16 bytes instead of kilobytes,
//! and the whole tree is one `Vec`.

use crate::action::{Input, Macro};

pub type NodeId = u32;

#[derive(Clone, Copy, Debug)]
struct Seg {
    parent: Option<NodeId>,
    m: Macro,
    /// Ticks consumed up to and including this segment.
    end_tick: u32,
    depth: u32,
}

/// The tree of committed input prefixes.
pub struct Trunk {
    segs: Vec<Seg>,
    /// The root's own tape, when the search was seeded from a banked run.
    seed: Vec<Input>,
}

impl Default for Trunk {
    fn default() -> Self {
        Self::new()
    }
}

impl Trunk {
    /// A new tree holding only the root: zero ticks of input.
    pub fn new() -> Trunk {
        Trunk {
            segs: vec![Seg {
                parent: None,
                m: Macro { input: Input::NEUTRAL, k: 0 },
                end_tick: 0,
                depth: 0,
            }],
            seed: Vec::new(),
        }
    }

    pub const ROOT: NodeId = 0;

    pub fn len(&self) -> usize {
        self.segs.len()
    }
    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn end_tick(&self, id: NodeId) -> u32 {
        self.segs[id as usize].end_tick
    }
    pub fn depth(&self, id: NodeId) -> u32 {
        self.segs[id as usize].depth
    }

    pub fn push(&mut self, parent: NodeId, m: Macro) -> NodeId {
        let p = self.segs[parent as usize];
        let id = self.segs.len() as NodeId;
        self.segs.push(Seg {
            parent: Some(parent),
            m,
            end_tick: p.end_tick + m.k as u32,
            depth: p.depth + 1,
        });
        id
    }

    /// The input prefix that reaches `id`, truncated to `n_ticks`.
    ///
    /// `n_ticks` must not exceed `end_tick(id)`; a request past the end is a
    /// caller bug and panics rather than silently returning a shorter tape,
    /// because a shorter tape is a different run and looks like a result.
    pub fn inputs_to(&self, id: NodeId, n_ticks: u32) -> Vec<Input> {
        let end = self.end_tick(id);
        assert!(
            n_ticks <= end,
            "asked for {} ticks of a prefix that is {} long",
            n_ticks,
            end
        );
        let mut out = vec![Input::NEUTRAL; end as usize];
        for (i, v) in self.seed.iter().enumerate().take(end as usize) {
            out[i] = *v;
        }
        let mut cur = Some(id);
        while let Some(c) = cur {
            let s = self.segs[c as usize];
            let lo = (s.end_tick - s.m.k as u32) as usize;
            for slot in out.iter_mut().take(s.end_tick as usize).skip(lo) {
                *slot = s.m.input;
            }
            cur = s.parent;
        }
        out.truncate(n_ticks as usize);
        out
    }

    /// The macros from the root to `id`, in order. For writing a run out.
    pub fn macros_to(&self, id: NodeId) -> Vec<Macro> {
        let mut v = Vec::new();
        let mut cur = Some(id);
        while let Some(c) = cur {
            let s = self.segs[c as usize];
            if s.m.k > 0 {
                v.push(s.m);
            }
            cur = s.parent;
        }
        v.reverse();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inp(steer: i8, gas: bool) -> Input {
        Input { steer, gas, brake: false }
    }

    #[test]
    fn a_prefix_reconstructs_exactly() {
        let mut t = Trunk::new();
        let a = t.push(Trunk::ROOT, Macro { input: inp(0, true), k: 3 });
        let b = t.push(a, Macro { input: inp(127, true), k: 2 });
        assert_eq!(t.end_tick(b), 5);
        let got = t.inputs_to(b, 5);
        assert_eq!(
            got,
            vec![inp(0, true), inp(0, true), inp(0, true), inp(127, true), inp(127, true)]
        );
        // and truncated
        assert_eq!(t.inputs_to(b, 4), got[..4].to_vec());
    }

    #[test]
    fn siblings_share_their_parent_and_do_not_disturb_it() {
        // The failure this guards: a shared arena that writes through to a
        // parent, so expanding one child silently rewrites another's history.
        let mut t = Trunk::new();
        let a = t.push(Trunk::ROOT, Macro { input: inp(0, true), k: 4 });
        let b = t.push(a, Macro { input: inp(-127, false), k: 2 });
        let c = t.push(a, Macro { input: inp(127, true), k: 2 });
        assert_eq!(t.inputs_to(b, 6)[..4], t.inputs_to(c, 6)[..4]);
        assert_ne!(t.inputs_to(b, 6)[4], t.inputs_to(c, 6)[4]);
        assert_eq!(t.inputs_to(a, 4), t.inputs_to(b, 4));
    }

    #[test]
    fn the_root_is_an_empty_tape() {
        let t = Trunk::new();
        assert_eq!(t.end_tick(Trunk::ROOT), 0);
        assert!(t.inputs_to(Trunk::ROOT, 0).is_empty());
    }
}

impl Trunk {
    /// A tree whose ROOT already holds a tape.
    ///
    /// This is how a search restarts from a banked result instead of from the
    /// grid. The first confirmed run on *Summer 2026 - 01* collected all three
    /// checkpoints and stopped 417 m from the finish; continuing the ordinary
    /// search re-explores those 1483 m on every node, which is work whose
    /// answer is already on disk.
    ///
    /// The seed is OUR OWN previous output, re-simulated by the plain oracle
    /// before it was banked. Building on it is regression-testing our own work,
    /// not consulting a reference.
    pub fn with_seed(seed: Vec<Input>) -> Trunk {
        let n = seed.len() as u32;
        Trunk {
            segs: vec![Seg {
                parent: None,
                m: Macro { input: Input::NEUTRAL, k: 0 },
                end_tick: n,
                depth: 0,
            }],
            seed,
        }
    }
}
