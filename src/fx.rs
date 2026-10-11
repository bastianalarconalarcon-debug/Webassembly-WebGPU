//! Partículas: confeti, chispas, fuegos artificiales, gotitas...
//! Todo lo que "explota" de alegría cuando la niña toca algo.

use crate::gfx::{Gfx, Shape};
use crate::math::{hsv, v2, Rgba, V2, TAU, WHITE};
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    Shape(Shape),
    Emoji(&'static str),
    Text(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub pos: V2,
    pub vel: V2,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    /// Cambio de tamaño por segundo (positivo = crece).
    pub grow: f32,
    pub rot: f32,
    pub spin: f32,
    pub color: Rgba,
    pub look: Look,
    pub gravity: f32,
    pub drag: f32,
    /// Al morir explota como fuego artificial.
    pub explode: bool,
}

impl Particle {
    pub fn new(pos: V2, vel: V2, life: f32, size: f32, color: Rgba, look: Look) -> Particle {
        Particle {
            pos,
            vel,
            life,
            max_life: life,
            size,
            grow: 0.0,
            rot: 0.0,
            spin: 0.0,
            color,
            look,
            gravity: 0.0,
            drag: 0.0,
            explode: false,
        }
    }
}

const MAX_PARTICLES: usize = 3000;

#[derive(Default)]
pub struct Fx {
    pub ps: Vec<Particle>,
}

impl Fx {
    pub fn add(&mut self, p: Particle) {
        if self.ps.len() >= MAX_PARTICLES {
            self.ps.remove(0);
        }
        self.ps.push(p);
    }

    pub fn clear(&mut self) {
        self.ps.clear();
    }

    pub fn update(&mut self, dt: f32, rng: &mut Rng) {
        let mut booms: Vec<(V2, Rgba)> = Vec::new();
        for p in self.ps.iter_mut() {
            p.life -= dt;
            p.vel.y += p.gravity * dt;
            if p.drag > 0.0 {
                p.vel *= (-p.drag * dt).exp();
            }
            p.pos += p.vel * dt;
            p.rot += p.spin * dt;
            p.size = (p.size + p.grow * dt).max(0.0);
            if p.life <= 0.0 && p.explode {
                booms.push((p.pos, p.color));
            }
        }
        self.ps.retain(|p| p.life > 0.0);
        for (pos, c) in booms {
            self.firework_burst(rng, pos, c);
        }
    }

    pub fn draw(&self, g: &mut Gfx) {
        for p in &self.ps {
            let t = (p.life / p.max_life).clamp(0.0, 1.0);
            // Se desvanece durante el último tercio de su vida.
            let fade = (t * 3.0).min(1.0);
            let c = p.color.alpha(fade);
            match p.look {
                Look::Shape(s) => g.shape(s, p.pos, p.size, p.rot, c),
                Look::Emoji(e) => g.emoji(e, p.pos, p.size, p.rot, c.a),
                Look::Text(s) => g.text(s, p.pos, p.size, p.rot, c, 0.12),
            }
        }
    }

    /// Estallido de confeti de colores.
    pub fn confetti(&mut self, rng: &mut Rng, at: V2, n: usize, power: f32) {
        const LOOKS: [Shape; 4] = [Shape::Star, Shape::Circle, Shape::Heart, Shape::Diamond];
        for _ in 0..n {
            let dir = rng.dir();
            let sp = rng.range(0.3, 1.0) * power;
            let mut p = Particle::new(
                at,
                dir * sp - v2(0.0, power * 0.3),
                rng.range(0.9, 1.8),
                rng.range(6.0, 14.0) * (power / 500.0).clamp(0.6, 1.6),
                hsv(rng.f(), 0.75, 1.0),
                Look::Shape(*rng.pick(&LOOKS)),
            );
            p.gravity = 600.0;
            p.drag = 1.6;
            p.spin = rng.range(-8.0, 8.0);
            p.rot = rng.f() * TAU;
            self.add(p);
        }
    }

    /// Chispitas que brillan.
    pub fn sparkle(&mut self, rng: &mut Rng, at: V2, n: usize, color: Rgba, spread: f32) {
        for _ in 0..n {
            let dir = rng.dir();
            let mut p = Particle::new(
                at + dir * rng.range(0.0, spread * 0.3),
                dir * rng.range(20.0, 120.0) * (spread / 60.0).max(0.5),
                rng.range(0.4, 0.9),
                rng.range(4.0, 9.0),
                color.lighter(rng.range(0.0, 0.5)),
                Look::Shape(Shape::Star),
            );
            p.spin = rng.range(-4.0, 4.0);
            p.drag = 2.5;
            p.grow = -6.0;
            self.add(p);
        }
    }

    /// Brillo suave que se agranda y desaparece.
    pub fn flash(&mut self, at: V2, r: f32, color: Rgba) {
        let mut p = Particle::new(at, V2::ZERO, 0.45, r * 0.6, color, Look::Shape(Shape::Glow));
        p.grow = r * 2.5;
        self.add(p);
    }

    /// Anillo que se expande (onda).
    pub fn ripple(&mut self, at: V2, r: f32, color: Rgba) {
        let mut p = Particle::new(at, V2::ZERO, 0.6, r * 0.3, color, Look::Shape(Shape::Ring));
        p.grow = r * 2.2;
        self.add(p);
    }

    /// Gotitas de una burbuja reventada.
    pub fn droplets(&mut self, rng: &mut Rng, at: V2, r: f32, color: Rgba) {
        let n = 14;
        for i in 0..n {
            let a = i as f32 / n as f32 * TAU + rng.range(-0.1, 0.1);
            let dir = V2::from_angle(a);
            let mut p = Particle::new(
                at + dir * r * 0.9,
                dir * rng.range(150.0, 320.0),
                rng.range(0.4, 0.7),
                rng.range(4.0, 8.0),
                color,
                Look::Shape(Shape::Circle),
            );
            p.gravity = 500.0;
            p.drag = 2.0;
            p.grow = -5.0;
            self.add(p);
        }
        self.ripple(at, r * 1.2, WHITE.alpha(0.7));
    }

    /// Cohete que sube y explota.
    pub fn rocket(&mut self, rng: &mut Rng, from: V2, height: f32) {
        let c = hsv(rng.f(), 0.7, 1.0);
        let t = rng.range(0.7, 1.0);
        let mut p = Particle::new(
            from,
            v2(rng.range(-60.0, 60.0), -height / t),
            t,
            7.0,
            c,
            Look::Shape(Shape::Glow),
        );
        p.explode = true;
        self.add(p);
        // Estela.
        for i in 0..6 {
            let mut s = p;
            s.explode = false;
            s.life = t * (0.3 + i as f32 * 0.1);
            s.max_life = s.life;
            s.size = 4.0;
            s.color = c.lighter(0.5);
            self.add(s);
        }
    }

    fn firework_burst(&mut self, rng: &mut Rng, at: V2, c: Rgba) {
        let n = 36;
        for i in 0..n {
            let a = i as f32 / n as f32 * TAU;
            let mut p = Particle::new(
                at,
                V2::from_angle(a) * rng.range(180.0, 320.0),
                rng.range(0.9, 1.4),
                rng.range(4.0, 7.0),
                c.lighter(rng.range(0.0, 0.4)),
                Look::Shape(if i % 3 == 0 { Shape::Star } else { Shape::Circle }),
            );
            p.gravity = 180.0;
            p.drag = 1.4;
            p.spin = 3.0;
            self.add(p);
        }
        self.flash(at, 90.0, c.alpha(0.6));
    }

    /// Un emoji que sale volando, gira y desaparece.
    pub fn fling_emoji(&mut self, rng: &mut Rng, at: V2, e: &'static str, size: f32) {
        let mut p = Particle::new(
            at,
            v2(rng.range(-200.0, 200.0), rng.range(-450.0, -250.0)),
            1.4,
            size,
            WHITE,
            Look::Emoji(e),
        );
        p.gravity = 700.0;
        p.spin = rng.range(-5.0, 5.0);
        self.add(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particles_die_and_rockets_explode() {
        let mut fx = Fx::default();
        let mut rng = Rng::new(1);
        fx.rocket(&mut rng, v2(100.0, 500.0), 300.0);
        for _ in 0..200 {
            fx.update(1.0 / 60.0, &mut rng);
        }
        // Después de ~3 segundos ya no queda nada.
        assert!(fx.ps.is_empty());
    }

    #[test]
    fn rocket_creates_burst() {
        let mut fx = Fx::default();
        let mut rng = Rng::new(2);
        fx.rocket(&mut rng, v2(100.0, 500.0), 300.0);
        let mut max = 0;
        for _ in 0..80 {
            fx.update(1.0 / 60.0, &mut rng);
            max = max.max(fx.ps.len());
        }
        assert!(max > 30);
    }
}
