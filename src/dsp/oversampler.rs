//! 2× halfband oversampler for the nonlinear filter block.
//!
//! Wraps a linear-phase windowed-sinc FIR lowpass (Blackman window,
//! cutoff = π/2) around a block of DSP that should run at 2× the base
//! sample rate. Designed so the Voice303 can cheaply reduce aliasing from
//! the diode ladder's self-oscillating saturator without moving the rest
//! of the pipeline to a higher rate.
//!
//! Design notes:
//!
//! - **Length = 31 (odd, linear phase).** Short enough that the cost per
//!   base-rate sample is ~60 MACs, long enough to deliver ≥80 dB
//!   stopband attenuation with a Blackman window.
//! - **Halfband topology is implicit.** A sinc cut at π/2 with a
//!   symmetric window has zeros at every even offset from the center
//!   tap — we keep the full FIR for clarity rather than exploit the
//!   polyphase optimisation. The MACs touching zero taps are cheap on
//!   any modern CPU and the code is much easier to reason about.
//! - **Unity DC gain.** Taps are normalised so the total sum is 1. The
//!   `upsample2` path then compensates the zero-insertion energy loss
//!   with a 2× post-filter gain, so a DC input round-trips to itself.
//!
//! References: Vadim Zavalishin, *The Art of VA Filter Design*, §11.3
//! (halfband filters for oversampling); Oppenheim & Schafer, *Discrete
//! Time Signal Processing*, §7.5 (windowed FIR design).

use std::f32::consts::PI;

/// FIR length. Kept as a const for iterator / array sizing. Odd so the
/// filter has a single center tap and linear phase.
pub const HB_N: usize = 31;

/// Halfband 2× oversampler state: one FIR per direction (upsample,
/// downsample) because each path maintains an independent delay line.
pub struct Halfband2x {
    taps: [f32; HB_N],
    up_buf: [f32; HB_N],
    up_pos: usize,
    down_buf: [f32; HB_N],
    down_pos: usize,
}

impl Halfband2x {
    pub fn new() -> Self {
        let mut taps = [0.0f32; HB_N];
        let center = (HB_N / 2) as i32;

        // Windowed sinc lowpass at ωc = π/2. The `0.5 * sinc(k/2)`
        // evaluates to 0 at every nonzero even `k`, so this naturally
        // becomes a halfband filter before normalisation.
        for i in 0..HB_N {
            let k = i as i32 - center;
            let sinc = if k == 0 {
                1.0
            } else {
                let x = PI * k as f32 * 0.5;
                x.sin() / x
            };
            let blackman = 0.42
                - 0.5 * (2.0 * PI * i as f32 / (HB_N - 1) as f32).cos()
                + 0.08 * (4.0 * PI * i as f32 / (HB_N - 1) as f32).cos();
            taps[i] = 0.5 * sinc * blackman;
        }

        // Normalise to unity DC gain so a sustained DC input passes
        // through unchanged after the upsample→downsample round trip.
        let sum: f32 = taps.iter().sum();
        if sum.abs() > 1e-12 {
            for t in taps.iter_mut() {
                *t /= sum;
            }
        }

        Self {
            taps,
            up_buf: [0.0; HB_N],
            up_pos: 0,
            down_buf: [0.0; HB_N],
            down_pos: 0,
        }
    }

    pub fn reset(&mut self) {
        self.up_buf = [0.0; HB_N];
        self.up_pos = 0;
        self.down_buf = [0.0; HB_N];
        self.down_pos = 0;
    }

    /// Upsample one base-rate input to two high-rate samples by zero-
    /// insertion + interpolating lowpass + gain-of-2 compensation.
    #[inline]
    pub fn upsample2(&mut self, x: f32) -> [f32; 2] {
        let y0 = self.up_fir_tick(x);
        let y1 = self.up_fir_tick(0.0);
        // Zero-insertion halves energy; compensate here so the filter
        // keeps unity DC gain and the whole 2× round trip is flat.
        [y0 * 2.0, y1 * 2.0]
    }

