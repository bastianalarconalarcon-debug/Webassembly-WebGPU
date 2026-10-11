//! Los juegos. Cada uno implementa `Activity`.

use crate::game::{Cx, Key};
use crate::math::{approach, V2};

mod bee;
mod bubbles;
mod home;
mod keys;
mod music;
mod paint;
mod shapes;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeId {
    Home,
    Keys,
    Paint,
    Bubbles,
    Shapes,
    Music,
    Bee,
}

pub fn make(id: ModeId, cx: &mut Cx) -> Box<dyn Activity> {
    match id {
        ModeId::Home => Box::new(home::Home::new(cx)),
        ModeId::Keys => Box::new(keys::Keys::new(cx)),
        ModeId::Paint => Box::new(paint::Paint::new(cx)),
        ModeId::Bubbles => Box::new(bubbles::Bubbles::new(cx)),
        ModeId::Shapes => Box::new(shapes::Shapes::new(cx)),
        ModeId::Music => Box::new(music::Music::new(cx)),
        ModeId::Bee => Box::new(bee::Bee::new(cx)),
    }
}

pub trait Activity {
    /// Al entrar al juego (después de la transición).
    fn enter(&mut self, _cx: &mut Cx) {}
    /// Lógica + dibujo de un cuadro.
    fn update(&mut self, cx: &mut Cx);
    fn resize(&mut self, _cx: &mut Cx) {}
    fn key(&mut self, _cx: &mut Cx, _k: &Key) {}
    fn key_up(&mut self, _cx: &mut Cx, _k: &Key) {}
    fn down(&mut self, _cx: &mut Cx, _p: V2, _button: u32) {}
    fn up(&mut self, _cx: &mut Cx, _p: V2, _button: u32) {}
    fn moved(&mut self, _cx: &mut Cx, _from: V2, _to: V2) {}
    /// Devuelve true si el juego usó la rueda del mouse.
    fn wheel(&mut self, _cx: &mut Cx, _dy: f32) -> bool {
        false
    }
    /// ¿Dejar estrellitas detrás del cursor?
    fn cursor_trail(&self) -> bool {
        true
    }
    /// ¿Recibir teclas repetidas al mantenerlas presionadas?
    fn wants_repeat(&self) -> bool {
        false
    }
}

/// Animación de "pasar el mouse por encima": crece suave y avisa al entrar.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hover {
    pub anim: f32,
    pub over: bool,
}

impl Hover {
    /// Actualiza y devuelve true justo cuando el mouse entra.
    pub fn update(&mut self, cx: &Cx, over: bool) -> bool {
        let entered = over && !self.over;
        self.over = over;
        self.anim = approach(self.anim, if over { 1.0 } else { 0.0 }, 12.0, cx.dt);
        entered
    }
}

/// Recordatorio hablado cuando la niña no toca nada por un rato.
pub struct IdlePrompt {
    after: f32,
    next: f32,
    count: u32,
}

impl IdlePrompt {
    pub fn new(after: f32) -> IdlePrompt {
        IdlePrompt { after, next: after, count: 0 }
    }

    /// Devuelve true en el momento de dar la pista.
    pub fn tick(&mut self, cx: &mut Cx, text: &str) -> bool {
        if cx.inp.idle < 0.5 {
            self.next = self.after;
            self.count = 0;
            return false;
        }
        if cx.inp.idle >= self.next && self.count < 3 {
            self.next += self.after * 1.5;
            self.count += 1;
            return cx.au.say_if_quiet(text);
        }
        false
    }

    /// ¿Está esperando hace rato? (para animar una pista visual)
    pub fn waiting(&self, cx: &Cx) -> bool {
        cx.inp.idle >= self.after
    }
}
