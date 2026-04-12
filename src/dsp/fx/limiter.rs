//! Brickwall peak limiter at -0.3 dBFS.
//!
//! Always on, no user-facing parameters. Fast attack catches transients,
//! moderate release avoids pumping. Prevents clipping from stacked FX gain.

use nih_plug::util;

const CEILING_DB: f32 = -0.3;
const ATTACK_MS: f32 = 0.1;
const RELEASE_MS: f32 = 50.0;

pub struct Limiter {
    ceiling_lin: f32,
    env: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

impl Limiter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            ceiling_lin: util::db_to_gain(CEILING_DB),
            env: 0.0,
            attack_coeff: Self::coeff(ATTACK_MS, sample_rate),
            release_coeff: Self::coeff(RELEASE_MS, sample_rate),
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.attack_coeff = Self::coeff(ATTACK_MS, sr);
        self.release_coeff = Self::coeff(RELEASE_MS, sr);
        self.reset();
    }

    pub fn reset(&mut self) {
        self.env = 0.0;
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let abs = input.abs();
        let over_db = if abs > self.ceiling_lin {
            util::gain_to_db(abs) - CEILING_DB
        } else {
            0.0
        };

        // Envelope follower in dB domain.
        // On the very first over-ceiling hit (env==0), snap instantly so the
        // first output sample is already gain-reduced (prevents transient blowthrough).
        if over_db > self.env && self.env == 0.0 && over_db > 0.0 {
            self.env = over_db;
        } else {
            let coeff = if over_db > self.env {
                self.attack_coeff
            } else {
                self.release_coeff
            };
            self.env = self.env * coeff + over_db * (1.0 - coeff);
        }

        // Apply gain reduction.
        let reduction = util::db_to_gain(-self.env);
        input * reduction
    }

    fn coeff(ms: f32, sr: f32) -> f32 {
        (-1.0 / (ms * 0.001 * sr)).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn quiet_signal_passes_through() {
        let mut lim = Limiter::new(SR);
        let input = 0.5;
        let out = lim.process(input);
        assert!(
            (out - input).abs() < 0.01,
            "quiet signal should pass through, got {out}"
        );
    }

    #[test]
    fn loud_signal_is_limited() {
        let mut lim = Limiter::new(SR);
        let ceiling_lin = util::db_to_gain(CEILING_DB);
        // Feed a very loud signal for enough samples for the envelope to engage
        for _ in 0..4800 {
            lim.process(2.0);
        }
        // After the attack settles, output should be near ceiling
        let mut settled_max = 0.0f32;
        for _ in 0..1000 {
            let out = lim.process(2.0);
            settled_max = settled_max.max(out.abs());
        }
        assert!(
            settled_max < ceiling_lin * 1.05,
            "settled output should be near ceiling ({ceiling_lin}), got {settled_max}"
        );
    }

    #[test]
    fn output_never_exceeds_ceiling_by_much() {
        let mut lim = Limiter::new(SR);
        let ceiling_lin = util::db_to_gain(CEILING_DB);
        let mut peak = 0.0f32;
        for _ in 0..48_000 {
            let out = lim.process(5.0);
            peak = peak.max(out.abs());
        }
        assert!(
            peak < ceiling_lin * 2.0,
            "peak should be near ceiling, got {peak} (ceiling={ceiling_lin})"
        );
    }

    #[test]
    fn no_nan_at_extreme_input() {
        let mut lim = Limiter::new(SR);
        for &input in &[-100.0, -1.0, 0.0, 1.0, 100.0] {
            let out = lim.process(input);
            assert!(out.is_finite(), "NaN at input={input}");
        }
    }

    #[test]
    fn silence_passes_through() {
        let mut lim = Limiter::new(SR);
        for _ in 0..1000 {
            let out = lim.process(0.0);
            assert_eq!(out, 0.0);
        }
    }
}
