//! Construir: un mundo de bloques tipo Minecraft sobre una pradera.
//!
//! - Clic en el pasto o en un bloque = poner el bloque elegido ahí (en la cara
//!   apuntada). Arrastrar con el botón apretado pone una fila.
//! - La goma 🧽 (o el clic derecho) saca bloques. ↩️ deshace (también Borrar).
//! - Interruptor arriba a la derecha: 🎈 Flotar (sin física) / 🍎 Caer (con
//!   física, "regla del andamio": algo puede sobresalir hasta 3 bloques; lo
//!   que queda sin apoyo tiembla un instante y cae).
//! - Flechas grandes a los lados (o del teclado, o arrastrar el cielo) giran
//!   el mundo; la rueda acerca y aleja.
//! - El mundo vive en `Cx`, así que la construcción no se pierde al ir a la
//!   casa y volver.

use std::collections::HashMap;

use super::{Activity, Hover, IdlePrompt};
use crate::audio::{Inst, Sfx};
use crate::game::{Cx, Key};
use crate::gfx::{Backdrop, Shape};
use crate::gfx3d::{ambient_occlusion, face_mask, mat, v3, Camera, Cube, AO_NONE, HL_ALL, V3};
use crate::math::{approach, ease_out_back, hex, hsv, midi, pentatonic, v2, Rgba, PI, V2, WHITE};
use crate::vocab;
use crate::voxel::{
    self, place_target, raycast, Event, PlaceResult, Target, AIR, CAM_DIST, PITCH_MAX, PITCH_MIN,
    SX, SY, SZ,
};

// ---------------------------------------------------------------------------
// Bloques
// ---------------------------------------------------------------------------

struct BlockDef {
    id: u8,
    /// Lo que dice la voz al elegirlo.
    name: &'static str,
    mat: u8,
    /// Tinte en el mundo (blanco = textura tal cual).
    tint: u32,
    /// Color del ícono en la paleta.
    icon: u32,
}

const fn b(id: u8, name: &'static str, mat: u8, tint: u32, icon: u32) -> BlockDef {
    BlockDef {
        id,
        name,
        mat,
        tint,
        icon,
    }
}

static BLOCKS: &[BlockDef] = &[
    b(1, "¡Rojo!", mat::JUGUETE, 0xE53935, 0xE53935),
    b(2, "¡Naranjo!", mat::JUGUETE, 0xFB8C00, 0xFB8C00),
    b(3, "¡Amarillo!", mat::JUGUETE, 0xFDD835, 0xFDD835),
    b(4, "¡Verde!", mat::JUGUETE, 0x43A047, 0x43A047),
    b(5, "¡Celeste!", mat::JUGUETE, 0x4FC3F7, 0x4FC3F7),
    b(6, "¡Azul!", mat::JUGUETE, 0x1E5BD8, 0x1E5BD8),
    b(7, "¡Morado!", mat::JUGUETE, 0x8E24AA, 0x8E24AA),
    b(8, "¡Rosado!", mat::JUGUETE, 0xF06292, 0xF06292),
    b(11, "¡Pasto!", mat::PASTO, 0xFFFFFF, 0x66BB33),
    b(12, "¡Tierra!", mat::TIERRA, 0xFFFFFF, 0x8D6E4A),
    b(13, "¡Madera!", mat::MADERA, 0xFFFFFF, 0xC8955A),
    b(14, "¡Piedra!", mat::PIEDRA, 0xFFFFFF, 0x9095A0),
    b(15, "¡Ladrillo!", mat::LADRILLO, 0xFFFFFF, 0xB84A35),
    b(16, "¡Hojas!", mat::HOJAS, 0xFFFFFF, 0x3E8E34),
    b(17, "¡Vidrio!", mat::VIDRIO, 0xFFFFFF, 0xB3E5FC),
    b(18, "¡Oro!", mat::ORO, 0xFFFFFF, 0xFFC928),
    b(19, "¡Tronco!", mat::TRONCO, 0xFFFFFF, 0x6D4C33),
    b(20, "¡Arena!", mat::ARENA, 0xFFFFFF, 0xF2DFA0),
    b(21, "¡Nube!", mat::JUGUETE, 0xF4F8FF, 0xF4F8FF),
    b(22, "¡Agua!", mat::AGUA, 0xFFFFFF, 0x3A7BE0),
];

const ID_TRONCO: u8 = 19;
const ID_HOJAS: u8 = 16;
const ID_LADRILLO: u8 = 15;
const ID_MADERA: u8 = 13;
const ID_VIDRIO: u8 = 17;
const ID_ROJO: u8 = 1;
const ID_AMARILLO: u8 = 3;
const ID_AZUL: u8 = 6;

fn def(id: u8) -> &'static BlockDef {
    BLOCKS.iter().find(|d| d.id == id).unwrap_or(&BLOCKS[0])
}

fn rgba(h: u32) -> Rgba {
    hex(h)
}

/// Piezas que se ponen enteras de una vez.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Piece {
    Tree,
    House,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tool {
    Block(u8),
    Piece(Piece),
    Eraser,
}

/// Cajones de la paleta (máximo 8 cosas cada uno: más abruma a los 3 años).
const DRAWERS: [(&str, &str, &[Tool]); 3] = [
    (
        "🎨",
        "¡Colores!",
        &[
            Tool::Block(1),
            Tool::Block(2),
            Tool::Block(3),
            Tool::Block(4),
            Tool::Block(5),
            Tool::Block(6),
            Tool::Block(7),
            Tool::Block(8),
        ],
    ),
    (
        "🌳",
        "¡Naturaleza!",
        &[
            Tool::Block(11),
            Tool::Block(12),
            Tool::Block(14),
            Tool::Block(20),
            Tool::Block(13),
            Tool::Block(19),
            Tool::Block(16),
            Tool::Block(22),
        ],
    ),
    (
        "✨",
        "¡Sorpresas!",
        &[
            Tool::Block(17),
            Tool::Block(15),
            Tool::Block(18),
            Tool::Block(21),
            Tool::Piece(Piece::Tree),
            Tool::Piece(Piece::House),
        ],
    ),
];

// ---------------------------------------------------------------------------
// Interfaz 2D
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Ui {
    Drawer(usize),
    Tool(Tool),
    Undo,
    Clear,
    Physics(bool),
    Turn(f32),
}

struct Button {
    ui: Ui,
    pos: V2,
    r: f32,
    hover: Hover,
}

// ---------------------------------------------------------------------------
// Animaciones
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum AnimKind {
    /// Aparece creciendo.
    Grow,
    /// Se aplasta al aterrizar.
    Land,
}

struct Anim {
    cell: [i32; 3],
    kind: AnimKind,
    t: f32,
}

/// Un bloque que se va encogiendo después de sacarlo.
struct Pop {
    center: V3,
    id: u8,
    t: f32,
}

struct FloatingNumber {
    at: V3,
    text: String,
    t: f32,
}

struct Drag {
    last: V2,
    moved: f32,
    button: u32,
    /// Girando la cámara (no poniendo bloques).
    orbit: bool,
    /// Capa (y) de la fila que se está poniendo.
    layer: Option<i32>,
    placed: u32,
    cooldown: f32,
}

/// Torre que se arma sola con un número del teclado.
struct Counting {
    x: i32,
    z: i32,
    id: u8,
    n: u32,
    done: u32,
    timer: f32,
}

