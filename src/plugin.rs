//! SquelchBox `Plugin` impl — M0 scaffolding.
//!
//! This drop exists to prove the build pipeline and plugin export work:
//! the plugin loads in a DAW, logs initialize, MIDI routing is declared,
//! and `process()` applies a smoothed master volume to silent output.
//! The voice, filter, sequencer, and FX modules all land in later
//! milestones and plug into this skeleton.

use nih_plug::prelude::*;
use std::sync::Arc;

use crate::logging;
use crate::params::SquelchBoxParams;

pub struct SquelchBox {
    params: Arc<SquelchBoxParams>,
    sample_rate: f32,
}

impl Default for SquelchBox {
    fn default() -> Self {
        Self {
            params: SquelchBoxParams::new(),
            sample_rate: 44_100.0,
        }
    }
}

impl Plugin for SquelchBox {
    const NAME: &'static str = "SquelchBox";
    const VENDOR: &'static str = "REXIST";
    const URL: &'static str = "https://github.com/natalia/squelchbox";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        logging::init();
        tracing::info!(
            "SquelchBox v{} initialized — sr: {}",
            Self::VERSION,
            self.sample_rate
        );
        true
    }

    fn reset(&mut self) {}

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // M0: silent output with smoothed master volume applied. No voice,
        // no sequencer, no FX yet — those arrive in M1..M6.
        for channel_samples in buffer.iter_samples() {
            let gain = self.params.master_volume.smoothed.next();
            for sample in channel_samples {
                *sample *= gain;
            }
        }
        ProcessStatus::Normal
    }
}

impl ClapPlugin for SquelchBox {
    const CLAP_ID: &'static str = "dev.rexist.squelchbox";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("TB-303-style bassline synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for SquelchBox {
    const VST3_CLASS_ID: [u8; 16] = *b"SquelchBox303v01";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Synth,
    ];
}
