//! Generador pseudoaleatorio pequeño (xorshift64*), sin dependencias.

use crate::math::{v2, TAU, V2};

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        let mut r = Rng((seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        for _ in 0..4 {
            r.next_u32();
        }
        r
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// Número en [0, 1).
    pub fn f(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }

    /// Entero en [0, n).
    pub fn int(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u32() as usize) % n
        }
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }

    pub fn pick<'a, T>(&mut self, s: &'a [T]) -> &'a T {
        &s[self.int(s.len())]
    }

    pub fn shuffle<T>(&mut self, s: &mut [T]) {
        for i in (1..s.len()).rev() {
            let j = self.int(i + 1);
            s.swap(i, j);
        }
    }

    /// Dirección aleatoria de largo 1.
    pub fn dir(&mut self) -> V2 {
        V2::from_angle(self.f() * TAU)
    }

    /// Punto aleatorio dentro de un rectángulo.
    pub fn point(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) -> V2 {
        v2(self.range(x0, x1), self.range(y0, y1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_stay_in_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let x = r.f();
            assert!((0.0..1.0).contains(&x));
            assert!(r.int(5) < 5);
        }
    }
}