const MAX_STROKE: u32 = 30;
const STROKE_RATE: f32 = 0.1;
const TURN_STEP: f32 = PI / 4.0;
const FOVS: [f32; 4] = [0.55, 0.7, 0.85, 1.05];

pub struct Build {
    cam: Camera,
    yaw_goal: f32,
    pitch_goal: f32,
    fov_i: usize,
    tool: Tool,
    drawer: usize,
    buttons: Vec<Button>,
    bar_top: f32,
    hover: Option<(voxel::Hit, Target)>,
    drag: Option<Drag>,
    anims: Vec<Anim>,
    pops: Vec<Pop>,
    numbers: Vec<FloatingNumber>,
    counting: Option<Counting>,
    /// Lista de cubos quietos (se rehace sólo cuando cambia el mundo).
    cache: Vec<([i32; 3], Cube)>,
    cache_index: HashMap<[i32; 3], usize>,
    cache_version: u64,
    floor: Vec<Cube>,
    floor_version: u64,
    scenery: Vec<Cube>,
    idle: IdlePrompt,
    land_sound_cd: f32,
    landed_recent: Vec<f32>,
    crash_cd: f32,
    full_cd: f32,
    sky_cd: f32,
    wobble_cd: f32,
    auto_turn: f32,
}

/// Altura de la cámara sobre el mundo según lo más alto construido.
fn tallest(w: &voxel::World) -> i32 {
    for y in (0..SY).rev() {
        for z in 0..SZ {
            for x in 0..SX {
                if w.get([x, y, z]) != AIR {
                    return y + 1;
                }
            }
        }
    }
    0
}

fn center_of(p: [i32; 3]) -> V3 {
    v3(p[0] as f32 + 0.5, p[1] as f32 + 0.5, p[2] as f32 + 0.5)
}

impl Build {
    pub fn new(cx: &mut Cx) -> Build {
        if !cx.build_seeded {
            cx.build_seeded = true;
            seed_world(&mut cx.world);
        }
        let mut b = Build {
            cam: Camera {
                target: v3(SX as f32 * 0.5, 1.0, SZ as f32 * 0.5),
                yaw: 0.65,
                pitch: 0.58,
                dist: CAM_DIST,
                fovy: FOVS[2],
            },
            yaw_goal: 0.65,
            pitch_goal: 0.58,
            fov_i: 2,
            tool: Tool::Block(1),
            drawer: 0,
            buttons: Vec::new(),
            bar_top: 0.0,
            hover: None,
            drag: None,
            anims: Vec::new(),
            pops: Vec::new(),
            numbers: Vec::new(),
            counting: None,
            cache: Vec::new(),
            cache_index: HashMap::new(),
            cache_version: u64::MAX,
            floor: Vec::new(),
            floor_version: u64::MAX,
            scenery: scenery(),
            idle: IdlePrompt::new(14.0),
            land_sound_cd: 0.0,
            landed_recent: Vec::new(),
            crash_cd: 0.0,
            full_cd: 0.0,
            sky_cd: 0.0,
            wobble_cd: 0.0,
            auto_turn: 0.0,
        };
        b.layout(cx);
        b
    }

    // --- Interfaz -----------------------------------------------------------

    fn layout(&mut self, cx: &Cx) {
        let tools = DRAWERS[self.drawer].2;
        // Barra: ↩️ | herramientas del cajón | 🧽 🧹
        let n = tools.len() as f32 + 3.0;
        let r = (cx.w / (n * 2.7)).min(cx.h * 0.055).max(18.0);
        let gap = r * 2.5;
        let y = cx.h - r * 1.45;
        self.bar_top = y - r * 1.45;
        let x0 = cx.w * 0.5 - (n - 1.0) * gap * 0.5;
        let mut buttons = Vec::new();
        let mut push = |ui: Ui, pos: V2, r: f32| {
            buttons.push(Button {
                ui,
                pos,
                r,
                hover: Hover::default(),
            })
        };
        push(Ui::Undo, v2(x0, y), r);
        for (i, t) in tools.iter().enumerate() {
            push(Ui::Tool(*t), v2(x0 + (i as f32 + 1.0) * gap, y), r);
        }
        push(Ui::Tool(Tool::Eraser), v2(x0 + (n - 2.0) * gap, y), r);
        push(Ui::Clear, v2(x0 + (n - 1.0) * gap, y), r);
        // Pestañas de los cajones, sobre la barra a la izquierda.
        let tr = r * 0.78;
        for i in 0..DRAWERS.len() {
            push(
                Ui::Drawer(i),
                v2(x0 + i as f32 * tr * 2.4, self.bar_top - tr * 1.1),
                tr,
            );
        }
        // Interruptor de física arriba a la derecha.
        let pr = (cx.unit() * 0.05).clamp(24.0, 44.0);
        push(Ui::Physics(false), v2(cx.w - pr * 3.6, pr + 18.0), pr);
        push(Ui::Physics(true), v2(cx.w - pr * 1.3, pr + 18.0), pr);
        // Flechas para girar el mundo.
        let ar = (cx.unit() * 0.055).clamp(24.0, 46.0);
        push(Ui::Turn(-1.0), v2(ar + 14.0, cx.h * 0.45), ar);
        push(Ui::Turn(1.0), v2(cx.w - ar - 14.0, cx.h * 0.45), ar);
        self.buttons = buttons;
    }

    fn button_at(&self, p: V2) -> Option<usize> {
        self.buttons.iter().position(|b| p.dist(b.pos) < b.r * 1.15)
    }

    fn over_ui(&self, p: V2) -> bool {
        p.y > self.bar_top || self.button_at(p).is_some()
    }

    fn press_ui(&mut self, cx: &mut Cx, i: usize) {
        let (ui, pos) = (self.buttons[i].ui, self.buttons[i].pos);
        let color = hsv(cx.rng.f(), 0.6, 1.0);
        cx.fx.sparkle(&mut cx.rng, pos, 10, color, 40.0);
        match ui {
            Ui::Drawer(d) => {
                self.drawer = d;
                cx.au.say(DRAWERS[d].1);
                cx.au.sfx(Sfx::Whoosh, 1.4, pos.x);
                // Elegir lo primero del cajón para que se note el cambio.
                self.tool = DRAWERS[d].2[0];
                self.layout(cx);
            }
            Ui::Tool(t) => {
                self.tool = t;
                match t {
                    Tool::Block(id) => {
                        cx.au.say(def(id).name);
                        cx.au.note(
                            pentatonic(id as i32 % 10 + 4, 60.0),
                            Inst::Marimba,
                            pos.x,
                            0.5,
                        );
                    }
                    Tool::Piece(Piece::Tree) => cx.au.say("¡Un árbol!"),
                    Tool::Piece(Piece::House) => cx.au.say("¡Una casita!"),
                    Tool::Eraser => {
                        cx.au.say("¡Goma! Haz clic en un bloque para sacarlo.");
                        cx.au.sfx(Sfx::Whoosh, 1.5, pos.x);
                    }
                }
            }
            Ui::Undo => self.undo(cx),
            Ui::Clear => {
                let n = cx.world.clear();
                if n > 0 {
                    cx.au.sfx(Sfx::Splash, 1.0, pos.x);
                    cx.au.say("¡Todo limpio!");
                    self.counting = None;
                    for _ in 0..6 {
                        let p = cx.rng.point(0.0, 0.0, cx.w, self.bar_top);
                        let c = hsv(cx.rng.f(), 0.4, 1.0).alpha(0.6);
                        cx.fx.ripple(p, 160.0, c);
                    }
                } else {
                    cx.au.sfx_vol(Sfx::Bloop, 1.0, pos.x, 0.5);
                }
            }
            Ui::Physics(on) => self.set_physics(cx, on, pos),
            Ui::Turn(dir) => {
                self.yaw_goal += dir * TURN_STEP;
                cx.au.sfx(Sfx::Whoosh, 1.1, pos.x);
            }
        }
    }

