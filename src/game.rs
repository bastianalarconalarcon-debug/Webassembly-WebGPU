//! El "caparazón" del juego: guarda el estado compartido, reparte la
//! entrada (mouse/teclado) al juego activo, dibuja el botón de casa y
//! hace las transiciones entre juegos.

use std::collections::HashSet;

use crate::audio::{Audio, Sfx};
use crate::fx::Fx;
use crate::gfx::{Gfx, Shape};
use crate::math::{ease_out_back, ease_out_cubic, hsv, v2, Rgba, V2, WHITE};
use crate::modes::{self, Activity, ModeId};
use crate::rng::Rng;
use crate::vocab;

/// Nombres de `KeyboardEvent.code` que entiende el juego. JS lee esta lista
/// para traducir cada tecla a un número (0 = desconocida).
pub static KEY_CODES: &[&str] = &[
    "",
    "Space",
    "Enter",
    "NumpadEnter",
    "Backspace",
    "Delete",
    "Tab",
    "Escape",
    "CapsLock",
    "ShiftLeft",
    "ShiftRight",
    "ControlLeft",
    "ControlRight",
    "AltLeft",
    "AltRight",
    "MetaLeft",
    "MetaRight",
    "ContextMenu",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Insert",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    "KeyA",
    "KeyB",
    "KeyC",
    "KeyD",
    "KeyE",
    "KeyF",
    "KeyG",
    "KeyH",
    "KeyI",
    "KeyJ",
    "KeyK",
    "KeyL",
    "KeyM",
    "KeyN",
    "KeyO",
    "KeyP",
    "KeyQ",
    "KeyR",
    "KeyS",
    "KeyT",
    "KeyU",
    "KeyV",
    "KeyW",
    "KeyX",
    "KeyY",
    "KeyZ",
    "Digit0",
    "Digit1",
    "Digit2",
    "Digit3",
    "Digit4",
    "Digit5",
    "Digit6",
    "Digit7",
    "Digit8",
    "Digit9",
    "Numpad0",
    "Numpad1",
    "Numpad2",
    "Numpad3",
    "Numpad4",
    "Numpad5",
    "Numpad6",
    "Numpad7",
    "Numpad8",
    "Numpad9",
    "NumpadAdd",
    "NumpadSubtract",
    "NumpadMultiply",
    "NumpadDivide",
    "NumpadDecimal",
    "Semicolon",
    "Quote",
    "BracketLeft",
    "BracketRight",
    "Backslash",
    "Comma",
    "Period",
    "Slash",
    "Minus",
    "Equal",
    "Backquote",
    "IntlBackslash",
];

/// Una tecla presionada.
#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub code: &'static str,
    /// Carácter que produce la tecla según la distribución del teclado.
    pub ch: Option<char>,
    pub repeat: bool,
}

impl Key {
    pub fn new(code_idx: u32, ch: u32, repeat: bool) -> Key {
        Key {
            code: KEY_CODES.get(code_idx as usize).copied().unwrap_or(""),
            ch: char::from_u32(ch).filter(|c| *c != '\0'),
            repeat,
        }
    }

    /// Letra mayúscula (A-Z, Ñ), sin tilde.
    pub fn letter(&self) -> Option<char> {
        self.ch.and_then(vocab::fold_letter)
    }

    pub fn digit(&self) -> Option<u32> {
        if let Some(d) = self.ch.and_then(|c| c.to_digit(10)) {
            return Some(d);
        }
        let s = self
            .code
            .strip_prefix("Digit")
            .or_else(|| self.code.strip_prefix("Numpad"))?;
        if self.ch.is_none() {
            s.parse().ok()
        } else {
            None
        }
    }

    /// Dirección de las flechas (y también WASD).
    pub fn arrow(&self) -> Option<V2> {
        match self.code {
            "ArrowUp" => Some(v2(0.0, -1.0)),
            "ArrowDown" => Some(v2(0.0, 1.0)),
            "ArrowLeft" => Some(v2(-1.0, 0.0)),
            "ArrowRight" => Some(v2(1.0, 0.0)),
            _ => None,
        }
    }

    pub fn is(&self, code: &str) -> bool {
        self.code == code
    }

    pub fn is_enter(&self) -> bool {
        self.code == "Enter" || self.code == "NumpadEnter"
    }

    pub fn is_erase(&self) -> bool {
        self.code == "Backspace" || self.code == "Delete"
    }
}

pub struct Input {
    pub mouse: V2,
    pub has_mouse: bool,
    pub buttons: [bool; 3],
    pub held: HashSet<&'static str>,
    /// Segundos sin tocar nada.
    pub idle: f32,
}

