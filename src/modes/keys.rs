//! Teclado mágico: cada tecla hace algo distinto.
//! - Letras: la letra gigante + un dibujo que empieza con ella ("¡Be! ¡Ballena!").
//! - Números: aparecen y se cuentan objetos ("uno, dos, tres... ¡tres manzanas!").
//! - Espacio, Enter, Borrar, flechas...: cada una con su efecto y su nombre.

use super::{Activity, IdlePrompt};
use crate::audio::{Inst, Sfx};
use crate::game::{Cx, Key};
use crate::gfx::{Backdrop, Shape};
use crate::math::{approach, ease_out_back, hsv, pentatonic, v2, Rgba, V2, WHITE};
use crate::vocab::{self, Word};

#[derive(Clone)]
enum Look {
    Text(String, Rgba),
    Emoji(&'static str),
    /// Una forma (barra espaciadora, flechas...).
    Shape(Shape, Rgba, V2),
}

#[derive(Clone)]
struct Item {
    look: Look,
    pos: V2,
    target: V2,
    r: f32,
    target_r: f32,
    age: f32,
    delay: f32,
    rot: f32,
    bounce: f32,
    /// Lo que se dice al hacer clic encima.
    speech: String,
    /// false = protagonista; true = se desvanece cuando `life` llega a 0.
    fading: bool,
    life: f32,
    vel: V2,
}

impl Item {
    fn new(look: Look, pos: V2, r: f32, speech: String) -> Item {
        Item {
            look,
            pos,
            target: pos,
            r: 0.0,
            target_r: r,
            age: 0.0,
            delay: 0.0,
            rot: 0.0,
            bounce: 0.0,
            speech,
            fading: false,
            life: 0.0,
            vel: V2::ZERO,
        }
    }

    fn visible_r(&self) -> f32 {
        let k = ease_out_back(((self.age - self.delay) / 0.45).clamp(0.0, 1.0));
        self.r * k * (1.0 + self.bounce * 0.25 * (self.bounce * 12.0).sin())
    }

