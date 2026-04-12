//! Silver + black faceplate painting: brushed-metal gradient, screws, bezel.

use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

use crate::ui::palette::*;
use crate::ui::widgets::lerp_color;

pub fn draw_faceplate(ui: &egui::Ui, rect: Rect) {
    let p = ui.painter();
    // Silver panel: brushed-metal gradient
    let silver = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + PANEL_SPLIT));
    let bands = 60;
    let h = silver.height() / bands as f32;
    for i in 0..bands {
        let t = i as f32 / (bands - 1) as f32;
        let shade = lerp_color(SILVER_LIGHT, SILVER_DARK, t * 0.65);
        let r = Rect::from_min_size(
            Pos2::new(silver.left(), silver.top() + i as f32 * h),
            Vec2::new(silver.width(), h + 0.5),
        );
        p.rect_filled(r, 0.0, shade);
    }
    // Horizontal brushed-metal shimmer lines
    for i in 0..((silver.height() / 2.0) as usize) {
        let y = silver.top() + i as f32 * 2.0 + ((i * 37) % 3) as f32 * 0.3;
        let alpha = 18 + ((i * 13) % 14) as u8;
        p.line_segment(
            [Pos2::new(silver.left(), y), Pos2::new(silver.right(), y)],
            Stroke::new(0.5, Color32::from_rgba_unmultiplied(255, 255, 255, alpha)),
        );
    }
    // Black sequencer panel
    let black = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + PANEL_SPLIT),
        rect.max,
    );
    p.rect_filled(black, 0.0, BLACK_PANEL);
    // Raised ledge between panels
    p.line_segment(
        [
            Pos2::new(rect.left(), rect.top() + PANEL_SPLIT - 1.0),
            Pos2::new(rect.right(), rect.top() + PANEL_SPLIT - 1.0),
        ],
        Stroke::new(1.5, SILVER_SHADOW),
    );
    p.line_segment(
        [
            Pos2::new(rect.left(), rect.top() + PANEL_SPLIT),
            Pos2::new(rect.right(), rect.top() + PANEL_SPLIT),
        ],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 180)),
    );
    // Outer bezel
    p.rect_stroke(rect, 4.0, Stroke::new(2.0, SILVER_SHADOW), egui::StrokeKind::Inside);
    // Corner screws
    for (dx, dy) in [
        (12.0f32, 12.0f32),
        (-12.0, 12.0),
        (12.0, -12.0),
        (-12.0, -12.0),
    ] {
        let cx = if dx > 0.0 { rect.left() + dx } else { rect.right() + dx };
        let cy = if dy > 0.0 { rect.top() + dy } else { rect.bottom() + dy };
        draw_screw(p, Pos2::new(cx, cy));
    }
}

fn draw_screw(p: &egui::Painter, c: Pos2) {
    p.circle_filled(c + Vec2::new(0.5, 0.8), 4.2, SILVER_SHADOW);
    p.circle_filled(c, 4.0, SILVER_MID);
    p.circle_filled(c - Vec2::new(0.8, 0.8), 2.4, SILVER_LIGHT);
    p.line_segment(
        [c + Vec2::new(-2.8, -2.8), c + Vec2::new(2.8, 2.8)],
        Stroke::new(1.0, INK),
    );
}

pub fn draw_connector_strip(ui: &egui::Ui, rect: Rect) {
    let p = ui.painter();
    let strip = Rect::from_min_size(rect.min, Vec2::new(rect.width(), BAND1_TOP));
    p.rect_filled(strip, 0.0, lerp_color(SILVER_DARK, INK, 0.3));
    p.line_segment(
        [
            Pos2::new(rect.left(), rect.top() + BAND1_TOP - 0.5),
            Pos2::new(rect.right(), rect.top() + BAND1_TOP - 0.5),
        ],
        Stroke::new(1.0, SILVER_SHADOW),
    );
    let cy = rect.top() + BAND1_TOP * 0.5;
    let font = egui::FontId::new(7.0, egui::FontFamily::Monospace);
    for (lbl, x) in [("MIX IN", 28.0f32), ("MIDI IN", 60.0), ("SYNC IN", 96.0)] {
        p.text(
            Pos2::new(rect.left() + x, cy),
            egui::Align2::LEFT_CENTER,
            lbl,
            font.clone(),
            BTN_LBL,
        );
        p.circle_filled(Pos2::new(rect.left() + x - 6.0, cy), 2.5, INK);
        p.circle_stroke(
            Pos2::new(rect.left() + x - 6.0, cy),
            2.5,
            Stroke::new(0.6, SILVER_SHADOW),
        );
    }
    for (lbl, xr) in [
        ("DC 9V", 28.0f32),
        ("OUTPUT", 64.0),
        ("HEADPHONE", 108.0),
        ("GATE", 154.0),
        ("CV", 180.0),
    ] {
        let x = rect.right() - xr;
        p.text(
            Pos2::new(x, cy),
            egui::Align2::RIGHT_CENTER,
            lbl,
            font.clone(),
            BTN_LBL,
        );
        p.circle_filled(Pos2::new(x + 6.0, cy), 2.5, INK);
        p.circle_stroke(
            Pos2::new(x + 6.0, cy),
            2.5,
            Stroke::new(0.6, SILVER_SHADOW),
        );
    }
}
