//! Keyboard input handling, pattern persistence, and helper functions.

use nih_plug::prelude::*;
use nih_plug_egui::egui;

use crate::kbd::{key_to_semitone, KbdEvent, KbdQueue};
use crate::params::SquelchBoxParams;

use super::ids;

pub fn midi_note_name(n: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = (n as i32 / 12) - 1;
    let name = NAMES[(n % 12) as usize];
    format!("{name}{octave}")
}

/// Re-serialize the entire pattern bank into `params.pattern_state`
/// whenever `KbdQueue::pattern_rev` changes. Tracked in egui temp data
/// so we don't pay for serde on every frame, only on actual edits. Runs
/// on the GUI thread, so allocation is fine.
pub fn persist_pattern_if_changed(
    ctx: &egui::Context,
    params: &SquelchBoxParams,
    kbd: &KbdQueue,
) {
    let id = ids::last_persist_rev();
    let last: u64 = ctx.data(|d| d.get_temp(id)).unwrap_or(u64::MAX);
    let cur = kbd.pattern_rev();
    if last == cur {
        return;
    }
    let snap = kbd.bank_snapshot();
    if let Ok(json) = serde_json::to_string(&snap) {
        *params.pattern_state.lock() = json;
    }
    ctx.data_mut(|d| d.insert_temp(id, cur));
}

/// Generate a fresh random pattern and push it through the editor
/// queue. Seed is derived from `ctx.input().time` so every click yields
/// a different riff (no `rand` dep needed).
pub fn randomize_pattern(ctx: &egui::Context, kbd: &KbdQueue) {
    let t = ctx.input(|i| i.time);
    let seed = (t * 1_000_000.0) as u64 ^ 0xA5A5_5A5A_DEAD_BEEFu64;
    let pat = crate::sequencer::Pattern::random(seed, 0.65, 0.35, 0.25, 36);
    kbd.edit_pattern(move |p| {
        let keep_len = p.length;
        let keep_swing = p.swing;
        *p = pat;
        p.length = keep_len;
        p.swing = keep_swing;
    });
}

pub fn handle_keyboard(ctx: &egui::Context, kbd: &KbdQueue) {
    ctx.request_repaint();
    ctx.memory_mut(|m| {
        if m.focused().is_none() {
            m.request_focus(ids::kbd_focus());
        }
    });
    let focused = ctx.memory(|m| m.focused().is_some());
    let prev_id = ids::prev_keys();
    let prev: std::collections::HashSet<egui::Key> = ctx
        .data(|d| d.get_temp::<std::collections::HashSet<egui::Key>>(prev_id))
        .unwrap_or_default();
    let (cur, events_len, shift) = ctx.input(|i| {
        let c: std::collections::HashSet<egui::Key> = i.keys_down.iter().copied().collect();
        (c, i.events.len(), i.modifiers.shift)
    });
    kbd.set_diag(events_len, cur.len(), focused);
    // Keys that have an outstanding gate_on we pushed to the audio
    // thread. Used to emit gate_off once the last note key is released,
    // so held notes don't sustain forever (AmpEnv is gate-driven and
    // only releases on an explicit `on: false` event).
    let sounding_id = ids::sounding_keys();
    let mut sounding: std::collections::HashSet<egui::Key> = ctx
        .data(|d| d.get_temp::<std::collections::HashSet<egui::Key>>(sounding_id))
        .unwrap_or_default();
    let edit_mode = kbd.selected_step().is_some();
    for key in cur.difference(&prev) {
        kbd.mark_key(&format!("{key:?}"));

        // ── Edit mode: step is selected ──
        if edit_mode {
            if matches!(
                key,
                egui::Key::ArrowUp
                    | egui::Key::ArrowDown
                    | egui::Key::ArrowLeft
                    | egui::Key::ArrowRight
                    | egui::Key::A
                    | egui::Key::S
                    | egui::Key::R
                    | egui::Key::Delete
                    | egui::Key::Backspace
                    | egui::Key::Escape
            ) {
                continue;
            }
            // T = trigger/preview the selected step's note
            if *key == egui::Key::T {
                if let Some(sel) = kbd.selected_step() {
                    let pat = kbd.pattern_snapshot();
                    let s = pat.steps[sel];
                    if !s.rest {
                        let velocity = if s.accent { 0.95 } else { 0.7 };
                        kbd.push(KbdEvent { on: true, note: s.semitone, velocity });
                        sounding.insert(*key);
                    }
                }
                continue;
            }
            // Note keys: write pitch onto selected step + audition
            if let Some(semi) = key_to_semitone(*key) {
                if let Some(sel) = kbd.selected_step() {
                    let base = 12 * (kbd.octave() as i32 + 1);
                    let note = (base + semi).clamp(24, 60) as u8;
                    kbd.edit_pattern(|p| {
                        p.steps[sel].semitone = note;
                        p.steps[sel].rest = false;
                    });
                    let velocity = if shift { 0.95 } else { 0.7 };
                    kbd.push(KbdEvent { on: true, note, velocity });
                    sounding.insert(*key);
                }
                continue;
            }
        }

        match key {
            egui::Key::ArrowDown => {
                let o = kbd.octave();
                kbd.set_octave(o - 1);
            }
            egui::Key::ArrowUp => {
                let o = kbd.octave();
                kbd.set_octave(o + 1);
            }
            egui::Key::P | egui::Key::Space => {
                kbd.toggle_seq_run();
            }
            egui::Key::Enter => {
                kbd.request_rewind();
            }
            egui::Key::OpenBracket => {
                kbd.edit_pattern(|p| p.rotate_left(1));
            }
            egui::Key::CloseBracket => {
                kbd.edit_pattern(|p| p.rotate_right(1));
            }
            egui::Key::Backtick => {
                randomize_pattern(ctx, kbd);
            }
            _ => {
                if let Some(semi) = key_to_semitone(*key) {
                    let base = 12 * (kbd.octave() as i32 + 1);
                    let note = (base + semi).clamp(0, 127) as u8;
                    let velocity = if shift { 0.95 } else { 0.7 };
                    kbd.push(KbdEvent { on: true, note, velocity });
                    sounding.insert(*key);
                }
            }
        }
    }

    // Monosynth gate-release: drop any sounding keys that are no
    // longer held, and when the set transitions to empty push a single
    // `on: false` so the voice's amp env releases.
    let was_sounding = !sounding.is_empty();
    sounding.retain(|k| cur.contains(k));
    if was_sounding && sounding.is_empty() {
        kbd.push(KbdEvent { on: false, note: 0, velocity: 0.0 });
    }
    // ── T-held tracking for audition-while-scrubbing ──
    let t_id = ids::t_held();
    let mut t_held: bool = ctx.data(|d| d.get_temp(t_id)).unwrap_or(false);

    let t_in_keys_down = cur.contains(&egui::Key::T);
    let t_event_press = ctx.input(|i| {
        i.events.iter().any(|ev| {
            matches!(
                ev,
                egui::Event::Key {
                    key: egui::Key::T,
                    pressed: true,
                    ..
                }
            )
        })
    });

    if t_in_keys_down || t_event_press {
        t_held = true;
    } else if !shift {
        t_held = false;
    }

    ctx.data_mut(|d| {
        d.insert_temp(prev_id, cur);
        d.insert_temp(t_id, t_held);
        d.insert_temp(sounding_id, sounding);
    });
}

