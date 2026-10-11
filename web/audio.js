// Sintetizador con Web Audio: todos los sonidos se generan en el momento,
// no hay archivos de audio. Rust pide notas y efectos; aquí suenan.

// Deben coincidir con `Inst` y `Sfx` en src/audio.rs.
const INST = { MARIMBA: 0, BELL: 1, PIANO: 2, PLUCK: 3, DRUM: 4, SOFT: 5 };
const SFX = {
  POP: 1, BOING: 2, WHOOSH: 3, SPARKLE: 4, FANFARE: 5, WIGGLE: 6, BUZZ: 7,
  BLOOP: 8, OOPS: 9, TICK: 10, APPLAUSE: 11, MAGIC: 12, SPLASH: 13,
};

const MAX_VOICES = 40;

export class Synth {
  constructor() {
    this.ctx = null;
    this.voices = 0;
    this.volume = 0.8;
  }

  /// Debe llamarse dentro de un clic (los navegadores exigen un gesto).
  start() {
    if (!this.ctx) {
      const AC = window.AudioContext || window.webkitAudioContext;
      this.ctx = new AC({ latencyHint: 'interactive' });
      const ctx = this.ctx;
      this.master = ctx.createGain();
      this.master.gain.value = this.volume;
      const comp = ctx.createDynamicsCompressor();
      comp.threshold.value = -14;
      comp.ratio.value = 4;
      this.master.connect(comp).connect(ctx.destination);
      // Reverberación suave para que suene "a sala".
      this.reverb = ctx.createConvolver();
      this.reverb.buffer = this.impulse(1.6);
      const wet = ctx.createGain();
      wet.gain.value = 0.22;
      this.reverb.connect(wet).connect(this.master);
      this.noise = this.noiseBuffer();
    }
    if (this.ctx.state !== 'running') this.ctx.resume();
  }

  setVolume(v) {
    this.volume = v;
    if (this.master) this.master.gain.value = v;
  }

  impulse(seconds) {
    const ctx = this.ctx;
    const len = Math.floor(ctx.sampleRate * seconds);
    const buf = ctx.createBuffer(2, len, ctx.sampleRate);
    for (let ch = 0; ch < 2; ch++) {
      const d = buf.getChannelData(ch);
      for (let i = 0; i < len; i++) d[i] = (Math.random() * 2 - 1) * Math.pow(1 - i / len, 2.5);
    }
    return buf;
  }

  noiseBuffer() {
    const ctx = this.ctx;
    const buf = ctx.createBuffer(1, ctx.sampleRate, ctx.sampleRate);
    const d = buf.getChannelData(0);
    for (let i = 0; i < d.length; i++) d[i] = Math.random() * 2 - 1;
    return buf;
  }

  /// Salida de una voz: volumen -> paneo -> (seco + reverb).
  out(pan, gain, dur) {
    const ctx = this.ctx;
    const g = ctx.createGain();
    g.gain.value = gain;
    const p = ctx.createStereoPanner ? ctx.createStereoPanner() : null;
    if (p) {
      p.pan.value = Math.max(-1, Math.min(1, pan || 0));
      g.connect(p);
      p.connect(this.master);
      p.connect(this.reverb);
    } else {
      g.connect(this.master);
      g.connect(this.reverb);
    }
    this.voices++;
    setTimeout(() => {
      this.voices--;
      g.disconnect();
    }, (dur + 0.3) * 1000);
    return g;
  }

  osc(type, freq, t0, dur, dest) {
    const o = this.ctx.createOscillator();
    o.type = type;
    o.frequency.setValueAtTime(freq, t0);
    o.connect(dest);
    o.start(t0);
    o.stop(t0 + dur + 0.05);
    return o;
  }

  env(dest, t0, attack, peak, decay) {
    const g = this.ctx.createGain();
    g.gain.setValueAtTime(0.0001, t0);
    g.gain.exponentialRampToValueAtTime(peak, t0 + attack);
    g.gain.exponentialRampToValueAtTime(0.0001, t0 + attack + decay);
    g.connect(dest);
    return g;
  }

