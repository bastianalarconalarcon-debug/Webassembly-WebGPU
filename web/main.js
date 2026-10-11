// Punto de entrada: carga el wasm (Rust), WebGPU, sonido y voz, y conecta
// el mouse y el teclado. La lógica del juego está toda en Rust (src/).

import { Renderer } from './renderer.js';
import { TEXT_FONT, TEXT_WEIGHT } from './atlas.js';
import { Synth } from './audio.js';
import { Voice } from './speech.js';
import { CUBE_BYTES, SCENE_FLOATS, QUALITY } from './voxels.js';

// Tipos de evento: deben coincidir con src/audio.rs.
const EV = { NOTE: 1, SFX: 2, SPEAK: 3, HUSH: 4, EXIT: 5 };
const EVENT_FLOATS = 6;
const INSTANCE_FLOATS = 12;
const NAME_KEY = 'juego.nombre';
const VOLUME_KEY = 'juego.volumen';
const QUALITY_KEY = 'juego.calidad3d';

const decoder = new TextDecoder();
const encoder = new TextEncoder();

/** Envoltorio de las funciones que exporta el módulo wasm. */
class Game {
  static async load(url) {
    let memory = null;
    const imports = {
      env: {
        host_log: (ptr, len) => console.log('[juego]', decoder.decode(new Uint8Array(memory.buffer, ptr, len))),
      },
    };
    let result;
    try {
      result = await WebAssembly.instantiateStreaming(fetch(url), imports);
    } catch (_) {
      // Algunos servidores no envían el tipo application/wasm.
      const bytes = await (await fetch(url)).arrayBuffer();
      result = await WebAssembly.instantiate(bytes, imports);
    }
    memory = result.instance.exports.memory;
    return new Game(result.instance.exports);
  }

  constructor(x) {
    this.x = x;
    this.codes = new Map();
    for (let i = 0; i < x.key_code_count(); i++) this.codes.set(this.str(x.key_code_ptr(i), x.key_code_len(i)), i);
  }

  get buffer() {
    return this.x.memory.buffer;
  }

  str(ptr, len) {
    return decoder.decode(new Uint8Array(this.buffer, ptr, len));
  }

  f32(ptr, len) {
    return new Float32Array(this.buffer, ptr, len);
  }

  glyphCount() {
    return this.x.glyph_count();
  }

  glyph(i) {
    return { text: this.str(this.x.glyph_ptr(i), this.x.glyph_len(i)), isText: this.x.glyph_is_text(i) === 1 };
  }

  setName(name) {
    const bytes = encoder.encode(name).slice(0, this.x.text_in_cap());
    new Uint8Array(this.buffer, this.x.text_in_ptr(), bytes.length).set(bytes);
    this.x.set_name(bytes.length);
  }

  frame() {
    const x = this.x;
    const bg = this.f32(x.bg_ptr(), 12);
    return {
      scene: this.f32(x.scene_ptr(), x.scene_len() * INSTANCE_FLOATS),
      paint: this.f32(x.paint_ptr(), x.paint_len() * INSTANCE_FLOATS),
      clearPaint: x.paint_clear() === 1,
      showPaint: bg[10] > 0.5,
      bg,
      d3: this.frame3d(),
    };
  }

  /// Escena 3D del cuadro (o null). Las vistas se crean cada cuadro: la
  /// memoria del wasm puede crecer y cambiar de lugar.
  frame3d() {
    const x = this.x;
    if (!x.d3_active || x.d3_active() !== 1) return null;
    const total = x.d3_cubes_len();
    return {
      scene: this.f32(x.d3_scene_ptr(), SCENE_FLOATS),
      cubes: new Uint8Array(this.buffer, x.d3_cubes_ptr(), total * CUBE_BYTES),
      total,
      opaque: x.d3_opaque_len(),
    };
  }

  *events() {
    const n = this.x.events_len();
    if (n === 0) return;
    const ev = this.f32(this.x.events_ptr(), n * EVENT_FLOATS).slice();
    const textPtr = this.x.speech_ptr();
    for (let i = 0; i < n; i++) {
      const e = ev.subarray(i * EVENT_FLOATS, (i + 1) * EVENT_FLOATS);
      if (e[0] === EV.SPEAK) yield { type: EV.SPEAK, text: this.str(textPtr + e[1], e[2]) };
      else yield { type: e[0], a: e[1], b: e[2], c: e[3], d: e[4] };
    }
  }
}

// ---------------------------------------------------------------------------

const $ = (id) => document.getElementById(id);
const canvas = $('juego');
const cover = $('portada');
const playBtn = $('jugar');
const playText = $('jugar-texto');
const notice = $('aviso');
const nameInput = $('nombre');
const voiceSelect = $('voz');
const volumeInput = $('volumen');
const fullscreenBox = $('completa');