    fn set_physics(&mut self, cx: &mut Cx, on: bool, at: V2) {
        if cx.world.gravity == on {
            cx.au.sfx_vol(Sfx::Bloop, 1.0, at.x, 0.4);
            return;
        }
        cx.world.set_gravity(on);
        if on {
            cx.au.say("¡Caer! Lo que no tiene apoyo se cae.");
            cx.au.sfx(Sfx::Boing, 0.9, at.x);
        } else {
            cx.au.say("¡Flotar! Los bloques se quedan en el aire.");
            cx.au.sfx(Sfx::Magic, 1.0, at.x);
        }
    }

    fn undo(&mut self, cx: &mut Cx) {
        if cx.world.undo() {
            self.counting = None;
            cx.au.sfx(Sfx::Whoosh, 0.8, cx.w * 0.5);
            cx.au.say_if_quiet("¡Deshacer!");
        } else {
            cx.au.sfx_vol(Sfx::Bloop, 0.9, cx.w * 0.5, 0.4);
        }
    }

    // --- Mundo ----------------------------------------------------------------

    fn pick(&self, cx: &Cx, p: V2) -> Option<(voxel::Hit, Target)> {
        let (o, d) = self.cam.ray(p.x, p.y, cx.w.max(1.0), cx.h.max(1.0));
        let hit = raycast(&cx.world.world, o, d)?;
        Some((hit, place_target(&hit)))
    }

    fn screen(&self, cx: &Cx, p: V3) -> Option<V2> {
        self.cam
            .project(p, cx.w.max(1.0), cx.h.max(1.0))
            .map(|(x, y)| v2(x, y))
    }

    /// Pone el bloque elegido en `cell`. Devuelve true si se puso.
    fn place(&mut self, cx: &mut Cx, cell: [i32; 3], in_stroke: bool) -> bool {
        let Tool::Block(id) = self.tool else {
            return false;
        };
        cx.world.place(cell, id, in_stroke) == PlaceResult::Ok
    }

    fn stamp(&mut self, cx: &mut Cx, piece: Piece, base: [i32; 3]) {
        let cells = piece_cells(piece, base);
        cx.world.begin_stroke();
        let mut placed = 0;
        for (i, (p, id)) in cells.iter().enumerate() {
            if cx.world.place(*p, *id, i > 0) == PlaceResult::Ok {
                placed += 1;
            }
        }
        cx.world.end_stroke();
        if placed > 0 {
            let s = self.screen(cx, center_of(base)).unwrap_or(cx.center());
            cx.fx.confetti(&mut cx.rng, s, 30, 500.0);
            cx.au.sfx(Sfx::Magic, 1.0, s.x);
            cx.au.say(match piece {
                Piece::Tree => "¡Un árbol!",
                Piece::House => "¡Una casita!",
            });
        }
    }

    fn click_world(&mut self, cx: &mut Cx, p: V2, button: u32) {
        let picked = self.pick(cx, p);
        let erase = button == 2 || self.tool == Tool::Eraser;
        match picked {
            Some((hit, target)) => {
                if erase {
                    if hit.floor() {
                        // El pasto no se saca: tiembla un poquito y ya.
                        cx.au.sfx_vol(Sfx::Boing, 1.3, p.x, 0.5);
                    } else {
                        cx.world.remove(hit.cell);
                    }
                    return;
                }
                match target {
                    Target::Place(cell) => match self.tool {
                        Tool::Piece(piece) => self.stamp(cx, piece, cell),
                        _ => {
                            if self.place(cx, cell, true) {
                                cx.world.begin_stroke();
                                if let Some(d) = self.drag.as_mut() {
                                    d.layer = Some(cell[1]);
                                    d.placed = 1;
                                    d.cooldown = STROKE_RATE;
                                }
                            }
                        }
                    },
                    Target::TooHigh(_) => {
                        if self.sky_cd <= 0.0 {
                            cx.au.say("¡Llegaste al cielo!");
                            self.sky_cd = 4.0;
                        }
                        cx.au.sfx(Sfx::Sparkle, 1.3, p.x);
                        cx.fx.sparkle(&mut cx.rng, p, 16, hex(0xFFF59D), 60.0);
                    }
                    Target::Outside => {
                        cx.au.sfx_vol(Sfx::Bloop, 1.0, p.x, 0.4);
                    }
                }
            }
            None => {
                // Clic en el cielo o en la pradera lejana: estrellitas.
                let c = hsv(cx.rng.f(), 0.5, 1.0);
                cx.fx.sparkle(&mut cx.rng, p, 10, c, 50.0);
                cx.au.note(
                    pentatonic(cx.rng.int(8) as i32 + 6, 60.0),
                    Inst::Bell,
                    p.x,
                    0.3,
                );
            }
        }
    }

    /// Bloque que cae del cielo en la columna (x, z) (teclado).
    fn drop_block(&mut self, cx: &mut Cx, x: i32, z: i32, id: u8) -> bool {
        let top = cx.world.world.top_below(x, z, SY) + 1;
        if top >= SY {
            return false;
        }
        // Con física aparece arriba y cae; sin física se apila directo.
        let y = if cx.world.gravity {
            (SY - 1).max(top)
        } else {
            top
        };
        cx.world.place([x, y, z], id, false) == PlaceResult::Ok
    }

    fn mouse_column(&self, cx: &Cx) -> (i32, i32) {
        if cx.inp.has_mouse {
            if let Some((hit, _)) = self.pick(cx, cx.inp.mouse) {
                return (hit.cell[0], hit.cell[2]);
            }
        }
        (SX / 2, SZ / 2)
    }

