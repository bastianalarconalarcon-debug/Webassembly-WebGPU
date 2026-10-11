//! Dibujo 3D con bloques. Igual que `gfx.rs`, Rust no habla con la GPU:
//! cada cuadro llena una lista de cubos (40 bytes cada uno) y un bloque de
//! uniformes (288 bytes) que JavaScript copia tal cual a WebGPU
//! (`web/voxels.js`, `web/cubos.wgsl`).
//!
//! Convenciones (deben coincidir con cubos.wgsl):
//! - Mundo con Y hacia arriba, mano derecha. 1 unidad = 1 bloque.
//! - El bloque de la celda (x, y, z) ocupa [x, x+1] × [y, y+1] × [z, z+1].
//! - Matrices por columnas (como `mat4x4f`), profundidad de recorte 0..1.

use std::ops::{Add, Mul, Neg, Sub};

use crate::math::Rgba;

// ---------------------------------------------------------------------------
// Vectores y matrices
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    pub const ZERO: V3 = v3(0.0, 0.0, 0.0);
    pub const UP: V3 = v3(0.0, 1.0, 0.0);
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn norm(self) -> V3 {
        let l = self.len();
        if l > 1e-6 {
            self * (1.0 / l)
        } else {
            V3::ZERO
        }
    }
    pub fn lerp(self, o: V3, t: f32) -> V3 {
        self + (o - self) * t
    }
    pub fn arr4(self, w: f32) -> [f32; 4] {
        [self.x, self.y, self.z, w]
    }
    /// Componente i (0 = x, 1 = y, 2 = z).
    pub fn axis(self, i: usize) -> f32 {
        match i {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}
impl std::ops::AddAssign for V3 {
    fn add_assign(&mut self, o: V3) {
        *self = *self + o;
    }
}
impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for V3 {
    type Output = V3;
    fn mul(self, k: f32) -> V3 {
        v3(self.x * k, self.y * k, self.z * k)
    }
}
impl Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        v3(-self.x, -self.y, -self.z)
    }
}

