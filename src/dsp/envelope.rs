//! Envelopes for SquelchBox: `AmpEnv`, `FilterEnv`, and `AccentEnv`.
//!
//! Adapted from slammer's `dsp/envelope.rs`:
//!
//! - `AmpEnv` — was slammer's `AmpEnvelope`, extended with a `gate_off()`
//!   call that switches into a fast 5 ms linear release so MIDI note-off
//!   (and rest steps) cleanly release the voice. A 303 step without slide
//!   retriggers the amp env; with slide, the env keeps ticking and only
//!   the oscillator frequency changes.
//!
//! - `FilterEnv` — was slammer's `PitchEnvelope`, renamed for its new
//!   consumer. Produces a normalised `[0, 1]` envelope whose curve
//!   (steepness) is user-adjustable. Drives the filter cutoff through the
//!   Env Mod knob.
//!
//! - `AccentEnv` — fresh, short secondary envelope. Triggered only on
//!   accented steps. Its output biases amp gain, filter cutoff, and
//!   filter resonance, and slightly extends the amp decay. Fast attack
//!   (~5 ms) so the "bounce" lands on the beat.

/// Amplitude envelope: 1 ms linear anti-click attack + exponential decay.
/// `gate_off()` switches to a 5 ms linear release from whatever gain is
/// currently on the output.
pub struct AmpEnv {
    tau: f32,
    t: f32,
    dt: f32,
    attack_samples: usize,
    attack_counter: usize,
    /// Phase: 0 = idle, 1 = attack+decay, 2 = release.
    phase: u8,
    /// Release gain at gate-off; release decays linearly from this.
    release_from: f32,
    release_samples: usize,
    release_counter: usize,
}