    fn handle_events(&mut self, cx: &mut Cx) {
        let events = std::mem::take(&mut cx.world.events);
        let mut landed = 0;
        for e in events {
            match e {
                Event::Placed(p, id) => {
                    self.anims.retain(|a| a.cell != p);
                    self.anims.push(Anim {
                        cell: p,
                        kind: AnimKind::Grow,
                        t: 0.0,
                    });
                    let s = self.screen(cx, center_of(p)).unwrap_or(cx.center());
                    let c = rgba(def(id).icon);
                    cx.fx.sparkle(&mut cx.rng, s, 8, c, 40.0);
                    cx.au.note(
                        pentatonic(p[1] + 4 + (id as i32 % 3), 60.0),
                        Inst::Marimba,
                        s.x,
                        0.5,
                    );
                    // Torre: el número de su altura flota arriba y se dice.
                    let below = cx.world.world.get([p[0], p[1] - 1, p[2]]);
                    if p[1] >= 1 && below != AIR && self.counting.is_none() {
                        let h = p[1] as u32 + 1;
                        self.numbers.retain(|n| n.t < 1.2);
                        self.numbers.push(FloatingNumber {
                            at: center_of(p) + v3(0.0, 1.0, 0.0),
                            text: h.to_string(),
                            t: 0.0,
                        });
                        if (h as usize) < vocab::NUMBERS.len() {
                            cx.au.say_if_quiet(&format!(
                                "¡{}!",
                                capitalize(vocab::NUMBERS[h as usize])
                            ));
                        }
                    }
                }
                Event::Removed(p, id) => {
                    self.pops.push(Pop {
                        center: center_of(p),
                        id,
                        t: 0.0,
                    });
                    let s = self.screen(cx, center_of(p)).unwrap_or(cx.center());
                    cx.fx.confetti(&mut cx.rng, s, 14, 380.0);
                    cx.au.sfx(Sfx::Pop, 1.0 + cx.rng.range(0.0, 0.3), s.x);
                }
                Event::Wobble(_) => {
                    if self.wobble_cd <= 0.0 {
                        cx.au.sfx_vol(Sfx::Wiggle, 1.0, cx.w * 0.5, 0.6);
                        self.wobble_cd = 0.6;
                    }
                }
                Event::Saved(_) => {
                    cx.au.sfx(Sfx::Sparkle, 1.0, cx.w * 0.5);
                    cx.au.say_if_quiet("¡Uf, lo salvaste!");
                }
                Event::Fell(_, _) => {}
                Event::Landed(p, _id, v) => {
                    landed += 1;
                    self.anims.retain(|a| a.cell != p);
                    self.anims.push(Anim {
                        cell: p,
                        kind: AnimKind::Land,
                        t: 0.0,
                    });
                    if self.land_sound_cd <= 0.0 {
                        let s = self
                            .screen(cx, center_of(p))
                            .map(|s| s.x)
                            .unwrap_or(cx.w * 0.5);
                        cx.au
                            .note(if v > 12.0 { 0.0 } else { 3.0 }, Inst::Drum, s, 0.5);
                        self.land_sound_cd = 0.045;
                    }
                }
                Event::Full => {
                    if self.full_cd <= 0.0 {
                        cx.au.say("¡Ya no caben más bloques!");
                        self.full_cd = 5.0;
                    }
                    cx.au.sfx_vol(Sfx::Boing, 0.8, cx.w * 0.5, 0.6);
                }
            }
        }
        if landed > 0 {
            let now = cx.t;
            for _ in 0..landed {
                self.landed_recent.push(now);
            }
            self.landed_recent.retain(|t| now - *t < 0.5);
            if self.landed_recent.len() >= 5 && self.crash_cd <= 0.0 {
                cx.au.say_if_quiet("¡Cataplúm!");
                self.crash_cd = 2.0;
            }
        }
        if self.anims.len() > 128 {
            let extra = self.anims.len() - 128;
            self.anims.drain(0..extra);
        }
    }

    /// Rehace la lista de cubos quietos si el mundo cambió.
    fn rebuild_cache(&mut self, cx: &Cx) {
        let sim = &cx.world;
        if self.cache_version == sim.version {
            return;
        }
        self.cache_version = sim.version;
        self.cache.clear();
        self.cache_index.clear();
        let w = &sim.world;
        let opaque = |x: i32, y: i32, z: i32| -> bool {
            if y < 0 {
                return true;
            }
            let k = w.get([x, y, z]);
            k != AIR && !mat::translucent(def(k).mat)
        };
        let wobbling: Vec<usize> = sim.wobble.iter().map(|(i, _)| *i as usize).collect();
        for y in 0..SY {
            for z in 0..SZ {
                for x in 0..SX {
                    let p = [x, y, z];
                    let k = w.get(p);
                    if k == AIR {
                        continue;
                    }
                    let d = def(k);
                    let i = voxel::idx(p);
                    let wob = wobbling.contains(&i);
                    let mask = face_mask(p, |a, b, c| {
                        if wob {
                            return b < 0;
                        }
                        if opaque(a, b, c) {
                            return true;
                        }
                        // El mismo material translúcido junto: sin pared entre medio.
                        let n = w.get([a, b, c]);
                        n != AIR && def(n).mat == d.mat && mat::translucent(d.mat)
                    });
                    if mask == 0 {
                        continue;
                    }
                    let ao = if wob {
                        AO_NONE
                    } else {
                        ambient_occlusion(p, opaque)
                    };
                    let cube = Cube::at_cell(x, y, z, d.mat, rgba(d.tint))
                        .faces(mask)
                        .with_ao(ao);
                    self.cache_index.insert(p, self.cache.len());
                    self.cache.push((p, cube));
                }
            }
        }
        // Pasto del área de construcción (con su grilla y su sombra de contacto).
        if self.floor_version != sim.version {
            self.floor_version = sim.version;
            self.floor.clear();
            let solid = |x: i32, y: i32, z: i32| {
                y < 0 || w.get([x, y, z]) != AIR && !mat::translucent(def(w.get([x, y, z])).mat)
            };
            for z in 0..SZ {
                for x in 0..SX {
                    let ao = ambient_occlusion([x, -1, z], solid);
                    self.floor.push(
                        Cube::at_cell(x, -1, z, mat::PASTO, WHITE)
                            .faces(1 << 2)
                            .with_ao(ao),
                    );
                }
            }
        }
    }

    fn draw_world(&mut self, cx: &mut Cx) {
        self.rebuild_cache(cx);
        let d3 = &mut cx.g.d3;
        for c in &self.floor {
            d3.opaque(*c);
        }
        for c in &self.scenery {
            d3.opaque(*c);
        }
        // Celdas animadas o resaltadas: se ajusta su copia.
        let mut tweak: HashMap<usize, Cube> = HashMap::new();
        for a in &self.anims {
            if let Some(&i) = self.cache_index.get(&a.cell) {
                let c = self.cache[i].1;
                let c = match a.kind {
                    AnimKind::Grow => {
                        let k = ease_out_back((a.t / 0.25).min(1.0)).max(0.05);
                        c.scaled(k, k)
                    }
                    AnimKind::Land => {
                        let k = (1.0 - a.t / 0.3).max(0.0);
                        c.scaled(1.0 + 0.14 * k, 1.0 - 0.18 * k)
                    }
                };
                tweak.insert(i, c);
            }
        }
        // Bloques que tiemblan antes de caer.
        let t = cx.t;
        for (ci, _) in &cx.world.wobble {
            let p = voxel::pos(*ci as usize);
            if let Some(&i) = self.cache_index.get(&p) {
                let c = tweak.get(&i).copied().unwrap_or(self.cache[i].1);
                let jitter = (t * 40.0 + p[0] as f32).sin() * 0.06;
                tweak.insert(i, c.rotated(v3(1.0, 0.0, 0.6), jitter));
            }
        }
        // Resaltado de lo apuntado.
        if let Some((hit, _)) = self.hover {
            if !hit.floor() {
                if let Some(&i) = self.cache_index.get(&hit.cell) {
                    let c = tweak.get(&i).copied().unwrap_or(self.cache[i].1);
                    let hl = if self.tool == Tool::Eraser {
                        HL_ALL
                    } else {
                        face_index(hit.normal) + 1
                    };
                    tweak.insert(i, c.highlight(hl));
                }
            }
        }
        for (i, (_, c)) in self.cache.iter().enumerate() {
            d3.cube(tweak.get(&i).copied().unwrap_or(*c));
        }
        // Bloques cayendo.
        for f in &cx.world.fallers {
            let d = def(f.kind);
            let c = Cube::new(
                v3(f.x as f32 + 0.5, f.y + 0.5, f.z as f32 + 0.5),
                d.mat,
                rgba(d.tint),
            )
            .seed((f.x.wrapping_mul(73856093) ^ f.z.wrapping_mul(83492791)) as u32 as u8);
            d3.cube(c);
        }
        // Bloques que se van.
        for p in &self.pops {
            let d = def(p.id);
            let k = (1.0 - p.t / 0.22).max(0.0);
            if k > 0.0 {
                let c = Cube::new(p.center, d.mat, rgba(d.tint)).scaled(k * 1.1, k * 1.1);
                d3.cube(c);
            }
        }
        // Fantasma: dónde va a quedar el bloque.
        if let Some((_, Target::Place(cell))) = self.hover {
            match self.tool {
                Tool::Block(id) => {
                    let d = def(id);
                    let tint = if d.mat == mat::JUGUETE {
                        rgba(d.tint)
                    } else {
                        rgba(d.icon)
                    };
                    d3.ghost(Cube::at_cell(cell[0], cell[1], cell[2], d.mat, tint));
                }
                Tool::Piece(piece) => {
                    for (p, id) in piece_cells(piece, cell).into_iter().take(8) {
                        let d = def(id);
                        d3.ghost(Cube::at_cell(p[0], p[1], p[2], d.mat, rgba(d.icon)));
                    }
                }
                Tool::Eraser => {}
            }
        }
    }

