//! Sumar: primero contar ("¿Cuántas manzanas hay?"), después sumas
//! pequeñas con objetos ("dos manzanas más una manzana, ¿cuántas son?").
//! Se responde con un clic en un número (cada uno trae sus puntitos) o con
//! las teclas de números. Si se equivoca, el juego cuenta con ella los
//! objetos uno por uno y vuelve a preguntar: contar es el camino a sumar.

use super::{Activity, Hover, IdlePrompt};
use crate::audio::{Inst, Sfx};
use crate::game::{Cx, Key};
use crate::gfx::{Backdrop, Shape};
use crate::math::{ease_out_back, hex, hsv, pentatonic, v2, Rgba, V2, WHITE};
use crate::vocab::{self, Word};

struct Obj {
    pos: V2,
    target: V2,
    bounce: f32,
}

struct Choice {
    n: u32,
    pos: V2,
    r: f32,
    hover: Hover,
    shake: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum After {
    Ask,
    Celebrate,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Asking,
    /// Contando objeto por objeto en voz alta.
    Counting { i: usize, timer: f32, then: After },
    Won(f32),
}

pub struct Adding {
    a: u32,
    b: u32,
    word: Word,
    objs: Vec<Obj>,
    choices: Vec<Choice>,
    level: u32,
    streak: u32,
    phase: Phase,
    ask_in: Option<f32>,
    idle: IdlePrompt,
    speaker: Hover,
    speaker_pulse: f32,
    appear: f32,
    merged: bool,
    tap_count: u32,
    tap_timer: f32,
}

impl Adding {
    pub fn new(cx: &mut Cx) -> Adding {
        let mut s = Adding {
            a: 1,
            b: 0,
            word: vocab::MANZANA,
            objs: Vec::new(),
            choices: Vec::new(),
            level: 0,
            streak: 0,
            phase: Phase::Asking,
            ask_in: None,
            idle: IdlePrompt::new(12.0),
            speaker: Hover::default(),
            speaker_pulse: 0.0,
            appear: 0.0,
            merged: false,
            tap_count: 0,
            tap_timer: 0.0,
        };
        s.new_round(cx);
        s
    }

    fn total(&self) -> u32 {
        self.a + self.b
    }

    fn adding(&self) -> bool {
        self.b > 0
    }

    /// Tamaño de los objetos: grandes si son pocos.
    fn obj_r(&self, cx: &Cx) -> f32 {
        let k = if self.total() <= 5 { 0.075 } else { 0.05 };
        (cx.unit() * k).min(cx.w * 0.03)
    }

    /// Posiciones en grupitos (como en un dado), centrados en `c`.
    fn group(c: V2, n: u32, r: f32, max_per_row: u32) -> Vec<V2> {
        let per_row = if n <= 3 { n.max(1) } else if n <= 6 { 3 } else { max_per_row };
        let rows = n.div_ceil(per_row);
        let gap = r * 2.3;
        (0..n)
            .map(|i| {
                let row = i / per_row;
                let in_row = if row == rows - 1 { n - row * per_row } else { per_row };
                let col = i % per_row;
                let x = c.x + (col as f32 - (in_row as f32 - 1.0) * 0.5) * gap;
                let y = c.y + (row as f32 - (rows as f32 - 1.0) * 0.5) * gap;
                v2(x, y)
            })
            .collect()
    }