  noiseSrc(t0, dur, dest) {
    const s = this.ctx.createBufferSource();
    s.buffer = this.noise;
    s.loop = true;
    s.connect(dest);
    s.start(t0, Math.random() * 0.5);
    s.stop(t0 + dur + 0.05);
    return s;
  }

  filter(type, freq, q, dest) {
    const f = this.ctx.createBiquadFilter();
    f.type = type;
    f.frequency.value = freq;
    f.Q.value = q;
    f.connect(dest);
    return f;
  }

  busy() {
    return !this.ctx || this.ctx.state !== 'running' || this.voices > MAX_VOICES;
  }

  // ---- Instrumentos -------------------------------------------------------

  note(freq, inst, pan, vol) {
    if (this.busy()) return;
    const t = this.ctx.currentTime + 0.005;
    vol = vol || 0.6;
    switch (inst) {
      case INST.MARIMBA: {
        const out = this.out(pan, vol * 0.5, 1.2);
        this.osc('sine', freq, t, 1.0, this.env(out, t, 0.004, 1.0, 0.9));
        this.osc('sine', freq * 4, t, 0.2, this.env(out, t, 0.002, 0.25, 0.12));
        this.osc('triangle', freq * 2, t, 0.4, this.env(out, t, 0.003, 0.15, 0.3));
        break;
      }
      case INST.BELL: {
        const out = this.out(pan, vol * 0.35, 2.2);
        const car = this.ctx.createOscillator();
        const mod = this.ctx.createOscillator();
        const modGain = this.ctx.createGain();
        car.frequency.value = freq;
        mod.frequency.value = freq * 3.5;
        modGain.gain.setValueAtTime(freq * 2.5, t);
        modGain.gain.exponentialRampToValueAtTime(1, t + 1.5);
        mod.connect(modGain).connect(car.frequency);
        car.connect(this.env(out, t, 0.003, 1.0, 2.0));
        car.start(t);
        mod.start(t);
        car.stop(t + 2.1);
        mod.stop(t + 2.1);
        break;
      }
      case INST.PIANO: {
        const out = this.out(pan, vol * 0.45, 1.6);
        const lp = this.filter('lowpass', Math.min(freq * 6, 9000), 0.7, out);
        this.osc('triangle', freq, t, 1.5, this.env(lp, t, 0.005, 1.0, 1.4));
        this.osc('sine', freq * 2, t, 0.8, this.env(lp, t, 0.005, 0.35, 0.7));
        this.osc('sawtooth', freq, t, 0.3, this.env(lp, t, 0.002, 0.08, 0.25));
        break;
      }
      case INST.PLUCK: {
        const out = this.out(pan, vol * 0.4, 1.3);
        const lp = this.ctx.createBiquadFilter();
        lp.type = 'lowpass';
        lp.Q.value = 2;
        lp.frequency.setValueAtTime(freq * 8, t);
        lp.frequency.exponentialRampToValueAtTime(freq * 1.2, t + 0.4);
        lp.connect(this.env(out, t, 0.003, 1.0, 1.1));
        this.osc('sawtooth', freq, t, 1.2, lp);
        this.osc('square', freq * 1.003, t, 1.2, lp);
        break;
      }
      case INST.DRUM:
        this.drum(Math.round(freq) % 8, t, pan, vol);
        break;
      default: {
        // SOFT: tono suave con vibrato.
        const out = this.out(pan, vol * 0.4, 0.9);
        const o = this.osc('sine', freq, t, 0.8, this.env(out, t, 0.04, 1.0, 0.7));
        const lfo = this.ctx.createOscillator();
        const lg = this.ctx.createGain();
        lfo.frequency.value = 6;
        lg.gain.value = freq * 0.01;
        lfo.connect(lg).connect(o.frequency);
        lfo.start(t);
        lfo.stop(t + 0.85);
      }
    }
  }

