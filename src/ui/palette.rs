//! Color palette and layout constants for the SquelchBox UI.

use nih_plug_egui::egui::Color32;

// ─── Layout constants ────────────────────────────────────────────────

/// Y where the silver panel ends and the black sequencer panel begins.
pub const PANEL_SPLIT: f32 = 220.0;

// Band 1 – brand/logo strip + the six 303-style knobs (y 18..108)
pub const BAND1_TOP: f32 = 18.0;
pub const BAND1_BOT: f32 = 108.0;
pub const KNOB_R:    f32 = 18.0;
pub const KNOB_Y:    f32 = 82.0;
pub const KNOB_XS: [f32; 6]       = [360.0, 414.0, 468.0, 522.0, 576.0, 630.0];
pub const KNOB_LABELS: [&str; 6]  = ["TUNING", "CUT FREQ", "RESO", "ENV MOD", "DECAY", "ACCENT"];

// Band 2 – TEMPO / SLIDE / MODE section / product name / VOLUME
pub const CTL_Y: f32 = 165.0;

// Lower (black) panel geometry
pub const LSTRIP_W:  f32 = 80.0;
pub const STEP_X0:   f32 = LSTRIP_W;
pub const STEP_X1:   f32 = 582.0;
pub const STEP_W:    f32 = STEP_X1 - STEP_X0;
pub const STEP_CELL: f32 = STEP_W / 16.0;
pub const TR_X:      f32 = 584.0;
pub const RSTRIP_X:  f32 = 652.0;

pub const PITCH_Y:    f32 = PANEL_SPLIT + 5.0;
pub const PITCH_H:    f32 = 20.0;
pub const SLD_Y0:     f32 = PITCH_Y + PITCH_H + 4.0;
pub const SLD_Y1:     f32 = crate::ui::BASE_H as f32 - 22.0;
pub const STEP_NUM_Y: f32 = crate::ui::BASE_H as f32 - 11.0;

// ─── Palette ─────────────────────────────────────────────────────────

pub const SILVER_LIGHT:  Color32 = Color32::from_rgb(214, 214, 218);
pub const SILVER_MID:    Color32 = Color32::from_rgb(186, 186, 192);
pub const SILVER_DARK:   Color32 = Color32::from_rgb(150, 150, 156);
pub const SILVER_SHADOW: Color32 = Color32::from_rgb(90, 90, 96);
pub const INK:           Color32 = Color32::from_rgb(30, 30, 36);
pub const RED:           Color32 = Color32::from_rgb(196, 42, 42);
pub const RED_DARK:      Color32 = Color32::from_rgb(120, 24, 24);
pub const KNOB_CORE:     Color32 = Color32::from_rgb(36, 36, 40);
pub const KNOB_RING:     Color32 = Color32::from_rgb(22, 22, 26);
pub const INDICATOR:     Color32 = Color32::from_rgb(230, 230, 234);
pub const TICK:          Color32 = Color32::from_rgb(60, 60, 66);
pub const LABEL_BG:      Color32 = Color32::from_rgb(24, 24, 28);
pub const LABEL_FG:      Color32 = Color32::from_rgb(232, 232, 236);
pub const INSET:         Color32 = Color32::from_rgb(28, 28, 32);
pub const INSET_TEXT:    Color32 = Color32::from_rgb(220, 80, 60);
pub const BLACK_PANEL:   Color32 = Color32::from_rgb(18, 18, 20);
pub const BTN_FACE:      Color32 = Color32::from_rgb(52, 52, 58);
pub const BTN_LBL:       Color32 = Color32::from_rgb(180, 180, 190);
pub const YELLOW:        Color32 = Color32::from_rgb(240, 220, 60);
pub const GREEN_LED:     Color32 = Color32::from_rgb(80, 200, 80);

pub const TOGGLE_W: f32 = 28.0;
pub const TOGGLE_H: f32 = 14.0;
pub const FX_KNOB_R: f32 = 14.0;
pub const FX_KNOB_SM: f32 = 11.0;
pub const ANIM_SPEED: f32 = 1.0 / 0.15;
