//! The action space.
//!
//! # Why not Linesight's twelve
//!
//! Linesight's action set is `{left, none, right} × {accel, nothing, brake,
//! accel+brake}` — twelve, and **no analog steer anywhere**
//! (`config_files/inputs_list.py`). That is a property of their input path, not
//! a finding: TMNF keyboard input is three-valued. Our tape stores steer as a
//! byte the engine reads as `(byte as i8) as f32 / 127.0`, so intermediate
//! steer costs us nothing and a keyboard-only alphabet would be throwing a
//! degree of freedom away for no reason.
//!
//! So the default is a **five-rung steer ladder × four throttle states = 20
//! actions**, and the keyboard-legal twelve is a strict subset of it (rungs
//! ±127 and 0). The ladder is coarse on purpose: a policy that has to choose
//! between 255 steer values spends its early samples learning that neighbouring
//! values are nearly the same.
//!
//! `gas + brake` is kept because it is a real Trackmania technique and
//! Linesight kept it deliberately (indices 9–11).

/// One action: the raw tape bytes for a tick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Act {
    /// Steer as the format's byte; the engine reads `(as i8) as f32 / 127.0`.
    pub steer: u8,
    pub gas: u8,
    pub brake: u8,
}

/// The steer rungs, as `i8`. Symmetric, and containing 0 and both locks.
pub const STEER_LADDER: [i8; 5] = [-127, -48, 0, 48, 127];

/// `(gas, brake)`, in the order the table walks them.
pub const THROTTLE: [(u8, u8); 4] = [(1, 0), (0, 0), (0, 1), (1, 1)];

/// The action table: steer-major, so `idx / 4` is the steer rung and `idx % 4`
/// the throttle state.
#[derive(Clone, Debug)]
pub struct ActionSpace {
    table: Vec<Act>,
}

impl Default for ActionSpace {
    fn default() -> Self {
        ActionSpace::ladder(&STEER_LADDER, &THROTTLE)
    }
}

impl ActionSpace {
    pub fn ladder(steer: &[i8], throttle: &[(u8, u8)]) -> ActionSpace {
        let mut table = Vec::with_capacity(steer.len() * throttle.len());
        for &s in steer {
            for &(g, b) in throttle {
                table.push(Act { steer: s as u8, gas: g, brake: b });
            }
        }
        ActionSpace { table }
    }

    /// Linesight's twelve, for a like-for-like comparison arm.
    pub fn keyboard() -> ActionSpace {
        ActionSpace::ladder(&[-127, 0, 127], &THROTTLE)
    }

    pub fn n(&self) -> usize {
        self.table.len()
    }

    pub fn get(&self, i: usize) -> Act {
        self.table[i]
    }

    /// The index of full throttle, straight ahead — the scripted policy the
    /// known-answer test drives.
    pub fn forward(&self) -> usize {
        self.table
            .iter()
            .position(|a| a.steer == 0 && a.gas == 1 && a.brake == 0)
            .expect("an action table with no gas-and-straight action is not a driving alphabet")
    }

    pub fn iter(&self) -> impl Iterator<Item = &Act> {
        self.table.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_table_contains_the_keyboard_legal_twelve() {
        let d = ActionSpace::default();
        let k = ActionSpace::keyboard();
        assert_eq!(d.n(), 20);
        assert_eq!(k.n(), 12);
        // The positive half: every keyboard action is reachable in the default
        // alphabet, so nothing Linesight can express is lost.
        for a in k.iter() {
            assert!(d.iter().any(|b| b == a), "default table lacks {:?}", a);
        }
        // The negative half, because "is a subset" is satisfied by two identical
        // tables and would say nothing about the analog rungs existing.
        assert!(d.iter().any(|a| a.steer == 48u8.wrapping_neg() || a.steer == 48));
        assert!(!k.iter().any(|a| a.steer == 48));
    }

    #[test]
    fn forward_is_gas_and_straight() {
        let d = ActionSpace::default();
        let f = d.get(d.forward());
        assert_eq!((f.steer, f.gas, f.brake), (0, 1, 0));
    }

    #[test]
    fn the_steer_byte_round_trips_through_the_engines_own_reading() {
        // The engine reads `(byte as i8) as f32 / 127.0`. A rung that does not
        // survive that reading is a rung the car never sees.
        for &s in &STEER_LADDER {
            let byte = s as u8;
            let engine = ((byte as i8) as f32) / 127.0;
            assert!((engine - (s as f32 / 127.0)).abs() < 1e-6, "rung {s} does not round-trip");
        }
    }
}