/// Matriz 4×4 guardada por columnas: `m[col * 4 + fila]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4(pub [f32; 16]);

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4([
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);

    fn at(&self, row: usize, col: usize) -> f32 {
        self.0[col * 4 + row]
    }

    /// `self * o` (primero se aplica `o`).
    pub fn mul(&self, o: &Mat4) -> Mat4 {
        let mut r = [0.0; 16];
        for c in 0..4 {
            for row in 0..4 {
                r[c * 4 + row] = (0..4).map(|k| self.at(row, k) * o.at(k, c)).sum();
            }
        }
        Mat4(r)
    }

    pub fn transform(&self, p: [f32; 4]) -> [f32; 4] {
        let mut r = [0.0; 4];
        for (row, out) in r.iter_mut().enumerate() {
            *out = (0..4).map(|k| self.at(row, k) * p[k]).sum();
        }
        r
    }

    /// Perspectiva de mano derecha con profundidad 0..1 (WebGPU).
    pub fn perspective(fovy: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fovy * 0.5).tan();
        let nf = 1.0 / (near - far);
        Mat4([
            f / aspect,
            0.0,
            0.0,
            0.0, //
            0.0,
            f,
            0.0,
            0.0, //
            0.0,
            0.0,
            far * nf,
            -1.0, //
            0.0,
            0.0,
            near * far * nf,
            0.0,
        ])
    }

    /// Ortográfica de mano derecha con profundidad 0..1 (para la sombra).
    pub fn ortho(l: f32, r: f32, b: f32, t: f32, near: f32, far: f32) -> Mat4 {
        Mat4([
            2.0 / (r - l),
            0.0,
            0.0,
            0.0, //
            0.0,
            2.0 / (t - b),
            0.0,
            0.0, //
            0.0,
            0.0,
            1.0 / (near - far),
            0.0, //
            -(r + l) / (r - l),
            -(t + b) / (t - b),
            near / (near - far),
            1.0,
        ])
    }

    pub fn look_at(eye: V3, target: V3, up: V3) -> Mat4 {
        let f = (target - eye).norm();
        let s = f.cross(up).norm();
        let u = s.cross(f);
        Mat4([
            s.x,
            u.x,
            -f.x,
            0.0, //
            s.y,
            u.y,
            -f.y,
            0.0, //
            s.z,
            u.z,
            -f.z,
            0.0, //
            -s.dot(eye),
            -u.dot(eye),
            f.dot(eye),
            1.0,
        ])
    }

    /// Inversa general (cofactores). Devuelve la identidad si no existe.
    pub fn inverse(&self) -> Mat4 {
        let m = &self.0;
        let mut inv = [0.0f32; 16];
        inv[0] = m[5] * m[10] * m[15] - m[5] * m[11] * m[14] - m[9] * m[6] * m[15]
            + m[9] * m[7] * m[14]
            + m[13] * m[6] * m[11]
            - m[13] * m[7] * m[10];
        inv[4] = -m[4] * m[10] * m[15] + m[4] * m[11] * m[14] + m[8] * m[6] * m[15]
            - m[8] * m[7] * m[14]
            - m[12] * m[6] * m[11]
            + m[12] * m[7] * m[10];
        inv[8] = m[4] * m[9] * m[15] - m[4] * m[11] * m[13] - m[8] * m[5] * m[15]
            + m[8] * m[7] * m[13]
            + m[12] * m[5] * m[11]
            - m[12] * m[7] * m[9];
        inv[12] = -m[4] * m[9] * m[14] + m[4] * m[10] * m[13] + m[8] * m[5] * m[14]
            - m[8] * m[6] * m[13]
            - m[12] * m[5] * m[10]
            + m[12] * m[6] * m[9];
        inv[1] = -m[1] * m[10] * m[15] + m[1] * m[11] * m[14] + m[9] * m[2] * m[15]
            - m[9] * m[3] * m[14]
            - m[13] * m[2] * m[11]
            + m[13] * m[3] * m[10];
        inv[5] = m[0] * m[10] * m[15] - m[0] * m[11] * m[14] - m[8] * m[2] * m[15]
            + m[8] * m[3] * m[14]
            + m[12] * m[2] * m[11]
            - m[12] * m[3] * m[10];
        inv[9] = -m[0] * m[9] * m[15] + m[0] * m[11] * m[13] + m[8] * m[1] * m[15]
            - m[8] * m[3] * m[13]
            - m[12] * m[1] * m[11]
            + m[12] * m[3] * m[9];
        inv[13] = m[0] * m[9] * m[14] - m[0] * m[10] * m[13] - m[8] * m[1] * m[14]
            + m[8] * m[2] * m[13]
            + m[12] * m[1] * m[10]
            - m[12] * m[2] * m[9];
        inv[2] = m[1] * m[6] * m[15] - m[1] * m[7] * m[14] - m[5] * m[2] * m[15]
            + m[5] * m[3] * m[14]
            + m[13] * m[2] * m[7]
            - m[13] * m[3] * m[6];
        inv[6] = -m[0] * m[6] * m[15] + m[0] * m[7] * m[14] + m[4] * m[2] * m[15]
            - m[4] * m[3] * m[14]
            - m[12] * m[2] * m[7]
            + m[12] * m[3] * m[6];
        inv[10] = m[0] * m[5] * m[15] - m[0] * m[7] * m[13] - m[4] * m[1] * m[15]
            + m[4] * m[3] * m[13]
            + m[12] * m[1] * m[7]
            - m[12] * m[3] * m[5];
        inv[14] = -m[0] * m[5] * m[14] + m[0] * m[6] * m[13] + m[4] * m[1] * m[14]
            - m[4] * m[2] * m[13]
            - m[12] * m[1] * m[6]
            + m[12] * m[2] * m[5];
        inv[3] = -m[1] * m[6] * m[11] + m[1] * m[7] * m[10] + m[5] * m[2] * m[11]
            - m[5] * m[3] * m[10]
            - m[9] * m[2] * m[7]
            + m[9] * m[3] * m[6];
        inv[7] = m[0] * m[6] * m[11] - m[0] * m[7] * m[10] - m[4] * m[2] * m[11]
            + m[4] * m[3] * m[10]
            + m[8] * m[2] * m[7]
            - m[8] * m[3] * m[6];
        inv[11] = -m[0] * m[5] * m[11] + m[0] * m[7] * m[9] + m[4] * m[1] * m[11]
            - m[4] * m[3] * m[9]
            - m[8] * m[1] * m[7]
            + m[8] * m[3] * m[5];
        inv[15] = m[0] * m[5] * m[10] - m[0] * m[6] * m[9] - m[4] * m[1] * m[10]
            + m[4] * m[2] * m[9]
            + m[8] * m[1] * m[6]
            - m[8] * m[2] * m[5];
        let det = m[0] * inv[0] + m[1] * inv[4] + m[2] * inv[8] + m[3] * inv[12];
        if det.abs() < 1e-12 {
            return Mat4::IDENTITY;
        }
        let k = 1.0 / det;
        for x in inv.iter_mut() {
            *x *= k;
        }
        Mat4(inv)
    }
}

