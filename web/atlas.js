// Atlas de glifos: una textura grande con una celda por emoji o letra.
// Rust decide qué glifos existen (los registra al usarlos por primera vez);
// aquí se dibujan con Canvas 2D y se suben a WebGPU con mipmaps.
//
// Las letras se guardan como "campo de distancia" (SDF): cada píxel dice qué
// tan lejos está del borde de la letra. El shader recorta en 0.5, así las
// letras se ven nítidas aunque se agranden mucho y pueden llevar borde.

const CELL = 192;
const ROWS = 10;
const MIP_LEVELS = 5;
/// Alcance del campo de distancia, en píxeles del atlas.
const SDF_RADIUS = 12;
const INF = 1e20;

// Andika: tipografía hecha para aprender a leer (la "a" y la "g" son como
// las que se escriben a mano en el colegio). Sin internet se usa la fuente
// del sistema más parecida.
export const TEXT_FONT = '"Andika", "Segoe UI", "Arial Rounded MT Bold", system-ui, sans-serif';
export const TEXT_WEIGHT = 700;
export const EMOJI_FONT = '"Segoe UI Emoji", "Apple Color Emoji", "Noto Color Emoji", "Twemoji Mozilla", sans-serif';

function makeCanvas(w, h) {
  if (typeof OffscreenCanvas !== 'undefined') return new OffscreenCanvas(w, h);
  const c = document.createElement('canvas');
  c.width = w;
  c.height = h;
  return c;
}

export class GlyphAtlas {
  constructor(device) {
    this.device = device;
    const maxDim = Math.min(device.limits.maxTextureDimension2D, 4096);
    this.cols = Math.floor(maxDim / CELL);
    this.rows = ROWS;
    this.capacity = this.cols * this.rows;
    this.width = this.cols * CELL;
    this.height = this.rows * CELL;
    this.texture = device.createTexture({
      size: [this.width, this.height],
      format: 'rgba8unorm',
      mipLevelCount: MIP_LEVELS,
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
    });
    this.canvas = makeCanvas(this.width, this.height);
    this.ctx = this.canvas.getContext('2d');
    this.mips = [];
    for (let l = 1; l < MIP_LEVELS; l++) this.mips.push(makeCanvas(this.width >> l, this.height >> l));
    this.count = 0;
  }

  /// Dibuja los glifos nuevos. Devuelve true si la textura cambió.
  sync(game) {
    const n = Math.min(game.glyphCount(), this.capacity);
    if (n <= this.count) return false;
    for (let i = this.count; i < n; i++) {
      const g = game.glyph(i);
      this.draw(i, g.text, g.isText);
    }
    this.count = n;
    this.upload();
    return true;
  }

  draw(i, text, isText) {
    const ctx = this.ctx;
    const x = (i % this.cols) * CELL;
    const y = Math.floor(i / this.cols) * CELL;
    const cx = x + CELL / 2;
    const cy = y + CELL / 2;
    ctx.save();
    ctx.beginPath();
    ctx.rect(x, y, CELL, CELL);
    ctx.clip();
    ctx.clearRect(x, y, CELL, CELL);
    ctx.textAlign = 'center';
    ctx.textBaseline = 'alphabetic';
    if (isText) {
      let size = CELL * 0.72;
      ctx.font = `${TEXT_WEIGHT} ${size}px ${TEXT_FONT}`;
      let m = ctx.measureText(text);
      const maxW = CELL * 0.82;
      if (m.width > maxW) {
        size *= maxW / m.width;
        ctx.font = `${TEXT_WEIGHT} ${size}px ${TEXT_FONT}`;
        m = ctx.measureText(text);
      }
      const asc = m.actualBoundingBoxAscent || size * 0.7;
      const desc = m.actualBoundingBoxDescent || 0;
      const font = ctx.font;
      ctx.restore();
      this.drawSdf(x, y, text, font, CELL / 2, CELL / 2 + (asc - desc) / 2);
      return;
    } else {
      const size = CELL * 0.7;
      ctx.font = `${size}px ${EMOJI_FONT}`;
      const m = ctx.measureText(text);
      const asc = m.actualBoundingBoxAscent || size * 0.8;
      const desc = m.actualBoundingBoxDescent || size * 0.1;
      ctx.fillStyle = '#000';
      ctx.fillText(text, cx, cy + (asc - desc) / 2);
    }
    ctx.restore();
  }

