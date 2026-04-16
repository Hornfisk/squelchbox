//! Envelopes for SquelchBox: `AmpEnv`, `FilterEnv`, and `AccentEnv`.
//!
//! - `AmpEnv` — authentic TB-303 VEG (Voltage-controlled Envelope Generator).
//!   Fixed shape: 1 ms linear attack → hold at unity until gate_off →
//!   16 ms two-segment release (8 ms flat + 8 ms linear fall). The DECAY
//!   knob does NOT affect this envelope; it only drives `FilterEnv`.
//!
//! - `FilterEnv` — decay-only power curve from 1.0 → 0.0, driven by the
//!   DECAY knob. Adjustable steepness exponent.
//!
//! - `AccentEnv` — 5 ms attack, 300 ms exponential RC decay. Does NOT
//!   reset on re-trigger: successive accents accumulate charge, producing
//!   the famous build-up wobble. Modulates amp + cutoff only (no reso).

/// Authentic TB-303 VEG: gate-driven with fixed shape.
/// Attack (1 ms linear) → Hold at 1.0 → Release (8 ms flat + 8 ms linear).
pub struct AmpEnv {
    sample_rate: f32,
    stage: AmpStage,
    gain: f32,
    attack_inc: f32,
    release_start: f32,
    release_t: f32,
    release_flat_dur: f32,
    release_total_dur: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AmpStage {
    Idle,
    Attack,
    Hold,
    Release,
}

impl AmpEnv {
    pub fn new(sample_rate: f32) -> Self {
        let mut env = Self {
            sample_rate,
            stage: AmpStage::Idle,
            gain: 0.0,
            attack_inc: 0.0,
            release_start: 0.0,
            release_t: 0.0,
            release_flat_dur: 0.0,
            release_total_dur: 0.0,
        };
        env.recalc_times();
        env
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr;
        self.recalc_times();
    }

    fn recalc_times(&mut self) {
        let attack_samples = 0.001 * self.sample_rate;
        self.attack_inc = if attack_samples > 0.0 { 1.0 / attack_samples } else { 1.0 };
        self.release_flat_dur = 0.008 * self.sample_rate;
        self.release_total_dur = 0.016 * self.sample_rate;
    }

    pub fn gate_on(&mut self) {
        self.stage = AmpStage::Attack;
        self.gain = 0.0;
    }

