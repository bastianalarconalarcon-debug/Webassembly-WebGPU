// Shaders 3D del juego "Construir": bloques tipo Minecraft.
//
// - vs_shadow:            mapa de sombras del sol (sólo profundidad).
// - vs_cube / fs_cube:    bloques opacos. Textura pixelada de 16×16 calculada
//                         en el shader (no hay archivos de imagen), luz por
//                         cara + sol con sombra, oclusión ambiental por esquina
//                         (la calcula Rust), borde de bloque y resaltado.
// - vs_cube / fs_glass:   vidrio, agua y el bloque fantasma (alfa premultiplicado).
// - vs_sky / fs_sky:      cielo con degradado, sol cuadrado y nubes.
// - vs_ground / fs_ground: pradera de pasto hasta el horizonte (misc.y = 1),
//                         fuera del área de construcción.
//
// Debe coincidir con src/gfx3d.rs (Cube = 40 bytes, Scene = 288 bytes).

struct Scene {
  view_proj: mat4x4f,
  inv_view_proj: mat4x4f,
  light_vp: mat4x4f,
  eye: vec4f,          // xyz cámara, w tiempo
  sun: vec4f,          // xyz hacia el sol, w intensidad de la sombra
  sky_top: vec4f,      // rgb, w inicio de la neblina
  sky_horizon: vec4f,  // rgb, w fin de la neblina
  grid: vec4f,         // x0, z0, tamaño, opacidad de la grilla
  misc: vec4f,         // x pulso del fantasma 0..1; y = 1 pradera; z, w libres
};

@group(0) @binding(0) var<uniform> S: Scene;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_samp: sampler_comparison;

// Materiales (= gfx3d::mat).
const M_PASTO: u32 = 0u;
const M_TIERRA: u32 = 1u;
const M_PIEDRA: u32 = 2u;
const M_MADERA: u32 = 3u;
const M_TRONCO: u32 = 4u;
const M_HOJAS: u32 = 5u;
const M_LADRILLO: u32 = 6u;
const M_ARENA: u32 = 7u;
const M_JUGUETE: u32 = 8u;
const M_ORO: u32 = 9u;
const M_LAMPARA: u32 = 10u;
const M_DIAMANTE: u32 = 11u;
const M_VIDRIO: u32 = 12u;
const M_AGUA: u32 = 13u;

const FLAG_GHOST: u32 = 8u;

// Caras (= gfx3d::FACES): normal, esquina base, eje u, eje v.
const FACE_N = array<vec3f, 6>(vec3f(1.0, 0.0, 0.0), vec3f(-1.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0),
                               vec3f(0.0, -1.0, 0.0), vec3f(0.0, 0.0, 1.0), vec3f(0.0, 0.0, -1.0));
const FACE_BASE = array<vec3f, 6>(vec3f(1.0, 0.0, 1.0), vec3f(0.0, 0.0, 0.0), vec3f(0.0, 1.0, 1.0),
                                  vec3f(0.0, 0.0, 0.0), vec3f(0.0, 0.0, 1.0), vec3f(1.0, 0.0, 0.0));
const FACE_U = array<vec3f, 6>(vec3f(0.0, 0.0, -1.0), vec3f(0.0, 0.0, 1.0), vec3f(1.0, 0.0, 0.0),
                               vec3f(1.0, 0.0, 0.0), vec3f(1.0, 0.0, 0.0), vec3f(-1.0, 0.0, 0.0));
const FACE_V = array<vec3f, 6>(vec3f(0.0, 1.0, 0.0), vec3f(0.0, 1.0, 0.0), vec3f(0.0, 0.0, -1.0),
                               vec3f(0.0, 0.0, 1.0), vec3f(0.0, 1.0, 0.0), vec3f(0.0, 1.0, 0.0));
// Dos triángulos por cara: esquinas en (u, v).
const QUAD = array<vec2f, 6>(vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
                             vec2f(0.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 1.0));

