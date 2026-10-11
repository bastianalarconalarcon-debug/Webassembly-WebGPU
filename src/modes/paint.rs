//! Pintar: el mouse pinta solo con moverlo (no hace falta mantener el
//! botón, que es difícil a los 3 años). Clic = estampa. Abajo hay una
//! barra de herramientas con colores, arcoíris, goma y basurero.
//! La pintura se guarda en una textura de WebGPU que no se borra.

use super::{Activity, Hover, IdlePrompt};
use crate::audio::{Inst, Sfx};
use crate::game::{Cx, Key};
use crate::gfx::{Backdrop, Shape};
use crate::math::{hex, hsv, pentatonic, v2, Rgba, V2, WHITE};
use crate::vocab::{self, ColorName};

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Color(usize),
    Rainbow,
    Eraser,
    Trash,
}

struct Button {
    tool: Tool,
    pos: V2,
    r: f32,
    hover: Hover,
}

const PAPER: Rgba = hex(0xFFFBF2);
const STAMPS: [Shape; 4] = [Shape::Star, Shape::Heart, Shape::Circle, Shape::Flower];
const EMOJI_STAMPS: [&str; 8] = ["🐱", "🐶", "🦋", "🌸", "⭐", "🐟", "🦄", "🍓"];

pub struct Paint {
    tool: Tool,
    buttons: Vec<Button>,
    bar_top: f32,
    brush: f32,
    rainbow_h: f32,
    stamp_i: usize,
    idle: IdlePrompt,
    painted: f32,
}

impl Paint {
    pub fn new(cx: &mut Cx) -> Paint {
        let mut p = Paint {
            tool: Tool::Rainbow,
            buttons: Vec::new(),
            bar_top: 0.0,
            brush: 1.0,
            rainbow_h: 0.0,
            stamp_i: 0,
            idle: IdlePrompt::new(12.0),
            painted: 0.0,
        };
        p.layout(cx);
        p
    }

    fn layout(&mut self, cx: &Cx) {
        let mut tools: Vec<Tool> = (0..vocab::PAINT_COLORS.len()).map(Tool::Color).collect();
        tools.insert(0, Tool::Rainbow);
        tools.push(Tool::Eraser);
        tools.push(Tool::Trash);
        let n = tools.len() as f32;
        let r = (cx.w / (n * 2.5)).min(cx.h * 0.055).max(16.0);
        let y = cx.h - r * 1.5;
        self.bar_top = y - r * 1.4;
        let gap = r * 2.45;
        let x0 = cx.w * 0.5 - (n - 1.0) * gap * 0.5;
        self.buttons = tools
            .into_iter()
            .enumerate()
            .map(|(i, tool)| Button { tool, pos: v2(x0 + i as f32 * gap, y), r, hover: Hover::default() })
            .collect();
    }

    fn brush_r(&self, cx: &Cx) -> f32 {
        cx.unit() * 0.018 * self.brush
    }

    fn color_at(&mut self, step: f32) -> Rgba {
        match self.tool {
            Tool::Color(i) => vocab::PAINT_COLORS[i].rgb,
            Tool::Eraser => PAPER,
            _ => {
                self.rainbow_h += step;
                hsv(self.rainbow_h, 0.8, 1.0)
            }
        }
    }

    fn select(&mut self, cx: &mut Cx, i: usize) {
        let b = &self.buttons[i];
        let (tool, pos) = (b.tool, b.pos);
        cx.fx.sparkle(&mut cx.rng, pos, 10, WHITE, 40.0);
        match tool {
            Tool::Trash => {
                cx.g.clear_paint = true;
                cx.au.sfx(Sfx::Splash, 1.0, pos.x);
                cx.au.say("¡Borrar todo!");
                for _ in 0..5 {
                    let p = cx.rng.point(0.0, 0.0, cx.w, self.bar_top);
                    cx.fx.ripple(p, 200.0, hsv(cx.rng.f(), 0.4, 1.0).alpha(0.6));
                }
            }
            Tool::Color(c) => {
                self.tool = tool;
                let name = vocab::PAINT_COLORS[c];
                cx.au.say(&format!("¡{}!", capital(name.m)));
                cx.au.note(pentatonic(c as i32 + 5, 60.0), Inst::Marimba, pos.x, 0.5);
            }
            Tool::Rainbow => {
                self.tool = tool;
                cx.au.say("¡Arcoíris!");
                cx.au.sfx(Sfx::Magic, 1.0, pos.x);
            }
            Tool::Eraser => {
                self.tool = tool;
                cx.au.say("¡Goma!");
                cx.au.sfx(Sfx::Whoosh, 1.5, pos.x);
            }
        }
    }

