//! Formas y colores: la voz pregunta "¿Dónde está el círculo rojo?" y la
//! niña hace clic. Si se equivoca no pasa nada malo: la voz le dice qué
//! tocó ("Ese es el cuadrado azul") y vuelve a preguntar. La dificultad sube
//! sola: primero sólo cambia el color, después la forma, después ambos.

use super::{Activity, Hover, IdlePrompt};
use crate::audio::{Inst, Sfx};
use crate::game::{Cx, Key};
use crate::gfx::{Backdrop, Shape};
use crate::math::{ease_out_back, ease_out_elastic, hex, hsv, midi, v2, V2, WHITE};
use crate::vocab::{self, Gender};

struct Piece {
    shape: usize,
    color: usize,
    pos: V2,
    r: f32,
    hover: Hover,
    shake: f32,
    spin: f32,
    appear: f32,
    delay: f32,
    leave: Option<V2>,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Playing,
    /// Celebrando el acierto; al terminar, nueva ronda.
    Won(f32),
}

pub struct Shapes {
    pieces: Vec<Piece>,
    target: usize,
    level: u32,
    streak: u32,
    wrongs: u32,
    state: State,
    ask_in: Option<f32>,
    idle: IdlePrompt,
    wrong_cooldown: f32,
    /// Botón 🔊 para escuchar la pregunta otra vez.
    speaker: Hover,
    speaker_pulse: f32,
}

impl Shapes {
    pub fn new(cx: &mut Cx) -> Shapes {
        let mut s = Shapes {
            pieces: Vec::new(),
            target: 0,
            level: 0,
            streak: 0,
            wrongs: 0,
            state: State::Playing,
            ask_in: None,
            idle: IdlePrompt::new(10.0),
            wrong_cooldown: 0.0,
            speaker: Hover::default(),
            speaker_pulse: 0.0,
        };
        s.new_round(cx);
        s
    }

    fn question(&self) -> String {
        let p = &self.pieces[self.target];
        format!("¿Dónde está {}?", vocab::shape_phrase(&vocab::SHAPES[p.shape], &vocab::GAME_COLORS[p.color]))
    }

    fn describe(&self, i: usize) -> String {
        let p = &self.pieces[i];
        let s = &vocab::SHAPES[p.shape];
        let this = if s.gender == Gender::F { "Esa es" } else { "Ese es" };
        format!("{} {}.", this, vocab::shape_phrase(s, &vocab::GAME_COLORS[p.color]))
    }

    fn new_round(&mut self, cx: &mut Cx) {
        let n = match self.level {
            0 | 1 => 3,
            2 => 4,
            3 => 5,
            _ => 6,
        };
        let ns = vocab::SHAPES.len();
        let nc = vocab::GAME_COLORS.len();
        let mut combos: Vec<(usize, usize)> = Vec::new();
        match self.level {
            0 => {
                // Misma forma, distintos colores.
                let s = cx.rng.int(ns);
                let mut cs: Vec<usize> = (0..nc).collect();
                cx.rng.shuffle(&mut cs);
                combos.extend(cs.iter().take(n).map(|&c| (s, c)));
            }
            1 => {
                // Mismo color, distintas formas.
                let c = cx.rng.int(nc);
                let mut ss: Vec<usize> = (0..ns).collect();
                cx.rng.shuffle(&mut ss);
                combos.extend(ss.iter().take(n).map(|&s| (s, c)));
            }
            _ => {
                while combos.len() < n {
                    let c = (cx.rng.int(ns), cx.rng.int(nc));
                    if !combos.contains(&c) {
                        combos.push(c);
                    }
                }
            }
        }
        self.pieces = combos
            .into_iter()
            .enumerate()
            .map(|(i, (shape, color))| Piece {
                shape,
                color,
                pos: V2::ZERO,
                r: 1.0,
                hover: Hover::default(),
                shake: 0.0,
                spin: 0.0,
                appear: 0.0,
                delay: i as f32 * 0.12,
                leave: None,
            })
            .collect();
        self.target = cx.rng.int(self.pieces.len());
        self.wrongs = 0;
        self.state = State::Playing;
        self.ask_in = Some(0.7);
        self.layout(cx);
    }

