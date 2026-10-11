//! Música: un xilófono de 8 barras de colores (Do Re Mi Fa Sol La Si Do).
//! - Pasar el mouse por las barras las toca (¡glissando!), clic también.
//! - Las filas del teclado tocan las barras (fila de arriba = más agudo).
//! - Arriba se elige instrumento: marimba, piano, campana, guitarra, tambor.
//! - La ⭐ ("Estrellita") y el 🐥 ("Los pollitos dicen") activan una canción:
//!   cualquier tecla toca la siguiente nota, así la niña siente que toca una
//!   canción de verdad. La barra que sigue brilla para aprender la melodía.

use super::{Activity, Hover, IdlePrompt};
use crate::audio::{Inst, Sfx};
use crate::game::{Cx, Key};
use crate::gfx::{Backdrop, Shape};
use crate::math::{hex, hsv, midi, v2, Rgba, V2, WHITE};

const SCALE: [f32; 8] = [60.0, 62.0, 64.0, 65.0, 67.0, 69.0, 71.0, 72.0];
const BAR_COLORS: [u32; 8] = [0xE53935, 0xFB8C00, 0xFDD835, 0x43A047, 0x00ACC1, 0x1E88E5, 0x8E24AA, 0xEC407A];

/// Estrellita, ¿dónde estás? (índices de barra: 0 = Do ... 7 = Do agudo)
const ESTRELLITA: [usize; 42] = [
    0, 0, 4, 4, 5, 5, 4, 3, 3, 2, 2, 1, 1, 0, 4, 4, 3, 3, 2, 2, 1, 4, 4, 3, 3, 2, 2, 1, 0, 0, 4, 4, 5, 5, 4, 3, 3, 2, 2,
    1, 1, 0,
];

/// Los pollitos dicen (misma melodía que "Alle meine Entchen").
const POLLITOS: [usize; 22] = [0, 1, 2, 3, 4, 4, 5, 5, 5, 5, 4, 3, 3, 3, 3, 2, 2, 1, 1, 1, 1, 0];

struct Song {
    name: &'static str,
    emoji: &'static str,
    notes: &'static [usize],
}

const SONGS: [Song; 2] = [
    Song { name: "Estrellita", emoji: "⭐", notes: &ESTRELLITA },
    Song { name: "Los pollitos dicen", emoji: "🐥", notes: &POLLITOS },
];

struct SongButton {
    pos: V2,
    r: f32,
    hover: Hover,
}

struct Bar {
    pos: V2,
    half: V2,
    color: Rgba,
    flash: f32,
    hover: Hover,
}

struct InstButton {
    inst: Inst,
    emoji: &'static str,
    name: &'static str,
    pos: V2,
    r: f32,
    hover: Hover,
}

pub struct Music {
    bars: Vec<Bar>,
    buttons: Vec<InstButton>,
    inst: usize,
    song_buttons: Vec<SongButton>,
    /// (canción, nota siguiente)
    song: Option<(usize, usize)>,
    idle: IdlePrompt,
}

const ROWS: [(&[&str], f32); 4] = [
    (&["KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyY", "KeyU", "KeyI", "KeyO", "KeyP"], 12.0),
    (&["KeyA", "KeyS", "KeyD", "KeyF", "KeyG", "KeyH", "KeyJ", "KeyK", "KeyL", "Semicolon"], 0.0),
    (&["KeyZ", "KeyX", "KeyC", "KeyV", "KeyB", "KeyN", "KeyM", "Comma", "Period", "Slash"], -12.0),
    (&["Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8", "Digit9", "Digit0"], 0.0),
];

impl Music {
    pub fn new(cx: &mut Cx) -> Music {
        let bars = (0..8)
            .map(|i| Bar { pos: V2::ZERO, half: v2(1.0, 1.0), color: hex(BAR_COLORS[i]), flash: 0.0, hover: Hover::default() })
            .collect();
        let b = |inst, emoji, name| InstButton { inst, emoji, name, pos: V2::ZERO, r: 1.0, hover: Hover::default() };
        let buttons = vec![
            b(Inst::Marimba, "🎵", "Marimba"),
            b(Inst::Piano, "🎹", "Piano"),
            b(Inst::Bell, "🔔", "Campana"),
            b(Inst::Pluck, "🎸", "Guitarra"),
            b(Inst::Drum, "🥁", "Tambor"),
        ];
        let mut m = Music {
            bars,
            buttons,
            inst: 0,
            song_buttons: SONGS.iter().map(|_| SongButton { pos: V2::ZERO, r: 1.0, hover: Hover::default() }).collect(),
            song: None,
            idle: IdlePrompt::new(12.0),
        };
        m.layout(cx);
        m
    }