struct CubeIn {
  @location(0) pos: vec3f,     // centro del bloque
  @location(1) scale: vec2f,   // unorm16x2: (horizontal, vertical) / 2
  @location(2) rot: vec4f,     // snorm16x4: cuaternión (giro alrededor del centro)
  @location(3) color: vec4f,   // unorm8x4: tinte
  @location(4) info: vec4u,    // uint8x4: material, caras visibles, resaltado|banderas, semilla
  @location(5) ao: vec2u,      // uint32x2: 2 bits por esquina
};

struct CubeOut {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
  @location(1) uv: vec2f,
  @location(2) color: vec4f,
  @location(3) @interpolate(flat) info: vec4u,  // material, cara, resaltado|banderas, semilla
  @location(4) @interpolate(flat) ao4: vec4f,   // oclusión de las 4 esquinas (0..3)
  @location(5) @interpolate(flat) normal: vec3f,
};

fn ao_corner(ao: vec2u, face: u32, k: u32) -> f32 {
  let bit = 2u * (4u * face + k);
  let hi = bit >= 32u;
  let word = select(ao.x, ao.y, hi);
  let sh = select(bit, bit - 32u, hi);
  return f32((word >> sh) & 3u);
}

fn qrot(q: vec4f, v: vec3f) -> vec3f {
  let t = 2.0 * cross(q.xyz, v);
  return v + q.w * t + cross(q.xyz, t);
}

// Posición en el mundo de una esquina. La escala crece desde el centro de la
// base: así un bloque nuevo "brota" del suelo y al caer se aplasta sin hundirse.
// El giro es alrededor del centro del bloque.
fn cube_corner(c: CubeIn, face: u32, st: vec2f) -> vec3f {
  let local = FACE_BASE[face] + FACE_U[face] * st.x + FACE_V[face] * st.y;
  let s = c.scale * 2.0;
  let p = vec3f((local.x - 0.5) * s.x, local.y * s.y - 0.5, (local.z - 0.5) * s.x);
  return c.pos + qrot(c.rot, p);
}

@vertex
fn vs_cube(@builtin(vertex_index) vi: u32, c: CubeIn) -> CubeOut {
  var out: CubeOut;
  let face = vi / 6u;
  if (((c.info.y >> face) & 1u) == 0u) {
    // Cara tapada por un vecino: triángulo degenerado fuera de la pantalla.
    out.clip = vec4f(2.0, 2.0, 2.0, 1.0);
    return out;
  }
  let st = QUAD[vi % 6u];
  let w = cube_corner(c, face, st);
  out.clip = S.view_proj * vec4f(w, 1.0);
  out.world = w;
  out.uv = st;
  out.color = c.color;
  out.info = vec4u(c.info.x, face, c.info.z, c.info.w);
  out.ao4 = vec4f(ao_corner(c.ao, face, 0u), ao_corner(c.ao, face, 1u), ao_corner(c.ao, face, 2u), ao_corner(c.ao, face, 3u));
  out.normal = qrot(c.rot, FACE_N[face]);
  return out;
}

@vertex
fn vs_shadow(@builtin(vertex_index) vi: u32, c: CubeIn) -> @builtin(position) vec4f {
  let face = vi / 6u;
  if (((c.info.y >> face) & 1u) == 0u) {
    return vec4f(2.0, 2.0, 2.0, 1.0);
  }
  return S.light_vp * vec4f(cube_corner(c, face, QUAD[vi % 6u]), 1.0);
}

// ---------------------------------------------------------------------------
// Texturas procedurales (16×16 "píxeles" por cara)
// ---------------------------------------------------------------------------

fn pcg(v: u32) -> u32 {
  let s = v * 747796405u + 2891336453u;
  let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
  return (w >> 22u) ^ w;
}

fn rnd(a: u32, b: u32) -> f32 {
  return f32(pcg(a ^ pcg(b)) >> 8u) / 16777216.0;
}