    fn layout(&mut self, cx: &Cx) {
        let n = self.pieces.len();
        let portrait = cx.h > cx.w;
        let per_row = if portrait { 2 } else if n <= 4 { n } else { 3 };
        let rows = n.div_ceil(per_row);
        let top = cx.h * 0.18 + 20.0;
        let cell_w = cx.w / per_row as f32;
        let cell_h = (cx.h - top - cx.h * 0.04) / rows as f32;
        let r = (cell_w.min(cell_h) * 0.33).min(cx.unit() * 0.2);
        for (i, p) in self.pieces.iter_mut().enumerate() {
            let row = i / per_row;
            let in_row = if row == rows - 1 { n - row * per_row } else { per_row };
            let col = i % per_row;
            let offset = (per_row - in_row) as f32 * cell_w * 0.5;
            p.pos = v2(offset + (col as f32 + 0.5) * cell_w, top + (row as f32 + 0.5) * cell_h);
            p.r = r;
        }
    }

    fn speaker_pos(cx: &Cx) -> (V2, f32) {
        let r = (cx.unit() * 0.055).clamp(28.0, 48.0);
        (v2(cx.w * 0.5, r + 18.0), r)
    }

    fn ask(&mut self, cx: &mut Cx) {
        let q = self.question();
        cx.au.say(&q);
        self.speaker_pulse = 1.0;
    }

    fn click_piece(&mut self, cx: &mut Cx, i: usize) {
        if !matches!(self.state, State::Playing) {
            return;
        }
        let pos = self.pieces[i].pos;
        if i == self.target {
            self.pieces[i].spin = 1.0;
            self.streak += 1;
            if self.streak >= 3 {
                self.streak = 0;
                self.level += 1;
            }
            let praise = *cx.rng.pick(vocab::PRAISE);
            let p = &self.pieces[i];
            let name = vocab::shape_phrase(&vocab::SHAPES[p.shape], &vocab::GAME_COLORS[p.color]);
            cx.au.say(&format!("{} ¡Es {}!", praise, name));
            cx.au.sfx(Sfx::Fanfare, 1.0, pos.x);
            cx.fx.confetti(&mut cx.rng, pos, 50, 700.0);
            cx.fx.flash(pos, self.pieces[i].r * 1.5, WHITE.alpha(0.8));
            self.state = State::Won(2.6);
            self.ask_in = None;
        } else {
            self.pieces[i].shake = 1.0;
            self.streak = 0;
            self.wrongs += 1;
            if self.wrong_cooldown <= 0.0 {
                let d = self.describe(i);
                cx.au.say(&d);
                let q = self.question();
                cx.au.say_after(1.9, &q);
                self.wrong_cooldown = 1.0;
            }
            cx.au.sfx(Sfx::Oops, 1.0, pos.x);
        }
    }
}

impl Activity for Shapes {
    fn enter(&mut self, cx: &mut Cx) {
        cx.au.say("¡Formas y colores!");
        self.ask_in = Some(1.6);
    }

    fn resize(&mut self, cx: &mut Cx) {
        self.layout(cx);
    }