    fn layout(&mut self, cx: &Cx) {
        let n = self.bars.len() as f32;
        let left = cx.w * 0.07;
        let cell = (cx.w * 0.86) / n;
        let base_h = cx.h * 0.27;
        for (i, b) in self.bars.iter_mut().enumerate() {
            b.half = v2(cell * 0.38, base_h * (1.0 - i as f32 * 0.055));
            b.pos = v2(left + (i as f32 + 0.5) * cell, cx.h * 0.6);
        }
        let r = (cx.unit() * 0.055).clamp(24.0, 48.0);
        let ni = self.buttons.len();
        let nb = (ni + self.song_buttons.len()) as f32;
        let gap = r * 2.6;
        // Un espacio extra separa instrumentos de canciones.
        let x0 = cx.w * 0.5 - nb * gap * 0.5;
        for (i, b) in self.buttons.iter_mut().enumerate() {
            b.pos = v2(x0 + i as f32 * gap, r * 1.6);
            b.r = r;
        }
        for (i, b) in self.song_buttons.iter_mut().enumerate() {
            b.pos = v2(x0 + (ni + i) as f32 * gap + gap, r * 1.6);
            b.r = r;
        }
    }

    fn play(&mut self, cx: &mut Cx, i: usize, octave: f32) {
        let b = &mut self.bars[i];
        b.flash = 1.0;
        let inst = self.buttons[self.inst].inst;
        let freq = if inst == Inst::Drum { i as f32 } else { midi(SCALE[i] + octave) };
        cx.au.note(freq, inst, b.pos.x, 0.7);
        let top = b.pos - v2(0.0, b.half.y);
        let mut p = crate::fx::Particle::new(
            top,
            v2(cx.rng.range(-40.0, 40.0), -cx.rng.range(120.0, 200.0)),
            1.6,
            (b.half.x * 0.7).max(14.0) * if octave > 0.0 { 0.7 } else if octave < 0.0 { 1.3 } else { 1.0 },
            b.color,
            crate::fx::Look::Text(if cx.rng.chance(0.5) { "♪" } else { "♫" }),
        );
        p.spin = cx.rng.range(-1.0, 1.0);
        cx.fx.add(p);
        cx.fx.flash(b.pos, b.half.x * 2.0, b.color.alpha(0.5));
    }

    /// Siguiente nota de la canción. Devuelve false si no hay canción activa.
    fn song_step(&mut self, cx: &mut Cx) -> bool {
        let Some((s, i)) = self.song else { return false };
        let song = &SONGS[s];
        self.play(cx, song.notes[i], 0.0);
        if i + 1 >= song.notes.len() {
            self.song = None;
            cx.au.sfx(Sfx::Applause, 1.0, cx.w * 0.5);
            cx.au.say_after(0.8, &format!("¡Bravo! ¡Tocaste {}!", song.name));
            for k in 0..5 {
                cx.fx.rocket(&mut cx.rng, v2(cx.w * (0.1 + 0.2 * k as f32), cx.h), cx.h * 0.6);
            }
        } else {
            self.song = Some((s, i + 1));
        }
        true
    }

    fn toggle_song(&mut self, cx: &mut Cx, s: usize) {
        let pos = self.song_buttons[s].pos;
        if matches!(self.song, Some((cur, _)) if cur == s) {
            self.song = None;
            cx.au.say("Música libre.");
        } else {
            self.song = Some((s, 0));
            cx.au.say(&format!("¡{}! Aprieta cualquier tecla para tocar la canción.", SONGS[s].name));
            cx.au.sfx(Sfx::Magic, 1.0, pos.x);
        }
        cx.fx.sparkle(&mut cx.rng, pos, 16, hsv(0.14, 0.8, 1.0), 60.0);
    }

    fn bar_at(&self, p: V2) -> Option<usize> {
        self.bars.iter().position(|b| (p.x - b.pos.x).abs() < b.half.x * 1.25 && (p.y - b.pos.y).abs() < b.half.y + 10.0)
    }
}

impl Activity for Music {
    fn enter(&mut self, cx: &mut Cx) {
        cx.au.say("¡Música! Toca las barras de colores.");
    }