// ---------------------------------------------------------------------------
// Caras del cubo. ORDEN Y EJES IGUALES A `FACE_*` EN cubos.wgsl.
// Cada cara: normal, esquina base, eje u, eje v (u × v = normal, así los
// triángulos quedan en sentido antihorario vistos desde afuera).
// Esquina k de la cara: 0 = (0,0), 1 = (1,0), 2 = (1,1), 3 = (0,1) en (u,v).
// ---------------------------------------------------------------------------

pub struct Face {
    pub n: [i32; 3],
    pub base: [i32; 3],
    pub u: [i32; 3],
    pub v: [i32; 3],
}

pub const FACES: [Face; 6] = [
    Face {
        n: [1, 0, 0],
        base: [1, 0, 1],
        u: [0, 0, -1],
        v: [0, 1, 0],
    }, // 0: +X
    Face {
        n: [-1, 0, 0],
        base: [0, 0, 0],
        u: [0, 0, 1],
        v: [0, 1, 0],
    }, // 1: -X
    Face {
        n: [0, 1, 0],
        base: [0, 1, 1],
        u: [1, 0, 0],
        v: [0, 0, -1],
    }, // 2: +Y (arriba)
    Face {
        n: [0, -1, 0],
        base: [0, 0, 0],
        u: [1, 0, 0],
        v: [0, 0, 1],
    }, // 3: -Y (abajo)
    Face {
        n: [0, 0, 1],
        base: [0, 0, 1],
        u: [1, 0, 0],
        v: [0, 1, 0],
    }, // 4: +Z
    Face {
        n: [0, 0, -1],
        base: [1, 0, 0],
        u: [-1, 0, 0],
        v: [0, 1, 0],
    }, // 5: -Z
];

/// Materiales. IDs iguales a `M_*` en cubos.wgsl. Los >= VIDRIO son translúcidos.
pub mod mat {
    pub const PASTO: u8 = 0;
    pub const TIERRA: u8 = 1;
    pub const PIEDRA: u8 = 2;
    pub const MADERA: u8 = 3;
    pub const TRONCO: u8 = 4;
    pub const HOJAS: u8 = 5;
    pub const LADRILLO: u8 = 6;
    pub const ARENA: u8 = 7;
    /// Bloque de juguete: el color de la instancia es el color del bloque.
    pub const JUGUETE: u8 = 8;
    pub const ORO: u8 = 9;
    /// Brilla sola (no le afecta la sombra).
    pub const LAMPARA: u8 = 10;
    pub const DIAMANTE: u8 = 11;
    pub const VIDRIO: u8 = 12;
    pub const AGUA: u8 = 13;
    pub fn translucent(m: u8) -> bool {
        m >= VIDRIO
    }
}

/// Resaltado (byte `info[2]`, bits 0..2): 0 nada, 1..=6 la cara (cara + 1), 7 todo el bloque.
pub const HL_ALL: u8 = 7;
/// Bit 3 de `info[2]`: bloque fantasma (vista previa de dónde va el bloque).
pub const FLAG_GHOST: u8 = 1 << 3;

