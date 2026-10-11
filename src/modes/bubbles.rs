//! Burbujas: suben burbujas con animales adentro. Al pasar el mouse
//! tiemblan; con un clic revientan y el animal dice su nombre y su sonido.
//! Apretar una letra revienta la burbuja cuyo animal empieza con esa letra
//! (o hace aparecer uno nuevo que empiece con ella).

use super::{Activity, Hover, IdlePrompt};
use crate::audio::{Inst, Sfx};
use crate::game::{Cx, Key};
use crate::gfx::{Backdrop, Shape};
use crate::math::{ease_out_back, hex, hsv, pentatonic, v2, V2, WHITE};
use crate::vocab::{self, Word};

struct Bubble {
    pos: V2,
    speed: f32,
    r: f32,
    word: Word,
    phase: f32,
    hover: Hover,
    age: f32,
    tint: f32,
}

impl Bubble {
    fn center(&self, t: f32) -> V2 {
        self.pos + v2((t * 0.8 + self.phase).sin() * self.r * 0.35, 0.0)
    }
}

/// Animal liberado: crece, salta y vuela al estante de arriba.
struct Freed {
    word: Word,
    from: V2,
    r: f32,
    t: f32,
    slot: usize,
}

const SHELF: usize = 10;
const FREED_TIME: f32 = 1.6;

pub struct Bubbles {
    bubbles: Vec<Bubble>,
    freed: Vec<Freed>,
    shelf: Vec<&'static str>,
    spawn_t: f32,
    ambient_t: f32,
    idle: IdlePrompt,
}

impl Bubbles {
    pub fn new(_cx: &mut Cx) -> Bubbles {
        Bubbles { bubbles: Vec::new(), freed: Vec::new(), shelf: Vec::new(), spawn_t: 0.3, ambient_t: 0.0, idle: IdlePrompt::new(12.0) }
    }

    fn max_bubbles(cx: &Cx) -> usize {
        ((cx.w / cx.unit()) * 4.0).clamp(4.0, 8.0) as usize
    }

    fn spawn(&mut self, cx: &mut Cx, word: Option<Word>, x: Option<f32>) {
        let r = cx.unit() * cx.rng.range(0.085, 0.12);
        let word = word.unwrap_or_else(|| {
            if cx.rng.chance(0.8) {
                *cx.rng.pick(vocab::ANIMALS)
            } else {
                *cx.rng.pick(vocab::COUNTABLES)
            }
        });
        // Buscar una x que no choque con burbujas recién nacidas.
        let mut px = x.unwrap_or(0.0);
        if x.is_none() {
            for _ in 0..8 {
                px = cx.rng.range(0.1, 0.9) * cx.w;
                let crowded = self.bubbles.iter().any(|b| b.pos.y > cx.h - r * 3.0 && (b.pos.x - px).abs() < r * 2.2);
                if !crowded {
                    break;
                }
            }
        }
        self.bubbles.push(Bubble {
            pos: v2(px, cx.h + r * 1.2),
            speed: cx.rng.range(45.0, 75.0) * (cx.h / 800.0).max(0.7),
            r,
            word,
            phase: cx.rng.range(0.0, 6.0),
            hover: Hover::default(),
            age: 0.0,
            tint: cx.rng.f(),
        });
    }

    fn pop(&mut self, cx: &mut Cx, i: usize) {
        let b = self.bubbles.remove(i);
        let p = b.center(cx.t);
        cx.fx.droplets(&mut cx.rng, p, b.r, hsv(b.tint, 0.3, 1.0).alpha(0.9));
        cx.au.sfx(Sfx::Pop, 1.4 - b.r / cx.unit() * 3.0, p.x);
        let mut speech = format!("¡{}!", capitalize(b.word.word));
        if !b.word.sound.is_empty() {
            speech.push(' ');
            speech.push_str(b.word.sound);
        }
        cx.au.say(&speech);
        let slot = (self.shelf.len() + self.freed.len()) % SHELF;
        self.freed.push(Freed { word: b.word, from: p, r: b.r * 0.62, t: 0.0, slot });
    }