    fn new_round(&mut self, cx: &mut Cx) {
        let (a, b) = match self.level {
            0 => (cx.rng.int(5) as u32 + 1, 0),
            1 => {
                let a = cx.rng.int(4) as u32 + 1;
                (a, cx.rng.int((5 - a) as usize) as u32 + 1)
            }
            _ => {
                let a = cx.rng.int(9) as u32 + 1;
                (a, cx.rng.int((10 - a) as usize) as u32 + 1)
            }
        };
        self.a = a;
        self.b = b;
        self.word = *cx.rng.pick(vocab::COUNTABLES);
        self.merged = false;
        self.appear = 0.0;
        self.tap_count = 0;
        self.layout_objs(cx);
        for o in self.objs.iter_mut() {
            o.pos = o.target;
        }
        // Opciones: la correcta y dos vecinas.
        let ans = self.total();
        let max = if self.level == 0 { 6 } else { 10 };
        let mut opts = vec![ans];
        let mut near: Vec<u32> = [ans.saturating_sub(1), ans + 1, ans.saturating_sub(2), ans + 2]
            .into_iter()
            .filter(|&n| n >= 1 && n <= max && n != ans)
            .collect();
        cx.rng.shuffle(&mut near);
        opts.extend(near.into_iter().take(2));
        cx.rng.shuffle(&mut opts);
        self.choices = opts.into_iter().map(|n| Choice { n, pos: V2::ZERO, r: 1.0, hover: Hover::default(), shake: 0.0 }).collect();
        self.layout_choices(cx);
        self.phase = Phase::Asking;
        self.ask_in = Some(0.8);
    }

    fn layout_objs(&mut self, cx: &Cx) {
        let r = self.obj_r(cx);
        let y = cx.h * 0.4;
        let mut targets = Vec::new();
        if !self.adding() || self.merged {
            targets.extend(Adding::group(v2(cx.w * 0.5, y), self.total(), r, 5));
        } else {
            targets.extend(Adding::group(v2(cx.w * 0.25, y), self.a, r, 3));
            targets.extend(Adding::group(v2(cx.w * 0.6, y), self.b, r, 3));
        }
        if self.objs.len() != targets.len() {
            self.objs = targets.iter().map(|&t| Obj { pos: t, target: t, bounce: 0.0 }).collect();
        }
        for (o, t) in self.objs.iter_mut().zip(targets) {
            o.target = t;
        }
    }

    fn layout_choices(&mut self, cx: &Cx) {
        let n = self.choices.len() as f32;
        let r = (cx.unit() * 0.11).min(cx.w / (n * 2.8));
        for (i, c) in self.choices.iter_mut().enumerate() {
            c.pos = v2(cx.w * 0.5 + (i as f32 - (n - 1.0) * 0.5) * r * 2.8, cx.h * 0.8);
            c.r = r;
        }
    }

    fn question(&self) -> String {
        let cuantos = match self.word.gender {
            vocab::Gender::M => "Cuántos",
            vocab::Gender::F => "Cuántas",
        };
        if self.adding() {
            format!(
                "{} más {}. ¿{} son?",
                capitalize(&vocab::count_phrase(self.a, &self.word)),
                vocab::count_phrase(self.b, &self.word),
                cuantos
            )
        } else {
            format!("¿{} {} hay?", cuantos, self.word.plural)
        }
    }

    fn ask(&mut self, cx: &mut Cx) {
        let q = self.question();
        cx.au.say(&q);
        self.speaker_pulse = 1.0;
    }

    fn answer(&mut self, cx: &mut Cx, n: u32, at: V2) {
        if self.phase != Phase::Asking {
            return;
        }
        self.ask_in = None;
        if n == self.total() {
            self.streak += 1;
            if self.streak >= 3 && self.level < 2 {
                self.streak = 0;
                self.level += 1;
            }
            cx.au.sfx(Sfx::Sparkle, 1.0, at.x);
            cx.fx.confetti(&mut cx.rng, at, 24, 450.0);
            // Juntar los grupos y contar todo.
            self.merged = true;
            self.layout_objs(cx);
            self.phase = Phase::Counting { i: 0, timer: 0.9, then: After::Celebrate };
            cx.au.say(&format!("¡Sí! ¡{}! Contemos.", capitalize(vocab::NUMBERS[n as usize])));
        } else {
            self.streak = 0;
            if let Some(c) = self.choices.iter_mut().find(|c| c.n == n) {
                c.shake = 1.0;
            }
            cx.au.sfx(Sfx::Oops, 1.0, at.x);
            let name = vocab::NUMBERS.get(n as usize).copied().unwrap_or("otro");
            cx.au.say(&format!("Ese es el {}. ¡Contemos juntos!", name));
            self.phase = Phase::Counting { i: 0, timer: 1.8, then: After::Ask };
        }
    }