    fn update_camera(&mut self, cx: &mut Cx) {
        let dt = cx.dt;
        let held = |a: &str, b: &str| cx.inp.held(a) || cx.inp.held(b);
        if held("ArrowLeft", "KeyA") {
            self.yaw_goal -= 1.6 * dt;
        }
        if held("ArrowRight", "KeyD") {
            self.yaw_goal += 1.6 * dt;
        }
        if held("ArrowUp", "KeyW") {
            self.pitch_goal += 0.9 * dt;
        }
        if held("ArrowDown", "KeyS") {
            self.pitch_goal -= 0.9 * dt;
        }
        // Vitrina: si nadie toca nada, el mundo gira muy despacio.
        let idle = cx.inp.idle > 20.0;
        self.auto_turn = approach(self.auto_turn, if idle { 0.1 } else { 0.0 }, 1.5, dt);
        self.yaw_goal += self.auto_turn * dt;
        self.pitch_goal = self.pitch_goal.clamp(PITCH_MIN + 0.05, PITCH_MAX - 0.05);
        if !self.yaw_goal.is_finite() {
            self.yaw_goal = 0.0;
        }
        self.cam.yaw = approach(self.cam.yaw, self.yaw_goal, 8.0, dt);
        self.cam.pitch =
            approach(self.cam.pitch, self.pitch_goal, 8.0, dt).clamp(PITCH_MIN, PITCH_MAX);
        // En pantallas angostas (verticales) se abre un poco la lente.
        let narrow = (cx.h / cx.w.max(1.0)).max(1.0);
        let fov = (FOVS[self.fov_i] * narrow.powf(0.6)).min(1.3);
        self.cam.fovy = approach(self.cam.fovy, fov, 8.0, dt);
        // Mirar un poco más arriba si la torre crece.
        let h = tallest(&cx.world.world) as f32;
        let ty = (0.4 * h).clamp(0.5, 6.0);
        self.cam.target.y = approach(self.cam.target.y, ty, 2.0, dt);
        self.cam.dist = CAM_DIST;
    }

    fn tick_counting(&mut self, cx: &mut Cx) {
        let Some(c) = self.counting.as_mut() else {
            return;
        };
        c.timer -= cx.dt;
        if c.timer > 0.0 {
            return;
        }
        if c.done >= c.n {
            let n = c.n;
            self.counting = None;
            if (n as usize) < vocab::NUMBERS.len() {
                let w = vocab::Word {
                    word: "bloque",
                    plural: "bloques",
                    emoji: "",
                    gender: vocab::Gender::M,
                    sound: "",
                };
                cx.au
                    .say(&format!("¡{}!", capitalize(&vocab::count_phrase(n, &w))));
            }
            cx.au.sfx(Sfx::Sparkle, 1.0, cx.w * 0.5);
            return;
        }
        let (x, z, id) = (c.x, c.z, c.id);
        let top = cx.world.world.top_below(x, z, SY) + 1;
        c.done += 1;
        c.timer = 0.6;
        let k = c.done;
        if top < SY && cx.world.place([x, top, z], id, false) == PlaceResult::Ok {
            cx.au.say(vocab::NUMBERS[k.min(10) as usize]);
        } else {
            // No cabe más: se termina la cuenta ahí.
            if let Some(c) = self.counting.as_mut() {
                c.n = c.done - 1;
            }
        }
    }

