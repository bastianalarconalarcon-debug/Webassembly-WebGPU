// Voz en español con SpeechSynthesis (viene incluida en el navegador).
// Se prefiere una voz de Chile; en Microsoft Edge existen voces neuronales
// es-CL ("Catalina", "Lorenzo") que suenan muy naturales.

const LANG_PREFERENCE = ['es-CL', 'es-419', 'es-AR', 'es-PE', 'es-CO', 'es-MX', 'es-US', 'es-ES'];
const STORAGE_KEY = 'juego.voz';

function score(v) {
  const lang = (v.lang || '').replace('_', '-');
  let s = 0;
  const i = LANG_PREFERENCE.findIndex((l) => lang.toLowerCase() === l.toLowerCase());
  if (i >= 0) s += 100 - i * 10;
  else if (lang.toLowerCase().startsWith('es')) s += 20;
  else return -1;
  if (/natural|neural|online/i.test(v.name)) s += 25; // voces neuronales (Edge)
  if (/google/i.test(v.name)) s += 8;
  if (v.localService) s += 2;
  return s;
}

export class Voice {
  constructor() {
    this.supported = 'speechSynthesis' in window;
    this.voice = null;
    this.voices = [];
    this.rate = 0.95;
    this.pitch = 1.1;
    this.onvoices = null;
    if (!this.supported) return;
    const load = () => {
      this.voices = speechSynthesis
        .getVoices()
        .filter((v) => score(v) >= 0)
        .sort((a, b) => score(b) - score(a));
      let saved = null;
      try {
        saved = localStorage.getItem(STORAGE_KEY);
      } catch (_) {}
      this.voice = this.voices.find((v) => v.name === saved) || this.voices[0] || null;
      if (this.onvoices) this.onvoices(this.voices, this.voice);
    };
    load();
    speechSynthesis.addEventListener('voiceschanged', load);
  }

  choose(name) {
    this.voice = this.voices.find((v) => v.name === name) || this.voice;
    try {
      localStorage.setItem(STORAGE_KEY, name);
    } catch (_) {}
  }

  get speaking() {
    return this.supported && (speechSynthesis.speaking || speechSynthesis.pending);
  }

  speak(text, interrupt = true) {
    if (!this.supported || !text) return;
    if (interrupt) speechSynthesis.cancel();
    const u = new SpeechSynthesisUtterance(text);
    if (this.voice) {
      u.voice = this.voice;
      u.lang = this.voice.lang;
    } else {
      u.lang = 'es-CL';
    }
    u.rate = this.rate;
    u.pitch = this.pitch;
    speechSynthesis.speak(u);
  }

  hush() {
    if (this.supported) speechSynthesis.cancel();
  }
}
