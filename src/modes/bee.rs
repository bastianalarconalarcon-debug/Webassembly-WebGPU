//! La abeja: se mueve con las flechas del teclado y junta flores.
//! En pantalla hay unas flechas iguales a las del teclado que se iluminan
//! al apretarlas, para aprender qué tecla mueve hacia dónde. También se puede
//! mantener el clic para que la abeja vuele hacia el mouse.

use super::{Activity, IdlePrompt};
use crate::audio::{Inst, Sfx};
use crate::game::{Cx, Key};
use crate::gfx::{Backdrop, Shape};
use crate::math::{ease_out_back, hex, hsv, pentatonic, v2, Rgba, TAU, V2, WHITE};
use crate::vocab;

const FLOWERS: [&str; 4] = ["🌸", "🌼", "🌻", "🌷"];
const GOAL: u32 = 10;
/// El emoji 🐝 mira hacia la izquierda en la mayoría de los sistemas.
const BEE_FACES_LEFT: bool = true;

struct Flower {
    pos: V2,
    emoji: &'static str,
    age: f32,
    phase: f32,
}

struct Pad {
    dir: V2,
    code: &'static str,
    pos: V2,
    half: f32,
    flash: f32,
}

pub struct Bee {
    pos: V2,
    vel: V2,
    face_right: bool,
    spin: f32,
    flowers: Vec<Flower>,
    respawn: Vec<f32>,
    count: u32,
    pads: Vec<Pad>,
    idle: IdlePrompt,
    trail_acc: f32,
    buzz_t: f32,
    pad_pressed: Option<usize>,
}

impl Bee {
    pub fn new(cx: &mut Cx) -> Bee {
        let pad = |dir, code| Pad {
            dir,
            code,
            pos: V2::ZERO,
            half: 1.0,
            flash: 0.0,
        };
        let mut b = Bee {
            pos: v2(cx.w * 0.4, cx.h * 0.5),
            vel: V2::ZERO,
            face_right: true,
            spin: 0.0,
            flowers: Vec::new(),
            respawn: vec![0.4, 0.8, 1.2],
            count: 0,
            pads: vec![
                pad(v2(0.0, -1.0), "ArrowUp"),
                pad(v2(-1.0, 0.0), "ArrowLeft"),
                pad(v2(0.0, 1.0), "ArrowDown"),
                pad(v2(1.0, 0.0), "ArrowRight"),
            ],
            idle: IdlePrompt::new(10.0),
            trail_acc: 0.0,
            buzz_t: 0.0,
            pad_pressed: None,
        };
        b.layout(cx);
        b
    }

    fn layout(&mut self, cx: &Cx) {
        let s = (cx.unit() * 0.06).clamp(26.0, 52.0);
        let base = v2(cx.w - s * 4.2, cx.h - s * 1.6);
        let gap = s * 2.25;
        let spots = [v2(0.0, -gap), v2(-gap, 0.0), v2(0.0, 0.0), v2(gap, 0.0)];
        for (p, off) in self.pads.iter_mut().zip(spots) {
            p.pos = base + off;
            p.half = s;
        }
    }

    fn bee_r(cx: &Cx) -> f32 {
        cx.unit() * 0.075
    }

    fn slot(cx: &Cx, i: usize) -> (V2, f32) {
        let r = (cx.unit() * 0.032).max(13.0);
        let gap = r * 2.4;
        let x0 = cx.w * 0.5 - (GOAL as f32 - 1.0) * gap * 0.5;
        (v2(x0 + i as f32 * gap, r * 1.6), r)
    }

    fn spawn_flower(&mut self, cx: &mut Cx) {
        let fr = cx.unit() * 0.06;
        let mut best = cx.center();
        for _ in 0..20 {
            let p = cx
                .rng
                .point(fr * 2.0, cx.h * 0.2, cx.w - fr * 2.0, cx.h - fr * 2.0);
            let near_bee = p.dist(self.pos) < cx.unit() * 0.3;
            let near_pad = self.pads.iter().any(|pad| p.dist(pad.pos) < pad.half * 4.0);
            let near_flower = self.flowers.iter().any(|f| f.pos.dist(p) < fr * 3.0);
            best = p;
            if !near_bee && !near_pad && !near_flower {
                break;
            }
        }
        self.flowers.push(Flower {
            pos: best,
            emoji: *cx.rng.pick(&FLOWERS),
            age: 0.0,
            phase: cx.rng.range(0.0, TAU),
        });
    }

