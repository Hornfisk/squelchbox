//! Lower black panel orchestrator.

mod left_strip;
mod pitch_row;
mod right_strip;
mod step_area;
mod transpose;

use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Stroke};

use crate::kbd::KbdQueue;
use crate::params::SquelchBoxParams;
use crate::ui::palette::*;

pub fn draw_lower_panel(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    params: &SquelchBoxParams,
    kbd: &KbdQueue,
    rect: Rect,
) {
    let top = rect.top();
    left_strip::draw_left_strip(ui, setter, params, kbd, rect);
    pitch_row::draw_pitch_buttons(ui, rect);
    step_area::draw_step_area(ui, kbd, rect);
    transpose::draw_transpose_section(ui, kbd, rect);
    right_strip::draw_right_strip(ui, setter, params, kbd, rect);
    // Thin highlight line at the very top of the black panel
    let p = ui.painter();
    p.line_segment(
        [
            Pos2::new(rect.left() + LSTRIP_W, top + PANEL_SPLIT + 1.5),
            Pos2::new(rect.left() + STEP_X1, top + PANEL_SPLIT + 1.5),
        ],
        Stroke::new(0.5, Color32::from_rgba_unmultiplied(255, 255, 255, 18)),
    );
}
