//! Parameter definitions for SquelchBox.
//!
//! M1 drop: master volume + the six classic TB-303 knobs (Tuning, Cutoff,
//! Resonance, Env Mod, Decay, Accent) + waveform select. FX, sequencer, and
//! Under-the-Hood params arrive in later milestones.

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

use crate::dsp::voice::Waveform;

/// Transport / sequencer source. Phoscyon-style: HOST follows the DAW,
/// INTERNAL is the standalone-style free-run sequencer, MIDI disables
/// the sequencer entirely so the voice only plays from incoming MIDI
/// notes. The `process()` loop branches on this each block.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    #[id = "internal"]
    #[name = "Internal"]
    Internal,
    #[id = "host"]
    #[name = "Host"]
    Host,
    #[id = "midi"]
    #[name = "MIDI"]
    Midi,
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveformParam {
    #[id = "saw"]
    #[name = "Saw"]
    Saw,
    #[id = "square"]
    #[name = "Square"]
    Square,
}

impl From<WaveformParam> for Waveform {
    fn from(w: WaveformParam) -> Self {
        match w {
            WaveformParam::Saw => Waveform::Saw,
            WaveformParam::Square => Waveform::Square,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelayModeParam {
    #[id = "clean"]
    #[name = "Clean"]
    Clean,
    #[id = "analog"]
    #[name = "Analog"]
    Analog,
}

impl From<DelayModeParam> for crate::dsp::fx::delay::DelayMode {
    fn from(m: DelayModeParam) -> Self {
        match m {
            DelayModeParam::Clean => Self::Clean,
            DelayModeParam::Analog => Self::Analog,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelaySyncParam {
    #[id = "quarter"]
    #[name = "1/4"]
    Quarter,
    #[id = "eighth"]
    #[name = "1/8"]
    Eighth,
    #[id = "dotted_eighth"]
    #[name = "1/8d"]
    DottedEighth,
    #[id = "sixteenth"]
    #[name = "1/16"]
    Sixteenth,
    #[id = "triplet_eighth"]
    #[name = "1/8t"]
    TripletEighth,
}

impl From<DelaySyncParam> for crate::dsp::fx::delay::SyncDiv {
    fn from(s: DelaySyncParam) -> Self {
        match s {
            DelaySyncParam::Quarter => Self::Quarter,
            DelaySyncParam::Eighth => Self::Eighth,
            DelaySyncParam::DottedEighth => Self::DottedEighth,
            DelaySyncParam::Sixteenth => Self::Sixteenth,
            DelaySyncParam::TripletEighth => Self::TripletEighth,
        }
    }
}

#[derive(Params)]
pub struct SquelchBoxParams {
    #[persist = "editor-state-v4"]
    pub editor_state: Arc<EguiState>,

    /// JSON-serialized `Pattern`. The plugin reads this in `initialize()`
    /// to restore a DAW-saved sequence; the editor rewrites it whenever
    /// `KbdQueue::pattern_rev` changes. Empty string = no saved state →
    /// fall back to the default classic riff.
    #[persist = "pattern-v1"]
    pub pattern_state: Arc<parking_lot::Mutex<String>>,

    #[id = "master_vol"]
    pub master_volume: FloatParam,

    #[id = "waveform"]
    pub waveform: EnumParam<WaveformParam>,

    /// Sequencer transport source: Internal / Host / MIDI.
    #[id = "sync_mode"]
    pub sync_mode: EnumParam<SyncMode>,

    /// Global tuning offset in semitones, ±12.
    #[id = "tuning"]
    pub tuning: FloatParam,

    /// Base filter cutoff, pre-envmod. Log-scaled 30 Hz..12 kHz.
    #[id = "cutoff"]
    pub cutoff: FloatParam,

    /// Filter resonance 0..1. 1.0 ≈ self-oscillation.
    #[id = "resonance"]
    pub resonance: FloatParam,

    /// Env Mod amount 0..1 — how far the filter env opens the cutoff.
    #[id = "env_mod"]
    pub env_mod: FloatParam,

    /// Amp/filter decay in ms (shared by both envelopes in a real 303).
    #[id = "decay"]
    pub decay_ms: FloatParam,

    /// Accent amount 0..1 — amp/cutoff/reso boost on accented steps.
    #[id = "accent"]
    pub accent: FloatParam,

    /// Portamento glide time in ms — how long an oscillator takes to
    /// travel between two slide-legato notes. Not on the M7-lite front
    /// panel yet; M7 full UI will surface it.
    #[id = "slide"]
    pub slide_ms: FloatParam,

    // ─── Sequencer (M5) ───────────────────────────────────────────
    // Pattern data itself is not yet a parameter — it lives as
    // runtime state on the plugin until M7 wires the grid UI and a
    // `PersistentField` serializer. For now the sequencer's tempo,
    // swing and gate length ARE host-automatable via these knobs so
    // Renoise can sweep them and the standalone can expose them in
    // the front panel when M7 lands.

    /// Internal sequencer tempo (BPM). Used when host transport is
    /// not driving us (standalone mode) or as a fallback.
    #[id = "seq_bpm"]
    pub seq_bpm: FloatParam,

    /// Swing 0..75%. See `Clock::set_swing`.
    #[id = "seq_swing"]
    pub seq_swing: FloatParam,

    /// Gate length as a fraction of the step, 1..100%. 100% = tied.
    #[id = "seq_gate"]
    pub seq_gate: FloatParam,

    // ─── FX (M6) ─────────────────────────────────────────────────

    #[id = "dist_enable"]
    pub dist_enable: BoolParam,
    #[id = "dist_drive"]
    pub dist_drive: FloatParam,
    #[id = "dist_mix"]
    pub dist_mix: FloatParam,

    #[id = "delay_enable"]
    pub delay_enable: BoolParam,
    #[id = "delay_mode"]
    pub delay_mode: EnumParam<DelayModeParam>,
    #[id = "delay_sync"]
    pub delay_sync: EnumParam<DelaySyncParam>,
    #[id = "delay_feedback"]
    pub delay_feedback: FloatParam,
    #[id = "delay_mix"]
    pub delay_mix: FloatParam,

    #[id = "reverb_enable"]
    pub reverb_enable: BoolParam,
    #[id = "reverb_decay"]
    pub reverb_decay: FloatParam,
    #[id = "reverb_mix"]
    pub reverb_mix: FloatParam,
}

impl Default for SquelchBoxParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(crate::ui::BASE_W, crate::ui::BASE_H),
            pattern_state: Arc::new(parking_lot::Mutex::new(String::new())),

            master_volume: FloatParam::new(
                "Master Volume",
                util::db_to_gain(-3.0),
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

            waveform: EnumParam::new("Waveform", WaveformParam::Saw),
            sync_mode: EnumParam::new("Sync", SyncMode::Internal),

            tuning: FloatParam::new(
                "Tuning",
                0.0,
                FloatRange::Linear { min: -12.0, max: 12.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" st")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            cutoff: FloatParam::new(
                "Cutoff",
                500.0,
                FloatRange::Skewed {
                    min: 30.0,
                    max: 5_500.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            resonance: FloatParam::new(
                "Resonance",
                0.6,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            env_mod: FloatParam::new(
                "Env Mod",
                0.6,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            decay_ms: FloatParam::new(
                "Decay",
                200.0,
                FloatRange::Skewed {
                    min: 30.0,
                    max: 2_500.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            accent: FloatParam::new(
                "Accent",
                0.6,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            slide_ms: FloatParam::new(
                "Slide",
                60.0,
                FloatRange::Skewed {
                    min: 5.0,
                    max: 500.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            seq_bpm: FloatParam::new(
                "Seq BPM",
                120.0,
                FloatRange::Linear { min: 40.0, max: 220.0 },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_unit(" BPM")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            seq_swing: FloatParam::new(
                "Seq Swing",
                0.0,
                FloatRange::Linear { min: 0.0, max: 0.75 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            seq_gate: FloatParam::new(
                "Seq Gate",
                0.5,
                FloatRange::Linear { min: 0.05, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            dist_enable: BoolParam::new("Dist Enable", false),
            dist_drive: FloatParam::new(
                "Dist Drive",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            dist_mix: FloatParam::new(
                "Dist Mix",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            delay_enable: BoolParam::new("Delay Enable", false),
            delay_mode: EnumParam::new("Delay Mode", DelayModeParam::Analog),
            delay_sync: EnumParam::new("Delay Sync", DelaySyncParam::Eighth),

            delay_feedback: FloatParam::new(
                "Delay Feedback",
                0.4,
                FloatRange::Linear { min: 0.0, max: 0.9 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            delay_mix: FloatParam::new(
                "Delay Mix",
                0.3,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            reverb_enable: BoolParam::new("Reverb Enable", false),

            reverb_decay: FloatParam::new(
                "Reverb Decay",
                0.4,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            reverb_mix: FloatParam::new(
                "Reverb Mix",
                0.2,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }
}

impl SquelchBoxParams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Seed every smoother's `current` state to its target value.
    ///
    /// **Critical for the standalone build.** nih-plug's `Smoother`
    /// defaults `current` to 0.0, and `SmoothingStyle::Logarithmic::next`
    /// is literally `current * step_size` — so a freshly-constructed
    /// logarithmic smoother is stuck at zero forever unless explicitly
    /// reset. `master_volume` and `cutoff` are both logarithmic here,
    /// so without this call the standalone is dead silent (the VST3
    /// in Renoise escapes it because the host writes every param on
    /// load, dragging the smoothers off zero).
    pub fn seed_smoothers(&self) {
        self.master_volume.smoothed.reset(self.master_volume.value());
        self.cutoff.smoothed.reset(self.cutoff.value());
        self.resonance.smoothed.reset(self.resonance.value());
        self.env_mod.smoothed.reset(self.env_mod.value());
        self.tuning.smoothed.reset(self.tuning.value());
        self.decay_ms.smoothed.reset(self.decay_ms.value());
        self.accent.smoothed.reset(self.accent.value());
        self.slide_ms.smoothed.reset(self.slide_ms.value());
        self.seq_bpm.smoothed.reset(self.seq_bpm.value());
        self.seq_swing.smoothed.reset(self.seq_swing.value());
        self.seq_gate.smoothed.reset(self.seq_gate.value());
        self.dist_drive.smoothed.reset(self.dist_drive.value());
        self.dist_mix.smoothed.reset(self.dist_mix.value());
        self.delay_feedback.smoothed.reset(self.delay_feedback.value());
        self.delay_mix.smoothed.reset(self.delay_mix.value());
        self.reverb_decay.smoothed.reset(self.reverb_decay.value());
        self.reverb_mix.smoothed.reset(self.reverb_mix.value());
    }

    /// Snapshot FX params for the per-sample FX chain.
    pub fn snapshot_fx_params(&self) -> crate::dsp::fx::fx_chain::FxParams {
        crate::dsp::fx::fx_chain::FxParams {
            dist_enable: self.dist_enable.value(),
            dist_drive: self.dist_drive.smoothed.next(),
            dist_mix: self.dist_mix.smoothed.next(),
            delay_enable: self.delay_enable.value(),
            delay_mode: self.delay_mode.value().into(),
            delay_feedback: self.delay_feedback.smoothed.next(),
            delay_mix: self.delay_mix.smoothed.next(),
            reverb_enable: self.reverb_enable.value(),
            reverb_decay: self.reverb_decay.smoothed.next(),
            reverb_mix: self.reverb_mix.smoothed.next(),
        }
    }

    /// Snapshot the current smoothed knob values into a `VoiceParams` to
    /// hand to `Voice303::trigger()`. Pulled once per note-on so the voice
    /// has stable per-trigger values.
    pub fn snapshot_voice_params(&self) -> crate::dsp::voice::VoiceParams {
        crate::dsp::voice::VoiceParams {
            waveform: self.waveform.value().into(),
            base_cutoff_hz: self.cutoff.smoothed.next(),
            resonance: self.resonance.smoothed.next(),
            env_mod: self.env_mod.smoothed.next(),
            decay_ms: self.decay_ms.smoothed.next(),
            filter_curve: 2.0,
            tuning_semitones: self.tuning.smoothed.next(),
            accent_amount: self.accent.smoothed.next(),
        }
    }
}
