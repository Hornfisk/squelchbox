//! 3-pole diode-ladder lowpass — the heart of the TB-303 sound.
//!
//! Topology: three cascaded 1-pole TPT lowpass integrators with a global
//! resonance feedback path from the output back to the input, saturated
//! through a soft diode curve `x / sqrt(1 + x²)`. The per-sample-delayed
//! feedback isn't a true zero-delay-feedback solution (that would require
//! solving an implicit equation), but combined with TPT integrators and the
//! bounded saturator it's unconditionally stable and musical at all
//! reasonable cutoff/resonance combinations.
//!
//! Character:
//! - At low Q the response is a clean 3-pole (≈ 18 dB/oct) lowpass.
//! - Above Q ≈ 0.8 the resonance peak rises until, near Q = 1.0, the loop
//!   self-oscillates at roughly the cutoff frequency — this is the 303's
//!   acid "scream."
//! - The feedback saturator keeps self-oscillation bounded and gives the
//!   classic asymmetric overdriven tone when the voice slams the filter.
//!
//! References:
//! - Vadim Zavalishin, *The Art of VA Filter Design*, §4 (TPT integrators)
//!   and §5.3 (nonlinear ladders).
//! - Open303 (MIT) for sanity-checking the feedback scaling range.

use std::f32::consts::PI;

use super::flush_denormal;

/// Cheap diode/tanh analog. Unity slope at 0, asymptote ±1, much cheaper
/// than `tanh` and indistinguishable at audio rates.
#[inline(always)]
fn diode_sat(x: f32) -> f32 {
    x / (1.0 + x * x).sqrt()
}

/// Per-stage soft-clip modelling the diode pair at each LP section of
/// the ladder. Gentler than the global feedback saturator — the real
/// diodes barely engage at normal oscillator levels but progressively
/// compress as resonance drives the internal signal hot. The cubic
/// `x − x³/6` shape gives a softer knee than `diode_sat` and adds
/// primarily 3rd-harmonic content per stage, which stacks through the
/// cascade into the rich "squishy" overtone texture that makes a
/// 303 self-oscillation sound fat rather than sine-pure.
#[inline(always)]
fn stage_sat(x: f32) -> f32 {
    let x = x.clamp(-2.0, 2.0);
    x - x * x * x * (1.0 / 6.0)
}

/// Asymmetric DC bias pushed into the feedback saturator. A real 303's
/// diode ladder sits on a ~100 mV operating-point offset, which makes
/// the positive and negative halves of the waveform compress by
/// unequal amounts. That asymmetry generates even harmonics (2nd, 4th)
/// that give the filter its vocal, "mouthy" quality at high
/// resonance. Small on purpose — bigger values quickly sound
/// distorted rather than warm.
const DIODE_BIAS: f32 = 0.08;

/// Per-stage bias: each diode pair has a slightly different forward
/// voltage, making the clipping per stage slightly asymmetric.
/// Generates subtle even-harmonic content (2nd, 4th) alongside the
/// 3rd from `stage_sat`. Sign alternates per stage in the real
/// circuit; we bake a fixed small offset for each pole.
const STAGE_BIAS: [f32; 3] = [0.05, -0.03, 0.04];

pub struct DiodeLadder3Pole {
    sr: f32,
    /// TPT coefficient g / (1 + g), cached when cutoff/sr changes.
    gp: f32,
    /// Effective feedback gain after HF roll-off, fed to the loop.
    k: f32,
    /// User-set resonance `0..=1.05`. Kept verbatim so we can re-derive
    /// `k` whenever cutoff changes (HF resonance loss).
    r_user: f32,
    /// Last-set cutoff in Hz. Used by `update_k` to apply the HF
    /// resonance taper.
    fc_hz: f32,
    /// Integrator states.
    s1: f32,
    s2: f32,
    s3: f32,
    /// One-sample-delayed output, used in the feedback path.
    y_prev: f32,
}

/// Max feedback gain at `resonance = 1.0`. For a 3-pole cascade the
/// phase-flip frequency is √3·fc where each pole's magnitude has dropped
/// to 0.5, so the critical loop gain for self-oscillation is 1 / 0.5³ = 8.
/// We push slightly past that so `resonance = 1.0` reliably self-oscillates
/// even when the feedback saturator compresses the loop at large amplitudes.
const K_MAX: f32 = 10.0;

