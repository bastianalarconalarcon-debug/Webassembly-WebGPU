//! Pantalla de inicio: el nombre de la niña en letras grandes y una
//! "puerta" redonda por cada juego. Pasar el mouse por encima dice el
//! nombre del juego; hacer clic entra.

use super::{Activity, Hover, IdlePrompt, ModeId};
use crate::audio::{Inst, Sfx};
use crate::game::{Cx, Key};
use crate::gfx::{Backdrop, Shape};
use crate::math::{ease_out_back, hex, hsv, pentatonic, v2, Rgba, V2, WHITE};
use crate::vocab;

struct Door {
    mode: ModeId,
    name: &'static str,
    color: Rgba,
    hover: Hover,
    pos: V2,
    r: f32,
}

struct NameLetter {
    text: String,
    letter: Option<char>,
    pos: V2,
    r: f32,
    bounce: f32,
    hover: Hover,
}

pub struct Home {
    doors: Vec<Door>,
    letters: Vec<NameLetter>,
    focus: Option<usize>,
    idle: IdlePrompt,
    appear: f32,
}

impl Home {
    pub fn new(cx: &mut Cx) -> Home {
        let door = |mode, name, color| Door {
            mode,
            name,
            color,
            hover: Hover::default(),
            pos: V2::ZERO,
            r: 1.0,
        };
        // Arriba lo de aprender (letras, escribir, números, formas);
        // abajo lo de explorar (pintar, música, burbujas, abeja).
        let doors = vec![
            door(ModeId::Keys, "Teclado mágico", hex(0xFF7043)),
            door(ModeId::Typing, "Escribir", hex(0x26A69A)),
            door(ModeId::Adding, "Sumar", hex(0xEC407A)),
            door(ModeId::Shapes, "Formas y colores", hex(0xFFCA28)),
            door(ModeId::Paint, "Pintar", hex(0xAB47BC)),
            door(ModeId::Music, "Música", hex(0x66BB6A)),
            door(ModeId::Bubbles, "Burbujas", hex(0x29B6F6)),
            door(ModeId::Bee, "La abeja", hex(0xFFA726)),
            door(ModeId::Build, "Construir", hex(0x8D6E63)),
        ];
        let word = if cx.name.is_empty() {
            "HOLA".to_string()
        } else {
            cx.name.to_uppercase()
        };
        let letters = word
            .chars()
            .filter(|c| !c.is_whitespace())
            .take(12)
            .map(|c| NameLetter {
                text: c.to_string(),
                letter: vocab::fold_letter(c),
                pos: V2::ZERO,
                r: 1.0,
                bounce: 0.0,
                hover: Hover::default(),
            })
            .collect();
        let mut h = Home {
            doors,
            letters,
            focus: None,
            idle: IdlePrompt::new(14.0),
            appear: 0.0,
        };
        h.layout(cx);
        h
    }

    /// Cuántas puertas va en cada fila. En horizontal: arriba lo de aprender
    /// (4) y abajo lo de explorar (5); en vertical, de a 3.
    fn rows(w: f32, h: f32, n: usize) -> Vec<usize> {
        let per = if w >= h { [4, 5] } else { [3, 3] };
        let mut rows = Vec::new();
        let mut left = n;
        let mut i = 0;
        while left > 0 {
            let k = per[i.min(per.len() - 1)].min(left);
            rows.push(k);
            left -= k;
            i += 1;
        }
        rows
    }

    fn layout(&mut self, cx: &Cx) {
        let (w, h) = (cx.w, cx.h);
        let rows = Home::rows(w, h, self.doors.len());
        let widest = rows.iter().copied().max().unwrap_or(1);
        let top = h * 0.25;
        let cell_w = w / widest as f32;
        let cell_h = (h - top) / rows.len() as f32;
        let r = (cell_w.min(cell_h) * 0.37).max(30.0);
        let mut i = 0;
        for (row, &count) in rows.iter().enumerate() {
            // Filas centradas aunque tengan menos puertas.
            let x0 = (w - count as f32 * cell_w) * 0.5;
            for c in 0..count {
                let d = &mut self.doors[i];
                d.pos = v2(
                    x0 + (c as f32 + 0.5) * cell_w,
                    top + (row as f32 + 0.5) * cell_h - h * 0.01,
                );
                d.r = r;
                i += 1;
            }
        }
        let n = self.letters.len().max(1) as f32;
        let lr = (h * 0.09).min(w / (n * 2.4)).max(16.0);
        for (i, l) in self.letters.iter_mut().enumerate() {
            l.pos = v2(w * 0.5 + (i as f32 - (n - 1.0) * 0.5) * lr * 2.1, h * 0.125);
            l.r = lr;
        }
    }

