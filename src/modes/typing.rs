//! Escribir: aparece un dibujo y su palabra con las letras "fantasma".
//! Abajo hay un teclado en pantalla donde brilla la tecla que toca buscar.
//! - Si aprieta la tecla correcta, la letra se escribe y la voz la nombra.
//! - Si se equivoca, la voz le dice qué letra apretó y cuál buscar. Después
//!   de dos intentos, cualquier tecla escribe la letra (nunca se queda pegada).
//! La primera palabra es su nombre, si papá o mamá lo escribieron.

use super::{Activity, Hover, IdlePrompt};
use crate::audio::{Inst, Sfx};
use crate::game::{Cx, Key};
use crate::gfx::{Backdrop, Shape};
use crate::math::{ease_out_back, hex, hsv, pentatonic, v2, Rgba, V2, WHITE};
use crate::vocab::{self, Word};

const ROWS: [&str; 3] = ["QWERTYUIOP", "ASDFGHJKLÑ", "ZXCVBNM"];
/// Intentos fallidos antes de que cualquier tecla sirva.
const HELP_AFTER: u32 = 2;

struct Slot {
    /// Letra tal como se muestra (puede tener tilde).
    shown: String,
    /// Tecla que la escribe (sin tilde). None = no es letra (espacio, guion).
    key: Option<char>,
    done: bool,
    pop: f32,
}

struct KeyCap {
    ch: char,
    pos: V2,
    half: f32,
    flash: f32,
    hover: Hover,
}

/// La palabra que se está escribiendo.
struct Target {
    emoji: &'static str,
    /// Lo que dice la voz al anunciarla y al terminarla.
    spoken: String,
    done_speech: String,
}

pub struct Typing {
    slots: Vec<Slot>,
    target: Target,
    cursor: usize,
    wrong: u32,
    caps: Vec<KeyCap>,
    word_i: usize,
    name_pending: bool,
    finished: Option<f32>,
    idle: IdlePrompt,
    nag_cooldown: f32,
    appear: f32,
    pic_bounce: f32,
}

impl Typing {
    pub fn new(cx: &mut Cx) -> Typing {
        let mut t = Typing {
            slots: Vec::new(),
            target: Target {
                emoji: "⭐",
                spoken: String::new(),
                done_speech: String::new(),
            },
            cursor: 0,
            wrong: 0,
            caps: Vec::new(),
            word_i: 0,
            name_pending: !cx.name.is_empty(),
            finished: None,
            idle: IdlePrompt::new(10.0),
            nag_cooldown: 0.0,
            appear: 0.0,
            pic_bounce: 0.0,
        };
        t.layout(cx);
        t.next_word(cx, false);
        t
    }

    fn layout(&mut self, cx: &Cx) {
        let kw = (cx.w * 0.92 / 10.6).min(cx.h * 0.38 / 3.4);
        let half = kw * 0.44;
        let top = cx.h * 0.6;
        let mut caps = Vec::new();
        for (r, row) in ROWS.iter().enumerate() {
            let n = row.chars().count() as f32;
            // Escalonado como un teclado real.
            let x0 = cx.w * 0.5 - (n - 1.0) * kw * 0.5 + r as f32 * kw * 0.25;
            for (i, ch) in row.chars().enumerate() {
                let old = self.caps.iter().find(|c| c.ch == ch);
                caps.push(KeyCap {
                    ch,
                    pos: v2(x0 + i as f32 * kw, top + r as f32 * kw * 1.05),
                    half,
                    flash: old.map(|c| c.flash).unwrap_or(0.0),
                    hover: Hover::default(),
                });
            }
        }
        self.caps = caps;
    }

    fn set_word(&mut self, text: &str, target: Target) {
        self.slots = text
            .chars()
            .map(|c| {
                let shown: String = c.to_uppercase().collect();
                let key = vocab::fold_letter(c);
                Slot {
                    shown,
                    key,
                    done: key.is_none(),
                    pop: 0.0,
                }
            })
            .collect();
        self.target = target;
        self.cursor = 0;
        self.skip_non_letters();
        self.wrong = 0;
        self.finished = None;
        self.appear = 0.0;
    }