    fn update(&mut self, cx: &mut Cx) {
        cx.g.background(hex(0xFFF8E1), hex(0xFFE0B2), Backdrop::Bokeh, 0.0);
        let dt = cx.dt;
        self.wrong_cooldown -= dt;

        if let Some(t) = self.ask_in.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                self.ask_in = None;
                self.ask(cx);
            }
        }
        if matches!(self.state, State::Playing) {
            let q = self.question();
            self.idle.tick(cx, &q);
        }
        if let State::Won(t) = self.state {
            let t = t - dt;
            self.state = State::Won(t);
            if t < 0.6 {
                // Las formas se van volando.
                for (i, p) in self.pieces.iter_mut().enumerate() {
                    if p.leave.is_none() {
                        let dir = if i % 2 == 0 { -1.0 } else { 1.0 };
                        p.leave = Some(v2(dir * cx.w, -cx.h * 0.3));
                    }
                }
            }
            if t <= 0.0 {
                self.new_round(cx);
            }
        }

        // Botón para repetir la pregunta.
        let (sp, sr) = Shapes::speaker_pos(cx);
        if self.speaker.update(cx, cx.inp.has_mouse && cx.inp.mouse.dist(sp) < sr * 1.1) {
            cx.au.sfx_vol(Sfx::Tick, 1.3, sp.x, 0.5);
        }
        self.speaker_pulse = (self.speaker_pulse - dt * 0.8).max(0.0);
        let beat = self.speaker_pulse * (cx.t * 12.0).sin().abs() * 0.12;
        let srr = sr * (1.0 + 0.15 * self.speaker.anim + beat);
        cx.g.shadow(Shape::Circle, sp, srr, 0.0, 1.0);
        cx.g.sticker(Shape::Circle, sp, srr, 0.0, WHITE.alpha(0.95));
        cx.g.emoji("🔊", sp, srr * 0.6, 0.0, 1.0);

        let hint = self.wrongs >= 2 || self.idle.waiting(cx);
        let mouse = cx.inp.mouse;
        let has_mouse = cx.inp.has_mouse;
        let t = cx.t;
        for (i, p) in self.pieces.iter_mut().enumerate() {
            p.appear += dt;
            p.shake = (p.shake - dt * 1.8).max(0.0);
            p.spin = (p.spin - dt * 0.6).max(0.0);
            if let Some(dir) = p.leave {
                p.pos += dir * (dt * 2.0);
            }
            let over = has_mouse && mouse.dist(p.pos) < p.r * 1.1;
            if p.hover.update(cx, over) && matches!(self.state, State::Playing) {
                cx.au.sfx_vol(Sfx::Tick, 0.9 + i as f32 * 0.1, p.pos.x, 0.4);
            }
        }
        for (i, p) in self.pieces.iter().enumerate() {
            let k = ease_out_back(((p.appear - p.delay) / 0.5).clamp(0.0, 1.0));
            let shake = (p.shake * 30.0).sin() * p.shake * p.r * 0.15;
            let win = if p.spin > 0.0 { ease_out_elastic(1.0 - p.spin) } else { 0.0 };
            let mut r = p.r * k * (1.0 + 0.08 * p.hover.anim + 0.25 * p.spin);
            let pos = p.pos + v2(shake, -(t * 1.5 + i as f32).sin() * p.r * 0.03);
            let rot = win * crate::math::TAU + (t + i as f32).sin() * 0.05;
            if hint && i == self.target && matches!(self.state, State::Playing) {
                let pulse = (t * 5.0).sin() * 0.5 + 0.5;
                cx.g.glow(pos, r * 1.6, hsv(0.15, 0.6, 1.0).alpha(0.5 * pulse));
                r *= 1.0 + 0.06 * pulse;
            }
            let shape = vocab::SHAPES[p.shape].shape;
            let color = vocab::GAME_COLORS[p.color].rgb;
            cx.g.shadow(shape, pos, r, rot, 1.0);
            cx.g.sticker(shape, pos, r, rot, color);
        }
    }

    fn down(&mut self, cx: &mut Cx, p: V2, _button: u32) {
        let (sp, sr) = Shapes::speaker_pos(cx);
        if p.dist(sp) < sr * 1.2 {
            if matches!(self.state, State::Playing) {
                self.ask(cx);
            }
            return;
        }
        let hit = self
            .pieces
            .iter()
            .enumerate()
            .filter(|(_, q)| q.pos.dist(p) < q.r * 1.15)
            .min_by(|(_, a), (_, b)| a.pos.dist(p).total_cmp(&b.pos.dist(p)))
            .map(|(i, _)| i);
        match hit {
            Some(i) => self.click_piece(cx, i),
            None => {
                cx.fx.ripple(p, 40.0, hsv(cx.rng.f(), 0.5, 1.0));
            }
        }
    }

    fn key(&mut self, cx: &mut Cx, k: &Key) {
        if (k.is("Space") || k.is_enter()) && matches!(self.state, State::Playing) {
            self.ask(cx);
        } else {
            let n = cx.rng.int(8) as f32;
            cx.au.note(midi(67.0 + n * 2.0), Inst::Soft, cx.w * 0.5, 0.3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    #[test]
    fn rounds_have_unique_target() {
        let mut g = Game::new(1280.0, 720.0, 3);
        for level in 0..6 {
            let mut s = Shapes::new(&mut g.cx);
            s.level = level;
            for _ in 0..30 {
                s.new_round(&mut g.cx);
                let t = &s.pieces[s.target];
                let same = s.pieces.iter().filter(|p| p.shape == t.shape && p.color == t.color).count();
                assert_eq!(same, 1);
            }
        }
    }

    #[test]
    fn wrong_answer_names_the_piece() {
        let mut g = Game::new(1280.0, 720.0, 5);
        let mut s = Shapes::new(&mut g.cx);
        g.cx.au.clear();
        let wrong = (s.target + 1) % s.pieces.len();
        s.click_piece(&mut g.cx, wrong);
        let said = g.cx.au.spoken();
        assert_eq!(said.len(), 1);
        assert!(said[0].starts_with("Ese es") || said[0].starts_with("Esa es"), "{said:?}");
    }
}
