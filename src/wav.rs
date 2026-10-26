//! Minimal 16-bit PCM mono WAV writer (for --render).

use std::fs::File;
use std::io::{self, Write};

pub fn write_wav_mono_16(path: &str, samples: &[f32], sr: u32) -> io::Result<()> {
    let mut f = File::create(path)?;
    let data_bytes = samples.len() as u32 * 2;
    let mut w = |b: &[u8]| f.write_all(b);
    w(b"RIFF")?;
    w(&(36 + data_bytes).to_le_bytes())?;
    w(b"WAVE")?;
    w(b"fmt ")?;
    w(&16u32.to_le_bytes())?;
    w(&1u16.to_le_bytes())?; // PCM
    w(&1u16.to_le_bytes())?; // mono
    w(&sr.to_le_bytes())?;
    w(&(sr * 2).to_le_bytes())?; // byte rate
    w(&2u16.to_le_bytes())?; // block align
    w(&16u16.to_le_bytes())?; // bits
    w(b"data")?;
    w(&data_bytes.to_le_bytes())?;
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        w(&v.to_le_bytes())?;
    }
    Ok(())
}
