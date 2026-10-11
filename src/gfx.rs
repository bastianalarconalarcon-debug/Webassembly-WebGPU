//! Lista de dibujo. Rust no habla con la GPU directamente: llena un arreglo
//! de "instancias" (una figura por instancia) que JavaScript copia a un
//! buffer de WebGPU cada cuadro. El shader (`web/shaders.wgsl`) dibuja cada
//! instancia como un cuadrado con una figura calculada por distancia (SDF).

use std::collections::HashMap;

use crate::math::{v2, Rgba, V2};

/// Una figura en pantalla. Debe coincidir byte a byte con el vertex buffer
/// definido en `web/renderer.js` (12 floats = 48 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Instance {
    pub pos: [f32; 2],
    /// Medio ancho / medio alto en píxeles. `half[0] < 0` refleja el glifo.
    pub half: [f32; 2],
    pub rot: f32,
    pub kind: f32,
    /// Índice de glifo (para Emoji/Text) o difuminado en píxeles (resto).
    pub glyph: f32,
    pub param: f32,
    pub color: [f32; 4],
}

pub const INSTANCE_FLOATS: usize = 12;
const MAX_INSTANCES: usize = 40_000;

/// Tipos de figura que entiende el shader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Circle = 0,
    /// `param` = radio de las esquinas (0..1 del lado menor).
    RoundRect = 1,
    Star = 2,
    Heart = 3,
    Triangle = 4,
    /// `param` = grosor en píxeles.
    Ring = 5,
    Emoji = 6,
    Glow = 7,
    Bubble = 8,
    Diamond = 9,
    /// Letra con borde nítido a cualquier tamaño; `param` = grosor del borde.
    Text = 10,
    Flower = 11,
    /// Flecha apuntando a la derecha (rotar para otras direcciones).
    Arrow = 12,
    /// Ícono de bloque en perspectiva isométrica (paleta del juego Construir).
    Cube = 13,
}

/// Estilos de fondo (ver `fs_background` en el shader).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backdrop {
    Bokeh = 0,
    Underwater = 1,
    Paper = 2,
    Night = 3,
    Meadow = 4,
}

pub struct Gfx {
    pub scene: Vec<Instance>,
    /// Figuras que se pintan sobre la capa persistente (modo Pintar).
    pub paint: Vec<Instance>,
    pub clear_paint: bool,
    /// [arriba rgba, abajo rgba, estilo, parámetro, mostrar pintura, 0]
    pub bg: [f32; 12],
    to_paint: bool,
    index: HashMap<String, u16>,
    /// Glifos registrados: (texto, es_letra). JS los dibuja en el atlas.
    pub glyphs: Vec<(String, bool)>,
    pub capacity: usize,
    /// Escena 3D (juego Construir). Se vacía en cada cuadro.
    pub d3: crate::gfx3d::Gfx3,
}

impl Default for Gfx {
    fn default() -> Self {
        Self::new()
    }
}

impl Gfx {
    pub fn new() -> Gfx {
        Gfx {
            scene: Vec::with_capacity(4096),
            paint: Vec::with_capacity(1024),
            clear_paint: true,
            bg: [0.0; 12],
            to_paint: false,
            index: HashMap::new(),
            glyphs: Vec::new(),
            capacity: 210,
            d3: crate::gfx3d::Gfx3::new(),
        }
    }

    pub fn begin_frame(&mut self) {
        self.scene.clear();
        self.d3.begin_frame();
        self.to_paint = false;
        self.bg[10] = 0.0;
    }

    /// Mostrar la capa de pintura persistente en este cuadro.
    pub fn show_paint(&mut self) {
        self.bg[10] = 1.0;
    }

    /// A partir de ahora todo se dibuja en la capa de pintura persistente.
    pub fn paint_layer(&mut self, on: bool) {
        self.to_paint = on;
    }