    fn draw_ui(&mut self, cx: &mut Cx) {
        let t = cx.t;
        let mouse = cx.inp.mouse;
        let has_mouse = cx.inp.has_mouse;
        // Fondo de la barra.
        let bar_h = cx.h - self.bar_top;
        cx.g.rrect(
            v2(cx.w * 0.5, self.bar_top + bar_h * 0.5 + 6.0),
            v2(cx.w * 0.5 + 10.0, bar_h * 0.5 + 6.0),
            0.0,
            WHITE.alpha(0.82),
        );
        let mut announce = None;
        for (i, b) in self.buttons.iter_mut().enumerate() {
            if b.hover
                .update(cx, has_mouse && mouse.dist(b.pos) < b.r * 1.15)
            {
                announce = Some(i);
            }
        }
        if let Some(i) = announce {
            let b = &self.buttons[i];
            cx.au
                .sfx_vol(Sfx::Tick, 1.0 + (i % 6) as f32 * 0.05, b.pos.x, 0.45);
            let name = match b.ui {
                Ui::Tool(Tool::Block(id)) => Some(def(id).name),
                Ui::Tool(Tool::Piece(Piece::Tree)) => Some("Árbol"),
                Ui::Tool(Tool::Piece(Piece::House)) => Some("Casita"),
                Ui::Tool(Tool::Eraser) => Some("Goma"),
                Ui::Undo => Some("Deshacer"),
                Ui::Clear => Some("Borrar todo"),
                Ui::Physics(true) => Some("Caer"),
                Ui::Physics(false) => Some("Flotar"),
                Ui::Drawer(d) => Some(DRAWERS[d].1),
                Ui::Turn(_) => None,
            };
            if let Some(n) = name {
                cx.au.say_if_quiet(n);
            }
        }
        let gravity = cx.world.gravity;
        for b in &self.buttons {
            let selected = match b.ui {
                Ui::Tool(tool) => tool == self.tool,
                Ui::Drawer(d) => d == self.drawer,
                Ui::Physics(on) => on == gravity,
                _ => false,
            };
            let lift = if selected { b.r * 0.3 } else { 0.0 } + b.hover.anim * b.r * 0.12;
            let r = b.r * (1.0 + 0.15 * b.hover.anim + if selected { 0.12 } else { 0.0 });
            let p = b.pos - v2(0.0, lift);
            match b.ui {
                Ui::Tool(Tool::Block(id)) => {
                    let d = def(id);
                    cx.g.shadow(Shape::Circle, p, r * 0.9, 0.0, 0.7);
                    cx.g.shape(Shape::Cube, p, r * 0.92, 0.0, rgba(d.icon));
                    if id == 21 {
                        cx.g.emoji("☁️", p + v2(0.0, r * 0.1), r * 0.42, 0.0, 1.0);
                    }
                    if id == ID_VIDRIO {
                        cx.g.emoji("🪟", p + v2(0.0, r * 0.1), r * 0.36, 0.0, 0.9);
                    }
                    if id == 22 {
                        cx.g.emoji("💧", p + v2(0.0, r * 0.08), r * 0.36, 0.0, 1.0);
                    }
                }
                Ui::Tool(tool) => {
                    let (e, c) = match tool {
                        Tool::Piece(Piece::Tree) => ("🌳", hex(0xE8F5E9)),
                        Tool::Piece(Piece::House) => ("🏠", hex(0xFFF3E0)),
                        _ => ("🧽", hex(0xFCE4EC)),
                    };
                    cx.g.shadow(Shape::Circle, p, r, 0.0, 0.8);
                    cx.g.sticker(Shape::Circle, p, r, 0.0, c);
                    cx.g.emoji(e, p, r * 0.66, 0.0, 1.0);
                }
                Ui::Undo | Ui::Clear | Ui::Drawer(_) => {
                    let e = match b.ui {
                        Ui::Undo => "↩️",
                        Ui::Clear => "🧹",
                        Ui::Drawer(d) => DRAWERS[d].0,
                        _ => "",
                    };
                    cx.g.shadow(Shape::Circle, p, r, 0.0, 0.8);
                    cx.g.sticker(
                        Shape::Circle,
                        p,
                        r,
                        0.0,
                        if selected {
                            hex(0xFFF59D)
                        } else {
                            hex(0xECEFF1)
                        },
                    );
                    cx.g.emoji(e, p, r * 0.66, 0.0, 1.0);
                }
                Ui::Physics(on) => {
                    let e = if on { "🍎" } else { "🎈" };
                    let k = if selected { 1.0 } else { 0.78 };
                    let face = if selected {
                        hex(0xFFF59D)
                    } else {
                        WHITE.alpha(0.85)
                    };
                    cx.g.shadow(Shape::Circle, p, r * k, 0.0, 1.0);
                    cx.g.sticker(Shape::Circle, p, r * k, 0.0, face);
                    let bob = if selected {
                        (t * 3.0).sin() * r * 0.06
                    } else {
                        0.0
                    };
                    cx.g.emoji(
                        e,
                        p - v2(0.0, bob),
                        r * k * 0.62,
                        0.0,
                        if selected { 1.0 } else { 0.6 },
                    );
                }
                Ui::Turn(dir) => {
                    cx.g.shadow(Shape::Circle, p, r, 0.0, 0.6);
                    cx.g.sticker(Shape::Circle, p, r, 0.0, WHITE.alpha(0.75));
                    let rot = if dir > 0.0 { 0.0 } else { PI };
                    cx.g.shape_full(
                        Shape::Arrow,
                        p,
                        v2(r * 0.55, r * 0.55),
                        rot,
                        hex(0x546E7A),
                        0.0,
                        0.0,
                    );
                }
            }
            if selected && matches!(b.ui, Ui::Tool(_)) {
                cx.g.ring(p, r * 1.2, 3.5, hex(0x37474F).alpha(0.7));
            }
        }

        // Números de altura flotando sobre las torres.
        let numbers: Vec<(V2, String, f32)> = self
            .numbers
            .iter()
            .filter_map(|n| self.screen(cx, n.at).map(|s| (s, n.text.clone(), n.t)))
            .collect();
        for (s, text, nt) in numbers {
            let k = ease_out_back((nt / 0.3).min(1.0));
            let a = (1.0 - (nt - 1.2).max(0.0) / 0.4).clamp(0.0, 1.0);
            let r = cx.unit() * 0.045 * k;
            cx.g.text(
                &text,
                s - v2(0.0, nt * 20.0),
                r,
                0.0,
                hex(0xFFB300).alpha(a),
                0.16,
            );
        }

        // Pista cuando no pasa nada: un cursor que "hace clic" en el pasto.
        if self.idle.waiting(cx) {
            if let Some(s) = self.screen(cx, v3(SX as f32 * 0.5, 0.0, SZ as f32 * 0.5)) {
                let k = 1.0 + 0.15 * (t * 4.0).sin();
                cx.g.emoji("👆", s + v2(0.0, 30.0), 34.0 * k, 0.0, 0.9);
            }
        }
    }
}

/// Celdas de una pieza armada, con su bloque. `base` = celda donde se hizo clic.
fn piece_cells(piece: Piece, base: [i32; 3]) -> Vec<([i32; 3], u8)> {
    let [x, y, z] = base;
    let mut v = Vec::new();
    match piece {
        Piece::Tree => {
            for k in 0..3 {
                v.push(([x, y + k, z], ID_TRONCO));
            }
            for dy in 3..5 {
                for dx in -1..=1 {
                    for dz in -1..=1 {
                        v.push(([x + dx, y + dy, z + dz], ID_HOJAS));
                    }
                }
            }
            v.push(([x, y + 5, z], ID_HOJAS));
        }
        Piece::House => {
            // Casita de 3×3: paredes de ladrillo, puerta de madera, ventanas
            // de vidrio y techo rojo. Se arma de abajo hacia arriba.
            for dy in 0..2 {
                for dx in -1..=1 {
                    for dz in -1..=1 {
                        let edge = dx != 0 || dz != 0;
                        if !edge {
                            continue;
                        }
                        let id = if dz == 1 && dx == 0 {
                            ID_MADERA
                        } else if dy == 1 && dz == 0 && dx != 0 {
                            ID_VIDRIO
                        } else {
                            ID_LADRILLO
                        };
                        v.push(([x + dx, y + dy, z + dz], id));
                    }
                }
            }
            for dx in -1..=1 {
                for dz in -1..=1 {
                    v.push(([x + dx, y + 2, z + dz], ID_ROJO));
                }
            }
        }
    }
    v.retain(|(p, _)| voxel::in_bounds(*p));
    v
}

