//! 16-step grid: input pass + keyboard edits + draw pass.
//! **Critical: preserve input → keyboard → draw phase ordering.**

use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

use crate::kbd::{KbdEvent, KbdQueue};
use crate::ui::ids;
use crate::ui::keyboard::midi_note_name;
use crate::ui::palette::*;

pub fn draw_step_area(ui: &mut egui::Ui, kbd: &KbdQueue, rect: Rect) {
    let top = rect.top();
    let mut pattern = kbd.pattern_snapshot();
    let running = kbd.is_seq_running();
    let playhead = (kbd.current_step() % pattern.length.max(1) as u64) as usize;
    let selected = kbd.selected_step();

    let slider_h = SLD_Y1 - SLD_Y0;
    const SEMI_LO: f32 = 24.0;
    const SEMI_HI: f32 = 60.0;
    let semi_range = SEMI_HI - SEMI_LO;
    let mut pattern_dirty = false;

    // ─── Input pass: per-step hit-rects ───
    for i in 0..16 {
        let cx = rect.left() + STEP_X0 + (i as f32 + 0.5) * STEP_CELL;
        let note_name = midi_note_name(pattern.steps[i].semitone);
        let s_flags = pattern.steps[i];
        let flags = {
            let mut parts: Vec<&str> = Vec::new();
            if s_flags.rest { parts.push("REST"); }
            if s_flags.accent { parts.push("ACC"); }
            if s_flags.slide { parts.push("SLD"); }
            if parts.is_empty() { String::new() } else { format!(" · {}", parts.join("+")) }
        };
        let tip = format!(
            "Step {}: {}{}\n\
             • Click/drag body: select + set pitch\n\
             • Right-click body: toggle rest\n\
             • A / S / R buttons above: toggle accent/slide/rest\n\
             • Keyboard (while selected):\n  \
               Up/Down pitch ±1 (Shift ±12)\n  \
               Left/Right move selection\n  \
               A accent  S slide  R rest\n  \
               Esc deselect",
            i + 1, note_name, flags
        );

        // ── A/S/R mini-toggles above the slider ──
        let toggle_y = top + SLD_Y0 - 14.0;
        let toggle_h = 10.0;
        let toggle_w = 9.0;
        let gap = 1.0;
        let total_w = toggle_w * 3.0 + gap * 2.0;
        let row_x = cx - total_w * 0.5;
        let acc_r = Rect::from_min_size(Pos2::new(row_x, toggle_y), Vec2::new(toggle_w, toggle_h));
        let sld_r = Rect::from_min_size(Pos2::new(row_x + (toggle_w + gap), toggle_y), Vec2::new(toggle_w, toggle_h));
        let rst_r = Rect::from_min_size(Pos2::new(row_x + 2.0 * (toggle_w + gap), toggle_y), Vec2::new(toggle_w, toggle_h));

        let acc_resp = ui.interact(acc_r, ids::step_acc(i), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Accent — toggle accent on this step.");
        let sld_resp = ui.interact(sld_r, ids::step_sld(i), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Slide — glide INTO the next step from this one.");
        let rst_resp = ui.interact(rst_r, ids::step_rst(i), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Rest — silence this step.");

        if acc_resp.clicked() {
            pattern.steps[i].rest = false;
            pattern.steps[i].accent = !pattern.steps[i].accent;
            pattern_dirty = true;
            kbd.set_selected_step(i);
        }
        if sld_resp.clicked() {
            pattern.steps[i].rest = false;
            pattern.steps[i].slide = !pattern.steps[i].slide;
            pattern_dirty = true;
            kbd.set_selected_step(i);
        }
        if rst_resp.clicked() {
            pattern.steps[i].rest = !pattern.steps[i].rest;
            pattern_dirty = true;
            kbd.set_selected_step(i);
        }

        // ── Cell body: pitch drag + right-click rest ──
        let cell = Rect::from_min_max(
            Pos2::new(cx - STEP_CELL * 0.5, top + SLD_Y0),
            Pos2::new(cx + STEP_CELL * 0.5, top + SLD_Y1 + 4.0),
        );
        let resp = ui
            .interact(cell, ids::step_cell(i), egui::Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(tip);

        if resp.clicked_by(egui::PointerButton::Secondary) {
            pattern.steps[i].rest = !pattern.steps[i].rest;
            pattern_dirty = true;
            kbd.set_selected_step(i);
        } else if resp.dragged() || resp.drag_started() || resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let t = ((pos.y - (top + SLD_Y0)) / slider_h).clamp(0.0, 1.0);
                let semi = (SEMI_HI - t * semi_range).round() as i32;
                pattern.steps[i].semitone = semi.clamp(SEMI_LO as i32, SEMI_HI as i32) as u8;
                pattern.steps[i].rest = false;
                pattern_dirty = true;
                kbd.set_selected_step(i);
            }
        }
    }

    // ─── Keyboard-driven edits on the selected step ───
    if let Some(sel) = selected {
        if sel < 16 {
            let t_held: bool = ui.ctx().data(|d| d.get_temp(ids::t_held())).unwrap_or(false);
            let (dp, nav, toggle_a, toggle_s, toggle_r, esc) = ui.input(|ip| {
                let shift = ip.modifiers.shift;
                let big = if shift { 12 } else { 1 };
                let mut dp = 0i32;
                let mut nav = 0i32;
                if ip.key_pressed(egui::Key::ArrowUp) { dp += big; }
                if ip.key_pressed(egui::Key::ArrowDown) { dp -= big; }
                if ip.key_pressed(egui::Key::ArrowLeft) { nav -= 1; }
                if ip.key_pressed(egui::Key::ArrowRight) { nav += 1; }
                (
                    dp, nav,
                    ip.key_pressed(egui::Key::A),
                    ip.key_pressed(egui::Key::S),
                    ip.key_pressed(egui::Key::R) || ip.key_pressed(egui::Key::Delete) || ip.key_pressed(egui::Key::Backspace),
                    ip.key_pressed(egui::Key::Escape),
                )
            });
            if dp != 0 {
                let cur = if pattern.steps[sel].rest { 36 } else { pattern.steps[sel].semitone as i32 };
                let next = (cur + dp).clamp(SEMI_LO as i32, SEMI_HI as i32);
                pattern.steps[sel].semitone = next as u8;
                pattern.steps[sel].rest = false;
                pattern_dirty = true;
                if t_held {
                    let velocity = if pattern.steps[sel].accent { 0.95 } else { 0.7 };
                    kbd.push(KbdEvent { on: true, note: next as u8, velocity });
                }
            }
            if nav != 0 {
                let len = pattern.length.max(1) as i32;
                let next = ((sel as i32 + nav).rem_euclid(len)) as usize;
                kbd.set_selected_step(next);
            }
            if toggle_a {
                pattern.steps[sel].rest = false;
                pattern.steps[sel].accent = !pattern.steps[sel].accent;
                pattern_dirty = true;
            }
            if toggle_s {
                pattern.steps[sel].rest = false;
                pattern.steps[sel].slide = !pattern.steps[sel].slide;
                pattern_dirty = true;
            }
            if toggle_r {
                pattern.steps[sel].rest = !pattern.steps[sel].rest;
                pattern_dirty = true;
            }
            if esc {
                kbd.clear_selected_step();
            }
        } else {
            kbd.clear_selected_step();
        }
    }

    if pattern_dirty {
        let pat_clone = pattern.clone();
        kbd.edit_pattern(move |p| *p = pat_clone);
    }

    // ─── Draw pass ───
    let p = ui.painter();

    let plate = Rect::from_min_max(
        Pos2::new(rect.left() + STEP_X0, top + SLD_Y0 - 4.0),
        Pos2::new(rect.left() + STEP_X1, top + SLD_Y1 + 4.0),
    );
    p.rect_filled(plate.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
    p.rect_filled(plate, 2.0, Color32::from_rgb(12, 12, 14));
    p.rect_stroke(plate, 2.0, Stroke::new(0.8, SILVER_SHADOW), egui::StrokeKind::Inside);

    let pat_len = pattern.length.max(1) as usize;
    for i in 0..16 {
        let cx = rect.left() + STEP_X0 + (i as f32 + 0.5) * STEP_CELL;
        let step = pattern.steps[i];
        let inactive = i >= pat_len;

        // ── A / S / R toggle row ──
        let toggle_y = top + SLD_Y0 - 14.0;
        let toggle_h = 10.0;
        let toggle_w = 9.0;
        let gap = 1.0;
        let total_w = toggle_w * 3.0 + gap * 2.0;
        let row_x = cx - total_w * 0.5;
        let acc_r = Rect::from_min_size(Pos2::new(row_x, toggle_y), Vec2::new(toggle_w, toggle_h));
        let sld_r = Rect::from_min_size(Pos2::new(row_x + (toggle_w + gap), toggle_y), Vec2::new(toggle_w, toggle_h));
        let rst_r = Rect::from_min_size(Pos2::new(row_x + 2.0 * (toggle_w + gap), toggle_y), Vec2::new(toggle_w, toggle_h));
        let lbl_font = egui::FontId::new(7.0, egui::FontFamily::Monospace);

        let acc_active = !step.rest && step.accent;
        p.rect_filled(acc_r, 1.0, if acc_active { RED } else { BTN_FACE });
        p.rect_stroke(acc_r, 1.0, Stroke::new(0.5, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(acc_r.center(), egui::Align2::CENTER_CENTER, "A", lbl_font.clone(),
            if acc_active { Color32::WHITE } else { BTN_LBL });

        let sld_active = !step.rest && step.slide;
        p.rect_filled(sld_r, 1.0, if sld_active { YELLOW } else { BTN_FACE });
        p.rect_stroke(sld_r, 1.0, Stroke::new(0.5, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(sld_r.center(), egui::Align2::CENTER_CENTER, "S", lbl_font.clone(),
            if sld_active { INK } else { BTN_LBL });

        let rst_active = step.rest;
        p.rect_filled(rst_r, 1.0, if rst_active { SILVER_LIGHT } else { BTN_FACE });
        p.rect_stroke(rst_r, 1.0, Stroke::new(0.5, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(rst_r.center(), egui::Align2::CENTER_CENTER, "R", lbl_font,
            if rst_active { INK } else { BTN_LBL });

        // Slider track
        let track_x = cx - 2.0;
        let track = Rect::from_min_size(Pos2::new(track_x, top + SLD_Y0), Vec2::new(4.0, slider_h));
        p.rect_filled(track, 1.5, INSET);
        p.rect_stroke(track, 1.5, Stroke::new(0.5, INK), egui::StrokeKind::Inside);

        if !step.rest {
            let semi = (step.semitone as f32).clamp(SEMI_LO, SEMI_HI);
            let t = 1.0 - (semi - SEMI_LO) / semi_range;
            let thumb_y = top + SLD_Y0 + t * slider_h;
            let thumb = Rect::from_center_size(Pos2::new(cx, thumb_y), Vec2::new(STEP_CELL - 6.0, 7.0));
            p.rect_filled(thumb.translate(Vec2::new(0.0, 1.0)), 2.0, SILVER_SHADOW);
            p.rect_filled(thumb, 2.0, if step.accent { RED_DARK } else { SILVER_MID });
            p.rect_stroke(thumb, 2.0, Stroke::new(0.8, SILVER_LIGHT), egui::StrokeKind::Outside);
        }

        if i % 4 == 0 && i > 0 {
            p.line_segment(
                [Pos2::new(cx - STEP_CELL * 0.5, top + SLD_Y0 - 2.0),
                 Pos2::new(cx - STEP_CELL * 0.5, top + SLD_Y1 + 2.0)],
                Stroke::new(0.8, Color32::from_rgba_unmultiplied(255, 255, 255, 20)),
            );
        }

        if running && i == playhead {
            let ph = Rect::from_min_size(
                Pos2::new(cx - STEP_CELL * 0.5 + 1.0, top + SLD_Y0 - 12.0),
                Vec2::new(STEP_CELL - 2.0, slider_h + 16.0),
            );
            p.rect_stroke(ph, 2.0, Stroke::new(1.8, Color32::from_rgb(240, 240, 255)), egui::StrokeKind::Outside);
            p.line_segment(
                [Pos2::new(ph.left(), ph.top() - 1.0), Pos2::new(ph.right(), ph.top() - 1.0)],
                Stroke::new(1.2, Color32::WHITE),
            );
        } else if !running {
            let stopped_pos = selected.unwrap_or(playhead);
            if i == stopped_pos {
                let sr = Rect::from_min_size(
                    Pos2::new(cx - STEP_CELL * 0.5 + 1.0, top + SLD_Y0 - 12.0),
                    Vec2::new(STEP_CELL - 2.0, slider_h + 16.0),
                );
                p.rect_stroke(sr, 2.0, Stroke::new(1.5, Color32::from_rgb(180, 60, 60)), egui::StrokeKind::Outside);
            }
        }

        // Step number
        p.text(
            Pos2::new(cx, top + STEP_NUM_Y),
            egui::Align2::CENTER_CENTER,
            format!("{}", i + 1),
            egui::FontId::new(7.5, egui::FontFamily::Monospace),
            if inactive { Color32::from_rgb(60, 60, 64) }
            else if i % 4 == 0 { LABEL_FG } else { BTN_LBL },
        );

        if inactive {
            let overlay = Rect::from_min_max(
                Pos2::new(cx - STEP_CELL * 0.5, top + SLD_Y0 - 16.0),
                Pos2::new(cx + STEP_CELL * 0.5, top + SLD_Y1 + 6.0),
            );
            p.rect_filled(overlay, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 170));
        }
    }

    // Selector label
    p.text(
        Pos2::new(rect.left() + STEP_X0, top + STEP_NUM_Y),
        egui::Align2::RIGHT_CENTER,
        "SEL",
        egui::FontId::new(7.0, egui::FontFamily::Monospace),
        BTN_LBL,
    );
}
