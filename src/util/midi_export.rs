//! Standard MIDI File (Type 0) writer for a single SquelchBox `Pattern`.
//!
//! Writes one .mid file per call into `$XDG_DATA_HOME/squelchbox/exports/`
//! (or `~/.local/share/squelchbox/exports/` as a fallback). The companion
//! Renoise tool watches this directory and imports the most recent file
//! into the active track. The file is a vanilla SMF Type 0 with one track
//! containing the 16 steps as note-on/note-off pairs at 96 PPQN.
//!
//! Slide steps are emitted as overlapping notes (next note-on lands a
//! tick before the prior note-off) so the importer can distinguish them
//! from rests; tools that don't honor that just see legato.

use std::io::Write;
use std::path::PathBuf;

use crate::sequencer::Pattern;

/// Write `pattern` as an SMF Type 0 file. Returns the path on success.
pub fn export_pattern(pattern: &Pattern, bpm: f32) -> std::io::Result<PathBuf> {
    let dir = export_dir();
    std::fs::create_dir_all(&dir)?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let path = dir.join(format!("squelchbox_pattern_{timestamp}.mid"));

    let ticks_per_quarter: u16 = 96;
    let ticks_per_step: u32 = (ticks_per_quarter / 4) as u32; // 16th note = 24 ticks
    let note_duration: u32 = ticks_per_step * 7 / 8; // hold most of the step

    let mut track: Vec<u8> = Vec::new();

    // Tempo meta event (microseconds per quarter note).
    let us_per_qn = (60_000_000.0 / bpm.max(20.0)) as u32;
    track.push(0x00);
    track.extend_from_slice(&[0xFF, 0x51, 0x03]);
    track.push(((us_per_qn >> 16) & 0xFF) as u8);
    track.push(((us_per_qn >> 8) & 0xFF) as u8);
    track.push((us_per_qn & 0xFF) as u8);

    struct Ev { tick: u32, status: u8, note: u8, velocity: u8 }
    let mut events: Vec<Ev> = Vec::new();

    let len = pattern.length.min(16) as usize;
    for (i, step) in pattern.steps.iter().take(len).enumerate() {
        if step.rest {
            continue;
        }
        let tick_on = i as u32 * ticks_per_step;
        // Slide: next step's slide flag means we're sliding INTO it,
        // so this step's note-off lands one tick AFTER the next note-on.
        let next = pattern.steps[(i + 1) % len];
        let slide_into_next = !next.rest && next.slide;
        let tick_off = if slide_into_next {
            tick_on + ticks_per_step + 1
        } else {
            tick_on + note_duration
        };
        let vel = if step.accent { 110 } else { 90 };
        events.push(Ev { tick: tick_on, status: 0x90, note: step.semitone, velocity: vel });
        events.push(Ev { tick: tick_off, status: 0x80, note: step.semitone, velocity: 0 });
    }

    // Sort: ascending tick, note-off before note-on at the same tick.
    events.sort_by(|a, b| a.tick.cmp(&b.tick).then(a.status.cmp(&b.status)));

    let mut last_tick = 0u32;
    for ev in &events {
        let delta = ev.tick - last_tick;
        last_tick = ev.tick;
        write_vlq(&mut track, delta);
        track.push(ev.status);
        track.push(ev.note);
        track.push(ev.velocity);
    }

    // End of track.
    write_vlq(&mut track, 0);
    track.extend_from_slice(&[0xFF, 0x2F, 0x00]);

    let mut f = std::fs::File::create(&path)?;
    f.write_all(b"MThd")?;
    f.write_all(&6u32.to_be_bytes())?;
    f.write_all(&0u16.to_be_bytes())?; // format 0
    f.write_all(&1u16.to_be_bytes())?; // 1 track
    f.write_all(&ticks_per_quarter.to_be_bytes())?;
    f.write_all(b"MTrk")?;
    f.write_all(&(track.len() as u32).to_be_bytes())?;
    f.write_all(&track)?;
    Ok(path)
}

fn write_vlq(buf: &mut Vec<u8>, mut value: u32) {
    if value == 0 {
        buf.push(0);
        return;
    }
    let mut bytes: Vec<u8> = Vec::with_capacity(4);
    bytes.push((value & 0x7F) as u8);
    value >>= 7;
    while value > 0 {
        bytes.push(((value & 0x7F) as u8) | 0x80);
        value >>= 7;
    }
    bytes.reverse();
    buf.extend_from_slice(&bytes);
}

pub fn export_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("XDG_DATA_HOME") {
        PathBuf::from(d).join("squelchbox/exports")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".local/share/squelchbox/exports")
    } else {
        PathBuf::from("/tmp/squelchbox/exports")
    }
}