struct Tex {
  rgb: vec3f,
  a: f32,      // opacidad (vidrio/agua)
  glow: f32,   // 1 = brilla sola (no la afecta la luz)
};

fn tex(rgb: vec3f) -> Tex {
  return Tex(rgb, 1.0, 0.0);
}

fn grass_rgb(n: f32, n2: f32) -> vec3f {
  var c = vec3f(0.40, 0.68, 0.25) * (0.84 + 0.3 * n);
  if (n2 > 0.9) { c = c * 1.15 + vec3f(0.05, 0.05, 0.0); }
  return c;
}

fn dirt_rgb(n: f32, n2: f32) -> vec3f {
  var c = vec3f(0.55, 0.39, 0.26) * (0.8 + 0.32 * n);
  if (n2 > 0.92) { c = vec3f(0.40, 0.28, 0.19); }
  if (n2 < 0.05) { c = vec3f(0.66, 0.58, 0.48); }
  return c;
}

// `t` = píxel de la textura (0..15, v hacia arriba en las caras laterales),
// `detail` baja el contraste a la distancia para que no "hierva".
fn material(m: u32, face: u32, t: vec2u, seed: u32, time: f32, detail: f32) -> Tex {
  let key = seed * 8u + face;
  let n = mix(0.5, rnd(key, t.x + t.y * 16u), detail);
  let n2 = rnd(key + 4099u, t.x + t.y * 16u);
  let top = face == 2u;
  let bottom = face == 3u;
  let border = t.x == 0u || t.y == 0u || t.x == 15u || t.y == 15u;
  switch m {
    case M_PASTO: {
      if (top) { return tex(grass_rgb(n, n2)); }
      // Costado: tierra con un flequillo de pasto disparejo arriba.
      let fringe = 13u - u32(rnd(key + 77u, t.x) * 2.9);
      if (!bottom && t.y >= fringe) { return tex(grass_rgb(n, n2) * 0.95); }
      return tex(dirt_rgb(n, n2));
    }
    case M_TIERRA: {
      return tex(dirt_rgb(n, n2));
    }
    case M_PIEDRA: {
      var c = vec3f(0.56, 0.57, 0.6) * (0.86 + 0.22 * n);
      let blob = rnd(key + 31u, (t.x / 4u) + (t.y / 3u) * 4u);
      if (blob < 0.35) { c *= 0.88; }
      if (n2 > 0.94) { c *= 0.72; }
      return tex(c);
    }
    case M_MADERA: {
      // Tablas: 4 por bloque, con junta oscura y un corte en cada tabla.
      let along = select(t.x, t.y, top || bottom);
      let across = select(t.y, t.x, top || bottom);
      let plank = across / 4u;
      var c = vec3f(0.74, 0.55, 0.32) * (0.88 + 0.16 * rnd(key + plank, 911u));
      c *= 0.94 + 0.1 * mix(0.5, rnd(key + plank * 31u, along / 3u), detail);
      if (across % 4u == 3u) { c *= mix(1.0, 0.62, detail); }
      if (along == (seed + plank * 7u) % 16u) { c *= mix(1.0, 0.7, detail); }
      return tex(c);
    }
    case M_TRONCO: {
      if (top || bottom) {
        let d = max(abs(f32(t.x) - 7.5), abs(f32(t.y) - 7.5));
        let ring = u32(d);
        if (ring >= 7u) { return tex(vec3f(0.42, 0.30, 0.18) * (0.85 + 0.2 * n)); }
        return tex(vec3f(0.80, 0.64, 0.42) * select(1.0, 0.84, ring % 2u == 1u) * (0.94 + 0.1 * n));
      }
      var c = vec3f(0.42, 0.30, 0.18) * (0.8 + 0.3 * rnd(key, t.x * 3u + 1u));
      c *= 0.9 + 0.15 * n;
      if (rnd(key + 7u, t.x + (t.y / 3u) * 16u) < 0.18) { c *= 0.7; }
      return tex(c);
    }
    case M_HOJAS: {
      var c = vec3f(0.26, 0.56, 0.21) * (0.75 + 0.4 * n);
      if (n2 < 0.2) { c *= mix(1.0, 0.55, detail); }
      if (n2 > 0.92) { c = c * 1.25 + vec3f(0.04, 0.06, 0.0); }
      return tex(c);
    }
    case M_LADRILLO: {
      let row = t.y / 4u;
      let shift = select(0u, 4u, row % 2u == 1u);
      let col = (t.x + shift) / 8u;
      let mortar = t.y % 4u == 0u || (t.x + shift) % 8u == 0u;
      if (mortar) { return tex(mix(vec3f(0.6, 0.35, 0.28), vec3f(0.82, 0.79, 0.74), detail) * (0.92 + 0.08 * n)); }
      return tex(vec3f(0.72, 0.30, 0.21) * (0.85 + 0.22 * rnd(key + row * 5u, col)) * (0.92 + 0.12 * n));
    }
    case M_ARENA: {
      var c = vec3f(0.94, 0.86, 0.62) * (0.9 + 0.13 * n);
      if (n2 > 0.93) { c *= 0.86; }
      return tex(c);
    }
    case M_JUGUETE: {
      // Bloque de juguete: casi liso (el color lo pone el tinte), con un
      // biselado de 1 píxel y un panel al centro.
      var k = 0.95 + 0.07 * n;
      if (t.x == 0u || t.y == 15u) { k *= 1.12; }
      if (t.x == 15u || t.y == 0u) { k *= 0.8; }
      let inner = t.x >= 3u && t.x <= 12u && t.y >= 3u && t.y <= 12u;
      let ring = inner && (t.x == 3u || t.x == 12u || t.y == 3u || t.y == 12u);
      if (ring) { k *= mix(1.0, select(0.86, 1.08, t.x == 3u || t.y == 12u), detail); }
      return tex(vec3f(k));
    }
    case M_ORO, M_DIAMANTE: {
      let gold = m == M_ORO;
      var c = select(vec3f(0.45, 0.90, 0.92), vec3f(1.0, 0.80, 0.24), gold) * (0.88 + 0.16 * n);
      let diag = (t.x + t.y) % 16u;
      if (diag == 5u || diag == 6u) { c = c * 1.15 + 0.08; }
      if (!gold && (t.x == t.y || t.x == 15u - t.y)) { c = c * 1.1 + 0.05; }
      if (border) { c *= 0.78; }
      // Destellos que aparecen y desaparecen.
      let blink = rnd(key + u32(time * 2.5) * 13u, t.x + t.y * 16u);
      if (blink > 0.985) { c = vec3f(1.0); }
      return tex(c);
    }
    case M_LAMPARA: {
      var c = vec3f(1.0, 0.84, 0.46) * (0.82 + 0.25 * n);
      if (rnd(key + 3u, (t.x / 3u) + (t.y / 3u) * 8u) > 0.6) { c = vec3f(1.0, 0.95, 0.75); }
      if (border) { c *= 0.75; }
      return Tex(c, 1.0, 1.0);
    }
    case M_VIDRIO: {
      if (border) { return Tex(vec3f(0.93, 0.97, 1.0), 0.85, 0.0); }
      let d = t.x + t.y;
      let glint = (d == 9u || d == 10u || d == 19u) && t.x > 2u && t.x < 12u;
      if (glint) { return Tex(vec3f(1.0), 0.6, 0.0); }
      return Tex(vec3f(0.78, 0.92, 1.0), 0.16, 0.0);
    }
    case M_AGUA: {
      // Agua: el dibujo baja por los costados y se mueve arriba.
      let k = u32(time * 4.0);
      let tt = select(vec2u(t.x, (t.y + k) % 16u), vec2u((t.x + k / 2u) % 16u, t.y), top);
      let w = mix(0.5, rnd(key, tt.x + tt.y * 16u), detail);
      var c = vec3f(0.22, 0.48, 0.92) * (0.82 + 0.25 * w);
      if (top && w > 0.86) { c = vec3f(0.65, 0.85, 1.0); }
      return Tex(c, 0.62, 0.0);
    }
    default: {
      return tex(vec3f(1.0, 0.0, 1.0));
    }
  }
}