  drum(kind, t, pan, vol) {
    const ctx = this.ctx;
    switch (kind) {
      case 0: { // bombo
        const out = this.out(pan, vol * 0.9, 0.5);
        const o = ctx.createOscillator();
        o.frequency.setValueAtTime(160, t);
        o.frequency.exponentialRampToValueAtTime(45, t + 0.3);
        o.connect(this.env(out, t, 0.002, 1.0, 0.4));
        o.start(t);
        o.stop(t + 0.45);
        break;
      }
      case 1: { // caja
        const out = this.out(pan, vol * 0.5, 0.3);
        this.noiseSrc(t, 0.2, this.filter('bandpass', 1800, 0.8, this.env(out, t, 0.002, 1.0, 0.18)));
        this.osc('triangle', 190, t, 0.12, this.env(out, t, 0.002, 0.6, 0.1));
        break;
      }
      case 2: { // platillo cerrado
        const out = this.out(pan, vol * 0.3, 0.15);
        this.noiseSrc(t, 0.08, this.filter('highpass', 7000, 0.5, this.env(out, t, 0.001, 1.0, 0.06)));
        break;
      }
      case 3:
      case 4:
      case 5: { // timbales
        const base = [200, 150, 110][kind - 3];
        const out = this.out(pan, vol * 0.7, 0.6);
        const o = ctx.createOscillator();
        o.frequency.setValueAtTime(base * 1.6, t);
        o.frequency.exponentialRampToValueAtTime(base, t + 0.15);
        o.connect(this.env(out, t, 0.002, 1.0, 0.5));
        o.start(t);
        o.stop(t + 0.55);
        break;
      }
      case 6: { // aplauso
        const out = this.out(pan, vol * 0.6, 0.3);
        for (let i = 0; i < 3; i++) {
          this.noiseSrc(t + i * 0.012, 0.1, this.filter('bandpass', 1200, 1.2, this.env(out, t + i * 0.012, 0.001, 1.0, 0.08)));
        }
        break;
      }
      default: { // cencerro
        const out = this.out(pan, vol * 0.3, 0.6);
        const bp = this.filter('bandpass', 800, 3, this.env(out, t, 0.002, 1.0, 0.5));
        this.osc('square', 540, t, 0.5, bp);
        this.osc('square', 800, t, 0.5, bp);
      }
    }
  }

  // ---- Efectos ------------------------------------------------------------