    fn alpha(&self) -> f32 {
        if self.fading {
            (self.life / 1.5).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

struct Counting {
    n: u32,
    shown: u32,
    timer: f32,
    word: Word,
}

pub struct Keys {
    items: Vec<Item>,
    counting: Option<Counting>,
    idle: IdlePrompt,
    hue: f32,
    target_hue: f32,
    word_turn: usize,
    pulse: f32,
}

const MAX_ITEMS: usize = 28;

impl Keys {
    pub fn new(cx: &mut Cx) -> Keys {
        Keys {
            items: Vec::new(),
            counting: None,
            idle: IdlePrompt::new(12.0),
            hue: 0.58,
            target_hue: 0.58,
            word_turn: cx.rng.int(5),
            pulse: 0.0,
        }
    }

    /// Los protagonistas actuales pasan a ser recuerdos pequeños.
    fn demote(&mut self, cx: &mut Cx) {
        self.counting = None;
        for it in self.items.iter_mut().filter(|i| !i.fading) {
            it.fading = true;
            it.life = cx.rng.range(5.0, 8.0);
            let margin = 0.1;
            it.target = v2(
                cx.rng.range(margin, 1.0 - margin) * cx.w,
                cx.rng.range(0.15, 0.95) * cx.h,
            );
            it.target_r = it.r * 0.35;
        }
        let extra = self.items.len().saturating_sub(MAX_ITEMS);
        self.items.drain(0..extra);
    }

    fn hero_r(cx: &Cx) -> f32 {
        (cx.unit() * 0.22).min(cx.w * 0.17)
    }

    fn press_letter(&mut self, cx: &mut Cx, c: char) {
        let Some(info) = vocab::letter_info(c) else { return };
        let idx = vocab::LETTERS.iter().position(|l| l.upper == info.upper).unwrap_or(0);
        self.demote(cx);
        self.word_turn += 1;
        let word = info.words[self.word_turn % info.words.len()];
        let r = Keys::hero_r(cx);
        let color = hsv(idx as f32 / 27.0, 0.78, 1.0);
        self.target_hue = idx as f32 / 27.0;

        let lp = v2(cx.w * 0.33, cx.h * 0.52);
        let ep = v2(cx.w * 0.68, cx.h * 0.52);
        let mut letter = Item::new(Look::Text(info.upper.to_string(), color), lp, r, info.name.to_string());
        letter.rot = cx.rng.range(-0.15, 0.15);
        self.items.push(letter);
        let mut pic = Item::new(Look::Emoji(word.emoji), ep, r * 0.8, word_speech(&word));
        pic.delay = 0.25;
        self.items.push(pic);

        cx.fx.confetti(&mut cx.rng, lp, 26, 550.0);
        cx.au.note(pentatonic(idx as i32 % 10 + 5, 60.0), Inst::Marimba, lp.x, 0.6);
        if cx.au.since_say() > 0.35 || !cx.au.speaking {
            cx.au.say(info.name);
        }
        cx.au.say_after(0.9, &word_speech(&word));
    }

    fn press_digit(&mut self, cx: &mut Cx, d: u32) {
        self.demote(cx);
        let r = Keys::hero_r(cx);
        let color = hsv(d as f32 / 10.0 + 0.05, 0.8, 1.0);
        self.target_hue = d as f32 / 10.0;
        let dp = v2(cx.w * 0.18, cx.h * 0.52);
        let text = d.to_string();
        self.items.push(Item::new(Look::Text(text, color), dp, r * 0.9, vocab::NUMBERS[d as usize].to_string()));
        cx.fx.confetti(&mut cx.rng, dp, 20, 450.0);
        cx.au.note(pentatonic(d as i32 + 3, 60.0), Inst::Bell, dp.x, 0.6);
        let word = *cx.rng.pick(vocab::COUNTABLES);
        if d == 0 {
            cx.au.say("¡Cero! No hay nada.");
            cx.fx.ripple(v2(cx.w * 0.62, cx.h * 0.52), r, WHITE.alpha(0.8));
            return;
        }
        cx.au.say(&format!("¡{}!", vocab::NUMBERS[d as usize]));
        self.counting = Some(Counting { n: d, shown: 0, timer: 0.9, word });
    }

    /// Posición del objeto `i` de `n` en una grilla a la derecha.
    fn count_slot(cx: &Cx, i: u32, n: u32) -> (V2, f32) {
        let cols = if n <= 3 { n } else if n <= 6 { 3 } else { (n as f32 / 2.0).ceil() as u32 };
        let rows = n.div_ceil(cols);
        let (x0, x1) = (cx.w * 0.36, cx.w * 0.95);
        let (y0, y1) = (cx.h * 0.22, cx.h * 0.85);
        let cw = (x1 - x0) / cols as f32;
        let ch = (y1 - y0) / rows as f32;
        let (c, r) = (i % cols, i / cols);
        // Centrar la última fila si está incompleta.
        let in_row = if r == rows - 1 { n - r * cols } else { cols };
        let offset = (cols - in_row) as f32 * cw * 0.5;
        let p = v2(x0 + offset + (c as f32 + 0.5) * cw, y0 + (r as f32 + 0.5) * ch);
        (p, (cw.min(ch) * 0.38).min(cx.unit() * 0.12))
    }

    fn tick_counting(&mut self, cx: &mut Cx) {
        let Some(c) = self.counting.as_mut() else { return };
        c.timer -= cx.dt;
        if c.timer > 0.0 {
            return;
        }
        if c.shown < c.n {
            let (p, r) = Keys::count_slot(cx, c.shown, c.n);
            c.shown += 1;
            let mut it = Item::new(Look::Emoji(c.word.emoji), p, r, c.word.word.to_string());
            it.rot = cx.rng.range(-0.2, 0.2);
            self.items.push(it);
            let col = hsv(cx.rng.f(), 0.6, 1.0);
            cx.fx.sparkle(&mut cx.rng, p, 8, col, 40.0);
            cx.au.note(pentatonic(c.shown as i32 + 4, 60.0), Inst::Marimba, p.x, 0.5);
            cx.au.say(vocab::NUMBERS[c.shown as usize]);
            c.timer = 0.85;
        } else {
            let phrase = format!("¡{}!", vocab::count_phrase(c.n, &c.word));
            cx.au.say(&phrase);
            cx.au.sfx(Sfx::Sparkle, 1.0, cx.w * 0.6);
            let p = v2(cx.w * 0.65, cx.h * 0.5);
            cx.fx.confetti(&mut cx.rng, p, 30, 600.0);
            self.counting = None;
        }
    }

    fn special(&mut self, cx: &mut Cx, k: &Key) {
        let r = Keys::hero_r(cx);
        if let Some(name) = vocab::key_name(k.code) {
            cx.au.say(name);
        }
        if k.is("Space") {
            self.demote(cx);
            let p = v2(cx.w * 0.5, cx.h * 0.78);
            let bar = Item::new(Look::Shape(Shape::RoundRect, hsv(cx.rng.f(), 0.5, 1.0), v2(4.0, 0.7)), p, r * 0.55, "espacio".into());
            self.items.push(bar);
            for _ in 0..14 {
                let mut b = crate::fx::Particle::new(
                    v2(cx.rng.range(0.1, 0.9) * cx.w, cx.h + 40.0),
                    v2(cx.rng.range(-30.0, 30.0), -cx.rng.range(150.0, 400.0)),
                    cx.rng.range(2.0, 3.5),
                    cx.rng.range(20.0, 45.0),
                    WHITE,
                    crate::fx::Look::Shape(Shape::Bubble),
                );
                b.drag = 0.2;
                cx.fx.add(b);
            }
            cx.au.sfx(Sfx::Boing, 1.0, p.x);
        } else if k.is_enter() {
            for i in 0..3 {
                let x = cx.w * (0.25 + 0.25 * i as f32);
                cx.fx.rocket(&mut cx.rng, v2(x, cx.h), cx.h * 0.55);
            }
            cx.au.sfx(Sfx::Magic, 1.0, cx.w * 0.5);
        } else if k.is_erase() {
            // ¡Borrar! Todo sale volando.
            self.counting = None;
            let c = cx.center();
            for it in self.items.iter_mut() {
                let dir = (it.pos - c).norm();
                let dir = if dir == V2::ZERO { v2(1.0, 0.0) } else { dir };
                it.vel = dir * 1400.0;
                it.fading = true;
                it.life = 1.0;
            }
            cx.au.sfx(Sfx::Whoosh, 1.0, cx.w * 0.5);
        } else if let Some(dir) = k.arrow() {
            let c = cx.center();
            let mut a = Item::new(Look::Shape(Shape::Arrow, hsv(cx.rng.f(), 0.75, 1.0), v2(1.0, 0.7)), c, r * 0.6, String::new());
            a.rot = dir.angle();
            a.vel = dir * 900.0;
            a.fading = true;
            a.life = 1.2;
            self.items.push(a);
            // Todo lo demás se mueve un poquito en esa dirección.
            for it in self.items.iter_mut() {
                it.target += dir * cx.unit() * 0.06;
            }
            cx.au.sfx(Sfx::Whoosh, 1.4, c.x + dir.x * cx.w * 0.3);
        } else if k.code.starts_with("Shift") || k.is("CapsLock") {
            for it in self.items.iter_mut() {
                it.bounce = 1.0;
            }
            cx.au.sfx(Sfx::Boing, 1.3, cx.w * 0.5);
        } else {
            // Cualquier otra tecla: una onda de colores en algún lugar.
            let p = cx.rng.point(cx.w * 0.15, cx.h * 0.2, cx.w * 0.85, cx.h * 0.85);
            let c = hsv(cx.rng.f(), 0.7, 1.0);
            cx.fx.ripple(p, 120.0, c);
            cx.fx.confetti(&mut cx.rng, p, 16, 400.0);
            cx.au.note(pentatonic(cx.rng.int(10) as i32 + 3, 60.0), Inst::Bell, p.x, 0.5);
        }
    }

    fn press_symbol(&mut self, cx: &mut Cx, glyph: &'static str, name: &'static str) {
        self.demote(cx);
        let r = Keys::hero_r(cx);
        let p = v2(cx.w * 0.5, cx.h * 0.52);
        let color = hsv(cx.rng.f(), 0.7, 1.0);
        self.items.push(Item::new(Look::Text(glyph.to_string(), color), p, r, name.to_string()));
        cx.fx.confetti(&mut cx.rng, p, 18, 450.0);
        cx.au.note(pentatonic(cx.rng.int(10) as i32 + 3, 60.0), Inst::Pluck, p.x, 0.5);
        cx.au.say(&format!("¡{}!", name));
    }

    fn draw_item(cx: &mut Cx, it: &Item) {
        let r = it.visible_r();
        if r <= 0.5 {
            return;
        }
        let a = it.alpha();
        let wob = (cx.t * 2.0 + it.pos.x * 0.01).sin() * 0.05;
        let rot = it.rot + wob;
        match &it.look {
            Look::Text(s, c) => {
                cx.g.text(s, it.pos + v2(r * 0.03, r * 0.1), r, rot, Rgba { r: 0.1, g: 0.05, b: 0.3, a: 0.22 * a }, 0.0);
                cx.g.text(s, it.pos, r, rot, c.alpha(a), 0.14);
            }
            Look::Emoji(e) => {
                cx.g.emoji_shadow(e, it.pos, r, rot, a);
                cx.g.emoji(e, it.pos, r, rot, a);
            }
            Look::Shape(s, c, aspect) => {
                let half = v2(r * aspect.x, r * aspect.y);
                if *s == Shape::RoundRect {
                    // Tecla: base oscura abajo, cara clara arriba.
                    let depth = v2(0.0, half.y * 0.35);
                    cx.g.shape_full(*s, it.pos + depth, half + v2(5.0, 5.0), it.rot, c.darker(0.45).alpha(a), 0.35, 0.0);
                    cx.g.shape_full(*s, it.pos, half + v2(5.0, 5.0), it.rot, c.darker(0.3).alpha(a), 0.35, 0.0);
                    cx.g.shape_full(*s, it.pos, half, it.rot, c.alpha(a), 0.35, 0.0);
                    cx.g.shape_full(*s, it.pos - v2(0.0, half.y * 0.3), v2(half.x * 0.92, half.y * 0.45), it.rot, WHITE.alpha(0.25 * a), 1.0, 3.0);
                } else {
                    cx.g.shape_full(*s, it.pos, half + v2(5.0, 5.0), it.rot, c.darker(0.35).alpha(a), 0.35, 0.0);
                    cx.g.shape_full(*s, it.pos, half, it.rot, c.alpha(a), 0.35, 0.0);
                }
            }
        }
    }
}

/// "¡Perro! ¡Guau, guau!" o "¡Manzana!"
fn word_speech(w: &Word) -> String {
    let mut s = String::from("¡");
    let mut chars = w.word.chars();
    if let Some(f) = chars.next() {
        s.extend(f.to_uppercase());
        s.push_str(chars.as_str());
    }
    s.push('!');
    if !w.sound.is_empty() {
        s.push(' ');
        s.push_str(w.sound);
    }
    s
}

impl Activity for Keys {
    fn enter(&mut self, cx: &mut Cx) {
        cx.au.say("¡Teclado mágico! Aprieta una tecla.");
    }

    fn update(&mut self, cx: &mut Cx) {
        self.hue = approach(self.hue, self.target_hue, 2.0, cx.dt);
        cx.g.background(hsv(self.hue, 0.32, 1.0), hsv(self.hue + 0.12, 0.22, 1.0), Backdrop::Bokeh, 1.0);

        if self.idle.tick(cx, "¡Aprieta una tecla!") {
            self.pulse = 1.0;
        }
        self.tick_counting(cx);

        let dt = cx.dt;
        for it in self.items.iter_mut() {
            it.age += dt;
            it.bounce = (it.bounce - dt * 1.5).max(0.0);
            if it.vel != V2::ZERO {
                it.pos += it.vel * dt;
                it.target = it.pos;
            } else {
                it.pos = it.pos.lerp(it.target, 1.0 - (-3.0 * dt).exp());
            }
            it.r = approach(it.r, it.target_r, 3.0, dt);
            if it.fading {
                it.life -= dt;
            }
        }
        self.items.retain(|it| !it.fading || it.life > 0.0);
        // Primero los recuerdos, encima los protagonistas.
        for it in self.items.iter().filter(|i| i.fading) {
            Keys::draw_item(cx, it);
        }
        for it in self.items.iter().filter(|i| !i.fading) {
            Keys::draw_item(cx, it);
        }

        // Pista: un teclado que late cuando no pasa nada.
        if self.idle.waiting(cx) || self.items.is_empty() {
            let k = 1.0 + 0.08 * (cx.t * 4.0).sin();
            let p = v2(cx.w * 0.5, cx.h * 0.55);
            cx.g.emoji_shadow("⌨️", p, cx.unit() * 0.16 * k, 0.0, 1.0);
            cx.g.emoji("⌨️", p, cx.unit() * 0.16 * k, 0.0, 1.0);
        }
        self.pulse = (self.pulse - dt).max(0.0);
    }

    fn key(&mut self, cx: &mut Cx, k: &Key) {
        if let Some(c) = k.letter() {
            self.press_letter(cx, c);
        } else if let Some(d) = k.digit() {
            self.press_digit(cx, d);
        } else if let Some((g, name)) = k.ch.and_then(vocab::symbol_name) {
            self.press_symbol(cx, g, name);
        } else {
            self.special(cx, k);
        }
    }

    fn down(&mut self, cx: &mut Cx, p: V2, button: u32) {
        // ¿Clic sobre una letra o dibujo? Salta y se nombra.
        if let Some(it) = self.items.iter_mut().rev().find(|it| it.vel == V2::ZERO && p.dist(it.pos) < it.visible_r().max(20.0)) {
            it.bounce = 1.0;
            if !it.speech.is_empty() {
                let s = it.speech.clone();
                cx.au.say(&s);
            }
            cx.au.sfx(Sfx::Boing, 1.0 + cx.rng.range(-0.1, 0.1), p.x);
            let col = hsv(cx.rng.f(), 0.6, 1.0);
            cx.fx.sparkle(&mut cx.rng, p, 10, col, 50.0);
            return;
        }
        let c = hsv(cx.rng.f(), 0.75, 1.0);
        match button {
            2 => {
                // Botón derecho: corazones.
                for _ in 0..8 {
                    let mut h = crate::fx::Particle::new(p, cx.rng.dir() * cx.rng.range(80.0, 260.0), 1.2, cx.rng.range(10.0, 18.0), hsv(0.95 + cx.rng.range(-0.04, 0.04), 0.7, 1.0), crate::fx::Look::Shape(Shape::Heart));
                    h.drag = 2.0;
                    h.gravity = -60.0;
                    cx.fx.add(h);
                }
                cx.au.sfx(Sfx::Bloop, 0.8, p.x);
            }
            1 => {
                cx.fx.ripple(p, 90.0, c);
                cx.fx.ripple(p, 140.0, c.lighter(0.3));
                cx.au.sfx(Sfx::Magic, 1.2, p.x);
            }
            _ => {
                cx.fx.confetti(&mut cx.rng, p, 18, 420.0);
                cx.au.note(pentatonic(cx.rng.int(10) as i32 + 5, 60.0), Inst::Bell, p.x, 0.45);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_capitalizes_and_adds_sound() {
        assert_eq!(word_speech(&vocab::PERRO), "¡Perro! ¡Guau, guau!");
        assert_eq!(word_speech(&vocab::MANZANA), "¡Manzana!");
    }
}