/// Un cubo. Debe coincidir byte a byte con el vertex buffer de `web/voxels.js`
/// (40 bytes): pos f32x3 | escala unorm16x2 | giro snorm16x4 | color unorm8x4 | info uint8x4 | ao uint32x2.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cube {
    /// Centro del bloque, en unidades de mundo (puede ser fraccionario al caer).
    pub pos: [f32; 3],
    /// Escala (horizontal, vertical) codificada 0..65535 = 0..2. Se escala
    /// desde el centro de la base: aparecer creciendo, aplastarse al caer.
    pub scale: [u16; 2],
    /// Giro alrededor del centro: cuaternión (x, y, z, w) × 32767. Sin giro = [0, 0, 0, 32767].
    pub rot: [i16; 4],
    /// Tinte RGBA (multiplica la textura; el alfa sólo cuenta en translúcidos).
    pub color: [u8; 4],
    /// [material, caras visibles (bit i = cara i), resaltado | banderas, semilla]
    pub info: [u8; 4],
    /// Oclusión ambiental: 2 bits por esquina (3 = sin sombra), 24 esquinas:
    /// bits 2*(4*cara + esquina); caras 0-3 en ao[0], caras 4-5 en ao[1].
    pub ao: [u32; 2],
}

pub const CUBE_BYTES: usize = 40;
const NO_ROT: [i16; 4] = [0, 0, 0, 32767];
pub const ALL_FACES: u8 = 0b11_1111;
pub const AO_NONE: [u32; 2] = [u32::MAX, 0xFFFF];

