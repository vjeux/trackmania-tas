//! The macro alphabet: what an edge in the search tree is.
//!
//! # Why macros and not ticks
//!
//! The raw action space is 255 steering values × gas × brake = 1020 per tick,
//! and a 45 s map is 4500 ticks. Searching that directly is what makes a tree
//! search over inputs sound hopeless. It is not the action space we want.
//!
//! A driver holds an input. A macro — one `(steer, gas, brake)` triple held for
//! `k` ticks — is both what a human actually does and what collapses the
//! branching factor to something a tree search can enumerate: 12 with the
//! keyboard alphabet, 20 or 36 with a coarse analog ladder.
//!
//! # Why the keyboard alphabet is the default
//!
//! Not taste — measurement, from three maps:
//!
//! | map | how established | cost of keyboard vs analog |
//! |---|---|---|
//! | *Great WTF of What* | keyboard arm vs analog arm, same seed and budget | 0.060 s = 0.75 % |
//! | *Get In The Hole* | keyboard vs analog, five seeds, all converge | 0 |
//! | *Turtle Trial Leto* | an unconstrained ±127 search chose {−127, 0, +127} unprompted | 0, demonstrated |
//!
//! So the analog freedom buys under 1 % on maps humans play, and it costs a
//! 3× to 10× wider tree at every layer. Start narrow; `--alphabet` widens it.
//! (`SEARCH.md` §5 item 4 wants a keyboard-legal search back in the maintained
//! lineage. This is it, for the explorer.)

/// One held input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Input {
    /// Steering, the engine's own signed byte. −127 is full left.
    pub steer: i8,
    pub gas: bool,
    pub brake: bool,
}

impl Input {
    pub const NEUTRAL: Input = Input { steer: 0, gas: false, brake: false };

    pub fn tag(&self) -> String {
        format!(
            "{}{}{}",
            match self.steer.signum() {
                -1 => "L",
                1 => "R",
                _ => "-",
            },
            if self.gas { "G" } else { "." },
            if self.brake { "B" } else { "." }
        )
    }
}

/// A held input and how long it is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Macro {
    pub input: Input,
    pub k: u16,
}

/// Which steering ladder the search may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alphabet {
    /// {−127, 0, +127} × gas × brake = 12 actions. A keyboard can produce
    /// every one of these and nothing else.
    Keyboard,
    /// {−127, −64, 0, +64, +127} × gas × brake = 20.
    Ladder5,
    /// Nine steering levels × gas × brake = 36.
    Ladder9,
}

impl Alphabet {
    pub fn parse(s: &str) -> Result<Alphabet, String> {
        match s {
            "keyboard" | "kb" => Ok(Alphabet::Keyboard),
            "ladder5" => Ok(Alphabet::Ladder5),
            "ladder9" => Ok(Alphabet::Ladder9),
            other => Err(format!(
                "unknown alphabet {:?}; want keyboard | ladder5 | ladder9",
                other
            )),
        }
    }

    pub fn steers(&self) -> &'static [i8] {
        match self {
            Alphabet::Keyboard => &[-127, 0, 127],
            Alphabet::Ladder5 => &[-127, -64, 0, 64, 127],
            Alphabet::Ladder9 => &[-127, -96, -64, -32, 0, 32, 64, 96, 127],
        }
    }

    /// Every action in the alphabet.
    ///
    /// gas-and-brake-together is included deliberately: it is legal, humans do
    /// it, and on this game it is not the same as neither. Removing it because
    /// it "looks wrong" would be a limit the search could not state.
    pub fn actions(&self) -> Vec<Input> {
        let mut v = Vec::new();
        for &steer in self.steers() {
            for &gas in &[true, false] {
                for &brake in &[false, true] {
                    v.push(Input { steer, gas, brake });
                }
            }
        }
        v
    }

    pub fn len(&self) -> usize {
        self.steers().len() * 4
    }

    pub fn is_empty(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keyboard_alphabet_is_keyboard_legal() {
        // Every steering value a keyboard can produce is full lock or nothing.
        // A ladder that quietly contained 64 would be an analog search wearing
        // the keyboard flag, and the alphabet cost table above would not apply
        // to it.
        for &s in Alphabet::Keyboard.steers() {
            assert!(s == -127 || s == 0 || s == 127, "not keyboard-legal: {}", s);
        }
        assert_eq!(Alphabet::Keyboard.actions().len(), 12);
        assert_eq!(Alphabet::Ladder5.actions().len(), 20);
        assert_eq!(Alphabet::Ladder9.actions().len(), 36);
    }

    #[test]
    fn the_alphabet_contains_the_do_nothing_action() {
        // The decoy test needs it, and a search that cannot express "hands off"
        // cannot coast.
        for a in [Alphabet::Keyboard, Alphabet::Ladder5, Alphabet::Ladder9] {
            assert!(a.actions().contains(&Input::NEUTRAL), "{:?}", a);
        }
    }

    #[test]
    fn actions_are_distinct() {
        for a in [Alphabet::Keyboard, Alphabet::Ladder5, Alphabet::Ladder9] {
            let v = a.actions();
            let mut u = v.clone();
            u.sort_by_key(|i| (i.steer, i.gas, i.brake));
            u.dedup();
            assert_eq!(u.len(), v.len(), "duplicate actions in {:?}", a);
            assert_eq!(v.len(), a.len());
        }
    }
}
