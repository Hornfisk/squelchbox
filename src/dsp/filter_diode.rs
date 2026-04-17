//! 4-pole diode-ladder lowpass — authentic TB-303 VCF.
//!
//! Topology: four cascaded 1-pole TPT lowpass integrators with a global
//! resonance feedback path saturated through a soft diode curve.
//!
//! **First cap half-value:** The real TB-303 filter uses a cap at the first
//! stage that is half the value of the other three, placing the first pole
//! an octave higher (at 2·fc). This is implemented by using a separate
//! coefficient `gp1 = tan(π·2·fc/sr) / (1 + tan(π·2·fc/sr))` for stage 1.
//!
//! **DC-blocking feedback:** A 1-pole HPF (~30 Hz) on the feedback signal
//! reduces the loop gain at very low frequencies, giving the filter a mild
//! bandpass character and matching the coupling-capacitor effects in the
//! real circuit.
//!
//! **No self-oscillation:** K_MAX = 4.0 sits just below the analytical K_crit
//! ≈ 4.3 for this topology — close enough to get a screamy peak at max res,
//! but still stable (the real TB-303 never self-oscillates, and neither does
//! this filter at any setting).
//!
//! References:
//! - Vadim Zavalishin, *The Art of VA Filter Design*, §4 (TPT integrators)
//!   and §5.3 (nonlinear ladders).
//! - TB-303 schematic analysis for the 4-pole topology and first-cap detail.

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
/// cascade into the rich "squishy" overtone texture.
#[inline(always)]
fn stage_sat(x: f32) -> f32 {
    let x = x.clamp(-2.0, 2.0);
    x - x * x * x * (1.0 / 6.0)
}

/// Asymmetric DC bias pushed into the feedback saturator. A real 303's
/// diode ladder sits on a ~100 mV operating-point offset, generating
/// even harmonics that give the filter its vocal quality at high resonance.
const DIODE_BIAS: f32 = 0.08;

/// Per-stage bias: each diode pair has a slightly different forward
/// voltage. Sign alternates per stage; we bake a fixed small offset.
const STAGE_BIAS: [f32; 4] = [0.05, -0.03, 0.04, -0.02];

/// HPF cutoff for the DC blocker in the feedback path.
const DC_HPF_HZ: f32 = 30.0;

pub struct DiodeLadder4Pole {
    sr: f32,
    /// TPT coefficient for stages 2–4: g/(1+g) where g = tan(π·fc/sr).
    gp: f32,
    /// TPT coefficient for stage 1 (first cap at half value → 2× cutoff):
    /// g1/(1+g1) where g1 = tan(π·2·fc/sr).
    gp1: f32,
    /// Effective feedback gain after HF roll-off.
    k: f32,
    /// User-set resonance 0..=1.05. Kept so we can re-derive `k` when
    /// cutoff changes (HF resonance taper).
    r_user: f32,
    /// Last-set cutoff in Hz.
    fc_hz: f32,
    /// Integrator states for the four poles.
    s1: f32,
    s2: f32,
    s3: f32,
    s4: f32,
    /// One-sample-delayed output used in the feedback path.
    y_prev: f32,
    /// DC blocker state on the feedback signal (x[n-1], y[n-1], pole R).
    dc_x1: f32,
    dc_y1: f32,
    dc_r: f32,
}

/// Max feedback gain. For the modified 4-pole topology (stage 1 at 2×fc)
/// the analytical self-oscillation threshold is K_crit ≈ 4.3. Sitting at
/// 4.0 puts max resonance at ~93% of threshold — screamy peak with long
/// ring-down, but the filter still decays to silence on its own.
const K_MAX: f32 = 4.0;

impl DiodeLadder4Pole {
    pub fn new(sample_rate: f32) -> Self {
        let dc_r = (-std::f32::consts::TAU * DC_HPF_HZ / sample_rate).exp();
        let mut f = Self {
            sr: sample_rate,
            gp: 0.0,
            gp1: 0.0,
            k: 0.0,
            r_user: 0.0,
            fc_hz: 1_000.0,
            s1: 0.0,
            s2: 0.0,
            s3: 0.0,
            s4: 0.0,
            y_prev: 0.0,
            dc_x1: 0.0,
            dc_y1: 0.0,
            dc_r,
        };
        f.set_cutoff(1_000.0);
        f.set_resonance(0.0);
        f
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sr = sr;
        self.dc_r = (-std::f32::consts::TAU * DC_HPF_HZ / sr).exp();
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
        self.s3 = 0.0;
        self.s4 = 0.0;
        self.y_prev = 0.0;
        self.dc_x1 = 0.0;
        self.dc_y1 = 0.0;
    }