    fn shelf_slot(cx: &Cx, i: usize) -> (V2, f32) {
        let r = (cx.unit() * 0.035).max(14.0);
        let gap = r * 2.4;
        let x0 = cx.w * 0.5 - (SHELF as f32 - 1.0) * gap * 0.5;
        (v2(x0 + i as f32 * gap, r * 1.6), r)
    }

    fn celebrate(&mut self, cx: &mut Cx) {
        cx.au.sfx(Sfx::Fanfare, 1.0, cx.w * 0.5);
        cx.au.say_after(1.2, "¡Diez burbujas! ¡Bravo!");
        for i in 0..4 {
            cx.fx.rocket(&mut cx.rng, v2(cx.w * (0.2 + 0.2 * i as f32), cx.h), cx.h * 0.6);
        }
        for i in 0..SHELF {
            let (p, _) = Bubbles::shelf_slot(cx, i);
            cx.fx.confetti(&mut cx.rng, p, 6, 300.0);
        }
        self.shelf.clear();
    }

    fn nearest(&self, cx: &Cx, p: V2) -> Option<usize> {
        let t = cx.t;
        self.bubbles
            .iter()
            .enumerate()
            .filter(|(_, b)| b.pos.y < cx.h + b.r * 0.5)
            .min_by(|(_, a), (_, b)| a.center(t).dist(p).total_cmp(&b.center(t).dist(p)))
            .map(|(i, _)| i)
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

impl Activity for Bubbles {
    fn enter(&mut self, cx: &mut Cx) {
        cx.au.say("¡Burbujas! Haz clic para reventarlas.");
    }

    fn update(&mut self, cx: &mut Cx) {
        cx.g.background(hex(0x4FC3F7), hex(0x1565C0), Backdrop::Underwater, 0.0);
        let dt = cx.dt;
        let t = cx.t;
        self.idle.tick(cx, "¡Busca una burbuja y haz clic!");

        // Burbujitas decorativas de fondo.
        self.ambient_t -= dt;
        if self.ambient_t <= 0.0 {
            self.ambient_t = cx.rng.range(0.1, 0.35);
            let mut p = crate::fx::Particle::new(
                v2(cx.rng.range(0.0, cx.w), cx.h + 10.0),
                v2(0.0, -cx.rng.range(60.0, 140.0)),
                cx.h / 80.0,
                cx.rng.range(3.0, 9.0),
                WHITE.alpha(0.5),
                crate::fx::Look::Shape(Shape::Bubble),
            );
            p.drag = 0.0;
            cx.fx.add(p);
        }

        self.spawn_t -= dt;
        if self.spawn_t <= 0.0 && self.bubbles.len() < Bubbles::max_bubbles(cx) {
            self.spawn(cx, None, None);
            self.spawn_t = cx.rng.range(0.9, 1.6);
        }

        let mouse = cx.inp.mouse;
        let has_mouse = cx.inp.has_mouse;
        for b in self.bubbles.iter_mut() {
            b.age += dt;
            b.pos.y -= b.speed * dt * if b.hover.over { 0.3 } else { 1.0 };
            let over = has_mouse && b.center(t).dist(mouse) < b.r * 1.15;
            if b.hover.update(cx, over) {
                cx.au.sfx_vol(Sfx::Wiggle, 1.0 + b.tint * 0.4, b.pos.x, 0.5);
            }
        }
        self.bubbles.retain(|b| b.pos.y > -b.r * 1.5);

        for b in &self.bubbles {
            let c = b.center(t);
            let wob = b.hover.anim * (t * 18.0).sin() * 0.07;
            let breathe = 1.0 + (t * 2.0 + b.phase).sin() * 0.03;
            let r = b.r * breathe * (1.0 + b.hover.anim * 0.1);
            cx.g.glow(c, r * 1.3, hsv(b.tint, 0.5, 1.0).alpha(0.18));
            cx.g.emoji(b.word.emoji, c, r * 0.6, wob * 2.0, 1.0);
            cx.g.shape_full(Shape::Bubble, c, v2(r * (1.0 + wob), r * (1.0 - wob)), 0.0, hsv(b.tint, 0.15, 1.0), 0.0, 0.0);
        }

        // Animales liberados.
        for f in self.freed.iter_mut() {
            f.t += dt;
        }
        let done: Vec<usize> = self.freed.iter().enumerate().filter(|(_, f)| f.t >= FREED_TIME).map(|(i, _)| i).collect();
        for i in done.into_iter().rev() {
            let f = self.freed.remove(i);
            self.shelf.push(f.word.emoji);
            let (p, _) = Bubbles::shelf_slot(cx, self.shelf.len() - 1);
            cx.fx.sparkle(&mut cx.rng, p, 8, WHITE, 30.0);
            cx.au.note(pentatonic(self.shelf.len() as i32 + 4, 60.0), Inst::Bell, p.x, 0.35);
            if self.shelf.len() >= SHELF {
                self.celebrate(cx);
            }
        }

        // Estante con lo que se ha reventado.
        let (first, sr) = Bubbles::shelf_slot(cx, 0);
        let (last, _) = Bubbles::shelf_slot(cx, SHELF - 1);
        cx.g.rrect(v2((first.x + last.x) * 0.5, first.y), v2((last.x - first.x) * 0.5 + sr * 1.4, sr * 1.3), 1.0, WHITE.alpha(0.25));
        for i in 0..SHELF {
            let (p, r) = Bubbles::shelf_slot(cx, i);
            match self.shelf.get(i) {
                Some(e) => cx.g.emoji(e, p, r, 0.0, 1.0),
                None => cx.g.circle(p, r * 0.25, WHITE.alpha(0.35)),
            }
        }
        for f in &self.freed {
            let k = f.t / FREED_TIME;
            let (slot, sr) = Bubbles::shelf_slot(cx, f.slot);
            let (p, r) = if k < 0.55 {
                let grow = ease_out_back((k / 0.25).min(1.0));
                (f.from - v2(0.0, k * 60.0), f.r * (0.8 + grow * 0.6))
            } else {
                let m = ((k - 0.55) / 0.45).clamp(0.0, 1.0);
                let m = m * m;
                let start = f.from - v2(0.0, 33.0);
                (start.lerp(slot, m), (f.r * 1.4) * (1.0 - m) + sr * m)
            };
            cx.g.emoji_shadow(f.word.emoji, p, r, 0.0, 1.0);
            cx.g.emoji(f.word.emoji, p, r, (f.t * 10.0).sin() * 0.1 * (1.0 - k), 1.0);
        }
    }

    fn down(&mut self, cx: &mut Cx, p: V2, _button: u32) {
        let t = cx.t;
        let hit = self
            .bubbles
            .iter()
            .enumerate()
            .filter(|(_, b)| b.center(t).dist(p) < b.r * 1.2)
            .min_by(|(_, a), (_, b)| a.center(t).dist(p).total_cmp(&b.center(t).dist(p)))
            .map(|(i, _)| i);
        match hit {
            Some(i) => self.pop(cx, i),
            None => {
                cx.fx.ripple(p, 50.0, WHITE.alpha(0.6));
                cx.au.sfx_vol(Sfx::Bloop, 0.9, p.x, 0.4);
            }
        }
    }

    fn key(&mut self, cx: &mut Cx, k: &Key) {
        if let Some(c) = k.letter() {
            // ¿Hay una burbuja con algo que empiece con esa letra?
            if let Some(i) = self.bubbles.iter().position(|b| b.word.first_letter() == c && b.pos.y < cx.h) {
                self.pop(cx, i);
                return;
            }
            if let Some(info) = vocab::letter_info(c) {
                let w = *cx.rng.pick(info.words);
                cx.au.say(info.name);
                let x = cx.rng.range(0.2, 0.8) * cx.w;
                self.spawn(cx, Some(w), Some(x));
                if let Some(b) = self.bubbles.last_mut() {
                    b.speed *= 2.0;
                }
            }
        } else if k.is("Space") {
            if let Some(i) = self.nearest(cx, cx.inp.mouse) {
                self.pop(cx, i);
            }
        } else if k.is_enter() {
            for _ in 0..4 {
                self.spawn(cx, None, None);
            }
            cx.au.sfx(Sfx::Magic, 1.0, cx.w * 0.5);
        } else {
            if let Some(i) = self.nearest(cx, cx.inp.mouse) {
                self.bubbles[i].hover.anim = 1.0;
            }
            cx.au.sfx_vol(Sfx::Bloop, 0.8 + cx.rng.f() * 0.5, cx.w * 0.5, 0.5);
        }
    }
}