// ---------------------------------------------------------------------------
// Luz
// ---------------------------------------------------------------------------

// 1 = al sol, 0 = en sombra. 4 muestras con filtro (bordes suaves).
fn sun_visibility(world: vec3f, n: vec3f) -> f32 {
  if (S.sun.w <= 0.0) {
    return 1.0;
  }
  // Las sombras también son "pixeladas": se ajustan a los píxeles de la textura.
  // (Sólo en caras alineadas con los ejes: un bloque girado usa la posición exacta.)
  let snapped = (floor(world * 16.0) + 0.5) / 16.0;
  let aligned = max(abs(n.x), max(abs(n.y), abs(n.z))) > 0.999;
  let p = select(world, select(snapped, world, abs(n) > vec3f(0.5)), aligned) + n * 0.03;
  let l = S.light_vp * vec4f(p, 1.0);
  let uv = vec2f(l.x * 0.5 + 0.5, 0.5 - l.y * 0.5);
  if (any(uv <= vec2f(0.0)) || any(uv >= vec2f(1.0)) || l.z >= 1.0) {
    return 1.0;
  }
  let texel = 1.0 / f32(textureDimensions(shadow_map).x);
  let z = l.z - 0.0015;
  var s = 0.0;
  s += textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2f(-0.7, -0.7) * texel, z);
  s += textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2f(0.7, -0.7) * texel, z);
  s += textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2f(-0.7, 0.7) * texel, z);
  s += textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2f(0.7, 0.7) * texel, z);
  return mix(1.0, s * 0.25, S.sun.w);
}

