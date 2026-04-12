//! Band 2: TEMPO / SLIDE / MODE section / BANK / LEN / OCT / VOLUME / BPM text entry.

use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

use crate::kbd::KbdQueue;
use crate::params::{SquelchBoxParams, SyncMode};
use crate::ui::ids;
use crate::ui::palette::*;
use crate::ui::widgets::{lerp_color, param_knob};

pub fn draw_band2(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    params: &SquelchBoxParams,
    kbd: &KbdQueue,
    rect: Rect,
) {
    let top = rect.top();
    let lbl_y = top + BAND1_BOT + 12.0;
    let tx = rect.left() + 68.0;
    let px = rect.left() + 162.0;
    let mx = rect.left() + 280.0;
    let _name_cx = rect.left() + 430.0;
    let vx = rect.left() + 724.0;
    let track_x0 = rect.left() + 128.0;

    {
        let p = ui.painter();
        p.text(Pos2::new(tx, lbl_y), egui::Align2::CENTER_TOP, "TEMPO",
            egui::FontId::new(7.5, egui::FontFamily::Monospace), INK);
        p.text(Pos2::new(tx - 32.0, top + CTL_Y + 34.0), egui::Align2::LEFT_CENTER, "SLOW",
            egui::FontId::new(6.5, egui::FontFamily::Monospace), SILVER_SHADOW);
        p.text(Pos2::new(tx + 32.0, top + CTL_Y + 34.0), egui::Align2::RIGHT_CENTER, "FAST",
            egui::FontId::new(6.5, egui::FontFamily::Monospace), SILVER_SHADOW);

        p.text(Pos2::new(track_x0, lbl_y), egui::Align2::LEFT_TOP, "BANK",
            egui::FontId::new(7.0, egui::FontFamily::Monospace), SILVER_SHADOW);
        p.text(Pos2::new(px, lbl_y + 28.0), egui::Align2::CENTER_TOP, "SLIDE",
            egui::FontId::new(7.5, egui::FontFamily::Monospace), INK);

        let mode_rect = Rect::from_min_size(Pos2::new(mx, top + BAND1_BOT + 6.0), Vec2::new(84.0, 104.0));
        p.rect_filled(mode_rect.translate(Vec2::new(0.0, 1.0)), 3.0, SILVER_LIGHT);
        p.rect_filled(mode_rect, 3.0, lerp_color(SILVER_MID, SILVER_DARK, 0.3));
        p.rect_stroke(mode_rect, 3.0, Stroke::new(1.0, SILVER_SHADOW), egui::StrokeKind::Inside);
        p.text(Pos2::new(mx + 42.0, top + BAND1_BOT + 8.0), egui::Align2::CENTER_TOP, "SYNC",
            egui::FontId::new(7.5, egui::FontFamily::Monospace), INK);

        p.text(Pos2::new(vx, lbl_y), egui::Align2::CENTER_TOP, "VOLUME",
            egui::FontId::new(7.5, egui::FontFamily::Monospace), INK);
    }

    // ── SYNC mode buttons ──
    let cur_mode = params.sync_mode.value();
    let modes = [
        (SyncMode::Internal, "INTERNAL", "Internal — free-run sequencer.\nRUN/STOP and TEMPO knob drive playback.\nIgnores DAW transport."),
        (SyncMode::Host,     "▶ HOST",   "Host — follow the DAW transport.\nTempo slaved to host BPM, plays when host plays.\nResets to step 1 on every host play press."),
        (SyncMode::Midi,     "MIDI IN",  "MIDI — sequencer disabled.\nVoice triggered only by incoming MIDI notes\nfrom the host or computer keyboard."),
    ];
    for (j, (mode, lbl, tip)) in modes.iter().enumerate() {
        let by = top + BAND1_BOT + 24.0 + j as f32 * 26.0;
        let br = Rect::from_min_size(Pos2::new(mx + 4.0, by), Vec2::new(76.0, 18.0));
        let id = ids::sync_btn(j);
        let resp = ui.interact(br, id, egui::Sense::click())
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

    // ── BANK I/II/III/IV ──
    let cur_bank = kbd.current_bank() as usize;
    let queued = kbd.queued_bank();
    for (j, lbl) in ["I", "II", "III", "IV"].iter().enumerate() {
        let bx = track_x0 + 2.0 + j as f32 * 18.0;
        let by = lbl_y + 12.0;
        let br = Rect::from_min_size(Pos2::new(bx, by), Vec2::new(14.0, 10.0));
        let id = ids::bank_btn(j);
        let resp = ui.interact(br, id, egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!(
                "Bank {} — switch the active pattern slot.\nQueued; takes effect at the next pattern-loop boundary.", lbl
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

    // ── LEN spinner ──
    let cur_len = kbd.pattern_snapshot().length.clamp(1, 16);
    let len_y = lbl_y + 12.0;
    let len_label_x = track_x0 + 80.0;
    let len_x0 = len_label_x + 20.0;
    ui.painter().text(Pos2::new(len_label_x, len_y + 1.0), egui::Align2::LEFT_TOP,
        "LEN", egui::FontId::new(7.0, egui::FontFamily::Monospace), SILVER_SHADOW);
    let len_dn = Rect::from_min_size(Pos2::new(len_x0, len_y), Vec2::new(12.0, 10.0));
    let len_box = Rect::from_min_size(Pos2::new(len_x0 + 14.0, len_y), Vec2::new(20.0, 10.0));
    let len_up = Rect::from_min_size(Pos2::new(len_x0 + 36.0, len_y), Vec2::new(12.0, 10.0));
    let dn_resp = ui.interact(len_dn, ids::len_dn(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("Pattern length − (down to 1)");
    let up_resp = ui.interact(len_up, ids::len_up(), egui::Sense::click())
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

    // Interactive knobs
    param_knob(ui, setter, ids::tempo(), &params.seq_bpm,
        Pos2::new(tx, top + CTL_Y), 30.0, "TEMPO", |v| format!("{v:.0} BPM"), false)
        .on_hover_text("Tempo — sequencer BPM (40..220).\nDrag the knob, or click the box below to type.");
    param_knob(ui, setter, ids::slide(), &params.slide_ms,
        Pos2::new(px, top + CTL_Y), 20.0, "SLIDE", |v| format!("{v:.0} ms"), false)
        .on_hover_text("Slide — portamento glide time (5..500 ms).\nHow long a slide-legato step takes to reach its target pitch.");

    // ── OCT ▼/▲ ──
    {
        let oct_y = top + CTL_Y + 38.0;
        let oct_dn = Rect::from_min_size(Pos2::new(px - 22.0, oct_y), Vec2::new(18.0, 10.0));
        let oct_up = Rect::from_min_size(Pos2::new(px + 4.0, oct_y), Vec2::new(18.0, 10.0));
        let dn_r = ui.interact(oct_dn, ids::oct_dn(), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Octave down — selected step −12 semitones.\nKeyboard: Shift+Down");
        let up_r = ui.interact(oct_up, ids::oct_up(), egui::Sense::click())
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

    param_knob(ui, setter, ids::vol(), &params.master_volume,
        Pos2::new(vx, top + CTL_Y), 26.0, "VOLUME",
        |v| { let db = 20.0 * v.max(1e-6).log10(); if db < -59.0 { "-inf".into() } else { format!("{db:+.1} dB") } },
        false)
        .on_hover_text("Volume — master output gain (-60..+6 dB).");

    // ── BPM text entry ──
    let bpm_id = ids::bpm_edit();
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
