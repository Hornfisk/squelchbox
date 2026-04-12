//! Temporary 1-pole TPT lowpass used in M1 to prove the voice pipeline.
//! Replaced by the authentic 3-pole diode ladder in M2 (`filter_diode.rs`).
//!
//! TPT (Topology-Preserving Transform) form from Zavalishin's
//! *The Art of VA Filter Design*: stable at any cutoff/sample-rate, no
//! nested denormals, branch-free hot path.

use std::f32::consts::PI;

pub struct OnePoleLp {
    z: f32,
    g_over_1_plus_g: f32,
    sr: f32,
}

impl OnePoleLp {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            z: 0.0,
            g_over_1_plus_g: 0.0,
            sr: sample_rate,
        };
        s.set_cutoff(1_000.0);
        s
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sr = sr;
    }

    pub fn reset(&mut self) {
        self.z = 0.0;
    }

    pub fn set_cutoff(&mut self, hz: f32) {
        let hz = hz.clamp(20.0, self.sr * 0.45);
        // Pre-warped cutoff via bilinear transform.
        let wd = 2.0 * PI * hz;
        let t = 1.0 / self.sr;
        let wa = (2.0 / t) * (wd * t * 0.5).tan();
        let g = wa * t * 0.5;
        self.g_over_1_plus_g = g / (1.0 + g);
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let v = (x - self.z) * self.g_over_1_plus_g;
        let y = v + self.z;
        self.z = y + v;
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn dc_passes() {
        let mut f = OnePoleLp::new(SR);
        f.set_cutoff(500.0);
        let mut y = 0.0;
        for _ in 0..10_000 {
            y = f.process(1.0);
        }
        assert!((y - 1.0).abs() < 0.01, "DC should pass, got {y}");
    }

    #[test]
    fn attenuates_above_cutoff() {
        let mut f = OnePoleLp::new(SR);
        f.set_cutoff(200.0);
        // Feed a 2 kHz sine — should come out much smaller than input.
        let freq = 2_000.0;
        let omega = 2.0 * PI * freq / SR;
        let mut peak_in = 0.0f32;
        let mut peak_out = 0.0f32;
        for i in 0..4_000 {
            let x = (omega * i as f32).sin();
            let y = f.process(x);
            if i > 500 {
                peak_in = peak_in.max(x.abs());
                peak_out = peak_out.max(y.abs());
            }
        }
        // 1-pole at fc=200, f=2k → one decade up, -20 dB nominal.
        let ratio = peak_out / peak_in;
        assert!(ratio < 0.15, "expected strong attenuation, got ratio {ratio}");
    }

    #[test]
    fn stable_at_extreme_cutoffs() {
        let mut f = OnePoleLp::new(SR);
        for fc in [20.0, 50.0, 500.0, 5_000.0, 20_000.0, 21_500.0] {
            f.set_cutoff(fc);
            for _ in 0..1_000 {
                let y = f.process(1.0);
                assert!(y.is_finite(), "non-finite at fc={fc}");
            }
        }
    }
}
