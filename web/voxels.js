// Dibujo 3D (juego "Construir"): bloques instanciados con WebGPU.
//
// Rust entrega cada cuadro (ver src/gfx3d.rs):
//   - una lista de cubos de 40 bytes: [opacos..., translúcidos..., fantasmas...]
//   - 72 floats de uniformes (cámara, luz del sol, cielo).
// Aquí se dibuja en tres pasos: sombra del sol (sólo profundidad), escena con
// antialiasing (MSAA 4×) que se resuelve directo en la pantalla, y después
// renderer.js dibuja encima la interfaz 2D de siempre.

export const CUBE_BYTES = 40; // = gfx3d::CUBE_BYTES
export const SCENE_FLOATS = 72; // = gfx3d::SCENE_FLOATS
const SCENE_BYTES = SCENE_FLOATS * 4;
const DEPTH = 'depth24plus';
const SHADOW_FORMAT = 'depth32float';

// Niveles de calidad: si el computador no alcanza, se baja solo.
export const QUALITY = [
  { msaa: 4, shadow: 2048 },
  { msaa: 4, shadow: 1024 },
  { msaa: 1, shadow: 1024 },
  { msaa: 1, shadow: 0 },
];

export class Voxels {
  constructor(device, format) {
    this.device = device;
    this.format = format;
    this.ready = false;
    this.failed = false;
    this.level = 0;
    this.pipes = new Map(); // msaa -> { cube, glass, sky }
    this.instBuf = null;
    this.instCap = 0;
    this.width = 0;
    this.height = 0;
    this.msaaTex = null;
    this.depthTex = null;
    this.sceneData = new Float32Array(SCENE_FLOATS);
    this.slow = 0;
    this.frames = 0;
    /// Bajar la calidad solo si va lento (se apaga en pruebas automáticas).
    this.adaptive = !navigator.webdriver;
    /// Avisa el nivel nuevo (main.js lo guarda en localStorage).
    this.onlevel = null;
  }

  /// 0 cargando, 1 listo, 2 no disponible (= gfx3d::Gfx3::status).
  get status() {
    return this.ready ? 1 : this.failed ? 2 : 0;
  }

