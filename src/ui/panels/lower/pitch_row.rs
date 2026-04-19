//! Decorative chromatic pitch-key display row + sequencer view-octave controls.

use nih_plug_egui::egui::{self, Pos2, Rect, Stroke, Vec2};

use crate::kbd::KbdQueue;
use crate::ui::ids;
use crate::ui::palette::*;

pub fn draw_pitch_buttons(ui: &mut egui::Ui, kbd: &KbdQueue, rect: Rect) {
    const NOTES: [&str; 13] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B", "C"];
    const SHARP: [bool; 13] = [false, true, false, true, false, false, true, false, true, false, true, false, false];
    let top = rect.top();

    let view_oct = kbd.view_oct();
    let lo_octave = view_oct + 1; // semi 24 = C1, semi 36 = C2, semi 48 = C3
    let hi_octave = lo_octave + 1;

    // ── OCT controls + label (top-left of pitch row) ──
    let oct_label = format!("OCT {} · C{}–C{}", view_oct + 2, lo_octave, hi_octave);
    let lbl_x = rect.left() + STEP_X0 + 1.0;
    let lbl_y = top + PITCH_Y;
    ui.painter().text(
        Pos2::new(lbl_x, lbl_y),
        egui::Align2::LEFT_TOP,
        &oct_label,
        egui::FontId::new(7.5, egui::FontFamily::Monospace),
        INSET_TEXT,
    );

    // OCT▼ / OCT▲ buttons
    let btn_w = 14.0;
    let btn_h = 9.0;
    let btn_y = top + PITCH_Y - 1.0;
    let btn_dn_x = lbl_x + 92.0;
    let btn_up_x = btn_dn_x + btn_w + 2.0;
    let dn_r = Rect::from_min_size(Pos2::new(btn_dn_x, btn_y), Vec2::new(btn_w, btn_h));
    let up_r = Rect::from_min_size(Pos2::new(btn_up_x, btn_y), Vec2::new(btn_w, btn_h));

    let dn_resp = ui.interact(dn_r, ids::view_oct_dn(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Shift the visible pitch window down one octave.\nNotes outside the window show as ▲/▼ markers.");
    let up_resp = ui.interact(up_r, ids::view_oct_up(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Shift the visible pitch window up one octave.");

    let p = ui.painter();
    let dn_dim = view_oct == 0;
    let up_dim = view_oct == 2;
    let dn_face = if dn_dim { INK } else { BTN_FACE };
    let up_face = if up_dim { INK } else { BTN_FACE };
    p.rect_filled(dn_r, 1.5, dn_face);
    p.rect_stroke(dn_r, 1.5, Stroke::new(0.5, SILVER_SHADOW), egui::StrokeKind::Inside);
    p.rect_filled(up_r, 1.5, up_face);
    p.rect_stroke(up_r, 1.5, Stroke::new(0.5, SILVER_SHADOW), egui::StrokeKind::Inside);
    let f = egui::FontId::new(7.5, egui::FontFamily::Monospace);
    p.text(dn_r.center(), egui::Align2::CENTER_CENTER, "▼",
        f.clone(), if dn_dim { SILVER_SHADOW } else { BTN_LBL });
    p.text(up_r.center(), egui::Align2::CENTER_CENTER, "▲",
        f, if up_dim { SILVER_SHADOW } else { BTN_LBL });

    if dn_resp.clicked() && !dn_dim {
        kbd.nudge_view_oct(-1);
    }
    if up_resp.clicked() && !up_dim {
        kbd.nudge_view_oct(1);
    }

    // ── Decorative chromatic key strip (right of OCT controls) ──
    let strip_left = btn_up_x + btn_w + 8.0;
    let btn_area_w = (rect.left() + STEP_X1) - strip_left;
    let btn_w_chrom = btn_area_w / 13.0;
    let btn_top_chrom = top + PITCH_Y + 10.0;
    let btn_h_chrom = PITCH_H - 10.0;

    for (i, (&note, &is_sharp)) in NOTES.iter().zip(SHARP.iter()).enumerate() {
        let bx = strip_left + i as f32 * btn_w_chrom;
        let (bg, fg) = if is_sharp { (INK, BTN_LBL) } else { (SILVER_MID, INK) };
        let raised = if is_sharp { 0.0 } else { 2.0 };
        let br = Rect::from_min_size(
            Pos2::new(bx + 1.0, btn_top_chrom + raised),
            Vec2::new(btn_w_chrom - 2.0, btn_h_chrom - raised),
        );
        p.rect_filled(br, 1.5, bg);
        p.rect_stroke(br, 1.5, Stroke::new(0.7, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(
            br.center(),
            egui::Align2::CENTER_CENTER,
            note,
            egui::FontId::new(6.0, egui::FontFamily::Monospace),
            fg,
        );
    }
}
