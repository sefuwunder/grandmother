//! grandmother: a Grandmother-style semi-modular analog synthesizer.
//!
//! - No args: launch the playable faceplate UI.
//! - `--smoke [secs]`: render headless for N seconds (default 2), exit 0.
//! - `--render [path] [secs]`: render a demo phrase to a WAV file.
//! - `--shot [path]`: render the faceplate offscreen to a BMP file.
//!
//! An homage to the Moog Grandmother's layout and signal flow, written
//! from scratch in Rust. Not affiliated with Moog Music.

mod arp;
mod audio;
mod env;
mod filter;
mod font;
mod lfo;
mod midi;
mod osc;
mod patch;
mod presets;
mod reverb;
mod seq;
mod ui;
mod voice;
mod wav;

use std::sync::{Arc, Mutex};
use voice::Synth;

const SR: f32 = 44100.0;

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|p| args.get(p + 1))
        .cloned()
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

/// Render a short demo phrase offline: four notes over a demo patch.
fn render_phrase(synth: &mut Synth, secs: f32) -> Vec<f32> {
    let n = (SR * secs) as usize;
    let mut buf = vec![0.0f32; n];
    // Demo patch: LFO into filter cutoff, for a moving sound.
    synth.patch.cables.push(patch::Cable {
        src: patch::Src::Out(patch::out::LFO),
        dst: patch::Dst::In(patch::inp::VCF_CUTOFF),
    });
    synth.params.lfo_rate = 4.0;
    let notes = [48u8, 55, 60, 64];
    let step = (SR * 0.5) as usize;
    let mut pos = 0;
    for (i, chunk) in buf.chunks_mut(step).enumerate() {
        if i < notes.len() {
            if i > 0 {
                synth.voice.note_off(notes[i - 1]);
            }
            synth.voice.note_on(notes[i]);
        }
        synth.render(chunk);
        pos += chunk.len();
        let _ = pos;
    }
    // Tail: release the last note, let the reverb ring out.
    if let Some(&last) = notes.last() {
        synth.voice.note_off(last);
    }
    let tail = (SR * 1.0) as usize;
    let mut tail_buf = vec![0.0f32; tail];
    synth.render(&mut tail_buf);
    buf.extend_from_slice(&tail_buf);
    buf
}

fn run_smoke(secs: f32) -> i32 {
    let mut synth = Synth::new(SR);
    let buf = render_phrase(&mut synth, secs);
    let finite = buf.iter().all(|x| x.is_finite());
    let peak = buf.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    let rms = (buf.iter().map(|x| x * x).sum::<f32>() / buf.len() as f32).sqrt();
    println!("smoke: {secs}s, {} samples, finite={finite}, peak={peak:.3}, rms={rms:.3}", buf.len());
    if finite && peak > 0.01 && rms > 0.005 {
        0
    } else {
        eprintln!("smoke FAILED: bad render");
        1
    }
}

fn run_render(path: &str, secs: f32) -> i32 {
    let mut synth = Synth::new(SR);
    let buf = render_phrase(&mut synth, secs);
    match wav::write_wav_mono_16(path, &buf, SR as u32) {
        Ok(()) => {
            let rms = (buf.iter().map(|x| x * x).sum::<f32>() / buf.len() as f32).sqrt();
            println!("wrote {path} ({} samples, rms {rms:.3})", buf.len());
            0
        }
        Err(e) => {
            eprintln!("render failed: {e}");
            1
        }
    }
}

fn run_shot(path: &str) -> i32 {
    let sdl = match sdl2::init() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sdl init failed: {e}");
            return 1;
        }
    };
    let _video = match sdl.video() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("video init failed: {e}");
            return 1;
        }
    };
    let surface = match sdl2::surface::Surface::new(ui::W, ui::H, sdl2::pixels::PixelFormatEnum::RGB24) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("surface failed: {e}");
            return 1;
        }
    };
    let mut canvas = match surface.into_canvas() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("canvas failed: {e}");
            return 1;
        }
    };
    let synth = Synth::new(SR);
    let ui_state = ui::UiState::new();
    ui::draw(&mut canvas, &synth, &ui_state);
    canvas.present();
    match canvas.surface().save_bmp(path) {
        Ok(()) => {
            println!("wrote {path}");
            0
        }
        Err(e) => {
            eprintln!("screenshot failed: {e}");
            1
        }
    }
}

