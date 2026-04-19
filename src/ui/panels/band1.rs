//! Band 1: brand/logo strip + six main 303-style knobs.

use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

use crate::params::SquelchBoxParams;
use crate::ui::ids;
use crate::ui::palette::*;
use crate::ui::widgets::{lerp_color, param_knob, param_knob_snap};

pub fn draw_band1(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    params: &SquelchBoxParams,
    rect: Rect,
) {
    let p = ui.painter();
    let top = rect.top();

    // Brand wordmark (left-aligned with DIST toggle below)
    p.text(
        Pos2::new(rect.left() + 28.0, top + BAND1_TOP + 9.0),
        egui::Align2::LEFT_TOP,
        "SQUELCHBOX",
        egui::FontId::new(15.0, egui::FontFamily::Proportional),
        INK,
    );
    // "Bass Line" right side
    p.text(
        Pos2::new(rect.right() - 28.0, top + BAND1_TOP + 9.0),
        egui::Align2::RIGHT_TOP,
        "Bass Line",
        egui::FontId::new(17.0, egui::FontFamily::Proportional),
        INK,
    );
    p.text(
        Pos2::new(rect.right() - 28.0, top + BAND1_TOP + 29.0),
        egui::Align2::RIGHT_TOP,
        "ACID SYNTH",
        egui::FontId::new(7.5, egui::FontFamily::Monospace),
        SILVER_SHADOW,
    );

    // UI scale badge — discreet clickable cycler beneath "ACID SYNTH".
    {
        let scale_val = params.ui_scale.lock().round().clamp(1.0, 3.0) as u32;
        let scale_text = format!("UI {scale_val}×");
        let font = egui::FontId::new(9.0, egui::FontFamily::Monospace);
        let anchor = Pos2::new(rect.right() - 28.0, top + BAND1_TOP + 44.0);
        // Generous fixed hit zone so Wayland / HiDPI pointer quantisation
        // can't miss it. ~56×18 px, right-aligned to the anchor.
        let hit = Rect::from_min_size(
            Pos2::new(anchor.x - 56.0, anchor.y - 3.0),
            Vec2::new(56.0, 18.0),
        );
        let resp = ui
            .interact(hit, ids::ui_scale_btn(), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(
                "UI scale — click to cycle (1× / 2× / 3×).\n\
                 Saved to disk; restart SquelchBox to apply the new size.",
            );
        let color = if resp.hovered() { INK } else { lerp_color(INK, SILVER_SHADOW, 0.35) };
        ui.painter()
            .text(anchor, egui::Align2::RIGHT_TOP, &scale_text, font, color);
        if resp.clicked() {
            let mut lock = params.ui_scale.lock();
            let cur = lock.round() as u32;
            let next = match cur {
                1 => 2.0,
                2 => 3.0,
                _ => 1.0,
            };
            *lock = next;
            // Standalone doesn't persist `#[persist]` params between
            // sessions, so mirror the new scale to disk. Reopen the
            // window (standalone: restart) for the resize to take effect.
            crate::util::paths::save_ui_scale(next);
            nih_plug::nih_log!("[ui_scale_btn] clicked: {cur}× → {next}× (saved)");
        }
    }

    // Band1 bottom groove
    let gy = top + BAND1_BOT;
    p.line_segment(
        [Pos2::new(rect.left() + 18.0, gy), Pos2::new(rect.right() - 18.0, gy)],
        Stroke::new(1.0, SILVER_SHADOW),
    );
    p.line_segment(
        [
            Pos2::new(rect.left() + 18.0, gy + 1.0),
            Pos2::new(rect.right() - 18.0, gy + 1.0),
        ],
        Stroke::new(0.8, Color32::from_rgba_unmultiplied(255, 255, 255, 70)),
    );

    // ── Six main knobs with labels above ──
    {
        let p = ui.painter();
        for (i, &lbl) in KNOB_LABELS.iter().enumerate() {
            let cx = rect.left() + KNOB_XS[i];
            let cy = top + KNOB_Y;
            p.text(
                Pos2::new(cx, cy - KNOB_R - 7.0),
                egui::Align2::CENTER_BOTTOM,
                lbl,
                egui::FontId::new(7.5, egui::FontFamily::Monospace),
                INK,
            );
        }
    }
    let tips: [&str; 6] = [
        "Tuning — master pitch offset in semitones (±12).\nDrag: snap to semitone · Shift+drag: continuous (microtuning) · Ctrl-click/dbl-click: reset",
        "Cutoff — base filter frequency (30 Hz..12 kHz).\nDrag: adjust · Shift+drag: fine · Ctrl-click/dbl-click: reset",
        "Resonance — filter Q. ~95%+ self-oscillates.\nDrag: adjust · Shift+drag: fine · Ctrl-click/dbl-click: reset",
        "Env Mod — how far the filter envelope opens the cutoff.\nDrag: adjust · Shift+drag: fine · Ctrl-click/dbl-click: reset",
        "Decay — amp/filter envelope decay (shared, 30..2500 ms).\nDrag: adjust · Shift+drag: fine · Ctrl-click/dbl-click: reset",
        "Accent — amp/cutoff/reso boost on accented steps.\nDrag: adjust · Shift+drag: fine · Ctrl-click/dbl-click: reset",
    ];
    for i in 0..6 {
        let center = Pos2::new(rect.left() + KNOB_XS[i], top + KNOB_Y);
        let id = ids::knob1(i);
        let resp = match i {
            0 => param_knob_snap(ui, setter, id, &params.tuning, center, KNOB_R, "TUNING", |v| format!("{v:+.2} st"), false, 1.0 / 24.0),
            1 => param_knob(ui, setter, id, &params.cutoff, center, KNOB_R, "CUT FREQ", |v| format!("{v:.0} Hz"), false),
            2 => param_knob(ui, setter, id, &params.resonance, center, KNOB_R, "RESO", |v| format!("{:.0}%", v * 100.0), false),
            3 => param_knob(ui, setter, id, &params.env_mod, center, KNOB_R, "ENV MOD", |v| format!("{:.0}%", v * 100.0), false),
            4 => param_knob(ui, setter, id, &params.decay_ms, center, KNOB_R, "DECAY", |v| format!("{v:.0} ms"), false),
            5 => param_knob(ui, setter, id, &params.accent, center, KNOB_R, "ACCENT", |v| format!("{:.0}%", v * 100.0), false),
            _ => unreachable!(),
        };
        resp.on_hover_text(tips[i]);
    }
}