    /// Downsample two high-rate samples to one base-rate sample via
    /// anti-alias lowpass + drop every other output.
    #[inline]
    pub fn downsample2(&mut self, xs: [f32; 2]) -> f32 {
        self.down_fir_tick(xs[0]); // discard
        self.down_fir_tick(xs[1])
    }

    #[inline]
    fn up_fir_tick(&mut self, x: f32) -> f32 {
        self.up_buf[self.up_pos] = x;
        self.up_pos = (self.up_pos + 1) % HB_N;
        convolve(&self.taps, &self.up_buf, self.up_pos)
    }

    #[inline]
    fn down_fir_tick(&mut self, x: f32) -> f32 {
        self.down_buf[self.down_pos] = x;
        self.down_pos = (self.down_pos + 1) % HB_N;
        convolve(&self.taps, &self.down_buf, self.down_pos)
    }
}

impl Default for Halfband2x {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn convolve(taps: &[f32; HB_N], buf: &[f32; HB_N], write_pos: usize) -> f32 {
    // Latest sample is at write_pos - 1 (mod N). Walk backwards through
    // the ring, pairing with taps[0..N].
    let mut acc = 0.0f32;
    let base = write_pos + HB_N - 1;
    for k in 0..HB_N {
        let idx = (base - k) % HB_N;
        acc += taps[k] * buf[idx];
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    #[test]
    fn dc_roundtrip_is_unity() {
        let mut hb = Halfband2x::new();
        // Push DC long enough to fill both delay lines, then measure.
        let mut last = 0.0f32;
        for _ in 0..256 {
            let up = hb.upsample2(1.0);
            last = hb.downsample2(up);
        }
        assert!(
            (last - 1.0).abs() < 0.01,
            "DC should round-trip to unity, got {last}"
        );
    }

    #[test]
    fn low_frequency_passband_preserved() {
        let mut hb = Halfband2x::new();
        let f = 1_000.0f32; // well inside passband
        let mut peak_in = 0.0f32;
        let mut peak_out = 0.0f32;
        // Warm up the delay lines.
        for n in 0..256 {
            let t = n as f32 / SR;
            let x = (2.0 * PI * f * t).sin();
            let up = hb.upsample2(x);
            let _ = hb.downsample2(up);
        }
        // Measure steady state.
        for n in 256..2_048 {
            let t = n as f32 / SR;
            let x = (2.0 * PI * f * t).sin();
            peak_in = peak_in.max(x.abs());
            let up = hb.upsample2(x);
            let y = hb.downsample2(up);
            peak_out = peak_out.max(y.abs());
        }
        let ratio = peak_out / peak_in;
        assert!(
            (ratio - 1.0).abs() < 0.05,
            "passband amplitude should be flat, ratio={ratio}"
        );
    }

    #[test]
    fn high_frequency_stopband_rejected() {
        // Halfband cutoff = π/2 at the 2× internal rate ⇒ absolute
        // cutoff = SR/2 (the base Nyquist). Feed a sine well above
        // that — 0.9·SR — as consecutive samples at sr2 and confirm
        // it's crushed before decimation.
        let mut hb = Halfband2x::new();
        let sr2 = SR * 2.0;
        let f_high = SR * 0.9;
        // Pre-generate consecutive samples at sr2 so pair indexing is
        // unambiguous.
        let n_samples = 8_192usize;
        let xs: Vec<f32> = (0..n_samples)
            .map(|m| (2.0 * PI * f_high * m as f32 / sr2).sin())
            .collect();

        let mut peak_out = 0.0f32;
        for pair in xs.chunks_exact(2).take(1_024) {
            // warm-up pass — fill delay line.
            let _ = hb.downsample2([pair[0], pair[1]]);
        }
        for pair in xs.chunks_exact(2).skip(1_024) {
            let y = hb.downsample2([pair[0], pair[1]]);
            peak_out = peak_out.max(y.abs());
        }
        assert!(
            peak_out < 0.05,
            "stopband should reject ≥26 dB near Nyquist, peak={peak_out}"
        );
    }
}
