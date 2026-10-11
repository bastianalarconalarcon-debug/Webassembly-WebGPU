//! Juego educativo para niñas y niños pequeños.
//!
//! Toda la lógica vive aquí, en Rust compilado a WebAssembly. JavaScript
//! (`web/`) sólo hace de puente con el navegador: entrega el mouse y el
//! teclado, dibuja con WebGPU lo que Rust pide y reproduce sonidos y voz.
//!
//! La interfaz con JS es una lista de funciones `extern "C"` sin
//! dependencias (no usa wasm-bindgen): JS lee la memoria del módulo
//! directamente usando los punteros que devuelven estas funciones.

pub mod audio;
pub mod fx;
pub mod game;
pub mod gfx;
pub mod math;
pub mod modes;
pub mod rng;
pub mod vocab;

use std::cell::RefCell;

use game::Game;

thread_local! {
    static GAME: RefCell<Option<Game>> = const { RefCell::new(None) };
    /// Buffer donde JS escribe texto (el nombre de la niña).
    static TEXT_IN: RefCell<Vec<u8>> = RefCell::new(vec![0; 256]);
}

fn with<R: Default>(f: impl FnOnce(&mut Game) -> R) -> R {
    GAME.with(|g| g.borrow_mut().as_mut().map(f).unwrap_or_default())
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn host_log(ptr: *const u8, len: usize);
}

/// Escribe en la consola del navegador (útil para depurar).
pub fn log(s: &str) {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        host_log(s.as_ptr(), s.len());
    }
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{s}");
}

#[no_mangle]
pub extern "C" fn init(w: f32, h: f32, seed: u32) {
    std::panic::set_hook(Box::new(|info| log(&format!("panic: {info}"))));
    GAME.with(|g| *g.borrow_mut() = Some(Game::new(w, h, seed)));
}

#[no_mangle]
pub extern "C" fn resize(w: f32, h: f32) {
    with(|g| g.resize(w, h))
}

#[no_mangle]
pub extern "C" fn update(dt: f32, speaking: u32) {
    with(|g| g.update(dt, speaking != 0))
}

#[no_mangle]
pub extern "C" fn pointer_move(x: f32, y: f32) {
    with(|g| g.pointer_move(x, y))
}

#[no_mangle]
pub extern "C" fn pointer_down(x: f32, y: f32, button: u32) {
    with(|g| g.pointer_down(x, y, button))
}

#[no_mangle]
pub extern "C" fn pointer_up(x: f32, y: f32, button: u32) {
    with(|g| g.pointer_up(x, y, button))
}

#[no_mangle]
pub extern "C" fn pointer_leave() {
    with(|g| g.pointer_leave())
}

#[no_mangle]
pub extern "C" fn wheel(dy: f32) {
    with(|g| g.wheel(dy))
}

#[no_mangle]
pub extern "C" fn key_down(code: u32, ch: u32, repeat: u32) {
    with(|g| g.key_down(code, ch, repeat != 0))
}

#[no_mangle]
pub extern "C" fn key_up(code: u32) {
    with(|g| g.key_up(code))
}

/// Índice del juego activo: 0 inicio, 1 teclado, 2 pintar, 3 burbujas,
/// 4 formas, 5 música, 6 abeja.
#[no_mangle]
pub extern "C" fn current_mode() -> u32 {
    with(|g| g.mode_index())
}

#[no_mangle]
pub extern "C" fn release_all() {
    with(|g| g.release_all())
}

// --- Texto de entrada ---------------------------------------------------