  sfx(id, pitch, pan, vol) {
    if (this.busy()) return;
    const ctx = this.ctx;
    const t = ctx.currentTime + 0.005;
    pitch = pitch || 1;
    vol = vol === undefined ? 1 : vol;
    switch (id) {
      case SFX.POP: {
        const out = this.out(pan, 0.5 * vol, 0.25);
        const o = ctx.createOscillator();
        o.frequency.setValueAtTime(900 * pitch, t);
        o.frequency.exponentialRampToValueAtTime(180 * pitch, t + 0.12);
        o.connect(this.env(out, t, 0.002, 1.0, 0.13));
        o.start(t);
        o.stop(t + 0.2);
        this.noiseSrc(t, 0.03, this.filter('highpass', 3000, 0.5, this.env(out, t, 0.001, 0.4, 0.02)));
        break;
      }
      case SFX.BOING: {
        const out = this.out(pan, 0.4 * vol, 0.6);
        const o = ctx.createOscillator();
        o.type = 'triangle';
        o.frequency.setValueAtTime(150 * pitch, t);
        o.frequency.exponentialRampToValueAtTime(520 * pitch, t + 0.12);
        o.frequency.exponentialRampToValueAtTime(260 * pitch, t + 0.45);
        const lfo = ctx.createOscillator();
        const lg = ctx.createGain();
        lfo.frequency.value = 18;
        lg.gain.value = 25 * pitch;
        lfo.connect(lg).connect(o.frequency);
        o.connect(this.env(out, t, 0.005, 1.0, 0.5));
        o.start(t);
        lfo.start(t);
        o.stop(t + 0.55);
        lfo.stop(t + 0.55);
        break;
      }
      case SFX.WHOOSH:
      case SFX.SPLASH: {
        const dur = id === SFX.SPLASH ? 0.9 : 0.5;
        const out = this.out(pan, 0.45 * vol, dur);
        const bp = ctx.createBiquadFilter();
        bp.type = 'bandpass';
        bp.Q.value = 1.2;
        bp.frequency.setValueAtTime(300 * pitch, t);
        bp.frequency.exponentialRampToValueAtTime(3500 * pitch, t + dur * 0.6);
        bp.frequency.exponentialRampToValueAtTime(800 * pitch, t + dur);
        bp.connect(this.env(out, t, dur * 0.3, 1.0, dur * 0.7));
        this.noiseSrc(t, dur, bp);
        break;
      }
      case SFX.SPARKLE:
      case SFX.MAGIC: {
        const notes = id === SFX.MAGIC ? [0, 4, 7, 12, 16, 19, 24] : [12, 19, 24, 16, 28];
        const out = this.out(pan, 0.25 * vol, 1.2);
        notes.forEach((n, i) => {
          const f = 523.25 * pitch * Math.pow(2, n / 12);
          const tt = t + i * (id === SFX.MAGIC ? 0.05 : 0.07);
          this.osc('sine', f, tt, 0.5, this.env(out, tt, 0.003, 0.8, 0.45));
        });
        break;
      }
      case SFX.FANFARE: {
        const out = this.out(pan, 0.35 * vol, 1.6);
        [0, 4, 7, 12].forEach((n, i) => {
          const f = 392 * Math.pow(2, n / 12);
          const tt = t + i * 0.11;
          const len = i === 3 ? 0.9 : 0.25;
          this.osc('triangle', f, tt, len, this.env(out, tt, 0.01, 1.0, len));
          this.osc('sine', f * 2, tt, len, this.env(out, tt, 0.01, 0.3, len));
        });
        break;
      }
      case SFX.WIGGLE: {
        const out = this.out(pan, 0.25 * vol, 0.5);
        const o = ctx.createOscillator();
        o.frequency.setValueAtTime(600 * pitch, t);
        o.frequency.exponentialRampToValueAtTime(400 * pitch, t + 0.35);
        const lfo = ctx.createOscillator();
        const lg = ctx.createGain();
        lfo.frequency.value = 12;
        lg.gain.value = 60 * pitch;
        lfo.connect(lg).connect(o.frequency);
        o.connect(this.env(out, t, 0.01, 1.0, 0.35));
        o.start(t);
        lfo.start(t);
        o.stop(t + 0.4);
        lfo.stop(t + 0.4);
        break;
      }
      case SFX.BUZZ: {
        const out = this.out(pan, 0.12 * vol, 0.6);
        const lp = this.filter('lowpass', 900, 1, this.env(out, t, 0.05, 1.0, 0.45));
        const o = this.osc('sawtooth', 170 * pitch, t, 0.5, lp);
        const lfo = ctx.createOscillator();
        const lg = ctx.createGain();
        lfo.frequency.value = 9;
        lg.gain.value = 12;
        lfo.connect(lg).connect(o.frequency);
        lfo.start(t);
        lfo.stop(t + 0.55);
        break;
      }
      case SFX.BLOOP: {
        const out = this.out(pan, 0.35 * vol, 0.3);
        const o = ctx.createOscillator();
        o.frequency.setValueAtTime(250 * pitch, t);
        o.frequency.exponentialRampToValueAtTime(700 * pitch, t + 0.1);
        o.connect(this.env(out, t, 0.005, 1.0, 0.15));
        o.start(t);
        o.stop(t + 0.2);
        break;
      }
      case SFX.OOPS: {
        // "Uh-oh" suave, nunca un sonido de error feo.
        const out = this.out(pan, 0.3 * vol, 0.6);
        this.osc('triangle', 440, t, 0.18, this.env(out, t, 0.01, 1.0, 0.16));
        this.osc('triangle', 330, t + 0.2, 0.3, this.env(out, t + 0.2, 0.01, 1.0, 0.28));
        break;
      }
      case SFX.TICK: {
        const out = this.out(pan, 0.2 * vol, 0.1);
        this.osc('sine', 1500 * pitch, t, 0.04, this.env(out, t, 0.001, 1.0, 0.04));
        break;
      }
      case SFX.APPLAUSE: {
        const out = this.out(pan, 0.5 * vol, 2.2);
        for (let i = 0; i < 40; i++) {
          const tt = t + Math.random() * 1.8;
          this.noiseSrc(tt, 0.05, this.filter('bandpass', 900 + Math.random() * 1500, 1.5, this.env(out, tt, 0.001, 0.6, 0.04)));
        }
        this.sfx(SFX.FANFARE, 1, pan, vol * 0.8);
        break;
      }
    }
  }
}
