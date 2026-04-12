//! SquelchBox `Plugin` impl.
//!
//! M1 drop: wires MIDI note-on/note-off through to a single `Voice303`.
//! The voice runs the oscillator + envelopes + placeholder 1-pole lowpass
//! — enough to hear that the full pipeline works end-to-end. The real
//! 3-pole diode ladder arrives in M2, sequencer in M5, FX in M6.

use nih_plug::prelude::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::dsp::fx::fx_chain::FxChain;
use crate::dsp::voice::{QualityMode, Voice303, VoiceLiveParams};
use crate::kbd::{KbdEvent, KbdQueue};
use crate::logging;
use crate::params::{SquelchBoxParams, SyncMode};
use crate::sequencer::{SeqTrigger, Sequencer};

pub struct SquelchBox {
    params: Arc<SquelchBoxParams>,
    voice: Voice303,
    sample_rate: f32,
    kbd_queue: Arc<KbdQueue>,
    kbd_scratch: Vec<KbdEvent>,
    sequencer: Sequencer,
    /// Last pattern revision the audio thread has ingested. Compared
    /// against `KbdQueue::pattern_rev()` each block — if they differ we
    /// try to snapshot the shared pattern into the sequencer.
    last_pattern_rev: u64,
    /// Has the host transport ever reported `playing = false`? Standalone
    /// backends (CPAL, JACK/PipeWire) report `playing = true` forever, so
    /// in HOST sync mode we only follow the transport once we've seen at
    /// least one stop edge — otherwise the standalone would auto-play.
    host_ever_stopped: bool,
    /// Edge-detect for the HOST-driven clock so we reset to step 0 on
    /// every play press, matching INTERNAL behavior.
    host_was_running: bool,
    fx_chain: FxChain,
}