impl AmpEnv {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            tau: 0.06,
            t: 0.0,
            dt: 1.0 / sample_rate,
            attack_samples: (0.001 * sample_rate) as usize,
            attack_counter: 0,
            phase: 0,
            release_from: 0.0,
            release_samples: (0.005 * sample_rate) as usize,
            release_counter: 0,
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.dt = 1.0 / sr;
        self.attack_samples = (0.001 * sr) as usize;
        self.release_samples = (0.005 * sr) as usize;
    }

    /// Start a new note. `decay_ms` sets the -60 dB time.
    pub fn trigger(&mut self, decay_ms: f32) {
        let decay_s = (decay_ms / 1000.0).max(0.001);
        // 60 dB ≈ 6.9078 time constants (ln(1000)).
        self.tau = decay_s / 6.9078;
        self.t = 0.0;
        self.attack_counter = 0;
        self.phase = 1;
    }

    /// Update the decay time constant mid-note without retriggering.
    /// Called per-sample from the voice so the Decay knob is live.
    /// Rescales `t` so the current decay-phase gain is preserved across
    /// the tau change — the rate of decay updates, but there's no audible
    /// jump on the envelope itself.
    #[inline]
    pub fn set_decay_ms(&mut self, decay_ms: f32) {
        let decay_s = (decay_ms / 1000.0).max(0.001);
        let new_tau = decay_s / 6.9078;
        if (new_tau - self.tau).abs() > 1e-6 {
            // exp(-t_new/tau_new) == exp(-t_old/tau_old)
            //   ⇒ t_new = t_old * tau_new / tau_old.
            if self.phase == 1 {
                self.t *= new_tau / self.tau;
            }
            self.tau = new_tau;
        }
    }

    /// Switch into fast linear release from the current gain.
    pub fn gate_off(&mut self) {
        if self.phase == 1 {
            // Sample current gain as release start.
            let attack_gain = self.current_attack_gain();
            let decay_gain = (-self.t / self.tau).exp();
            self.release_from = attack_gain * decay_gain;
            self.release_counter = 0;
            self.phase = 2;
        }
    }

    /// Snapshot the current attack-ramp gain without advancing state.
    fn current_attack_gain(&self) -> f32 {
        if self.attack_counter >= self.attack_samples {
            1.0
        } else {
            (self.attack_counter as f32 + 1.0) / self.attack_samples as f32
        }
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        match self.phase {
            0 => 0.0,
            1 => {
                let attack_gain = if self.attack_counter < self.attack_samples {
                    let g = (self.attack_counter as f32 + 1.0) / self.attack_samples as f32;
                    self.attack_counter += 1;
                    g
                } else {
                    1.0
                };
                let decay_gain = (-self.t / self.tau).exp();
                self.t += self.dt;
                let gain = attack_gain * decay_gain;
                if gain < 0.0001 {
                    self.phase = 0;
                    return 0.0;
                }
                gain
            }
            2 => {
                if self.release_counter >= self.release_samples {
                    self.phase = 0;
                    return 0.0;
                }
                let r = 1.0 - (self.release_counter as f32 / self.release_samples as f32);
                self.release_counter += 1;
                self.release_from * r
            }
            _ => 0.0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.phase != 0
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

/// Accent envelope: fast-attack, short-decay secondary envelope used to
/// bias amp gain, filter cutoff, and resonance on accented steps.
/// Independent of the main amp/filter envs — a slide-accented step still
/// gets accent-env modulation without retriggering the amp.
pub struct AccentEnv {
    t: f32,
    dt: f32,
    attack_samples: usize,
    attack_counter: usize,
    decay_tau: f32,
    active: bool,
}

impl AccentEnv {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            t: 0.0,
            dt: 1.0 / sample_rate,
            attack_samples: (0.005 * sample_rate) as usize, // 5 ms attack
            attack_counter: 0,
            decay_tau: 0.08 / 6.9078, // ~80 ms to -60 dB
            active: false,
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.dt = 1.0 / sr;
        self.attack_samples = (0.005 * sr) as usize;
    }

    pub fn trigger(&mut self) {
        self.t = 0.0;
        self.attack_counter = 0;
        self.active = true;
    }

    #[inline]
    pub fn tick(&mut self) -> f32 {
        if !self.active {
            return 0.0;
        }
        let attack_gain = if self.attack_counter < self.attack_samples {
            let g = (self.attack_counter as f32 + 1.0) / self.attack_samples as f32;
            self.attack_counter += 1;
            g
        } else {
            1.0
        };
        let decay_gain = (-self.t / self.decay_tau).exp();
        self.t += self.dt;
        let gain = attack_gain * decay_gain;
        if gain < 0.0001 {
            self.active = false;
            return 0.0;
        }
        gain
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn amp_env_has_attack_ramp() {
        let mut env = AmpEnv::new(SR);
        env.trigger(200.0);
        let first = env.tick();
        assert!(first < 0.1, "attack ramp should start near zero, got {first}");
    }

    #[test]
    fn amp_env_reaches_decay_floor() {
        let mut env = AmpEnv::new(SR);
        env.trigger(100.0);
        let n = (0.2 * SR) as usize; // twice the decay length
        let mut g = 1.0;
        for _ in 0..n {
            g = env.tick();
        }
        assert!(g < 0.01, "expected near-zero at 2x decay, got {g}");
    }

    #[test]
    fn amp_env_monotonic_after_attack() {
        let mut env = AmpEnv::new(SR);
        env.trigger(500.0);
        for _ in 0..100 {
            env.tick();
        } // skip attack
        let mut prev = env.tick();
        for _ in 0..10_000 {
            let g = env.tick();
            assert!(g <= prev + 1e-6, "non-monotonic: {g} > {prev}");
            prev = g;
        }
    }

    #[test]
    fn amp_env_gate_off_releases_within_5ms() {
        let mut env = AmpEnv::new(SR);
        env.trigger(10_000.0); // very long decay so gate_off dominates
        for _ in 0..200 {
            env.tick();
        }
        env.gate_off();
        let release_samples = (0.005 * SR) as usize;
        for _ in 0..release_samples {
            env.tick();
        }
        let g = env.tick();
        assert!(g < 0.001, "expected near-zero after release, got {g}");
        assert!(!env.is_active());
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
        for _ in 0..((0.2 * SR) as usize) {
            peak = peak.max(env.tick());
        }
        assert!(peak > 0.5 && peak <= 1.0, "accent peak out of range: {peak}");
        // After decay window, should have released.
        let g = env.tick();
        assert!(g < 0.1, "expected decay, got {g}");
    }

    #[test]
    fn amp_env_live_decay_updates_tau() {
        let mut env = AmpEnv::new(SR);
        env.trigger(2_000.0);
        for _ in 0..100 {
            env.tick();
        }
        env.set_decay_ms(50.0);
        // With a 50 ms decay, 150 ms of ticking should land us near zero.
        for _ in 0..((0.15 * SR) as usize) {
            env.tick();
        }
        let g = env.tick();
        assert!(g < 0.01, "expected live decay shortening, got {g}");
    }

    #[test]
    fn filter_env_live_duration_stretches_tail() {
        let mut env = FilterEnv::new(SR);
        env.trigger(0.05, 2.0);
        for _ in 0..((0.03 * SR) as usize) {
            env.tick();
        }
        let before = env.tick();
        // Stretch the tail: longer duration means slower normalised
        // progress, so the next sample should not jump to zero.
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