    pub fn background(&mut self, top: Rgba, bottom: Rgba, style: Backdrop, param: f32) {
        self.bg[0..4].copy_from_slice(&top.arr());
        self.bg[4..8].copy_from_slice(&bottom.arr());
        self.bg[8] = style as u32 as f32;
        self.bg[9] = param;
    }

    /// Devuelve el índice del glifo en el atlas, registrándolo si es nuevo.
    pub fn glyph_id(&mut self, s: &str, is_text: bool) -> Option<u16> {
        let key_prefix = if is_text { 'T' } else { 'E' };
        let mut key = String::with_capacity(s.len() + 1);
        key.push(key_prefix);
        key.push_str(s);
        if let Some(&i) = self.index.get(&key) {
            return Some(i);
        }
        if self.glyphs.len() >= self.capacity {
            return None;
        }
        let i = self.glyphs.len() as u16;
        self.glyphs.push((s.to_string(), is_text));
        self.index.insert(key, i);
        Some(i)
    }

    fn push(&mut self, inst: Instance) {
        let list = if self.to_paint {
            &mut self.paint
        } else {
            &mut self.scene
        };
        if list.len() < MAX_INSTANCES {
            list.push(inst);
        }
    }

    /// Figura con todas las opciones.
    #[allow(clippy::too_many_arguments)]
    pub fn shape_full(
        &mut self,
        kind: Shape,
        p: V2,
        half: V2,
        rot: f32,
        c: Rgba,
        param: f32,
        soft: f32,
    ) {
        if c.a <= 0.002 || half.x == 0.0 || half.y <= 0.0 {
            return;
        }
        self.push(Instance {
            pos: [p.x, p.y],
            half: [half.x, half.y],
            rot,
            kind: kind as u32 as f32,
            glyph: soft,
            param,
            color: c.arr(),
        });
    }

    /// Figura cuadrada de "radio" `r` (medio lado).
    pub fn shape(&mut self, kind: Shape, p: V2, r: f32, rot: f32, c: Rgba) {
        self.shape_full(kind, p, v2(r, r), rot, c, default_param(kind, r), 0.0);
    }

    pub fn circle(&mut self, p: V2, r: f32, c: Rgba) {
        self.shape(Shape::Circle, p, r, 0.0, c);
    }

    pub fn glow(&mut self, p: V2, r: f32, c: Rgba) {
        self.shape(Shape::Glow, p, r, 0.0, c);
    }

    pub fn ring(&mut self, p: V2, r: f32, thickness: f32, c: Rgba) {
        self.shape_full(Shape::Ring, p, v2(r, r), 0.0, c, thickness, 0.0);
    }

    pub fn rrect(&mut self, p: V2, half: V2, corner: f32, c: Rgba) {
        self.shape_full(Shape::RoundRect, p, half, 0.0, c, corner, 0.0);
    }

    /// Sombra suave bajo una figura.
    pub fn shadow(&mut self, kind: Shape, p: V2, r: f32, rot: f32, strength: f32) {
        let soft = r * 0.25 + 4.0;
        self.shape_full(
            kind,
            p + v2(0.0, r * 0.12 + 3.0),
            v2(r, r),
            rot,
            Rgba {
                r: 0.05,
                g: 0.05,
                b: 0.2,
                a: 0.22 * strength,
            },
            default_param(kind, r),
            soft,
        );
    }

    /// Figura con borde oscuro estilo "sticker".
    pub fn sticker(&mut self, kind: Shape, p: V2, r: f32, rot: f32, c: Rgba) {
        let border = (r * 0.09).max(2.5);
        self.shape(kind, p, r + border, rot, c.darker(0.35));
        self.shape(kind, p, r, rot, c);
        // Brillo superior.
        if kind == Shape::Circle || kind == Shape::RoundRect {
            self.shape_full(
                kind,
                p - v2(r * 0.25, r * 0.3),
                v2(r * 0.35, r * 0.2),
                -0.5,
                c.lighter(0.6).alpha(0.45),
                0.5,
                r * 0.15,
            );
        }
    }