    fn resize(&mut self, cx: &mut Cx) {
        self.layout(cx);
    }

    fn update(&mut self, cx: &mut Cx) {
        cx.g.background(hex(0x9BE7FF), hex(0xFFF1B8), Backdrop::Bokeh, 0.0);
        let dt = cx.dt;
        self.idle.tick(cx, "¡Pasa el mouse por las barras!");
        let mouse = cx.inp.mouse;
        let has_mouse = cx.inp.has_mouse;
        let t = cx.t;

        // Botones de instrumentos y estrella.
        let mut announce = None;
        for (i, b) in self.buttons.iter_mut().enumerate() {
            if b.hover.update(cx, has_mouse && mouse.dist(b.pos) < b.r * 1.1) {
                announce = Some(i);
            }
        }
        if let Some(i) = announce {
            let name = self.buttons[i].name;
            cx.au.say_if_quiet(name);
        }
        let mut song_hover = None;
        for (i, b) in self.song_buttons.iter_mut().enumerate() {
            if b.hover.update(cx, has_mouse && mouse.dist(b.pos) < b.r * 1.1) {
                song_hover = Some(i);
            }
        }
        if let Some(i) = song_hover {
            cx.au.say_if_quiet(SONGS[i].name);
        }
        for (i, b) in self.buttons.iter().enumerate() {
            let sel = i == self.inst;
            let r = b.r * (1.0 + 0.15 * b.hover.anim + if sel { 0.15 } else { 0.0 });
            cx.g.shadow(Shape::Circle, b.pos, r, 0.0, 1.0);
            cx.g.sticker(Shape::Circle, b.pos, r, 0.0, if sel { hex(0xFFF59D) } else { WHITE.alpha(0.92) });
            cx.g.emoji(b.emoji, b.pos, r * 0.62, if sel { (t * 4.0).sin() * 0.1 } else { 0.0 }, 1.0);
        }
        for (i, b) in self.song_buttons.iter().enumerate() {
            let on = matches!(self.song, Some((s, _)) if s == i);
            let sr = b.r * (1.0 + 0.15 * b.hover.anim + if on { 0.15 + 0.05 * (t * 5.0).sin() } else { 0.0 });
            if on {
                cx.g.glow(b.pos, sr * 2.2, hsv(0.14, 0.7, 1.0).alpha(0.6));
            }
            cx.g.shadow(Shape::Circle, b.pos, sr, 0.0, 1.0);
            cx.g.sticker(Shape::Circle, b.pos, sr, 0.0, if on { hex(0xFFF59D) } else { WHITE.alpha(0.92) });
            let wiggle = if on { (t * 4.0).sin() * 0.15 } else { 0.0 };
            cx.g.emoji(SONGS[i].emoji, b.pos, sr * 0.65, wiggle, 1.0);
        }

        // Barras.
        let next_bar = self.song.map(|(s, i)| SONGS[s].notes[i]);
        for (i, b) in self.bars.iter_mut().enumerate() {
            b.flash = (b.flash - dt * 2.5).max(0.0);
            let k = 1.0 + b.flash * 0.08;
            let p = b.pos + v2(0.0, -b.flash * 12.0);
            let half = v2(b.half.x * k, b.half.y * k);
            if next_bar == Some(i) {
                let pulse = (t * 6.0).sin() * 0.5 + 0.5;
                cx.g.shape_full(Shape::RoundRect, p, half + v2(14.0, 14.0), 0.0, WHITE.alpha(0.35 + 0.4 * pulse), 0.45, 10.0);
            }
            cx.g.shape_full(Shape::RoundRect, p + v2(0.0, 8.0), half, 0.0, Rgba { r: 0.0, g: 0.0, b: 0.1, a: 0.3 }, 0.45, 12.0);
            cx.g.shape_full(Shape::RoundRect, p, half + v2(4.0, 4.0), 0.0, b.color.darker(0.4), 0.45, 0.0);
            cx.g.shape_full(Shape::RoundRect, p, half, 0.0, b.color.lighter(b.flash * 0.6), 0.45, 0.0);
            // Brillo y "clavos" del xilófono.
            cx.g.shape_full(Shape::RoundRect, p - v2(half.x * 0.35, 0.0), v2(half.x * 0.18, half.y * 0.8), 0.0, WHITE.alpha(0.25), 1.0, 4.0);
            cx.g.circle(p - v2(0.0, half.y * 0.8), half.x * 0.16, WHITE.alpha(0.7));
            cx.g.circle(p + v2(0.0, half.y * 0.8), half.x * 0.16, WHITE.alpha(0.7));
            if b.flash > 0.0 {
                cx.g.glow(p, half.y * 1.2, b.color.alpha(b.flash * 0.5));
            }
        }
    }