impl Default for SquelchBox {
    fn default() -> Self {
        Self {
            params: SquelchBoxParams::new(),
            voice: Voice303::new(44_100.0),
            sample_rate: 44_100.0,
            kbd_queue: KbdQueue::new(),
            kbd_scratch: Vec::with_capacity(64),
            sequencer: Sequencer::new(44_100.0),
            last_pattern_rev: 0,
            host_ever_stopped: false,
            host_was_running: false,
            fx_chain: FxChain::new(44_100.0),
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

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        crate::ui::create(
            self.params.clone(),
            self.params.editor_state.clone(),
            self.kbd_queue.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.voice.set_sample_rate(self.sample_rate);
        self.sequencer.set_sample_rate(self.sample_rate);
        self.fx_chain.set_sample_rate(self.sample_rate);

        // Restore persisted pattern from the host's saved state. nih-plug
        // has already deserialized the `pattern_state` JSON into the
        // mutex by the time `initialize()` runs, so we can read it
        // synchronously here. Empty string = no saved state, keep the
        // default classic riff.
        let saved = self.params.pattern_state.lock().clone();
        if !saved.is_empty() {
            // Try the new bank format first; fall back to the v1 single
            // pattern format so saves from before the bank refactor still
            // load (they get parked in slot I).
            if let Ok(bank) = serde_json::from_str::<crate::sequencer::PatternBank>(&saved) {
                tracing::info!("restored persisted bank ({} bytes)", saved.len());
                self.kbd_queue.replace_bank(bank);
            } else if let Ok(pat) = serde_json::from_str::<crate::sequencer::Pattern>(&saved) {
                tracing::info!("restored legacy single-pattern save into slot I");
                self.kbd_queue.edit_pattern(|p| *p = pat);
            } else {
                tracing::warn!("failed to deserialize persisted pattern_state");
            }
        }
        // M3: default to 2× oversampling for the nonlinear filter
        // block. Hard-wired here until the M7 full-UI surfaces a
        // quality selector.
        self.voice.set_quality(QualityMode::High);
        self.params.seed_smoothers();
        logging::init();
        tracing::info!(
            "SquelchBox v{} initialized — sr: {}",
            Self::VERSION,
            self.sample_rate
        );
        true
    }

    fn reset(&mut self) {
        self.voice.reset();
        self.sequencer.reset();
        self.fx_chain.reset();
        self.params.seed_smoothers();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // ─── Pattern sync from editor ────────────────────────────
        // If the UI has edited the pattern since we last looked, try
        // to pull a fresh snapshot. Non-blocking: if the editor holds
        // the lock this block, we just use the last-known pattern and
        // try again next block.
        let rev = self.kbd_queue.pattern_rev();
        if rev != self.last_pattern_rev
            && self.kbd_queue.audio_sync_pattern(&mut self.sequencer.pattern)
        {
            self.last_pattern_rev = rev;
        }

        // ─── Sequencer per-block setup ───────────────────────────
        // Tempo/swing/gate are pulled once per block from the
        // smoothed params and pushed into the Clock. No need to be
        // sample-accurate — knob sweeps of these are already smoothed
        // upstream.
        self.sequencer
            .clock
            .set_swing(self.params.seq_swing.smoothed.next());
        self.sequencer
            .clock
            .set_gate_length(self.params.seq_gate.smoothed.next());

        // ─── Transport sync (Phoscyon-style modes) ───────────────
        // INTERNAL: free-run from the editor's RUN/STOP toggle, BPM
        //   from the seq_bpm knob.
        // HOST: follow the DAW. Tempo is slaved to transport.tempo;
        //   running is gated by `host_ever_stopped` so the standalone
        //   doesn't auto-play (CPAL/JACK report playing=true forever).
        //   On every host play→running edge we reset() so the pattern
        //   starts at step 1, mirroring INTERNAL's start-from-1 behavior.
        // MIDI: clock disabled; only incoming MIDI notes (and the
        //   computer-keyboard event queue) play the voice.
        let transport = context.transport();
        if !transport.playing {
            self.host_ever_stopped = true;
        }
        let mode = self.params.sync_mode.value();
        let (clock_running, clock_bpm) = match mode {
            SyncMode::Internal => (
                self.kbd_queue.is_seq_running(),
                self.params.seq_bpm.smoothed.next(),
            ),
            SyncMode::Host => {
                let host_driving = self.host_ever_stopped && transport.playing;
                let bpm = transport
                    .tempo
                    .map(|t| t as f32)
                    .unwrap_or_else(|| self.params.seq_bpm.smoothed.next());
                (host_driving, bpm)
            }
            SyncMode::Midi => (false, self.params.seq_bpm.smoothed.next()),
        };
        self.sequencer.clock.set_bpm(clock_bpm);
        self.sequencer.clock.set_running(clock_running);

        // ─── FX per-block setup ─────────────────────────────────
        self.fx_chain.set_delay_tempo(
            clock_bpm,
            self.params.delay_sync.value().into(),
        );

        if matches!(mode, SyncMode::Host) {
            if clock_running && !self.host_was_running {
                self.sequencer.reset();
            }
            self.host_was_running = clock_running;
        } else {
            self.host_was_running = false;
        }
        if self.kbd_queue.take_rewind() {
            self.sequencer.reset();
        }

        // Drain any computer-keyboard events the editor pushed since
        // last block. These are dispatched at i=0 of this block — sub-
        // block timing for GUI key events doesn't matter at typing speed.
        self.kbd_scratch.clear();
        self.kbd_queue.drain_into(&mut self.kbd_scratch);
        for ev in self.kbd_scratch.drain(..) {
            if ev.on {
                let accent = ev.velocity > 0.8;
                let vp = self.params.snapshot_voice_params();
                // Computer-keyboard input is percussive typing, not
                // host legato — always hard-retrigger so every press
                // gets a fresh amp-env attack. Using `slide_to` here
                // (which skips the amp retrigger) makes repeated same-
                // key presses feel like the input has frozen until the
                // decay expires.
                self.voice.trigger(ev.note, accent, &vp);
            } else {
                self.voice.gate_off();
            }
        }

        // Sample-accurate event handling: peek at the next event and
        // process samples up to that event's timing, then dispatch.
        let mut next_event = context.next_event();

        for (i, channel_samples) in buffer.iter_samples().enumerate() {
            // Dispatch any MIDI events that land on this sample.
            while let Some(ev) = next_event {
                if (ev.timing() as usize) > i {
                    break;
                }
                match ev {
                    NoteEvent::NoteOn { note, velocity, .. } => {
                        let accent = velocity > 0.8;
                        let vp = self.params.snapshot_voice_params();
                        // Overlapping note-ons glide: if the voice is
                        // still ringing, treat the new note as a 303
                        // slide-legato step instead of retriggering.
                        if self.voice.is_active() {
                            self.voice.slide_to(note, accent, &vp);
                        } else {
                            self.voice.trigger(note, accent, &vp);
                        }
                    }
                    NoteEvent::NoteOff { .. } => {
                        self.voice.gate_off();
                    }
                    _ => {}
                }
                next_event = context.next_event();
            }

            // Per-sample sequencer tick. Apply gate-off *before* any
            // new trigger on the same sample so the fresh note isn't
            // killed by its own step's preceding gate-off.
            let mut seq_tick = self.sequencer.tick();
            // Bar-quantized bank switch: when the playhead just
            // crossed into the first step of a fresh pattern loop,
            // apply any pending bank swap by replacing the audio
            // thread's pattern from the new slot, then re-emitting the
            // step-0 trigger so the new bank's first note plays on
            // exactly this sample. Falls through silently on lock
            // contention — we'll retry on the next loop.
            if let Some(abs) = seq_tick.boundary {
                let pat_len = self.sequencer.pattern.length.max(1) as u64;
                if abs % pat_len == 0 {
                    if let Some(slot) = self.kbd_queue.take_pending_bank() {
                        if self.kbd_queue
                            .swap_active_bank(slot, &mut self.sequencer.pattern)
                        {
                            self.last_pattern_rev = self.kbd_queue.pattern_rev();
                            seq_tick = self.sequencer.emit_for_step(abs);
                        } else {
                            // Re-arm the swap for the next sample.
                            self.kbd_queue.pending_bank.store(slot as i8, Ordering::Relaxed);
                        }
                    }
                }
            }
            if seq_tick.gate_off {
                self.voice.gate_off();
            }
            if let Some(trig) = seq_tick.trigger {
                let vp = self.params.snapshot_voice_params();
                match trig {
                    SeqTrigger::Hard { semitone, accent } => {
                        self.voice.trigger(semitone, accent, &vp);
                    }
                    SeqTrigger::Slide { semitone, accent } => {
                        if self.voice.is_active() {
                            self.voice.slide_to(semitone, accent, &vp);
                        } else {
                            self.voice.trigger(semitone, accent, &vp);
                        }
                    }
                }
            }

            // Per-sample live param pull: every front-panel knob the
            // user can sweep mid-note goes through here.
            let live = VoiceLiveParams {
                waveform: self.params.waveform.value().into(),
                base_cutoff_hz: self.params.cutoff.smoothed.next(),
                resonance: self.params.resonance.smoothed.next(),
                env_mod: self.params.env_mod.smoothed.next(),
                accent_amount: self.params.accent.smoothed.next(),
                tuning_semitones: self.params.tuning.smoothed.next(),
                decay_ms: self.params.decay_ms.smoothed.next(),
                slide_ms: self.params.slide_ms.smoothed.next(),
            };
            self.voice.set_live(&live);

            let gain = self.params.master_volume.smoothed.next();
            let voice_out = self.voice.tick();
            let fx_params = self.params.snapshot_fx_params();
            let fx_out = self.fx_chain.process(voice_out, &fx_params);
            let final_out = fx_out * gain;
            for sample in channel_samples {
                *sample = final_out;
            }
        }

        // Publish the current playhead step to the editor so the
        // sequencer strip can draw a moving cursor.
        self.kbd_queue
            .set_current_step(self.sequencer.clock.current_step());
        self.kbd_queue
            .set_step_phase(self.sequencer.clock.step_phase());

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
