//! Mundo de bloques del juego Construir: rayo DDA, apoyos ("regla del
//! andamio"), bloques que tambalean y caen, deshacer. Puro: no conoce
//! Cx, ni audio, ni dibujo; devuelve eventos que el modo convierte en
//! sonido, voz y chispitas.

use std::collections::VecDeque;

use crate::gfx3d::{v3, V3};

pub const SX: i32 = 24;
pub const SY: i32 = 16;
pub const SZ: i32 = 24;
pub const CELLS: usize = (SX * SY * SZ) as usize;

pub const AIR: u8 = 0;
pub const MAX_BLOCKS: u32 = 3000;
pub const MAX_UNDO: usize = 40;
/// Cuántos bloques puede "sobresalir" algo de lado sin apoyo abajo.
pub const REACH: u8 = 3;
/// Segundos que tambalea un bloque antes de caer.
pub const WOBBLE: f32 = 0.35;
pub const GRAVITY: f32 = 35.0;
pub const VMAX: f32 = 25.0;
pub const STEP: f32 = 1.0 / 120.0;
pub const MAX_STEPS_PER_FRAME: u32 = 8;
/// Límites de la cámara: con esta distancia y esta inclinación el ojo nunca
/// queda dentro de la caja del mundo (hay una prueba).
pub const CAM_DIST: f32 = 30.0;
pub const PITCH_MIN: f32 = 0.30;
pub const PITCH_MAX: f32 = 1.20;
const NONE: u8 = u8::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fall {
    /// Se sostiene desde abajo o de lado (hasta REACH).
    Normal,
    /// Sólo se sostiene desde abajo (arena).
    Sand,
    /// Flota siempre y sostiene a sus vecinos (globo).
    Float,
}

/// Comportamiento por tipo de bloque (el resto de la tabla — nombre,
/// color, material, emoji — vive en el modo / vocab).
pub fn fall_of(kind: u8) -> Fall {
    match kind {
        20 => Fall::Sand,
        21 => Fall::Float,
        _ => Fall::Normal,
    }
}

pub fn in_bounds(p: [i32; 3]) -> bool {
    (0..SX).contains(&p[0]) && (0..SY).contains(&p[1]) && (0..SZ).contains(&p[2])
}

pub fn idx(p: [i32; 3]) -> usize {
    ((p[1] * SZ + p[2]) * SX + p[0]) as usize
}

pub fn pos(i: usize) -> [i32; 3] {
    let i = i as i32;
    [i % SX, i / (SX * SZ), (i / SX) % SZ]
}

#[derive(Clone)]
pub struct World {
    pub cells: Box<[u8]>,
}

impl Default for World {
    fn default() -> Self {
        World {
            cells: vec![AIR; CELLS].into_boxed_slice(),
        }
    }
}

impl World {
    pub fn get(&self, p: [i32; 3]) -> u8 {
        if in_bounds(p) {
            self.cells[idx(p)]
        } else {
            AIR
        }
    }
    /// Sólido para el rayo: bloques, y el suelo (y = -1) dentro del mundo.
    pub fn solid(&self, p: [i32; 3]) -> bool {
        if p[1] == -1 {
            return (0..SX).contains(&p[0]) && (0..SZ).contains(&p[2]);
        }
        self.get(p) != AIR
    }
    pub fn count(&self) -> u32 {
        self.cells.iter().filter(|&&c| c != AIR).count() as u32
    }
    /// Bloque sólido más alto con y < `below` en la columna (o -1 = suelo).
    pub fn top_below(&self, x: i32, z: i32, below: i32) -> i32 {
        let mut y = below.min(SY) - 1;
        while y >= 0 {
            if self.cells[idx([x, y, z])] != AIR {
                return y;
            }
            y -= 1;
        }
        -1
    }
}

// ---------------------------------------------------------------------------
// Rayo (Amanatides & Woo), recortado a la caja del mundo + capa de suelo.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub cell: [i32; 3],
    /// Normal de la cara por donde entró el rayo ([0,0,0] = no se sabe).
    pub normal: [i32; 3],
    pub t: f32,
}

