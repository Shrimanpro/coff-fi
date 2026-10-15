use std::f32::consts::PI;

use crate::settings::{self, EQ_FREQS};

const Q: f32 = 1.41;

/// RBJ peaking biquad, transposed direct form II, stereo state.
#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: [f32; 2],
    z2: [f32; 2],
}

impl Biquad {
    fn peaking(freq: f32, gain_db: f32, rate: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * PI * freq / rate;
        let alpha = w.sin() / (2.0 * Q);
        let cos = w.cos();
        let a0 = 1.0 + alpha / a;
        Self {
            b0: (1.0 + alpha * a) / a0,
            b1: (-2.0 * cos) / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha / a) / a0,
            ..Default::default()
        }
    }

    #[inline]
    fn tick(&mut self, x: f32, ch: usize) -> f32 {
        let y = self.b0 * x + self.z1[ch];
        self.z1[ch] = self.b1 * x - self.a1 * y + self.z2[ch];
        self.z2[ch] = self.b2 * x - self.a2 * y;
        y
    }

    /// Magnitude response in dB at `freq`.
    #[cfg(test)]
    fn response_db(&self, freq: f32, rate: f32) -> f32 {
        let w = 2.0 * PI * freq / rate;
        let (c1, s1, c2, s2) = (w.cos(), w.sin(), (2.0 * w).cos(), (2.0 * w).sin());
        let num = ((self.b0 + self.b1 * c1 + self.b2 * c2).powi(2) + (self.b1 * s1 + self.b2 * s2).powi(2)).sqrt();
        let den = ((1.0 + self.a1 * c1 + self.a2 * c2).powi(2) + (self.a1 * s1 + self.a2 * s2).powi(2)).sqrt();
        20.0 * (num / den).log10()
    }
}

pub struct Equalizer {
    bands: Vec<Biquad>,
    preamp: f32,
    enabled: bool,
}

impl Equalizer {
    pub fn new(cfg: &settings::Eq, rate: u32) -> Self {
        let rate = rate as f32;
        let bands = EQ_FREQS
            .iter()
            .zip(cfg.bands)
            // Flat bands cost nothing; bands near Nyquist are skipped to stay stable.
            .filter(|(f, g)| g.abs() > 0.05 && **f < rate * 0.45)
            .map(|(f, g)| Biquad::peaking(*f, g.clamp(-12.0, 12.0), rate))
            .collect();
        Self { bands, preamp: 10f32.powf(cfg.preamp / 20.0), enabled: cfg.enabled }
    }

    /// Processes interleaved stereo in place.
    pub fn process(&mut self, buf: &mut [f32]) {
        if !self.enabled {
            return;
        }
        for frame in buf.chunks_exact_mut(2) {
            for (ch, s) in frame.iter_mut().enumerate() {
                let mut x = *s * self.preamp;
                for b in &mut self.bands {
                    x = b.tick(x, ch);
                }
                *s = x;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaking_gain_at_center() {
        for g in [-9.0, 3.0, 12.0] {
            let b = Biquad::peaking(1000.0, g, 48000.0);
            assert!((b.response_db(1000.0, 48000.0) - g).abs() < 0.05, "gain {g}");
            assert!(b.response_db(20.0, 48000.0).abs() < 0.3, "far bands stay flat");
        }
    }

    #[test]
    fn disabled_is_bit_exact() {
        let cfg = settings::Eq { enabled: false, bands: [6.0; 10], ..Default::default() };
        let mut eq = Equalizer::new(&cfg, 44100);
        let mut buf = vec![0.25, -0.5, 0.1, 0.2];
        eq.process(&mut buf);
        assert_eq!(buf, vec![0.25, -0.5, 0.1, 0.2]);
    }

    #[test]
    fn boosted_low_band_amplifies_bass_sine() {
        let mut cfg = settings::Eq { enabled: true, ..Default::default() };
        cfg.bands[2] = 6.0; // 125 Hz
        let mut eq = Equalizer::new(&cfg, 48000);
        let mut buf: Vec<f32> = (0..48000).flat_map(|i| { let s = (2.0 * PI * 125.0 * i as f32 / 48000.0).sin() * 0.1; [s, s] }).collect();
        eq.process(&mut buf);
        let peak = buf[24000..].iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!((peak / 0.1 - 2.0).abs() < 0.1, "≈ +6 dB, got {}", peak / 0.1);
    }
}
