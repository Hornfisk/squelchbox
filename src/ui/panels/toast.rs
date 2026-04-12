//! Transient notification overlay — shown after MIDI export etc.

use nih_plug_egui::egui::{self, Color32, Pos2, Rect, Vec2};
use std::path::PathBuf;

use crate::ui::ids;

/// Stored in egui temp data so it survives across frames.
#[derive(Clone)]
pub struct Toast {
    pub message: String,
    pub path: PathBuf,
    pub frame_born: u64,
}

const TOAST_LIFETIME: u64 = 600;
const TOAST_FADE_START: u64 = 510;

pub fn draw_toast(ui: &mut egui::Ui, rect: Rect) {
    let frame_id = ids::frame();
    let toast_id = ids::toast();

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

    let text_x = bar.left() + 8.0;
    let text_max_w = bar.width() - 120.0;
    let mut text = msg.clone();
    let galley = p.layout_no_wrap(text.clone(), font.clone(), fg);
    if galley.size().x > text_max_w {
        let prefix = "Exported: ...";
        let filename = toast
            .path
            .file_name()
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
    let open_resp = ui
        .interact(open_r, ids::toast_open(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    p.text(
        open_r.center(),
        egui::Align2::CENTER_CENTER,
        "[OPEN]",
        font.clone(),
        action_col,
    );
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
    let copy_resp = ui
        .interact(copy_r, ids::toast_copy(), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    p.text(
        copy_r.center(),
        egui::Align2::CENTER_CENTER,
        "[COPY]",
        font,
        action_col,
    );
    if copy_resp.clicked() {
        ui.ctx().copy_text(path_str);
    }
}

pub fn set_toast(ctx: &egui::Context, message: String, path: PathBuf) {
    let frame: u64 = ctx.data(|d| d.get_temp(ids::frame())).unwrap_or(0);
    ctx.data_mut(|d| {
        d.insert_temp(
            ids::toast(),
            Toast { message, path, frame_born: frame },
        )
    });
}