    pub fn set_cutoff(&mut self, hz: f32) {
        let hz = hz.clamp(20.0, self.sr * 0.45);
        self.fc_hz = hz;

        // Bilinear-prewarped TPT coefficient for stages 2–4.
        let g = (PI * hz / self.sr).tan();
        self.gp = g / (1.0 + g);

        // Stage 1 coefficient at 2× cutoff (first cap is half value).
        let hz1 = (2.0 * hz).min(self.sr * 0.45);
        let g1 = (PI * hz1 / self.sr).tan();
        self.gp1 = g1 / (1.0 + g1);

        self.update_k();
    }

    pub fn set_resonance(&mut self, r: f32) {
        self.r_user = r.clamp(0.0, 1.05);
        self.update_k();
    }

    /// Recompute effective loop gain from `r_user` and `fc_hz`, applying
    /// the 303-style HF resonance taper. The diode ladder loses resonance
    /// as cutoff rises (transistor bandwidth + parasitic losses); past
    /// ~2.5 kHz the peak softens. This also keeps the unit-delay feedback
    /// stable as fc climbs.
    fn update_k(&mut self) {
        const FC_TAPER_LO: f32 = 3_000.0;
        const FC_TAPER_HI: f32 = 10_000.0;
        const TAPER_FLOOR: f32 = 0.60;
        let scale = if self.fc_hz <= FC_TAPER_LO {
            1.0
        } else if self.fc_hz >= FC_TAPER_HI {
            TAPER_FLOOR
        } else {
            let t = (self.fc_hz - FC_TAPER_LO) / (FC_TAPER_HI - FC_TAPER_LO);
            let s = t * t * (3.0 - 2.0 * t);
            1.0 + (TAPER_FLOOR - 1.0) * s
        };
        self.k = self.r_user * K_MAX * scale;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        // DC-block the feedback signal. Very-low-frequency signals get less
        // resonance peak, matching the coupling-cap bandpass character of
        // the real circuit. Formula: y = x - x1 + R·y1.
        let fb_raw = self.y_prev;
        let fb = fb_raw - self.dc_x1 + self.dc_r * self.dc_y1;
        self.dc_x1 = fb_raw;
        self.dc_y1 = flush_denormal(fb);

        let sum = x - self.k * fb;
        let drive_in = diode_sat(sum + DIODE_BIAS) - diode_sat(DIODE_BIAS);

        // Stage 1: uses gp1 (2× cutoff — first cap is half value).
        let v1 = (drive_in - self.s1) * self.gp1;
        let y1 = v1 + self.s1;
        self.s1 = flush_denormal(y1 + v1);
        let y1 = stage_sat(y1 + STAGE_BIAS[0]) - stage_sat(STAGE_BIAS[0]);

        // Stages 2–4: standard cutoff coefficient gp.
        let v2 = (y1 - self.s2) * self.gp;
        let y2 = v2 + self.s2;
        self.s2 = flush_denormal(y2 + v2);
        let y2 = stage_sat(y2 + STAGE_BIAS[1]) - stage_sat(STAGE_BIAS[1]);

        let v3 = (y2 - self.s3) * self.gp;
        let y3 = v3 + self.s3;
        self.s3 = flush_denormal(y3 + v3);
        let y3 = stage_sat(y3 + STAGE_BIAS[2]) - stage_sat(STAGE_BIAS[2]);

        let v4 = (y3 - self.s4) * self.gp;
        let y4 = v4 + self.s4;
        self.s4 = flush_denormal(y4 + v4);
        let y4 = stage_sat(y4 + STAGE_BIAS[3]) - stage_sat(STAGE_BIAS[3]);

        self.y_prev = flush_denormal(y4);
        y4
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::oscillator::fundamental_power;

    const SR: f32 = 48_000.0;

    #[test]
    fn dc_passes_at_zero_resonance() {
        let mut f = DiodeLadder4Pole::new(SR);
        f.set_cutoff(800.0);
        f.set_resonance(0.0);
        let mut y = 0.0;
        for _ in 0..20_000 {
            y = f.process(0.5);
        }
        // k=0 → no feedback. DC passes through input saturator + four
        // stages of per-pole diode soft-clip. Cascaded compression reduces
        // 0.5 to ~0.30–0.48 depending on the bias offsets.
        assert!(y > 0.25 && y < 0.55, "DC out of range: {y}");
    }

    #[test]
    fn attenuates_well_above_cutoff() {
        let mut f = DiodeLadder4Pole::new(SR);
        f.set_cutoff(200.0);
        f.set_resonance(0.0);
        let freq = 4_000.0;
        let omega = 2.0 * PI * freq / SR;
        let mut peak_in = 0.0f32;
        let mut peak_out = 0.0f32;
        for i in 0..8_000 {
            let x = (omega * i as f32).sin() * 0.3;
            let y = f.process(x);
            if i > 1_000 {
                peak_in = peak_in.max(x.abs());
                peak_out = peak_out.max(y.abs());
            }
        }
        // 4-pole LP at >1 decade above cutoff: very strong attenuation.
        let ratio = peak_out / peak_in;
        assert!(ratio < 0.05, "expected strong attenuation, got ratio {ratio}");
    }

    #[test]
    fn stable_at_extreme_cutoffs_and_high_resonance() {
        let mut f = DiodeLadder4Pole::new(SR);
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

    /// Full resonance must show a strong boosted peak when driven at the
    /// cutoff frequency, and must ring down to silence — matching authentic
    /// TB-303 behaviour. The filter MUST NOT self-oscillate at any setting.
    #[test]
    fn strong_resonance_peak_no_self_osc() {
        // Drive at the cutoff with a short burst, then measure the response
        // peak (resonance boost) and the eventual decay to silence.
        let fc = 500.0;
        let omega = 2.0 * PI * fc / SR;
        let mut f = DiodeLadder4Pole::new(SR);
        f.set_cutoff(fc);
        f.set_resonance(1.0);
        // Reference: same test at zero resonance.
        let mut f_flat = DiodeLadder4Pole::new(SR);
        f_flat.set_cutoff(fc);
        f_flat.set_resonance(0.0);

        let mut peak_resonant = 0.0f32;
        let mut peak_flat = 0.0f32;
        for i in 0..8_000 {
            let x = (omega * i as f32).sin() * 0.2;
            let yr = f.process(x);
            let yf = f_flat.process(x);
            if i > 1_000 {
                peak_resonant = peak_resonant.max(yr.abs());
                peak_flat = peak_flat.max(yf.abs());
            }
        }
        // Resonance should boost the peak significantly vs flat response.
        assert!(
            peak_resonant > peak_flat * 1.5,
            "resonance should boost the peak: flat={peak_flat:.4}, resonant={peak_resonant:.4}"
        );

        // After removing the drive signal, feed silence — must decay to zero.
        for _ in 0..24_000 {
            f.process(0.0);
        }
        let mut late_energy = 0.0f32;
        for _ in 0..4_000 {
            late_energy += f.process(0.0).powi(2);
        }
        let late_rms = (late_energy / 4_000.0_f32).sqrt();
        assert!(
            late_rms < 1e-4,
            "filter must ring down, not self-oscillate: late_rms={late_rms}"
        );
    }

    #[test]
    fn resonance_peak_boosts_near_cutoff() {
        let fc = 800.0;
        let omega = 2.0 * PI * fc / SR;
        let render = |res: f32| -> f32 {
            let mut f = DiodeLadder4Pole::new(SR);
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
            "resonance should boost near cutoff: flat={flat}, peaky={peaky}"
        );
    }

    /// The DC-blocking feedback path should reduce resonance at very low
    /// frequencies compared with mid-range cutoff.
    #[test]
    fn dc_blocking_reduces_very_low_freq_resonance() {
        let render_peak = |fc_hz: f32, tone_hz: f32| -> f32 {
            let mut f = DiodeLadder4Pole::new(SR);
            f.set_cutoff(fc_hz);
            f.set_resonance(0.9);
            let omega = 2.0 * PI * tone_hz / SR;
            let mut peak = 0.0f32;
            for i in 0..24_000 {
                let x = (omega * i as f32).sin() * 0.2;
                let y = f.process(x);
                if i > 8_000 {
                    peak = peak.max(y.abs());
                }
            }
            peak
        };
        let lo = render_peak(20.0, 20.0);
        let mid = render_peak(200.0, 200.0);
        assert!(
            mid > lo * 1.5,
            "DC blocker should weaken very-low-freq resonance: lo={lo:.4}, mid={mid:.4}"
        );
    }
}
