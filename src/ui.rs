//! SquelchBox egui editor — faithful TB-303 Bass Line skin (v1).
//!
//! Two-panel layout:
//!   Upper silver panel  – connector strip / brand+knob band / main controls
//!   Lower black panel   – pitch keys / 16-step sliders / transport strip

use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use nih_plug_egui::{create_egui_editor, EguiState};
use std::path::PathBuf;
use std::sync::Arc;

use crate::kbd::{key_to_semitone, KbdEvent, KbdQueue};
use crate::params::{DelayModeParam, DelaySyncParam, SquelchBoxParams, SyncMode, WaveformParam};

pub const BASE_W: u32 = 780;
pub const BASE_H: u32 = 360;

// ─── Layout constants ─────────────────────────────────────────────────

/// Y where the silver panel ends and the black sequencer panel begins.
const PANEL_SPLIT: f32 = 220.0;

// Band 1 – brand/logo strip + the six 303-style knobs (y 18..108)
const BAND1_TOP: f32 = 18.0;
const BAND1_BOT: f32 = 108.0;
const KNOB_R:    f32 = 18.0;
const KNOB_Y:    f32 = 82.0;   // knob centre row
// TUNING / CUT FREQ / RESO / ENV MOD / DECAY / ACCENT
const KNOB_XS: [f32; 6]       = [360.0, 414.0, 468.0, 522.0, 576.0, 630.0];
const KNOB_LABELS: [&str; 6]  = ["TUNING", "CUT FREQ", "RESO", "ENV MOD", "DECAY", "ACCENT"];

// Band 2 – TEMPO / SLIDE / MODE / product name / VOLUME (y 108..220)
const CTL_Y: f32 = 165.0;   // knob centres

// Lower (black) panel geometry
const LSTRIP_W:  f32 = 80.0;   // left transport strip width
const STEP_X0:   f32 = LSTRIP_W;
const STEP_X1:   f32 = 582.0;
const STEP_W:    f32 = STEP_X1 - STEP_X0;  // 502 px
const STEP_CELL: f32 = STEP_W / 16.0;       // ~31.4 px
const TR_X:      f32 = 584.0;  // Transpose section x
const RSTRIP_X:  f32 = 652.0;  // BACK / STEP / WRITE/NEXT / TAP column

const PITCH_Y:    f32 = PANEL_SPLIT + 5.0;
const PITCH_H:    f32 = 20.0;
const SLD_Y0:     f32 = PITCH_Y + PITCH_H + 4.0;   // = 249
const SLD_Y1:     f32 = BASE_H as f32 - 22.0;       // = 338
const STEP_NUM_Y: f32 = BASE_H as f32 - 11.0;       // = 349

// ─── Palette ──────────────────────────────────────────────────────────

const SILVER_LIGHT:  Color32 = Color32::from_rgb(214, 214, 218);
const SILVER_MID:    Color32 = Color32::from_rgb(186, 186, 192);
const SILVER_DARK:   Color32 = Color32::from_rgb(150, 150, 156);
const SILVER_SHADOW: Color32 = Color32::from_rgb(90, 90, 96);
const INK:           Color32 = Color32::from_rgb(30, 30, 36);
const RED:           Color32 = Color32::from_rgb(196, 42, 42);
const RED_DARK:      Color32 = Color32::from_rgb(120, 24, 24);
const KNOB_CORE:     Color32 = Color32::from_rgb(36, 36, 40);
const KNOB_RING:     Color32 = Color32::from_rgb(22, 22, 26);
const INDICATOR:     Color32 = Color32::from_rgb(230, 230, 234);
const TICK:          Color32 = Color32::from_rgb(60, 60, 66);
const LABEL_BG:      Color32 = Color32::from_rgb(24, 24, 28);
const LABEL_FG:      Color32 = Color32::from_rgb(232, 232, 236);
const INSET:         Color32 = Color32::from_rgb(28, 28, 32);
const INSET_TEXT:    Color32 = Color32::from_rgb(220, 80, 60);
const BLACK_PANEL:   Color32 = Color32::from_rgb(18, 18, 20);
const BTN_FACE:      Color32 = Color32::from_rgb(52, 52, 58);
const BTN_LBL:       Color32 = Color32::from_rgb(180, 180, 190);
const YELLOW:        Color32 = Color32::from_rgb(240, 220, 60);
const GREEN_LED:     Color32 = Color32::from_rgb(80, 200, 80);

const TOGGLE_W: f32 = 28.0;
const TOGGLE_H: f32 = 14.0;
const FX_KNOB_R: f32 = 14.0;
const ANIM_SPEED: f32 = 1.0 / 0.15; // fully open/close in ~150ms

// ─── Toast overlay ───────────────────────────────────────────────────

/// Transient notification shown after MIDI export. Stored in egui temp
/// data so it survives across frames without any editor-state struct.
#[derive(Clone)]
struct Toast {
    message: String,
    path: PathBuf,
    frame_born: u64,
}

const TOAST_LIFETIME: u64 = 600; // ~10s at 60fps
const TOAST_FADE_START: u64 = 510; // fade over last ~1.5s

fn draw_toast(ui: &mut egui::Ui, rect: Rect) {
    let frame_id = egui::Id::new("sqb_frame");
    let toast_id = egui::Id::new("sqb_toast");

    let frame: u64 = ui.ctx().data(|d| d.get_temp(frame_id)).unwrap_or(0);
    ui.ctx().data_mut(|d| d.insert_temp(frame_id, frame + 1));

    let toast: Option<Toast> = ui.ctx().data(|d| d.get_temp(toast_id));
    let Some(toast) = toast else { return };

    let age = frame.saturating_sub(toast.frame_born);
    if age > TOAST_LIFETIME {
        ui.ctx().data_mut(|d| d.remove::<Toast>(toast_id));
        return;
    }

    let alpha = if age > TOAST_FADE_START {
        ((TOAST_LIFETIME - age) as f32 / (TOAST_LIFETIME - TOAST_FADE_START) as f32 * 255.0) as u8
    } else {
        255
    };

    let path_str = toast.path.display().to_string();
    let msg = &toast.message;

    // Toast bar: centred horizontally, 24px above bottom
    let bar_w = 500.0f32.min(rect.width() - 20.0);
    let bar_h = 32.0;
    let bar_x = rect.left() + (rect.width() - bar_w) * 0.5;
    let bar_y = rect.bottom() - bar_h - 24.0;
    let bar = Rect::from_min_size(Pos2::new(bar_x, bar_y), Vec2::new(bar_w, bar_h));

    let bg = Color32::from_rgba_unmultiplied(30, 30, 36, (200.0 * alpha as f32 / 255.0) as u8);
    let fg = Color32::from_rgba_unmultiplied(232, 232, 236, alpha);
    let action_col = Color32::from_rgba_unmultiplied(80, 200, 80, alpha);
    let font = egui::FontId::new(8.5, egui::FontFamily::Monospace);

    let p = ui.painter();
    p.rect_filled(bar, 4.0, bg);

    // Message text (left-aligned, truncated)
    let text_x = bar.left() + 8.0;
    let text_max_w = bar.width() - 120.0; // leave room for buttons
    let mut text = msg.clone();
    // Crude truncation — measure galley width
    let galley = p.layout_no_wrap(text.clone(), font.clone(), fg);
    if galley.size().x > text_max_w {
        // Truncate path portion, keep prefix
        let prefix = "Exported: ...";
        let filename = toast.path.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        text = format!("{prefix}/{filename}");
    }
    p.text(
        Pos2::new(text_x, bar.center().y),
        egui::Align2::LEFT_CENTER,
        &text,
        font.clone(),
        fg,
    );

    // [OPEN] button
    let open_r = Rect::from_min_size(
        Pos2::new(bar.right() - 108.0, bar.top() + 4.0),
        Vec2::new(48.0, bar_h - 8.0),
    );
    let open_resp = ui.interact(open_r, egui::Id::new("sqb_toast_open"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    p.text(open_r.center(), egui::Align2::CENTER_CENTER, "[OPEN]", font.clone(), action_col);
    if open_resp.clicked() {
        if let Some(dir) = toast.path.parent() {
            let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
        }
    }

    // [COPY] button
    let copy_r = Rect::from_min_size(
        Pos2::new(bar.right() - 54.0, bar.top() + 4.0),
        Vec2::new(48.0, bar_h - 8.0),
    );
    let copy_resp = ui.interact(copy_r, egui::Id::new("sqb_toast_copy"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    p.text(copy_r.center(), egui::Align2::CENTER_CENTER, "[COPY]", font, action_col);
    if copy_resp.clicked() {
        ui.ctx().copy_text(path_str);
    }
}

fn set_toast(ctx: &egui::Context, message: String, path: PathBuf) {
    let frame: u64 = ctx.data(|d| d.get_temp(egui::Id::new("sqb_frame"))).unwrap_or(0);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("sqb_toast"), Toast {
        message,
        path,
        frame_born: frame,
    }));
}

// ─── Entry point ──────────────────────────────────────────────────────

pub fn create(
    params: Arc<SquelchBoxParams>,
    editor_state: Arc<EguiState>,
    kbd: Arc<KbdQueue>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        editor_state,
        (),
        |ctx, _state| {
            ctx.set_fonts(egui::FontDefinitions::default());
        },
        move |ctx, setter, _state| {
            handle_keyboard(ctx, &kbd);
            persist_pattern_if_changed(ctx, &params, &kbd);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let rect = ui.max_rect();
                    draw_faceplate(ui, rect);
                    draw_connector_strip(ui, rect);
                    draw_band1(ui, setter, &params, rect);
                    draw_fx_dist(ui, setter, &params, rect);
                    draw_band2(ui, setter, &params, &kbd, rect);
                    draw_fx_time(ui, setter, &params, rect);
                    draw_lower_panel(ui, setter, &params, &kbd, rect);
                    draw_toast(ui, rect);
                });
        },
    )
}

// ─── Keyboard input ───────────────────────────────────────────────────

fn midi_note_name(n: u8) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = (n as i32 / 12) - 1;
    let name = NAMES[(n % 12) as usize];
    format!("{name}{octave}")
}

/// Re-serialize the entire pattern bank into `params.pattern_state`
/// whenever `KbdQueue::pattern_rev` changes. Tracked in egui temp data
/// so we don't pay for serde on every frame, only on actual edits. Runs
/// on the GUI thread, so allocation is fine.
fn persist_pattern_if_changed(
    ctx: &egui::Context,
    params: &SquelchBoxParams,
    kbd: &KbdQueue,
) {
    let id = egui::Id::new("sqb_last_persisted_rev");
    let last: u64 = ctx.data(|d| d.get_temp(id)).unwrap_or(u64::MAX);
    let cur = kbd.pattern_rev();
    if last == cur {
        return;
    }
    let snap = kbd.bank_snapshot();
    if let Ok(json) = serde_json::to_string(&snap) {
        *params.pattern_state.lock() = json;
    }
    ctx.data_mut(|d| d.insert_temp(id, cur));
}

/// Generate a fresh random pattern and push it through the editor
/// queue. Seed is derived from `ctx.input().time` so every click yields
/// a different riff (no `rand` dep needed).
fn randomize_pattern(ctx: &egui::Context, kbd: &KbdQueue) {
    let t = ctx.input(|i| i.time);
    let seed = (t * 1_000_000.0) as u64 ^ 0xA5A5_5A5A_DEAD_BEEFu64;
    // Root from current Tuning isn't pulled here (params not in scope);
    // C2 (36) gives the canonical 303 register and the user can shift
    // pitch with arrow keys after.
    let pat = crate::sequencer::Pattern::random(seed, 0.65, 0.35, 0.25, 36);
    kbd.edit_pattern(move |p| {
        let keep_len = p.length;
        let keep_swing = p.swing;
        *p = pat;
        p.length = keep_len;
        p.swing = keep_swing;
    });
}

