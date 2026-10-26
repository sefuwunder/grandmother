//! MIDI input via ALSA (midir): connects to the first available input
//! port, keeps it while the port exists, and reconnects when ports
//! appear or change. Note on/off (velocity 0 counts as off), pitch bend
//! (±2 semitones), and mod wheel (CC 1) are forwarded to the UI loop
//! over a channel; all channels are accepted.
//!
//! Build needs libasound2-dev, runtime needs libasound2 — both standard
//! on desktop Linux. With no MIDI ports present the thread just idles
//! and retries, and the instrument plays exactly as before.

use crate::voice::Synth;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug)]
pub enum MidiEvent {
    NoteOn(u8, f32), // midi note, velocity 0..1
    NoteOff(u8),
    PitchBend(f32), // semitones, ±2
    ModWheel(f32),  // 0..1
    Connected(String),
    Disconnected,
}

/// Apply one MIDI event to the instrument. Connection events are for
/// the UI status line and need no synth access.
pub fn apply_event(synth: &mut Synth, ev: &MidiEvent) {
    match ev {
        MidiEvent::NoteOn(n, v) => synth.voice.note_on_vel(*n, *v),
        MidiEvent::NoteOff(n) => synth.voice.note_off(*n),
        MidiEvent::PitchBend(s) => synth.voice.set_bend(*s),
        MidiEvent::ModWheel(v) => synth.voice.set_mod_wheel(*v),
        MidiEvent::Connected(_) | MidiEvent::Disconnected => {}
    }
}

fn handle_message(tx: &Sender<MidiEvent>, msg: &[u8]) {
    if msg.is_empty() {
        return;
    }
    let status = msg[0] & 0xF0;
    match status {
        0x80 if msg.len() >= 3 => {
            let _ = tx.send(MidiEvent::NoteOff(msg[1]));
        }
        0x90 if msg.len() >= 3 => {
            if msg[2] == 0 {
                let _ = tx.send(MidiEvent::NoteOff(msg[1]));
            } else {
                let _ = tx.send(MidiEvent::NoteOn(msg[1], msg[2] as f32 / 127.0));
            }
        }
        0xB0 if msg.len() >= 3 && msg[1] == 1 => {
            let _ = tx.send(MidiEvent::ModWheel(msg[2] as f32 / 127.0));
        }
        0xE0 if msg.len() >= 3 => {
            let v = ((msg[2] as u32) << 7) | msg[1] as u32;
            let semi = (v as f32 - 8192.0) / 8192.0 * 2.0;
            let _ = tx.send(MidiEvent::PitchBend(semi));
        }
        _ => {}
    }
}

pub fn spawn_midi_thread() -> Receiver<MidiEvent> {
    let (tx, rx) = channel();
    thread::spawn(move || midi_loop(tx));
    rx
}

fn midi_loop(tx: Sender<MidiEvent>) {
    // Held only to keep the connection alive; never read.
    let mut _conn: Option<midir::MidiInputConnection<()>> = None;
    let mut current: Option<String> = None;
    loop {
        let mut input = match midir::MidiInput::new("grandmother") {
            Ok(i) => i,
            Err(_) => {
                thread::sleep(Duration::from_secs(2));
                continue;
            }
        };
        input.ignore(midir::Ignore::SysexAndTime);
        let ports = input.ports();
        let names: Vec<String> = ports
            .iter()
            .map(|p| input.port_name(p).unwrap_or_default())
            .collect();
        let still_there = current
            .as_ref()
            .map(|n| names.contains(n))
            .unwrap_or(false);
        if !still_there {
            _conn = None;
            if current.is_some() {
                let _ = tx.send(MidiEvent::Disconnected);
            }
            current = None;
            if let Some(port) = ports.first() {
                let name = names[0].clone();
                let tx2 = tx.clone();
                match input.connect(
                    port,
                    "grandmother-in",
                    move |_, msg, _| handle_message(&tx2, msg),
                    (),
                ) {
                    Ok(c) => {
                        _conn = Some(c);
                        current = Some(name.clone());
                        let _ = tx.send(MidiEvent::Connected(name));
                    }
                    Err(_) => {}
                }
            }
        }
        thread::sleep(Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(rx: &Receiver<MidiEvent>) -> Vec<MidiEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            out.push(ev);
        }
        out
    }

    #[test]
    fn message_parsing() {
        let (tx, rx) = channel();
        handle_message(&tx, &[0x90, 60, 100]); // note on
        handle_message(&tx, &[0x90, 62, 0]); // vel 0 = note off
        handle_message(&tx, &[0x80, 64, 0]); // note off
        handle_message(&tx, &[0xB0, 1, 64]); // mod wheel ~0.5
        handle_message(&tx, &[0xE0, 0x00, 0x40]); // bend center = 0
        handle_message(&tx, &[0xB0, 7, 100]); // volume: ignored
        handle_message(&tx, &[0xF8]); // clock: ignored
        let evs = collect(&rx);
        assert_eq!(evs.len(), 5);
        assert!(matches!(evs[0], MidiEvent::NoteOn(60, _)));
        if let MidiEvent::NoteOn(_, v) = evs[0] {
            assert!((v - 100.0 / 127.0).abs() < 0.01);
        }
        assert!(matches!(evs[1], MidiEvent::NoteOff(62)));
        assert!(matches!(evs[2], MidiEvent::NoteOff(64)));
        assert!(matches!(evs[3], MidiEvent::ModWheel(_)));
        assert!(matches!(evs[4], MidiEvent::PitchBend(s) if s.abs() < 0.01));
    }

    #[test]
    fn apply_event_drives_voice() {
        let mut synth = Synth::new(44100.0);
        apply_event(&mut synth, &MidiEvent::NoteOn(60, 0.5));
        assert!(synth.voice.held_notes().contains(&60));
        apply_event(&mut synth, &MidiEvent::PitchBend(1.5));
        apply_event(&mut synth, &MidiEvent::ModWheel(0.25));
        apply_event(&mut synth, &MidiEvent::NoteOff(60));
        assert!(synth.voice.held_notes().is_empty());
    }
}
