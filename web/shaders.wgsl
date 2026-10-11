// Shaders WebGPU del juego.
//
// - vs_full / fs_background: fondo animado que cubre la pantalla.
// - vs_full / fs_composite:  capa de pintura persistente (modo Pintar).
// - vs_sprite / fs_sprite:   todas las figuras, letras y emojis. Cada
//   instancia es un cuadrado; el fragment shader calcula la figura con
//   funciones de distancia (SDF), lo que da bordes suaves a cualquier tamaño.
//
// Todo el color se maneja con alfa premultiplicado.

struct Uniforms {
  view: vec2f,       // tamaño de la pantalla en píxeles CSS
  time: f32,
  atlas_cols: f32,
  bg_top: vec4f,
  bg_bottom: vec4f,
  bg_style: f32,
  bg_param: f32,
  atlas_rows: f32,
  dpr: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

@group(1) @binding(0) var paint_tex: texture_2d<f32>;
@group(1) @binding(1) var paint_samp: sampler;

// ---------------------------------------------------------------------------
// Pantalla completa
// ---------------------------------------------------------------------------

struct FullOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
};

@vertex
fn vs_full(@builtin(vertex_index) vi: u32) -> FullOut {
  var pts = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  let p = pts[vi];
  var out: FullOut;
  out.clip = vec4f(p, 0.0, 1.0);
  out.uv = vec2f(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
  return out;
}

fn hash11(n: f32) -> f32 {
  return fract(sin(n * 127.1) * 43758.5453);
}

fn hash21(p: vec2f) -> f32 {
  return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453);
}

// Círculos suaves que flotan (efecto "bokeh").
fn bokeh(px: vec2f, t: f32, strength: f32) -> f32 {
  var a = 0.0;
  let m = min(u.view.x, u.view.y);
  for (var i = 0; i < 14; i++) {
    let s = f32(i) + 1.0;
    let r = m * (0.04 + 0.1 * hash11(s * 3.1));
    let speed = 6.0 + 14.0 * hash11(s * 5.7);
    let x = hash11(s * 1.3) * u.view.x + sin(t * 0.2 + s) * 30.0;
    let span = u.view.y + 2.0 * r;
    let y = fract(hash11(s * 2.9) - t * speed / span) * span - r;
    let d = length(px - vec2f(x, y));
    a += (1.0 - smoothstep(r * 0.6, r, d)) * (0.08 + 0.08 * hash11(s * 9.1));
  }
  return a * strength;
}