impl DiodeLadder3Pole {
    pub fn new(sample_rate: f32) -> Self {
        let mut f = Self {
            sr: sample_rate,
            gp: 0.0,
            k: 0.0,
            r_user: 0.0,
            fc_hz: 1_000.0,
            s1: 0.0,
            s2: 0.0,
            s3: 0.0,
            y_prev: 0.0,
        };
        f.set_cutoff(1_000.0);
        f.set_resonance(0.0);
        f
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sr = sr;
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
        self.s3 = 0.0;
        self.y_prev = 0.0;
    }

    pub fn set_cutoff(&mut self, hz: f32) {
        let hz = hz.clamp(20.0, self.sr * 0.45);
        self.fc_hz = hz;
        // Bilinear prewarp, matching the placeholder 1-pole for consistency.
        let wd = 2.0 * PI * hz;
        let t = 1.0 / self.sr;
        let wa = (2.0 / t) * (wd * t * 0.5).tan();
        let g = wa * t * 0.5;
        self.gp = g / (1.0 + g);
        self.update_k();
    }

    pub fn set_resonance(&mut self, r: f32) {
        self.r_user = r.clamp(0.0, 1.05);
        self.update_k();
    }

    /// Recompute the effective loop gain from `r_user` and `fc_hz`,
    /// applying the 303-style HF resonance taper. The real diode ladder
    /// loses resonance as cutoff rises (transistor bandwidth + parasitic
    /// losses); past ~2.5 kHz the peak softens, and by ~8 kHz it's
    /// roughly a third of its low-frequency height. This is also what
    /// keeps the unit-delay feedback loop stable as fc climbs — the
    /// extra phase shift from the 1-sample delay would otherwise
    /// detune the resonance peak and turn the filter into noise.
    fn update_k(&mut self) {
        const FC_TAPER_LO: f32 = 2_500.0;
        const FC_TAPER_HI: f32 = 8_000.0;
        const TAPER_FLOOR: f32 = 0.30;
        let scale = if self.fc_hz <= FC_TAPER_LO {
            1.0
        } else if self.fc_hz >= FC_TAPER_HI {
            TAPER_FLOOR
        } else {
            let t = (self.fc_hz - FC_TAPER_LO) / (FC_TAPER_HI - FC_TAPER_LO);
            // Smoothstep so the boundaries don't audibly click on slow
            // cutoff sweeps.
            let s = t * t * (3.0 - 2.0 * t);
            1.0 + (TAPER_FLOOR - 1.0) * s
        };
        self.k = self.r_user * K_MAX * scale;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        // Feedback path: previous output times loop gain, saturated.
        // Put the saturator on the SUM (input - fb) rather than on each
        // separately so small signals see a near-linear response and large
        // signals compress toward ±1.
        let sum = x - self.k * self.y_prev;
        // Bias pushes the operating point off center before the
        // saturator, then we subtract the steady-state bias response
        // after so DC is preserved.
        let drive_in = diode_sat(sum + DIODE_BIAS) - diode_sat(DIODE_BIAS);

        // Three TPT 1-pole lowpass integrators with per-stage diode
        // saturation. Each pole's output is soft-clipped through the
        // stage_sat + per-pole bias before feeding the next, modelling
        // the individual diode pairs in the real ladder.
        let v1 = (drive_in - self.s1) * self.gp;
        let y1 = v1 + self.s1;
        self.s1 = flush_denormal(y1 + v1);
        let y1 = stage_sat(y1 + STAGE_BIAS[0]) - stage_sat(STAGE_BIAS[0]);

        let v2 = (y1 - self.s2) * self.gp;
        let y2 = v2 + self.s2;
        self.s2 = flush_denormal(y2 + v2);
        let y2 = stage_sat(y2 + STAGE_BIAS[1]) - stage_sat(STAGE_BIAS[1]);

        let v3 = (y2 - self.s3) * self.gp;
        let y3 = v3 + self.s3;
        self.s3 = flush_denormal(y3 + v3);
        let y3 = stage_sat(y3 + STAGE_BIAS[2]) - stage_sat(STAGE_BIAS[2]);

        self.y_prev = flush_denormal(y3);
        y3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::oscillator::fundamental_power;

    const SR: f32 = 48_000.0;

    #[test]
    fn dc_passes_at_zero_resonance() {
        let mut f = DiodeLadder3Pole::new(SR);
        f.set_cutoff(800.0);
        f.set_resonance(0.0);
        let mut y = 0.0;
        for _ in 0..20_000 {
            y = f.process(0.5);
        }
        // With k=0 there's no feedback, so DC passes through the input
        // saturator + three stages of per-pole diode soft-clip. The
        // cascaded compression reduces the 0.5 input to ~0.35–0.45
        // depending on the bias offsets.
        assert!(y > 0.25 && y < 0.55, "DC out of range: {y}");
    }

    #[test]
    fn attenuates_well_above_cutoff() {
        let mut f = DiodeLadder3Pole::new(SR);
        f.set_cutoff(200.0);
        f.set_resonance(0.0);
        // Drive a 4 kHz sine (more than a decade above cutoff).
        let freq = 4_000.0;
        let omega = 2.0 * PI * freq / SR;
        let mut peak_in = 0.0f32;
        let mut peak_out = 0.0f32;
        for i in 0..8_000 {
            let x = (omega * i as f32).sin() * 0.3; // small signal — stay linear
            let y = f.process(x);
            if i > 1_000 {
                peak_in = peak_in.max(x.abs());
                peak_out = peak_out.max(y.abs());
            }
        }
        // 3-pole LP at more than a decade above cutoff should knock it
        // down by way more than 40 dB nominally, but we're conservative.
        let ratio = peak_out / peak_in;
        assert!(ratio < 0.05, "expected strong attenuation, got ratio {ratio}");
    }

    #[test]
    fn stable_at_extreme_cutoffs_and_high_resonance() {
        let mut f = DiodeLadder3Pole::new(SR);
        for fc in [20.0, 60.0, 500.0, 5_000.0, 15_000.0, 21_500.0] {
            f.set_cutoff(fc);
            for res in [0.0, 0.5, 0.9, 1.0] {
                f.reset();
                f.set_resonance(res);
                for i in 0..4_000 {
                    let x = (i as f32 * 0.001).sin() * 0.5;
                    let y = f.process(x);
                    assert!(
                        y.is_finite(),
                        "non-finite at fc={fc}, res={res}, i={i}"
                    );
                    assert!(y.abs() < 10.0, "blew up at fc={fc}, res={res}: {y}");
                }
            }
        }
    }

    #[test]
    fn self_oscillates_at_max_resonance() {
        // At full resonance with zero input the loop should sustain a tone
        // near the cutoff frequency.
        let mut f = DiodeLadder3Pole::new(SR);
        let fc = 500.0;
        f.set_cutoff(fc);
        f.set_resonance(1.0);
        // Kick the loop so self-osc has an amplitude to grow from.
        f.process(1.0);
        // Skip transient, then measure.
        for _ in 0..4_000 {
            f.process(0.0);
        }
        let mut buf = vec![0.0f32; 16_384];
        for y in buf.iter_mut() {
            *y = f.process(0.0);
        }
        // Bounded amplitude check.
        let peak = buf.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
        assert!(peak > 0.05 && peak < 1.5, "self-osc peak out of range: {peak}");
        // For a 3-pole LP cascade the phase-flip (and thus self-osc)
        // frequency is ≈ √3 · fc, not fc itself — each pole contributes
        // -60° there, summing to the required -180° for loop inversion.
        let expected = fc * 3f32.sqrt();
        let at_osc = fundamental_power(&buf, SR, expected);
        let off = fundamental_power(&buf, SR, expected * 0.3); // far below
        assert!(
            at_osc > off * 10.0,
            "self-osc energy at {expected} Hz ({at_osc}) should dominate off-bin ({off})"
        );
    }

    #[test]
    fn resonance_peak_boosts_near_cutoff() {
        // Feeding a sine at cutoff with Q ≈ 0.9 should give more output
        // than the same sine at Q = 0, i.e. resonance produces a peak.
        let fc = 800.0;
        let omega = 2.0 * PI * fc / SR;
        let render = |res: f32| -> f32 {
            let mut f = DiodeLadder3Pole::new(SR);
            f.set_cutoff(fc);
            f.set_resonance(res);
            let mut peak = 0.0f32;
            for i in 0..8_000 {
                let x = (omega * i as f32).sin() * 0.2;
                let y = f.process(x);
                if i > 2_000 {
                    peak = peak.max(y.abs());
                }
            }
            peak
        };
        let flat = render(0.0);
        let peaky = render(0.9);
        assert!(
            peaky > flat * 1.5,
            "resonance should boost at cutoff: flat={flat}, peaky={peaky}"
        );
    }
}
