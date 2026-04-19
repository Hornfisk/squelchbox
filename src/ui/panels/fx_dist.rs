//! FX Left Zone: Distortion compartment with animated tray.

use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Vec2};

use crate::params::SquelchBoxParams;
use crate::ui::ids;
use crate::ui::palette::*;
use crate::ui::widgets::param_knob;

pub fn draw_fx_dist(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    params: &SquelchBoxParams,
    rect: Rect,
) {
    let top = rect.top();
    let dt = ui.ctx().input(|i| i.stable_dt).min(0.05);
    let enabled = params.dist_enable.value();

    // Animation progress: 0.0 = closed, 1.0 = open.
    let anim_id = ids::fx_dist_anim();
    let mut progress: f32 = ui.ctx().data(|d| d.get_temp(anim_id)).unwrap_or(0.0);
    let target = if enabled { 1.0 } else { 0.0 };
    if (progress - target).abs() > 0.001 {
        progress += (target - progress).signum() * dt * ANIM_SPEED;
        progress = progress.clamp(0.0, 1.0);
        ui.ctx().request_repaint();
    } else {
        progress = target;
    }
    ui.ctx().data_mut(|d| d.insert_temp(anim_id, progress));

    // Toggle position: aligned with DELAY/REVERB in the unified FX strip.
    let toggle_x = rect.left() + 345.0;
    let toggle_y = top + BAND1_BOT + 10.0;

    // Draw toggle switch.
    let toggle_rect = Rect::from_min_size(
        Pos2::new(toggle_x, toggle_y),
        Vec2::new(TOGGLE_W, TOGGLE_H),
    );
    let toggle_resp = ui
        .interact(toggle_rect, ids::dist_toggle(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Distortion — stomp-box diode waveshaper.\nClick to enable/disable.");

    {
        let p = ui.painter();
        let toggle_bg = if enabled { RED } else { Color32::from_rgb(52, 52, 56) };
        p.rect_filled(toggle_rect, TOGGLE_H / 2.0, toggle_bg);
        let circle_x = if enabled {
            toggle_rect.right() - TOGGLE_H / 2.0
        } else {
            toggle_rect.left() + TOGGLE_H / 2.0
        };
        let circle_color = if enabled {
            Color32::WHITE
        } else {
            Color32::from_rgb(100, 100, 106)
        };
        p.circle_filled(
            Pos2::new(circle_x, toggle_rect.center().y),
            5.0,
            circle_color,
        );

        // "DIST" label
        let label_color = if enabled { RED } else { SILVER_SHADOW };
        p.text(
            Pos2::new(toggle_rect.right() + 6.0, toggle_rect.center().y),
            egui::Align2::LEFT_CENTER,
            "DIST",
            egui::FontId::new(7.5, egui::FontFamily::Monospace),
            label_color,
        );

    }

    if toggle_resp.clicked() {
        setter.begin_set_parameter(&params.dist_enable);
        setter.set_parameter(&params.dist_enable, !enabled);
        setter.end_set_parameter(&params.dist_enable);
    }

    // DRIVE + MIX knobs — aligned with DELAY/REVERB knob row (y ≈ top+162).
    if progress > 0.5 {
        let kr = FX_KNOB_R;
        let knob_y = top + BAND1_BOT + 54.0; // matches fx_time knob_cy = zone_y+44
        let drive_cx = toggle_x + 30.0;
        let mix_cx = toggle_x + 70.0;

        {
            let p = ui.painter();
            p.text(
                Pos2::new(drive_cx, knob_y + kr + 5.0),
                egui::Align2::CENTER_TOP,
                "DRIVE",
                egui::FontId::new(6.5, egui::FontFamily::Monospace),
                INK,
            );
            p.text(
                Pos2::new(mix_cx, knob_y + kr + 5.0),
                egui::Align2::CENTER_TOP,
                "MIX",
                egui::FontId::new(6.5, egui::FontFamily::Monospace),
                INK,
            );
        }

        param_knob(
            ui,
            setter,
            ids::dist_drive(),
            &params.dist_drive,
            Pos2::new(drive_cx, knob_y),
            kr,
            "DRIVE",
            |v| format!("{:.0}%", v * 100.0),
            false,
        )
        .on_hover_text("Drive — distortion intensity.\nDrag: adjust · Shift+drag: fine · Ctrl-click: reset");

        param_knob(
            ui,
            setter,
            ids::dist_mix(),
            &params.dist_mix,
            Pos2::new(mix_cx, knob_y),
            kr,
            "MIX",
            |v| format!("{:.0}%", v * 100.0),
            false,
        )
        .on_hover_text("Dist Mix — dry/wet blend.\nDrag: adjust · Shift+drag: fine · Ctrl-click: reset");
    }
}
