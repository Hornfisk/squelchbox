//! Tempo-synced delay with optional analog-style feedback darkening.
//!
//! Circular buffer sized for ~2 seconds at 96 kHz. Delay time derived
//! from BPM + sync subdivision. Two modes: Clean (pristine repeats) and
//! Analog (one-pole LP in the feedback path, each repeat loses HF).

/// Maximum delay buffer: 2 seconds at 96 kHz.
const MAX_DELAY_SAMPLES: usize = 192_000;

/// Feedback LP cutoff for analog mode (Hz).
const ANALOG_LP_HZ: f32 = 3_000.0;

/// Sync subdivision factors (multiply by beat duration in seconds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncDiv {
    Quarter,
    Eighth,
    DottedEighth,
    Sixteenth,
    TripletEighth,
}

impl SyncDiv {
    pub fn factor(self) -> f32 {
        match self {
            SyncDiv::Quarter => 1.0,
            SyncDiv::Eighth => 0.5,
            SyncDiv::DottedEighth => 0.75,
            SyncDiv::Sixteenth => 0.25,
            SyncDiv::TripletEighth => 1.0 / 3.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelayMode {
    Clean,
    Analog,
}

pub struct Delay {
    buffer: Vec<f32>,
    write_pos: usize,
    pub(crate) delay_samples: usize,
    feedback_lp_z: f32,
    feedback_lp_coeff: f32,
    sample_rate: f32,
}

impl Delay {
    pub fn new(sample_rate: f32) -> Self {
        let buf_len = ((2.0 * sample_rate) as usize).min(MAX_DELAY_SAMPLES);
        Self {
            buffer: vec![0.0; buf_len],
            write_pos: 0,
            delay_samples: 0,
            feedback_lp_z: 0.0,
            feedback_lp_coeff: Self::lp_coeff(ANALOG_LP_HZ, sample_rate),
            sample_rate,
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr;
        let buf_len = ((2.0 * sr) as usize).min(MAX_DELAY_SAMPLES);
        self.buffer.resize(buf_len, 0.0);
        self.feedback_lp_coeff = Self::lp_coeff(ANALOG_LP_HZ, sr);
        self.reset();
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
        self.feedback_lp_z = 0.0;
    }

    /// Update delay time from tempo. Call once per block.
    pub fn set_tempo(&mut self, bpm: f32, div: SyncDiv) {
        let beat_secs = 60.0 / bpm.max(1.0);
        let delay_secs = beat_secs * div.factor();
        self.delay_samples = ((delay_secs * self.sample_rate) as usize)
            .clamp(1, self.buffer.len() - 1);
    }

    /// Process a single sample. `feedback`: 0.0–0.9, `mix`: 0.0–1.0.
    #[inline]
    pub fn process(
        &mut self,
        input: f32,
        feedback: f32,
        mix: f32,
        mode: DelayMode,
    ) -> f32 {
        let buf_len = self.buffer.len();
        let read_pos = (self.write_pos + buf_len - self.delay_samples) % buf_len;
        let delayed = self.buffer[read_pos];

        // Feedback path — optionally filter for analog mode.
        // In Analog mode the LP is also used as the output signal so that
        // the first repeat is already darkened (not just subsequent ones).
        let fb_signal = match mode {
            DelayMode::Clean => delayed,
            DelayMode::Analog => {
                self.feedback_lp_z += self.feedback_lp_coeff * (delayed - self.feedback_lp_z);
                self.feedback_lp_z
            }
        };

        // Write input + fed-back signal into the buffer.
        let fb = feedback.clamp(0.0, 0.9);
        self.buffer[self.write_pos] = input + fb_signal * fb;
        self.write_pos = (self.write_pos + 1) % buf_len;

        // Dry/wet mix — in Analog mode use the LP-filtered signal as wet output.
        input * (1.0 - mix) + fb_signal * mix
    }

    fn lp_coeff(cutoff_hz: f32, sr: f32) -> f32 {
        let rc = 1.0 / (std::f32::consts::TAU * cutoff_hz);
        let dt = 1.0 / sr;
        dt / (rc + dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn silence_in_silence_out() {
        let mut delay = Delay::new(SR);
        delay.set_tempo(120.0, SyncDiv::Eighth);
        for _ in 0..48_000 {
            let out = delay.process(0.0, 0.5, 1.0, DelayMode::Clean);
            assert_eq!(out, 0.0);
        }
    }

    #[test]
    fn zero_mix_is_dry_passthrough() {
        let mut delay = Delay::new(SR);
        delay.set_tempo(120.0, SyncDiv::Eighth);
        let out = delay.process(1.0, 0.5, 0.0, DelayMode::Clean);
        assert!((out - 1.0).abs() < 1e-6, "mix=0 should pass dry, got {out}");
    }

    #[test]
    fn impulse_produces_delayed_repeat() {
        let mut delay = Delay::new(SR);
        delay.set_tempo(120.0, SyncDiv::Eighth);
        // 120 BPM, 1/8 note = 0.25 sec = 12000 samples
        let expected_delay = 12_000;

        delay.process(1.0, 0.5, 1.0, DelayMode::Clean);
        let mut found_repeat = false;
        for i in 1..24_000 {
            let out = delay.process(0.0, 0.5, 1.0, DelayMode::Clean);
            if i == expected_delay {
                assert!(
                    out.abs() > 0.1,
                    "expected repeat at sample {expected_delay}, got {out}"
                );
                found_repeat = true;
            }
        }
        assert!(found_repeat, "never found the delayed repeat");
    }

    #[test]
    fn correct_delay_time_for_subdivision() {
        for (div, factor) in [
            (SyncDiv::Quarter, 1.0),
            (SyncDiv::Eighth, 0.5),
            (SyncDiv::DottedEighth, 0.75),
            (SyncDiv::Sixteenth, 0.25),
        ] {
            let mut delay = Delay::new(SR);
            delay.set_tempo(120.0, div);
            let expected = (60.0 / 120.0 * factor * SR) as usize;
            assert_eq!(
                delay.delay_samples, expected,
                "wrong delay for {div:?}: got {}, expected {expected}",
                delay.delay_samples
            );
        }
    }

    #[test]
    fn analog_mode_darkens_repeats() {
        let mut clean = Delay::new(SR);
        let mut analog = Delay::new(SR);
        clean.set_tempo(120.0, SyncDiv::Eighth);
        analog.set_tempo(120.0, SyncDiv::Eighth);

        for i in 0..100 {
            let sig = if i % 2 == 0 { 1.0 } else { -1.0 };
            clean.process(sig, 0.7, 1.0, DelayMode::Clean);
            analog.process(sig, 0.7, 1.0, DelayMode::Analog);
        }
        let delay_samps = clean.delay_samples;
        let mut clean_hf = 0.0f32;
        let mut analog_hf = 0.0f32;
        for i in 100..(delay_samps + 200) {
            let c = clean.process(0.0, 0.7, 1.0, DelayMode::Clean);
            let a = analog.process(0.0, 0.7, 1.0, DelayMode::Analog);
            if i >= delay_samps {
                clean_hf += c.abs();
                analog_hf += a.abs();
            }
        }
        assert!(
            analog_hf < clean_hf,
            "analog should have less HF: analog={analog_hf}, clean={clean_hf}"
        );
    }

    #[test]
    fn feedback_does_not_explode() {
        let mut delay = Delay::new(SR);
        delay.set_tempo(120.0, SyncDiv::Sixteenth);
        for _ in 0..100 {
            delay.process(1.0, 0.9, 1.0, DelayMode::Clean);
        }
        for _ in 0..96_000 {
            let out = delay.process(0.0, 0.9, 1.0, DelayMode::Clean);
            assert!(out.abs() < 10.0, "feedback exploded: {out}");
        }
    }

    #[test]
    fn no_nan_at_any_params() {
        let mut delay = Delay::new(SR);
        delay.set_tempo(40.0, SyncDiv::Quarter);
        for &fb in &[0.0, 0.5, 0.9] {
            for &mix in &[0.0, 0.5, 1.0] {
                for mode in [DelayMode::Clean, DelayMode::Analog] {
                    let out = delay.process(1.0, fb, mix, mode);
                    assert!(out.is_finite(), "NaN at fb={fb} mix={mix} mode={mode:?}");
                }
            }
        }
    }
}