  /// Dibuja la letra nítida en un lienzo aparte y guarda su campo de distancia.
  drawSdf(x, y, text, font, tx, ty) {
    if (!this.scratch) {
      this.scratch = makeCanvas(CELL, CELL);
      this.sctx = this.scratch.getContext('2d', { willReadFrequently: true });
      const n = CELL * CELL;
      this.gridOuter = new Float64Array(n);
      this.gridInner = new Float64Array(n);
      this.f = new Float64Array(CELL);
      this.z = new Float64Array(CELL + 1);
      this.v = new Uint16Array(CELL);
      this.d = new Float64Array(CELL);
    }
    const c = this.sctx;
    c.clearRect(0, 0, CELL, CELL);
    c.font = font;
    c.textAlign = 'center';
    c.textBaseline = 'alphabetic';
    c.fillStyle = '#fff';
    c.fillText(text, tx, ty);
    const img = c.getImageData(0, 0, CELL, CELL);
    const data = img.data;
    const outer = this.gridOuter;
    const inner = this.gridInner;
    for (let i = 0; i < CELL * CELL; i++) {
      const a = data[i * 4 + 3] / 255;
      if (a === 1) {
        outer[i] = 0;
        inner[i] = INF;
      } else if (a === 0) {
        outer[i] = INF;
        inner[i] = 0;
      } else {
        const d = 0.5 - a;
        outer[i] = d > 0 ? d * d : 0;
        inner[i] = d < 0 ? d * d : 0;
      }
    }
    this.edt(outer);
    this.edt(inner);
    for (let i = 0; i < CELL * CELL; i++) {
      const dist = Math.sqrt(outer[i]) - Math.sqrt(inner[i]);
      const a = Math.max(0, Math.min(1, 0.5 - dist / (2 * SDF_RADIUS)));
      data[i * 4] = data[i * 4 + 1] = data[i * 4 + 2] = 255;
      data[i * 4 + 3] = Math.round(a * 255);
    }
    this.ctx.putImageData(img, x, y);
  }

  /// Transformada de distancia euclidiana en 2D (Felzenszwalb & Huttenlocher).
  edt(grid) {
    for (let x = 0; x < CELL; x++) this.edt1d(grid, x, CELL);
    for (let y = 0; y < CELL; y++) this.edt1d(grid, y * CELL, 1);
  }

  edt1d(grid, offset, stride) {
    const { f, z, v, d } = this;
    const n = CELL;
    for (let q = 0; q < n; q++) f[q] = grid[offset + q * stride];
    let k = 0;
    v[0] = 0;
    z[0] = -INF;
    z[1] = INF;
    for (let q = 1; q < n; q++) {
      let s;
      do {
        const r = v[k];
        s = (f[q] - f[r] + q * q - r * r) / (q - r) / 2;
      } while (s <= z[k] && --k > -1);
      k++;
      v[k] = q;
      z[k] = s;
      z[k + 1] = INF;
    }
    k = 0;
    for (let q = 0; q < n; q++) {
      while (z[k + 1] < q) k++;
      const r = v[k];
      d[q] = f[r] + (q - r) * (q - r);
    }
    for (let q = 0; q < n; q++) grid[offset + q * stride] = d[q];
  }

  upload() {
    const q = this.device.queue;
    q.copyExternalImageToTexture({ source: this.canvas }, { texture: this.texture, mipLevel: 0, premultipliedAlpha: true }, [this.width, this.height]);
    let src = this.canvas;
    for (let l = 1; l < MIP_LEVELS; l++) {
      const dst = this.mips[l - 1];
      const c = dst.getContext('2d');
      c.clearRect(0, 0, dst.width, dst.height);
      c.imageSmoothingEnabled = true;
      c.imageSmoothingQuality = 'high';
      c.drawImage(src, 0, 0, dst.width, dst.height);
      q.copyExternalImageToTexture({ source: dst }, { texture: this.texture, mipLevel: l, premultipliedAlpha: true }, [dst.width, dst.height]);
      src = dst;
    }
  }
}