    fn collect(&mut self, cx: &mut Cx, i: usize) {
        let f = self.flowers.remove(i);
        self.count += 1;
        let p = f.pos;
        cx.fx.sparkle(&mut cx.rng, p, 18, hex(0xFFEB3B), 70.0);
        let size = cx.unit() * 0.05;
        cx.fx.fling_emoji(&mut cx.rng, p, f.emoji, size);
        cx.au.note(
            pentatonic(self.count as i32 + 4, 60.0),
            Inst::Marimba,
            p.x,
            0.6,
        );
        let n = self.count;
        cx.au
            .say(&format!("¡{}!", vocab::count_phrase(n, &vocab::FLOR)));
        self.respawn.push(0.9);
        if n >= GOAL {
            cx.au.sfx(Sfx::Fanfare, 1.0, cx.w * 0.5);
            cx.au.say_after(1.6, "¡La abeja está feliz! ¡Bravo!");
            for k in 0..4 {
                cx.fx.rocket(
                    &mut cx.rng,
                    v2(cx.w * (0.2 + 0.2 * k as f32), cx.h),
                    cx.h * 0.6,
                );
            }
            for k in 0..GOAL as usize {
                let (sp, _) = Bee::slot(cx, k);
                cx.fx.confetti(&mut cx.rng, sp, 5, 300.0);
            }
            self.count = 0;
        }
    }

    fn direction(&self, cx: &Cx) -> V2 {
        let mut d = V2::ZERO;
        let held = |a: &str, b: &str| cx.inp.held(a) || cx.inp.held(b);
        if held("ArrowUp", "KeyW") {
            d.y -= 1.0;
        }
        if held("ArrowDown", "KeyS") {
            d.y += 1.0;
        }
        if held("ArrowLeft", "KeyA") {
            d.x -= 1.0;
        }
        if held("ArrowRight", "KeyD") {
            d.x += 1.0;
        }
        if let Some(i) = self.pad_pressed {
            d += self.pads[i].dir;
        }
        d.norm()
    }
}

impl Activity for Bee {
    fn enter(&mut self, cx: &mut Cx) {
        cx.au.say("¡La abeja! Usa las flechas para juntar flores.");
    }

    fn resize(&mut self, cx: &mut Cx) {
        self.layout(cx);
    }