fn handle_keyboard(ctx: &egui::Context, kbd: &KbdQueue) {
    ctx.request_repaint();
    ctx.memory_mut(|m| {
        if m.focused().is_none() {
            m.request_focus(egui::Id::new("sqb_kbd_focus"));
        }
    });
    let focused = ctx.memory(|m| m.focused().is_some());
    let prev_id = egui::Id::new("sqb_prev_keys");
    let prev: std::collections::HashSet<egui::Key> = ctx
        .data(|d| d.get_temp::<std::collections::HashSet<egui::Key>>(prev_id))
        .unwrap_or_default();
    let (cur, events_len, shift) = ctx.input(|i| {
        let c: std::collections::HashSet<egui::Key> = i.keys_down.iter().copied().collect();
        (c, i.events.len(), i.modifiers.shift)
    });
    kbd.set_diag(events_len, cur.len(), focused);
    let edit_mode = kbd.selected_step().is_some();
    for key in cur.difference(&prev) {
        kbd.mark_key(&format!("{key:?}"));

        // ── Edit mode: step is selected ──
        // Arrow/A/S/R/Esc are handled by draw_step_area via key_pressed.
        // Note keys write pitch onto the selected step + audition.
        // T triggers/previews the selected step.
        if edit_mode {
            if matches!(
                key,
                egui::Key::ArrowUp
                    | egui::Key::ArrowDown
                    | egui::Key::ArrowLeft
                    | egui::Key::ArrowRight
                    | egui::Key::A
                    | egui::Key::S
                    | egui::Key::R
                    | egui::Key::Delete
                    | egui::Key::Backspace
                    | egui::Key::Escape
            ) {
                continue;
            }
            // T = trigger/preview the selected step's note
            if *key == egui::Key::T {
                if let Some(sel) = kbd.selected_step() {
                    let pat = kbd.pattern_snapshot();
                    let s = pat.steps[sel];
                    if !s.rest {
                        let velocity = if s.accent { 0.95 } else { 0.7 };
                        kbd.push(KbdEvent { on: true, note: s.semitone, velocity });
                    }
                }
                continue;
            }
            // Note keys: write pitch onto selected step + audition
            if let Some(semi) = key_to_semitone(*key) {
                if let Some(sel) = kbd.selected_step() {
                    let base = 12 * (kbd.octave() as i32 + 1);
                    let note = (base + semi).clamp(24, 60) as u8;
                    kbd.edit_pattern(|p| {
                        p.steps[sel].semitone = note;
                        p.steps[sel].rest = false;
                    });
                    let velocity = if shift { 0.95 } else { 0.7 };
                    kbd.push(KbdEvent { on: true, note, velocity });
                }
                continue;
            }
            // Fall through to global shortcuts (P/Space/Enter/brackets/etc.)
        }

        match key {
            egui::Key::ArrowDown => { let o = kbd.octave(); kbd.set_octave(o - 1); }
            egui::Key::ArrowUp   => { let o = kbd.octave(); kbd.set_octave(o + 1); }
            egui::Key::P | egui::Key::Space => { kbd.toggle_seq_run(); }
            egui::Key::Enter => { kbd.request_rewind(); }
            egui::Key::OpenBracket => { kbd.edit_pattern(|p| p.rotate_left(1)); }
            egui::Key::CloseBracket => { kbd.edit_pattern(|p| p.rotate_right(1)); }
            egui::Key::Backtick => { randomize_pattern(ctx, kbd); }
            _ => {
                // No step selected: note keys trigger live MIDI
                if let Some(semi) = key_to_semitone(*key) {
                    let base = 12 * (kbd.octave() as i32 + 1);
                    let note = (base + semi).clamp(0, 127) as u8;
                    let velocity = if shift { 0.95 } else { 0.7 };
                    kbd.push(KbdEvent { on: true, note, velocity });
                }
            }
        }
    }
    // ── T-held tracking for audition-while-scrubbing ──
    //
    // Goal: detect "user is holding T" so draw_step_area can audition
    // pitch changes from arrow keys.
    //
    // Problem: on XWayland/baseview, pressing Shift while T is held
    // causes T to vanish from keys_down AND generates a false
    // Event::Key { T, pressed: false }. Both data sources lie about
    // T being released.
    //
    // Strategy — two sources, asymmetric trust:
    //   SET t_held:   keys_down contains T  OR  Event::Key pressed=true
    //   CLEAR t_held: keys_down lacks T AND Shift is NOT held
    //
    // We never clear based on Event::Key release (unreliable).
    // We never clear while Shift is held (the false-release window).
    // We DO clear when keys_down lacks T and Shift is up — that's a
    // genuine release on a quiescent keyboard.
    let t_id = egui::Id::new("sqb_t_held");
    let mut t_held: bool = ctx.data(|d| d.get_temp(t_id)).unwrap_or(false);

    let t_in_keys_down = cur.contains(&egui::Key::T);
    let t_event_press = ctx.input(|i| {
        i.events.iter().any(|ev| {
            matches!(ev, egui::Event::Key { key: egui::Key::T, pressed: true, .. })
        })
    });

    if t_in_keys_down || t_event_press {
        t_held = true;
    } else if !shift {
        // T absent from keys_down AND Shift not held → genuine release
        t_held = false;
    }
    // When Shift IS held and T is absent from keys_down, keep
    // previous t_held state — ride through the false-release glitch.

    ctx.data_mut(|d| {
        d.insert_temp(prev_id, cur);
        d.insert_temp(t_id, t_held);
    });
}

// ─── Silver + black faceplate ─────────────────────────────────────────

fn draw_faceplate(ui: &egui::Ui, rect: Rect) {
    let p = ui.painter();
    // Silver panel: brushed-metal gradient
    let silver = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + PANEL_SPLIT));
    let bands = 60;
    let h = silver.height() / bands as f32;
    for i in 0..bands {
        let t = i as f32 / (bands - 1) as f32;
        let shade = lerp_color(SILVER_LIGHT, SILVER_DARK, t * 0.65);
        let r = Rect::from_min_size(Pos2::new(silver.left(), silver.top() + i as f32 * h), Vec2::new(silver.width(), h + 0.5));
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
    let black = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + PANEL_SPLIT), rect.max);
    p.rect_filled(black, 0.0, BLACK_PANEL);
    // Raised ledge between panels
    p.line_segment(
        [Pos2::new(rect.left(), rect.top() + PANEL_SPLIT - 1.0), Pos2::new(rect.right(), rect.top() + PANEL_SPLIT - 1.0)],
        Stroke::new(1.5, SILVER_SHADOW),
    );
    p.line_segment(
        [Pos2::new(rect.left(), rect.top() + PANEL_SPLIT), Pos2::new(rect.right(), rect.top() + PANEL_SPLIT)],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 180)),
    );
    // Outer bezel
    p.rect_stroke(rect, 4.0, Stroke::new(2.0, SILVER_SHADOW), egui::StrokeKind::Inside);
    // Corner screws
    for (dx, dy) in [(12.0f32, 12.0f32), (-12.0, 12.0), (12.0, -12.0), (-12.0, -12.0)] {
        let cx = if dx > 0.0 { rect.left() + dx } else { rect.right() + dx };
        let cy = if dy > 0.0 { rect.top() + dy } else { rect.bottom() + dy };
        draw_screw(p, Pos2::new(cx, cy));
    }
}

fn draw_screw(p: &egui::Painter, c: Pos2) {
    p.circle_filled(c + Vec2::new(0.5, 0.8), 4.2, SILVER_SHADOW);
    p.circle_filled(c, 4.0, SILVER_MID);
    p.circle_filled(c - Vec2::new(0.8, 0.8), 2.4, SILVER_LIGHT);
    p.line_segment([c + Vec2::new(-2.8, -2.8), c + Vec2::new(2.8, 2.8)], Stroke::new(1.0, INK));
}

// ─── Connector strip (decorative top strip) ───────────────────────────

fn draw_connector_strip(ui: &egui::Ui, rect: Rect) {
    let p = ui.painter();
    let strip = Rect::from_min_size(rect.min, Vec2::new(rect.width(), BAND1_TOP));
    p.rect_filled(strip, 0.0, lerp_color(SILVER_DARK, INK, 0.3));
    p.line_segment(
        [Pos2::new(rect.left(), rect.top() + BAND1_TOP - 0.5), Pos2::new(rect.right(), rect.top() + BAND1_TOP - 0.5)],
        Stroke::new(1.0, SILVER_SHADOW),
    );
    let cy = rect.top() + BAND1_TOP * 0.5;
    let font = egui::FontId::new(7.0, egui::FontFamily::Monospace);
    for (lbl, x) in [("MIX IN", 28.0f32), ("MIDI IN", 60.0), ("SYNC IN", 96.0)] {
        p.text(Pos2::new(rect.left() + x, cy), egui::Align2::LEFT_CENTER, lbl, font.clone(), BTN_LBL);
        // Tiny jack hole
        p.circle_filled(Pos2::new(rect.left() + x - 6.0, cy), 2.5, INK);
        p.circle_stroke(Pos2::new(rect.left() + x - 6.0, cy), 2.5, Stroke::new(0.6, SILVER_SHADOW));
    }
    for (lbl, xr) in [("DC 9V", 28.0f32), ("OUTPUT", 64.0), ("HEADPHONE", 108.0), ("GATE", 154.0), ("CV", 180.0)] {
        let x = rect.right() - xr;
        p.text(Pos2::new(x, cy), egui::Align2::RIGHT_CENTER, lbl, font.clone(), BTN_LBL);
        p.circle_filled(Pos2::new(x + 6.0, cy), 2.5, INK);
        p.circle_stroke(Pos2::new(x + 6.0, cy), 2.5, Stroke::new(0.6, SILVER_SHADOW));
    }
}

// ─── Band 1: brand logo + six main knobs + "Bass Line" ─────────────────

