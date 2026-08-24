//! A deterministic PRNG, in the crate rather than as a dependency.
//!
//! Two reasons, and neither is taste. A search whose sequence depends on a
//! crate version is a search whose results cannot be reproduced from a seed
//! six months later — and reproducibility from a seed is the only reason a
//! stochastic policy is acceptable here at all. And the crate has no other
//! dependencies, so it builds and tests on a box with no network.
//!
//! xorshift64*, which is fine for choosing bins and is not used for anything
//! that a statistician would care about.

pub struct Rng {
    s: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        // splitmix the seed so that 0, 1, 2 are not near-identical streams.
        let mut z = seed.wrapping_add(0x9E3779B97F4A7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        Rng { s: (z ^ (z >> 31)) | 1 }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.s;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.s = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in [0, n). `n == 0` gives 0.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }

    /// Fisher-Yates.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            v.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_stream() {
        let a: Vec<u64> = (0..8).map(|_| Rng::new(7).next_u64()).collect();
        let mut r = Rng::new(7);
        let b: Vec<u64> = (0..8).map(|_| r.next_u64()).collect();
        assert_eq!(a[0], b[0]);
        let mut r2 = Rng::new(7);
        let c: Vec<u64> = (0..8).map(|_| r2.next_u64()).collect();
        assert_eq!(b, c);
    }

    #[test]
    fn different_seeds_give_different_streams() {
        // The other half. A generator that ignored its seed would pass the
        // test above perfectly.
        assert_ne!(Rng::new(1).next_u64(), Rng::new(2).next_u64());
        assert_ne!(Rng::new(0).next_u64(), Rng::new(1).next_u64());
    }

    #[test]
    fn below_is_in_range_and_not_constant() {
        let mut r = Rng::new(3);
        let mut seen = [0u32; 5];
        for _ in 0..5000 {
            let v = r.below(5) as usize;
            assert!(v < 5);
            seen[v] += 1;
        }
        assert!(seen.iter().all(|&c| c > 700), "{:?}", seen);
    }
}
