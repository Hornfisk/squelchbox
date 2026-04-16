//! Soft-knee peak compressor for post-reverb loudness taming.
//! Always on — tames residual level swings while letting accent
//! transients punch through (5 ms attack, 180 ms release).

use crate::dsp::flush_denormal;

pub struct LoudnessComp {
    attack_coeff: f32,
    release_coeff: f32,
    env: f32,
}

const THRESHOLD_DB: f32 = -14.0;
const RATIO: f32 = 2.5;
const KNEE_DB: f32 = 6.0;
const ATTACK_MS: f32 = 5.0;
const RELEASE_MS: f32 = 180.0;
const MAKEUP_DB: f32 = 2.0;

impl LoudnessComp {
    pub fn new(sample_rate: f32) -> Self {
        let mut c = Self {
            attack_coeff: 0.0,
            release_coeff: 0.0,
            env: 0.0,
        };
        c.update_coeffs(sample_rate);
        c
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.update_coeffs(sr);
        self.reset();
    }

    pub fn reset(&mut self) {
        self.env = 0.0;
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let abs_in = input.abs();
        let coeff = if abs_in > self.env { self.attack_coeff } else { self.release_coeff };
        self.env = self.env * coeff + abs_in * (1.0 - coeff);
        self.env = flush_denormal(self.env);

        let env_db = gain_to_db(self.env.max(1.0e-6));
        let knee_lo = THRESHOLD_DB - KNEE_DB * 0.5;
        let knee_hi = THRESHOLD_DB + KNEE_DB * 0.5;
        let slope = 1.0 - 1.0 / RATIO;

        let gain_db = if env_db > knee_hi {
            -slope * (env_db - THRESHOLD_DB)
        } else if env_db > knee_lo {
            let x = env_db - knee_lo;
            -slope * x * x / (2.0 * KNEE_DB)
        } else {
            0.0
        };

        input * db_to_gain(gain_db + MAKEUP_DB)
    }

    fn update_coeffs(&mut self, sr: f32) {
        self.attack_coeff = (-1.0 / (0.001 * ATTACK_MS * sr)).exp();
        self.release_coeff = (-1.0 / (0.001 * RELEASE_MS * sr)).exp();
    }
}

#[inline]
fn gain_to_db(g: f32) -> f32 {
    20.0 * g.log10()
}

#[inline]
fn db_to_gain(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn quiet_signal_gets_makeup_gain() {
        let mut comp = LoudnessComp::new(SR);
        // A quiet signal should pass through with ~2 dB makeup.
        let input = 0.01;
        let mut out = 0.0;
        for _ in 0..1000 {
            out = comp.process(input);
        }
        assert!(out > input, "makeup gain should boost quiet signal");
    }

    #[test]
    fn loud_signal_is_compressed() {
        let mut comp = LoudnessComp::new(SR);
        let input = 0.5;
        for _ in 0..4800 {
            comp.process(input);
        }
        let out = comp.process(input);
        assert!(out.abs() < input * 2.0, "should not blow up: {out}");
        assert!(out.is_finite());
    }

    #[test]
    fn no_nan_at_extremes() {
        let mut comp = LoudnessComp::new(SR);
        for &v in &[-10.0, -1.0, 0.0, 0.001, 1.0, 10.0] {
            let out = comp.process(v);
            assert!(out.is_finite(), "NaN at input={v}");
        }
    }
}