    fn update(&mut self, cx: &mut Cx) {
        cx.g.background(hex(0x81D4FA), hex(0xE1F5C4), Backdrop::Meadow, 0.0);
        let dt = cx.dt;
        let t = cx.t;
        self.idle
            .tick(cx, "¡Usa las flechas para mover a la abeja!");
        let waiting = self.idle.waiting(cx);

        // Flores nuevas.
        for r in self.respawn.iter_mut() {
            *r -= dt;
        }
        let ready = self.respawn.iter().filter(|r| **r <= 0.0).count();
        self.respawn.retain(|r| *r > 0.0);
        for _ in 0..ready {
            self.spawn_flower(cx);
        }

        // Movimiento.
        let mut dir = self.direction(cx);
        if dir == V2::ZERO && cx.inp.buttons[0] && self.pad_pressed.is_none() {
            let to = cx.inp.mouse - self.pos;
            if to.len() > 20.0 {
                dir = to.norm();
            }
        }
        let accel = cx.unit() * 3.2;
        self.vel += dir * accel * dt;
        self.vel *= (-2.5 * dt).exp();
        let max = cx.unit() * 0.9;
        if self.vel.len() > max {
            self.vel = self.vel.norm() * max;
        }
        let r = Bee::bee_r(cx);
        let before = self.pos;
        self.pos += self.vel * dt;
        self.pos.x = self.pos.x.clamp(r, cx.w - r);
        self.pos.y = self.pos.y.clamp(r * 1.5, cx.h - r);
        if self.vel.x.abs() > 20.0 {
            self.face_right = self.vel.x > 0.0;
        }
        let moved = before.dist(self.pos);
        if moved > 0.5 {
            self.buzz_t -= dt;
            if self.buzz_t <= 0.0 {
                cx.au.sfx_vol(
                    Sfx::Buzz,
                    1.0 + self.vel.len() / max * 0.3,
                    self.pos.x,
                    0.35,
                );
                self.buzz_t = 0.7;
            }
            self.trail_acc += moved;
            while self.trail_acc > 22.0 {
                self.trail_acc -= 22.0;
                let back = if self.face_right { -1.0 } else { 1.0 };
                let mut p = crate::fx::Particle::new(
                    self.pos + v2(back * r * 0.6, r * 0.1),
                    V2::ZERO,
                    0.9,
                    4.0,
                    hex(0xFFF59D),
                    crate::fx::Look::Shape(Shape::Circle),
                );
                p.grow = -3.0;
                cx.fx.add(p);
            }
        }
        self.spin = (self.spin - dt * 1.2).max(0.0);

        // ¿Tocó una flor?
        let fr = cx.unit() * 0.06;
        if let Some(i) = self
            .flowers
            .iter()
            .position(|f| f.age > 0.3 && f.pos.dist(self.pos) < (r + fr) * 0.75)
        {
            self.collect(cx, i);
        }

        // Flores.
        for f in self.flowers.iter_mut() {
            f.age += dt;
            let k = ease_out_back((f.age / 0.6).min(1.0));
            let sway = (t * 2.0 + f.phase).sin() * 0.12;
            let stem_base = f.pos + v2(0.0, fr * 1.6);
            cx.g.shape_full(
                Shape::RoundRect,
                (f.pos + stem_base) * 0.5,
                v2(fr * 0.07, fr * 0.8 * k),
                sway * 0.5,
                hex(0x43A047),
                1.0,
                0.0,
            );
            cx.g.emoji_shadow(f.emoji, f.pos, fr * k, sway, 1.0);
            cx.g.emoji(f.emoji, f.pos, fr * k, sway, 1.0);
        }

        // Abeja.
        let bob = (t * 7.0).sin() * r * 0.06;
        let tilt = (self.vel.y / max) * 0.35 * if self.face_right { 1.0 } else { -1.0 };
        let loop_rot = if self.spin > 0.0 {
            (1.0 - self.spin) * TAU * if self.face_right { -1.0 } else { 1.0 }
        } else {
            0.0
        };
        let flip = self.face_right == BEE_FACES_LEFT;
        let wing = 1.0 + (t * 40.0).sin() * 0.03;
        let bp = self.pos + v2(0.0, bob);
        cx.g.emoji_flip("🐝", bp, r * wing, tilt + loop_rot, 1.0, flip);

        // Contador de flores.
        let (first, sr) = Bee::slot(cx, 0);
        let (last, _) = Bee::slot(cx, GOAL as usize - 1);
        cx.g.rrect(
            v2((first.x + last.x) * 0.5, first.y),
            v2((last.x - first.x) * 0.5 + sr * 1.4, sr * 1.3),
            1.0,
            WHITE.alpha(0.45),
        );
        for i in 0..GOAL as usize {
            let (p, sr) = Bee::slot(cx, i);
            if (i as u32) < self.count {
                cx.g.emoji(FLOWERS[i % FLOWERS.len()], p, sr, 0.0, 1.0);
            } else {
                cx.g.circle(p, sr * 0.25, hex(0x558B2F).alpha(0.35));
            }
        }

        // Flechas en pantalla (como las del teclado).
        for (i, pad) in self.pads.iter_mut().enumerate() {
            let held = cx.inp.held(pad.code) || self.pad_pressed == Some(i);
            pad.flash = if held {
                1.0
            } else {
                (pad.flash - dt * 4.0).max(0.0)
            };
            let hint = if waiting {
                ((t * 3.0 - i as f32 * 0.8).sin().max(0.0)).powi(3)
            } else {
                0.0
            };
            let lit = pad.flash.max(hint);
            let s = pad.half;
            let press = v2(0.0, lit * s * 0.12);
            cx.g.shape_full(
                Shape::RoundRect,
                pad.pos + v2(0.0, s * 0.18),
                v2(s, s),
                0.0,
                hex(0x546E7A),
                0.25,
                0.0,
            );
            let face = Rgba {
                r: 0.97,
                g: 0.97,
                b: 1.0,
                a: 1.0,
            }
            .mix(hex(0xFFD54F), lit);
            cx.g.shape_full(
                Shape::RoundRect,
                pad.pos + press,
                v2(s, s),
                0.0,
                face,
                0.25,
                0.0,
            );
            let rot = pad.dir.angle();
            cx.g.shape_full(
                Shape::Arrow,
                pad.pos + press,
                v2(s * 0.6, s * 0.6),
                rot,
                hex(0x37474F),
                0.0,
                0.0,
            );
        }
    }

    fn down(&mut self, cx: &mut Cx, p: V2, _button: u32) {
        if let Some(i) = self.pads.iter().position(|pad| {
            (p.x - pad.pos.x).abs() < pad.half * 1.1 && (p.y - pad.pos.y).abs() < pad.half * 1.1
        }) {
            self.pad_pressed = Some(i);
            cx.au.sfx_vol(Sfx::Tick, 1.0, p.x, 0.6);
        }
    }

    fn up(&mut self, _cx: &mut Cx, _p: V2, _button: u32) {
        self.pad_pressed = None;
    }

    fn key(&mut self, cx: &mut Cx, k: &Key) {
        if k.arrow().is_some() {
            // La dirección se lee en `update` con las teclas mantenidas.
            if let Some(name) = vocab::key_name(k.code) {
                cx.au.say_if_quiet(name);
            }
            return;
        }
        if k.is("Space") {
            self.spin = 1.0;
            cx.au.sfx(Sfx::Boing, 1.2, self.pos.x);
            cx.au.say("¡Bzzzz!");
            return;
        }
        if matches!(k.code, "KeyW" | "KeyA" | "KeyS" | "KeyD") {
            return;
        }
        cx.au.sfx_vol(Sfx::Buzz, 1.3, self.pos.x, 0.6);
        cx.fx
            .sparkle(&mut cx.rng, self.pos, 10, hsv(0.14, 0.7, 1.0), 50.0);
    }
}
