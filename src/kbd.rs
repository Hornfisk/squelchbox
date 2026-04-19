//! Shared computer-keyboard → audio-thread event queue.
//!
//! Lets the standalone (and any host that forwards keystrokes) play the
//! synth from the QWERTY bottom row, Renoise-style. The editor pushes
//! events on egui key up/down; `process()` drains them at the top of
//! each audio block and dispatches them to the voice exactly like MIDI.

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI8, AtomicU32, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::sequencer::{Pattern, PatternBank};

#[derive(Clone, Copy, Debug)]
pub struct KbdEvent {
    pub on: bool,
    pub note: u8,
    pub velocity: f32,
}

pub struct KbdQueue {
    inner: Mutex<Vec<KbdEvent>>,
    octave: Mutex<i8>,
    last_key: Mutex<String>,
    /// Live four-slot pattern bank shared between editor and audio
    /// thread. Editor mutates on click/drag; audio thread snapshots the
    /// active slot via `try_lock` at the top of each process block.
    bank: Mutex<PatternBank>,
    /// Bumped on every pattern edit so the audio thread can cheaply
    /// tell whether it needs to re-snapshot.
    pattern_rev: AtomicU64,
    /// Bank slot the audio thread is currently playing (0..3). Mirrors
    /// `bank.lock().active`; published as an atomic so the GUI can
    /// highlight without taking the lock.
    pub active_bank_pub: AtomicU8,
    /// Pending bank switch: -1 = none, 0..3 = swap-to-this-slot at the
    /// next pattern-loop boundary. Audio thread takes it inside the
    /// per-sample loop on the step where the playhead crosses
    /// `current_step % pattern.length == 0`.
    pub pending_bank: AtomicI8,
    pub events_seen: AtomicUsize,
    pub keys_down: AtomicUsize,
    pub focused: AtomicBool,
    /// Sequencer run toggle. Editor flips this on `P`; plugin reads it
    /// each block and pushes into `Sequencer::clock.set_running`.
    pub seq_run: AtomicBool,
    /// Rewind pulse: editor sets to `true` on `Enter`, plugin reads +
    /// clears in its process loop.
    pub seq_rewind: AtomicBool,
    /// Absolute playhead step published by the plugin at the end of
    /// each process block so the editor can draw a moving cursor.
    pub seq_current_step: AtomicU64,
    /// Phase 0..1 within the current step, published alongside
    /// `seq_current_step`. Stored as `f32::to_bits` so the GUI can
    /// interpolate the playhead between published step boundaries
    /// without paying for a mutex.
    pub seq_step_phase: AtomicU32,
    /// Currently-selected step in the editor (0..=15), or `u32::MAX`
    /// for no selection. Keyboard shortcuts route to this step when set.
    pub selected_step: AtomicU32,
    /// Sequencer pitch-window octave (0..=2). Window covers
    /// 13 semis from `24 + view_oct*12` to `36 + view_oct*12` inclusive.
    /// Notes outside the window render as ▲/▼ markers.
    pub view_oct: AtomicU8,
}

impl Default for KbdQueue {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Vec::new()),
            octave: Mutex::new(3),
            last_key: Mutex::new(String::new()),
            bank: Mutex::new(PatternBank::default()),
            pattern_rev: AtomicU64::new(0),
            active_bank_pub: AtomicU8::new(0),
            pending_bank: AtomicI8::new(-1),
            events_seen: AtomicUsize::new(0),
            keys_down: AtomicUsize::new(0),
            focused: AtomicBool::new(false),
            seq_run: AtomicBool::new(false),
            seq_rewind: AtomicBool::new(false),
            seq_current_step: AtomicU64::new(0),
            seq_step_phase: AtomicU32::new(0),
            selected_step: AtomicU32::new(0),
            view_oct: AtomicU8::new(2),
        }
    }
}

impl KbdQueue {
    pub fn new() -> Arc<Self> {
        let q = Self::default();
        *q.octave.lock() = 3;
        Arc::new(q)
    }

    pub fn push(&self, ev: KbdEvent) {
        let mut g = self.inner.lock();
        if g.len() < 128 {
            g.push(ev);
        }
    }