    fn tick_phase(&mut self, cx: &mut Cx) {
        let dt = cx.dt;
        match self.phase {
            Phase::Counting { i, timer, then } => {
                let timer = timer - dt;
                if timer > 0.0 {
                    self.phase = Phase::Counting { i, timer, then };
                    return;
                }
                if i < self.objs.len() {
                    let o = &mut self.objs[i];
                    o.bounce = 1.0;
                    let p = o.pos;
                    cx.au.say(vocab::NUMBERS[(i + 1).min(10)]);
                    cx.au.note(pentatonic(i as i32 + 4, 60.0), Inst::Marimba, p.x, 0.5);
                    let c = hsv(i as f32 * 0.1, 0.6, 1.0);
                    cx.fx.sparkle(&mut cx.rng, p, 6, c, 30.0);
                    self.phase = Phase::Counting { i: i + 1, timer: 0.8, then };
                } else if then == After::Celebrate {
                    let n = self.total();
                    let phrase = if self.adding() {
                        format!(
                            "¡{} más {} son {}!",
                            capitalize(vocab::NUMBERS[self.a as usize]),
                            vocab::NUMBERS[self.b as usize],
                            vocab::NUMBERS[n as usize]
                        )
                    } else {
                        format!("¡Hay {}!", vocab::count_phrase(n, &self.word))
                    };
                    cx.au.say(&phrase);
                    cx.au.sfx(Sfx::Fanfare, 1.0, cx.w * 0.5);
                    for k in 0..3 {
                        cx.fx.rocket(&mut cx.rng, v2(cx.w * (0.25 + 0.25 * k as f32), cx.h), cx.h * 0.55);
                    }
                    self.phase = Phase::Won(3.0);
                } else {
                    self.phase = Phase::Asking;
                    self.ask_in = Some(0.5);
                }
            }
            Phase::Won(t) => {
                let t = t - dt;
                if t <= 0.0 {
                    self.new_round(cx);
                } else {
                    self.phase = Phase::Won(t);
                }
            }
            Phase::Asking => {}
        }
    }

    fn speaker_pos(cx: &Cx) -> (V2, f32) {
        let r = (cx.unit() * 0.055).clamp(28.0, 48.0);
        (v2(cx.w * 0.5, r + 18.0), r)
    }

