//! Centralized egui ID registry — single source of truth for all
//! stringly-typed `egui::Id` values used across the UI module tree.

use nih_plug_egui::egui;

// ─── Simple (global) IDs ─────────────────────────────────────────────

pub fn frame()           -> egui::Id { egui::Id::new("sqb_frame") }
pub fn toast()           -> egui::Id { egui::Id::new("sqb_toast") }
pub fn toast_open()      -> egui::Id { egui::Id::new("sqb_toast_open") }
pub fn toast_copy()      -> egui::Id { egui::Id::new("sqb_toast_copy") }
pub fn last_persist_rev()-> egui::Id { egui::Id::new("sqb_last_persisted_rev") }
pub fn kbd_focus()       -> egui::Id { egui::Id::new("sqb_kbd_focus") }
pub fn prev_keys()       -> egui::Id { egui::Id::new("sqb_prev_keys") }
pub fn t_held()          -> egui::Id { egui::Id::new("sqb_t_held") }
pub fn fx_dist_anim()    -> egui::Id { egui::Id::new("sqb_fx_dist_anim") }
pub fn dist_toggle()     -> egui::Id { egui::Id::new("sqb_dist_toggle") }
pub fn dist_drive()      -> egui::Id { egui::Id::new("sqb_dist_drive") }
pub fn dist_mix()        -> egui::Id { egui::Id::new("sqb_dist_mix") }
pub fn fx_time_anim()    -> egui::Id { egui::Id::new("sqb_fx_time_anim") }
pub fn delay_toggle()    -> egui::Id { egui::Id::new("sqb_delay_toggle") }
pub fn reverb_toggle()   -> egui::Id { egui::Id::new("sqb_reverb_toggle") }
pub fn display()         -> egui::Id { egui::Id::new("sqb_display") }
pub fn delay_mode_btn()  -> egui::Id { egui::Id::new("sqb_delay_mode_btn") }
pub fn delay_sync_btn()  -> egui::Id { egui::Id::new("sqb_delay_sync_btn") }
pub fn delay_fdbk()      -> egui::Id { egui::Id::new("sqb_delay_fdbk") }
pub fn delay_mix()       -> egui::Id { egui::Id::new("sqb_delay_mix") }
pub fn reverb_decay()    -> egui::Id { egui::Id::new("sqb_reverb_decay") }
pub fn reverb_mix()      -> egui::Id { egui::Id::new("sqb_reverb_mix") }
pub fn tempo()           -> egui::Id { egui::Id::new("sqb_tempo") }
pub fn slide()           -> egui::Id { egui::Id::new("sqb_slide") }
pub fn vol()             -> egui::Id { egui::Id::new("sqb_vol") }
pub fn bpm_edit()        -> egui::Id { egui::Id::new("sqb_bpm_edit") }
pub fn len_dn()          -> egui::Id { egui::Id::new("sqb_len_dn") }
pub fn len_up()          -> egui::Id { egui::Id::new("sqb_len_up") }
pub fn oct_dn()          -> egui::Id { egui::Id::new("sqb_oct_dn") }
pub fn oct_up()          -> egui::Id { egui::Id::new("sqb_oct_up") }
pub fn rand()            -> egui::Id { egui::Id::new("sqb_rand") }
pub fn clear()           -> egui::Id { egui::Id::new("sqb_clear") }
pub fn shl()             -> egui::Id { egui::Id::new("sqb_shl") }
pub fn shr()             -> egui::Id { egui::Id::new("sqb_shr") }
pub fn saw()             -> egui::Id { egui::Id::new("sqb_saw") }
pub fn sqr()             -> egui::Id { egui::Id::new("sqb_sqr") }
pub fn runstop()         -> egui::Id { egui::Id::new("sqb_runstop") }
pub fn tr_dn()           -> egui::Id { egui::Id::new("sqb_tr_dn") }
pub fn tr_up()           -> egui::Id { egui::Id::new("sqb_tr_up") }
pub fn del()             -> egui::Id { egui::Id::new("sqb_del") }
pub fn ins()             -> egui::Id { egui::Id::new("sqb_ins") }
pub fn tm_acc()          -> egui::Id { egui::Id::new("sqb_tm_acc") }
pub fn tm_sld()          -> egui::Id { egui::Id::new("sqb_tm_sld") }
pub fn back()            -> egui::Id { egui::Id::new("sqb_back") }
pub fn step()            -> egui::Id { egui::Id::new("sqb_step") }
pub fn writenext()       -> egui::Id { egui::Id::new("sqb_writenext") }
pub fn tap()             -> egui::Id { egui::Id::new("sqb_tap") }
pub fn dump_midi()       -> egui::Id { egui::Id::new("sqb_dump_midi") }
pub fn tap_history()     -> egui::Id { egui::Id::new("sqb_tap_history") }

// ─── Indexed IDs (steps, knobs, banks) ───────────────────────────────

pub fn knob1(i: usize)    -> egui::Id { egui::Id::new(("sqb_k1", i)) }
pub fn sync_btn(j: usize) -> egui::Id { egui::Id::new(("sqb_sync", j)) }
pub fn bank_btn(j: usize) -> egui::Id { egui::Id::new(("sqb_bank", j)) }
pub fn step_acc(i: usize) -> egui::Id { egui::Id::new(("sqb_step_acc", i)) }
pub fn step_sld(i: usize) -> egui::Id { egui::Id::new(("sqb_step_sld", i)) }
pub fn step_rst(i: usize) -> egui::Id { egui::Id::new(("sqb_step_rst", i)) }
pub fn step_cell(i: usize)-> egui::Id { egui::Id::new(("sqb_step_cell", i)) }