  /// Compila el shader y arma los pipelines (en segundo plano, sin trabar el juego).
  async init(level = 0) {
    const device = this.device;
    try {
      const code = await (await fetch('cubos.wgsl')).text();
      this.module = device.createShaderModule({ code });
      const info = await this.module.getCompilationInfo();
      const errors = info.messages.filter((m) => m.type === 'error');
      for (const m of info.messages) console[m.type === 'error' ? 'error' : 'warn'](`cubos.wgsl ${m.lineNum}:${m.linePos} ${m.message}`);
      if (errors.length) throw new Error('cubos.wgsl no compila');

      this.uniforms = device.createBuffer({ size: SCENE_BYTES, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
      this.sceneLayout = device.createBindGroupLayout({
        entries: [
          { binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: 'uniform' } },
          { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'depth' } },
          { binding: 2, visibility: GPUShaderStage.FRAGMENT, sampler: { type: 'comparison' } },
        ],
      });
      // El paso de sombra no puede leer la textura en la que escribe: layout aparte.
      this.shadowLayout = device.createBindGroupLayout({
        entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: 'uniform' } }],
      });
      this.shadowBindGroup = device.createBindGroup({ layout: this.shadowLayout, entries: [{ binding: 0, resource: { buffer: this.uniforms } }] });
      this.shadowSampler = device.createSampler({ compare: 'less-equal', magFilter: 'linear', minFilter: 'linear' });
      this.scenePL = device.createPipelineLayout({ bindGroupLayouts: [this.sceneLayout] });
      this.shadowPL = device.createPipelineLayout({ bindGroupLayouts: [this.shadowLayout] });

      this.shadowPipe = await device.createRenderPipelineAsync({
        layout: this.shadowPL,
        vertex: { module: this.module, entryPoint: 'vs_shadow', buffers: [this.cubeBufferLayout()] },
        primitive: { topology: 'triangle-list', cullMode: 'none' },
        depthStencil: { format: SHADOW_FORMAT, depthWriteEnabled: true, depthCompare: 'less', depthBias: 2, depthBiasSlopeScale: 2.0 },
      });
      await this.setLevel(level);
      this.ready = true;
      return true;
    } catch (err) {
      console.error('3D no disponible:', err);
      this.failed = true;
      return false;
    }
  }

  cubeBufferLayout() {
    return {
      arrayStride: CUBE_BYTES,
      stepMode: 'instance',
      attributes: [
        { shaderLocation: 0, offset: 0, format: 'float32x3' }, // pos
        { shaderLocation: 1, offset: 12, format: 'unorm16x2' }, // escala
        { shaderLocation: 2, offset: 16, format: 'snorm16x4' }, // giro (cuaternión)
        { shaderLocation: 3, offset: 24, format: 'unorm8x4' }, // color
        { shaderLocation: 4, offset: 28, format: 'uint8x4' }, // info
        { shaderLocation: 5, offset: 32, format: 'uint32x2' }, // ao
      ],
    };
  }

  async pipelinesFor(msaa) {
    if (this.pipes.has(msaa)) return this.pipes.get(msaa);
    const device = this.device;
    const premultiplied = {
      color: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
      alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
    };
    const vertex = { module: this.module, entryPoint: 'vs_cube', buffers: [this.cubeBufferLayout()] };
    const multisample = { count: msaa };
    const [cube, glass, sky, ground] = await Promise.all([
      device.createRenderPipelineAsync({
        layout: this.scenePL,
        vertex,
        fragment: { module: this.module, entryPoint: 'fs_cube', targets: [{ format: this.format }] },
        primitive: { topology: 'triangle-list', cullMode: 'back' },
        depthStencil: { format: DEPTH, depthWriteEnabled: true, depthCompare: 'less' },
        multisample,
      }),
      device.createRenderPipelineAsync({
        layout: this.scenePL,
        vertex,
        fragment: { module: this.module, entryPoint: 'fs_glass', targets: [{ format: this.format, blend: premultiplied }] },
        primitive: { topology: 'triangle-list', cullMode: 'back' },
        depthStencil: { format: DEPTH, depthWriteEnabled: false, depthCompare: 'less' },
        multisample,
      }),
      device.createRenderPipelineAsync({
        layout: this.scenePL,
        vertex: { module: this.module, entryPoint: 'vs_sky' },
        fragment: { module: this.module, entryPoint: 'fs_sky', targets: [{ format: this.format }] },
        primitive: { topology: 'triangle-list' },
        depthStencil: { format: DEPTH, depthWriteEnabled: false, depthCompare: 'less-equal' },
        multisample,
      }),
      device.createRenderPipelineAsync({
        layout: this.scenePL,
        vertex: { module: this.module, entryPoint: 'vs_ground' },
        fragment: { module: this.module, entryPoint: 'fs_ground', targets: [{ format: this.format }] },
        primitive: { topology: 'triangle-list', cullMode: 'none' },
        depthStencil: { format: DEPTH, depthWriteEnabled: true, depthCompare: 'less' },
        multisample,
      }),
    ]);
    const p = { cube, glass, sky, ground };
    this.pipes.set(msaa, p);
    return p;
  }

  async setLevel(level) {
    level = Math.max(0, Math.min(QUALITY.length - 1, level));
    const q = QUALITY[level];
    const pipes = await this.pipelinesFor(q.msaa);
    const size = Math.max(1, q.shadow);
    if (!this.shadowTex || this.shadowTex.width !== size) {
      if (this.shadowTex) this.shadowTex.destroy();
      this.shadowTex = this.device.createTexture({
        size: [size, size],
        format: SHADOW_FORMAT,
        usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
      });
      this.shadowView = this.shadowTex.createView();
      this.sceneBindGroup = this.device.createBindGroup({
        layout: this.sceneLayout,
        entries: [
          { binding: 0, resource: { buffer: this.uniforms } },
          { binding: 1, resource: this.shadowView },
          { binding: 2, resource: this.shadowSampler },
        ],
      });
      this.shadowDirty = true;
    }
    this.level = level;
    this.quality = q;
    this.pipe = pipes;
    this.width = 0; // rehacer MSAA/profundidad
  }

  /// Si los cuadros 3D tardan mucho, baja la calidad un nivel (una vez cada vez).
  measure(dt) {
    if (!this.ready || this.busy || !this.adaptive) return;
    this.frames++;
    if (dt > 0.026) this.slow++;
    if (this.frames >= 180) {
      const tooSlow = this.slow > this.frames * 0.5;
      this.frames = 0;
      this.slow = 0;
      if (tooSlow && this.level < QUALITY.length - 1) {
        this.busy = true;
        this.setLevel(this.level + 1)
          .then(() => this.onlevel && this.onlevel(this.level))
          .catch((err) => console.warn('3D: no se pudo bajar la calidad', err))
          .finally(() => (this.busy = false));
      }
    }
  }

  ensureTargets(w, h) {
    const msaa = this.quality.msaa;
    if (w === this.width && h === this.height && msaa === this.targetMsaa) return;
    if (this.msaaTex) this.msaaTex.destroy();
    if (this.depthTex) this.depthTex.destroy();
    this.msaaTex =
      msaa > 1 ? this.device.createTexture({ size: [w, h], format: this.format, sampleCount: msaa, usage: GPUTextureUsage.RENDER_ATTACHMENT }) : null;
    this.depthTex = this.device.createTexture({ size: [w, h], format: DEPTH, sampleCount: msaa, usage: GPUTextureUsage.RENDER_ATTACHMENT });
    this.msaaView = this.msaaTex && this.msaaTex.createView();
    this.depthView = this.depthTex.createView();
    this.width = w;
    this.height = h;
    this.targetMsaa = msaa;
  }

  ensureInstances(bytes) {
    if (bytes <= this.instCap) return;
    let cap = Math.max(64 * 1024, this.instCap);
    while (cap < bytes) cap *= 2;
    if (this.instBuf) this.instBuf.destroy();
    this.instBuf = this.device.createBuffer({ size: cap, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
    this.instCap = cap;
  }

  /// Dibuja la escena 3D en `target` (la textura de la pantalla).
  /// `d3` = { scene: Float32Array(72), cubes: Uint8Array, total, opaque }.
  /// Antes de llamar: `ready` debe ser true.
  encode(enc, target, d3) {
    const device = this.device;
    const q = this.quality;
    this.ensureTargets(target.width, target.height);
    const sd = this.sceneData;
    sd.set(d3.scene);
    if (!q.shadow) sd[55] = 0; // sun.w: sin sombra en calidad baja
    device.queue.writeBuffer(this.uniforms, 0, sd);
    if (d3.total > 0) {
      this.ensureInstances(d3.cubes.byteLength);
      device.queue.writeBuffer(this.instBuf, 0, d3.cubes);
    }

    // 1) Sombra del sol.
    if (q.shadow) {
      const pass = enc.beginRenderPass({
        colorAttachments: [],
        depthStencilAttachment: { view: this.shadowView, depthClearValue: 1, depthLoadOp: 'clear', depthStoreOp: 'store' },
      });
      if (d3.opaque > 0) {
        pass.setPipeline(this.shadowPipe);
        pass.setBindGroup(0, this.shadowBindGroup);
        pass.setVertexBuffer(0, this.instBuf);
        pass.draw(36, d3.opaque);
      }
      pass.end();
    }

    // 2) Escena: opacos, pradera, cielo (sólo donde no hay nada), translúcidos y fantasma.
    const view = target.createView();
    const msaa = q.msaa > 1;
    const p = this.pipe;
    const pass = enc.beginRenderPass({
      colorAttachments: [
        {
          view: msaa ? this.msaaView : view,
          resolveTarget: msaa ? view : undefined,
          loadOp: 'clear',
          storeOp: msaa ? 'discard' : 'store',
          clearValue: [sd[60], sd[61], sd[62], 1],
        },
      ],
      depthStencilAttachment: { view: this.depthView, depthClearValue: 1, depthLoadOp: 'clear', depthStoreOp: 'discard' },
    });
    pass.setBindGroup(0, this.sceneBindGroup);
    if (d3.opaque > 0) {
      pass.setPipeline(p.cube);
      pass.setVertexBuffer(0, this.instBuf);
      pass.draw(36, d3.opaque);
    }
    if (sd[69] > 0.5) {
      pass.setPipeline(p.ground);
      pass.draw(6);
    }
    pass.setPipeline(p.sky);
    pass.draw(3);
    const rest = d3.total - d3.opaque;
    if (rest > 0) {
      pass.setPipeline(p.glass);
      pass.setVertexBuffer(0, this.instBuf);
      pass.draw(36, rest, 0, d3.opaque);
    }
    pass.end();
  }
}
