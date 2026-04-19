//! SquelchBox egui editor — faithful TB-303 Bass Line skin (v1).
//!
//! Two-panel layout:
//!   Upper silver panel  – connector strip / brand+knob band / main controls
//!   Lower black panel   – pitch keys / 16-step sliders / transport strip

mod ids;
mod palette;
pub mod widgets;
pub mod keyboard;
pub mod panels;

use nih_plug::prelude::*;
use nih_plug_egui::egui;
use nih_plug_egui::{create_egui_editor, EguiState};
use std::sync::Arc;

use crate::kbd::KbdQueue;
use crate::params::SquelchBoxParams;

pub const BASE_W: u32 = 780;
pub const BASE_H: u32 = 360;

pub fn create(
    params: Arc<SquelchBoxParams>,
    editor_state: Arc<EguiState>,
    kbd: Arc<KbdQueue>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        editor_state,
        (),
        move |ctx, _state| {
            ctx.set_fonts(egui::FontDefinitions::default());
        },
        move |ctx, setter, _state| {
            keyboard::handle_keyboard(ctx, &kbd);
            keyboard::persist_pattern_if_changed(ctx, &params, &kbd);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let rect = ui.max_rect();
                    panels::faceplate::draw_faceplate(ui, rect);
                    panels::faceplate::draw_connector_strip(ui, rect);
                    panels::band1::draw_band1(ui, setter, &params, rect);
                    panels::fx_dist::draw_fx_dist(ui, setter, &params, rect);
                    panels::band2::draw_band2(ui, setter, &params, &kbd, rect);
                    panels::fx_time::draw_fx_time(ui, setter, &params, rect);
                    panels::lower::draw_lower_panel(ui, setter, &params, &kbd, rect);
                    panels::toast::draw_toast(ui, rect);
                });
        },
    )
}
