//! Schroeder room reverb — 4 parallel comb filters + 2 series allpass.
//!
//! Tuned for short, dense ambient room character. No cathedral tails.
//! Comb delay lengths are mutually prime to avoid coloring.

/// Comb filter delay lengths at 48 kHz. Mutually prime.
const COMB_DELAYS_48K: [usize; 4] = [1117, 1188, 1277, 1356];

/// Allpass delay lengths at 48 kHz. Mutually prime to combs and each other.
const AP_DELAYS_48K: [usize; 2] = [556, 441];

/// Fixed allpass coefficient.
const AP_COEFF: f32 = 0.5;

/// Fixed pre-delay in samples at 48 kHz (~10 ms).
const PREDELAY_48K: usize = 480;

/// Maximum buffer size per line (generous headroom for 96 kHz).
const MAX_LINE: usize = 4096;

struct CombFilter {
    buffer: [f32; MAX_LINE],
    len: usize,
    pos: usize,
    feedback: f32,
}

impl CombFilter {
    fn new(delay: usize) -> Self {
        Self {
            buffer: [0.0; MAX_LINE],
            len: delay.min(MAX_LINE),
            pos: 0,
            feedback: 0.5,
        }
    }

    fn set_feedback(&mut self, g: f32) {
        self.feedback = g;
    }

    fn set_delay(&mut self, samples: usize) {
        self.len = samples.clamp(1, MAX_LINE);
    }

    fn reset(&mut self) {
        self.buffer = [0.0; MAX_LINE];
        self.pos = 0;
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let out = self.buffer[self.pos];
        self.buffer[self.pos] = input + out * self.feedback;
        self.pos = (self.pos + 1) % self.len;
        out
    }
}

struct AllpassFilter {
    buffer: [f32; MAX_LINE],
    len: usize,
    pos: usize,
}

impl AllpassFilter {
    fn new(delay: usize) -> Self {
        Self {
            buffer: [0.0; MAX_LINE],
            len: delay.min(MAX_LINE),
            pos: 0,
        }
    }

    fn set_delay(&mut self, samples: usize) {
        self.len = samples.clamp(1, MAX_LINE);
    }

    fn reset(&mut self) {
        self.buffer = [0.0; MAX_LINE];
        self.pos = 0;
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.pos];
        let node = input - AP_COEFF * delayed;
        self.buffer[self.pos] = node;
        self.pos = (self.pos + 1) % self.len;
        delayed + AP_COEFF * node
    }
}

struct PreDelay {
    buffer: [f32; MAX_LINE],
    len: usize,
    pos: usize,
}

impl PreDelay {
    fn new(samples: usize) -> Self {
        Self {
            buffer: [0.0; MAX_LINE],
            len: samples.clamp(1, MAX_LINE),
            pos: 0,
        }
    }

    fn set_delay(&mut self, samples: usize) {
        self.len = samples.clamp(1, MAX_LINE);
    }

    fn reset(&mut self) {
        self.buffer = [0.0; MAX_LINE];
        self.pos = 0;
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let out = self.buffer[self.pos];
        self.buffer[self.pos] = input;
        self.pos = (self.pos + 1) % self.len;
        out
    }
}

pub struct Reverb {
    predelay: PreDelay,
    combs: [CombFilter; 4],
    allpasses: [AllpassFilter; 2],
    sample_rate: f32,
}

impl Reverb {
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 48_000.0;
        let sc = |s: usize| (s as f32 * scale) as usize;
        Self {
            predelay: PreDelay::new(sc(PREDELAY_48K)),
            combs: [
                CombFilter::new(sc(COMB_DELAYS_48K[0])),
                CombFilter::new(sc(COMB_DELAYS_48K[1])),
                CombFilter::new(sc(COMB_DELAYS_48K[2])),
                CombFilter::new(sc(COMB_DELAYS_48K[3])),
            ],
            allpasses: [
                AllpassFilter::new(sc(AP_DELAYS_48K[0])),
                AllpassFilter::new(sc(AP_DELAYS_48K[1])),
            ],
            sample_rate,
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr;
        let scale = sr / 48_000.0;
        let sc = |s: usize| (s as f32 * scale) as usize;
        self.predelay.set_delay(sc(PREDELAY_48K));
        for (i, &d) in COMB_DELAYS_48K.iter().enumerate() {
            self.combs[i].set_delay(sc(d));
        }
        for (i, &d) in AP_DELAYS_48K.iter().enumerate() {
            self.allpasses[i].set_delay(sc(d));
        }
        self.reset();
    }

