//! Reusable UI widgets: param knob, button painters, waveform toggle, color lerp.

use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

use super::ids;
use super::palette::*;

/// Paint a button face + label. Configurable font size and stroke width
/// so the same function serves both the transpose section (7.5 / 0.7)
/// and the right strip (8.5 / 0.8).
pub fn paint_btn(
    p: &egui::Painter,
    r: Rect,
    label: &str,
    active: bool,
    font_size: f32,
    stroke_w: f32,
) {
    p.rect_filled(r.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
    p.rect_filled(r, 2.0, if active { RED } else { BTN_FACE });
    p.rect_stroke(r, 2.0, Stroke::new(stroke_w, SILVER_SHADOW), egui::StrokeKind::Inside);
    p.text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::new(font_size, egui::FontFamily::Monospace),
        if active { Color32::WHITE } else { BTN_LBL },
    );
}

/// Waveform toggle button (SAW / SQR).
pub fn draw_wave_button(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    param: &EnumParam<crate::params::WaveformParam>,
    rect: Rect,
    id: egui::Id,
    label: &str,
    active: bool,
    target: crate::params::WaveformParam,
) {
    let tip = match target {
        crate::params::WaveformParam::Saw => "Saw wave — brighter, classic acid timbre.",
        crate::params::WaveformParam::Square => "Square wave — hollow, rubbery sub-bass timbre.",
    };
    let resp = ui
        .interact(rect, id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tip);
    let p = ui.painter_at(rect);
    let (bg, fg) = if active {
        (RED, Color32::WHITE)
    } else {
        (BTN_FACE, BTN_LBL)
    };
    p.rect_filled(rect.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
    p.rect_filled(rect, 2.0, bg);
    p.rect_stroke(
        rect,
        2.0,
        Stroke::new(0.8, SILVER_SHADOW),
        egui::StrokeKind::Inside,
    );
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::new(9.0, egui::FontFamily::Monospace),
        fg,
    );
    if resp.clicked() && !active {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, target);
        setter.end_set_parameter(param);
    }
}

/// Interactive knob for a `nih_plug` parameter.
/// `label_chip`: when true draw the dark label chip below; when false skip it
/// (caller renders label separately as panel text).
pub fn param_knob<P: Param>(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    id: egui::Id,
    param: &P,
    center: Pos2,
    radius: f32,
    label: &str,
    format_value: impl Fn(f32) -> String,
    label_chip: bool,
) -> egui::Response
where
    P::Plain: Into<f32> + Copy,
{
    let total = (radius + 10.0) * 2.0;
    let rect = Rect::from_center_size(center, Vec2::splat(total));
    let resp = ui
        .interact(rect, id, egui::Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::ResizeVertical);

    let mut norm = param.unmodulated_normalized_value();
    if resp.drag_started() {
        setter.begin_set_parameter(param);
    }
    if resp.dragged() {
        let dy = -resp.drag_delta().y;
        let speed = if ui.input(|i| i.modifiers.shift) {
            0.0015
        } else {
            0.0065
        };
        norm = (norm + dy * speed).clamp(0.0, 1.0);
        setter.set_parameter_normalized(param, norm);
    }
    if resp.drag_stopped() {
        setter.end_set_parameter(param);
    }
    if (resp.clicked() && ui.input(|i| i.modifiers.ctrl)) || resp.double_clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter_normalized(param, param.default_normalized_value());
        setter.end_set_parameter(param);
        norm = param.default_normalized_value();
    }

    let p = ui.painter_at(rect);
    // Shadow + recess
    p.circle_filled(center + Vec2::new(0.5, 1.6), radius + 4.0, SILVER_SHADOW);
    p.circle_filled(center, radius + 3.0, SILVER_DARK);
    // Grip ring
    p.circle_filled(center, radius, KNOB_RING);
    p.circle_filled(
        center - Vec2::new(0.0, radius * 0.12),
        radius * 0.96,
        Color32::from_rgb(44, 44, 48),
    );
    p.circle_filled(center, radius * 0.86, KNOB_RING);
    // Metal core
    let cr = radius * 0.58;
    p.circle_filled(center, cr + 1.8, Color32::from_rgb(140, 140, 146));
    p.circle_filled(center, cr, KNOB_CORE);
    p.circle_filled(
        center - Vec2::new(cr * 0.2, cr * 0.22),
        cr * 0.55,
        Color32::from_rgb(80, 80, 86),
    );
    p.circle_filled(center, cr * 0.1, Color32::BLACK);
    // Indicator line
    let start = std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    let angle = start + sweep * norm;
    let (s, c) = angle.sin_cos();
    let dir = Vec2::new(c, s);
    p.line_segment(
        [center + dir * (cr * 0.2), center + dir * (cr * 0.88)],
        Stroke::new(2.2, INDICATOR),
    );
    // Tick marks
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let a = start + sweep * t;
        let (sn, cs) = a.sin_cos();
        let d = Vec2::new(cs, sn);
        let major = i % 5 == 0;
        p.line_segment(
            [
                center + d * (radius + 2.0),
                center + d * (radius + if major { 6.0 } else { 4.0 }),
            ],
            Stroke::new(if major { 1.2 } else { 0.6 }, TICK),
        );
    }
    // Optional label chip below
    if label_chip && !label.is_empty() {
        let chip_w = radius * 2.8;
        let chip = Rect::from_center_size(
            Pos2::new(center.x, center.y + radius + 15.0),
            Vec2::new(chip_w, 16.0),
        );
        p.rect_filled(chip.translate(Vec2::new(0.0, 1.0)), 2.0, SILVER_LIGHT);
        p.rect_filled(chip, 2.0, LABEL_BG);
        p.rect_stroke(chip, 2.0, Stroke::new(0.8, INK), egui::StrokeKind::Inside);
        p.text(
            chip.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::new(11.0, egui::FontFamily::Monospace),
            LABEL_FG,
        );
    }
    // Stash display value
    if resp.hovered() || resp.dragged() {
        let plain: f32 = param.unmodulated_plain_value().into();
        let s = if !label.is_empty() {
            format!("{label} {}", format_value(plain))
        } else {
            format_value(plain)
        };
        ui.ctx()
            .data_mut(|d| d.insert_temp(ids::display(), s));
    }
    resp
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let lerp = |x: u8, y: u8| -> u8 {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(
        lerp(a.r(), b.r()),
        lerp(a.g(), b.g()),
        lerp(a.b(), b.b()),
    )
}