const synth = new Synth();
const voice = new Voice();
let game = null;
let renderer = null;
let playing = false;
let lastTime = 0;
let lastSize = [0, 0];
let wheelAt = 0;

function storageGet(key) {
  try {
    return localStorage.getItem(key);
  } catch (_) {
    return null;
  }
}

function storageSet(key, value) {
  try {
    localStorage.setItem(key, value);
  } catch (_) {}
}

function showNotice(html) {
  notice.innerHTML = html;
  notice.hidden = false;
}

function fillVoices(list, current) {
  voiceSelect.innerHTML = '';
  if (!list.length) {
    const o = document.createElement('option');
    o.textContent = voice.supported ? 'Voz del sistema (no hay voces en español)' : 'Este navegador no tiene voz';
    voiceSelect.appendChild(o);
    return;
  }
  for (const v of list) {
    const o = document.createElement('option');
    o.value = v.name;
    o.textContent = `${v.name} (${v.lang})`;
    if (current && v.name === current.name) o.selected = true;
    voiceSelect.appendChild(o);
  }
}

voice.onvoices = fillVoices;
fillVoices(voice.voices, voice.voice);
voiceSelect.addEventListener('change', () => voice.choose(voiceSelect.value));
$('probar').addEventListener('click', () => {
  const n = nameInput.value.trim();
  voice.speak(n ? `¡Hola, ${n}! ¿Vamos a jugar?` : '¡Hola! ¿Vamos a jugar?');
});

nameInput.value = storageGet(NAME_KEY) || '';
const savedVolume = parseFloat(storageGet(VOLUME_KEY));
if (!Number.isNaN(savedVolume)) volumeInput.value = String(savedVolume);
synth.setVolume(parseFloat(volumeInput.value));
volumeInput.addEventListener('input', () => {
  synth.setVolume(parseFloat(volumeInput.value));
  storageSet(VOLUME_KEY, volumeInput.value);
});

// ---------------------------------------------------------------------------

async function boot() {
  if (!navigator.gpu) {
    showNotice(
      'Este navegador no tiene <b>WebGPU</b>. Abre el juego en <b>Microsoft Edge</b> o <b>Google Chrome</b> actualizados (versión 113 o más nueva).'
    );
    playText.textContent = 'Sin WebGPU';
    return;
  }
  try {
    // Esperar la tipografía de las letras (máximo 2,5 s) antes de armar el atlas.
    const font = document.fonts ? Promise.race([document.fonts.load(`${TEXT_WEIGHT} 64px ${TEXT_FONT}`), new Promise((r) => setTimeout(r, 2500))]) : null;
    const [r, g] = await Promise.all([Renderer.create(canvas), Game.load('juego.wasm'), font]);
    renderer = r;
    game = g;
    const [w, h] = renderer.resize();
    lastSize = [w, h];
    game.x.init(w, h, (Math.random() * 0xffffffff) >>> 0);
    game.x.set_glyph_capacity(renderer.atlas.capacity);
    renderer.syncGlyphs(game);
    initVoxels();
    playBtn.disabled = false;
    playText.textContent = '¡A jugar!';
    playBtn.focus();
  } catch (err) {
    console.error(err);
    const msg = String(err && err.message);
    if (msg === 'no-adapter') {
      showNotice('El navegador tiene WebGPU, pero no encontró una tarjeta de video compatible. Prueba actualizar el navegador y los controladores de video.');
    } else {
      showNotice(`No se pudo iniciar el juego: ${msg}`);
    }
    playText.textContent = 'Error';
  }
}

/// Compila el 3D en segundo plano y le avisa a Rust si quedó listo.
async function initVoxels() {
  const x = game.x;
  if (!x.d3_set_status) return;
  if (x.d3_cube_bytes() !== CUBE_BYTES || x.d3_scene_floats() !== SCENE_FLOATS) {
    console.error('3D: el formato de Rust y JS no coincide');
    x.d3_set_status(2);
    return;
  }
  const saved = parseInt(storageGet(QUALITY_KEY), 10);
  const level = Number.isInteger(saved) ? Math.min(Math.max(saved, 0), QUALITY.length - 1) : 0;
  renderer.voxels.onlevel = (l) => storageSet(QUALITY_KEY, String(l));
  const ok = await renderer.voxels.init(level);
  x.d3_set_status(ok ? 1 : 2);
}

async function enterFullscreen() {
  if (!fullscreenBox.checked) return;
  try {
    if (!document.fullscreenElement) await document.documentElement.requestFullscreen({ navigationUI: 'hide' });
  } catch (_) {}
  try {
    // Captura teclas como Esc, Ctrl+W o la tecla Windows mientras se juega.
    if (navigator.keyboard && navigator.keyboard.lock) await navigator.keyboard.lock();
  } catch (_) {}
}