    fn moved(&mut self, cx: &mut Cx, _from: V2, to: V2) {
        // Glissando: al entrar a una barra con el mouse, suena.
        if self.song.is_some() {
            return;
        }
        let over = self.bar_at(to);
        for i in 0..self.bars.len() {
            if self.bars[i].hover.update(cx, over == Some(i)) {
                self.play(cx, i, 0.0);
            }
        }
    }

    fn down(&mut self, cx: &mut Cx, p: V2, _button: u32) {
        if let Some(s) = self.song_buttons.iter().position(|b| p.dist(b.pos) < b.r * 1.2) {
            self.toggle_song(cx, s);
            return;
        }
        if let Some(i) = self.buttons.iter().position(|b| p.dist(b.pos) < b.r * 1.2) {
            self.inst = i;
            let b = &self.buttons[i];
            cx.au.say(b.name);
            cx.fx.sparkle(&mut cx.rng, b.pos, 12, WHITE, 50.0);
            let demo = if b.inst == Inst::Drum { 0.0 } else { midi(67.0) };
            cx.au.note(demo, b.inst, b.pos.x, 0.6);
            return;
        }
        if self.song_step(cx) {
            return;
        }
        if let Some(i) = self.bar_at(p) {
            self.play(cx, i, 0.0);
        } else {
            cx.fx.ripple(p, 50.0, hsv(cx.rng.f(), 0.5, 1.0));
        }
    }

    fn key(&mut self, cx: &mut Cx, k: &Key) {
        if let Some(dir) = k.arrow() {
            if dir.y != 0.0 {
                // Arriba/abajo: cambiar instrumento.
                let n = self.buttons.len() as i32;
                self.inst = (self.inst as i32 + if dir.y < 0.0 { -1 } else { 1 }).rem_euclid(n) as usize;
                let b = &self.buttons[self.inst];
                cx.au.say(b.name);
                cx.fx.sparkle(&mut cx.rng, b.pos, 12, WHITE, 50.0);
            } else {
                // Izquierda/derecha: escala completa.
                let i = if dir.x > 0.0 { 7 } else { 0 };
                self.play(cx, i, 0.0);
            }
            return;
        }
        if self.song_step(cx) {
            return;
        }
        if k.is("Space") {
            // Bombo (tambor 0) y todas las barras saltan.
            cx.au.note(0.0, Inst::Drum, cx.w * 0.5, 0.9);
            for b in self.bars.iter_mut() {
                b.flash = b.flash.max(0.4);
            }
            return;
        }
        if k.is_enter() {
            // Acorde de Do mayor.
            for i in [0, 2, 4, 7] {
                self.play(cx, i, 0.0);
            }
            return;
        }
        for (row, octave) in ROWS {
            if let Some(i) = row.iter().position(|c| *c == k.code) {
                let extra = if i >= 8 { 12.0 } else { 0.0 };
                self.play(cx, i % 8, octave + extra);
                return;
            }
        }
        let i = cx.rng.int(8);
        self.play(cx, i, 0.0);
    }

    fn wheel(&mut self, cx: &mut Cx, dy: f32) -> bool {
        // La rueda recorre las barras hacia arriba o abajo.
        let i = self.bars.iter().position(|b| b.flash > 0.7).unwrap_or(0) as i32;
        let next = (i + if dy < 0.0 { 1 } else { -1 }).rem_euclid(8) as usize;
        self.play(cx, next, 0.0);
        true
    }

    fn cursor_trail(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    #[test]
    fn song_finishes_and_celebrates() {
        let mut g = Game::new(1280.0, 720.0, 9);
        let mut m = Music::new(&mut g.cx);
        for (s, song) in SONGS.iter().enumerate() {
            m.toggle_song(&mut g.cx, s);
            for _ in 0..song.notes.len() {
                assert!(m.song_step(&mut g.cx));
            }
            assert!(m.song.is_none());
            assert!(!m.song_step(&mut g.cx));
        }
    }
}