impl Input {
    pub fn held(&self, code: &str) -> bool {
        self.held.contains(code)
    }
    pub fn any_button(&self) -> bool {
        self.buttons.iter().any(|b| *b)
    }
}

/// Estado compartido que reciben todos los juegos.
pub struct Cx {
    pub g: Gfx,
    pub au: Audio,
    pub rng: Rng,
    pub fx: Fx,
    pub inp: Input,
    pub w: f32,
    pub h: f32,
    /// Tiempo total en segundos.
    pub t: f32,
    pub dt: f32,
    /// Nombre de la niña (vacío si no se configuró).
    pub name: String,
    /// Un juego pide cambiar a otro.
    pub goto: Option<(ModeId, V2, Rgba)>,
    /// Mundo de bloques del juego Construir (vive aquí para no perderse al ir a la casa).
    pub world: crate::voxel::Sim,
    /// ¿Ya se armó el mundo inicial de Construir?
    pub build_seeded: bool,
}

impl Cx {
    pub fn center(&self) -> V2 {
        v2(self.w * 0.5, self.h * 0.5)
    }
    /// Lado menor de la pantalla: la unidad para tamaños responsivos.
    pub fn unit(&self) -> f32 {
        self.w.min(self.h)
    }
    pub fn go(&mut self, to: ModeId, from: V2, color: Rgba) {
        if self.goto.is_none() {
            self.goto = Some((to, from, color));
        }
    }
}

struct Transition {
    t: f32,
    to: ModeId,
    origin: V2,
    color: Rgba,
    switched: bool,
}

const TRANSITION: f32 = 0.8;
const EXIT_HOLD: f32 = 1.5;

pub struct Game {
    pub cx: Cx,
    mode: Box<dyn Activity>,
    pub mode_id: ModeId,
    started: bool,
    trans: Option<Transition>,
    home_anim: f32,
    home_hovered: bool,
    esc_hold: f32,
    trail_acc: f32,
}