fn fog(col: vec3f, world: vec3f) -> vec3f {
  let d = length(world - S.eye.xyz);
  return mix(col, S.sky_horizon.rgb, smoothstep(S.sky_top.w, S.sky_horizon.w, d) * 0.85);
}

// Color final de un fragmento de cubo (alfa premultiplicado).
fn shade(in: CubeOut, fw_uv: vec2f) -> vec4f {
  let m = in.info.x;
  let face = in.info.y;
  let hl = in.info.z & 7u;
  let ghost = (in.info.z & FLAG_GHOST) != 0u;
  let seed = in.info.w;
  let n = normalize(in.normal);

  // Píxel de la textura y cuánto detalle se alcanza a ver.
  let t = vec2u(min(floor(in.uv * 16.0), vec2f(15.0)));
  let texel_px = max(fw_uv.x, fw_uv.y) * 16.0;
  let detail = 1.0 - smoothstep(0.6, 1.4, texel_px);
  let tx = material(m, face, t, seed, S.eye.w, detail);

  // Borde de bloque: línea de ~1 px que separa cada bloque (ayuda a contarlos).
  let e = min(in.uv, vec2f(1.0) - in.uv) / max(fw_uv, vec2f(1e-5));
  let edge_px = min(e.x, e.y);
  let selected = hl == 7u || hl == face + 1u;
  let line_w = select(1.0, 2.5, selected || ghost);
  let line = 1.0 - smoothstep(line_w - 0.5, line_w + 0.5, edge_px);

  if (ghost) {
    // Fantasma: color del bloque que se va a poner, translúcido y latiendo.
    let pulse = S.misc.x;
    let body = in.color.rgb * (0.85 + 0.3 * tx.rgb.g);
    let a = mix(0.28 + 0.2 * pulse, 0.95, line);
    let rgb = mix(body, vec3f(1.0), line);
    return vec4f(rgb * a, a);
  }

  let albedo = tx.rgb * in.color.rgb;

  // Oclusión ambiental: interpolación bilineal de las 4 esquinas.
  let ao = mix(mix(in.ao4.x, in.ao4.y, in.uv.x), mix(in.ao4.w, in.ao4.z, in.uv.x), in.uv.y);
  let aof = 0.5 + 0.5 * smoothstep(0.0, 3.0, ao);

  let sky = mix(0.48, 0.66, n.y * 0.5 + 0.5);
  let sun = max(dot(n, S.sun.xyz), 0.0) * 0.5 * sun_visibility(in.world, n);
  let light = vec3f(0.92, 0.96, 1.06) * sky * aof + vec3f(1.0, 0.95, 0.85) * sun * (0.6 + 0.4 * aof);
  var col = mix(albedo * light, albedo, tx.glow);

  // Borde un poco más oscuro (o blanco si está resaltado).
  col = mix(col, select(col * 0.62, vec3f(1.0), selected), line * select(0.85, 1.0, selected));
  if (selected) {
    col = mix(col, vec3f(1.0), 0.18 + 0.17 * S.misc.x);
  }
  col = fog(col, in.world);
  let a = tx.a * in.color.a;
  return vec4f(col * a, a);
}

