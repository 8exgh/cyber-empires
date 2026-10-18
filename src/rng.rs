//! Turbo C 2.0 compatible pseudo random generator (LCG) plus helpers.

#[derive(Clone, Debug)]
pub struct Rng {
    seed: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng { seed }
    }
    pub fn seed(&mut self, s: u32) {
        self.seed = s;
    }
    /// Turbo C rand(): seed = seed * 0x015A4E35 + 1; return (seed >> 16) & 0x7FFF
    pub fn rand(&mut self) -> i32 {
        self.seed = self.seed.wrapping_mul(0x015A4E35).wrapping_add(1);
        ((self.seed >> 16) & 0x7FFF) as i32
    }
    /// Random integer in [lo, hi] inclusive.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.rand() % (hi - lo + 1)
    }
    pub fn chance(&mut self, percent: i32) -> bool {
        self.range(1, 100) <= percent
    }
    pub fn below(&mut self, n: i32) -> i32 {
        if n <= 1 {
            0
        } else {
            self.rand() % n
        }
    }
}