impl Game {
    pub fn new(w: f32, h: f32, seed: u32) -> Game {
        let mut cx = Cx {
            g: Gfx::new(),
            au: Audio::new(),
            rng: Rng::new(seed),
            fx: Fx::default(),
            inp: Input {
                mouse: v2(w * 0.5, h * 0.5),
                has_mouse: false,
                buttons: [false; 3],
                held: HashSet::new(),
                idle: 0.0,
            },
            w,
            h,
            t: 0.0,
            dt: 0.0,
            name: String::new(),
            goto: None,
            world: crate::voxel::Sim::default(),
            build_seeded: false,
        };
        cx.au.width = w;
        preload_glyphs(&mut cx.g);
        Game {
            mode: modes::make(ModeId::Home, &mut cx),
            cx,
            mode_id: ModeId::Home,
            started: false,
            trans: None,
            home_anim: 0.0,
            home_hovered: false,
            esc_hold: 0.0,
            trail_acc: 0.0,
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        self.cx.w = w.max(1.0);
        self.cx.h = h.max(1.0);
        self.cx.au.width = w;
        self.mode.resize(&mut self.cx);
    }

    pub fn set_name(&mut self, name: &str) {
        self.cx.name = name.trim().chars().take(16).collect();
        // La pantalla de inicio muestra el nombre: rehacerla.
        if self.mode_id == ModeId::Home && self.trans.is_none() {
            self.mode = modes::make(ModeId::Home, &mut self.cx);
        }
    }

    /// Juego activo (para pruebas automáticas).
    pub fn mode_index(&self) -> u32 {
        self.mode_id as u32
    }

    fn busy(&self) -> bool {
        self.trans.is_some()
    }

    fn home_button(&self) -> (V2, f32) {
        let r = (self.cx.unit() * 0.06).clamp(28.0, 50.0);
        (v2(r + 18.0, r + 18.0), r)
    }

    pub fn update(&mut self, dt: f32, speaking: bool) {
        let dt = dt.clamp(0.0, 0.05);
        {
            let cx = &mut self.cx;
            cx.dt = dt;
            cx.t += dt;
            cx.inp.idle += dt;
            cx.au.speaking = speaking;
            cx.au.tick(dt);
            cx.g.begin_frame();
        }
        if !self.started {
            self.started = true;
            self.mode.enter(&mut self.cx);
        }

        // Mantener Esc presionado = salir (para papá/mamá).
        if self.cx.inp.held("Escape") {
            if self.esc_hold >= 0.0 {
                self.esc_hold += dt;
                if self.esc_hold >= EXIT_HOLD {
                    self.cx.au.exit_request();
                    self.esc_hold = -1.0;
                }
            }
        } else {
            self.esc_hold = 0.0;
        }

        self.mode.update(&mut self.cx);
        self.cx.g.d3.finish();

        let cx = &mut self.cx;
        cx.fx.update(dt, &mut cx.rng);
        cx.fx.draw(&mut cx.g);

        if self.mode_id != ModeId::Home {
            self.draw_home_button();
        }
        self.draw_cursor_halo();
        self.draw_exit_ring();
        self.update_transition();

        if let Some((to, from, color)) = self.cx.goto.take() {
            if self.trans.is_none() && to != self.mode_id {
                self.cx.au.sfx(Sfx::Whoosh, 1.0, from.x);
                self.trans = Some(Transition {
                    t: 0.0,
                    to,
                    origin: from,
                    color,
                    switched: false,
                });
            }
        }
    }

    fn draw_home_button(&mut self) {
        let (p, r) = self.home_button();
        let cx = &mut self.cx;
        let hovered = cx.inp.has_mouse && cx.inp.mouse.dist(p) < r * 1.15;
        if hovered && !self.home_hovered {
            cx.au.sfx_vol(Sfx::Tick, 1.2, p.x, 0.6);
            cx.au.say_if_quiet("Casa");
        }
        self.home_hovered = hovered;
        self.home_anim =
            crate::math::approach(self.home_anim, if hovered { 1.0 } else { 0.0 }, 12.0, cx.dt);
        let k = 1.0 + 0.18 * self.home_anim;
        let wob = (cx.t * 9.0).sin() * 0.08 * self.home_anim;
        cx.g.shadow(Shape::Circle, p, r * k, 0.0, 1.0);
        cx.g.sticker(
            Shape::Circle,
            p,
            r * k,
            0.0,
            hsv(0.12, 0.25 + 0.2 * self.home_anim, 1.0),
        );
        cx.g.emoji("🏠", p, r * k * 0.62, wob, 1.0);
    }

    fn draw_cursor_halo(&mut self) {
        let cx = &mut self.cx;
        if !cx.inp.has_mouse {
            return;
        }
        let pulse = 0.85 + 0.15 * (cx.t * 4.0).sin();
        cx.g.glow(
            cx.inp.mouse,
            38.0 * pulse,
            hsv(cx.t * 0.1, 0.4, 1.0).alpha(0.35),
        );
    }

    fn draw_exit_ring(&mut self) {
        if self.esc_hold <= 0.15 {
            return;
        }
        let cx = &mut self.cx;
        let p = v2(cx.w * 0.5, 70.0);
        let k = (self.esc_hold / EXIT_HOLD).clamp(0.0, 1.0);
        cx.g.circle(p, 46.0, WHITE.alpha(0.9));
        // Anillo que se llena: puntos alrededor.
        let n = 24;
        for i in 0..n {
            let f = i as f32 / n as f32;
            let a = f * crate::math::TAU - crate::math::PI * 0.5;
            let c = if f <= k {
                hsv(0.55, 0.7, 0.9)
            } else {
                Rgba {
                    r: 0.8,
                    g: 0.8,
                    b: 0.85,
                    a: 0.8,
                }
            };
            cx.g.circle(p + V2::from_angle(a) * 38.0, 5.0, c);
        }
        cx.g.emoji("🚪", p, 22.0, 0.0, 1.0);
    }

    fn update_transition(&mut self) {
        let Some(tr) = self.trans.as_mut() else {
            return;
        };
        tr.t += self.cx.dt;
        let half = TRANSITION * 0.5;
        if tr.t >= half && !tr.switched {
            tr.switched = true;
            let to = tr.to;
            self.cx.fx.clear();
            self.cx.au.hush();
            self.mode = modes::make(to, &mut self.cx);
            self.mode_id = to;
            self.home_hovered = false;
            self.mode.enter(&mut self.cx);
        }
        let tr = self.trans.as_ref().unwrap();
        let cx = &mut self.cx;
        let max_r = (cx.w * cx.w + cx.h * cx.h).sqrt() * 1.05;
        let (center, r) = if tr.t < half {
            (tr.origin, ease_out_cubic(tr.t / half) * max_r)
        } else {
            (
                cx.center(),
                (1.0 - ease_out_back((tr.t - half) / half).min(1.0)) * max_r,
            )
        };
        if r > 1.0 {
            cx.g.circle(center, r, tr.color);
            cx.g.ring(center, r, 14.0, WHITE.alpha(0.6));
        }
        if tr.t >= TRANSITION {
            self.trans = None;
        }
    }

    // --- Entrada ---------------------------------------------------------

    pub fn pointer_move(&mut self, x: f32, y: f32) {
        let to = v2(x, y);
        let from = if self.cx.inp.has_mouse {
            self.cx.inp.mouse
        } else {
            to
        };
        self.cx.inp.mouse = to;
        self.cx.inp.has_mouse = true;
        self.cx.inp.idle = 0.0;
        if self.busy() {
            return;
        }
        // Estela de estrellitas detrás del cursor.
        if self.mode.cursor_trail() {
            self.trail_acc += from.dist(to);
            while self.trail_acc > 28.0 {
                self.trail_acc -= 28.0;
                let c = hsv(self.cx.t * 0.2 + self.cx.rng.f() * 0.2, 0.6, 1.0);
                self.cx.fx.sparkle(&mut self.cx.rng, to, 1, c, 20.0);
            }
        }
        self.mode.moved(&mut self.cx, from, to);
    }

    pub fn pointer_down(&mut self, x: f32, y: f32, button: u32) {
        let p = v2(x, y);
        self.cx.inp.mouse = p;
        self.cx.inp.has_mouse = true;
        self.cx.inp.idle = 0.0;
        if let Some(b) = self.cx.inp.buttons.get_mut(button as usize) {
            *b = true;
        }
        if self.busy() {
            return;
        }
        if self.mode_id != ModeId::Home {
            let (hp, hr) = self.home_button();
            if p.dist(hp) < hr * 1.15 {
                self.cx.au.sfx(Sfx::Pop, 1.0, hp.x);
                self.cx.go(ModeId::Home, hp, hsv(0.12, 0.3, 1.0));
                return;
            }
        }
        self.mode.down(&mut self.cx, p, button);
    }

    /// El mouse salió de la ventana o se levantó el dedo de la pantalla.
    pub fn pointer_leave(&mut self) {
        self.cx.inp.has_mouse = false;
    }

    pub fn pointer_up(&mut self, x: f32, y: f32, button: u32) {
        let p = v2(x, y);
        if let Some(b) = self.cx.inp.buttons.get_mut(button as usize) {
            *b = false;
        }
        if self.busy() {
            return;
        }
        self.mode.up(&mut self.cx, p, button);
    }

    pub fn wheel(&mut self, dy: f32) {
        self.cx.inp.idle = 0.0;
        if self.busy() {
            return;
        }
        if !self.mode.wheel(&mut self.cx, dy) {
            // Efecto por defecto: un remolino de colores con sonido que sube o baja.
            let cx = &mut self.cx;
            let up = dy < 0.0;
            let p = cx.inp.mouse;
            let c = hsv(cx.t * 0.3, 0.7, 1.0);
            cx.fx.ripple(p, 60.0, c.alpha(0.8));
            cx.fx.sparkle(&mut cx.rng, p, 4, c, 50.0);
            let step = if up { 1.15 } else { 0.85 };
            cx.au.sfx_vol(Sfx::Bloop, step, p.x, 0.5);
        }
    }

    pub fn key_down(&mut self, code: u32, ch: u32, repeat: bool) {
        let k = Key::new(code, ch, repeat);
        self.cx.inp.idle = 0.0;
        if !k.code.is_empty() {
            self.cx.inp.held.insert(k.code);
        }
        if self.busy() || (k.repeat && !self.mode.wants_repeat()) {
            return;
        }
        self.mode.key(&mut self.cx, &k);
    }

    pub fn key_up(&mut self, code: u32) {
        let k = Key::new(code, 0, false);
        self.cx.inp.held.remove(k.code);
        if !self.busy() {
            self.mode.key_up(&mut self.cx, &k);
        }
    }

    /// Suelta todas las teclas (por ejemplo, si la ventana pierde el foco).
    pub fn release_all(&mut self) {
        self.cx.inp.held.clear();
        self.cx.inp.buttons = [false; 3];
    }
}

/// Registra de antemano los glifos más usados para que el atlas se arme de
/// una vez al inicio (los demás se agregan solos cuando aparecen).
fn preload_glyphs(g: &mut Gfx) {
    for l in vocab::LETTERS {
        g.glyph_id(l.upper, true);
        for w in l.words {
            g.glyph_id(w.emoji, false);
        }
    }
    for d in 0..10 {
        g.glyph_id(&d.to_string(), true);
    }
    for w in vocab::ANIMALS.iter().chain(vocab::COUNTABLES) {
        g.glyph_id(w.emoji, false);
    }
    for e in [
        "🏠", "🚪", "⌨️", "🎨", "🎵", "🐝", "🌸", "🌼", "🌻", "🌷", "🍯", "🗑️", "🌈", "🎹", "🔔",
        "🎸", "🥁", "⭐",
    ] {
        g.glyph_id(e, false);
    }
    g.glyph_id("♪", true);
    g.glyph_id("♫", true);
}
