//! Transpose section + TIME MODE: DN/UP, DEL/INS, ACCENT/SLIDE toggles, beat LEDs.

use nih_plug_egui::egui::{self, Pos2, Rect, Stroke, Vec2};

use crate::kbd::KbdQueue;
use crate::ui::ids;
use crate::ui::palette::*;
use crate::ui::widgets::paint_btn;

pub fn draw_transpose_section(ui: &mut egui::Ui, kbd: &KbdQueue, rect: Rect) {
    let top = rect.top();
    let x0 = rect.left() + TR_X;
    let font = egui::FontId::new(7.0, egui::FontFamily::Monospace);
    let panel_top = top + PANEL_SPLIT;

    let shift_held = ui.input(|i| i.modifiers.shift);

    // ── TRANSPOSE label + DN/UP ──
    ui.painter().text(Pos2::new(x0 + 32.0, panel_top + 6.0), egui::Align2::CENTER_TOP,
        "TRANSPOSE", font.clone(), BTN_LBL);
    let dn_r = Rect::from_min_size(Pos2::new(x0, panel_top + 17.0), Vec2::new(30.0, 13.0));
    let up_r = Rect::from_min_size(Pos2::new(x0 + 34.0, panel_top + 17.0), Vec2::new(30.0, 13.0));
    let dn_resp = ui.interact(dn_r, ids::tr_dn(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Transpose down — every non-rest step −1 semitone.\nShift+click: −1 octave.");
    let up_resp = ui.interact(up_r, ids::tr_up(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Transpose up — every non-rest step +1 semitone.\nShift+click: +1 octave.");
    paint_btn(ui.painter(), dn_r, "▼ DN", false, 7.5, 0.7);
    paint_btn(ui.painter(), up_r, "▲ UP", false, 7.5, 0.7);
    if dn_resp.clicked() {
        let d = if shift_held { -12 } else { -1 };
        kbd.edit_pattern(|p| transpose_pattern(p, d));
    }
    if up_resp.clicked() {
        let d = if shift_held { 12 } else { 1 };
        kbd.edit_pattern(|p| transpose_pattern(p, d));
    }

    // ── DEL / INS ──
    let del_r = Rect::from_min_size(Pos2::new(x0, panel_top + 35.0), Vec2::new(30.0, 13.0));
    let ins_r = Rect::from_min_size(Pos2::new(x0 + 34.0, panel_top + 35.0), Vec2::new(30.0, 13.0));
    let del_resp = ui.interact(del_r, ids::del(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Delete — turn the selected step into a rest.\n(If no step is selected, acts on step 1.)");
    let ins_resp = ui.interact(ins_r, ids::ins(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Insert — un-rest the selected step (audible note).\n(If no step is selected, acts on step 1.)");
    paint_btn(ui.painter(), del_r, "DEL", false, 7.5, 0.7);
    paint_btn(ui.painter(), ins_r, "INS", false, 7.5, 0.7);
    let sel = kbd.selected_step().unwrap_or(0);
    if del_resp.clicked() {
        kbd.edit_pattern(|p| { p.steps[sel].rest = true; });
        kbd.set_selected_step(sel);
    }
    if ins_resp.clicked() {
        kbd.edit_pattern(|p| {
            p.steps[sel].rest = false;
            if p.steps[sel].semitone > 60 || p.steps[sel].semitone < 24 {
                p.steps[sel].semitone = 36;
            }
        });
        kbd.set_selected_step(sel);
    }

    // ── TIME MODE label + ACCENT / SLIDE toggles ──
    ui.painter().text(Pos2::new(x0 + 32.0, panel_top + 53.0), egui::Align2::CENTER_TOP,
        "TIME MODE", font.clone(), BTN_LBL);
    let snapshot = kbd.pattern_snapshot();
    let acc_active = snapshot.steps[sel].accent && !snapshot.steps[sel].rest;
    let sld_active = snapshot.steps[sel].slide && !snapshot.steps[sel].rest;
    let acc_r = Rect::from_min_size(Pos2::new(x0, panel_top + 64.0), Vec2::new(64.0, 14.0));
    let sld_r = Rect::from_min_size(Pos2::new(x0, panel_top + 82.0), Vec2::new(64.0, 14.0));
    let acc_resp = ui.interact(acc_r, ids::tm_acc(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Accent — toggle the accent flag on the selected step.\nShortcut (with step selected): A");
    let sld_resp = ui.interact(sld_r, ids::tm_sld(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Slide — toggle the slide flag on the selected step.\nShortcut (with step selected): S");
    paint_btn(ui.painter(), acc_r, "ACCENT", acc_active, 7.5, 0.7);
    paint_btn(ui.painter(), sld_r, "SLIDE", sld_active, 7.5, 0.7);
    if acc_resp.clicked() {
        kbd.edit_pattern(|p| {
            p.steps[sel].rest = false;
            p.steps[sel].accent = !p.steps[sel].accent;
        });
        kbd.set_selected_step(sel);
    }
    if sld_resp.clicked() {
        kbd.edit_pattern(|p| {
            p.steps[sel].rest = false;
            p.steps[sel].slide = !p.steps[sel].slide;
        });
        kbd.set_selected_step(sel);
    }

    // ── Beat indicator dots ──
    const LED_LOOKAHEAD: f32 = 0.30;
    let raw_pos = kbd.current_step() as f32 + kbd.step_phase() + LED_LOOKAHEAD;
    let beat = ((raw_pos as u64 / 4) % 4) as usize;
    let p = ui.painter();
    let row_left = x0 + 8.0;
    let row_right = x0 + 56.0;
    let span = row_right - row_left;
    let gap = span / 3.0;
    for j in 0..4 {
        let dot_x = row_left + j as f32 * gap;
        let lit = j == beat && kbd.is_seq_running();
        p.circle_filled(Pos2::new(dot_x, panel_top + 103.0), 3.5, if lit { RED } else { BTN_FACE });
        p.circle_stroke(Pos2::new(dot_x, panel_top + 103.0), 3.5, Stroke::new(0.6, SILVER_SHADOW));
    }
}

fn transpose_pattern(p: &mut crate::sequencer::Pattern, delta: i32) {
    for s in p.steps.iter_mut() {
        if s.rest { continue; }
        let next = (s.semitone as i32 + delta).clamp(24, 60);
        s.semitone = next as u8;
    }
}