@fragment
fn fs_background(in: FullOut) -> @location(0) vec4f {
  let px = in.uv * u.view;
  let t = u.time;
  var col = mix(u.bg_top.rgb, u.bg_bottom.rgb, smoothstep(0.0, 1.0, in.uv.y));
  let style = u32(u.bg_style + 0.5);

  if (style == 0u) {
    // Bokeh
    col = mix(col, vec3f(1.0), bokeh(px, t, 1.0 + u.bg_param * 0.5));
  } else if (style == 1u) {
    // Bajo el agua: rayos de luz, brillo y arena.
    let ray = pow(max(0.0, sin(px.x * 0.008 + px.y * 0.003 + t * 0.25) * sin(px.x * 0.013 - t * 0.17 + 1.3)), 3.0);
    col += vec3f(0.9, 1.0, 1.0) * ray * 0.22 * (1.0 - in.uv.y);
    let caustic = sin(px.x * 0.05 + t * 1.3) * sin(px.y * 0.04 - t * 1.1) * sin((px.x + px.y) * 0.03 + t);
    col += vec3f(0.6, 0.9, 1.0) * max(caustic, 0.0) * 0.06 * (1.0 - in.uv.y);
    let sand_y = u.view.y * 0.93 + sin(px.x * 0.01) * 10.0 + sin(px.x * 0.027 + 2.0) * 5.0;
    let sand = smoothstep(sand_y - 2.0, sand_y + 2.0, px.y);
    col = mix(col, vec3f(0.98, 0.88, 0.6) - hash21(floor(px / 3.0)) * 0.05, sand);
  } else if (style == 2u) {
    // Papel: granito muy suave.
    col *= 1.0 - hash21(floor(px)) * 0.025;
  } else if (style == 3u) {
    // Noche: estrellas que titilan.
    let cell = floor(px / 46.0);
    let h = hash21(cell);
    if (h > 0.82) {
      let c = (cell + vec2f(0.2 + 0.6 * hash21(cell + 7.0), 0.2 + 0.6 * hash21(cell + 13.0))) * 46.0;
      let tw = 0.5 + 0.5 * sin(t * (1.5 + h * 3.0) + h * 40.0);
      let d = length(px - c);
      col += vec3f(1.0, 0.95, 0.85) * (1.0 - smoothstep(0.5, 2.5 + tw, d)) * (0.4 + 0.6 * tw);
    }
    col = mix(col, vec3f(1.0, 0.8, 1.0), bokeh(px, t, 0.5));
  } else if (style == 4u) {
    // Pradera: nubes, dos colinas de pasto.
    for (var i = 0; i < 4; i++) {
      let s = f32(i) + 1.0;
      let w = u.view.x + 400.0;
      let cx = fract(hash11(s * 4.3) + t * 0.004 * (1.0 + hash11(s))) * w - 200.0;
      let cy = u.view.y * (0.1 + 0.25 * hash11(s * 8.1));
      let q = (px - vec2f(cx, cy)) / vec2f(110.0, 38.0);
      var cloud = 1.0 - smoothstep(0.7, 1.0, length(q));
      cloud = max(cloud, 1.0 - smoothstep(0.6, 0.9, length(q - vec2f(-0.4, -0.5))));
      cloud = max(cloud, 1.0 - smoothstep(0.6, 0.9, length(q - vec2f(0.35, -0.45))));
      col = mix(col, vec3f(1.0), cloud * 0.9);
    }
    let back = u.view.y * 0.62 + sin(px.x * 0.004 + 2.0) * 30.0 + sin(px.x * 0.011) * 12.0;
    col = mix(col, vec3f(0.65, 0.84, 0.42), smoothstep(back - 1.5, back + 1.5, px.y));
    let front = u.view.y * 0.74 + sin(px.x * 0.006 + 0.5) * 22.0 + sin(px.x * 0.019) * 8.0;
    let grass = mix(vec3f(0.55, 0.78, 0.29), vec3f(0.36, 0.6, 0.18), clamp((px.y - front) / (u.view.y * 0.3), 0.0, 1.0));
    col = mix(col, grass, smoothstep(front - 1.5, front + 1.5, px.y));
  }

  // Viñeta suave.
  let v = length(in.uv - vec2f(0.5));
  col *= 1.0 - v * v * 0.25;
  return vec4f(col, 1.0);
}

@fragment
fn fs_composite(in: FullOut) -> @location(0) vec4f {
  return textureSample(paint_tex, paint_samp, in.uv);
}

// ---------------------------------------------------------------------------
// Figuras (sprites)
// ---------------------------------------------------------------------------

struct SpriteIn {
  @location(0) pos: vec2f,
  @location(1) half: vec2f,
  @location(2) misc: vec4f,   // rotación, tipo, glifo|difuminado, parámetro
  @location(3) color: vec4f,
};

struct SpriteOut {
  @builtin(position) clip: vec4f,
  @location(0) lp: vec2f,      // posición local en píxeles (sin rotar)
  @location(1) color: vec4f,
  @location(2) @interpolate(flat) misc: vec4f,
  @location(3) @interpolate(flat) half: vec2f,
};

const K_CIRCLE: u32 = 0u;
const K_RRECT: u32 = 1u;
const K_STAR: u32 = 2u;
const K_HEART: u32 = 3u;
const K_TRIANGLE: u32 = 4u;
const K_RING: u32 = 5u;
const K_EMOJI: u32 = 6u;
const K_GLOW: u32 = 7u;
const K_BUBBLE: u32 = 8u;
const K_DIAMOND: u32 = 9u;
const K_TEXT: u32 = 10u;
const K_FLOWER: u32 = 11u;
const K_ARROW: u32 = 12u;

@vertex
fn vs_sprite(@builtin(vertex_index) vi: u32, inst: SpriteIn) -> SpriteOut {
  var corners = array<vec2f, 6>(
    vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0),
    vec2f(-1.0, 1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0));
  let kind = u32(inst.misc.y + 0.5);
  let is_glyph = kind == K_EMOJI || kind == K_TEXT;
  // Margen extra para el borde suave (y el difuminado de las sombras).
  let pad = 2.0 + select(inst.misc.z, 0.0, is_glyph);
  let ext = abs(inst.half) + vec2f(pad);
  let lp = corners[vi] * ext;
  let cs = cos(inst.misc.x);
  let sn = sin(inst.misc.x);
  let wp = inst.pos + vec2f(lp.x * cs - lp.y * sn, lp.x * sn + lp.y * cs);
  var out: SpriteOut;
  out.clip = vec4f(wp.x / u.view.x * 2.0 - 1.0, 1.0 - wp.y / u.view.y * 2.0, 0.0, 1.0);
  out.lp = lp;
  out.color = inst.color;
  out.misc = inst.misc;
  out.half = inst.half;
  return out;
}