    fn stroke(&mut self, cx: &mut Cx, from: V2, to: V2) {
        if to.y > self.bar_top || from.y > self.bar_top {
            return;
        }
        let r = self.brush_r(cx) * if self.tool == Tool::Eraser { 2.5 } else { 1.0 };
        let d = from.dist(to);
        let steps = ((d / (r * 0.35)).ceil() as usize).clamp(1, 200);
        cx.g.paint_layer(true);
        for i in 1..=steps {
            let p = from.lerp(to, i as f32 / steps as f32);
            let c = self.color_at(0.0015);
            cx.g.circle(p, r, c);
        }
        cx.g.paint_layer(false);
        self.painted += d;
        // Notitas suaves mientras pinta, más agudas arriba.
        if self.painted > 160.0 {
            self.painted = 0.0;
            let step = ((1.0 - to.y / cx.h) * 10.0) as i32;
            cx.au.note(pentatonic(step + 5, 60.0), Inst::Soft, to.x, 0.18);
        }
    }

    fn stamp(&mut self, cx: &mut Cx, p: V2, emoji: bool) {
        let r = (cx.unit() * 0.06) * (0.8 + self.brush * 0.3);
        let rot = cx.rng.range(-0.4, 0.4);
        cx.g.paint_layer(true);
        if emoji {
            let e = EMOJI_STAMPS[self.stamp_i % EMOJI_STAMPS.len()];
            cx.g.emoji(e, p, r * 1.1, rot, 1.0);
        } else {
            let s = STAMPS[self.stamp_i % STAMPS.len()];
            let c = match self.tool {
                Tool::Color(i) => vocab::PAINT_COLORS[i].rgb,
                _ => hsv(cx.rng.f(), 0.75, 1.0),
            };
            cx.g.shape(s, p, r + 4.0, rot, c.darker(0.3));
            cx.g.shape(s, p, r, rot, c);
        }
        cx.g.paint_layer(false);
        self.stamp_i += 1;
        let col = hsv(cx.rng.f(), 0.6, 1.0);
        cx.fx.sparkle(&mut cx.rng, p, 10, col, 50.0);
        cx.au.sfx(Sfx::Pop, 0.9 + cx.rng.range(0.0, 0.3), p.x);
    }