    fn open(&mut self, cx: &mut Cx, i: usize) {
        let d = &self.doors[i];
        cx.au.sfx(Sfx::Pop, 1.0, d.pos.x);
        cx.fx.confetti(&mut cx.rng, d.pos, 30, 500.0);
        cx.au.say(d.name);
        cx.go(d.mode, d.pos, d.color);
    }

    fn announce(&mut self, cx: &mut Cx, i: usize) {
        let d = &self.doors[i];
        cx.au.say(d.name);
        cx.au
            .note(pentatonic(i as i32 + 5, 60.0), Inst::Marimba, d.pos.x, 0.5);
    }

    fn draw_icon(cx: &mut Cx, mode: ModeId, p: V2, r: f32, t: f32) {
        let g = &mut cx.g;
        match mode {
            ModeId::Keys => {
                g.emoji("⌨️", p + v2(0.0, r * 0.15), r * 0.55, 0.0, 1.0);
                for (i, s) in ["A", "B", "C"].iter().enumerate() {
                    let x = (i as f32 - 1.0) * r * 0.36;
                    let y = -r * 0.42 + (t * 3.0 + i as f32).sin() * r * 0.05;
                    g.text(
                        s,
                        p + v2(x, y),
                        r * 0.2,
                        0.0,
                        hsv(i as f32 * 0.3, 0.8, 1.0),
                        0.18,
                    );
                }
            }
            ModeId::Paint => g.emoji("🎨", p, r * 0.6, (t * 1.5).sin() * 0.1, 1.0),
            ModeId::Bubbles => {
                g.emoji(
                    "🐠",
                    p + v2(0.0, (t * 2.0).sin() * r * 0.04),
                    r * 0.38,
                    0.0,
                    1.0,
                );
                g.shape(Shape::Bubble, p, r * 0.62, 0.0, WHITE);
                g.shape(
                    Shape::Bubble,
                    p + v2(r * 0.52, -r * 0.42),
                    r * 0.16,
                    0.0,
                    WHITE,
                );
            }
            ModeId::Shapes => {
                let s = r * 0.24;
                g.sticker(
                    Shape::Circle,
                    p + v2(-r * 0.3, -r * 0.2),
                    s,
                    0.0,
                    vocab::ROJO.rgb,
                );
                g.sticker(
                    Shape::RoundRect,
                    p + v2(r * 0.3, -r * 0.2),
                    s * 0.9,
                    0.2,
                    vocab::AZUL.rgb,
                );
                g.sticker(
                    Shape::Triangle,
                    p + v2(-r * 0.3, r * 0.3),
                    s,
                    0.0,
                    vocab::AMARILLO.rgb,
                );
                g.sticker(
                    Shape::Star,
                    p + v2(r * 0.3, r * 0.3),
                    s * 1.05,
                    t.sin() * 0.2,
                    vocab::VERDE.rgb,
                );
            }
            ModeId::Music => {
                g.emoji(
                    "🎵",
                    p + v2(0.0, (t * 4.0).sin() * r * 0.05),
                    r * 0.55,
                    0.0,
                    1.0,
                );
            }
            ModeId::Bee => {
                g.emoji("🌸", p + v2(r * 0.25, r * 0.3), r * 0.28, 0.0, 1.0);
                let b = p + v2(
                    (t * 1.3).cos() * r * 0.25,
                    (t * 2.6).sin() * r * 0.12 - r * 0.1,
                );
                g.emoji_flip("🐝", b, r * 0.4, 0.0, 1.0, (t * 1.3).sin() > 0.0);
            }
            ModeId::Typing => {
                g.emoji("✏️", p + v2(r * 0.18, -r * 0.12), r * 0.42, 0.3, 1.0);
                for (i, s) in ["M", "A", "M", "Á"].iter().enumerate() {
                    let x = (i as f32 - 1.5) * r * 0.3;
                    let y = r * 0.42 + (t * 3.0 + i as f32).sin() * r * 0.03;
                    g.text(
                        s,
                        p + v2(x, y),
                        r * 0.17,
                        0.0,
                        hsv(0.45 + i as f32 * 0.12, 0.7, 0.9),
                        0.15,
                    );
                }
            }
            ModeId::Adding => {
                let s = r * 0.24;
                g.text(
                    "1",
                    p + v2(-r * 0.42, -r * 0.05),
                    s,
                    0.0,
                    vocab::ROJO.rgb,
                    0.15,
                );
                g.text(
                    "+",
                    p + v2(-r * 0.14, -r * 0.05),
                    s * 0.85,
                    0.0,
                    vocab::NARANJO.rgb,
                    0.15,
                );
                g.text(
                    "2",
                    p + v2(r * 0.14, -r * 0.05),
                    s,
                    0.0,
                    vocab::AZUL.rgb,
                    0.15,
                );
                g.text(
                    "=",
                    p + v2(r * 0.42, -r * 0.05),
                    s * 0.85,
                    0.0,
                    vocab::VERDE.rgb,
                    0.15,
                );
                for i in 0..3 {
                    let x = (i as f32 - 1.0) * r * 0.3;
                    g.emoji(
                        "🍎",
                        p + v2(x, r * 0.42 + (t * 3.0 + i as f32).sin() * r * 0.03),
                        r * 0.15,
                        0.0,
                        1.0,
                    );
                }
            }
            ModeId::Build => {
                let s = r * 0.24;
                g.shape(
                    Shape::Cube,
                    p + v2(-s * 0.87, s * 0.75),
                    s,
                    0.0,
                    vocab::ROJO.rgb,
                );
                g.shape(
                    Shape::Cube,
                    p + v2(s * 0.87, s * 0.75),
                    s,
                    0.0,
                    vocab::AZUL.rgb,
                );
                g.shape(
                    Shape::Cube,
                    p + v2(0.0, -s * 0.75 + (t * 3.0).sin().abs() * -r * 0.08),
                    s,
                    0.0,
                    vocab::AMARILLO.rgb,
                );
            }
            ModeId::Home => {}
        }
    }
}