    /// Puntitos como los de un dado, para quien todavía no lee números.
    fn dots(cx: &mut Cx, c: V2, r: f32, n: u32, color: Rgba) {
        let per_row = 5;
        let rows = n.div_ceil(per_row);
        let dot = r * 0.085;
        let gap = dot * 2.6;
        for i in 0..n {
            let row = i / per_row;
            let in_row = if row == rows - 1 { n - row * per_row } else { per_row };
            let col = i % per_row;
            let x = c.x + (col as f32 - (in_row as f32 - 1.0) * 0.5) * gap;
            let y = c.y + r * 0.5 + (row as f32 - (rows as f32 - 1.0) * 0.5) * gap;
            cx.g.circle(v2(x, y), dot, color);
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

impl Activity for Adding {
    fn enter(&mut self, cx: &mut Cx) {
        cx.au.say("¡A contar y sumar!");
        self.ask_in = Some(1.4);
    }

    fn resize(&mut self, cx: &mut Cx) {
        self.layout_objs(cx);
        self.layout_choices(cx);
    }

    fn update(&mut self, cx: &mut Cx) {
        cx.g.background(hex(0xE8F5E9), hex(0xFFF3E0), Backdrop::Bokeh, 0.0);
        let dt = cx.dt;
        let t = cx.t;
        self.appear += dt;
        self.tap_timer -= dt;
        if self.tap_timer <= 0.0 {
            self.tap_count = 0;
        }
        if let Some(a) = self.ask_in.as_mut() {
            *a -= dt;
            if *a <= 0.0 {
                self.ask_in = None;
                self.ask(cx);
            }
        }
        if self.phase == Phase::Asking {
            let q = self.question();
            self.idle.tick(cx, &q);
        }
        self.tick_phase(cx);

        // Botón 🔊.
        let (sp, sr) = Adding::speaker_pos(cx);
        if self.speaker.update(cx, cx.inp.has_mouse && cx.inp.mouse.dist(sp) < sr * 1.1) {
            cx.au.sfx_vol(Sfx::Tick, 1.3, sp.x, 0.5);
        }
        self.speaker_pulse = (self.speaker_pulse - dt * 0.8).max(0.0);
        let srr = sr * (1.0 + 0.15 * self.speaker.anim + self.speaker_pulse * (t * 12.0).sin().abs() * 0.12);
        cx.g.shadow(Shape::Circle, sp, srr, 0.0, 1.0);
        cx.g.sticker(Shape::Circle, sp, srr, 0.0, WHITE.alpha(0.95));
        cx.g.emoji("🔊", sp, srr * 0.6, 0.0, 1.0);

        // Objetos.
        let r = self.obj_r(cx);
        for (i, o) in self.objs.iter_mut().enumerate() {
            o.pos = o.pos.lerp(o.target, 1.0 - (-5.0 * dt).exp());
            o.bounce = (o.bounce - dt * 2.0).max(0.0);
            let k = ease_out_back(((self.appear - i as f32 * 0.07) / 0.4).clamp(0.0, 1.0));
            let jump = (o.bounce * std::f32::consts::PI).sin() * r * 0.6;
            let p = o.pos - v2(0.0, jump);
            let rr = r * k * (1.0 + 0.2 * o.bounce);
            cx.g.emoji_shadow(self.word.emoji, p, rr, 0.0, 1.0);
            cx.g.emoji(self.word.emoji, p, rr, (t * 2.0 + i as f32).sin() * 0.06, 1.0);
        }

        // Signos + = ?
        let sign_r = r * 1.1;
        let y = cx.h * 0.4;
        let ink = hex(0x37474F);
        if self.adding() && !self.merged {
            cx.g.text("+", v2(cx.w * 0.425, y), sign_r, 0.0, hex(0xFF7043), 0.12);
            cx.g.text("=", v2(cx.w * 0.78, y), sign_r, 0.0, hex(0x42A5F5), 0.12);
            let q = v2(cx.w * 0.89, y);
            cx.g.shape_full(Shape::RoundRect, q, v2(sign_r, sign_r * 1.15), 0.0, WHITE, 0.3, 0.0);
            cx.g.ring(q, sign_r * 1.25, 3.0, ink.alpha(0.3));
            let shown = if matches!(self.phase, Phase::Won(_)) { self.total().to_string() } else { "?".to_string() };
            cx.g.text(&shown, q, sign_r * 0.9, 0.0, hex(0x8E24AA), 0.1);
        } else if self.adding() && matches!(self.phase, Phase::Won(_)) {
            let txt = format!("{}+{}={}", self.a, self.b, self.total());
            let n = txt.chars().count() as f32;
            for (i, ch) in txt.chars().enumerate() {
                let x = cx.w * 0.5 + (i as f32 - (n - 1.0) * 0.5) * sign_r * 1.5;
                let s = ch.to_string();
                cx.g.text(&s, v2(x, cx.h * 0.62), sign_r * 0.8, 0.0, hsv(i as f32 * 0.15, 0.7, 0.95), 0.12);
            }
        }

        // Respuestas.
        let mouse = cx.inp.mouse;
        let has_mouse = cx.inp.has_mouse;
        let asking = self.phase == Phase::Asking;
        for (i, c) in self.choices.iter_mut().enumerate() {
            c.shake = (c.shake - dt * 1.8).max(0.0);
            if c.hover.update(cx, has_mouse && mouse.dist(c.pos) < c.r) && asking {
                cx.au.sfx_vol(Sfx::Tick, 0.9 + i as f32 * 0.15, c.pos.x, 0.4);
            }
        }
        for (i, c) in self.choices.iter().enumerate() {
            let k = ease_out_back(((self.appear - 0.3 - i as f32 * 0.1) / 0.4).clamp(0.0, 1.0));
            let shake = (c.shake * 30.0).sin() * c.shake * c.r * 0.12;
            let p = c.pos + v2(shake, 0.0);
            let rr = c.r * k * (1.0 + 0.1 * c.hover.anim);
            let color = hsv(0.08 + i as f32 * 0.3, 0.55, 1.0);
            cx.g.shadow(Shape::Circle, p, rr, 0.0, 1.0);
            cx.g.sticker(Shape::Circle, p, rr, 0.0, color);
            let s = c.n.to_string();
            cx.g.text(&s, p - v2(0.0, rr * 0.14), rr * 0.55, 0.0, WHITE, 0.14);
            Adding::dots(cx, p, rr, c.n, ink.alpha(0.75));
        }
    }

    fn down(&mut self, cx: &mut Cx, p: V2, _button: u32) {
        let (sp, sr) = Adding::speaker_pos(cx);
        if p.dist(sp) < sr * 1.2 {
            if self.phase == Phase::Asking {
                self.ask(cx);
            }
            return;
        }
        if let Some(n) = self.choices.iter().find(|c| p.dist(c.pos) < c.r * 1.1).map(|c| c.n) {
            self.answer(cx, n, p);
            return;
        }
        // Tocar los objetos para contarlos con el dedo (o el mouse).
        let r = self.obj_r(cx);
        if self.phase == Phase::Asking {
            if let Some(o) = self.objs.iter_mut().find(|o| o.pos.dist(p) < r * 1.2) {
                if o.bounce < 0.3 {
                    o.bounce = 1.0;
                    self.tap_count = (self.tap_count % 10) + 1;
                    self.tap_timer = 3.0;
                    cx.au.say(vocab::NUMBERS[self.tap_count as usize]);
                    cx.au.note(pentatonic(self.tap_count as i32 + 4, 60.0), Inst::Marimba, p.x, 0.4);
                }
                return;
            }
        }
        let c = hsv(cx.rng.f(), 0.6, 1.0);
        cx.fx.sparkle(&mut cx.rng, p, 8, c, 40.0);
    }

    fn key(&mut self, cx: &mut Cx, k: &Key) {
        if let Some(d) = k.digit() {
            // "1" y "0" seguidos no se distinguen de 10: el 0 vale como 10.
            let n = if d == 0 { 10 } else { d };
            let at = self.choices.iter().find(|c| c.n == n).map(|c| c.pos).unwrap_or(cx.center());
            self.answer(cx, n, at);
        } else if (k.is("Space") || k.is_enter()) && self.phase == Phase::Asking {
            self.ask(cx);
        } else {
            let x = cx.rng.range(0.2, 0.8) * cx.w;
            cx.au.note(pentatonic(cx.rng.int(8) as i32 + 4, 60.0), Inst::Soft, x, 0.3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    #[test]
    fn sums_stay_small_and_choices_include_answer() {
        let mut g = Game::new(1280.0, 720.0, 4);
        let mut s = Adding::new(&mut g.cx);
        for level in 0..3 {
            s.level = level;
            for _ in 0..50 {
                s.new_round(&mut g.cx);
                let max = if level == 0 { 5 } else if level == 1 { 5 } else { 10 };
                assert!(s.total() >= 1 && s.total() <= max, "nivel {level}: {}+{}", s.a, s.b);
                assert!(s.choices.iter().any(|c| c.n == s.total()));
                assert_eq!(s.objs.len() as u32, s.total());
            }
        }
    }

    #[test]
    fn question_grammar() {
        let mut g = Game::new(1280.0, 720.0, 4);
        let mut s = Adding::new(&mut g.cx);
        s.a = 2;
        s.b = 1;
        s.word = vocab::MANZANA;
        assert_eq!(s.question(), "Dos manzanas más una manzana. ¿Cuántas son?");
        s.b = 0;
        s.word = vocab::PERRO;
        assert_eq!(s.question(), "¿Cuántos perros hay?");
    }
}