    pub fn drain_into(&self, out: &mut Vec<KbdEvent>) {
        if let Some(mut g) = self.inner.try_lock() {
            out.extend(g.drain(..));
            nih_plug::util::permit_alloc(|| drop(g));
        }
    }

    pub fn octave(&self) -> i8 {
        *self.octave.lock()
    }

    pub fn set_octave(&self, o: i8) {
        *self.octave.lock() = o.clamp(0, 8);
    }

    pub fn mark_key(&self, s: &str) {
        *self.last_key.lock() = s.to_string();
    }

    pub fn last_key(&self) -> String {
        self.last_key.lock().clone()
    }

    pub fn set_diag(&self, events: usize, keys_down: usize, focused: bool) {
        self.events_seen.store(events, Ordering::Relaxed);
        self.keys_down.store(keys_down, Ordering::Relaxed);
        self.focused.store(focused, Ordering::Relaxed);
    }

    pub fn diag(&self) -> (usize, usize, bool) {
        (
            self.events_seen.load(Ordering::Relaxed),
            self.keys_down.load(Ordering::Relaxed),
            self.focused.load(Ordering::Relaxed),
        )
    }

    pub fn toggle_seq_run(&self) -> bool {
        let prev = self.seq_run.load(Ordering::Relaxed);
        let next = !prev;
        self.seq_run.store(next, Ordering::Relaxed);
        if next {
            // Starting: rewind to step 1 and clear the stopped-selection
            // so the white playhead ring is the only indicator.
            self.seq_rewind.store(true, Ordering::Relaxed);
            self.selected_step.store(u32::MAX, Ordering::Relaxed);
        } else {
            // Stopping: pre-select step 0 (rewind target) so STEP/BACK/NEXT
            // work immediately without needing the None→fallback path.
            self.selected_step.store(0, Ordering::Relaxed);
        }
        next
    }

    pub fn request_rewind(&self) {
        self.seq_rewind.store(true, Ordering::Relaxed);
    }

    pub fn take_rewind(&self) -> bool {
        self.seq_rewind.swap(false, Ordering::Relaxed)
    }

    pub fn is_seq_running(&self) -> bool {
        self.seq_run.load(Ordering::Relaxed)
    }

    pub fn set_current_step(&self, step: u64) {
        self.seq_current_step.store(step, Ordering::Relaxed);
    }

    pub fn current_step(&self) -> u64 {
        self.seq_current_step.load(Ordering::Relaxed)
    }

    pub fn set_step_phase(&self, phase: f32) {
        self.seq_step_phase.store(phase.to_bits(), Ordering::Relaxed);
    }

    pub fn step_phase(&self) -> f32 {
        f32::from_bits(self.seq_step_phase.load(Ordering::Relaxed))
    }

    /// Snapshot the *active* pattern. UI uses this once per frame to
    /// draw the step grid.
    pub fn pattern_snapshot(&self) -> Pattern {
        self.bank.lock().active().clone()
    }

    /// Snapshot the entire bank. UI uses this when it needs to know
    /// which slots are non-empty / for save-load later.
    pub fn bank_snapshot(&self) -> PatternBank {
        self.bank.lock().clone()
    }

    /// Apply a mutation to the *active* pattern. Bumps the revision
    /// counter so the audio thread knows to re-snapshot.
    pub fn edit_pattern(&self, f: impl FnOnce(&mut Pattern)) {
        {
            let mut g = self.bank.lock();
            f(g.active_mut());
        }
        self.pattern_rev.fetch_add(1, Ordering::Release);
    }

    /// Replace the entire bank (used by `initialize()` to restore from
    /// persisted state).
    pub fn replace_bank(&self, b: PatternBank) {
        {
            let mut g = self.bank.lock();
            *g = b;
            self.active_bank_pub.store(g.active, Ordering::Relaxed);
        }
        self.pattern_rev.fetch_add(1, Ordering::Release);
    }

    /// Queue a bank switch for the next pattern-loop boundary. UI side.
    pub fn queue_bank(&self, slot: u8) {
        self.pending_bank.store((slot % 4) as i8, Ordering::Relaxed);
    }

    /// Audio thread: pull the queued bank switch (returns Some on the
    /// first call after `queue_bank`). Atomically clears the slot.
    pub fn take_pending_bank(&self) -> Option<u8> {
        let v = self.pending_bank.swap(-1, Ordering::Relaxed);
        if v >= 0 { Some(v as u8) } else { None }
    }

