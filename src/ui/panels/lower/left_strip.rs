//! Left strip: RAND, CLEAR, SHIFT L/R, waveform toggle, RUN/STOP.

use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

use crate::kbd::KbdQueue;
use crate::params::{SquelchBoxParams, WaveformParam};
use crate::ui::ids;
use crate::ui::keyboard::randomize_pattern;
use crate::ui::palette::*;
use crate::ui::widgets::{draw_wave_button, lerp_color};

pub fn draw_left_strip(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    params: &SquelchBoxParams,
    kbd: &KbdQueue,
    rect: Rect,
) {
    let p = ui.painter();
    let top = rect.top();
    let lx = rect.left() + 6.0;
    let panel_top = top + PANEL_SPLIT;
    let strip = Rect::from_min_size(
        Pos2::new(rect.left(), panel_top),
        Vec2::new(LSTRIP_W, crate::ui::BASE_H as f32 - PANEL_SPLIT),
    );
    p.rect_filled(strip, 0.0, lerp_color(BLACK_PANEL, BTN_FACE, 0.08));
    p.line_segment(
        [Pos2::new(rect.left() + LSTRIP_W, panel_top), Pos2::new(rect.left() + LSTRIP_W, rect.bottom())],
        Stroke::new(1.0, lerp_color(SILVER_SHADOW, INK, 0.4)),
    );

    let font_sm = egui::FontId::new(7.0, egui::FontFamily::Monospace);

    // ── RAND ──
    let rand_r = Rect::from_min_size(Pos2::new(lx, panel_top + 5.0), Vec2::new(68.0, 13.0));
    let rand_resp = ui
        .interact(rand_r, ids::rand(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Randomize — generate a fresh acid pattern.\nMinor pentatonic, dense, with accents and slides.\nShortcut: ` (backtick)");
    let p = ui.painter();
    p.rect_filled(rand_r.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
    p.rect_filled(rand_r, 2.0, BTN_FACE);
    p.rect_stroke(rand_r, 2.0, Stroke::new(0.7, SILVER_SHADOW), egui::StrokeKind::Inside);
    p.text(rand_r.center(), egui::Align2::CENTER_CENTER, "↺ RANDOMIZE",
        egui::FontId::new(7.0, egui::FontFamily::Monospace), LABEL_FG);
    if rand_resp.clicked() {
        randomize_pattern(ui.ctx(), kbd);
    }

    // ── CLEAR ──
    let clr_r = Rect::from_min_size(Pos2::new(lx, panel_top + 22.0), Vec2::new(68.0, 13.0));
    let clr_resp = ui
        .interact(clr_r, ids::clear(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Clear — turn every step into a rest.");
    let p = ui.painter();
    p.rect_filled(clr_r.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
    p.rect_filled(clr_r, 2.0, BTN_FACE);
    p.rect_stroke(clr_r, 2.0, Stroke::new(0.7, SILVER_SHADOW), egui::StrokeKind::Inside);
    p.text(clr_r.center(), egui::Align2::CENTER_CENTER, "■ PATTERN CLEAR",
        egui::FontId::new(6.5, egui::FontFamily::Monospace), BTN_LBL);
    if clr_resp.clicked() {
        kbd.edit_pattern(|p| {
            for s in p.steps.iter_mut() { s.rest = true; }
        });
    }

    // ── SHIFT L / SHIFT R ──
    p.text(Pos2::new(lx, panel_top + 42.0), egui::Align2::LEFT_TOP,
        "SHIFT PATTERN", font_sm, BTN_LBL);
    let shl_r = Rect::from_min_size(Pos2::new(lx, panel_top + 52.0), Vec2::new(32.0, 13.0));
    let shr_r = Rect::from_min_size(Pos2::new(lx + 36.0, panel_top + 52.0), Vec2::new(32.0, 13.0));
    let shl_resp = ui.interact(shl_r, ids::shl(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Shift left — rotate the pattern one step earlier.\nShortcut: [");
    let shr_resp = ui.interact(shr_r, ids::shr(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Shift right — rotate the pattern one step later.\nShortcut: ]");
    let p = ui.painter();
    for (r, lbl) in [(shl_r, "◀ L"), (shr_r, "R ▶")] {
        p.rect_filled(r.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
        p.rect_filled(r, 2.0, BTN_FACE);
        p.rect_stroke(r, 2.0, Stroke::new(0.6, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(r.center(), egui::Align2::CENTER_CENTER, lbl,
            egui::FontId::new(7.5, egui::FontFamily::Monospace), LABEL_FG);
    }
    if shl_resp.clicked() {
        kbd.edit_pattern(|p| p.rotate_left(1));
    }
    if shr_resp.clicked() {
        kbd.edit_pattern(|p| p.rotate_right(1));
    }

    // ── Waveform toggle ──
    let wf = params.waveform.value();
    let saw_r = Rect::from_min_size(Pos2::new(lx, panel_top + 80.0), Vec2::new(32.0, 16.0));
    let sqr_r = Rect::from_min_size(Pos2::new(lx + 35.0, panel_top + 80.0), Vec2::new(33.0, 16.0));
    p.text(Pos2::new(lx + 34.0, panel_top + 77.0), egui::Align2::CENTER_BOTTOM,
        "WAVEFORM", egui::FontId::new(6.5, egui::FontFamily::Monospace), BTN_LBL);
    draw_wave_button(ui, setter, &params.waveform, saw_r, ids::saw(), "SAW",
        wf == WaveformParam::Saw, WaveformParam::Saw);
    draw_wave_button(ui, setter, &params.waveform, sqr_r, ids::sqr(), "SQR",
        wf == WaveformParam::Square, WaveformParam::Square);

    // ── RUN / STOP ──
    let run_rect = Rect::from_min_size(Pos2::new(lx, panel_top + 102.0), Vec2::new(68.0, 22.0));
    let running = kbd.is_seq_running();
    let run_resp = ui.interact(run_rect, ids::runstop(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Run / Stop — start or stop the sequencer.\nShortcut: Space or P");
    let p = ui.painter();
    p.rect_filled(run_rect.translate(Vec2::new(0.0, 1.5)), 3.0, INK);
    p.rect_filled(run_rect, 3.0, if running { RED } else { BTN_FACE });
    p.rect_stroke(run_rect, 3.0, Stroke::new(1.0, SILVER_SHADOW), egui::StrokeKind::Inside);
    p.text(run_rect.center(), egui::Align2::CENTER_CENTER,
        if running { "■ STOP" } else { "▶ RUN/STOP" },
        egui::FontId::new(9.5, egui::FontFamily::Monospace),
        if running { Color32::WHITE } else { BTN_LBL });
    if run_resp.clicked() { kbd.toggle_seq_run(); }
}