impl Activity for Home {
    fn enter(&mut self, cx: &mut Cx) {
        let greeting = if cx.t < 1.0 {
            if cx.name.is_empty() {
                "¡Hola! ¿A qué quieres jugar?".to_string()
            } else {
                format!("¡Hola, {}! ¿A qué quieres jugar?", cx.name)
            }
        } else {
            "¿A qué quieres jugar ahora?".to_string()
        };
        cx.au.say(&greeting);
    }

    fn resize(&mut self, cx: &mut Cx) {
        self.layout(cx);
    }

    fn update(&mut self, cx: &mut Cx) {
        cx.g.background(hex(0x8EC5FC), hex(0xF3D1F4), Backdrop::Bokeh, 0.0);
        self.appear += cx.dt;
        let hint = self
            .idle
            .tick(cx, "¡Elige un juego! Haz clic en un dibujo.");
        if hint {
            self.focus = None;
        }
        let waiting = self.idle.waiting(cx);

        // Letras del nombre.
        let mouse = cx.inp.mouse;
        for (i, l) in self.letters.iter_mut().enumerate() {
            let over = cx.inp.has_mouse && mouse.dist(l.pos) < l.r;
            if l.hover.update(cx, over) {
                l.bounce = 1.0;
                if let Some(info) = l.letter.and_then(vocab::letter_info) {
                    cx.au.say(info.name);
                }
                cx.au
                    .note(pentatonic(i as i32 + 3, 60.0), Inst::Bell, l.pos.x, 0.4);
            }
            l.bounce = (l.bounce - cx.dt * 1.5).max(0.0);
            let jump = (l.bounce * 9.0).sin().abs() * l.bounce * l.r * 0.5;
            let float = (cx.t * 2.0 + i as f32 * 0.7).sin() * l.r * 0.06;
            let appear = ease_out_back((self.appear * 2.0 - i as f32 * 0.12).clamp(0.0, 1.0));
            let c = hsv(i as f32 * 0.13 + 0.95, 0.75, 1.0);
            let p = l.pos - v2(0.0, jump + float);
            cx.g.text(
                &l.text,
                p + v2(0.0, l.r * 0.08),
                l.r * appear,
                0.0,
                Rgba {
                    r: 0.1,
                    g: 0.1,
                    b: 0.3,
                    a: 0.18,
                },
                0.0,
            );
            cx.g.text(
                &l.text,
                p,
                l.r * appear * (1.0 + l.hover.anim * 0.15),
                (cx.t * 1.5 + i as f32).sin() * 0.06,
                c,
                0.16,
            );
        }

        // Puertas a los juegos.
        let mut announce = None;
        for i in 0..self.doors.len() {
            let d = &mut self.doors[i];
            let over = cx.inp.has_mouse && mouse.dist(d.pos) < d.r;
            if d.hover.update(cx, over) {
                announce = Some(i);
            }
        }
        if let Some(i) = announce {
            self.focus = Some(i);
            self.announce(cx, i);
        }
        let t = cx.t;
        for i in 0..self.doors.len() {
            let (pos, r, color, mode, hover) = {
                let d = &self.doors[i];
                (d.pos, d.r, d.color, d.mode, d.hover.anim)
            };
            let appear = ease_out_back((self.appear * 1.8 - 0.2 - i as f32 * 0.1).clamp(0.0, 1.0));
            let focused = self.focus == Some(i);
            // Si está esperando, las puertas "saludan" una por una.
            let wave = if waiting {
                ((t * 2.0 - i as f32 * 0.6).sin().max(0.0)).powi(4) * 0.1
            } else {
                0.0
            };
            let k = appear * (1.0 + 0.12 * hover + wave + if focused { 0.04 } else { 0.0 });
            let bob = (t * 1.6 + i as f32).sin() * r * 0.03;
            let p = pos + v2(0.0, bob);
            let rr = r * k;
            cx.g.shadow(Shape::Circle, p, rr, 0.0, 1.0);
            if focused {
                cx.g.ring(p, rr * 1.12, rr * 0.06, WHITE.alpha(0.85));
            }
            cx.g.sticker(Shape::Circle, p, rr, 0.0, color);
            cx.g.circle(p, rr * 0.82, color.lighter(0.55));
            Home::draw_icon(cx, mode, p, rr, t + i as f32);
        }
    }