    /// Emoji (dibujo a todo color). `r` = medio alto.
    pub fn emoji(&mut self, s: &str, p: V2, r: f32, rot: f32, alpha: f32) {
        self.emoji_flip(s, p, r, rot, alpha, false);
    }

    pub fn emoji_flip(&mut self, s: &str, p: V2, r: f32, rot: f32, alpha: f32, flip: bool) {
        self.emoji_color(
            s,
            p,
            r,
            rot,
            Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: alpha,
            },
            flip,
        );
    }

    /// Emoji multiplicado por un color (negro = silueta, útil para sombras).
    pub fn emoji_color(&mut self, s: &str, p: V2, r: f32, rot: f32, c: Rgba, flip: bool) {
        if c.a <= 0.002 || r <= 0.0 {
            return;
        }
        let Some(id) = self.glyph_id(s, false) else {
            return;
        };
        let hx = if flip { -r } else { r };
        self.push(Instance {
            pos: [p.x, p.y],
            half: [hx, r],
            rot,
            kind: Shape::Emoji as u32 as f32,
            glyph: id as f32,
            param: 0.0,
            color: c.arr(),
        });
    }

    /// Sombra suave de un emoji.
    pub fn emoji_shadow(&mut self, s: &str, p: V2, r: f32, rot: f32, alpha: f32) {
        let c = Rgba {
            r: 0.05,
            g: 0.05,
            b: 0.2,
            a: 0.2 * alpha,
        };
        self.emoji_color(s, p + v2(0.0, r * 0.1 + 2.0), r, rot, c, false);
    }

    /// Letra o número teñido de color con borde. `r` = medio alto.
    pub fn text(&mut self, s: &str, p: V2, r: f32, rot: f32, c: Rgba, outline: f32) {
        if c.a <= 0.002 || r <= 0.0 {
            return;
        }
        let Some(id) = self.glyph_id(s, true) else {
            return;
        };
        self.push(Instance {
            pos: [p.x, p.y],
            half: [r, r],
            rot,
            kind: Shape::Text as u32 as f32,
            glyph: id as f32,
            param: outline,
            color: c.arr(),
        });
    }

    /// Rectángulo que cubre toda la pantalla.
    pub fn fullscreen(&mut self, w: f32, h: f32, c: Rgba) {
        self.shape_full(
            Shape::RoundRect,
            v2(w * 0.5, h * 0.5),
            v2(w * 0.5 + 4.0, h * 0.5 + 4.0),
            0.0,
            c,
            0.0,
            0.0,
        );
    }
}

fn default_param(kind: Shape, r: f32) -> f32 {
    match kind {
        Shape::RoundRect => 0.3,
        Shape::Ring => (r * 0.15).max(2.0),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::WHITE;

    #[test]
    fn instance_layout_matches_js() {
        assert_eq!(std::mem::size_of::<Instance>(), INSTANCE_FLOATS * 4);
    }

    #[test]
    fn glyphs_register_once_per_kind() {
        let mut g = Gfx::new();
        g.emoji("🐶", V2::ZERO, 10.0, 0.0, 1.0);
        g.emoji("🐶", V2::ZERO, 10.0, 0.0, 1.0);
        g.text("A", V2::ZERO, 10.0, 0.0, WHITE, 0.0);
        g.text("🐶", V2::ZERO, 10.0, 0.0, WHITE, 0.0);
        assert_eq!(g.glyphs.len(), 3);
        assert_eq!(g.scene.len(), 4);
    }

    #[test]
    fn paint_layer_routes_instances() {
        let mut g = Gfx::new();
        g.paint_layer(true);
        g.circle(V2::ZERO, 5.0, WHITE);
        g.paint_layer(false);
        g.circle(V2::ZERO, 5.0, WHITE);
        assert_eq!(g.paint.len(), 1);
        assert_eq!(g.scene.len(), 1);
    }
}