    fn next_word(&mut self, cx: &mut Cx, speak: bool) {
        if self.name_pending {
            self.name_pending = false;
            let name = cx.name.clone();
            self.set_word(
                &name,
                Target {
                    emoji: "⭐",
                    spoken: "¡Vamos a escribir tu nombre!".to_string(),
                    done_speech: format!("¡{}! ¡Escribiste tu nombre!", name),
                },
            );
        } else {
            let w: Word = vocab::WRITE_WORDS[self.word_i % vocab::WRITE_WORDS.len()];
            self.word_i += 1;
            let mut done = format!("¡{}!", capitalize(w.word));
            if !w.sound.is_empty() {
                done.push(' ');
                done.push_str(w.sound);
            }
            self.set_word(
                w.word,
                Target {
                    emoji: w.emoji,
                    spoken: format!("Vamos a escribir: {}.", w.word),
                    done_speech: done,
                },
            );
        }
        if speak {
            self.announce(cx);
        }
    }

    fn announce(&mut self, cx: &mut Cx) {
        cx.au.say(&self.target.spoken.clone());
        if let Some(name) = self.expected().and_then(vocab::letter_info).map(|l| l.name) {
            cx.au.say_after(1.9, &format!("Busca la {}.", name));
        }
    }

    fn skip_non_letters(&mut self) {
        while self.cursor < self.slots.len() && self.slots[self.cursor].key.is_none() {
            self.cursor += 1;
        }
    }

    fn expected(&self) -> Option<char> {
        self.slots.get(self.cursor).and_then(|s| s.key)
    }

    fn slot_layout(&self, cx: &Cx) -> (f32, f32, f32) {
        let n = self.slots.len().max(1) as f32;
        let size = (cx.w * 0.86 / n).min(cx.h * 0.17);
        let x0 = cx.w * 0.5 - (n - 1.0) * size * 0.5;
        (x0, cx.h * 0.4, size)
    }

    /// Escribe la letra actual (bien por mérito o por ayuda).
    fn type_current(&mut self, cx: &mut Cx, earned: bool) {
        let Some(ch) = self.expected() else { return };
        let (x0, y, size) = self.slot_layout(cx);
        let p = v2(x0 + self.cursor as f32 * size, y);
        let slot = &mut self.slots[self.cursor];
        slot.done = true;
        slot.pop = 1.0;
        let step = self.cursor as i32;
        self.cursor += 1;
        self.skip_non_letters();
        self.wrong = 0;
        if let Some(cap) = self.caps.iter_mut().find(|c| c.ch == ch) {
            cap.flash = 1.0;
        }
        cx.au
            .note(pentatonic(step + 5, 60.0), Inst::Marimba, p.x, 0.6);
        if earned {
            cx.fx.confetti(&mut cx.rng, p, 18, 420.0);
        } else {
            cx.fx.sparkle(&mut cx.rng, p, 10, WHITE, 40.0);
        }
        let name = vocab::letter_info(ch).map(|l| l.name).unwrap_or("");
        if self.cursor >= self.slots.len() {
            // ¡Palabra completa!
            cx.au.say(name);
            cx.au.say_after(0.7, &self.target.done_speech.clone());
            cx.au.sfx(Sfx::Fanfare, 1.0, cx.w * 0.5);
            self.finished = Some(3.2);
            self.pic_bounce = 1.0;
            for i in 0..3 {
                cx.fx.rocket(
                    &mut cx.rng,
                    v2(cx.w * (0.25 + 0.25 * i as f32), cx.h),
                    cx.h * 0.5,
                );
            }
        } else {
            cx.au.say(name);
            if let Some(next) = self.expected().and_then(vocab::letter_info).map(|l| l.name) {
                cx.au.say_after(1.0, &format!("Ahora la {}.", next));
            }
        }
    }