fn dot2(v: vec2f) -> f32 {
  return dot(v, v);
}

fn sd_ellipse(p: vec2f, h: vec2f) -> f32 {
  return (length(p / h) - 1.0) * min(h.x, h.y);
}

fn sd_round_box(p: vec2f, b: vec2f, r: f32) -> f32 {
  let q = abs(p) - b + vec2f(r);
  return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// Estrella de 5 puntas (Inigo Quilez). p con y hacia arriba.
fn sd_star5(p_in: vec2f, r: f32, rf: f32) -> f32 {
  let k1 = vec2f(0.809016994375, -0.587785252292);
  let k2 = vec2f(-k1.x, k1.y);
  var p = vec2f(abs(p_in.x), p_in.y);
  p -= 2.0 * max(dot(k1, p), 0.0) * k1;
  p -= 2.0 * max(dot(k2, p), 0.0) * k2;
  p.x = abs(p.x);
  p.y -= r;
  let ba = rf * vec2f(-k1.y, k1.x) - vec2f(0.0, 1.0);
  let h = clamp(dot(p, ba) / dot(ba, ba), 0.0, r);
  return length(p - ba * h) * sign(p.y * ba.x - p.x * ba.y);
}

// Corazón (Inigo Quilez). p con y hacia arriba, punta en el origen.
fn sd_heart(p_in: vec2f) -> f32 {
  let p = vec2f(abs(p_in.x), p_in.y);
  if (p.y + p.x > 1.0) {
    return sqrt(dot2(p - vec2f(0.25, 0.75))) - sqrt(2.0) / 4.0;
  }
  return sqrt(min(dot2(p - vec2f(0.0, 1.0)), dot2(p - 0.5 * max(p.x + p.y, 0.0)))) * sign(p.x - p.y);
}

// Triángulo cualquiera (Inigo Quilez).
fn sd_triangle(p: vec2f, p0: vec2f, p1: vec2f, p2: vec2f) -> f32 {
  let e0 = p1 - p0;
  let e1 = p2 - p1;
  let e2 = p0 - p2;
  let v0 = p - p0;
  let v1 = p - p1;
  let v2 = p - p2;
  let pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
  let pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
  let pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
  let s = sign(e0.x * e2.y - e0.y * e2.x);
  let d = min(min(vec2f(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)),
                  vec2f(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x))),
                  vec2f(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x)));
  return -sqrt(d.x) * sign(d.y);
}

fn sd_rhombus(p_in: vec2f, b: vec2f) -> f32 {
  let p = abs(p_in);
  let q = b - 2.0 * p;
  let h = clamp((q.x * b.x - q.y * b.y) / dot(b, b), -1.0, 1.0);
  let d = length(p - 0.5 * b * vec2f(1.0 - h, 1.0 + h));
  return d * sign(p.x * b.y + p.y * b.x - b.x * b.y);
}