    fn down(&mut self, cx: &mut Cx, p: V2, button: u32) {
        if let Some(i) = self.doors.iter().position(|d| p.dist(d.pos) < d.r) {
            self.open(cx, i);
            return;
        }
        // Clic en el fondo: estrellitas.
        let c = hsv(cx.rng.f(), 0.7, 1.0);
        cx.fx.sparkle(&mut cx.rng, p, 10, c, 60.0);
        cx.au.note(
            pentatonic(cx.rng.int(8) as i32 + 5, 60.0),
            Inst::Bell,
            p.x,
            0.35,
        );
        if button == 2 {
            cx.fx.confetti(&mut cx.rng, p, 12, 300.0);
        }
    }

    fn key(&mut self, cx: &mut Cx, k: &Key) {
        let n = self.doors.len();
        if let Some(dir) = k.arrow() {
            let cur = self.focus.unwrap_or(0);
            let next = if self.focus.is_none() {
                0
            } else if dir.x != 0.0 {
                (cur as i32 + dir.x as i32).rem_euclid(n as i32) as usize
            } else {
                // Arriba/abajo: la puerta más cercana en la fila de al lado.
                let here = self.doors[cur].pos;
                self.doors
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| (d.pos.y - here.y) * dir.y > 1.0)
                    .min_by(|(_, a), (_, b)| {
                        let da = (a.pos.y - here.y).abs() * 4.0 + (a.pos.x - here.x).abs();
                        let db = (b.pos.y - here.y).abs() * 4.0 + (b.pos.x - here.x).abs();
                        da.total_cmp(&db)
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(cur)
            };
            self.focus = Some(next);
            self.announce(cx, next);
            return;
        }
        if k.is_enter() {
            if let Some(i) = self.focus {
                self.open(cx, i);
            } else {
                self.focus = Some(0);
                self.announce(cx, 0);
            }
            return;
        }
        // Una letra del nombre salta y se dice.
        if let Some(c) = k.letter() {
            if let Some(i) = self.letters.iter().position(|l| l.letter == Some(c)) {
                self.letters[i].bounce = 1.0;
                let pos = self.letters[i].pos;
                cx.fx
                    .sparkle(&mut cx.rng, pos, 12, hsv(i as f32 * 0.13, 0.7, 1.0), 60.0);
            }
            if let Some(info) = vocab::letter_info(c) {
                cx.au.say(info.name);
            }
        }
        let x = cx.rng.range(0.1, 0.9) * cx.w;
        cx.au.note(
            pentatonic(cx.rng.int(10) as i32 + 3, 60.0),
            Inst::Marimba,
            x,
            0.4,
        );
        let p = v2(x, cx.rng.range(0.3, 0.9) * cx.h);
        let c = hsv(cx.rng.f(), 0.7, 1.0);
        cx.fx.sparkle(&mut cx.rng, p, 8, c, 50.0);
    }
}
