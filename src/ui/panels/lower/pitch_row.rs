//! Decorative chromatic pitch-key display row.

use nih_plug_egui::egui::{self, Pos2, Rect, Stroke, Vec2};

use crate::ui::palette::*;

pub fn draw_pitch_buttons(ui: &egui::Ui, rect: Rect) {
    const NOTES: [&str; 13] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B", "C"];
    const SHARP: [bool; 13] = [false, true, false, true, false, false, true, false, true, false, true, false, false];
    let p = ui.painter();
    let top = rect.top();

    p.text(
        Pos2::new(rect.left() + STEP_X0 + 1.0, top + PITCH_Y),
        egui::Align2::LEFT_TOP,
        "PITCH MODE",
        egui::FontId::new(7.0, egui::FontFamily::Monospace),
        BTN_LBL,
    );

    let btn_area_w = STEP_X1 - STEP_X0;
    let btn_w = btn_area_w / 13.0;
    let btn_top = top + PITCH_Y + 10.0;
    let btn_h = PITCH_H - 10.0;

    for (i, (&note, &is_sharp)) in NOTES.iter().zip(SHARP.iter()).enumerate() {
        let bx = rect.left() + STEP_X0 + i as f32 * btn_w;
        let (bg, fg) = if is_sharp { (INK, BTN_LBL) } else { (SILVER_MID, INK) };
        let raised = if is_sharp { 0.0 } else { 2.0 };
        let br = Rect::from_min_size(
            Pos2::new(bx + 1.0, btn_top + raised),
            Vec2::new(btn_w - 2.0, btn_h - raised),
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