    fn current_color(&self) -> Rgba {
        match self.tool {
            Tool::Color(i) => vocab::PAINT_COLORS[i].rgb,
            Tool::Eraser => PAPER,
            _ => hsv(self.rainbow_h, 0.8, 1.0),
        }
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

impl Activity for Paint {
    fn enter(&mut self, cx: &mut Cx) {
        cx.au.say("¡A pintar! Mueve el mouse.");
    }

    fn resize(&mut self, cx: &mut Cx) {
        self.layout(cx);
    }

    fn cursor_trail(&self) -> bool {
        false
    }

    fn update(&mut self, cx: &mut Cx) {
        cx.g.background(PAPER, PAPER, Backdrop::Paper, 0.0);
        cx.g.show_paint();
        self.idle.tick(cx, "¡Mueve el mouse para pintar!");

        // Barra de herramientas.
        let bar_h = cx.h - self.bar_top;
        cx.g.rrect(v2(cx.w * 0.5, self.bar_top + bar_h * 0.5 + 6.0), v2(cx.w * 0.5 + 10.0, bar_h * 0.5 + 6.0), 0.0, WHITE.alpha(0.85));
        let mouse = cx.inp.mouse;
        let mut announce = None;
        for (i, b) in self.buttons.iter_mut().enumerate() {
            if b.hover.update(cx, cx.inp.has_mouse && mouse.dist(b.pos) < b.r * 1.1) {
                announce = Some(i);
            }
        }
        if let Some(i) = announce {
            cx.au.sfx_vol(Sfx::Tick, 1.0 + i as f32 * 0.05, self.buttons[i].pos.x, 0.5);
        }
        for b in &self.buttons {
            let selected = b.tool == self.tool;
            let lift = if selected { b.r * 0.35 } else { 0.0 } + b.hover.anim * b.r * 0.15;
            let r = b.r * (1.0 + 0.15 * b.hover.anim + if selected { 0.15 } else { 0.0 });
            let p = b.pos - v2(0.0, lift);
            cx.g.shadow(Shape::Circle, p, r, 0.0, 0.8);
            match b.tool {
                Tool::Color(i) => {
                    let c: ColorName = vocab::PAINT_COLORS[i];
                    cx.g.sticker(Shape::Circle, p, r, 0.0, c.rgb);
                }
                Tool::Rainbow => {
                    cx.g.sticker(Shape::Circle, p, r, 0.0, WHITE);
                    cx.g.emoji("🌈", p, r * 0.75, 0.0, 1.0);
                }
                Tool::Eraser => {
                    cx.g.sticker(Shape::Circle, p, r, 0.0, hex(0xF8BBD0));
                    cx.g.shape_full(Shape::RoundRect, p, v2(r * 0.55, r * 0.32), -0.5, WHITE, 0.3, 0.0);
                    cx.g.shape_full(Shape::RoundRect, p + v2(r * 0.22, r * 0.13), v2(r * 0.25, r * 0.3), -0.5 + std::f32::consts::FRAC_PI_2, hex(0x42A5F5), 0.3, 0.0);
                }
                Tool::Trash => {
                    cx.g.sticker(Shape::Circle, p, r, 0.0, hex(0xECEFF1));
                    cx.g.emoji("🗑️", p, r * 0.7, 0.0, 1.0);
                }
            }
            if selected {
                cx.g.ring(p, r * 1.18, 4.0, hex(0x37474F).alpha(0.7));
            }
        }

        // Vista previa del pincel bajo el mouse.
        if cx.inp.has_mouse && mouse.y < self.bar_top {
            let r = self.brush_r(cx) * if self.tool == Tool::Eraser { 2.5 } else { 1.0 };
            cx.g.ring(mouse, r + 3.0, 3.0, hex(0x37474F).alpha(0.5));
            cx.g.circle(mouse, r, self.current_color().alpha(0.9));
        }
    }

    fn moved(&mut self, cx: &mut Cx, from: V2, to: V2) {
        self.stroke(cx, from, to);
    }

    fn down(&mut self, cx: &mut Cx, p: V2, button: u32) {
        if let Some(i) = self.buttons.iter().position(|b| p.dist(b.pos) < b.r * 1.25) {
            self.select(cx, i);
            return;
        }
        if p.y < self.bar_top {
            self.stamp(cx, p, button == 2);
        }
    }

    fn wheel(&mut self, cx: &mut Cx, dy: f32) -> bool {
        // La rueda cambia el tamaño del pincel.
        let before = self.brush;
        self.brush = (self.brush * if dy < 0.0 { 1.2 } else { 1.0 / 1.2 }).clamp(0.4, 4.0);
        if self.brush != before {
            let p = cx.inp.mouse;
            cx.fx.ripple(p, self.brush_r(cx) * 2.0, self.current_color());
            cx.au.sfx_vol(Sfx::Bloop, if dy < 0.0 { 1.3 } else { 0.8 }, p.x, 0.5);
        }
        true
    }

    fn key(&mut self, cx: &mut Cx, k: &Key) {
        if k.is_erase() {
            if let Some(i) = self.buttons.iter().position(|b| b.tool == Tool::Trash) {
                self.select(cx, i);
            }
        } else if k.is("Space") || k.is_enter() {
            let p = if cx.inp.has_mouse && cx.inp.mouse.y < self.bar_top {
                cx.inp.mouse
            } else {
                cx.rng.point(cx.w * 0.1, cx.h * 0.1, cx.w * 0.9, self.bar_top - 40.0)
            };
            self.stamp(cx, p, k.is_enter());
        } else if let Some(d) = k.digit() {
            // Los números cambian el tamaño del pincel.
            self.brush = 0.4 + d as f32 * 0.4;
            cx.au.say(vocab::NUMBERS[d as usize]);
            let p = cx.inp.mouse;
            cx.fx.ripple(p, self.brush_r(cx) * 2.0, self.current_color());
        } else if k.arrow().is_some() || k.letter().is_some() {
            // Letras y flechas: otro color de la paleta.
            let colors: Vec<usize> = self
                .buttons
                .iter()
                .enumerate()
                .filter(|(_, b)| matches!(b.tool, Tool::Color(_)))
                .map(|(i, _)| i)
                .collect();
            let i = *cx.rng.pick(&colors);
            self.select(cx, i);
        }
    }
}