    fn press(&mut self, cx: &mut Cx, letter: char) {
        if self.finished.is_some() {
            return;
        }
        let Some(want) = self.expected() else { return };
        if letter == want {
            self.type_current(cx, true);
            return;
        }
        if self.wrong >= HELP_AFTER {
            self.type_current(cx, false);
            return;
        }
        self.wrong += 1;
        if let Some(cap) = self.caps.iter_mut().find(|c| c.ch == letter) {
            cap.flash = 0.6;
        }
        cx.au.sfx_vol(Sfx::Oops, 1.0, cx.w * 0.5, 0.6);
        if self.nag_cooldown <= 0.0 {
            let want_name = vocab::letter_info(want).map(|l| l.name).unwrap_or("");
            match vocab::letter_info(letter) {
                Some(got) => cx
                    .au
                    .say(&format!("Esa es la {}. Busca la {}.", got.name, want_name)),
                None => cx.au.say(&format!("Busca la {}.", want_name)),
            }
            self.nag_cooldown = 2.5;
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

impl Activity for Typing {
    fn enter(&mut self, cx: &mut Cx) {
        cx.au.say("¡A escribir!");
        let spoken = self.target.spoken.clone();
        cx.au.say_after(1.2, &spoken);
    }

    fn resize(&mut self, cx: &mut Cx) {
        self.layout(cx);
    }

    fn update(&mut self, cx: &mut Cx) {
        cx.g.background(hex(0xE3F2FD), hex(0xFFF8E1), Backdrop::Bokeh, 0.0);
        let dt = cx.dt;
        let t = cx.t;
        self.appear += dt;
        self.nag_cooldown -= dt;
        self.pic_bounce = (self.pic_bounce - dt * 0.8).max(0.0);
        if let Some(name) = self.expected().and_then(vocab::letter_info).map(|l| l.name) {
            if self.finished.is_none() {
                self.idle
                    .tick(cx, &format!("Busca la {} en el teclado.", name));
            }
        }
        if let Some(f) = self.finished.as_mut() {
            *f -= dt;
            if *f <= 0.0 {
                self.next_word(cx, true);
            }
        }

        // Dibujo de la palabra (clic = la dice).
        let pic = v2(cx.w * 0.5, cx.h * 0.17);
        let pr = cx.unit()
            * 0.1
            * ease_out_back((self.appear / 0.5).min(1.0))
            * (1.0 + 0.2 * (self.pic_bounce * 12.0).sin() * self.pic_bounce);
        cx.g.glow(pic, pr * 1.8, WHITE.alpha(0.7));
        cx.g.emoji_shadow(self.target.emoji, pic, pr, 0.0, 1.0);
        cx.g.emoji(self.target.emoji, pic, pr, (t * 1.5).sin() * 0.06, 1.0);

        // Casillas con las letras.
        let (x0, y, size) = self.slot_layout(cx);
        for (i, s) in self.slots.iter_mut().enumerate() {
            s.pop = (s.pop - dt * 2.0).max(0.0);
            if s.key.is_none() {
                continue;
            }
            let p = v2(x0 + i as f32 * size, y);
            let current = i == self.cursor && self.finished.is_none();
            let k = ease_out_back(((self.appear - i as f32 * 0.08) / 0.4).clamp(0.0, 1.0));
            let half = size * 0.44 * k;
            let lift = if current {
                (t * 4.0).sin().abs() * size * 0.05
            } else {
                0.0
            };
            let card = if current { hex(0xFFF59D) } else { WHITE };
            cx.g.shape_full(
                Shape::RoundRect,
                p + v2(0.0, 5.0),
                v2(half, half * 1.1),
                0.0,
                Rgba {
                    r: 0.1,
                    g: 0.1,
                    b: 0.3,
                    a: 0.15,
                },
                0.25,
                6.0,
            );
            cx.g.shape_full(
                Shape::RoundRect,
                p - v2(0.0, lift),
                v2(half, half * 1.1),
                0.0,
                card,
                0.25,
                0.0,
            );
            if current {
                cx.g.ring(
                    p - v2(0.0, lift),
                    half * 1.25,
                    3.0,
                    hex(0xFFB300).alpha(0.7 + 0.3 * (t * 6.0).sin()),
                );
            }
            let color = hsv(i as f32 * 0.13 + 0.95, 0.75, 0.95);
            let pop = 1.0 + 0.35 * (s.pop * 10.0).sin() * s.pop;
            if s.done {
                cx.g.text(
                    &s.shown,
                    p - v2(0.0, lift),
                    half * 0.95 * pop,
                    0.0,
                    color,
                    0.12,
                );
            } else {
                // Letra fantasma: se ve la forma para buscarla.
                let a = if current {
                    0.35 + 0.15 * (t * 4.0).sin()
                } else {
                    0.18
                };
                cx.g.text(
                    &s.shown,
                    p - v2(0.0, lift),
                    half * 0.95,
                    0.0,
                    Rgba {
                        r: 0.3,
                        g: 0.35,
                        b: 0.5,
                        a,
                    },
                    0.0,
                );
            }
        }

        // Teclado en pantalla con la tecla que hay que buscar.
        let want = if self.finished.is_none() {
            self.expected()
        } else {
            None
        };
        let mouse = cx.inp.mouse;
        let has_mouse = cx.inp.has_mouse;
        for cap in self.caps.iter_mut() {
            cap.flash = (cap.flash - dt * 2.5).max(0.0);
            let over = has_mouse
                && (mouse.x - cap.pos.x).abs() < cap.half
                && (mouse.y - cap.pos.y).abs() < cap.half;
            cap.hover.update(cx, over);
            let is_want = want == Some(cap.ch);
            let pulse = if is_want {
                (t * 5.0).sin() * 0.5 + 0.5
            } else {
                0.0
            };
            let h = cap.half * (1.0 + 0.08 * cap.hover.anim + 0.06 * pulse);
            let press = v2(0.0, cap.flash * h * 0.15);
            if is_want {
                cx.g.glow(cap.pos, h * 2.6, hex(0xFFD54F).alpha(0.4 + 0.4 * pulse));
            }
            cx.g.shape_full(
                Shape::RoundRect,
                cap.pos + v2(0.0, h * 0.18),
                v2(h, h),
                0.0,
                hex(0x78909C),
                0.25,
                0.0,
            );
            let face = if is_want { hex(0xFFE082) } else { WHITE }.mix(hex(0x80DEEA), cap.flash);
            cx.g.shape_full(
                Shape::RoundRect,
                cap.pos + press,
                v2(h, h),
                0.0,
                face,
                0.25,
                0.0,
            );
            let mut buf = [0u8; 4];
            let s: &str = cap.ch.encode_utf8(&mut buf);
            cx.g.text(s, cap.pos + press, h * 0.7, 0.0, hex(0x37474F), 0.0);
        }
    }

    fn key(&mut self, cx: &mut Cx, k: &Key) {
        if let Some(c) = k.letter() {
            self.press(cx, c);
        } else if k.is("Space") || k.is_enter() {
            self.announce(cx);
        } else if k.is_erase() {
            cx.au.say_if_quiet("¡Borrar! Aquí sólo se escriben letras.");
        } else if self.finished.is_none() {
            // Cualquier otra tecla también ayuda, como si fuera una letra equivocada.
            self.press(cx, '?');
        }
    }

    fn down(&mut self, cx: &mut Cx, p: V2, _button: u32) {
        if let Some(cap) = self
            .caps
            .iter()
            .find(|c| (p.x - c.pos.x).abs() < c.half && (p.y - c.pos.y).abs() < c.half)
        {
            let ch = cap.ch;
            self.press(cx, ch);
            return;
        }
        let pic = v2(cx.w * 0.5, cx.h * 0.17);
        if p.dist(pic) < cx.unit() * 0.12 {
            self.pic_bounce = 1.0;
            let s = self.target.done_speech.clone();
            cx.au.say(&s);
            return;
        }
        let c = hsv(cx.rng.f(), 0.6, 1.0);
        cx.fx.sparkle(&mut cx.rng, p, 8, c, 40.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    #[test]
    fn name_comes_first_and_accents_fold() {
        let mut g = Game::new(1280.0, 720.0, 1);
        g.set_name("Sofía");
        let mut t = Typing::new(&mut g.cx);
        let keys: Vec<char> = t.slots.iter().filter_map(|s| s.key).collect();
        assert_eq!(keys, vec!['S', 'O', 'F', 'I', 'A']);
        for c in keys {
            t.press(&mut g.cx, c);
        }
        assert!(t.finished.is_some());
    }

    #[test]
    fn wrong_keys_eventually_help() {
        let mut g = Game::new(1280.0, 720.0, 2);
        let mut t = Typing::new(&mut g.cx);
        let before = t.cursor;
        for _ in 0..=HELP_AFTER {
            t.press(&mut g.cx, '?');
        }
        assert_eq!(t.cursor, before + 1);
    }
}
