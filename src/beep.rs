use sdl2::audio::{AudioCallback, AudioSpec};

/// A simple square wave.
/// Adapted from sdl2::audio sample code.
///
/// A good tool for testing tone changes is https://onlinetonegenerator.com/?waveform=square
pub struct Beep {
    phase_inc: f32,
    phase: f32,
    volume: f32,
}

impl From<AudioSpec> for Beep {
    fn from(spec: AudioSpec) -> Self {
        Beep {
            phase_inc: 120.0 / spec.freq as f32,
            phase: 0.0,
            volume: 0.10,
        }
    }
}

impl AudioCallback for Beep {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        // Generate a square wave
        for x in out.iter_mut() {
            *x = if self.phase <= 0.5 {
                self.volume
            } else {
                -self.volume
            };
            self.phase = (self.phase + self.phase_inc) % 1.0;
        }
    }
}
