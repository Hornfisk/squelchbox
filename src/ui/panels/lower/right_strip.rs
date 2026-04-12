//! Right column: BACK / STEP / WRITE-NEXT / TAP / DUMP MIDI.

use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Pos2, Rect, Stroke, Vec2};
use std::path::PathBuf;

use crate::kbd::KbdQueue;
use crate::params::SquelchBoxParams;
use crate::ui::ids;
use crate::ui::keyboard::{handle_tap_tempo, single_step_audition};
use crate::ui::palette::*;
use crate::ui::panels::toast::set_toast;
use crate::ui::widgets::{lerp_color, paint_btn};

pub fn draw_right_strip(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    params: &SquelchBoxParams,
    kbd: &KbdQueue,
    rect: Rect,
) {
    let top = rect.top();
    let x0 = rect.left() + RSTRIP_X;
    let panel_top = top + PANEL_SPLIT;
    let strip = Rect::from_min_size(
        Pos2::new(x0, panel_top),
        Vec2::new(rect.right() - x0, crate::ui::BASE_H as f32 - PANEL_SPLIT),
    );
    {
        let p = ui.painter();
        p.rect_filled(strip, 0.0, lerp_color(BLACK_PANEL, BTN_FACE, 0.06));
        p.line_segment(
            [Pos2::new(x0, panel_top), Pos2::new(x0, rect.bottom())],
            Stroke::new(1.0, lerp_color(SILVER_SHADOW, INK, 0.4)),
        );
    }

    let btn_w = rect.right() - x0 - 10.0;

    // ── BACK ──
    let back_r = Rect::from_min_size(Pos2::new(x0 + 5.0, panel_top + 5.0), Vec2::new(btn_w, 18.0));
    let back_resp = ui.interact(back_r, ids::back(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Back — move the editing selection backward by one step.\nWhile stopped, also auditions the step.");
    paint_btn(ui.painter(), back_r, "◀ BACK", false, 8.5, 0.8);
    if back_resp.clicked() && !kbd.is_seq_running() {
        let pat = kbd.pattern_snapshot();
        let len = pat.length.max(1) as usize;
        let cur = kbd.selected_step()
            .unwrap_or_else(|| (kbd.current_step() % len as u64) as usize);
        let prev = if cur == 0 { len - 1 } else { cur - 1 };
        kbd.set_selected_step(prev);
        kbd.set_current_step(prev as u64);
        let s = pat.steps[prev];
        if !s.rest {
            let velocity = if s.accent { 0.95 } else { 0.7 };
            kbd.push(crate::kbd::KbdEvent { on: true, note: s.semitone, velocity });
        }
    }

    // ── STEP ──
    let step_r = Rect::from_min_size(Pos2::new(x0 + 5.0, panel_top + 30.0), Vec2::new(btn_w, 20.0));
    let step_resp = ui.interact(step_r, ids::step(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Step — advance the playhead one step and audition the note.\nWorks while the sequencer is stopped.");
    paint_btn(ui.painter(), step_r, "STEP", false, 8.5, 0.8);
    if step_resp.clicked() && !kbd.is_seq_running() {
        single_step_audition(kbd);
    }

    // ── WRITE / NEXT ──
    let wn_r = Rect::from_min_size(Pos2::new(x0 + 5.0, panel_top + 58.0), Vec2::new(btn_w, 20.0));
    let wn_resp = ui.interact(wn_r, ids::writenext(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Write / Next — move the editing selection forward by one step.");
    paint_btn(ui.painter(), wn_r, "WRITE/NEXT", false, 8.5, 0.8);
    if wn_resp.clicked() && !kbd.is_seq_running() {
        let pat = kbd.pattern_snapshot();
        let len = pat.length.max(1) as usize;
        let cur = kbd.selected_step()
            .unwrap_or_else(|| (kbd.current_step() % len as u64) as usize);
        let next = (cur + 1) % len;
        kbd.set_selected_step(next);
        kbd.set_current_step(next as u64);
        let s = pat.steps[next];
        if !s.rest {
            let velocity = if s.accent { 0.95 } else { 0.7 };
            kbd.push(crate::kbd::KbdEvent { on: true, note: s.semitone, velocity });
        }
    }

    // ── TAP ──
    let tap_r = Rect::from_min_size(Pos2::new(x0 + 5.0, panel_top + 86.0), Vec2::new(btn_w, 20.0));
    let tap_resp = ui.interact(tap_r, ids::tap(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Tap tempo — tap 2+ times in rhythm to set BPM.\nTaps separated by more than 2s reset the streak.");
    paint_btn(ui.painter(), tap_r, "TAP", false, 8.5, 0.8);
    if tap_resp.clicked() {
        handle_tap_tempo(ui.ctx(), setter, &params.seq_bpm);
    }

    // ── DUMP MIDI ──
    let dump_r = Rect::from_min_size(Pos2::new(x0 + 5.0, panel_top + 114.0), Vec2::new(btn_w, 20.0));
    let dump_resp = ui.interact(dump_r, ids::dump_midi(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(
            "Dump MIDI — export the current pattern as a .mid file.\n\
             Saved to ~/.local/share/squelchbox/exports/.\n\
             A companion Renoise tool can pick it up from there."
        );
    paint_btn(ui.painter(), dump_r, "DUMP MIDI", false, 8.5, 0.8);
    if dump_resp.clicked() {
        let snap = kbd.pattern_snapshot();
        let bpm = params.seq_bpm.unmodulated_plain_value();
        match crate::util::midi_export::export_pattern(&snap, bpm) {
            Ok(path) => {
                tracing::info!("MIDI export: {}", path.display());
                let msg = format!("Exported: {}", path.display());
                set_toast(ui.ctx(), msg, path);
            }
            Err(e) => {
                tracing::warn!("MIDI export failed: {e}");
                set_toast(ui.ctx(), format!("Export failed: {e}"), PathBuf::new());
            }
        }
    }
}