    pub fn gate_off(&mut self) {
        if self.stage == AmpStage::Idle {
            return;
        }
        self.release_start = self.gain;
        self.release_t = 0.0;
        self.stage = AmpStage::Release;
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        match self.stage {
            AmpStage::Idle => 0.0,
            AmpStage::Attack => {
                self.gain += self.attack_inc;
                if self.gain >= 1.0 {
                    self.gain = 1.0;
                    self.stage = AmpStage::Hold;
                }
                self.gain
            }
            AmpStage::Hold => self.gain,
            AmpStage::Release => {
                self.release_t += 1.0;
                if self.release_t >= self.release_total_dur {
                    self.gain = 0.0;
                    self.stage = AmpStage::Idle;
                    return 0.0;
                }
                if self.release_t < self.release_flat_dur {
                    self.gain = self.release_start;
                } else {
                    let fall_t = self.release_t - self.release_flat_dur;
                    let fall_dur = self.release_total_dur - self.release_flat_dur;
                    self.gain = self.release_start * (1.0 - fall_t / fall_dur);
                }
                self.gain
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.stage != AmpStage::Idle
    }
}

/// Filter envelope: decay-only exponential curve from 1.0 → 0.0 over a
/// configurable duration, with an adjustable steepness exponent. Output is
/// in `[0, 1]` and is scaled by the Env Mod knob before being added to the
/// base cutoff.
pub struct FilterEnv {
    t: f32,
    dt: f32,
    duration: f32,
    curve: f32,
    active: bool,
}

impl FilterEnv {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            t: 0.0,
            dt: 1.0 / sample_rate,
            duration: 0.2,
            curve: 2.0,
            active: false,
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.dt = 1.0 / sr;
    }

    pub fn trigger(&mut self, duration_s: f32, curve: f32) {
        self.duration = duration_s.max(0.001);
        self.curve = curve.clamp(0.25, 8.0);
        self.t = 0.0;
        self.active = true;
    }

    /// Reshape the tail in flight. Rescales `t` so the envelope value is
    /// continuous across the change — the normalized progress `t/duration`
    /// stays the same, meaning the remaining tail just becomes longer or
    /// shorter in real time. This prevents a click when the Decay knob
    /// moves mid-note.
    #[inline]
    pub fn set_duration_s(&mut self, duration_s: f32) {
        let new_dur = duration_s.max(0.001);
        if (new_dur - self.duration).abs() > 1e-6 {
            if self.active {
                self.t *= new_dur / self.duration;
            }
            self.duration = new_dur;
        }
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }
        if self.t >= self.duration {
            self.active = false;
            return 0.0;
        }
        let x = self.t / self.duration;
        // f(x) = (1 - x)^curve — sharper-than-linear decay by default.
        let y = (1.0 - x).powf(self.curve);
        self.t += self.dt;
        y
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// Authentic TB-303 accent envelope. 5 ms linear attack, ~300 ms
/// exponential RC decay. Does NOT reset on re-trigger — successive
/// accents accumulate on the still-charged cap, producing the famous
/// build-up wobble. Modulates amp + cutoff only (no resonance).
pub struct AccentEnv {
    stage: AccStage,
    value: f32,
    attack_inc: f32,
    decay_coef: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AccStage {
    Idle,
    Attack,
    Decay,
}

impl AccentEnv {
    const DECAY_TAU: f32 = 0.300 / 6.9078; // ~43 ms tau → ~300 ms full decay
    const ATTACK_MS: f32 = 5.0;

    pub fn new(sample_rate: f32) -> Self {
        let mut env = Self {
            stage: AccStage::Idle,
            value: 0.0,
            attack_inc: 0.0,
            decay_coef: 0.0,
        };
        env.recalc_coeffs(sample_rate);
        env
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.recalc_coeffs(sr);
    }

    fn recalc_coeffs(&mut self, sr: f32) {
        let attack_samples = (Self::ATTACK_MS / 1000.0) * sr;
        self.attack_inc = if attack_samples > 0.0 { 1.0 / attack_samples } else { 1.0 };
        self.decay_coef = (-1.0 / (Self::DECAY_TAU * sr)).exp();
    }

    pub fn trigger(&mut self) {
        if self.value >= 1.0 {
            self.stage = AccStage::Decay;
        } else {
            self.stage = AccStage::Attack;
        }
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        match self.stage {
            AccStage::Idle => 0.0,
            AccStage::Attack => {
                self.value += self.attack_inc;
                if self.value >= 1.0 {
                    self.value = 1.0;
                    self.stage = AccStage::Decay;
                }
                self.value
            }
            AccStage::Decay => {
                self.value *= self.decay_coef;
                if self.value < 1.0e-6 {
                    self.value = 0.0;
                    self.stage = AccStage::Idle;
                }
                self.value
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.stage != AccStage::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn amp_env_has_attack_ramp() {
        let mut env = AmpEnv::new(SR);
        env.gate_on();
        let first = env.tick();
        assert!(first < 0.1, "attack ramp should start near zero, got {first}");
    }

    #[test]
    fn amp_env_holds_at_unity() {
        let mut env = AmpEnv::new(SR);
        env.gate_on();
        // Tick past the 1 ms attack.
        for _ in 0..((0.002 * SR) as usize) {
            env.tick();
        }
        // Should hold at 1.0 indefinitely.
        for _ in 0..10_000 {
            let g = env.tick();
            assert!((g - 1.0).abs() < 0.001, "expected hold at 1.0, got {g}");
        }
    }

    #[test]
    fn amp_env_gate_off_releases_within_16ms() {
        let mut env = AmpEnv::new(SR);
        env.gate_on();
        for _ in 0..200 {
            env.tick();
        }
        env.gate_off();
        let release_samples = (0.016 * SR) as usize;
        for _ in 0..release_samples {
            env.tick();
        }
        let g = env.tick();
        assert!(g < 0.001, "expected near-zero after 16 ms release, got {g}");
        assert!(!env.is_active());
    }

    #[test]
    fn amp_env_release_has_flat_segment() {
        let mut env = AmpEnv::new(SR);
        env.gate_on();
        for _ in 0..200 {
            env.tick();
        }
        env.gate_off();
        // First ~4 ms of release should stay near the captured gain.
        let mut min_flat = 1.0f32;
        for _ in 0..((0.004 * SR) as usize) {
            min_flat = min_flat.min(env.tick());
        }
        assert!(min_flat > 0.9, "flat segment should hold near unity, got {min_flat}");
    }

    #[test]
    fn filter_env_starts_at_one_and_decays() {
        let mut env = FilterEnv::new(SR);
        env.trigger(0.1, 2.0);
        let first = env.tick();
        assert!(first > 0.99, "filter env should start at 1.0, got {first}");
        let half = (0.05 * SR) as usize;
        for _ in 0..half {
            env.tick();
        }
        let mid = env.tick();
        assert!(mid < 0.5 && mid > 0.05, "expected decaying mid value, got {mid}");
    }

    #[test]
    fn filter_env_monotonic() {
        let mut env = FilterEnv::new(SR);
        env.trigger(0.3, 2.0);
        let mut prev = env.tick();
        for _ in 0..((0.3 * SR) as usize) {
            let v = env.tick();
            assert!(v <= prev + 1e-6, "non-monotonic: {v} > {prev}");
            prev = v;
        }
    }

    #[test]
    fn accent_env_bounded_and_decays() {
        let mut env = AccentEnv::new(SR);
        env.trigger();
        let mut peak = 0.0f32;
        for _ in 0..((0.5 * SR) as usize) {
            peak = peak.max(env.tick());
        }
        assert!(peak > 0.5 && peak <= 1.0, "accent peak out of range: {peak}");
        // After 500 ms (well past the 300 ms decay), should be near zero.
        let g = env.tick();
        assert!(g < 0.01, "expected decay, got {g}");
    }

    #[test]
    fn accent_env_accumulates_on_retrigger() {
        let mut env = AccentEnv::new(SR);
        env.trigger();
        // Tick past attack into early decay.
        for _ in 0..((0.01 * SR) as usize) {
            env.tick();
        }
        let before = env.tick();
        // Re-trigger while cap is still charged — should ramp back up
        // from current value, not reset to zero.
        env.trigger();
        let after = env.tick();
        assert!(after >= before - 0.01, "retrigger should not reset: before={before} after={after}");
    }

    #[test]
    fn filter_env_live_duration_stretches_tail() {
        let mut env = FilterEnv::new(SR);
        env.trigger(0.05, 2.0);
        for _ in 0..((0.03 * SR) as usize) {
            env.tick();
        }
        let before = env.tick();
        env.set_duration_s(0.5);
        let after = env.tick();
        assert!(env.is_active());
        assert!(after > 0.0);
        assert!(
            (after - before).abs() < 0.5,
            "set_duration_s should reshape, not retrigger: before={before} after={after}"
        );
    }

    #[test]
    fn envs_idle_return_zero() {
        assert_eq!(AmpEnv::new(SR).tick(), 0.0);
        assert_eq!(FilterEnv::new(SR).tick(), 0.0);
        assert_eq!(AccentEnv::new(SR).tick(), 0.0);
    }
}
