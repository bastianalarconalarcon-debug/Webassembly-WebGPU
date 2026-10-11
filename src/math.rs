//! Matemática básica: vectores 2D, colores y curvas de animación.

use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

pub const TAU: f32 = std::f32::consts::TAU;
pub const PI: f32 = std::f32::consts::PI;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

pub const fn v2(x: f32, y: f32) -> V2 {
    V2 { x, y }
}

impl V2 {
    pub const ZERO: V2 = v2(0.0, 0.0);

    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn dist(self, o: V2) -> f32 {
        (self - o).len()
    }
    pub fn norm(self) -> V2 {
        let l = self.len();
        if l > 1e-6 {
            self * (1.0 / l)
        } else {
            V2::ZERO
        }
    }
    pub fn lerp(self, o: V2, t: f32) -> V2 {
        self + (o - self) * t
    }
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
    pub fn from_angle(a: f32) -> V2 {
        v2(a.cos(), a.sin())
    }
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        v2(self.x + o.x, self.y + o.y)
    }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        v2(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, k: f32) -> V2 {
        v2(self.x * k, self.y * k)
    }
}
impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        v2(-self.x, -self.y)
    }
}
impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        self.x += o.x;
        self.y += o.y;
    }
}
impl SubAssign for V2 {
    fn sub_assign(&mut self, o: V2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}
impl MulAssign<f32> for V2 {
    fn mul_assign(&mut self, k: f32) {
        self.x *= k;
        self.y *= k;
    }
}

/// Color RGBA lineal-ish (0..1). Se envía tal cual al shader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Rgba {
    Rgba { r, g, b, a }
}

/// Color desde un entero 0xRRGGBB.
pub const fn hex(h: u32) -> Rgba {
    rgba(
        ((h >> 16) & 0xff) as f32 / 255.0,
        ((h >> 8) & 0xff) as f32 / 255.0,
        (h & 0xff) as f32 / 255.0,
        1.0,
    )
}

pub const WHITE: Rgba = rgba(1.0, 1.0, 1.0, 1.0);
pub const BLACK: Rgba = rgba(0.0, 0.0, 0.0, 1.0);

impl Rgba {
    /// Multiplica la opacidad.
    pub fn alpha(self, k: f32) -> Rgba {
        rgba(self.r, self.g, self.b, self.a * k)
    }
    pub fn mix(self, o: Rgba, t: f32) -> Rgba {
        rgba(
            lerp(self.r, o.r, t),
            lerp(self.g, o.g, t),
            lerp(self.b, o.b, t),
            lerp(self.a, o.a, t),
        )
    }
    pub fn darker(self, k: f32) -> Rgba {
        self.mix(rgba(0.0, 0.0, 0.0, self.a), k)
    }
    pub fn lighter(self, k: f32) -> Rgba {
        self.mix(rgba(1.0, 1.0, 1.0, self.a), k)
    }
    pub fn arr(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

/// Matiz/saturación/valor -> RGB. `h` en vueltas (0..1).
pub fn hsv(h: f32, s: f32, v: f32) -> Rgba {
    let h = (h - h.floor()) * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    rgba(r, g, b, 1.0)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp01((x - e0) / (e1 - e0));
    t * t * (3.0 - 2.0 * t)
}

/// Acerca `cur` a `target` de forma exponencial e independiente de los FPS.
pub fn approach(cur: f32, target: f32, rate: f32, dt: f32) -> f32 {
    cur + (target - cur) * (1.0 - (-rate * dt).exp())
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let t = clamp01(t) - 1.0;
    t * t * t + 1.0
}

/// Sobrepasa un poco y vuelve: ideal para "aparecer con rebote".
pub fn ease_out_back(t: f32) -> f32 {
    let t = clamp01(t);
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Rebote elástico tipo gelatina.
pub fn ease_out_elastic(t: f32) -> f32 {
    let t = clamp01(t);
    if t <= 0.0 || t >= 1.0 {
        return t;
    }
    let c4 = TAU / 3.0;
    2f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
}

/// Frecuencia (Hz) de una nota MIDI.
pub fn midi(n: f32) -> f32 {
    440.0 * 2f32.powf((n - 69.0) / 12.0)
}

/// Escala pentatónica mayor: cualquier combinación suena bonita.
pub fn pentatonic(step: i32, base: f32) -> f32 {
    const S: [f32; 5] = [0.0, 2.0, 4.0, 7.0, 9.0];
    let oct = step.div_euclid(5);
    let i = step.rem_euclid(5) as usize;
    midi(base + oct as f32 * 12.0 + S[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_primaries() {
        assert_eq!(hsv(0.0, 1.0, 1.0), rgba(1.0, 0.0, 0.0, 1.0));
        let g = hsv(1.0 / 3.0, 1.0, 1.0);
        assert!(g.g > 0.99 && g.r < 0.01);
    }

    #[test]
    fn eases_end_at_one() {
        assert!((ease_out_back(1.0) - 1.0).abs() < 1e-5);
        assert!((ease_out_elastic(1.0) - 1.0).abs() < 1e-5);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn midi_a4() {
        assert!((midi(69.0) - 440.0).abs() < 1e-3);
        assert!((pentatonic(5, 60.0) - midi(72.0)).abs() < 1e-3);
    }
}