fn draw_band1(ui: &mut egui::Ui, setter: &ParamSetter, params: &SquelchBoxParams, rect: Rect) {
    let p = ui.painter();
    let top = rect.top();

    // ── Our logo (replaces Roland red square) ──
    let logo_x = rect.left() + 28.0;
    let logo_y = top + BAND1_TOP + 8.0;
    let logo = Rect::from_min_size(Pos2::new(logo_x, logo_y), Vec2::new(22.0, 22.0));
    p.rect_filled(logo.translate(Vec2::new(0.5, 1.0)), 2.0, RED_DARK);
    p.rect_filled(logo, 2.0, RED);
    p.text(logo.center() + Vec2::new(0.0, 0.5), egui::Align2::CENTER_CENTER, "S",
        egui::FontId::new(14.0, egui::FontFamily::Proportional), Color32::WHITE);

    // Brand wordmark
    p.text(Pos2::new(rect.left() + 56.0, top + BAND1_TOP + 9.0), egui::Align2::LEFT_TOP,
        "SQUELCHBOX", egui::FontId::new(15.0, egui::FontFamily::Proportional), INK);
    p.text(Pos2::new(rect.left() + 56.0, top + BAND1_TOP + 27.0), egui::Align2::LEFT_TOP,
        "COMPUTER CONTROLLED BASS LINE",
        egui::FontId::new(7.5, egui::FontFamily::Monospace), SILVER_SHADOW);

    // "Bass Line" right side
    p.text(Pos2::new(rect.right() - 28.0, top + BAND1_TOP + 9.0), egui::Align2::RIGHT_TOP,
        "Bass Line", egui::FontId::new(17.0, egui::FontFamily::Proportional), INK);
    p.text(Pos2::new(rect.right() - 28.0, top + BAND1_TOP + 29.0), egui::Align2::RIGHT_TOP,
        "ACID SYNTH", egui::FontId::new(7.5, egui::FontFamily::Monospace), SILVER_SHADOW);

    // Band1 bottom groove
    let gy = top + BAND1_BOT;
    p.line_segment([Pos2::new(rect.left() + 18.0, gy), Pos2::new(rect.right() - 18.0, gy)],
        Stroke::new(1.0, SILVER_SHADOW));
    p.line_segment([Pos2::new(rect.left() + 18.0, gy + 1.0), Pos2::new(rect.right() - 18.0, gy + 1.0)],
        Stroke::new(0.8, Color32::from_rgba_unmultiplied(255, 255, 255, 70)));

    // ── Six main knobs with labels above ──────────────────────────────
    // Labels are drawn in the scoped painter block above, before param_knob
    // calls that need a mutable borrow of ui.
    {
        let p = ui.painter();
        for (i, &lbl) in KNOB_LABELS.iter().enumerate() {
            let cx = rect.left() + KNOB_XS[i];
            let cy = top + KNOB_Y;
            p.text(Pos2::new(cx, cy - KNOB_R - 7.0), egui::Align2::CENTER_BOTTOM,
                lbl, egui::FontId::new(7.5, egui::FontFamily::Monospace), INK);
        }
    }
    let tips: [&str; 6] = [
        "Tuning — master pitch offset in semitones (±12).\nDrag: adjust · Shift+drag: fine · Ctrl-click/dbl-click: reset",
        "Cutoff — base filter frequency (30 Hz..12 kHz).\nDrag: adjust · Shift+drag: fine · Ctrl-click/dbl-click: reset",
        "Resonance — filter Q. ~95%+ self-oscillates.\nDrag: adjust · Shift+drag: fine · Ctrl-click/dbl-click: reset",
        "Env Mod — how far the filter envelope opens the cutoff.\nDrag: adjust · Shift+drag: fine · Ctrl-click/dbl-click: reset",
        "Decay — amp/filter envelope decay (shared, 30..2500 ms).\nDrag: adjust · Shift+drag: fine · Ctrl-click/dbl-click: reset",
        "Accent — amp/cutoff/reso boost on accented steps.\nDrag: adjust · Shift+drag: fine · Ctrl-click/dbl-click: reset",
    ];
    for i in 0..6 {
        let center = Pos2::new(rect.left() + KNOB_XS[i], top + KNOB_Y);
        let id = egui::Id::new(("sqb_k1", i));
        let resp = match i {
            0 => param_knob(ui, setter, id, &params.tuning,    center, KNOB_R, "TUNING",   |v| format!("{v:+.2} st"), false),
            1 => param_knob(ui, setter, id, &params.cutoff,    center, KNOB_R, "CUT FREQ", |v| format!("{v:.0} Hz"),  false),
            2 => param_knob(ui, setter, id, &params.resonance, center, KNOB_R, "RESO",     |v| format!("{:.0}%", v * 100.0), false),
            3 => param_knob(ui, setter, id, &params.env_mod,   center, KNOB_R, "ENV MOD",  |v| format!("{:.0}%", v * 100.0), false),
            4 => param_knob(ui, setter, id, &params.decay_ms,  center, KNOB_R, "DECAY",    |v| format!("{v:.0} ms"), false),
            5 => param_knob(ui, setter, id, &params.accent,    center, KNOB_R, "ACCENT",   |v| format!("{:.0}%", v * 100.0), false),
            _ => unreachable!(),
        };
        resp.on_hover_text(tips[i]);
    }
}

// ─── FX Left Zone: Distortion compartment ───────────────────────────

fn draw_fx_dist(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    params: &SquelchBoxParams,
    rect: Rect,
) {
    let top = rect.top();
    let dt = ui.ctx().input(|i| i.stable_dt).min(0.05);
    let enabled = params.dist_enable.value();

    // Animation progress: 0.0 = closed, 1.0 = open.
    let anim_id = egui::Id::new("sqb_fx_dist_anim");
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

    // Toggle position: below brand text.
    let toggle_x = rect.left() + 28.0;
    let toggle_y = top + 50.0;

    // Draw toggle switch.
    let toggle_rect = Rect::from_min_size(
        Pos2::new(toggle_x, toggle_y),
        Vec2::new(TOGGLE_W, TOGGLE_H),
    );
    let toggle_id = egui::Id::new("sqb_dist_toggle");
    let toggle_resp = ui
        .interact(toggle_rect, toggle_id, egui::Sense::click())
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
        let circle_color = if enabled { Color32::WHITE } else { Color32::from_rgb(100, 100, 106) };
        p.circle_filled(Pos2::new(circle_x, toggle_rect.center().y), 5.0, circle_color);

        // "DIST" label
        let label_color = if enabled { RED } else { SILVER_SHADOW };
        p.text(
            Pos2::new(toggle_rect.right() + 6.0, toggle_rect.center().y),
            egui::Align2::LEFT_CENTER,
            "DIST",
            egui::FontId::new(7.5, egui::FontFamily::Monospace),
            label_color,
        );

        // Separator line
        p.line_segment(
            [
                Pos2::new(toggle_rect.right() + 36.0, toggle_rect.center().y),
                Pos2::new(rect.left() + 320.0, toggle_rect.center().y),
            ],
            Stroke::new(0.5, SILVER_SHADOW),
        );
    }

    if toggle_resp.clicked() {
        setter.begin_set_parameter(&params.dist_enable);
        setter.set_parameter(&params.dist_enable, !enabled);
        setter.end_set_parameter(&params.dist_enable);
    }

    // Animated compartment: DRIVE + MIX knobs in a recessed tray.
    if progress > 0.001 {
        let tray_y = toggle_y + TOGGLE_H + 4.0;
        let tray_h = 44.0 * progress;
        let tray = Rect::from_min_size(
            Pos2::new(toggle_x, tray_y),
            Vec2::new(120.0, tray_h),
        );

        // Recessed inset background
        {
            let p = ui.painter();
            p.rect_filled(tray, 3.0, lerp_color(SILVER_MID, SILVER_DARK, 0.4));
            p.rect_stroke(tray, 3.0, Stroke::new(0.8, SILVER_SHADOW), egui::StrokeKind::Inside);
        }

        // Only draw knobs when enough space is revealed.
        if progress > 0.5 {
            let knob_y = tray_y + 16.0;
            let drive_cx = toggle_x + 30.0;
            let mix_cx = toggle_x + 80.0;

            {
                let p = ui.painter();
                p.text(
                    Pos2::new(drive_cx, knob_y + FX_KNOB_R + 5.0),
                    egui::Align2::CENTER_TOP,
                    "DRIVE",
                    egui::FontId::new(6.5, egui::FontFamily::Monospace),
                    INK,
                );
                p.text(
                    Pos2::new(mix_cx, knob_y + FX_KNOB_R + 5.0),
                    egui::Align2::CENTER_TOP,
                    "MIX",
                    egui::FontId::new(6.5, egui::FontFamily::Monospace),
                    INK,
                );
            }

            param_knob(
                ui, setter,
                egui::Id::new("sqb_dist_drive"),
                &params.dist_drive,
                Pos2::new(drive_cx, knob_y),
                FX_KNOB_R,
                "DRIVE",
                |v| format!("{:.0}%", v * 100.0),
                false,
            )
            .on_hover_text("Drive — distortion intensity.\nDrag: adjust · Shift+drag: fine · Ctrl-click: reset");

            param_knob(
                ui, setter,
                egui::Id::new("sqb_dist_mix"),
                &params.dist_mix,
                Pos2::new(mix_cx, knob_y),
                FX_KNOB_R,
                "MIX",
                |v| format!("{:.0}%", v * 100.0),
                false,
            )
            .on_hover_text("Dist Mix — dry/wet blend.\nDrag: adjust · Shift+drag: fine · Ctrl-click: reset");
        }
    }
}

// ─── Band 2: TEMPO / SLIDE / MODE section / product name / VOLUME ──────

