//! SDL2 audio: the callback renders the shared synth into the output
//! buffer. The UI and the callback share one `Synth` behind a mutex;
//! both sides hold it only briefly, so playback stays glitch-free.

use crate::voice::Synth;
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use std::sync::{Arc, Mutex};

pub type SharedSynth = Arc<Mutex<Synth>>;

pub struct SynthCallback {
    pub synth: SharedSynth,
}

impl AudioCallback for SynthCallback {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        match self.synth.lock() {
            Ok(mut s) => s.render(out),
            Err(_) => {
                for x in out.iter_mut() {
                    *x = 0.0;
                }
            }
        }
    }
}

pub fn open_audio(
    sdl: &sdl2::Sdl,
    synth: SharedSynth,
) -> Result<AudioDevice<SynthCallback>, String> {
    let audio = sdl.audio()?;
    let spec = AudioSpecDesired {
        freq: Some(44100),
        channels: Some(1),
        samples: Some(1024),
    };
    let device = audio.open_playback(None, &spec, |_spec| SynthCallback { synth })?;
    device.resume();
    Ok(device)
}
