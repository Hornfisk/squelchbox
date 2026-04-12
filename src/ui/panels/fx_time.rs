//! FX Right Zone: Delay + Reverb panel with animation and LED readout.

use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

use crate::params::{DelayModeParam, DelaySyncParam, SquelchBoxParams};
use crate::ui::ids;
use crate::ui::palette::*;
use crate::ui::widgets::param_knob;

pub fn draw_fx_time(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    params: &SquelchBoxParams,
    rect: Rect,
) {
    let top = rect.top();
    let dt = ui.ctx().input(|i| i.stable_dt).min(0.05);
    let delay_on = params.delay_enable.value();
    let reverb_on = params.reverb_enable.value();
    let any_on = delay_on || reverb_on;

    // Animation: 0.0 = show branding, 1.0 = show controls.
    let anim_id = ids::fx_time_anim();
    let mut progress: f32 = ui.ctx().data(|d| d.get_temp(anim_id)).unwrap_or(0.0);
    let target = if any_on { 1.0 } else { 0.0 };
    if (progress - target).abs() > 0.001 {
        progress += (target - progress).signum() * dt * (1.0 / 0.2);
        progress = progress.clamp(0.0, 1.0);
        ui.ctx().request_repaint();
    } else {
        progress = target;
    }
    ui.ctx().data_mut(|d| d.insert_temp(anim_id, progress));

    let zone_x = rect.left() + 370.0;
    let zone_y = top + BAND1_BOT + 10.0;
    let zone_w = 250.0;
    let toggle_y = zone_y;

    // ── Delay toggle ──
    let dly_toggle_rect = Rect::from_min_size(
        Pos2::new(zone_x, toggle_y),
        Vec2::new(TOGGLE_W, TOGGLE_H),
    );
    let dly_resp = ui
        .interact(dly_toggle_rect, ids::delay_toggle(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Delay — tempo-synced echo.\nClick to enable/disable.");
    {
        let p = ui.painter();
        let bg = if delay_on { RED } else { Color32::from_rgb(52, 52, 56) };
        p.rect_filled(dly_toggle_rect, TOGGLE_H / 2.0, bg);
        let cx = if delay_on { dly_toggle_rect.right() - TOGGLE_H / 2.0 } else { dly_toggle_rect.left() + TOGGLE_H / 2.0 };
        let cc = if delay_on { Color32::WHITE } else { Color32::from_rgb(100, 100, 106) };
        p.circle_filled(Pos2::new(cx, dly_toggle_rect.center().y), 5.0, cc);
        p.text(
            Pos2::new(dly_toggle_rect.right() + 4.0, dly_toggle_rect.center().y),
            egui::Align2::LEFT_CENTER, "DELAY",
            egui::FontId::new(7.0, egui::FontFamily::Monospace),
            if delay_on { RED } else { SILVER_SHADOW },
        );
    }
    if dly_resp.clicked() {
        setter.begin_set_parameter(&params.delay_enable);
        setter.set_parameter(&params.delay_enable, !delay_on);
        setter.end_set_parameter(&params.delay_enable);
    }

    // ── Reverb toggle ──
    let vrb_toggle_rect = Rect::from_min_size(
        Pos2::new(zone_x + 100.0, toggle_y),
        Vec2::new(TOGGLE_W, TOGGLE_H),
    );
    let vrb_resp = ui
        .interact(vrb_toggle_rect, ids::reverb_toggle(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Reverb — ambient room.\nClick to enable/disable.");
    {
        let p = ui.painter();
        let bg = if reverb_on { RED } else { Color32::from_rgb(52, 52, 56) };
        p.rect_filled(vrb_toggle_rect, TOGGLE_H / 2.0, bg);
        let cx = if reverb_on { vrb_toggle_rect.right() - TOGGLE_H / 2.0 } else { vrb_toggle_rect.left() + TOGGLE_H / 2.0 };
        let cc = if reverb_on { Color32::WHITE } else { Color32::from_rgb(100, 100, 106) };
        p.circle_filled(Pos2::new(cx, vrb_toggle_rect.center().y), 5.0, cc);
        p.text(
            Pos2::new(vrb_toggle_rect.right() + 4.0, vrb_toggle_rect.center().y),
            egui::Align2::LEFT_CENTER, "REVERB",
            egui::FontId::new(7.0, egui::FontFamily::Monospace),
            if reverb_on { RED } else { SILVER_SHADOW },
        );
    }
    if vrb_resp.clicked() {
        setter.begin_set_parameter(&params.reverb_enable);
        setter.set_parameter(&params.reverb_enable, !reverb_on);
        setter.end_set_parameter(&params.reverb_enable);
    }

    // Separator
    ui.painter().line_segment(
        [Pos2::new(zone_x, toggle_y + TOGGLE_H + 2.0),
         Pos2::new(zone_x + 200.0, toggle_y + TOGGLE_H + 2.0)],
        Stroke::new(0.5, SILVER_SHADOW),
    );

    // ── LED readout ──
    let led_y = zone_y + 80.0;
    {
        let display = ui.ctx()
            .data(|d| d.get_temp::<String>(ids::display()))
            .unwrap_or_else(|| format!("CUT {:.0}Hz", params.cutoff.unmodulated_plain_value()));
        let dr = Rect::from_min_size(Pos2::new(zone_x, led_y), Vec2::new(100.0, 14.0));
        let p = ui.painter();
        p.rect_filled(dr.translate(Vec2::new(0.0, 1.0)), 2.0, SILVER_LIGHT);
        p.rect_filled(dr, 2.0, INSET);
        p.rect_stroke(dr, 2.0, Stroke::new(0.8, INK), egui::StrokeKind::Inside);
        p.text(dr.center(), egui::Align2::CENTER_CENTER, &display,
            egui::FontId::new(9.0, egui::FontFamily::Monospace), INSET_TEXT);
    }

    // ── Content area ──
    let content_y = zone_y + 18.0;
    let content_h = 60.0;

    // Branding (fades out)
    if progress < 0.999 {
        let alpha = ((1.0 - progress) * 255.0) as u8;
        let brand_ink = Color32::from_rgba_unmultiplied(30, 30, 36, alpha);
        let brand_sub = Color32::from_rgba_unmultiplied(90, 90, 96, alpha);
        let p = ui.painter();
        let name_cx = zone_x + zone_w * 0.5;
        p.text(Pos2::new(name_cx, content_y + 4.0), egui::Align2::CENTER_TOP, "SB-303",
            egui::FontId::new(26.0, egui::FontFamily::Proportional), brand_ink);
        p.text(Pos2::new(name_cx, content_y + 34.0), egui::Align2::CENTER_TOP, "Computer Controlled",
            egui::FontId::new(9.0, egui::FontFamily::Proportional), brand_sub);
    }

    // ── Control rows (fade in) ──
    if progress > 0.3 {
        let both = delay_on && reverb_on;
        let kr = if both { FX_KNOB_SM } else { FX_KNOB_R };

        // ── Delay row ──
        if delay_on {
            let knob_cy = if both { content_y + 14.0 } else { content_y + content_h * 0.5 - 4.0 };
            let row_left = zone_x;

            // Mode button (ANA / CLN)
            let mode_val = params.delay_mode.value();
            let mode_lbl = match mode_val {
                DelayModeParam::Analog => "ANA",
                DelayModeParam::Clean => "CLN",
            };
            let mode_rect = Rect::from_min_size(
                Pos2::new(row_left, knob_cy - 5.0),
                Vec2::new(24.0, 12.0),
            );
            let mode_resp = ui
                .interact(mode_rect, ids::delay_mode_btn(), egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Delay mode: Analog (LP feedback) / Clean (pristine).\nClick to toggle.");
            {
                let p = ui.painter();
                let is_ana = mode_val == DelayModeParam::Analog;
                p.rect_filled(mode_rect, 2.0, if is_ana { RED_DARK } else { BTN_FACE });
                p.text(mode_rect.center(), egui::Align2::CENTER_CENTER, mode_lbl,
                    egui::FontId::new(6.5, egui::FontFamily::Monospace),
                    if is_ana { Color32::WHITE } else { BTN_LBL });
            }
            if mode_resp.clicked() {
                let next = match mode_val {
                    DelayModeParam::Analog => DelayModeParam::Clean,
                    DelayModeParam::Clean => DelayModeParam::Analog,
                };
                setter.begin_set_parameter(&params.delay_mode);
                setter.set_parameter(&params.delay_mode, next);
                setter.end_set_parameter(&params.delay_mode);
            }

            let sync_cx = row_left + 50.0;
            let fdbk_cx = row_left + 90.0;
            let mix_cx = row_left + 130.0;

            {
                let p = ui.painter();
                for (cx, lbl) in [(sync_cx, "SYNC"), (fdbk_cx, "FDBK"), (mix_cx, "MIX")] {
                    p.text(Pos2::new(cx, knob_cy + kr + 3.0), egui::Align2::CENTER_TOP, lbl,
                        egui::FontId::new(5.5, egui::FontFamily::Monospace), INK);
                }
            }

            // SYNC cycler button
            let sync_rect = Rect::from_center_size(
                Pos2::new(sync_cx, knob_cy), Vec2::new(kr * 2.0, kr * 2.0));
            let sync_resp = ui
                .interact(sync_rect, ids::delay_sync_btn(), egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Delay sync subdivision.\nClick to cycle: 1/4 → 1/8 → 1/8d → 1/16 → 1/8t");
            {
                let cur = params.delay_sync.value();
                let lbl = match cur {
                    DelaySyncParam::Quarter => "1/4",
                    DelaySyncParam::Eighth => "1/8",
                    DelaySyncParam::DottedEighth => "1/8d",
                    DelaySyncParam::Sixteenth => "1/16",
                    DelaySyncParam::TripletEighth => "1/8t",
                };
                let p = ui.painter();
                p.rect_filled(sync_rect, kr, KNOB_CORE);
                p.rect_stroke(sync_rect, kr, Stroke::new(2.0, KNOB_RING), egui::StrokeKind::Outside);
                p.text(sync_rect.center(), egui::Align2::CENTER_CENTER, lbl,
                    egui::FontId::new(if both { 6.5 } else { 8.0 }, egui::FontFamily::Monospace), INDICATOR);
            }
            if sync_resp.clicked() {
                let next = match params.delay_sync.value() {
                    DelaySyncParam::Quarter => DelaySyncParam::Eighth,
                    DelaySyncParam::Eighth => DelaySyncParam::DottedEighth,
                    DelaySyncParam::DottedEighth => DelaySyncParam::Sixteenth,
                    DelaySyncParam::Sixteenth => DelaySyncParam::TripletEighth,
                    DelaySyncParam::TripletEighth => DelaySyncParam::Quarter,
                };
                setter.begin_set_parameter(&params.delay_sync);
                setter.set_parameter(&params.delay_sync, next);
                setter.end_set_parameter(&params.delay_sync);
            }

            param_knob(ui, setter, ids::delay_fdbk(),
                &params.delay_feedback, Pos2::new(fdbk_cx, knob_cy), kr,
                "FDBK", |v| format!("{:.0}%", v * 100.0), false)
                .on_hover_text("Feedback — repeat intensity (0–90%).\nDrag: adjust · Shift+drag: fine · Ctrl-click: reset");

            param_knob(ui, setter, ids::delay_mix(),
                &params.delay_mix, Pos2::new(mix_cx, knob_cy), kr,
                "MIX", |v| format!("{:.0}%", v * 100.0), false)
                .on_hover_text("Delay Mix — dry/wet blend.\nDrag: adjust · Shift+drag: fine · Ctrl-click: reset");
        }

        // ── Separator ──
        if both {
            let sep_y = content_y + content_h * 0.5;
            ui.painter().line_segment(
                [Pos2::new(zone_x, sep_y), Pos2::new(zone_x + 160.0, sep_y)],
                Stroke::new(0.5, SILVER_SHADOW),
            );
        }

        // ── Reverb row ──
        if reverb_on {
            let knob_cy = if both { content_y + content_h - 14.0 } else { content_y + content_h * 0.5 - 4.0 };
            let row_left = zone_x;
            let decay_cx = row_left + 50.0;
            let mix_cx = row_left + 90.0;

            {
                let p = ui.painter();
                for (cx, lbl) in [(decay_cx, "DECAY"), (mix_cx, "MIX")] {
                    p.text(Pos2::new(cx, knob_cy + kr + 3.0), egui::Align2::CENTER_TOP, lbl,
                        egui::FontId::new(5.5, egui::FontFamily::Monospace), INK);
                }
            }

            param_knob(ui, setter, ids::reverb_decay(),
                &params.reverb_decay, Pos2::new(decay_cx, knob_cy), kr,
                "DECAY", |v| format!("{:.0}%", v * 100.0), false)
                .on_hover_text("Reverb Decay — room size / tail length.\nDrag: adjust · Shift+drag: fine · Ctrl-click: reset");

            param_knob(ui, setter, ids::reverb_mix(),
                &params.reverb_mix, Pos2::new(mix_cx, knob_cy), kr,
                "MIX", |v| format!("{:.0}%", v * 100.0), false)
                .on_hover_text("Reverb Mix — dry/wet blend.\nDrag: adjust · Shift+drag: fine · Ctrl-click: reset");
        }
    }
}