    /// Audio-thread bank swap: locks the bank briefly to update its
    /// active index and snapshot the new active pattern into `dst`.
    /// `permit_alloc` wraps the guard drop per the parking_lot gotcha.
    /// Returns false on lock contention (caller can retry next sample).
    pub fn swap_active_bank(&self, slot: u8, dst: &mut Pattern) -> bool {
        if let Some(mut g) = self.bank.try_lock() {
            g.set_active(slot);
            *dst = g.active().clone();
            self.active_bank_pub.store(g.active, Ordering::Relaxed);
            nih_plug::util::permit_alloc(|| drop(g));
            true
        } else {
            false
        }
    }

    /// Currently-playing bank slot (lock-free read for the GUI).
    pub fn current_bank(&self) -> u8 {
        self.active_bank_pub.load(Ordering::Relaxed)
    }

    /// Currently-queued bank slot (or `None` if no swap is pending).
    pub fn queued_bank(&self) -> Option<u8> {
        let v = self.pending_bank.load(Ordering::Relaxed);
        if v >= 0 { Some(v as u8) } else { None }
    }

    /// Current pattern revision. Audio thread tracks this to avoid
    /// snapshotting every buffer when nothing has changed.
    pub fn pattern_rev(&self) -> u64 {
        self.pattern_rev.load(Ordering::Acquire)
    }

    /// RT-safe pattern snapshot for the audio thread. Attempts a
    /// `try_lock`; if contended, leaves `dst` untouched. The mutex
    /// guard drop is wrapped in `permit_alloc` per the nih-plug +
    /// parking_lot gotcha.
    pub fn selected_step(&self) -> Option<usize> {
        let v = self.selected_step.load(Ordering::Relaxed);
        if v == u32::MAX { None } else { Some(v as usize) }
    }

    pub fn set_selected_step(&self, i: usize) {
        self.selected_step.store(i as u32, Ordering::Relaxed);
    }

    pub fn clear_selected_step(&self) {
        self.selected_step.store(u32::MAX, Ordering::Relaxed);
    }

    pub fn view_oct(&self) -> u8 {
        self.view_oct.load(Ordering::Relaxed)
    }

    pub fn set_view_oct(&self, o: u8) {
        self.view_oct.store(o.min(2), Ordering::Relaxed);
    }

    pub fn nudge_view_oct(&self, delta: i8) {
        let cur = self.view_oct.load(Ordering::Relaxed) as i8;
        let next = (cur + delta).clamp(0, 2) as u8;
        self.view_oct.store(next, Ordering::Relaxed);
    }

    pub fn audio_sync_pattern(&self, dst: &mut Pattern) -> bool {
        if let Some(g) = self.bank.try_lock() {
            *dst = g.active().clone();
            nih_plug::util::permit_alloc(|| drop(g));
            true
        } else {
            false
        }
    }
}

/// Map an egui key to a semitone offset within a one-octave span across
/// the QWERTY bottom row (Renoise convention). Returns `None` for keys
/// that aren't part of the note map.
pub fn key_to_semitone(key: nih_plug_egui::egui::Key) -> Option<i32> {
    use nih_plug_egui::egui::Key;
    Some(match key {
        // Lower row: one octave from C
        Key::Z => 0,   // C
        Key::S => 1,   // C#
        Key::X => 2,   // D
        Key::D => 3,   // D#
        Key::C => 4,   // E
        Key::V => 5,   // F
        Key::G => 6,   // F#
        Key::B => 7,   // G
        Key::H => 8,   // G#
        Key::N => 9,   // A
        Key::J => 10,  // A#
        Key::M => 11,  // B
        Key::Comma => 12,
        Key::L => 13,
        Key::Period => 14,
        // Upper row: second octave from C+12
        Key::Q => 12,
        Key::Num2 => 13,
        Key::W => 14,
        Key::Num3 => 15,
        Key::E => 16,
        Key::R => 17,
        Key::Num5 => 18,
        Key::T => 19,
        Key::Num6 => 20,
        Key::Y => 21,
        Key::Num7 => 22,
        Key::U => 23,
        Key::I => 24,
        _ => return None,
    })
}
