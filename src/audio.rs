//! Sonido y voz. Rust no reproduce audio: deja "eventos" en una cola que
//! JavaScript lee cada cuadro y convierte en Web Audio (sintetizador) o en
//! SpeechSynthesis (voz en español).

/// Tipos de evento. Deben coincidir con `web/main.js`.
pub const EV_NOTE: f32 = 1.0;
pub const EV_SFX: f32 = 2.0;
pub const EV_SPEAK: f32 = 3.0;
pub const EV_HUSH: f32 = 4.0;
pub const EV_EXIT: f32 = 5.0;

/// Instrumentos del sintetizador (`web/audio.js`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inst {
    Marimba = 0,
    Bell = 1,
    Piano = 2,
    Pluck = 3,
    /// Batería: la "frecuencia" elige el tambor (ver audio.js).
    Drum = 4,
    Soft = 5,
}

/// Efectos de sonido (`web/audio.js`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    Pop = 1,
    Boing = 2,
    Whoosh = 3,
    Sparkle = 4,
    Fanfare = 5,
    Wiggle = 6,
    Buzz = 7,
    Bloop = 8,
    Oops = 9,
    Tick = 10,
    Applause = 11,
    Magic = 12,
    Splash = 13,
}

pub struct Audio {
    pub events: Vec<[f32; 6]>,
    pub text: Vec<u8>,
    /// Ancho de pantalla, para ubicar el sonido a la izquierda/derecha.
    pub width: f32,
    /// JS nos avisa si la voz está hablando.
    pub speaking: bool,
    pending: Option<(f32, String)>,
    since_say: f32,
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}

impl Audio {
    pub fn new() -> Audio {
        Audio {
            events: Vec::with_capacity(64),
            text: Vec::with_capacity(1024),
            width: 1.0,
            speaking: false,
            pending: None,
            since_say: 99.0,
        }
    }

    fn pan(&self, x: f32) -> f32 {
        ((x / self.width.max(1.0)) * 2.0 - 1.0).clamp(-1.0, 1.0) * 0.6
    }

    pub fn note(&mut self, freq: f32, inst: Inst, x: f32, vol: f32) {
        if self.events.len() < 256 {
            let pan = self.pan(x);
            self.events
                .push([EV_NOTE, freq, inst as u32 as f32, pan, vol, 0.0]);
        }
    }

    pub fn sfx(&mut self, s: Sfx, pitch: f32, x: f32) {
        if self.events.len() < 256 {
            let pan = self.pan(x);
            self.events
                .push([EV_SFX, s as u32 as f32, pitch, pan, 1.0, 0.0]);
        }
    }

    pub fn sfx_vol(&mut self, s: Sfx, pitch: f32, x: f32, vol: f32) {
        if self.events.len() < 256 {
            let pan = self.pan(x);
            self.events
                .push([EV_SFX, s as u32 as f32, pitch, pan, vol, 0.0]);
        }
    }

    fn push_speech(&mut self, text: &str) {
        let off = self.text.len() as f32;
        self.text.extend_from_slice(text.as_bytes());
        self.events
            .push([EV_SPEAK, off, text.len() as f32, 1.0, 0.0, 0.0]);
        self.since_say = 0.0;
    }

    /// Habla ahora, interrumpiendo lo que se estuviera diciendo.
    pub fn say(&mut self, text: &str) {
        self.pending = None;
        self.push_speech(text);
    }

    /// Habla después de `delay` segundos, si nada lo reemplaza antes.
    pub fn say_after(&mut self, delay: f32, text: &str) {
        self.pending = Some((delay, text.to_string()));
    }

    /// Habla sólo si la voz está callada (para frases no importantes).
    pub fn say_if_quiet(&mut self, text: &str) -> bool {
        if self.speaking || self.pending.is_some() || self.since_say < 0.3 {
            return false;
        }
        self.say(text);
        true
    }

    /// Cancela la voz y cualquier frase pendiente.
    pub fn hush(&mut self) {
        self.pending = None;
        self.events.push([EV_HUSH, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    pub fn cancel_pending(&mut self) {
        self.pending = None;
    }

    pub fn exit_request(&mut self) {
        self.events.push([EV_EXIT, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    /// Segundos desde la última frase.
    pub fn since_say(&self) -> f32 {
        self.since_say
    }

    pub fn tick(&mut self, dt: f32) {
        self.since_say += dt;
        if let Some((t, _)) = self.pending.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                let (_, text) = self.pending.take().unwrap();
                self.push_speech(&text);
            }
        }
    }

    /// JS llama a esto después de leer los eventos.
    pub fn clear(&mut self) {
        self.events.clear();
        self.text.clear();
    }

    /// Frases emitidas en esta tanda (para pruebas).
    #[cfg(test)]
    pub fn spoken(&self) -> Vec<String> {
        self.events
            .iter()
            .filter(|e| e[0] == EV_SPEAK)
            .map(|e| {
                let (o, l) = (e[1] as usize, e[2] as usize);
                String::from_utf8(self.text[o..o + l].to_vec()).unwrap()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_speech_is_replaced() {
        let mut a = Audio::new();
        a.say_after(0.5, "uno");
        a.tick(0.3);
        a.say_after(0.5, "dos");
        a.tick(0.3);
        assert!(a.spoken().is_empty());
        a.tick(0.3);
        assert_eq!(a.spoken(), vec!["dos".to_string()]);
    }

    #[test]
    fn say_cancels_pending() {
        let mut a = Audio::new();
        a.say_after(0.1, "tarde");
        a.say("ahora");
        a.tick(1.0);
        assert_eq!(a.spoken(), vec!["ahora".to_string()]);
    }
}