@fragment
fn fs_cube(in: CubeOut) -> @location(0) vec4f {
  let fw = fwidth(in.uv);
  let c = shade(in, fw);
  return vec4f(c.rgb, 1.0);
}

@fragment
fn fs_glass(in: CubeOut) -> @location(0) vec4f {
  let fw = fwidth(in.uv);
  return shade(in, fw);
}

// ---------------------------------------------------------------------------
// Pradera: un plano de pasto enorme en y = 0 alrededor del área de construcción
// (adentro del área el pasto son bloques, con su grilla).
// ---------------------------------------------------------------------------

struct GroundOut {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
};

const GROUND_RADIUS: f32 = 260.0;

@vertex
fn vs_ground(@builtin(vertex_index) vi: u32) -> GroundOut {
  var out: GroundOut;
  let c = S.grid.xy + vec2f(S.grid.z * 0.5);
  let xz = c + (QUAD[vi] * 2.0 - 1.0) * GROUND_RADIUS;
  let w = vec3f(xz.x, 0.0, xz.y);
  out.world = w;
  out.clip = select(S.view_proj * vec4f(w, 1.0), vec4f(2.0, 2.0, 2.0, 1.0), S.misc.y < 0.5);
  return out;
}

@fragment
fn fs_ground(in: GroundOut) -> @location(0) vec4f {
  let p = in.world.xz;
  // Derivadas antes de cualquier discard (control de flujo uniforme).
  let fw = fwidth(p);
  let a0 = S.grid.xy;
  let a1 = a0 + vec2f(S.grid.z);
  if (all(p > a0) && all(p < a1)) {
    discard;
  }
  let cell = vec2i(floor(p));
  let t = vec2u(min(floor(fract(p) * 16.0), vec2f(15.0)));
  let seed = ((u32(cell.x + 4096) * 73856093u) ^ (u32(cell.y + 4096) * 19349663u)) & 255u;
  let detail = 1.0 - smoothstep(0.6, 1.4, max(fw.x, fw.y) * 16.0);
  let tx = material(M_PASTO, 2u, t, seed, S.eye.w, detail);
  // Manchones grandes de pasto más claro y más oscuro, para que no sea monótono.
  let spot = vnoise(p / 9.0);
  var albedo = tx.rgb * (0.9 + 0.2 * spot);
  albedo = mix(albedo, albedo * vec3f(0.95, 1.06, 0.82), smoothstep(0.6, 0.8, vnoise(p / 23.0 + vec2f(7.0))));
  let n = vec3f(0.0, 1.0, 0.0);
  let sun = max(dot(n, S.sun.xyz), 0.0) * 0.5 * sun_visibility(in.world, n);
  let light = vec3f(0.92, 0.96, 1.06) * 0.66 + vec3f(1.0, 0.95, 0.85) * sun;
  // Neblina más larga que la de los bloques: el pasto se funde con el horizonte.
  let d = length(in.world - S.eye.xyz);
  let col = mix(albedo * light, S.sky_horizon.rgb, smoothstep(S.sky_top.w, S.sky_horizon.w * 2.2, d));
  return vec4f(col, 1.0);
}