function start() {
  if (!game || playing) return;
  synth.start();
  const name = nameInput.value.trim();
  storageSet(NAME_KEY, name);
  game.setName(name);
  enterFullscreen();
  cover.hidden = true;
  playing = true;
  canvas.focus();
  lastTime = performance.now();
  requestAnimationFrame(loop);
}

function pause() {
  if (!playing) return;
  playing = false;
  voice.hush();
  game.x.release_all();
  if (navigator.keyboard && navigator.keyboard.unlock) navigator.keyboard.unlock();
  if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
  playText.textContent = 'Seguir jugando';
  cover.hidden = false;
  playBtn.focus();
}

playBtn.addEventListener('click', start);

// Para pruebas automáticas y para curiosear desde la consola.
window.juego = {
  get modo() {
    return game ? game.x.current_mode() : -1;
  },
  /// 0 cargando, 1 listo, 2 no disponible.
  get d3() {
    return renderer ? renderer.voxels.status : 0;
  },
};

document.addEventListener('fullscreenchange', () => {
  // Si se sale de pantalla completa (Esc mantenido en Chrome/Edge), pausar.
  if (!document.fullscreenElement && playing && fullscreenBox.checked) pause();
});

function handleEvents() {
  for (const e of game.events()) {
    switch (e.type) {
      case EV.NOTE:
        synth.note(e.a, e.b, e.c, e.d);
        break;
      case EV.SFX:
        synth.sfx(e.a, e.b, e.c, e.d);
        break;
      case EV.SPEAK:
        voice.speak(e.text, true);
        break;
      case EV.HUSH:
        voice.hush();
        break;
      case EV.EXIT:
        pause();
        break;
    }
  }
  game.x.events_done();
}

function loop(now) {
  if (!playing) return;
  const rawDt = Math.max(0, (now - lastTime) / 1000);
  const dt = Math.min(0.05, rawDt);
  lastTime = now;
  const [w, h] = renderer.resize();
  if (w !== lastSize[0] || h !== lastSize[1]) {
    lastSize = [w, h];
    game.x.resize(w, h);
  }
  game.x.update(dt, voice.speaking ? 1 : 0);
  handleEvents();
  renderer.syncGlyphs(game);
  const frame = game.frame();
  renderer.render(now / 1000, frame);
  if (frame.d3) renderer.voxels.measure(rawDt);
  game.x.paint_done();
  requestAnimationFrame(loop);
}

// ---------------------------------------------------------------------------
// Entrada

function pointerPos(e) {
  return [e.clientX, e.clientY];
}

canvas.addEventListener('pointermove', (e) => {
  if (!playing) return;
  const list = e.getCoalescedEvents ? e.getCoalescedEvents() : [e];
  for (const ev of list.length ? list : [e]) game.x.pointer_move(...pointerPos(ev));
});

canvas.addEventListener('pointerdown', (e) => {
  if (!playing) return;
  e.preventDefault();
  try {
    canvas.setPointerCapture(e.pointerId);
  } catch (_) {}
  game.x.pointer_down(...pointerPos(e), Math.min(e.button, 2));
});

canvas.addEventListener('pointerup', (e) => {
  if (!playing) return;
  game.x.pointer_up(...pointerPos(e), Math.min(Math.max(e.button, 0), 2));
});

canvas.addEventListener('pointercancel', () => game && game.x.release_all());
canvas.addEventListener('pointerleave', () => playing && game.x.pointer_leave());
canvas.addEventListener('contextmenu', (e) => e.preventDefault());

window.addEventListener(
  'wheel',
  (e) => {
    if (!playing) return;
    e.preventDefault();
    const now = performance.now();
    if (now - wheelAt < 90) return;
    wheelAt = now;
    game.x.wheel(e.deltaY);
  },
  { passive: false }
);

function keyChar(e) {
  if (e.key && [...e.key].length === 1) return e.key.codePointAt(0);
  return 0;
}

window.addEventListener('keydown', (e) => {
  if (!playing) {
    if (e.key === 'Enter' && document.activeElement === nameInput) start();
    return;
  }
  e.preventDefault();
  const code = game.codes.get(e.code) || 0;
  game.x.key_down(code, keyChar(e), e.repeat ? 1 : 0);
});

window.addEventListener('keyup', (e) => {
  if (!playing) return;
  e.preventDefault();
  game.x.key_up(game.codes.get(e.code) || 0);
});

window.addEventListener('blur', () => game && game.x.release_all());
document.addEventListener('visibilitychange', () => {
  if (document.hidden && playing) {
    voice.hush();
    game.x.release_all();
  }
});

// Por si se aprieta Ctrl+W sin querer: el navegador pregunta antes de cerrar.
window.addEventListener('beforeunload', (e) => {
  if (playing) {
    e.preventDefault();
    e.returnValue = '';
  }
});

boot();