    pub fn reset(&mut self) {
        self.predelay.reset();
        for c in &mut self.combs {
            c.reset();
        }
        for a in &mut self.allpasses {
            a.reset();
        }
    }

    /// Set reverb decay. `decay`: 0.0–1.0.
    pub fn set_decay(&mut self, decay: f32) {
        let g = 0.7 * (0.3 + 0.7 * decay.clamp(0.0, 1.0));
        for c in &mut self.combs {
            c.set_feedback(g);
        }
    }

    /// Process a single sample. `mix`: 0.0–1.0.
    #[inline]
    pub fn process(&mut self, input: f32, mix: f32) -> f32 {
        let predelayed = self.predelay.process(input);

        // 4 parallel comb filters, summed.
        let mut comb_sum = 0.0f32;
        for c in &mut self.combs {
            comb_sum += c.process(predelayed);
        }
        comb_sum *= 0.25; // normalize

        // 2 series allpass diffusers.
        let mut diffused = comb_sum;
        for a in &mut self.allpasses {
            diffused = a.process(diffused);
        }

        input * (1.0 - mix) + diffused * mix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn silence_in_silence_out() {
        let mut rev = Reverb::new(SR);
        rev.set_decay(0.5);
        for _ in 0..48_000 {
            let out = rev.process(0.0, 1.0);
            assert_eq!(out, 0.0);
        }
    }

    #[test]
    fn zero_mix_is_dry_passthrough() {
        let mut rev = Reverb::new(SR);
        rev.set_decay(0.5);
        let out = rev.process(1.0, 0.0);
        assert!((out - 1.0).abs() < 1e-6, "mix=0 should pass dry, got {out}");
    }

    #[test]
    fn impulse_produces_reverb_tail() {
        let mut rev = Reverb::new(SR);
        rev.set_decay(0.7);
        rev.process(1.0, 1.0);
        let mut found_tail = false;
        for _ in 1..24_000 {
            let out = rev.process(0.0, 1.0);
            if out.abs() > 0.001 {
                found_tail = true;
                break;
            }
        }
        assert!(found_tail, "reverb should produce a tail after impulse");
    }

    #[test]
    fn tail_decays_to_silence() {
        let mut rev = Reverb::new(SR);
        rev.set_decay(0.4);
        for _ in 0..100 {
            rev.process(0.5, 1.0);
        }
        let mut last = 0.0f32;
        for _ in 0..96_000 {
            last = rev.process(0.0, 1.0).abs();
        }
        assert!(last < 0.001, "tail should decay, got {last}");
    }

    #[test]
    fn higher_decay_produces_longer_tail() {
        let mut short = Reverb::new(SR);
        let mut long = Reverb::new(SR);
        short.set_decay(0.1);
        long.set_decay(0.9);

        short.process(1.0, 1.0);
        long.process(1.0, 1.0);

        let mut e_short = 0.0f32;
        let mut e_long = 0.0f32;
        for _ in 1..24_000 {
            e_short += short.process(0.0, 1.0).powi(2);
            e_long += long.process(0.0, 1.0).powi(2);
        }
        assert!(
            e_long > e_short,
            "higher decay should produce more energy: short={e_short}, long={e_long}"
        );
    }

    #[test]
    fn no_nan_at_extremes() {
        let mut rev = Reverb::new(SR);
        for &decay in &[0.0, 0.5, 1.0] {
            rev.set_decay(decay);
            for _ in 0..1000 {
                let out = rev.process(1.0, 1.0);
                assert!(out.is_finite(), "NaN at decay={decay}");
            }
        }
    }

    #[test]
    fn output_stays_bounded() {
        let mut rev = Reverb::new(SR);
        rev.set_decay(1.0);
        for _ in 0..48_000 {
            let out = rev.process(1.0, 1.0);
            assert!(out.abs() < 10.0, "output should be bounded, got {out}");
        }
    }
}