#[no_mangle]
pub extern "C" fn text_in_ptr() -> *mut u8 {
    TEXT_IN.with(|t| t.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn text_in_cap() -> u32 {
    TEXT_IN.with(|t| t.borrow().len() as u32)
}

/// JS escribió `len` bytes UTF-8 en `text_in_ptr()`: es el nombre.
#[no_mangle]
pub extern "C" fn set_name(len: u32) {
    let name = TEXT_IN.with(|t| {
        let t = t.borrow();
        let len = (len as usize).min(t.len());
        String::from_utf8_lossy(&t[..len]).into_owned()
    });
    with(|g| g.set_name(&name))
}

// --- Dibujo -------------------------------------------------------------

#[no_mangle]
pub extern "C" fn scene_ptr() -> *const f32 {
    with(|g| g.cx.g.scene.as_ptr() as usize) as *const f32
}

#[no_mangle]
pub extern "C" fn scene_len() -> u32 {
    with(|g| g.cx.g.scene.len() as u32)
}

#[no_mangle]
pub extern "C" fn paint_ptr() -> *const f32 {
    with(|g| g.cx.g.paint.as_ptr() as usize) as *const f32
}

#[no_mangle]
pub extern "C" fn paint_len() -> u32 {
    with(|g| g.cx.g.paint.len() as u32)
}

/// 1 si hay que borrar la capa de pintura antes de pintar.
#[no_mangle]
pub extern "C" fn paint_clear() -> u32 {
    with(|g| g.cx.g.clear_paint as u32)
}

/// JS ya dibujó la capa de pintura de este cuadro.
#[no_mangle]
pub extern "C" fn paint_done() {
    with(|g| {
        g.cx.g.paint.clear();
        g.cx.g.clear_paint = false;
    })
}

#[no_mangle]
pub extern "C" fn bg_ptr() -> *const f32 {
    with(|g| g.cx.g.bg.as_ptr() as usize) as *const f32
}

#[no_mangle]
pub extern "C" fn glyph_count() -> u32 {
    with(|g| g.cx.g.glyphs.len() as u32)
}

#[no_mangle]
pub extern "C" fn glyph_ptr(i: u32) -> *const u8 {
    with(|g| g.cx.g.glyphs.get(i as usize).map(|s| s.0.as_ptr() as usize).unwrap_or(0)) as *const u8
}

#[no_mangle]
pub extern "C" fn glyph_len(i: u32) -> u32 {
    with(|g| g.cx.g.glyphs.get(i as usize).map(|s| s.0.len() as u32).unwrap_or(0))
}

#[no_mangle]
pub extern "C" fn glyph_is_text(i: u32) -> u32 {
    with(|g| g.cx.g.glyphs.get(i as usize).map(|s| s.1 as u32).unwrap_or(0))
}

/// Cuántos glifos caben en el atlas que creó JS.
#[no_mangle]
pub extern "C" fn set_glyph_capacity(n: u32) {
    with(|g| g.cx.g.capacity = n as usize)
}

// --- Sonido -------------------------------------------------------------

#[no_mangle]
pub extern "C" fn events_ptr() -> *const f32 {
    with(|g| g.cx.au.events.as_ptr() as usize) as *const f32
}

#[no_mangle]
pub extern "C" fn events_len() -> u32 {
    with(|g| g.cx.au.events.len() as u32)
}

#[no_mangle]
pub extern "C" fn speech_ptr() -> *const u8 {
    with(|g| g.cx.au.text.as_ptr() as usize) as *const u8
}

#[no_mangle]
pub extern "C" fn events_done() {
    with(|g| g.cx.au.clear())
}

// --- Tabla de teclas ----------------------------------------------------

#[no_mangle]
pub extern "C" fn key_code_count() -> u32 {
    game::KEY_CODES.len() as u32
}

#[no_mangle]
pub extern "C" fn key_code_ptr(i: u32) -> *const u8 {
    game::KEY_CODES.get(i as usize).map(|s| s.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn key_code_len(i: u32) -> u32 {
    game::KEY_CODES.get(i as usize).map(|s| s.len() as u32).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::ModeId;

    /// Simula un rato de juego en cada modo con entradas variadas: no debe
    /// entrar en pánico y siempre debe dibujar algo.
    #[test]
    fn every_mode_survives_a_toddler() {
        let modes = [ModeId::Home, ModeId::Keys, ModeId::Paint, ModeId::Bubbles, ModeId::Shapes, ModeId::Music, ModeId::Bee];
        for (mi, mode) in modes.into_iter().enumerate() {
            let mut g = Game::new(1280.0, 720.0, 42 + mi as u32);
            g.set_name("Sofía");
            g.update(0.016, false);
            if mode != ModeId::Home {
                g.cx.go(mode, g.cx.center(), crate::math::WHITE);
                for _ in 0..120 {
                    g.update(0.016, false);
                }
                assert_eq!(g.mode_id, mode);
            }
            let mut rng = crate::rng::Rng::new(7);
            for step in 0..3000 {
                let x = rng.range(0.0, 1280.0);
                let y = rng.range(0.0, 720.0);
                match rng.int(10) {
                    0 => g.pointer_down(x, y, rng.int(3) as u32),
                    1 => g.pointer_up(x, y, 0),
                    2 => g.wheel(rng.range(-100.0, 100.0)),
                    3 | 4 => {
                        let code = rng.int(game::KEY_CODES.len()) as u32;
                        let ch = *rng.pick(&['a', 'ñ', 'é', '7', ' ', '?', '#', '\0', 'Z']) as u32;
                        g.key_down(code, ch, rng.chance(0.1));
                        if rng.chance(0.7) {
                            g.key_up(code);
                        }
                    }
                    _ => g.pointer_move(x, y),
                }
                g.update(rng.range(0.0, 0.05), rng.chance(0.5));
                // Evitar volver a casa por clics al azar en el botón de casa.
                if g.mode_id != mode && step % 50 == 0 {
                    g.cx.go(mode, g.cx.center(), crate::math::WHITE);
                }
                assert!(!g.cx.g.scene.is_empty(), "{mode:?} no dibujó nada");
                g.cx.au.clear();
                g.cx.g.paint.clear();
            }
        }
    }

    #[test]
    fn all_key_codes_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for c in game::KEY_CODES {
            assert!(seen.insert(c), "repetido: {c}");
        }
    }
}
