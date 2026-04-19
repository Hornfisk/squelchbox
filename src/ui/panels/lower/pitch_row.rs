//! Decorative chromatic pitch-key display row.

use nih_plug_egui::egui::{self, Pos2, Rect, Stroke, Vec2};

use crate::kbd::KbdQueue;
use crate::ui::palette::*;

pub fn draw_pitch_buttons(ui: &mut egui::Ui, _kbd: &KbdQueue, rect: Rect) {
    const NOTES: [&str; 13] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B", "C"];
    const SHARP: [bool; 13] = [false, true, false, true, false, false, true, false, true, false, true, false, false];
    let top = rect.top();
    let p = ui.painter();

    let strip_left = rect.left() + STEP_X0 + 1.0;
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