fn draw_band2(ui: &mut egui::Ui, setter: &ParamSetter, params: &SquelchBoxParams, kbd: &KbdQueue, rect: Rect) {
    let top = rect.top();
    let lbl_y = top + BAND1_BOT + 12.0;
    let tx   = rect.left() + 68.0;
    let px   = rect.left() + 162.0;
    let mx   = rect.left() + 280.0;
    let _name_cx = rect.left() + 430.0;
    let vx   = rect.left() + 724.0;
    let track_x0 = rect.left() + 128.0;

    // All decorative painting in one scoped block so the immutable borrow
    // of ui ends before the param_knob calls below.
    {
        let p = ui.painter();

        // TEMPO labels
        p.text(Pos2::new(tx, lbl_y), egui::Align2::CENTER_TOP,
            "TEMPO", egui::FontId::new(7.5, egui::FontFamily::Monospace), INK);
        p.text(Pos2::new(tx - 32.0, top + CTL_Y + 34.0), egui::Align2::LEFT_CENTER,
            "SLOW", egui::FontId::new(6.5, egui::FontFamily::Monospace), SILVER_SHADOW);
        p.text(Pos2::new(tx + 32.0, top + CTL_Y + 34.0), egui::Align2::RIGHT_CENTER,
            "FAST", egui::FontId::new(6.5, egui::FontFamily::Monospace), SILVER_SHADOW);

        // BANK label (the I/II/III/IV buttons themselves are painted
        // below after the painter borrow ends so we can wire clicks).
        p.text(Pos2::new(track_x0, lbl_y), egui::Align2::LEFT_TOP,
            "BANK", egui::FontId::new(7.0, egui::FontFamily::Monospace), SILVER_SHADOW);
        p.text(Pos2::new(px, lbl_y + 28.0), egui::Align2::CENTER_TOP,
            "SLIDE", egui::FontId::new(7.5, egui::FontFamily::Monospace), INK);

        // MODE panel frame (the interactive buttons themselves are
        // painted below, after the painter borrow ends, so we can
        // service their click responses).
        let mode_rect = Rect::from_min_size(Pos2::new(mx, top + BAND1_BOT + 6.0), Vec2::new(84.0, 104.0));
        p.rect_filled(mode_rect.translate(Vec2::new(0.0, 1.0)), 3.0, SILVER_LIGHT);
        p.rect_filled(mode_rect, 3.0, lerp_color(SILVER_MID, SILVER_DARK, 0.3));
        p.rect_stroke(mode_rect, 3.0, Stroke::new(1.0, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(Pos2::new(mx + 42.0, top + BAND1_BOT + 8.0), egui::Align2::CENTER_TOP,
            "SYNC", egui::FontId::new(7.5, egui::FontFamily::Monospace), INK);

        // VOLUME label
        p.text(Pos2::new(vx, lbl_y), egui::Align2::CENTER_TOP,
            "VOLUME", egui::FontId::new(7.5, egui::FontFamily::Monospace), INK);
    } // painter borrow ends here

    // ── SYNC mode buttons (interactive) ─────────────────────────────
    let cur_mode = params.sync_mode.value();
    let modes = [
        (SyncMode::Internal, "INTERNAL", "Internal — free-run sequencer.\nRUN/STOP and TEMPO knob drive playback.\nIgnores DAW transport."),
        (SyncMode::Host,     "▶ HOST",   "Host — follow the DAW transport.\nTempo slaved to host BPM, plays when host plays.\nResets to step 1 on every host play press."),
        (SyncMode::Midi,     "MIDI IN",  "MIDI — sequencer disabled.\nVoice triggered only by incoming MIDI notes\nfrom the host or computer keyboard."),
    ];
    for (j, (mode, lbl, tip)) in modes.iter().enumerate() {
        let by = top + BAND1_BOT + 24.0 + j as f32 * 26.0;
        let br = Rect::from_min_size(Pos2::new(mx + 4.0, by), Vec2::new(76.0, 18.0));
        let id = egui::Id::new(("sqb_sync", j));
        let resp = ui
            .interact(br, id, egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(*tip);
        let active = cur_mode == *mode;
        let p = ui.painter();
        p.rect_filled(br.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
        p.rect_filled(br, 2.0, if active { RED } else { BTN_FACE });
        p.rect_stroke(br, 2.0, Stroke::new(0.8, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(br.center(), egui::Align2::CENTER_CENTER, *lbl,
            egui::FontId::new(7.5, egui::FontFamily::Monospace),
            if active { Color32::WHITE } else { BTN_LBL });
        if resp.clicked() {
            setter.begin_set_parameter(&params.sync_mode);
            setter.set_parameter(&params.sync_mode, *mode);
            setter.end_set_parameter(&params.sync_mode);
        }
    }

    // ── BANK I/II/III/IV (interactive, queued at next bar) ─────────
    let cur_bank = kbd.current_bank() as usize;
    let queued = kbd.queued_bank();
    for (j, lbl) in ["I", "II", "III", "IV"].iter().enumerate() {
        let bx = track_x0 + 2.0 + j as f32 * 18.0;
        let by = lbl_y + 12.0;
        let br = Rect::from_min_size(Pos2::new(bx, by), Vec2::new(14.0, 10.0));
        let id = egui::Id::new(("sqb_bank", j));
        let resp = ui
            .interact(br, id, egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!(
                "Bank {} — switch the active pattern slot.\nQueued; takes effect at the next pattern-loop boundary.",
                lbl
            ));
        let active = j == cur_bank;
        let pending = queued == Some(j as u8) && !active;
        let p = ui.painter();
        p.rect_filled(br, 2.0, if active { RED } else if pending { lerp_color(BTN_FACE, RED, 0.45) } else { BTN_FACE });
        p.rect_stroke(br, 2.0, Stroke::new(0.7, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(br.center(), egui::Align2::CENTER_CENTER, *lbl,
            egui::FontId::new(6.5, egui::FontFamily::Monospace),
            if active { Color32::WHITE } else { BTN_LBL });
        if resp.clicked() {
            kbd.queue_bank(j as u8);
        }
    }

    // ── LEN spinner (pattern length 1..16) ─────────────────────────
    // Sits to the RIGHT of the BANK row on the same Y, because the
    // area directly below BANK overlaps the SLIDE knob's hit-rect and
    // eats clicks.
    let cur_len = kbd.pattern_snapshot().length.clamp(1, 16);
    let len_y = lbl_y + 12.0;
    let len_label_x = track_x0 + 80.0;
    let len_x0 = len_label_x + 20.0;
    ui.painter().text(Pos2::new(len_label_x, len_y + 1.0), egui::Align2::LEFT_TOP,
        "LEN", egui::FontId::new(7.0, egui::FontFamily::Monospace), SILVER_SHADOW);
    let len_dn = Rect::from_min_size(Pos2::new(len_x0, len_y), Vec2::new(12.0, 10.0));
    let len_box = Rect::from_min_size(Pos2::new(len_x0 + 14.0, len_y), Vec2::new(20.0, 10.0));
    let len_up = Rect::from_min_size(Pos2::new(len_x0 + 36.0, len_y), Vec2::new(12.0, 10.0));
    let dn_resp = ui.interact(len_dn, egui::Id::new("sqb_len_dn"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Pattern length − (down to 1)");
    let up_resp = ui.interact(len_up, egui::Id::new("sqb_len_up"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Pattern length + (up to 16)");
    let p = ui.painter();
    for (r, lbl) in [(len_dn, "−"), (len_up, "+")] {
        p.rect_filled(r, 2.0, BTN_FACE);
        p.rect_stroke(r, 2.0, Stroke::new(0.6, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(r.center(), egui::Align2::CENTER_CENTER, lbl,
            egui::FontId::new(8.5, egui::FontFamily::Monospace), LABEL_FG);
    }
    p.rect_filled(len_box, 2.0, INSET);
    p.rect_stroke(len_box, 2.0, Stroke::new(0.6, INK), egui::StrokeKind::Inside);
    p.text(len_box.center(), egui::Align2::CENTER_CENTER, format!("{cur_len}"),
        egui::FontId::new(9.0, egui::FontFamily::Monospace), INSET_TEXT);
    if dn_resp.clicked() && cur_len > 1 {
        kbd.edit_pattern(|p| p.length = (p.length - 1).max(1));
    }
    if up_resp.clicked() && cur_len < 16 {
        kbd.edit_pattern(|p| p.length = (p.length + 1).min(16));
    }

    // Interactive knobs (need mutable ui)
    param_knob(ui, setter, egui::Id::new("sqb_tempo"), &params.seq_bpm,
        Pos2::new(tx, top + CTL_Y), 30.0, "TEMPO", |v| format!("{v:.0} BPM"), false)
        .on_hover_text("Tempo — sequencer BPM (40..220).\nDrag the knob, or click the box below to type.");
    param_knob(ui, setter, egui::Id::new("sqb_slide"), &params.slide_ms,
        Pos2::new(px, top + CTL_Y), 20.0, "SLIDE", |v| format!("{v:.0} ms"), false)
        .on_hover_text("Slide — portamento glide time (5..500 ms).\nHow long a slide-legato step takes to reach its target pitch.");

    // ── OCT ▼/▲ for the selected step ──────────────────────────────
    // Below the SLIDE knob, in the gap before the lower panel.
    {
        let oct_y = top + CTL_Y + 38.0;
        let oct_dn = Rect::from_min_size(Pos2::new(px - 22.0, oct_y), Vec2::new(18.0, 10.0));
        let oct_up = Rect::from_min_size(Pos2::new(px + 4.0, oct_y), Vec2::new(18.0, 10.0));
        let dn_r = ui.interact(oct_dn, egui::Id::new("sqb_oct_dn"), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Octave down — selected step −12 semitones.\nKeyboard: Shift+Down");
        let up_r = ui.interact(oct_up, egui::Id::new("sqb_oct_up"), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Octave up — selected step +12 semitones.\nKeyboard: Shift+Up");
        let p = ui.painter();
        p.text(Pos2::new(px - 8.0, oct_y - 2.0), egui::Align2::CENTER_BOTTOM,
            "OCT", egui::FontId::new(6.5, egui::FontFamily::Monospace), SILVER_SHADOW);
        for (r, lbl) in [(oct_dn, "▼"), (oct_up, "▲")] {
            p.rect_filled(r, 2.0, BTN_FACE);
            p.rect_stroke(r, 2.0, Stroke::new(0.6, SILVER_SHADOW), egui::StrokeKind::Inside);
            p.text(r.center(), egui::Align2::CENTER_CENTER, lbl,
                egui::FontId::new(7.0, egui::FontFamily::Monospace), BTN_LBL);
        }
        if let Some(sel) = kbd.selected_step() {
            if sel < 16 {
                if dn_r.clicked() {
                    kbd.edit_pattern(|pat| {
                        let cur = pat.steps[sel].semitone as i32;
                        pat.steps[sel].semitone = (cur - 12).clamp(24, 60) as u8;
                    });
                }
                if up_r.clicked() {
                    kbd.edit_pattern(|pat| {
                        let cur = pat.steps[sel].semitone as i32;
                        pat.steps[sel].semitone = (cur + 12).clamp(24, 60) as u8;
                    });
                }
            }
        }
    }

    param_knob(ui, setter, egui::Id::new("sqb_vol"), &params.master_volume,
        Pos2::new(vx, top + CTL_Y), 26.0, "VOLUME",
        |v| { let db = 20.0 * v.max(1e-6).log10(); if db < -59.0 { "-inf".into() } else { format!("{db:+.1} dB") } },
        false)
        .on_hover_text("Volume — master output gain (-60..+6 dB).");

    // Manual BPM text entry under the TEMPO knob.
    let bpm_id = egui::Id::new("sqb_bpm_edit");
    let bpm_rect = Rect::from_center_size(
        Pos2::new(tx, top + CTL_Y + 50.0),
        Vec2::new(46.0, 15.0),
    );
    let focused = ui.memory(|m| m.has_focus(bpm_id));
    let mut bpm_str = if focused {
        ui.ctx()
            .data(|d| d.get_temp::<String>(bpm_id))
            .unwrap_or_else(|| format!("{:.0}", params.seq_bpm.unmodulated_plain_value()))
    } else {
        format!("{:.0}", params.seq_bpm.unmodulated_plain_value())
    };
    let resp = ui.put(
        bpm_rect,
        egui::TextEdit::singleline(&mut bpm_str)
            .id(bpm_id)
            .font(egui::FontId::new(9.0, egui::FontFamily::Monospace))
            .horizontal_align(egui::Align::Center)
            .desired_width(46.0),
    ).on_hover_text("BPM — click to type a value, Enter to commit (40..220).");
    if resp.has_focus() {
        ui.ctx().data_mut(|d| d.insert_temp(bpm_id, bpm_str.clone()));
    }
    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        if let Ok(v) = bpm_str.trim().parse::<f32>() {
            let clamped = v.clamp(40.0, 220.0);
            setter.begin_set_parameter(&params.seq_bpm);
            setter.set_parameter(&params.seq_bpm, clamped);
            setter.end_set_parameter(&params.seq_bpm);
        }
        ui.ctx().data_mut(|d| d.remove::<String>(bpm_id));
    }
}

// ─── FX Right Zone: Delay + Reverb panel ────────────────────────────
//
// Layout uses three fixed vertical anchors so nothing jumps:
//
//   zone_y + 0  : DELAY / REVERB toggles  (always here)
//   zone_y + 16 : content area (62 px)     branding OR control rows
//   zone_y + 80 : LED readout             (always here)
//
// Knob radius shrinks from FX_KNOB_R to FX_KNOB_SM when both rows
// need to share the content area.

const FX_KNOB_SM: f32 = 11.0; // compact knob for dual-row mode

fn draw_fx_time(
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
    let anim_id = egui::Id::new("sqb_fx_time_anim");
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

    // Zone geometry — right of SYNC panel, left of VOLUME knob.
    let zone_x = rect.left() + 370.0;
    let zone_y = top + BAND1_BOT + 10.0;
    let zone_w = 250.0;

    // ── Fixed anchor: toggle row (always at zone_y) ──
    let toggle_y = zone_y;

    // Delay toggle
    let dly_toggle_rect = Rect::from_min_size(
        Pos2::new(zone_x, toggle_y),
        Vec2::new(TOGGLE_W, TOGGLE_H),
    );
    let dly_resp = ui
        .interact(dly_toggle_rect, egui::Id::new("sqb_delay_toggle"), egui::Sense::click())
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

    // Reverb toggle
    let vrb_toggle_rect = Rect::from_min_size(
        Pos2::new(zone_x + 100.0, toggle_y),
        Vec2::new(TOGGLE_W, TOGGLE_H),
    );
    let vrb_resp = ui
        .interact(vrb_toggle_rect, egui::Id::new("sqb_reverb_toggle"), egui::Sense::click())
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

    // Separator line below toggles
    ui.painter().line_segment(
        [Pos2::new(zone_x, toggle_y + TOGGLE_H + 2.0),
         Pos2::new(zone_x + 200.0, toggle_y + TOGGLE_H + 2.0)],
        Stroke::new(0.5, SILVER_SHADOW),
    );

    // ── Fixed anchor: LED readout (always at zone_y + 80) ──
    let led_y = zone_y + 80.0;
    {
        let display = ui.ctx()
            .data(|d| d.get_temp::<String>(egui::Id::new("sqb_display")))
            .unwrap_or_else(|| format!("CUT {:.0}Hz", params.cutoff.unmodulated_plain_value()));
        let dr = Rect::from_min_size(Pos2::new(zone_x, led_y), Vec2::new(100.0, 14.0));
        let p = ui.painter();
        p.rect_filled(dr.translate(Vec2::new(0.0, 1.0)), 2.0, SILVER_LIGHT);
        p.rect_filled(dr, 2.0, INSET);
        p.rect_stroke(dr, 2.0, Stroke::new(0.8, INK), egui::StrokeKind::Inside);
        p.text(dr.center(), egui::Align2::CENTER_CENTER, &display,
            egui::FontId::new(9.0, egui::FontFamily::Monospace), INSET_TEXT);
    }

    // ── Content area: zone_y+18 to zone_y+78 (60px) ──
    let content_y = zone_y + 18.0;
    let content_h = 60.0;

    // Branding (fades out)
    if progress < 0.999 {
        let alpha = ((1.0 - progress) * 255.0) as u8;
        let brand_ink = Color32::from_rgba_unmultiplied(30, 30, 36, alpha);
        let brand_sub = Color32::from_rgba_unmultiplied(90, 90, 96, alpha);
        let p = ui.painter();
        let name_cx = zone_x + zone_w * 0.5;
        p.text(
            Pos2::new(name_cx, content_y + 4.0),
            egui::Align2::CENTER_TOP, "SB-303",
            egui::FontId::new(26.0, egui::FontFamily::Proportional), brand_ink,
        );
        p.text(
            Pos2::new(name_cx, content_y + 34.0),
            egui::Align2::CENTER_TOP, "Computer Controlled",
            egui::FontId::new(9.0, egui::FontFamily::Proportional), brand_sub,
        );
    }

    // ── Control rows (fade in) ──
    if progress > 0.3 {
        let both = delay_on && reverb_on;
        let kr = if both { FX_KNOB_SM } else { FX_KNOB_R };

        // ── Delay row ──
        if delay_on {
            // When solo, center vertically in content area. When both, use top half.
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
                .interact(mode_rect, egui::Id::new("sqb_delay_mode_btn"), egui::Sense::click())
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

            // Knob positions
            let sync_cx = row_left + 50.0;
            let fdbk_cx = row_left + 90.0;
            let mix_cx  = row_left + 130.0;

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
                .interact(sync_rect, egui::Id::new("sqb_delay_sync_btn"), egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Delay sync subdivision.\nClick to cycle: 1/4 Right 1/8 Right 1/8d Right 1/16 Right 1/8t");
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

            param_knob(ui, setter, egui::Id::new("sqb_delay_fdbk"),
                &params.delay_feedback, Pos2::new(fdbk_cx, knob_cy), kr,
                "FDBK", |v| format!("{:.0}%", v * 100.0), false)
                .on_hover_text("Feedback — repeat intensity (0–90%).\nDrag: adjust · Shift+drag: fine · Ctrl-click: reset");

            param_knob(ui, setter, egui::Id::new("sqb_delay_mix"),
                &params.delay_mix, Pos2::new(mix_cx, knob_cy), kr,
                "MIX", |v| format!("{:.0}%", v * 100.0), false)
                .on_hover_text("Delay Mix — dry/wet blend.\nDrag: adjust · Shift+drag: fine · Ctrl-click: reset");
        }

        // ── Separator (only when both rows visible) ──
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
            let mix_cx   = row_left + 90.0;

            {
                let p = ui.painter();
                for (cx, lbl) in [(decay_cx, "DECAY"), (mix_cx, "MIX")] {
                    p.text(Pos2::new(cx, knob_cy + kr + 3.0), egui::Align2::CENTER_TOP, lbl,
                        egui::FontId::new(5.5, egui::FontFamily::Monospace), INK);
                }
            }

            param_knob(ui, setter, egui::Id::new("sqb_reverb_decay"),
                &params.reverb_decay, Pos2::new(decay_cx, knob_cy), kr,
                "DECAY", |v| format!("{:.0}%", v * 100.0), false)
                .on_hover_text("Reverb Decay — room size / tail length.\nDrag: adjust · Shift+drag: fine · Ctrl-click: reset");

            param_knob(ui, setter, egui::Id::new("sqb_reverb_mix"),
                &params.reverb_mix, Pos2::new(mix_cx, knob_cy), kr,
                "MIX", |v| format!("{:.0}%", v * 100.0), false)
                .on_hover_text("Reverb Mix — dry/wet blend.\nDrag: adjust · Shift+drag: fine · Ctrl-click: reset");
        }
    }
}

// ─── Lower black panel ────────────────────────────────────────────────

fn draw_lower_panel(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    params: &SquelchBoxParams,
    kbd: &KbdQueue,
    rect: Rect,
) {
    let top = rect.top();
    draw_left_strip(ui, setter, params, kbd, rect);
    draw_step_area(ui, kbd, rect);
    draw_transpose_section(ui, kbd, rect);
    draw_right_strip(ui, setter, params, kbd, rect);
    // Thin highlight line at the very top of the black panel
    let p = ui.painter();
    p.line_segment(
        [Pos2::new(rect.left() + LSTRIP_W, top + PANEL_SPLIT + 1.5),
         Pos2::new(rect.left() + STEP_X1, top + PANEL_SPLIT + 1.5)],
        Stroke::new(0.5, Color32::from_rgba_unmultiplied(255, 255, 255, 18)),
    );
}

// Left strip: transport controls + waveform selector
fn draw_left_strip(
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
    let strip = Rect::from_min_size(Pos2::new(rect.left(), panel_top), Vec2::new(LSTRIP_W, BASE_H as f32 - PANEL_SPLIT));
    p.rect_filled(strip, 0.0, lerp_color(BLACK_PANEL, BTN_FACE, 0.08));
    p.line_segment(
        [Pos2::new(rect.left() + LSTRIP_W, panel_top), Pos2::new(rect.left() + LSTRIP_W, rect.bottom())],
        Stroke::new(1.0, lerp_color(SILVER_SHADOW, INK, 0.4)),
    );

    let font_sm = egui::FontId::new(7.0, egui::FontFamily::Monospace);

    // ── RAND (randomize pattern) ──────────────────────────────────────
    let rand_r = Rect::from_min_size(Pos2::new(lx, panel_top + 5.0), Vec2::new(68.0, 13.0));
    let rand_resp = ui
        .interact(rand_r, egui::Id::new("sqb_rand"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(
            "Randomize — generate a fresh acid pattern.\n\
             Minor pentatonic, dense, with accents and slides.\n\
             Shortcut: ` (backtick)",
        );
    let p = ui.painter();
    p.rect_filled(rand_r.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
    p.rect_filled(rand_r, 2.0, BTN_FACE);
    p.rect_stroke(rand_r, 2.0, Stroke::new(0.7, SILVER_SHADOW), egui::StrokeKind::Inside);
    p.text(rand_r.center(), egui::Align2::CENTER_CENTER, "↺ RANDOMIZE",
        egui::FontId::new(7.0, egui::FontFamily::Monospace), LABEL_FG);
    if rand_resp.clicked() {
        randomize_pattern(ui.ctx(), kbd);
    }

    // ── CLEAR (all rests) ─────────────────────────────────────────────
    let clr_r = Rect::from_min_size(Pos2::new(lx, panel_top + 22.0), Vec2::new(68.0, 13.0));
    let clr_resp = ui
        .interact(clr_r, egui::Id::new("sqb_clear"), egui::Sense::click())
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

    // ── SHIFT L / SHIFT R (rotate pattern) ────────────────────────────
    p.text(Pos2::new(lx, panel_top + 42.0), egui::Align2::LEFT_TOP,
        "SHIFT PATTERN", font_sm.clone(), BTN_LBL);
    let shl_r = Rect::from_min_size(Pos2::new(lx, panel_top + 52.0), Vec2::new(32.0, 13.0));
    let shr_r = Rect::from_min_size(Pos2::new(lx + 36.0, panel_top + 52.0), Vec2::new(32.0, 13.0));
    let shl_resp = ui.interact(shl_r, egui::Id::new("sqb_shl"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Shift left — rotate the pattern one step earlier.\nShortcut: [");
    let shr_resp = ui.interact(shr_r, egui::Id::new("sqb_shr"), egui::Sense::click())
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

    // ── Waveform toggle (SAW / SQR) ───────────────────────────────────
    let wf = params.waveform.value();
    let saw_r = Rect::from_min_size(Pos2::new(lx, panel_top + 80.0), Vec2::new(32.0, 16.0));
    let sqr_r = Rect::from_min_size(Pos2::new(lx + 35.0, panel_top + 80.0), Vec2::new(33.0, 16.0));
    p.text(Pos2::new(lx + 34.0, panel_top + 77.0), egui::Align2::CENTER_BOTTOM,
        "WAVEFORM", egui::FontId::new(6.5, egui::FontFamily::Monospace), BTN_LBL);
    draw_wave_button(ui, setter, &params.waveform, saw_r, egui::Id::new("sqb_saw"), "SAW",
        wf == WaveformParam::Saw, WaveformParam::Saw);
    draw_wave_button(ui, setter, &params.waveform, sqr_r, egui::Id::new("sqb_sqr"), "SQR",
        wf == WaveformParam::Square, WaveformParam::Square);

    // ── RUN / STOP (interactive) ──────────────────────────────────────
    let run_rect = Rect::from_min_size(Pos2::new(lx, panel_top + 102.0), Vec2::new(68.0, 22.0));
    let running = kbd.is_seq_running();
    let run_resp = ui.interact(run_rect, egui::Id::new("sqb_runstop"), egui::Sense::click())
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

// Chromatic pitch buttons row
fn draw_pitch_buttons(ui: &egui::Ui, rect: Rect) {
    const NOTES: [&str; 13]  = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B", "C"];
    const SHARP: [bool; 13]  = [false, true, false, true, false, false, true, false, true, false, true, false, false];
    let p = ui.painter();
    let top = rect.top();

    p.text(
        Pos2::new(rect.left() + STEP_X0 + 1.0, top + PITCH_Y),
        egui::Align2::LEFT_TOP,
        "PITCH MODE",
        egui::FontId::new(7.0, egui::FontFamily::Monospace),
        BTN_LBL,
    );

    let btn_area_w = STEP_X1 - STEP_X0;
    let btn_w = btn_area_w / 13.0;
    let btn_top = top + PITCH_Y + 10.0;
    let btn_h = PITCH_H - 10.0;

    for (i, (&note, &is_sharp)) in NOTES.iter().zip(SHARP.iter()).enumerate() {
        let bx = rect.left() + STEP_X0 + i as f32 * btn_w;
        let (bg, fg) = if is_sharp {
            (INK, BTN_LBL)
        } else {
            (SILVER_MID, INK)
        };
        let raised = if is_sharp { 0.0 } else { 2.0 };
        let br = Rect::from_min_size(
            Pos2::new(bx + 1.0, btn_top + raised),
            Vec2::new(btn_w - 2.0, btn_h - raised),
        );
        p.rect_filled(br, 1.5, bg);
        p.rect_stroke(br, 1.5, Stroke::new(0.7, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(br.center(), egui::Align2::CENTER_CENTER, note,
            egui::FontId::new(6.0, egui::FontFamily::Monospace), fg);
    }
}

// 16-step slider display
fn draw_step_area(ui: &mut egui::Ui, kbd: &KbdQueue, rect: Rect) {
    let top = rect.top();
    let mut pattern = kbd.pattern_snapshot();
    let running = kbd.is_seq_running();
    let playhead = (kbd.current_step() % pattern.length.max(1) as u64) as usize;
    let selected = kbd.selected_step();

    let slider_h = SLD_Y1 - SLD_Y0;
    // C1 (24) to C4 (60) = 3 octaves — authentic 303 range.
    const SEMI_LO: f32 = 24.0;
    const SEMI_HI: f32 = 60.0;
    let semi_range = SEMI_HI - SEMI_LO;
    let mut pattern_dirty = false;

    // ─── Input pass: per-step hit-rects ───
    // The top strip above the slider is reserved for three small
    // toggles (A=accent, S=slide, R=rest), so we shrink the slider
    // cell to start AT SLD_Y0 to keep them from overlapping.
    // Body of the cell: left-click + drag set pitch, right-click
    // toggles rest, any interaction selects the step. Keyboard
    // shortcuts (handled below) operate on the selection.
    for i in 0..16 {
        let cx = rect.left() + STEP_X0 + (i as f32 + 0.5) * STEP_CELL;
        let note_name = midi_note_name(pattern.steps[i].semitone);
        let s_flags = pattern.steps[i];
        let flags = {
            let mut parts: Vec<&str> = Vec::new();
            if s_flags.rest { parts.push("REST"); }
            if s_flags.accent { parts.push("ACC"); }
            if s_flags.slide { parts.push("SLD"); }
            if parts.is_empty() { String::new() } else { format!(" · {}", parts.join("+")) }
        };
        let tip = format!(
            "Step {}: {}{}\n\
             • Click/drag body: select + set pitch\n\
             • Right-click body: toggle rest\n\
             • A / S / R buttons above: toggle accent/slide/rest\n\
             • Keyboard (while selected):\n  \
               Up/Down pitch ±1 (Shift ±12)\n  \
               Left/Right move selection\n  \
               A accent  S slide  R rest\n  \
               Esc deselect",
            i + 1, note_name, flags
        );

        // ── A/S/R mini-toggles above the slider ──
        // Three 9-px-wide hit-rects centered across the cell.
        let toggle_y = top + SLD_Y0 - 14.0;
        let toggle_h = 10.0;
        let toggle_w = 9.0;
        let gap = 1.0;
        let total_w = toggle_w * 3.0 + gap * 2.0;
        let row_x = cx - total_w * 0.5;
        let acc_r = Rect::from_min_size(Pos2::new(row_x, toggle_y), Vec2::new(toggle_w, toggle_h));
        let sld_r = Rect::from_min_size(Pos2::new(row_x + (toggle_w + gap), toggle_y), Vec2::new(toggle_w, toggle_h));
        let rst_r = Rect::from_min_size(Pos2::new(row_x + 2.0 * (toggle_w + gap), toggle_y), Vec2::new(toggle_w, toggle_h));

        let acc_resp = ui.interact(acc_r, egui::Id::new(("sqb_step_acc", i)), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Accent — toggle accent on this step.");
        let sld_resp = ui.interact(sld_r, egui::Id::new(("sqb_step_sld", i)), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Slide — glide INTO the next step from this one.");
        let rst_resp = ui.interact(rst_r, egui::Id::new(("sqb_step_rst", i)), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Rest — silence this step.");

        if acc_resp.clicked() {
            pattern.steps[i].rest = false;
            pattern.steps[i].accent = !pattern.steps[i].accent;
            pattern_dirty = true;
            kbd.set_selected_step(i);
        }
        if sld_resp.clicked() {
            pattern.steps[i].rest = false;
            pattern.steps[i].slide = !pattern.steps[i].slide;
            pattern_dirty = true;
            kbd.set_selected_step(i);
        }
        if rst_resp.clicked() {
            pattern.steps[i].rest = !pattern.steps[i].rest;
            pattern_dirty = true;
            kbd.set_selected_step(i);
        }

        // ── Cell body: pitch drag + right-click rest ──
        let cell = Rect::from_min_max(
            Pos2::new(cx - STEP_CELL * 0.5, top + SLD_Y0),
            Pos2::new(cx + STEP_CELL * 0.5, top + SLD_Y1 + 4.0),
        );
        let resp = ui
            .interact(
                cell,
                egui::Id::new(("sqb_step_cell", i)),
                egui::Sense::click_and_drag(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(tip);

        if resp.clicked_by(egui::PointerButton::Secondary) {
            pattern.steps[i].rest = !pattern.steps[i].rest;
            pattern_dirty = true;
            kbd.set_selected_step(i);
        } else if resp.dragged() || resp.drag_started() || resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let t = ((pos.y - (top + SLD_Y0)) / slider_h).clamp(0.0, 1.0);
                let semi = (SEMI_HI - t * semi_range).round() as i32;
                pattern.steps[i].semitone = semi.clamp(SEMI_LO as i32, SEMI_HI as i32) as u8;
                pattern.steps[i].rest = false;
                pattern_dirty = true;
                kbd.set_selected_step(i);
            }
        }
    }

    // ─── Keyboard-driven edits on the selected step ───
    if let Some(sel) = selected {
        if sel < 16 {
            let t_held: bool = ui.ctx().data(|d| d.get_temp(egui::Id::new("sqb_t_held"))).unwrap_or(false);
            let (dp, nav, toggle_a, toggle_s, toggle_r, esc) = ui.input(|ip| {
                let shift = ip.modifiers.shift;
                let big = if shift { 12 } else { 1 };
                let mut dp = 0i32;
                let mut nav = 0i32;
                if ip.key_pressed(egui::Key::ArrowUp)    { dp += big; }
                if ip.key_pressed(egui::Key::ArrowDown)  { dp -= big; }
                if ip.key_pressed(egui::Key::ArrowLeft)  { nav -= 1; }
                if ip.key_pressed(egui::Key::ArrowRight) { nav += 1; }
                (
                    dp, nav,
                    ip.key_pressed(egui::Key::A),
                    ip.key_pressed(egui::Key::S),
                    ip.key_pressed(egui::Key::R) || ip.key_pressed(egui::Key::Delete) || ip.key_pressed(egui::Key::Backspace),
                    ip.key_pressed(egui::Key::Escape),
                )
            });
            if dp != 0 {
                // If the step is a rest, start from middle C2 (36)
                let cur = if pattern.steps[sel].rest { 36 } else { pattern.steps[sel].semitone as i32 };
                let next = (cur + dp).clamp(SEMI_LO as i32, SEMI_HI as i32);
                pattern.steps[sel].semitone = next as u8;
                pattern.steps[sel].rest = false;
                pattern_dirty = true;
                // T held + pitch change: auto-audition the new note
                if t_held {
                    let velocity = if pattern.steps[sel].accent { 0.95 } else { 0.7 };
                    kbd.push(KbdEvent { on: true, note: next as u8, velocity });
                }
            }
            if nav != 0 {
                let len = pattern.length.max(1) as i32;
                let next = ((sel as i32 + nav).rem_euclid(len)) as usize;
                kbd.set_selected_step(next);
            }
            if toggle_a {
                pattern.steps[sel].rest = false;
                pattern.steps[sel].accent = !pattern.steps[sel].accent;
                pattern_dirty = true;
            }
            if toggle_s {
                pattern.steps[sel].rest = false;
                pattern.steps[sel].slide = !pattern.steps[sel].slide;
                pattern_dirty = true;
            }
            if toggle_r {
                pattern.steps[sel].rest = !pattern.steps[sel].rest;
                pattern_dirty = true;
            }
            if esc {
                kbd.clear_selected_step();
            }
        } else {
            kbd.clear_selected_step();
        }
    }

    if pattern_dirty {
        let pat_clone = pattern.clone();
        kbd.edit_pattern(move |p| *p = pat_clone);
    }

    // ─── Draw pass ───
    let p = ui.painter();

    // Background plate behind all sliders
    let plate = Rect::from_min_max(
        Pos2::new(rect.left() + STEP_X0, top + SLD_Y0 - 4.0),
        Pos2::new(rect.left() + STEP_X1, top + SLD_Y1 + 4.0),
    );
    p.rect_filled(plate.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
    p.rect_filled(plate, 2.0, Color32::from_rgb(12, 12, 14));
    p.rect_stroke(plate, 2.0, Stroke::new(0.8, SILVER_SHADOW), egui::StrokeKind::Inside);

    let pat_len = pattern.length.max(1) as usize;
    for i in 0..16 {
        let cx = rect.left() + STEP_X0 + (i as f32 + 0.5) * STEP_CELL;
        let step = pattern.steps[i];
        let inactive = i >= pat_len;

        // ── A / S / R toggle row above the slider ──
        // Geometry must match the input pass exactly.
        let toggle_y = top + SLD_Y0 - 14.0;
        let toggle_h = 10.0;
        let toggle_w = 9.0;
        let gap = 1.0;
        let total_w = toggle_w * 3.0 + gap * 2.0;
        let row_x = cx - total_w * 0.5;
        let acc_r = Rect::from_min_size(Pos2::new(row_x, toggle_y), Vec2::new(toggle_w, toggle_h));
        let sld_r = Rect::from_min_size(Pos2::new(row_x + (toggle_w + gap), toggle_y), Vec2::new(toggle_w, toggle_h));
        let rst_r = Rect::from_min_size(Pos2::new(row_x + 2.0 * (toggle_w + gap), toggle_y), Vec2::new(toggle_w, toggle_h));
        let lbl_font = egui::FontId::new(7.0, egui::FontFamily::Monospace);

        let acc_active = !step.rest && step.accent;
        p.rect_filled(acc_r, 1.0, if acc_active { RED } else { BTN_FACE });
        p.rect_stroke(acc_r, 1.0, Stroke::new(0.5, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(acc_r.center(), egui::Align2::CENTER_CENTER, "A", lbl_font.clone(),
            if acc_active { Color32::WHITE } else { BTN_LBL });

        let sld_active = !step.rest && step.slide;
        p.rect_filled(sld_r, 1.0, if sld_active { YELLOW } else { BTN_FACE });
        p.rect_stroke(sld_r, 1.0, Stroke::new(0.5, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(sld_r.center(), egui::Align2::CENTER_CENTER, "S", lbl_font.clone(),
            if sld_active { INK } else { BTN_LBL });

        let rst_active = step.rest;
        p.rect_filled(rst_r, 1.0, if rst_active { SILVER_LIGHT } else { BTN_FACE });
        p.rect_stroke(rst_r, 1.0, Stroke::new(0.5, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(rst_r.center(), egui::Align2::CENTER_CENTER, "R", lbl_font,
            if rst_active { INK } else { BTN_LBL });

        // Slider track (vertical inset)
        let track_x = cx - 2.0;
        let track = Rect::from_min_size(Pos2::new(track_x, top + SLD_Y0), Vec2::new(4.0, slider_h));
        p.rect_filled(track, 1.5, INSET);
        p.rect_stroke(track, 1.5, Stroke::new(0.5, INK), egui::StrokeKind::Inside);

        if !step.rest {
            // Map semitone (36=C2..72=C5) to slider thumb position
            let semi = (step.semitone as f32).clamp(SEMI_LO, SEMI_HI);
            let t = 1.0 - (semi - SEMI_LO) / semi_range;
            let thumb_y = top + SLD_Y0 + t * slider_h;
            let thumb = Rect::from_center_size(Pos2::new(cx, thumb_y), Vec2::new(STEP_CELL - 6.0, 7.0));
            p.rect_filled(thumb.translate(Vec2::new(0.0, 1.0)), 2.0, SILVER_SHADOW);
            p.rect_filled(thumb, 2.0, if step.accent { RED_DARK } else { SILVER_MID });
            p.rect_stroke(thumb, 2.0, Stroke::new(0.8, SILVER_LIGHT), egui::StrokeKind::Outside);
        }

        // Beat group divider at every 4th step
        if i % 4 == 0 && i > 0 {
            p.line_segment(
                [Pos2::new(cx - STEP_CELL * 0.5, top + SLD_Y0 - 2.0),
                 Pos2::new(cx - STEP_CELL * 0.5, top + SLD_Y1 + 2.0)],
                Stroke::new(0.8, Color32::from_rgba_unmultiplied(255, 255, 255, 20)),
            );
        }

        // Single step indicator — never two rings at once.
        // Running: white ring on the playing step.
        // Stopped: red ring on the selected step (or playhead if no selection).
        if running && i == playhead {
            let ph = Rect::from_min_size(
                Pos2::new(cx - STEP_CELL * 0.5 + 1.0, top + SLD_Y0 - 12.0),
                Vec2::new(STEP_CELL - 2.0, slider_h + 16.0),
            );
            p.rect_stroke(ph, 2.0, Stroke::new(1.8, Color32::from_rgb(240, 240, 255)), egui::StrokeKind::Outside);
            p.line_segment(
                [Pos2::new(ph.left(), ph.top() - 1.0), Pos2::new(ph.right(), ph.top() - 1.0)],
                Stroke::new(1.2, Color32::WHITE),
            );
        } else if !running {
            let stopped_pos = selected.unwrap_or(playhead);
            if i == stopped_pos {
                let sr = Rect::from_min_size(
                    Pos2::new(cx - STEP_CELL * 0.5 + 1.0, top + SLD_Y0 - 12.0),
                    Vec2::new(STEP_CELL - 2.0, slider_h + 16.0),
                );
                p.rect_stroke(sr, 2.0, Stroke::new(1.5, Color32::from_rgb(180, 60, 60)), egui::StrokeKind::Outside);
            }
        }

        // Step number
        p.text(
            Pos2::new(cx, top + STEP_NUM_Y),
            egui::Align2::CENTER_CENTER,
            format!("{}", i + 1),
            egui::FontId::new(7.5, egui::FontFamily::Monospace),
            if inactive { Color32::from_rgb(60, 60, 64) }
            else if i % 4 == 0 { LABEL_FG } else { BTN_LBL },
        );

        // Dim inactive steps (index >= pattern.length): translucent
        // black overlay covering the whole cell column. Drawn last so
        // it sits on top of the slider/toggles, but the playhead /
        // selection rings remain visible because they're drawn after
        // this loop... actually they're drawn above. Use a soft alpha
        // so the user can still read the muted notes underneath.
        if inactive {
            let overlay = Rect::from_min_max(
                Pos2::new(cx - STEP_CELL * 0.5, top + SLD_Y0 - 16.0),
                Pos2::new(cx + STEP_CELL * 0.5, top + SLD_Y1 + 6.0),
            );
            p.rect_filled(overlay, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 170));
        }
    }

    // Selector label
    p.text(
        Pos2::new(rect.left() + STEP_X0, top + STEP_NUM_Y),
        egui::Align2::RIGHT_CENTER,
        "SEL",
        egui::FontId::new(7.0, egui::FontFamily::Monospace),
        BTN_LBL,
    );
}

// Transpose + TIME MODE section
fn draw_transpose_section(ui: &mut egui::Ui, kbd: &KbdQueue, rect: Rect) {
    let top = rect.top();
    let x0 = rect.left() + TR_X;
    let font = egui::FontId::new(7.0, egui::FontFamily::Monospace);
    let panel_top = top + PANEL_SPLIT;

    let shift_held = ui.input(|i| i.modifiers.shift);

    // ── TRANSPOSE label + DN/UP ──
    ui.painter().text(Pos2::new(x0 + 32.0, panel_top + 6.0), egui::Align2::CENTER_TOP,
        "TRANSPOSE", font.clone(), BTN_LBL);
    let dn_r = Rect::from_min_size(Pos2::new(x0, panel_top + 17.0), Vec2::new(30.0, 13.0));
    let up_r = Rect::from_min_size(Pos2::new(x0 + 34.0, panel_top + 17.0), Vec2::new(30.0, 13.0));
    let dn_resp = ui.interact(dn_r, egui::Id::new("sqb_tr_dn"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Transpose down — every non-rest step −1 semitone.\nShift+click: −1 octave.");
    let up_resp = ui.interact(up_r, egui::Id::new("sqb_tr_up"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Transpose up — every non-rest step +1 semitone.\nShift+click: +1 octave.");
    paint_btn(ui.painter(), dn_r, "▼ DN", false);
    paint_btn(ui.painter(), up_r, "▲ UP", false);
    if dn_resp.clicked() {
        let d = if shift_held { -12 } else { -1 };
        kbd.edit_pattern(|p| transpose_pattern(p, d));
    }
    if up_resp.clicked() {
        let d = if shift_held { 12 } else { 1 };
        kbd.edit_pattern(|p| transpose_pattern(p, d));
    }

    // ── DEL / INS ──
    let del_r = Rect::from_min_size(Pos2::new(x0, panel_top + 35.0), Vec2::new(30.0, 13.0));
    let ins_r = Rect::from_min_size(Pos2::new(x0 + 34.0, panel_top + 35.0), Vec2::new(30.0, 13.0));
    let del_resp = ui.interact(del_r, egui::Id::new("sqb_del"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Delete — turn the selected step into a rest.\n(If no step is selected, acts on step 1.)");
    let ins_resp = ui.interact(ins_r, egui::Id::new("sqb_ins"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Insert — un-rest the selected step (audible note).\n(If no step is selected, acts on step 1.)");
    paint_btn(ui.painter(), del_r, "DEL", false);
    paint_btn(ui.painter(), ins_r, "INS", false);
    let sel = kbd.selected_step().unwrap_or(0);
    if del_resp.clicked() {
        kbd.edit_pattern(|p| { p.steps[sel].rest = true; });
        kbd.set_selected_step(sel);
    }
    if ins_resp.clicked() {
        kbd.edit_pattern(|p| {
            p.steps[sel].rest = false;
            if p.steps[sel].semitone > 60 || p.steps[sel].semitone < 24 { p.steps[sel].semitone = 36; }
        });
        kbd.set_selected_step(sel);
    }

    // ── TIME MODE label + ACCENT / SLIDE toggles for selected step ──
    ui.painter().text(Pos2::new(x0 + 32.0, panel_top + 53.0), egui::Align2::CENTER_TOP,
        "TIME MODE", font.clone(), BTN_LBL);
    let snapshot = kbd.pattern_snapshot();
    let acc_active = snapshot.steps[sel].accent && !snapshot.steps[sel].rest;
    let sld_active = snapshot.steps[sel].slide && !snapshot.steps[sel].rest;
    let acc_r = Rect::from_min_size(Pos2::new(x0, panel_top + 64.0), Vec2::new(64.0, 14.0));
    let sld_r = Rect::from_min_size(Pos2::new(x0, panel_top + 82.0), Vec2::new(64.0, 14.0));
    let acc_resp = ui.interact(acc_r, egui::Id::new("sqb_tm_acc"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Accent — toggle the accent flag on the selected step.\nShortcut (with step selected): A");
    let sld_resp = ui.interact(sld_r, egui::Id::new("sqb_tm_sld"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Slide — toggle the slide flag on the selected step.\nShortcut (with step selected): S");
    paint_btn(ui.painter(), acc_r, "ACCENT", acc_active);
    paint_btn(ui.painter(), sld_r, "SLIDE", sld_active);
    if acc_resp.clicked() {
        kbd.edit_pattern(|p| {
            p.steps[sel].rest = false;
            p.steps[sel].accent = !p.steps[sel].accent;
        });
        kbd.set_selected_step(sel);
    }
    if sld_resp.clicked() {
        kbd.edit_pattern(|p| {
            p.steps[sel].rest = false;
            p.steps[sel].slide = !p.steps[sel].slide;
        });
        kbd.set_selected_step(sel);
    }

    // Indicator dots — one per group of 4 steps (4 dots total).
    // We compute the beat from a *look-ahead* fractional position so
    // the LED transition compensates for one process-block of audio
    // latency + ~one egui frame of GUI latency. ~0.30 of a step = a
    // bit under 40 ms at 120 BPM, which is roughly the perceived
    // visual lag and lines the LED change up with the audible
    // downbeat.
    const LED_LOOKAHEAD: f32 = 0.30;
    let raw_pos = kbd.current_step() as f32 + kbd.step_phase() + LED_LOOKAHEAD;
    let beat = ((raw_pos as u64 / 4) % 4) as usize;
    let p = ui.painter();
    let row_left = x0 + 8.0;
    let row_right = x0 + 56.0;
    let span = row_right - row_left;
    let gap = span / 3.0;
    for j in 0..4 {
        let dot_x = row_left + j as f32 * gap;
        let lit = j == beat && kbd.is_seq_running();
        p.circle_filled(Pos2::new(dot_x, panel_top + 103.0),
            3.5, if lit { RED } else { BTN_FACE });
        p.circle_stroke(Pos2::new(dot_x, panel_top + 103.0), 3.5, Stroke::new(0.6, SILVER_SHADOW));
    }
}

/// Apply a semitone transpose to every non-rest step in the pattern,
/// clamped to MIDI range. Used by the TRANSPOSE DN/UP buttons.
fn transpose_pattern(p: &mut crate::sequencer::Pattern, delta: i32) {
    for s in p.steps.iter_mut() {
        if s.rest { continue; }
        let next = (s.semitone as i32 + delta).clamp(24, 60);
        s.semitone = next as u8;
    }
}

/// Paint a button face + label. Used by the transpose / time-mode rows.
fn paint_btn(p: &egui::Painter, r: Rect, label: &str, active: bool) {
    p.rect_filled(r.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
    p.rect_filled(r, 2.0, if active { RED } else { BTN_FACE });
    p.rect_stroke(r, 2.0, Stroke::new(0.7, SILVER_SHADOW), egui::StrokeKind::Inside);
    p.text(r.center(), egui::Align2::CENTER_CENTER, label,
        egui::FontId::new(7.5, egui::FontFamily::Monospace),
        if active { Color32::WHITE } else { BTN_LBL });
}

// Right column: BACK / STEP / WRITE/NEXT / TAP
fn draw_right_strip(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    params: &SquelchBoxParams,
    kbd: &KbdQueue,
    rect: Rect,
) {
    let top = rect.top();
    let x0 = rect.left() + RSTRIP_X;
    let panel_top = top + PANEL_SPLIT;
    let strip = Rect::from_min_size(Pos2::new(x0, panel_top), Vec2::new(rect.right() - x0, BASE_H as f32 - PANEL_SPLIT));
    {
        let p = ui.painter();
        p.rect_filled(strip, 0.0, lerp_color(BLACK_PANEL, BTN_FACE, 0.06));
        p.line_segment(
            [Pos2::new(x0, panel_top), Pos2::new(x0, rect.bottom())],
            Stroke::new(1.0, lerp_color(SILVER_SHADOW, INK, 0.4)),
        );
    }

    let btn_w = rect.right() - x0 - 10.0;

    // ── BACK (move selection backward) ──
    let back_r = Rect::from_min_size(Pos2::new(x0 + 5.0, panel_top + 5.0), Vec2::new(btn_w, 18.0));
    let back_resp = ui.interact(back_r, egui::Id::new("sqb_back"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Back — move the editing selection backward by one step.\nWhile stopped, also auditions the step.");
    paint_right_btn(ui.painter(), back_r, "◀ BACK", false);
    if back_resp.clicked() && !kbd.is_seq_running() {
        let pat = kbd.pattern_snapshot();
        let len = pat.length.max(1) as usize;
        // Derive position from selection if active, otherwise from playhead
        let cur = kbd.selected_step()
            .unwrap_or_else(|| (kbd.current_step() % len as u64) as usize);
        let prev = if cur == 0 { len - 1 } else { cur - 1 };
        kbd.set_selected_step(prev);
        kbd.set_current_step(prev as u64);
        let s = pat.steps[prev];
        if !s.rest {
            let velocity = if s.accent { 0.95 } else { 0.7 };
            kbd.push(crate::kbd::KbdEvent { on: true, note: s.semitone, velocity });
        }
    }

    // ── STEP (single-step audition while stopped) ──
    let step_r = Rect::from_min_size(Pos2::new(x0 + 5.0, panel_top + 30.0), Vec2::new(btn_w, 20.0));
    let step_resp = ui.interact(step_r, egui::Id::new("sqb_step"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Step — advance the playhead one step and audition the note.\nWorks while the sequencer is stopped.");
    paint_right_btn(ui.painter(), step_r, "STEP", false);
    if step_resp.clicked() && !kbd.is_seq_running() {
        single_step_audition(kbd);
    }

    // ── WRITE / NEXT (move selection forward) ──
    let wn_r = Rect::from_min_size(Pos2::new(x0 + 5.0, panel_top + 58.0), Vec2::new(btn_w, 20.0));
    let wn_resp = ui.interact(wn_r, egui::Id::new("sqb_writenext"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Write / Next — move the editing selection forward by one step.");
    paint_right_btn(ui.painter(), wn_r, "WRITE/NEXT", false);
    if wn_resp.clicked() && !kbd.is_seq_running() {
        let pat = kbd.pattern_snapshot();
        let len = pat.length.max(1) as usize;
        let cur = kbd.selected_step()
            .unwrap_or_else(|| (kbd.current_step() % len as u64) as usize);
        let next = (cur + 1) % len;
        kbd.set_selected_step(next);
        kbd.set_current_step(next as u64);
        let s = pat.steps[next];
        if !s.rest {
            let velocity = if s.accent { 0.95 } else { 0.7 };
            kbd.push(crate::kbd::KbdEvent { on: true, note: s.semitone, velocity });
        }
    }

    // ── TAP (tap tempo) ──
    let tap_r = Rect::from_min_size(Pos2::new(x0 + 5.0, panel_top + 86.0), Vec2::new(btn_w, 20.0));
    let tap_resp = ui.interact(tap_r, egui::Id::new("sqb_tap"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Tap tempo — tap 2+ times in rhythm to set BPM.\nTaps separated by more than 2s reset the streak.");
    paint_right_btn(ui.painter(), tap_r, "TAP", false);
    if tap_resp.clicked() {
        handle_tap_tempo(ui.ctx(), setter, &params.seq_bpm);
    }

    // ── DUMP MIDI (export current pattern as .mid) ──
    let dump_r = Rect::from_min_size(Pos2::new(x0 + 5.0, panel_top + 114.0), Vec2::new(btn_w, 20.0));
    let dump_resp = ui.interact(dump_r, egui::Id::new("sqb_dump_midi"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(
            "Dump MIDI — export the current pattern as a .mid file.\n\
             Saved to ~/.local/share/squelchbox/exports/.\n\
             A companion Renoise tool can pick it up from there."
        );
    paint_right_btn(ui.painter(), dump_r, "DUMP MIDI", false);
    if dump_resp.clicked() {
        let snap = kbd.pattern_snapshot();
        let bpm = params.seq_bpm.unmodulated_plain_value();
        match crate::util::midi_export::export_pattern(&snap, bpm) {
            Ok(path) => {
                tracing::info!("MIDI export: {}", path.display());
                let msg = format!("Exported: {}", path.display());
                set_toast(ui.ctx(), msg, path);
            }
            Err(e) => {
                tracing::warn!("MIDI export failed: {e}");
                set_toast(ui.ctx(), format!("Export failed: {e}"), PathBuf::new());
            }
        }
    }
}

fn paint_right_btn(p: &egui::Painter, r: Rect, label: &str, active: bool) {
    p.rect_filled(r.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
    p.rect_filled(r, 2.0, if active { RED } else { BTN_FACE });
    p.rect_stroke(r, 2.0, Stroke::new(0.8, SILVER_SHADOW), egui::StrokeKind::Inside);
    p.text(r.center(), egui::Align2::CENTER_CENTER, label,
        egui::FontId::new(8.5, egui::FontFamily::Monospace),
        if active { Color32::WHITE } else { BTN_LBL });
}

/// STEP button: select the current step and audition it without
/// advancing. Use NEXT/BACK to move, STEP to listen.
fn single_step_audition(kbd: &KbdQueue) {
    let pat = kbd.pattern_snapshot();
    let len = pat.length.max(1) as usize;
    let idx = kbd.selected_step()
        .unwrap_or_else(|| (kbd.current_step() % len as u64) as usize);
    kbd.set_selected_step(idx);
    kbd.set_current_step(idx as u64);
    let s = pat.steps[idx];
    if !s.rest {
        let velocity = if s.accent { 0.95 } else { 0.7 };
        kbd.push(crate::kbd::KbdEvent { on: true, note: s.semitone, velocity });
    }
}


/// TAP button: store recent tap timestamps in egui temp data, average
/// the intervals (rejecting any > 2s as a streak break), and write the
/// resulting BPM to the seq_bpm parameter via the standard
/// begin/set/end dance.
fn handle_tap_tempo(ctx: &egui::Context, setter: &ParamSetter, bpm: &nih_plug::params::FloatParam) {
    const MAX_GAP: f64 = 2.0;
    const HISTORY: usize = 4;
    let id = egui::Id::new("sqb_tap_history");
    let now = ctx.input(|i| i.time);
    let mut taps: Vec<f64> = ctx.data(|d| d.get_temp::<Vec<f64>>(id)).unwrap_or_default();
    if let Some(&last) = taps.last() {
        if now - last > MAX_GAP {
            taps.clear();
        }
    }
    taps.push(now);
    while taps.len() > HISTORY { taps.remove(0); }
    if taps.len() >= 2 {
        let mut sum = 0.0f64;
        for w in taps.windows(2) {
            sum += w[1] - w[0];
        }
        let avg = sum / (taps.len() - 1) as f64;
        if avg > 0.0 {
            let bpm_val = (60.0 / avg).clamp(40.0, 220.0) as f32;
            setter.begin_set_parameter(bpm);
            setter.set_parameter(bpm, bpm_val);
            setter.end_set_parameter(bpm);
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, taps));
}

// ─── Utilities ────────────────────────────────────────────────────────

fn small_btn(p: &egui::Painter, x: f32, y: f32, w: f32, h: f32, label: &str) {
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    p.rect_filled(r.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
    p.rect_filled(r, 2.0, BTN_FACE);
    p.rect_stroke(r, 2.0, Stroke::new(0.7, SILVER_SHADOW), egui::StrokeKind::Inside);
    p.text(r.center(), egui::Align2::CENTER_CENTER, label,
        egui::FontId::new(6.5, egui::FontFamily::Monospace), BTN_LBL);
}

fn draw_wave_button(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    param: &EnumParam<WaveformParam>,
    rect: Rect,
    id: egui::Id,
    label: &str,
    active: bool,
    target: WaveformParam,
) {
    let tip = match target {
        WaveformParam::Saw => "Saw wave — brighter, classic acid timbre.",
        WaveformParam::Square => "Square wave — hollow, rubbery sub-bass timbre.",
    };
    let resp = ui.interact(rect, id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tip);
    let p = ui.painter_at(rect);
    let (bg, fg) = if active { (RED, Color32::WHITE) } else { (BTN_FACE, BTN_LBL) };
    p.rect_filled(rect.translate(Vec2::new(0.0, 1.0)), 2.0, INK);
    p.rect_filled(rect, 2.0, bg);
    p.rect_stroke(rect, 2.0, Stroke::new(0.8, SILVER_SHADOW), egui::StrokeKind::Inside);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, label,
        egui::FontId::new(9.0, egui::FontFamily::Monospace), fg);
    if resp.clicked() && !active {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, target);
        setter.end_set_parameter(param);
    }
}

// ─── Knob widget ──────────────────────────────────────────────────────

/// Interactive knob for a `nih_plug` parameter.
/// `label_chip`: when true draw the dark label chip below; when false skip it
/// (caller renders label separately as panel text).
fn param_knob<P: Param>(
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
    let resp = ui.interact(rect, id, egui::Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::ResizeVertical);

    let mut norm = param.unmodulated_normalized_value();
    if resp.drag_started() { setter.begin_set_parameter(param); }
    if resp.dragged() {
        let dy = -resp.drag_delta().y;
        let speed = if ui.input(|i| i.modifiers.shift) { 0.0015 } else { 0.0065 };
        norm = (norm + dy * speed).clamp(0.0, 1.0);
        setter.set_parameter_normalized(param, norm);
    }
    if resp.drag_stopped() { setter.end_set_parameter(param); }
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
    p.circle_filled(center - Vec2::new(0.0, radius * 0.12), radius * 0.96, Color32::from_rgb(44, 44, 48));
    p.circle_filled(center, radius * 0.86, KNOB_RING);
    // Metal core
    let cr = radius * 0.58;
    p.circle_filled(center, cr + 1.8, Color32::from_rgb(140, 140, 146));
    p.circle_filled(center, cr, KNOB_CORE);
    p.circle_filled(center - Vec2::new(cr * 0.2, cr * 0.22), cr * 0.55, Color32::from_rgb(80, 80, 86));
    p.circle_filled(center, cr * 0.1, Color32::BLACK);
    // Indicator line
    let start = std::f32::consts::PI * 0.75;
    let sweep = std::f32::consts::PI * 1.5;
    let angle = start + sweep * norm;
    let (s, c) = angle.sin_cos();
    let dir = Vec2::new(c, s);
    p.line_segment([center + dir * (cr * 0.2), center + dir * (cr * 0.88)], Stroke::new(2.2, INDICATOR));
    // Tick marks
    for i in 0..=10 {
        let t = i as f32 / 10.0;
        let a = start + sweep * t;
        let (sn, cs) = a.sin_cos();
        let d = Vec2::new(cs, sn);
        let major = i % 5 == 0;
        p.line_segment(
            [center + d * (radius + 2.0), center + d * (radius + if major { 6.0 } else { 4.0 })],
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
        p.text(chip.center(), egui::Align2::CENTER_CENTER, label,
            egui::FontId::new(11.0, egui::FontFamily::Monospace), LABEL_FG);
    }
    // Stash display value
    if resp.hovered() || resp.dragged() {
        let plain: f32 = param.unmodulated_plain_value().into();
        let s = if !label.is_empty() {
            format!("{label} {}", format_value(plain))
        } else {
            format_value(plain)
        };
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("sqb_display"), s));
    }
    resp
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let lerp = |x: u8, y: u8| -> u8 {
        (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}