/// Índice de cara (orden de gfx3d::FACES) según su normal.
fn face_index(n: [i32; 3]) -> u8 {
    match n {
        [1, 0, 0] => 0,
        [-1, 0, 0] => 1,
        [0, 1, 0] => 2,
        [0, -1, 0] => 3,
        [0, 0, 1] => 4,
        _ => 5,
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Mundo inicial: un árbol y una torre empezada (invita a seguirla).
fn seed_world(sim: &mut voxel::Sim) {
    for (p, id) in piece_cells(Piece::Tree, [4, 0, 5]) {
        sim.world.cells[voxel::idx(p)] = id;
    }
    for (y, id) in [ID_ROJO, ID_AMARILLO, ID_AZUL].into_iter().enumerate() {
        sim.world.cells[voxel::idx([SX / 2, y as i32, SZ / 2])] = id;
    }
    sim.gravity = true;
    sim.version += 1;
}

/// Árboles decorativos en la pradera, fuera del área de construcción.
fn scenery() -> Vec<Cube> {
    let spots: [(i32, i32, i32); 14] = [
        (-12, 3, 4),
        (-15, 14, 5),
        (-11, 25, 3),
        (36, 2, 5),
        (39, 13, 4),
        (35, 25, 3),
        (5, -13, 4),
        (18, -15, 5),
        (4, 37, 4),
        (19, 39, 5),
        (-40, -20, 5),
        (60, 10, 6),
        (12, 64, 5),
        (-30, 50, 4),
    ];
    let mut cells: HashMap<[i32; 3], u8> = HashMap::new();
    for (x, z, h) in spots {
        for y in 0..h {
            cells.insert([x, y, z], mat::TRONCO);
        }
        for dy in h..h + 2 {
            for dx in -1..=1 {
                for dz in -1..=1 {
                    cells.insert([x + dx, dy, z + dz], mat::HOJAS);
                }
            }
        }
        cells.insert([x, h + 2, z], mat::HOJAS);
    }
    let solid = |x: i32, y: i32, z: i32| y < 0 || cells.contains_key(&[x, y, z]);
    let mut out: Vec<Cube> = cells
        .iter()
        .map(|(p, m)| {
            let mask = face_mask(*p, solid);
            Cube::at_cell(p[0], p[1], p[2], *m, WHITE)
                .faces(mask)
                .with_ao(ambient_occlusion(*p, solid))
        })
        .filter(|c| c.info[1] != 0)
        .collect();
    // Orden fijo (el HashMap no tiene orden): así la imagen no parpadea.
    out.sort_by(|a, b| {
        a.pos[0]
            .total_cmp(&b.pos[0])
            .then(a.pos[2].total_cmp(&b.pos[2]))
            .then(a.pos[1].total_cmp(&b.pos[1]))
    });
    out
}

impl Activity for Build {
    fn enter(&mut self, cx: &mut Cx) {
        if cx.world.count() > 3 {
            cx.au.say("¡Tu construcción te estaba esperando!");
        } else {
            cx.au
                .say("¡A construir! Haz clic en el pasto para poner un bloque.");
        }
    }

    fn resize(&mut self, cx: &mut Cx) {
        self.layout(cx);
    }

    fn cursor_trail(&self) -> bool {
        false
    }

    fn update(&mut self, cx: &mut Cx) {
        let dt = cx.dt;
        // Respaldo 2D mientras el 3D carga (o si el computador no lo soporta).
        cx.g.background(hex(0x8CCBFF), hex(0xBFE6A8), Backdrop::Meadow, 0.0);
        for cd in [
            &mut self.land_sound_cd,
            &mut self.crash_cd,
            &mut self.full_cd,
            &mut self.sky_cd,
            &mut self.wobble_cd,
        ] {
            *cd -= dt;
        }
        self.idle
            .tick(cx, "¡Haz clic en el pasto para poner un bloque!");

        // Arrastre: se termina si se soltaron los botones (también al perder el foco).
        if self.drag.is_some() && !cx.inp.any_button() {
            self.drag = None;
            cx.world.end_stroke();
        }
        if let Some(d) = self.drag.as_mut() {
            d.cooldown -= dt;
        }

        self.update_camera(cx);
        cx.world.update(dt);
        self.tick_counting(cx);
        self.handle_events(cx);

        for a in self.anims.iter_mut() {
            a.t += dt;
        }
        self.anims.retain(|a| a.t < 0.35);
        for p in self.pops.iter_mut() {
            p.t += dt;
        }
        self.pops.retain(|p| p.t < 0.25);
        for n in self.numbers.iter_mut() {
            n.t += dt;
        }
        self.numbers.retain(|n| n.t < 1.6);

        // Lo que apunta el mouse.
        self.hover = None;
        if cx.inp.has_mouse
            && !self.over_ui(cx.inp.mouse)
            && !self.drag.as_ref().is_some_and(|d| d.orbit)
        {
            self.hover = self.pick(cx, cx.inp.mouse);
        }

        if cx.g.d3.status == 2 {
            // El computador no puede mostrar 3D: un aviso amable.
            let c = cx.center();
            cx.g.emoji("🧱", c, cx.unit() * 0.12, 0.0, 1.0);
            if self.idle.waiting(cx) {
                cx.au
                    .say_if_quiet("Este computador no puede mostrar el mundo de bloques.");
            }
            return;
        }

        let (w, h, t) = (cx.w.max(1.0), cx.h.max(1.0), cx.t);
        cx.g.d3.setup(
            &self.cam,
            w,
            h,
            t,
            hex(0x5BA7F5),
            hex(0xCFE8FF),
            (0, 0, SX),
            true,
        );
        cx.g.d3.scene.misc[1] = 1.0; // pradera
        self.draw_world(cx);
        self.draw_ui(cx);
    }

    fn down(&mut self, cx: &mut Cx, p: V2, button: u32) {
        if let Some(i) = self.button_at(p) {
            self.press_ui(cx, i);
            return;
        }
        if p.y > self.bar_top {
            return;
        }
        // Botón derecho o del medio, o izquierdo en el cielo: girar el mundo.
        let on_world = self.pick(cx, p).is_some();
        let orbit = button == 1 || (button == 0 && !on_world);
        self.drag = Some(Drag {
            last: p,
            moved: 0.0,
            button,
            orbit,
            layer: None,
            placed: 0,
            cooldown: STROKE_RATE,
        });
        if button == 0 {
            self.click_world(cx, p, 0);
        }
    }

    fn up(&mut self, cx: &mut Cx, p: V2, button: u32) {
        if let Some(d) = self.drag.take() {
            // Clic derecho sin arrastrar = sacar el bloque.
            if d.button == 2 && button == 2 && d.moved < 6.0 {
                self.click_world(cx, p, 2);
            }
            cx.world.end_stroke();
        }
    }

    fn moved(&mut self, cx: &mut Cx, from: V2, to: V2) {
        let Some(mut d) = self.drag.take() else {
            return;
        };
        d.moved += from.dist(to);
        let delta = to - d.last;
        d.last = to;
        if d.button == 2 && d.moved >= 6.0 {
            d.orbit = true;
        }
        if d.orbit {
            self.yaw_goal -= delta.x * 0.008;
            self.pitch_goal += delta.y * 0.006;
            self.cam.yaw = self.yaw_goal;
        } else if d.button == 0 {
            // Fila de bloques: sólo en la misma capa del primero.
            if self.tool == Tool::Eraser {
                if d.cooldown <= 0.0 && d.placed < MAX_STROKE {
                    if let Some((hit, _)) = self.pick(cx, to) {
                        if !hit.floor() && cx.world.remove(hit.cell).is_some() {
                            d.placed += 1;
                            d.cooldown = STROKE_RATE;
                        }
                    }
                }
            } else if let (Some(layer), Tool::Block(_)) = (d.layer, self.tool) {
                if d.cooldown <= 0.0 && d.placed < MAX_STROKE {
                    if let Some((_, Target::Place(cell))) = self.pick(cx, to) {
                        if cell[1] == layer && self.place(cx, cell, true) {
                            d.placed += 1;
                            d.cooldown = STROKE_RATE;
                        }
                    }
                }
            }
        }
        self.drag = Some(d);
    }

    fn wheel(&mut self, cx: &mut Cx, dy: f32) -> bool {
        let before = self.fov_i;
        if dy < 0.0 && self.fov_i > 0 {
            self.fov_i -= 1;
        } else if dy > 0.0 && self.fov_i + 1 < FOVS.len() {
            self.fov_i += 1;
        }
        if before != self.fov_i {
            cx.au.sfx_vol(
                Sfx::Bloop,
                if dy < 0.0 { 1.3 } else { 0.8 },
                cx.inp.mouse.x,
                0.5,
            );
        }
        true
    }

    fn key(&mut self, cx: &mut Cx, k: &Key) {
        let ctrl = cx.inp.held("ControlLeft") || cx.inp.held("ControlRight");
        if ctrl && k.is("KeyZ") || k.is_erase() {
            self.undo(cx);
            return;
        }
        if k.is("KeyG") {
            let on = !cx.world.gravity;
            let at = v2(cx.w - 60.0, 60.0);
            self.set_physics(cx, on, at);
            return;
        }
        if k.arrow().is_some() || matches!(k.code, "KeyW" | "KeyA" | "KeyS" | "KeyD") {
            // La cámara se mueve en `update` mientras la tecla esté apretada.
            if let Some(name) = vocab::key_name(k.code) {
                cx.au.say_if_quiet(name);
            }
            return;
        }
        if k.is("Tab") {
            let i = (0..self.buttons.len())
                .find(|&i| self.buttons[i].ui == Ui::Drawer((self.drawer + 1) % DRAWERS.len()));
            if let Some(i) = i {
                self.press_ui(cx, i);
            }
            return;
        }
        if k.is("PageUp") || k.is("NumpadAdd") || k.ch == Some('+') {
            self.wheel(cx, -1.0);
            return;
        }
        if k.is("PageDown") || k.is("NumpadSubtract") || k.ch == Some('-') {
            self.wheel(cx, 1.0);
            return;
        }
        if let Some(d) = k.digit() {
            if d == 0 {
                cx.au.say("¡Cero! No hay nada.");
                return;
            }
            // Torre de N bloques que se arma contando.
            let (x, z) = self.mouse_column(cx);
            let id = match self.tool {
                Tool::Block(id) => id,
                _ => (cx.rng.int(8) + 1) as u8,
            };
            cx.au
                .say(&format!("¡{}!", capitalize(vocab::NUMBERS[d as usize])));
            self.counting = Some(Counting {
                x,
                z,
                id,
                n: d,
                done: 0,
                timer: 0.8,
            });
            return;
        }
        if k.is_enter() {
            let n = cx.world.count();
            let phrase = match n {
                0 => "No hay bloques todavía.".to_string(),
                1 => "¡Hay un bloque!".to_string(),
                n if (n as usize) < vocab::NUMBERS.len() => {
                    format!("¡Hay {} bloques!", vocab::NUMBERS[n as usize])
                }
                n => format!("¡Hay {} bloques!", n),
            };
            cx.au.say(&phrase);
            cx.au.sfx(Sfx::Fanfare, 1.0, cx.w * 0.5);
            return;
        }
        if k.is("Space") {
            let (x, z) = self.mouse_column(cx);
            let id = match self.tool {
                Tool::Block(id) => id,
                _ => (cx.rng.int(8) + 1) as u8,
            };
            if self.drop_block(cx, x, z, id) {
                cx.au.say_if_quiet("¡Bloque!");
            }
            return;
        }
        if k.letter().is_some() {
            // Lluvia de bloques de colores en cualquier lugar.
            let x = cx.rng.int(SX as usize) as i32;
            let z = cx.rng.int(SZ as usize) as i32;
            let id = (cx.rng.int(8) + 1) as u8;
            if self.drop_block(cx, x, z, id) {
                cx.au.say_if_quiet(def(id).name);
            }
            return;
        }
        let x = cx.rng.range(0.2, 0.8) * cx.w;
        cx.au
            .note(midi(67.0 + cx.rng.int(8) as f32 * 2.0), Inst::Soft, x, 0.3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::modes::ModeId;

    fn enter_build() -> Game {
        let mut g = Game::new(1280.0, 720.0, 11);
        g.update(0.016, false);
        g.cx.go(ModeId::Build, g.cx.center(), WHITE);
        for _ in 0..80 {
            g.update(0.016, false);
        }
        assert_eq!(g.mode_id, ModeId::Build);
        g
    }

    #[test]
    fn starts_with_tree_and_tower() {
        let g = enter_build();
        assert!(g.cx.world.count() > 10);
        assert_eq!(g.cx.world.world.get([SX / 2, 2, SZ / 2]), ID_AZUL);
    }

    #[test]
    fn clicking_the_grass_places_a_block_and_draws_3d() {
        let mut g = enter_build();
        g.cx.g.d3.status = 1;
        let before = g.cx.world.count();
        // Buscar un punto de la pantalla que apunte al pasto libre.
        let mut placed = false;
        'outer: for y in (200..560).step_by(20) {
            for x in (300..1000).step_by(20) {
                g.pointer_move(x as f32, y as f32);
                g.update(0.016, false);
                g.pointer_down(x as f32, y as f32, 0);
                g.pointer_up(x as f32, y as f32, 0);
                g.update(0.016, false);
                if g.cx.world.count() > before {
                    placed = true;
                    break 'outer;
                }
            }
        }
        assert!(placed, "ningún clic puso un bloque");
        assert!(g.cx.g.d3.active);
        assert!(g.cx.g.d3.cubes.len() > (SX * SZ) as usize);
    }

    #[test]
    fn world_survives_going_home() {
        let mut g = enter_build();
        g.cx.world.place([1, 0, 1], 6, false);
        let n = g.cx.world.count();
        g.cx.go(ModeId::Home, g.cx.center(), WHITE);
        for _ in 0..80 {
            g.update(0.016, false);
        }
        g.cx.go(ModeId::Build, g.cx.center(), WHITE);
        for _ in 0..80 {
            g.update(0.016, false);
        }
        assert_eq!(g.cx.world.count(), n);
    }

    #[test]
    fn pieces_stay_inside_the_world() {
        for piece in [Piece::Tree, Piece::House] {
            for base in [[0, 0, 0], [SX - 1, SY - 1, SZ - 1], [5, 3, 5]] {
                for (p, _) in piece_cells(piece, base) {
                    assert!(voxel::in_bounds(p));
                }
            }
        }
    }

    #[test]
    fn a_house_stands_with_physics() {
        let mut s = voxel::Sim::default();
        s.gravity = true;
        for (p, id) in piece_cells(Piece::House, [8, 0, 8]) {
            assert_eq!(s.place(p, id, false), PlaceResult::Ok);
        }
        let n = s.count();
        for _ in 0..240 {
            s.update(1.0 / 60.0);
        }
        assert_eq!(s.count(), n);
        assert!(s.fallers.is_empty());
        // El techo sigue arriba.
        assert_eq!(s.world.get([8, 2, 8]), ID_ROJO);
    }

    #[test]
    fn counting_tower_with_number_key() {
        let mut g = enter_build();
        let before = g.cx.world.count();
        let code = crate::game::KEY_CODES
            .iter()
            .position(|c| *c == "Digit3")
            .unwrap() as u32;
        g.key_down(code, '3' as u32, false);
        g.key_up(code);
        for _ in 0..300 {
            g.update(0.016, false);
        }
        assert_eq!(g.cx.world.count(), before + 3);
    }
}