@fragment
fn fs_sprite(in: SpriteOut) -> @location(0) vec4f {
  let kind = u32(in.misc.y + 0.5);
  let h = abs(in.half);
  let p = in.lp;
  let c = in.color;
  let param = in.misc.w;
  let soft = in.misc.z;
  let r = min(h.x, h.y);
  let aa = max(length(fwidth(p)) * 0.6, 1e-4);

  // El atlas se muestrea siempre (control de flujo uniforme para las derivadas).
  let cols = u.atlas_cols;
  let gi = floor(in.misc.z + 0.5);
  let cell = vec2f(gi - cols * floor(gi / cols), floor(gi / cols));
  var guv = p / h * 0.5 + vec2f(0.5);
  guv.x = select(guv.x, 1.0 - guv.x, in.half.x < 0.0);
  let inside = all(guv >= vec2f(0.0)) && all(guv <= vec2f(1.0));
  let auv = (cell + clamp(guv, vec2f(0.0), vec2f(1.0))) / vec2f(cols, u.atlas_rows);
  let tex = textureSample(atlas, samp, auv);
  let fwa = max(fwidth(tex.a), 1e-4);

  if (kind == K_EMOJI) {
    if (!inside) {
      discard;
    }
    return vec4f(tex.rgb * c.rgb, tex.a) * c.a;
  }

  if (kind == K_TEXT) {
    if (!inside) {
      discard;
    }
    // El glifo está difuminado en el atlas: el contorno 0.5 es el borde.
    let fill = smoothstep(0.5 - fwa, 0.5 + fwa, tex.a);
    let edge = 0.5 - param;
    let outer = select(fill, smoothstep(edge - fwa, edge + fwa, tex.a), param > 0.0);
    let rgb = mix(c.rgb * 0.42, c.rgb, fill);
    let a = max(fill, outer) * c.a;
    if (a <= 0.001) {
      discard;
    }
    return vec4f(rgb * a, a);
  }

  if (kind == K_GLOW) {
    let t = clamp(1.0 - length(p / h), 0.0, 1.0);
    let a = t * t * c.a;
    return vec4f(c.rgb * a, a);
  }

  if (kind == K_BUBBLE) {
    let d = length(p) - r;
    let cov = 1.0 - smoothstep(-aa, aa, d);
    if (cov <= 0.0) {
      discard;
    }
    let nd = length(p) / r;
    let ang = atan2(p.y, p.x);
    let iri = 0.6 + 0.4 * cos(ang * 2.0 + u.time * 0.8 + vec3f(0.0, 2.1, 4.2));
    let rim = smoothstep(0.72, 1.0, nd);
    let spot = 1.0 - smoothstep(0.0, 0.22, length(p / r - vec2f(-0.38, -0.42)));
    let spot2 = 1.0 - smoothstep(0.0, 0.1, length(p / r - vec2f(0.42, 0.45)));
    let body = 0.08;
    let rgb_rim = mix(c.rgb, iri, 0.5);
    var col = c.rgb * body + rgb_rim * rim * 0.6 + vec3f(1.0) * (spot * 0.85 + spot2 * 0.5);
    let a = clamp(body + rim * 0.6 + spot * 0.85 + spot2 * 0.5, 0.0, 1.0) * cov * c.a;
    col *= cov * c.a;
    return vec4f(col, a);
  }

  var d: f32;
  let up = vec2f(p.x, -p.y); // y hacia arriba para las fórmulas clásicas
  switch kind {
    case K_RRECT: {
      d = sd_round_box(p, h, clamp(param, 0.0, 1.0) * r);
    }
    case K_STAR: {
      d = sd_star5(up + vec2f(0.0, 0.095 * r), r * 0.94, 0.5) - r * 0.06;
    }
    case K_HEART: {
      let s = 0.62;
      d = sd_heart(up / r * s + vec2f(0.0, 0.552)) / s * r;
    }
    case K_TRIANGLE: {
      let k = 0.92 * r;
      d = sd_triangle(p, vec2f(0.0, -0.85 * k), vec2f(0.98 * k, 0.85 * k), vec2f(-0.98 * k, 0.85 * k)) - 0.07 * r;
    }
    case K_RING: {
      let th = max(param, 1.0);
      d = abs(length(p) - (r - th * 0.5)) - th * 0.5;
    }
    case K_DIAMOND: {
      d = sd_rhombus(p, vec2f(h.x * 0.8, h.y));
    }
    case K_FLOWER: {
      let ang = atan2(p.y, p.x);
      let rr = r * (0.74 + 0.26 * cos(5.0 * ang));
      d = (length(p) - rr) * 0.75;
    }
    case K_ARROW: {
      let shaft = sd_round_box(p - vec2f(-0.4 * h.x, 0.0), vec2f(0.55 * h.x, 0.3 * h.y), 0.12 * h.y);
      let head = sd_triangle(p, vec2f(h.x * 0.95, 0.0), vec2f(-0.05 * h.x, 0.92 * h.y), vec2f(-0.05 * h.x, -0.92 * h.y));
      d = min(shaft, head) - 0.04 * r;
    }
    default: {
      d = sd_ellipse(p, h);
    }
  }
  let w = max(aa, soft);
  let cov = 1.0 - smoothstep(-w, w, d);
  if (cov <= 0.001) {
    discard;
  }
  let a = cov * c.a;
  return vec4f(c.rgb * a, a);
}
