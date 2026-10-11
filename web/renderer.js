// Dibujo con WebGPU. Recibe desde Rust (vía memoria del wasm) la lista de
// figuras de cada cuadro y la dibuja con instancing: un solo draw call para
// todo. El modo Pintar usa además una textura que nunca se borra.

import { GlyphAtlas } from './atlas.js';
import { Voxels } from './voxels.js';

const INSTANCE_FLOATS = 12; // debe coincidir con `Instance` en src/gfx.rs
const INSTANCE_BYTES = INSTANCE_FLOATS * 4;
const UNIFORM_BYTES = 64;

export class Renderer {
  static async create(canvas) {
    if (!navigator.gpu) throw new Error('no-webgpu');
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
    if (!adapter) throw new Error('no-adapter');
    const device = await adapter.requestDevice();
    const code = await (await fetch('shaders.wgsl')).text();
    return new Renderer(canvas, adapter, device, code);
  }

  constructor(canvas, adapter, device, code) {
    this.canvas = canvas;
    // Guardar el adapter: si el recolector de basura lo libera, algunos
    // navegadores pierden el dispositivo.
    this.adapter = adapter;
    this.device = device;
    this.context = canvas.getContext('webgpu');
    this.format = navigator.gpu.getPreferredCanvasFormat();
    this.context.configure({ device, format: this.format, alphaMode: 'opaque' });
    this.lost = false;
    device.lost.then((info) => {
      this.lost = true;
      console.error('WebGPU: dispositivo perdido', info.message);
    });

    this.module = device.createShaderModule({ code });
    this.module.getCompilationInfo().then((info) => {
      for (const m of info.messages) console[m.type === 'error' ? 'error' : 'warn'](`WGSL ${m.lineNum}:${m.linePos} ${m.message}`);
    });

    this.atlas = new GlyphAtlas(device);
    this.uniforms = device.createBuffer({ size: UNIFORM_BYTES, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    this.uniformData = new Float32Array(UNIFORM_BYTES / 4);
    this.sampler = device.createSampler({ magFilter: 'linear', minFilter: 'linear', mipmapFilter: 'linear' });

    this.mainLayout = device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: 'uniform' } },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: {} },
        { binding: 2, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
      ],
    });
    this.paintLayout = device.createBindGroupLayout({
      entries: [
        { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: {} },
        { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
      ],
    });

    const premultiplied = {
      color: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
      alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
    };
    const mainPL = device.createPipelineLayout({ bindGroupLayouts: [this.mainLayout] });
    const compPL = device.createPipelineLayout({ bindGroupLayouts: [this.mainLayout, this.paintLayout] });

    this.bgPipeline = device.createRenderPipeline({
      layout: mainPL,
      vertex: { module: this.module, entryPoint: 'vs_full' },
      fragment: { module: this.module, entryPoint: 'fs_background', targets: [{ format: this.format }] },
      primitive: { topology: 'triangle-list' },
    });
    this.compositePipeline = device.createRenderPipeline({
      layout: compPL,
      vertex: { module: this.module, entryPoint: 'vs_full' },
      fragment: { module: this.module, entryPoint: 'fs_composite', targets: [{ format: this.format, blend: premultiplied }] },
      primitive: { topology: 'triangle-list' },
    });
    const spritePipeline = (format) =>
      device.createRenderPipeline({
        layout: mainPL,
        vertex: {
          module: this.module,
          entryPoint: 'vs_sprite',
          buffers: [
            {
              arrayStride: INSTANCE_BYTES,
              stepMode: 'instance',
              attributes: [
                { shaderLocation: 0, offset: 0, format: 'float32x2' },
                { shaderLocation: 1, offset: 8, format: 'float32x2' },
                { shaderLocation: 2, offset: 16, format: 'float32x4' },
                { shaderLocation: 3, offset: 32, format: 'float32x4' },
              ],
            },
          ],
        },
        fragment: { module: this.module, entryPoint: 'fs_sprite', targets: [{ format, blend: premultiplied }] },
        primitive: { topology: 'triangle-list' },
      });
    this.spritePipeline = spritePipeline(this.format);
    this.paintPipeline = this.format === 'rgba8unorm' ? this.spritePipeline : spritePipeline('rgba8unorm');

    // Escena 3D (juego Construir): se compila en segundo plano (initVoxels).
    this.voxels = new Voxels(device, this.format);

    this.sceneBuf = null;
    this.sceneCap = 0;
    this.paintBuf = null;
    this.paintCap = 0;
    this.paintTex = null;
    this.mainBindGroup = null;
    this.width = 0;
    this.height = 0;
  }

  /// Ajusta el canvas al tamaño de la ventana. Devuelve [ancho, alto] en px CSS.
  resize() {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const cssW = Math.max(1, window.innerWidth);
    const cssH = Math.max(1, window.innerHeight);
    const w = Math.max(1, Math.round(cssW * dpr));
    const h = Math.max(1, Math.round(cssH * dpr));
    this.dpr = dpr;
    this.cssW = cssW;
    this.cssH = cssH;
    if (w !== this.width || h !== this.height) {
      this.canvas.width = w;
      this.canvas.height = h;
      this.width = w;
      this.height = h;
      this.resizePaint(w, h);
    }
    return [cssW, cssH];
  }

  resizePaint(w, h) {
    const tex = this.device.createTexture({
      size: [w, h],
      format: 'rgba8unorm',
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_SRC | GPUTextureUsage.COPY_DST,
    });
    const enc = this.device.createCommandEncoder();
    // Empezar transparente.
    enc.beginRenderPass({ colorAttachments: [{ view: tex.createView(), loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 0] }] }).end();
    // Conservar el dibujo anterior al cambiar el tamaño de la ventana.
    if (this.paintTex) {
      const cw = Math.min(w, this.paintTex.width);
      const ch = Math.min(h, this.paintTex.height);
      enc.copyTextureToTexture({ texture: this.paintTex }, { texture: tex }, [cw, ch]);
      this.paintTex.destroy();
    }
    this.device.queue.submit([enc.finish()]);
    this.paintTex = tex;
    this.paintBindGroup = this.device.createBindGroup({
      layout: this.paintLayout,
      entries: [
        { binding: 0, resource: tex.createView() },
        { binding: 1, resource: this.sampler },
      ],
    });
  }

  /// Agrega al atlas los glifos nuevos que Rust registró.
  syncGlyphs(game) {
    const before = this.atlas.count;
    const changed = this.atlas.sync(game);
    if (changed || !this.mainBindGroup) {
      this.mainBindGroup = this.device.createBindGroup({
        layout: this.mainLayout,
        entries: [
          { binding: 0, resource: { buffer: this.uniforms } },
          { binding: 1, resource: this.atlas.texture.createView() },
          { binding: 2, resource: this.sampler },
        ],
      });
    }
    return this.atlas.count - before;
  }

  ensureBuffer(which, count) {
    const capKey = which + 'Cap';
    const bufKey = which + 'Buf';
    if (count <= this[capKey]) return this[bufKey];
    let cap = Math.max(1024, this[capKey]);
    while (cap < count) cap *= 2;
    if (this[bufKey]) this[bufKey].destroy();
    this[bufKey] = this.device.createBuffer({ size: cap * INSTANCE_BYTES, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
    this[capKey] = cap;
    return this[bufKey];
  }

  /// Dibuja un cuadro. `frame` trae vistas a la memoria del wasm.
  render(time, frame) {
    if (this.lost) return;
    const device = this.device;
    const ud = this.uniformData;
    ud[0] = this.cssW;
    ud[1] = this.cssH;
    ud[2] = time;
    ud[3] = this.atlas.cols;
    ud.set(frame.bg.subarray(0, 8), 4); // bg_top, bg_bottom
    ud[12] = frame.bg[8]; // estilo
    ud[13] = frame.bg[9]; // parámetro
    ud[14] = this.atlas.rows;
    ud[15] = this.dpr;
    device.queue.writeBuffer(this.uniforms, 0, ud);

    const enc = device.createCommandEncoder();

    // 1) Capa de pintura persistente.
    const paintCount = frame.paint.length / INSTANCE_FLOATS;
    if (frame.clearPaint || paintCount > 0) {
      let buf = null;
      if (paintCount > 0) {
        buf = this.ensureBuffer('paint', paintCount);
        device.queue.writeBuffer(buf, 0, frame.paint);
      }
      const pass = enc.beginRenderPass({
        colorAttachments: [
          {
            view: this.paintTex.createView(),
            loadOp: frame.clearPaint ? 'clear' : 'load',
            storeOp: 'store',
            clearValue: [0, 0, 0, 0],
          },
        ],
      });
      if (paintCount > 0) {
        pass.setPipeline(this.paintPipeline);
        pass.setBindGroup(0, this.mainBindGroup);
        pass.setVertexBuffer(0, buf);
        pass.draw(6, paintCount);
      }
      pass.end();
    }

    // 2) Escena 3D (si el juego la pidió y ya está lista): sombra + cubos +
    //    cielo, directo en la pantalla. Reemplaza al fondo 2D.
    const target = this.context.getCurrentTexture();
    const use3d = !!frame.d3 && this.voxels.ready;
    if (use3d) this.voxels.encode(enc, target, frame.d3);

    // 3) Pantalla: fondo, pintura y figuras (con 3D: sólo figuras, encima).
    const sceneCount = frame.scene.length / INSTANCE_FLOATS;
    let sceneBuf = null;
    if (sceneCount > 0) {
      sceneBuf = this.ensureBuffer('scene', sceneCount);
      device.queue.writeBuffer(sceneBuf, 0, frame.scene);
    }
    const pass = enc.beginRenderPass({
      colorAttachments: [{ view: target.createView(), loadOp: use3d ? 'load' : 'clear', storeOp: 'store', clearValue: [1, 1, 1, 1] }],
    });
    pass.setBindGroup(0, this.mainBindGroup);
    if (!use3d) {
      pass.setPipeline(this.bgPipeline);
      pass.draw(3);
      if (frame.showPaint) {
        pass.setPipeline(this.compositePipeline);
        pass.setBindGroup(1, this.paintBindGroup);
        pass.draw(3);
      }
    }
    if (sceneCount > 0) {
      pass.setPipeline(this.spritePipeline);
      pass.setVertexBuffer(0, sceneBuf);
      pass.draw(6, sceneCount);
    }
    pass.end();
    device.queue.submit([enc.finish()]);
  }
}