fn run_ui() -> i32 {
    let sdl = match sdl2::init() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sdl init failed: {e}");
            return 1;
        }
    };
    let video = match sdl.video() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("video init failed: {e}");
            return 1;
        }
    };
    let window = match video
        .window("Grandmother", ui::W, ui::H)
        .position_centered()
        .build()
    {
        Ok(w) => w,
        Err(e) => {
            eprintln!("window failed: {e}");
            return 1;
        }
    };
    let mut canvas = match window.into_canvas().build() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("canvas failed: {e}");
            return 1;
        }
    };
    let shared: audio::SharedSynth = Arc::new(Mutex::new(Synth::new(SR)));
    match audio::open_audio(&sdl, shared.clone()) {
        Ok(_device) => {
            // Keep the device alive for the whole UI run.
            std::mem::forget(_device);
        }
        Err(e) => {
            eprintln!("audio unavailable, continuing muted: {e}");
        }
    }
    let midi_rx = midi::spawn_midi_thread();
    let mut ui_state = ui::UiState::new();
    ui_state.presets = presets::PresetStore::load();
    let mut pump = match sdl.event_pump() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("event pump failed: {e}");
            return 1;
        }
    };
    'running: loop {
        for event in pump.poll_iter() {
            use sdl2::event::Event;
            match event {
                Event::Quit { .. } => break 'running,
                Event::MouseButtonDown { x, y, .. } => {
                    if let Ok(mut s) = shared.lock() {
                        ui::on_mouse_down(&mut ui_state, &mut s, x, y);
                    }
                }
                Event::MouseButtonUp { x, y, .. } => {
                    if let Ok(mut s) = shared.lock() {
                        ui::on_mouse_up(&mut ui_state, &mut s, x, y);
                    }
                }
                Event::MouseMotion { x, y, .. } => {
                    if let Ok(mut s) = shared.lock() {
                        ui::on_mouse_motion(&mut ui_state, &mut s, x, y);
                    }
                }
                Event::KeyDown {
                    keycode: Some(k),
                    repeat,
                    ..
                } => {
                    if k == sdl2::keyboard::Keycode::Escape {
                        break 'running;
                    }
                    if let Ok(mut s) = shared.lock() {
                        ui::on_key_down(&mut ui_state, &mut s, k, repeat);
                    }
                }
                Event::KeyUp {
                    keycode: Some(k), ..
                } => {
                    if let Ok(mut s) = shared.lock() {
                        ui::on_key_up(&mut ui_state, &mut s, k);
                    }
                }
                _ => {}
            }
        }
        // Drain MIDI input events into the instrument.
        while let Ok(ev) = midi_rx.try_recv() {
            match &ev {
                midi::MidiEvent::Connected(name) => ui_state.midi_status = name.clone(),
                midi::MidiEvent::Disconnected => ui_state.midi_status = "-".to_string(),
                _ => {
                    if let Ok(mut s) = shared.lock() {
                        midi::apply_event(&mut s, &ev);
                    }
                }
            }
        }
        if let Ok(s) = shared.lock() {
            ui::draw(&mut canvas, &s, &ui_state);
        }
        canvas.present();
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = if has_flag(&args, "--smoke") {
        let secs = flag_value(&args, "--smoke")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(2.0);
        run_smoke(secs)
    } else if has_flag(&args, "--render") {
        let path = flag_value(&args, "--render").unwrap_or_else(|| "out.wav".to_string());
        let secs = args
            .iter()
            .position(|a| a == "--render")
            .and_then(|p| args.get(p + 2))
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(3.0);
        run_render(&path, secs)
    } else if has_flag(&args, "--shot") {
        let path = flag_value(&args, "--shot").unwrap_or_else(|| "shot.bmp".to_string());
        run_shot(&path)
    } else if has_flag(&args, "--help") {
        println!("grandmother: semi-modular analog synthesizer");
        println!("  grandmother               launch the playable UI");
        println!("  grandmother --smoke [s]   headless render check (default 2s)");
        println!("  grandmother --render [path] [s]  render a demo phrase to WAV");
        println!("  grandmother --shot [path] render the faceplate to a BMP");
        println!("  MIDI input: first ALSA input port, auto-reconnecting;");
        println!("    notes, velocity, pitch bend (+/-2 semitones), mod wheel");
        0
    } else {
        run_ui()
    };
    std::process::exit(code);
}
