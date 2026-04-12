//! Parameter definitions for SquelchBox.
//!
//! M0 drop: only a master volume. Full TB-303 parameter set (Tuning, Cutoff,
//! Resonance, EnvMod, Decay, Accent, FX, quality mode, Under-the-Hood, etc.)
//! is added in later milestones as the DSP engines come online.

use nih_plug::prelude::*;
use std::sync::Arc;

#[derive(Params)]
pub struct SquelchBoxParams {
    #[id = "master_vol"]
    pub master_volume: FloatParam,
}

impl Default for SquelchBoxParams {
    fn default() -> Self {
        Self {
            master_volume: FloatParam::new(
                "Master Volume",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-60.0),
                    max: util::db_to_gain(6.0),
                    factor: FloatRange::gain_skew_factor(-60.0, 6.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(10.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
        }
    }
}

impl SquelchBoxParams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}