// ---------------------------------------------------------------------------
// Cielo
// ---------------------------------------------------------------------------

struct SkyOut {
  @builtin(position) clip: vec4f,
  @location(0) ndc: vec2f,
};

@vertex
fn vs_sky(@builtin(vertex_index) vi: u32) -> SkyOut {
  var pts = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  var out: SkyOut;
  // z = 1: queda detrás de todo; sólo se pinta donde no hay bloques.
  out.clip = vec4f(pts[vi], 1.0, 1.0);
  out.ndc = pts[vi];
  return out;
}

fn vnoise(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  let ix = vec2u(vec2i(i) + vec2i(4096));
  let a = rnd(ix.x, ix.y * 7919u);
  let b = rnd(ix.x + 1u, ix.y * 7919u);
  let c = rnd(ix.x, (ix.y + 1u) * 7919u);
  let d = rnd(ix.x + 1u, (ix.y + 1u) * 7919u);
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Capa de nubes a la altura `h`, vista en la dirección `dir`.
fn clouds(dir: vec3f, h: f32, time: f32) -> f32 {
  let k = (h - S.eye.y) / dir.y;
  if (k <= 0.0) {
    return 0.0;
  }
  let p = (S.eye.xz + dir.xz * k) / 14.0 + vec2f(time * 0.02, time * 0.008);
  let v = vnoise(p) * 0.65 + vnoise(p * 2.3 + 5.0) * 0.35;
  let fade = 1.0 - smoothstep(60.0, 180.0, k);
  return smoothstep(0.56, 0.64, v) * fade;
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4f {
  let p = S.inv_view_proj * vec4f(in.ndc, 1.0, 1.0);
  let dir = normalize(p.xyz / p.w - S.eye.xyz);
  let up = clamp(dir.y, 0.0, 1.0);
  var col = mix(S.sky_horizon.rgb, S.sky_top.rgb, pow(up, 0.6));
  // Abajo de la isla: el mismo cielo, más claro (como volar sobre las nubes).
  // En la pradera, bajo el horizonte sólo se ve el color del horizonte.
  let below = select(mix(S.sky_horizon.rgb, vec3f(1.0), 0.35), S.sky_horizon.rgb, S.misc.y > 0.5);
  col = mix(col, below, smoothstep(0.0, -0.6, dir.y));

  // Sol cuadrado, como en Minecraft, con un halo suave.
  let sd = S.sun.xyz;
  let t1 = normalize(cross(sd, vec3f(0.0, 0.0, 1.0)));
  let t2 = cross(t1, sd);
  let facing = dot(dir, sd);
  let q = vec2f(dot(dir, t1), dot(dir, t2)) / max(facing, 1e-3);
  let sq = max(abs(q.x), abs(q.y));
  if (facing > 0.0) {
    col += vec3f(1.0, 0.9, 0.6) * exp(-sq * 6.0) * 0.35;
    col = mix(col, vec3f(1.0, 0.97, 0.82), 1.0 - smoothstep(0.075, 0.085, sq));
  }

  // Nubes arriba y abajo.
  let tm = S.eye.w;
  var cl = 0.0;
  if (dir.y > 0.02) {
    cl = clouds(dir, 34.0, tm);
  } else if (dir.y < -0.02 && S.misc.y < 0.5) {
    cl = clouds(dir, -16.0, tm) * 0.9;
  }
  col = mix(col, vec3f(1.0), cl * 0.92);
  return vec4f(col, 1.0);
}