fn unorm8(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

impl Cube {
    /// Bloque en la celda (x, y, z), del material y color dados, sin AO.
    pub fn at_cell(x: i32, y: i32, z: i32, material: u8, tint: Rgba) -> Cube {
        Cube::new(
            v3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5),
            material,
            tint,
        )
        .seed(
            (x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663) ^ z.wrapping_mul(83492791)) as u32
                as u8,
        )
    }

    pub fn new(center: V3, material: u8, tint: Rgba) -> Cube {
        Cube {
            pos: [center.x, center.y, center.z],
            scale: [32768, 32768],
            rot: NO_ROT,
            color: [
                unorm8(tint.r),
                unorm8(tint.g),
                unorm8(tint.b),
                unorm8(tint.a),
            ],
            info: [material, ALL_FACES, 0, 0],
            ao: AO_NONE,
        }
    }

    pub fn scaled(mut self, horizontal: f32, vertical: f32) -> Cube {
        let enc = |s: f32| ((s * 0.5).clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
        self.scale = [enc(horizontal), enc(vertical)];
        self
    }
    /// Gira `angle` radianes alrededor de `axis` (para bloques que se vuelcan
    /// o tambalean). Un bloque girado no debería llevar AO (`AO_NONE`).
    pub fn rotated(mut self, axis: V3, angle: f32) -> Cube {
        let a = axis.norm() * (angle * 0.5).sin();
        let w = (angle * 0.5).cos();
        let q = |x: f32| (x.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        self.rot = [q(a.x), q(a.y), q(a.z), q(w)];
        self
    }
    pub fn faces(mut self, mask: u8) -> Cube {
        self.info[1] = mask & ALL_FACES;
        self
    }
    pub fn highlight(mut self, face_plus_one: u8) -> Cube {
        self.info[2] = (self.info[2] & !7) | (face_plus_one & 7);
        self
    }
    pub fn ghost(mut self) -> Cube {
        self.info[2] |= FLAG_GHOST;
        self
    }
    /// Semilla de la textura: no cambia aunque el bloque se mueva al caer.
    pub fn seed(mut self, s: u8) -> Cube {
        self.info[3] = s;
        self
    }
    pub fn with_ao(mut self, ao: [u32; 2]) -> Cube {
        self.ao = ao;
        self
    }
    pub fn center(&self) -> V3 {
        v3(self.pos[0], self.pos[1], self.pos[2])
    }
}

/// Caras visibles del bloque en `p`: `hides(vecino)` dice si el vecino tapa
/// esa cara (por ejemplo: vecino opaco, o el mismo material translúcido).
pub fn face_mask(p: [i32; 3], hides: impl Fn(i32, i32, i32) -> bool) -> u8 {
    let mut m = 0;
    for (i, f) in FACES.iter().enumerate() {
        if !hides(p[0] + f.n[0], p[1] + f.n[1], p[2] + f.n[2]) {
            m |= 1 << i;
        }
    }
    m
}

/// Oclusión ambiental por esquina, al estilo Minecraft. `solid` debe decir
/// true bajo el suelo (y < 0) para que las paredes se oscurezcan al tocarlo.
pub fn ambient_occlusion(p: [i32; 3], solid: impl Fn(i32, i32, i32) -> bool) -> [u32; 2] {
    let mut bits: u64 = 0;
    for (fi, f) in FACES.iter().enumerate() {
        // Capa de celdas justo afuera de la cara.
        let q = [p[0] + f.n[0], p[1] + f.n[1], p[2] + f.n[2]];
        for (k, (s, t)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate() {
            let du = if s == 0 { -1 } else { 1 };
            let dv = if t == 0 { -1 } else { 1 };
            let at = |a: i32, b: i32| {
                solid(
                    q[0] + f.u[0] * a + f.v[0] * b,
                    q[1] + f.u[1] * a + f.v[1] * b,
                    q[2] + f.u[2] * a + f.v[2] * b,
                ) as u32
            };
            let (s1, s2, c) = (at(du, 0), at(0, dv), at(du, dv));
            let ao = if s1 == 1 && s2 == 1 {
                0
            } else {
                3 - (s1 + s2 + c)
            };
            bits |= (ao as u64) << (2 * (4 * fi + k));
        }
    }
    [bits as u32, (bits >> 32) as u32]
}

// ---------------------------------------------------------------------------
// Cámara
// ---------------------------------------------------------------------------

/// Cámara que orbita alrededor de un punto (lo más simple para los 3 años:
/// girar y acercar, nunca "perderse").
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub target: V3,
    /// Ángulo alrededor del eje Y (radianes).
    pub yaw: f32,
    /// Inclinación hacia abajo (radianes, 0.25..1.25).
    pub pitch: f32,
    pub dist: f32,
    pub fovy: f32,
}

impl Camera {
    pub fn eye(&self) -> V3 {
        let (cp, sp) = (self.pitch.cos(), self.pitch.sin());
        self.target + v3(self.yaw.sin() * cp, sp, self.yaw.cos() * cp) * self.dist
    }
    pub fn view(&self) -> Mat4 {
        Mat4::look_at(self.eye(), self.target, V3::UP)
    }
    pub fn proj(&self, aspect: f32) -> Mat4 {
        Mat4::perspective(self.fovy, aspect, 0.1, 300.0)
    }
    pub fn view_proj(&self, w: f32, h: f32) -> Mat4 {
        self.proj(w / h.max(1.0)).mul(&self.view())
    }

    /// Rayo bajo el mouse (px CSS) -> (origen, dirección normalizada).
    pub fn ray(&self, mx: f32, my: f32, w: f32, h: f32) -> (V3, V3) {
        let inv = self.view_proj(w, h).inverse();
        let nx = mx / w * 2.0 - 1.0;
        let ny = 1.0 - my / h * 2.0;
        let unproject = |z: f32| {
            let p = inv.transform([nx, ny, z, 1.0]);
            v3(p[0] / p[3], p[1] / p[3], p[2] / p[3])
        };
        let a = unproject(0.0);
        let b = unproject(1.0);
        (a, (b - a).norm())
    }

    /// Punto del mundo -> px CSS (para chispitas 2D y etiquetas). None si
    /// queda detrás de la cámara.
    pub fn project(&self, p: V3, w: f32, h: f32) -> Option<(f32, f32)> {
        let c = self.view_proj(w, h).transform(p.arr4(1.0));
        if c[3] <= 1e-4 {
            return None;
        }
        Some(((c[0] / c[3] * 0.5 + 0.5) * w, (0.5 - c[1] / c[3] * 0.5) * h))
    }
}

/// Recorre las celdas que cruza el rayo (Amanatides–Woo). Devuelve la celda
/// sólida tocada y la normal de la cara por donde entró el rayo.
pub fn raycast(
    origin: V3,
    dir: V3,
    max_dist: f32,
    solid: impl Fn(i32, i32, i32) -> bool,
) -> Option<([i32; 3], [i32; 3])> {
    let mut cell = [
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    ];
    let d = [dir.x, dir.y, dir.z];
    let o = [origin.x, origin.y, origin.z];
    let mut step = [0i32; 3];
    let mut t_max = [f32::INFINITY; 3];
    let mut t_delta = [f32::INFINITY; 3];
    for i in 0..3 {
        if d[i] > 0.0 {
            step[i] = 1;
            t_max[i] = ((cell[i] as f32 + 1.0) - o[i]) / d[i];
            t_delta[i] = 1.0 / d[i];
        } else if d[i] < 0.0 {
            step[i] = -1;
            t_max[i] = (o[i] - cell[i] as f32) / -d[i];
            t_delta[i] = -1.0 / d[i];
        }
    }
    let mut normal = [0i32; 3];
    for _ in 0..512 {
        if solid(cell[0], cell[1], cell[2]) {
            return Some((cell, normal));
        }
        let i = if t_max[0] < t_max[1] {
            if t_max[0] < t_max[2] {
                0
            } else {
                2
            }
        } else if t_max[1] < t_max[2] {
            1
        } else {
            2
        };
        if t_max[i] > max_dist {
            return None;
        }
        cell[i] += step[i];
        t_max[i] += t_delta[i];
        normal = [0; 3];
        normal[i] = -step[i];
    }
    None
}

// ---------------------------------------------------------------------------
// Uniformes de la escena (288 bytes = 72 floats). Igual a `Scene` en cubos.wgsl.
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Scene3 {
    pub view_proj: [f32; 16],
    pub inv_view_proj: [f32; 16],
    /// Matriz de la luz del sol (mapa de sombras).
    pub light_vp: [f32; 16],
    /// xyz = posición de la cámara, w = tiempo (s).
    pub eye: [f32; 4],
    /// xyz = dirección HACIA el sol (normalizada), w = intensidad de la sombra (0 = sin sombra).
    pub sun: [f32; 4],
    /// rgb = cielo arriba, w = distancia donde empieza la neblina.
    pub sky_top: [f32; 4],
    /// rgb = horizonte, w = distancia donde la neblina es total.
    pub sky_horizon: [f32; 4],
    /// Reservado: x0, z0 y tamaño del área de construcción (no lo usa el shader todavía).
    pub grid: [f32; 4],
    /// x = pulso del fantasma 0..1 (lo calcula `setup`); y, z, w libres.
    pub misc: [f32; 4],
}

pub const SCENE_FLOATS: usize = 72;

impl Default for Scene3 {
    fn default() -> Self {
        Scene3 {
            view_proj: Mat4::IDENTITY.0,
            inv_view_proj: Mat4::IDENTITY.0,
            light_vp: Mat4::IDENTITY.0,
            eye: [0.0; 4],
            sun: [0.0, 1.0, 0.0, 0.0],
            sky_top: [0.0; 4],
            sky_horizon: [0.0; 4],
            grid: [0.0; 4],
            misc: [0.0; 4],
        }
    }
}

/// Todo lo 3D de un cuadro. Vive dentro de `Gfx` (campo `d3`) y se vacía en
/// `Gfx::begin_frame`.
pub struct Gfx3 {
    /// ¿Dibujar 3D este cuadro? (lo pide el juego activo)
    pub active: bool,
    /// Lista final: [opacos..., translúcidos (de lejos a cerca)..., fantasmas...]
    pub cubes: Vec<Cube>,
    pub n_opaque: u32,
    pub n_trans: u32,
    trans: Vec<Cube>,
    ghosts: Vec<Cube>,
    pub scene: Scene3,
    /// Estado que informa JS: 0 cargando, 1 listo, 2 no disponible.
    pub status: u32,
}

const MAX_CUBES: usize = 32_768;

impl Default for Gfx3 {
    fn default() -> Self {
        Self::new()
    }
}

impl Gfx3 {
    pub fn new() -> Gfx3 {
        Gfx3 {
            active: false,
            cubes: Vec::with_capacity(4096),
            n_opaque: 0,
            n_trans: 0,
            trans: Vec::new(),
            ghosts: Vec::new(),
            scene: Scene3::default(),
            status: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        self.active = false;
        self.cubes.clear();
        self.trans.clear();
        self.ghosts.clear();
        self.n_opaque = 0;
        self.n_trans = 0;
    }

    /// Cámara, cielo y sol del cuadro. `w`, `h` en px CSS.
    #[allow(clippy::too_many_arguments)]
    pub fn setup(
        &mut self,
        cam: &Camera,
        w: f32,
        h: f32,
        time: f32,
        sky_top: Rgba,
        horizon: Rgba,
        area: (i32, i32, i32),
        shadows: bool,
    ) {
        self.active = true;
        let vp = cam.view_proj(w, h);
        let s = &mut self.scene;
        s.view_proj = vp.0;
        s.inv_view_proj = vp.inverse().0;
        let eye = cam.eye();
        s.eye = eye.arr4(time);
        // Sol alto y un poco de lado: sombras cortas, casi debajo de cada
        // bloque (así se ve dónde va a caer uno que viene bajando).
        let sun = v3(-0.35, 1.0, 0.45).norm();
        s.sun = sun.arr4(if shadows { 1.0 } else { 0.0 });
        let (x0, z0, n) = area;
        let half = n as f32 * 0.5;
        let center = v3(x0 as f32 + half, 0.0, z0 as f32 + half);
        let reach = half * 1.6 + 6.0;
        let light_view = Mat4::look_at(center + sun * 60.0, center, v3(0.0, 0.0, -1.0));
        s.light_vp = Mat4::ortho(-reach, reach, -reach, reach, 1.0, 120.0)
            .mul(&light_view)
            .0;
        s.sky_top = [sky_top.r, sky_top.g, sky_top.b, cam.dist + 18.0];
        s.sky_horizon = [horizon.r, horizon.g, horizon.b, cam.dist + 75.0];
        s.grid = [x0 as f32, z0 as f32, n as f32, 0.22];
        s.misc[0] = 0.5 + 0.5 * (time * 5.0).sin();
    }

    pub fn opaque(&mut self, c: Cube) {
        if self.cubes.len() < MAX_CUBES {
            self.cubes.push(c);
        }
    }
    pub fn translucent(&mut self, c: Cube) {
        if self.trans.len() < MAX_CUBES {
            self.trans.push(c);
        }
    }
    pub fn ghost(&mut self, c: Cube) {
        if self.ghosts.len() < 8 {
            self.ghosts.push(c.ghost());
        }
    }
    /// Cubo según su material.
    pub fn cube(&mut self, c: Cube) {
        if mat::translucent(c.info[0]) {
            self.translucent(c)
        } else {
            self.opaque(c)
        }
    }

    /// Arma la lista final. Llamar después de que el juego dibuje
    /// (`Game::update` lo hace solo).
    pub fn finish(&mut self) {
        self.cubes.truncate(MAX_CUBES);
        self.n_opaque = self.cubes.len() as u32;
        let e = v3(self.scene.eye[0], self.scene.eye[1], self.scene.eye[2]);
        let d2 = |c: &Cube| (c.center() - e).dot(c.center() - e);
        self.trans.sort_by(|a, b| d2(b).total_cmp(&d2(a)));
        let room = MAX_CUBES.saturating_sub(self.cubes.len());
        self.n_trans = self.trans.len().min(room) as u32;
        self.cubes.extend(self.trans.drain(..).take(room));
        let room = MAX_CUBES.saturating_sub(self.cubes.len());
        self.cubes.extend(self.ghosts.drain(..).take(room));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::WHITE;

    #[test]
    fn layouts_match_js() {
        assert_eq!(std::mem::size_of::<Cube>(), CUBE_BYTES);
        assert_eq!(std::mem::align_of::<Cube>(), 4);
        assert_eq!(std::mem::size_of::<Scene3>(), SCENE_FLOATS * 4);
        // Desplazamientos que usa web/voxels.js.
        let c = Cube::default();
        let base = &c as *const Cube as usize;
        assert_eq!(&c.scale as *const _ as usize - base, 12);
        assert_eq!(&c.rot as *const _ as usize - base, 16);
        assert_eq!(&c.color as *const _ as usize - base, 24);
        assert_eq!(&c.info as *const _ as usize - base, 28);
        assert_eq!(&c.ao as *const _ as usize - base, 32);
    }

    #[test]
    fn faces_are_counter_clockwise_from_outside() {
        for f in FACES.iter() {
            let u = v3(f.u[0] as f32, f.u[1] as f32, f.u[2] as f32);
            let v = v3(f.v[0] as f32, f.v[1] as f32, f.v[2] as f32);
            let n = v3(f.n[0] as f32, f.n[1] as f32, f.n[2] as f32);
            assert_eq!(u.cross(v), n);
            // La cara queda en el plano correcto del cubo unitario.
            let b = v3(f.base[0] as f32, f.base[1] as f32, f.base[2] as f32);
            let c = b + u * 0.5 + v * 0.5;
            assert_eq!(c - v3(0.5, 0.5, 0.5), n * 0.5);
        }
    }

    #[test]
    fn inverse_roundtrip() {
        let cam = Camera {
            target: v3(0.0, 2.0, 0.0),
            yaw: 0.7,
            pitch: 0.6,
            dist: 20.0,
            fovy: 0.8,
        };
        let m = cam.view_proj(1280.0, 720.0);
        let p = m.mul(&m.inverse());
        for i in 0..16 {
            let want = if i % 5 == 0 { 1.0 } else { 0.0 };
            assert!((p.0[i] - want).abs() < 1e-3, "{i}: {}", p.0[i]);
        }
    }

    #[test]
    fn depth_range_is_zero_to_one() {
        let p = Mat4::perspective(0.8, 1.5, 0.1, 300.0);
        let n = p.transform([0.0, 0.0, -0.1, 1.0]);
        let f = p.transform([0.0, 0.0, -300.0, 1.0]);
        assert!((n[2] / n[3]).abs() < 1e-5);
        assert!((f[2] / f[3] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn center_ray_hits_target_block() {
        let cam = Camera {
            target: v3(0.5, 0.5, 0.5),
            yaw: 0.7,
            pitch: 0.6,
            dist: 20.0,
            fovy: 0.8,
        };
        let (o, d) = cam.ray(640.0, 360.0, 1280.0, 720.0);
        let hit = raycast(o, d, 100.0, |x, y, z| (x, y, z) == (0, 0, 0) || y < 0);
        assert_eq!(hit.map(|h| h.0), Some([0, 0, 0]));
        let (sx, sy) = cam.project(v3(0.5, 0.5, 0.5), 1280.0, 720.0).unwrap();
        assert!((sx - 640.0).abs() < 0.5 && (sy - 360.0).abs() < 0.5);
    }

    #[test]
    fn finish_orders_opaque_translucent_far_to_near_then_ghosts() {
        let mut g = Gfx3::new();
        let cam = Camera {
            target: V3::ZERO,
            yaw: 0.0,
            pitch: 0.5,
            dist: 20.0,
            fovy: 0.8,
        };
        g.setup(&cam, 1280.0, 720.0, 0.0, WHITE, WHITE, (-8, -8, 16), true);
        g.ghost(Cube::at_cell(0, 0, 0, mat::JUGUETE, WHITE));
        g.cube(Cube::at_cell(0, 0, 5, mat::AGUA, WHITE)); // cerca de la cámara (z+)
        g.cube(Cube::at_cell(0, 0, -5, mat::VIDRIO, WHITE)); // lejos
        g.cube(Cube::at_cell(1, 0, 0, mat::PIEDRA, WHITE));
        g.finish();
        assert_eq!((g.cubes.len(), g.n_opaque, g.n_trans), (4, 1, 2));
        assert_eq!(g.cubes[0].info[0], mat::PIEDRA);
        assert_eq!(g.cubes[1].info[0], mat::VIDRIO);
        assert_eq!(g.cubes[2].info[0], mat::AGUA);
        assert_ne!(g.cubes[3].info[2] & FLAG_GHOST, 0);
    }

    #[test]
    fn lonely_block_on_ground_has_dark_bottom_corners() {
        let ao = ambient_occlusion([0, 0, 0], |_, y, _| y < 0);
        let get = |f: usize, k: usize| {
            let bit = 2 * (4 * f + k);
            if bit < 32 {
                (ao[0] >> bit) & 3
            } else {
                (ao[1] >> (bit - 32)) & 3
            }
        };
        // Cara +X: esquinas de abajo (0, 1) tocan el suelo (abajo y diagonal = 2 vecinos).
        assert_eq!(get(0, 0), 1);
        assert_eq!(get(0, 1), 1);
        assert_eq!(get(0, 2), 3);
        // Arriba: sin sombra.
        assert!((0..4).all(|k| get(2, k) == 3));
    }
}