/// STEP button: select the current step and audition it without
/// advancing. Use NEXT/BACK to move, STEP to listen.
pub fn single_step_audition(kbd: &KbdQueue) {
    let pat = kbd.pattern_snapshot();
    let len = pat.length.max(1) as usize;
    let idx = kbd
        .selected_step()
        .unwrap_or_else(|| (kbd.current_step() % len as u64) as usize);
    kbd.set_selected_step(idx);
    kbd.set_current_step(idx as u64);
    let s = pat.steps[idx];
    if !s.rest {
        let velocity = if s.accent { 0.95 } else { 0.7 };
        kbd.push(crate::kbd::KbdEvent { on: true, note: s.semitone, velocity });
    }
}

/// TAP button: store recent tap timestamps in egui temp data, average
/// the intervals (rejecting any > 2s as a streak break), and write the
/// resulting BPM to the seq_bpm parameter.
pub fn handle_tap_tempo(
    ctx: &egui::Context,
    setter: &ParamSetter,
    bpm: &nih_plug::params::FloatParam,
) {
    const MAX_GAP: f64 = 2.0;
    const HISTORY: usize = 4;
    let id = ids::tap_history();
    let now = ctx.input(|i| i.time);
    let mut taps: Vec<f64> = ctx
        .data(|d| d.get_temp::<Vec<f64>>(id))
        .unwrap_or_default();
    if let Some(&last) = taps.last() {
        if now - last > MAX_GAP {
            taps.clear();
        }
    }
    taps.push(now);
    while taps.len() > HISTORY {
        taps.remove(0);
    }
    if taps.len() >= 2 {
        let mut sum = 0.0f64;
        for w in taps.windows(2) {
            sum += w[1] - w[0];
        }
        let avg = sum / (taps.len() - 1) as f64;
        if avg > 0.0 {
            let bpm_val = (60.0 / avg).clamp(40.0, 220.0) as f32;
            setter.begin_set_parameter(bpm);
            setter.set_parameter(bpm, bpm_val);
            setter.end_set_parameter(bpm);
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, taps));
}