impl Hit {
    pub fn floor(&self) -> bool {
        self.cell[1] < 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Place([i32; 3]),
    /// Más arriba del techo del mundo: se celebra, no se pone nada.
    TooHigh([i32; 3]),
    Outside,
}

pub fn place_target(h: &Hit) -> Target {
    if h.normal == [0, 0, 0] {
        return Target::Outside;
    }
    let p = [
        h.cell[0] + h.normal[0],
        h.cell[1] + h.normal[1],
        h.cell[2] + h.normal[2],
    ];
    if p[1] >= SY && (0..SX).contains(&p[0]) && (0..SZ).contains(&p[2]) {
        Target::TooHigh(p)
    } else if in_bounds(p) {
        Target::Place(p)
    } else {
        Target::Outside
    }
}

pub fn raycast(w: &World, o: V3, d: V3) -> Option<Hit> {
    if !o.is_finite() || !d.is_finite() || d.len() < 1e-6 {
        return None;
    }
    let lo = [0.0, -1.0, 0.0];
    let hi = [SX as f32, SY as f32, SZ as f32];
    let (mut t0, mut t1) = (0.0f32, f32::INFINITY);
    let mut entry = [0i32; 3];
    for i in 0..3 {
        let (oi, di) = (o.axis(i), d.axis(i));
        if di.abs() < 1e-9 {
            if oi < lo[i] || oi > hi[i] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / di;
        let (mut ta, mut tb) = ((lo[i] - oi) * inv, (hi[i] - oi) * inv);
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        if ta > t0 {
            t0 = ta;
            entry = [0; 3];
            entry[i] = if di > 0.0 { -1 } else { 1 };
        }
        t1 = t1.min(tb);
        if t0 > t1 {
            return None;
        }
    }
    let start = o + d * (t0 + 1e-4);
    let mut cell = [0i32; 3];
    for i in 0..3 {
        cell[i] = (start.axis(i).floor() as i32).clamp(lo[i] as i32, hi[i] as i32 - 1);
    }
    let mut normal = if t0 > 0.0 { entry } else { [0; 3] };
    // Si la cámara está dentro de bloques, se ignoran los que la envuelven.
    let mut wrapped = t0 <= 0.0 && w.solid(cell);
    let mut step = [0i32; 3];
    let mut tmax = [f32::INFINITY; 3];
    let mut tdelta = [f32::INFINITY; 3];
    for i in 0..3 {
        let di = d.axis(i);
        if di > 0.0 {
            step[i] = 1;
            tmax[i] = ((cell[i] + 1) as f32 - o.axis(i)) / di;
            tdelta[i] = 1.0 / di;
        } else if di < 0.0 {
            step[i] = -1;
            tmax[i] = (cell[i] as f32 - o.axis(i)) / di;
            tdelta[i] = -1.0 / di;
        }
    }
    let mut t = t0.max(0.0);
    // Cota: nunca más pasos que celdas en la diagonal de la caja.
    for _ in 0..(SX + SY + SZ + 4) {
        if w.solid(cell) {
            if !wrapped {
                return Some(Hit { cell, normal, t });
            }
        } else {
            wrapped = false;
        }
        let a = if tmax[0] < tmax[1] {
            if tmax[0] < tmax[2] {
                0
            } else {
                2
            }
        } else if tmax[1] < tmax[2] {
            1
        } else {
            2
        };
        if tmax[a] > t1 {
            return None;
        }
        t = tmax[a];
        cell[a] += step[a];
        tmax[a] += tdelta[a];
        normal = [0; 3];
        normal[a] = -step[a];
    }
    let _ = t;
    None
}

// ---------------------------------------------------------------------------
// Física
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Faller {
    pub x: i32,
    pub z: i32,
    /// Altura de la base (continua).
    pub y: f32,
    pub vy: f32,
    pub kind: u8,
    pub age: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Placed([i32; 3], u8),
    Removed([i32; 3], u8),
    Wobble([i32; 3]),
    Saved([i32; 3]),
    Fell([i32; 3], u8),
    Landed([i32; 3], u8, f32),
    Full,
}

#[derive(Clone)]
struct Snapshot {
    cells: Box<[u8]>,
    fallers: Vec<Faller>,
    gravity: bool,
}

pub struct Sim {
    pub world: World,
    pub gravity: bool,
    pub fallers: Vec<Faller>,
    /// (índice de celda, segundos que le quedan tambaleando)
    pub wobble: Vec<(u32, f32)>,
    pub support: Box<[u8]>,
    dirty: bool,
    acc: f32,
    undo: VecDeque<Snapshot>,
    stroke_open: bool,
    pub events: Vec<Event>,
    /// Sube cada vez que cambia algo visible (para rehacer la lista de cubos).
    pub version: u64,
}

impl Default for Sim {
    fn default() -> Self {
        Sim {
            world: World::default(),
            gravity: false,
            fallers: Vec::new(),
            wobble: Vec::new(),
            support: vec![NONE; CELLS].into_boxed_slice(),
            dirty: true,
            acc: 0.0,
            undo: VecDeque::new(),
            stroke_open: false,
            events: Vec::new(),
            version: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceResult {
    Ok,
    Full,
    Occupied,
}

impl Sim {
    /// Bloques en total (quietos + cayendo).
    pub fn count(&self) -> u32 {
        self.world.count() + self.fallers.len() as u32
    }

    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    fn snapshot(&mut self) {
        if self.stroke_open {
            return;
        }
        self.undo.push_back(Snapshot {
            cells: self.world.cells.clone(),
            fallers: self.fallers.clone(),
            gravity: self.gravity,
        });
        while self.undo.len() > MAX_UNDO {
            self.undo.pop_front();
        }
    }

    /// Un "trazo" (arrastrar poniendo bloques) se deshace de una vez.
    pub fn begin_stroke(&mut self) {
        self.stroke_open = false;
    }
    pub fn end_stroke(&mut self) {
        self.stroke_open = false;
    }

    fn faller_overlaps(&self, p: [i32; 3]) -> bool {
        self.fallers.iter().any(|f| {
            f.x == p[0] && f.z == p[2] && (p[1] as f32) < f.y + 1.0 && (p[1] + 1) as f32 > f.y
        })
    }

    pub fn place(&mut self, p: [i32; 3], kind: u8, in_stroke: bool) -> PlaceResult {
        if !in_bounds(p) || kind == AIR || self.world.get(p) != AIR || self.faller_overlaps(p) {
            return PlaceResult::Occupied;
        }
        if self.count() >= MAX_BLOCKS {
            self.events.push(Event::Full);
            return PlaceResult::Full;
        }
        if !in_stroke {
            self.stroke_open = false;
        }
        self.snapshot();
        self.stroke_open = in_stroke;
        self.world.cells[idx(p)] = kind;
        self.dirty = true;
        self.version += 1;
        self.events.push(Event::Placed(p, kind));
        PlaceResult::Ok
    }

    pub fn remove(&mut self, p: [i32; 3]) -> Option<u8> {
        let k = self.world.get(p);
        if k == AIR {
            return None;
        }
        self.stroke_open = false;
        self.snapshot();
        self.world.cells[idx(p)] = AIR;
        self.dirty = true;
        self.version += 1;
        self.events.push(Event::Removed(p, k));
        Some(k)
    }

    pub fn clear(&mut self) -> u32 {
        let n = self.count();
        if n == 0 {
            return 0;
        }
        self.stroke_open = false;
        self.snapshot();
        self.world.cells.fill(AIR);
        self.fallers.clear();
        self.wobble.clear();
        self.dirty = true;
        self.version += 1;
        n
    }

    pub fn undo(&mut self) -> bool {
        self.stroke_open = false;
        let Some(s) = self.undo.pop_back() else {
            return false;
        };
        self.world.cells = s.cells;
        self.fallers = s.fallers;
        self.gravity = s.gravity;
        self.wobble.clear();
        self.dirty = true;
        self.version += 1;
        true
    }

    pub fn set_gravity(&mut self, on: bool) {
        if on == self.gravity {
            return;
        }
        self.stroke_open = false;
        self.snapshot();
        self.gravity = on;
        self.dirty = true;
        self.version += 1;
        if !on {
            self.wobble.clear();
            // Lo que venía cayendo se queda quieto en la celda más cercana.
            for f in std::mem::take(&mut self.fallers) {
                let y0 = (f.y.round() as i32).clamp(0, SY - 1);
                let free = (y0..SY)
                    .chain((0..y0).rev())
                    .find(|&y| self.world.get([f.x, y, f.z]) == AIR);
                if let Some(y) = free {
                    self.world.cells[idx([f.x, y, f.z])] = f.kind;
                }
            }
        }
    }

    /// Recalcula los apoyos con BFS 0-1: subir cuesta 0, ir de lado cuesta 1.
    pub fn compute_support(&mut self) {
        let cells = &self.world.cells;
        let sup = &mut self.support;
        sup.fill(NONE);
        let mut dq: VecDeque<u32> = VecDeque::new();
        for (i, &k) in cells.iter().enumerate() {
            if k != AIR && (i < (SX * SZ) as usize || fall_of(k) == Fall::Float) {
                sup[i] = 0;
                dq.push_back(i as u32);
            }
        }
        while let Some(i) = dq.pop_front() {
            let i = i as usize;
            let s = sup[i];
            let p = pos(i);
            if p[1] + 1 < SY {
                let j = idx([p[0], p[1] + 1, p[2]]);
                if cells[j] != AIR && sup[j] > s {
                    sup[j] = s;
                    dq.push_front(j as u32);
                }
            }
            if s < REACH {
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let q = [p[0] + dx, p[1], p[2] + dz];
                    if !in_bounds(q) {
                        continue;
                    }
                    let j = idx(q);
                    if cells[j] != AIR && fall_of(cells[j]) != Fall::Sand && sup[j] > s + 1 {
                        sup[j] = s + 1;
                        dq.push_back(j as u32);
                    }
                }
            }
        }
    }

    pub fn unstable(&self) -> Vec<usize> {
        (0..CELLS)
            .filter(|&i| self.world.cells[i] != AIR && self.support[i] == NONE)
            .collect()
    }

    fn refresh(&mut self) {
        self.dirty = false;
        self.version += 1;
        if !self.gravity {
            return;
        }
        self.compute_support();
        let bad = self.unstable();
        // Los que ya no necesitan caer se salvan (alguien les puso apoyo).
        let mut kept = Vec::with_capacity(bad.len());
        for &(i, t) in &self.wobble {
            if bad.binary_search(&(i as usize)).is_ok() {
                kept.push((i, t));
            } else if self.world.cells[i as usize] != AIR {
                self.events.push(Event::Saved(pos(i as usize)));
            }
        }
        for i in bad {
            if !kept.iter().any(|&(j, _)| j as usize == i) {
                kept.push((i as u32, WOBBLE));
                self.events.push(Event::Wobble(pos(i)));
            }
        }
        self.wobble = kept;
    }

    /// Avanza la simulación `dt` segundos (pasos fijos, con tope).
    pub fn update(&mut self, dt: f32) {
        self.acc = (self.acc + dt.clamp(0.0, 0.1)).min(STEP * MAX_STEPS_PER_FRAME as f32);
        while self.acc >= STEP {
            self.acc -= STEP;
            self.step(STEP);
        }
    }

    pub fn step(&mut self, dt: f32) {
        if self.dirty {
            self.refresh();
        }
        if !self.gravity {
            return;
        }
        // 1) Tambaleo -> se suelta.
        let mut detached = false;
        let mut i = 0;
        while i < self.wobble.len() {
            self.wobble[i].1 -= dt;
            if self.wobble[i].1 <= 0.0 {
                let (c, _) = self.wobble.swap_remove(i);
                let p = pos(c as usize);
                let kind = self.world.cells[c as usize];
                if kind != AIR {
                    self.world.cells[c as usize] = AIR;
                    self.fallers.push(Faller {
                        x: p[0],
                        z: p[2],
                        y: p[1] as f32,
                        vy: 0.0,
                        kind,
                        age: 0.0,
                    });
                    self.events.push(Event::Fell(p, kind));
                    detached = true;
                }
            } else {
                i += 1;
            }
        }
        if detached {
            self.dirty = true;
            self.version += 1;
        }
        // 2) Caída por columnas, de abajo hacia arriba.
        self.fallers
            .sort_by(|a, b| (a.x, a.z).cmp(&(b.x, b.z)).then(a.y.total_cmp(&b.y)));
        let mut prev: Option<(i32, i32, f32, f32)> = None;
        let mut k = 0;
        while k < self.fallers.len() {
            let mut f = self.fallers[k];
            f.age += dt;
            f.vy = (f.vy + GRAVITY * dt).min(VMAX);
            let ny = f.y - f.vy * dt;
            let ground = self.world.top_below(f.x, f.z, (f.y + 1e-3).floor() as i32) + 1;
            if ny <= ground as f32 || f.age > 5.0 {
                let c = [f.x, ground, f.z];
                self.world.cells[idx(c)] = f.kind;
                self.events.push(Event::Landed(c, f.kind, f.vy));
                self.fallers.remove(k);
                self.dirty = true;
                self.version += 1;
                prev = None;
                continue;
            }
            f.y = ny;
            if let Some((px, pz, py, pv)) = prev {
                if px == f.x && pz == f.z && f.y < py + 1.0 {
                    f.y = py + 1.0;
                    f.vy = pv;
                }
            }
            prev = Some((f.x, f.z, f.y, f.vy));
            self.fallers[k] = f;
            k += 1;
        }
    }

    pub fn settled(&self) -> bool {
        self.fallers.is_empty() && self.wobble.is_empty() && !self.dirty
    }
}

pub fn center(p: [i32; 3]) -> V3 {
    v3(p[0] as f32 + 0.5, p[1] as f32 + 0.5, p[2] as f32 + 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx3d::Camera;

    const RED: u8 = 1;
    const BLUE: u8 = 6;
    const SAND: u8 = 20;
    const BALLOON: u8 = 21;

    fn run(s: &mut Sim, secs: f32) {
        for _ in 0..(secs * 60.0) as usize {
            s.update(1.0 / 60.0);
        }
    }

    fn tower(s: &mut Sim, x: i32, z: i32, h: i32) {
        for y in 0..h {
            assert_eq!(s.place([x, y, z], RED, false), PlaceResult::Ok);
        }
    }

    // --- Rayo -------------------------------------------------------------

    #[test]
    fn click_on_grass_places_on_floor() {
        let w = World::default();
        let h = raycast(
            &w,
            v3(12.5, 20.0, 30.0),
            (v3(12.5, 0.0, 12.5) - v3(12.5, 20.0, 30.0)).norm(),
        )
        .unwrap();
        assert_eq!(h.cell, [12, -1, 12]);
        assert_eq!(h.normal, [0, 1, 0]);
        assert_eq!(place_target(&h), Target::Place([12, 0, 12]));
    }

    #[test]
    fn top_and_side_faces() {
        let mut w = World::default();
        w.cells[idx([12, 0, 12])] = RED;
        let down = raycast(&w, v3(12.5, 10.0, 12.5), v3(0.0, -1.0, 0.0)).unwrap();
        assert_eq!(place_target(&down), Target::Place([12, 1, 12]));
        let side = raycast(&w, v3(12.5, 0.5, 40.0), v3(0.0, 0.0, -1.0)).unwrap();
        assert_eq!((side.cell, side.normal), ([12, 0, 12], [0, 0, 1]));
        assert_eq!(place_target(&side), Target::Place([12, 0, 13]));
        let west = raycast(&w, v3(-30.0, 0.5, 12.5), v3(1.0, 0.0, 0.0)).unwrap();
        assert_eq!(place_target(&west), Target::Place([11, 0, 12]));
    }

    #[test]
    fn sky_and_outside_miss() {
        let w = World::default();
        assert!(raycast(&w, v3(12.0, 20.0, 30.0), v3(0.0, 1.0, 0.0)).is_none());
        assert!(raycast(&w, v3(-5.0, 5.0, -5.0), v3(-1.0, 0.0, 0.0)).is_none());
        assert!(raycast(&w, v3(f32::NAN, 0.0, 0.0), v3(1.0, 0.0, 0.0)).is_none());
        assert!(raycast(&w, v3(1.0, 1.0, 1.0), V3::ZERO).is_none());
    }

    #[test]
    fn above_the_ceiling_and_off_the_edge() {
        let mut w = World::default();
        for y in 0..SY {
            w.cells[idx([3, y, 3])] = RED;
        }
        let h = raycast(&w, v3(3.5, 40.0, 3.5), v3(0.0, -1.0, 0.0)).unwrap();
        assert_eq!(h.cell, [3, SY - 1, 3]);
        assert_eq!(place_target(&h), Target::TooHigh([3, SY, 3]));
        w.cells[idx([SX - 1, 0, 5])] = RED;
        let h = raycast(
            &w,
            v3(SX as f32 - 0.5, 0.5, 40.0),
            (v3(SX as f32 - 0.5, 0.5, 5.5) - v3(SX as f32 + 30.0, 0.5, 5.5)).norm() * -1.0,
        );
        // Rayo desde +X hacia la cara +X del borde -> la celda sería x = SX.
        let h2 = raycast(&w, v3(SX as f32 + 30.0, 0.5, 5.5), v3(-1.0, 0.0, 0.0)).unwrap();
        assert_eq!(h2.normal, [1, 0, 0]);
        assert_eq!(place_target(&h2), Target::Outside);
        let _ = h;
    }

    #[test]
    fn camera_inside_blocks_ignores_wrapping_cells() {
        let mut w = World::default();
        for x in 4..8 {
            w.cells[idx([x, 2, 4])] = RED;
        }
        w.cells[idx([10, 2, 4])] = BLUE;
        let h = raycast(&w, v3(4.5, 2.5, 4.5), v3(1.0, 0.0, 0.0)).unwrap();
        assert_eq!(h.cell, [10, 2, 4]);
        assert_eq!(h.normal, [-1, 0, 0]);
    }

    #[test]
    fn far_cameras_and_grazing_rays_are_bounded_and_finite() {
        let w = World::default();
        let mut hits = 0;
        for i in 0..2000 {
            let a = i as f32 * 0.137;
            let o = v3(
                12.0 + a.cos() * 200.0,
                3.0 + (i % 50) as f32,
                12.0 + a.sin() * 200.0,
            );
            let target = v3((i * 7 % 24) as f32 + 0.3, 0.0, (i * 13 % 24) as f32 + 0.7);
            if let Some(h) = raycast(&w, o, (target - o).norm()) {
                assert!(h.t.is_finite());
                hits += 1;
            }
        }
        assert!(hits > 1900);
        // Rayos exactamente sobre las líneas de la grilla.
        let h = raycast(&w, v3(5.0, 3.0, 5.0), v3(0.0, -1.0, 0.0)).unwrap();
        assert_eq!(h.normal, [0, 1, 0]);
    }

    #[test]
    fn mouse_pick_through_orbit_camera() {
        let mut w = World::default();
        w.cells[idx([12, 0, 12])] = RED;
        let cam = Camera {
            target: v3(12.5, 0.5, 12.5),
            yaw: 0.7,
            pitch: 0.8,
            dist: CAM_DIST,
            fovy: 0.6,
        };
        let (o, d) = cam.ray(640.0, 360.0, 1280.0, 720.0);
        let h = raycast(&w, o, d).unwrap();
        assert_eq!(h.cell, [12, 0, 12]);
        assert_eq!(place_target(&h), Target::Place([12, 1, 12]));
    }

    /// Con dist = 30 y pitch en [PITCH_MIN, PITCH_MAX] el ojo queda fuera
    /// de la caja del mundo, para cualquier giro y altura del objetivo.
    #[test]
    fn eye_never_inside_world() {
        for ty in [0.5f32, 2.0, 4.0, 6.0] {
            for i in 0..360 {
                for j in 0..=20 {
                    let pitch = PITCH_MIN + (PITCH_MAX - PITCH_MIN) * j as f32 / 20.0;
                    let cam = Camera {
                        target: v3(12.0, ty, 12.0),
                        yaw: i as f32 / 360.0 * 6.2832,
                        pitch,
                        dist: CAM_DIST,
                        fovy: 0.6,
                    };
                    let e = cam.eye();
                    let inside = (0.0..=SX as f32).contains(&e.x)
                        && (-1.0..=SY as f32).contains(&e.y)
                        && (0.0..=SZ as f32).contains(&e.z);
                    assert!(!inside, "{e:?}");
                }
            }
        }
    }

    // --- Física -------------------------------------------------------------

    #[test]
    fn towers_stand() {
        let mut s = Sim {
            gravity: true,
            ..Default::default()
        };
        tower(&mut s, 3, 3, SY);
        run(&mut s, 2.0);
        assert_eq!(s.world.count(), SY as u32);
        assert!(s.settled());
    }

    #[test]
    fn removing_the_base_drops_the_tower_one_step() {
        let mut s = Sim {
            gravity: true,
            ..Default::default()
        };
        tower(&mut s, 3, 3, 5);
        s.remove([3, 0, 3]);
        run(&mut s, 2.0);
        assert!(s.settled());
        assert_eq!(s.world.count(), 4);
        for y in 0..4 {
            assert_eq!(s.world.get([3, y, 3]), RED);
        }
        assert_eq!(s.world.get([3, 4, 3]), AIR);
    }

    #[test]
    fn overhang_up_to_reach_then_falls() {
        let mut s = Sim {
            gravity: true,
            ..Default::default()
        };
        tower(&mut s, 5, 5, 3);
        for x in 6..6 + REACH as i32 {
            s.place([x, 2, 5], BLUE, false);
        }
        run(&mut s, 1.0);
        assert_eq!(s.world.count(), 3 + REACH as u32);
        let tip = [6 + REACH as i32, 2, 5];
        s.place(tip, BLUE, false);
        run(&mut s, 2.0);
        assert_eq!(s.world.get(tip), AIR);
        assert_eq!(
            s.world.get([tip[0], 0, 5]),
            BLUE,
            "cayó al suelo, justo abajo"
        );
        assert_eq!(s.count(), 4 + REACH as u32);
    }

    #[test]
    fn support_during_wobble_saves_the_block() {
        let mut s = Sim {
            gravity: true,
            ..Default::default()
        };
        s.place([8, 0, 8], RED, false);
        s.place([8, 1, 8], RED, false);
        s.place([9, 1, 8], BLUE, false);
        s.place([10, 1, 8], BLUE, false);
        s.place([11, 1, 8], BLUE, false);
        s.place([12, 1, 8], BLUE, false); // 4 de lado: tambalea
        run(&mut s, WOBBLE * 0.5);
        assert!(!s.wobble.is_empty());
        s.place([12, 0, 8], RED, false); // apoyo justo a tiempo
        run(&mut s, 1.0);
        assert!(s.events.iter().any(|e| matches!(e, Event::Saved(_))));
        assert_eq!(s.world.get([12, 1, 8]), BLUE);
        assert!(s.settled());
    }

    #[test]
    fn sand_only_rests_on_something_and_balloons_float() {
        let mut s = Sim {
            gravity: true,
            ..Default::default()
        };
        tower(&mut s, 2, 2, 3);
        s.place([3, 2, 2], SAND, false);
        s.place([10, 8, 10], BALLOON, false);
        s.place([11, 8, 10], RED, false);
        run(&mut s, 2.0);
        assert_eq!(s.world.get([3, 0, 2]), SAND);
        assert_eq!(s.world.get([10, 8, 10]), BALLOON);
        assert_eq!(s.world.get([11, 8, 10]), RED, "colgado del globo");
    }

    #[test]
    fn same_column_fallers_never_overlap_and_count_is_conserved() {
        let mut s = Sim {
            gravity: false,
            ..Default::default()
        };
        for y in [3, 4, 9, 10, 15] {
            s.place([7, y, 7], RED, false);
        }
        s.set_gravity(true);
        let n = s.count();
        for _ in 0..240 {
            s.update(1.0 / 60.0);
            assert_eq!(s.count(), n);
            let mut ys: Vec<f32> = s
                .fallers
                .iter()
                .filter(|f| f.x == 7 && f.z == 7)
                .map(|f| f.y)
                .collect();
            ys.sort_by(f32::total_cmp);
            for w in ys.windows(2) {
                assert!(w[1] >= w[0] + 1.0 - 1e-3, "{ys:?}");
            }
            for f in &s.fallers {
                let base = f.y.floor() as i32;
                assert_eq!(s.world.get([f.x, base, f.z]), AIR);
            }
        }
        assert!(s.settled());
        for y in 0..5 {
            assert_eq!(s.world.get([7, y, 7]), RED);
        }
    }

    #[test]
    fn stability_after_settling_is_final() {
        let mut s = Sim {
            gravity: true,
            ..Default::default()
        };
        let mut r = 12345u32;
        let mut rnd = |n: i32| {
            r ^= r << 13;
            r ^= r >> 17;
            r ^= r << 5;
            (r % n as u32) as i32
        };
        s.gravity = false;
        for _ in 0..800 {
            let p = [rnd(SX), rnd(SY), rnd(SZ)];
            s.place(p, 1 + rnd(22) as u8, false);
        }
        let n = s.count();
        s.set_gravity(true);
        run(&mut s, 6.0);
        assert!(
            s.settled(),
            "fallers {} wobble {}",
            s.fallers.len(),
            s.wobble.len()
        );
        assert_eq!(s.count(), n, "caer nunca crea ni destruye bloques");
        s.compute_support();
        assert!(s.unstable().is_empty());
    }

    #[test]
    fn placing_into_a_falling_block_is_refused() {
        let mut s = Sim {
            gravity: true,
            ..Default::default()
        };
        tower(&mut s, 4, 4, 3);
        s.fallers.push(Faller {
            x: 4,
            z: 4,
            y: 9.4,
            vy: 0.0,
            kind: BLUE,
            age: 0.0,
        });
        assert_eq!(s.place([4, 9, 4], RED, false), PlaceResult::Occupied);
        assert_eq!(s.place([4, 10, 4], RED, false), PlaceResult::Occupied);
        s.place([4, 3, 4], RED, false); // la niña agranda la torre bajo el que cae
        run(&mut s, 2.0);
        assert!(s.settled());
        assert_eq!(s.world.get([4, 3, 4]), RED);
        assert_eq!(s.world.get([4, 4, 4]), BLUE, "aterriza encima de lo nuevo");
    }

    #[test]
    fn gravity_off_freezes_fallers_on_the_grid() {
        let mut s = Sim {
            gravity: true,
            ..Default::default()
        };
        s.place([6, 9, 6], RED, false);
        run(&mut s, WOBBLE + 0.2);
        assert_eq!(s.fallers.len(), 1);
        let n = s.count();
        s.set_gravity(false);
        assert!(s.fallers.is_empty());
        assert_eq!(s.count(), n);
        run(&mut s, 1.0);
        assert_eq!(s.count(), n);
    }

    // --- Deshacer y límites ---------------------------------------------------

    #[test]
    fn undo_restores_exactly() {
        let mut s = Sim::default();
        assert!(!s.undo());
        s.place([1, 0, 1], RED, false);
        let after_one = s.world.cells.clone();
        s.place([1, 1, 1], BLUE, false);
        s.remove([1, 0, 1]);
        assert!(s.undo());
        assert!(s.undo());
        assert_eq!(s.world.cells, after_one);
        assert!(s.undo());
        assert_eq!(s.count(), 0);
        assert!(!s.undo());
    }

    #[test]
    fn undo_after_collapse_brings_back_the_tower_and_the_gravity() {
        let mut s = Sim::default();
        tower(&mut s, 3, 3, 6);
        s.place([4, 5, 3], BLUE, false);
        for x in 5..9 {
            s.place([x, 5, 3], BLUE, false);
        }
        let before = s.world.cells.clone();
        s.set_gravity(true);
        run(&mut s, 2.0);
        assert_ne!(s.world.cells, before);
        assert!(s.undo());
        assert!(!s.gravity);
        assert_eq!(s.world.cells, before);
    }

    #[test]
    fn strokes_undo_at_once_and_failed_actions_leave_no_snapshot() {
        let mut s = Sim::default();
        s.begin_stroke();
        for x in 0..6 {
            s.place([x, 0, 0], RED, true);
        }
        s.end_stroke();
        assert_eq!(s.undo_depth(), 1);
        s.place([0, 0, 0], RED, false); // ocupado
        assert!(s.remove([20, 0, 20]).is_none()); // vacío
        assert_eq!(s.undo_depth(), 1);
        s.undo();
        assert_eq!(s.count(), 0);
    }

    #[test]
    fn undo_and_blocks_are_capped() {
        let mut s = Sim::default();
        'outer: for y in 0..SY {
            for z in 0..SZ {
                for x in 0..SX {
                    if s.place([x, y, z], RED, false) == PlaceResult::Full {
                        break 'outer;
                    }
                }
            }
        }
        assert_eq!(s.count(), MAX_BLOCKS);
        assert!(s.events.iter().any(|e| *e == Event::Full));
        assert_eq!(s.undo_depth(), MAX_UNDO);
        s.clear();
        assert_eq!(s.count(), 0);
        s.undo();
        assert_eq!(s.count(), MAX_BLOCKS);
    }

    #[test]
    fn worst_case_collapse_is_cheap() {
        // Mundo casi lleno flotando: todo cae a la vez.
        let mut s = Sim::default();
        for i in 0..MAX_BLOCKS as usize {
            let p = pos(CELLS - 1 - i);
            s.place(p, RED, false);
        }
        s.set_gravity(true);
        let t = std::time::Instant::now();
        let mut frames = 0;
        while !s.settled() && frames < 600 {
            s.update(0.05);
            frames += 1;
        }
        assert!(s.settled());
        assert_eq!(s.count(), MAX_BLOCKS);
        eprintln!("derrumbe total: {frames} cuadros, {:?}", t.elapsed());
    }
}
