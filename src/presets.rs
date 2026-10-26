//! Presets: 8 slots of full instrument state (all knobs, patch cables,
//! sequencer steps), saved as plain text so they stay readable and
//! hand-editable. One file, `~/.grandmother/presets.txt`, with a
//! `[slot n]` section per occupied slot. Parsing is lenient: unknown
//! lines and unparseable values are skipped, so a hand-edited file
//! degrades instead of failing.

use crate::arp::ArpMode;
use crate::lfo::LfoWave;
use crate::osc::Wave;
use crate::patch::{Cable, Dst, Src, IN_JACKS, MULT_JACKS, OUT_JACKS};
use crate::seq::{Seq, SeqStep, STEPS};
use crate::voice::{ModDest, Params, Synth};
use std::path::PathBuf;

pub const SLOTS: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct PresetData {
    pub params: Params,
    pub cables: Vec<Cable>,
    pub steps: [SeqStep; STEPS],
}

impl PresetData {
    pub fn from_synth(synth: &Synth) -> Self {
        PresetData {
            params: synth.params.clone(),
            cables: synth.patch.cables.clone(),
            steps: synth.voice.seq.steps,
        }
    }

    pub fn apply_to(&self, synth: &mut Synth) {
        synth.params = self.params.clone();
        synth.patch.cables = self.cables.clone();
        synth.patch.selected = None;
        synth.voice.seq.steps = self.steps;
        synth.voice.seq_stop();
    }

    pub fn to_text(&self) -> String {
        let p = &self.params;
        let mut s = String::from("grandmother-preset-v1\n");
        let mut param = |name: &str, value: String| {
            s.push_str(&format!("param {name} {value}\n"));
        };
        param("osc1_wave", wave_name(p.osc1_wave).to_string());
        param("osc1_oct", p.osc1_oct.to_string());
        param("osc1_fine", p.osc1_fine.to_string());
        param("osc1_pw", p.osc1_pw.to_string());
        param("osc1_level", p.osc1_level.to_string());
        param("osc2_wave", wave_name(p.osc2_wave).to_string());
        param("osc2_oct", p.osc2_oct.to_string());
        param("osc2_fine", p.osc2_fine.to_string());
        param("osc2_pw", p.osc2_pw.to_string());
        param("osc2_level", p.osc2_level.to_string());
        param("noise_level", p.noise_level.to_string());
        param("glide", p.glide.to_string());
        param("cutoff", p.cutoff.to_string());
        param("reso", p.reso.to_string());
        param("env_amt", p.env_amt.to_string());
        param("kb_track", p.kb_track.to_string());
        param("f_attack", p.f_attack.to_string());
        param("f_decay", p.f_decay.to_string());
        param("f_sustain", p.f_sustain.to_string());
        param("f_release", p.f_release.to_string());
        param("a_attack", p.a_attack.to_string());
        param("a_decay", p.a_decay.to_string());
        param("a_sustain", p.a_sustain.to_string());
        param("a_release", p.a_release.to_string());
        param("lfo_rate", p.lfo_rate.to_string());
        param("lfo_wave", lfo_name(p.lfo_wave).to_string());
        param("mod_amt", p.mod_amt.to_string());
        param("mod_dest", mod_name(p.mod_dest).to_string());
        param("reverb_mix", p.reverb_mix.to_string());
        param("master", p.master.to_string());
        param("arp_enabled", bool_name(p.arp_enabled).to_string());
        param("arp_mode", arp_name(p.arp_mode).to_string());
        param("arp_rate", p.arp_rate.to_string());
        param("arp_oct", p.arp_oct.to_string());
        param("att", p.att.to_string());
        param("hp_cutoff", p.hp_cutoff.to_string());
        param("seq_rate", p.seq_rate.to_string());
        for c in &self.cables {
            s.push_str(&format!("cable {} {}\n", src_name(c.src), dst_name(c.dst)));
        }
        for (i, st) in self.steps.iter().enumerate() {
            s.push_str(&format!(
                "seq {} {} {} {}\n",
                i,
                st.note,
                bool_name(st.gate),
                bool_name(st.accent)
            ));
        }
        s
    }

    /// Parse one preset's lines. Unrecognized lines are skipped.
    pub fn from_text(text: &str) -> (PresetData, bool) {
        let mut data = PresetData {
            params: Params::default(),
            cables: Vec::new(),
            steps: Seq::new().steps,
        };
        let mut seen = false;
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                ["param", name, value] => {
                    if is_known_param(name) {
                        seen = true;
                    }
                    set_param(&mut data.params, name, value);
                }
                ["cable", src, dst] => {
                    if let (Some(s), Some(d)) = (parse_src(src), parse_dst(dst)) {
                        seen = true;
                        data.cables.push(Cable { src: s, dst: d });
                    }
                }
                ["seq", i, note, gate, accent] => {
                    if let (Ok(i), Ok(note)) = (i.parse::<usize>(), note.parse::<u8>()) {
                        if i < STEPS {
                            seen = true;
                            data.steps[i] = SeqStep {
                                note,
                                gate: *gate == "1",
                                accent: *accent == "1",
                            };
                        }
                    }
                }
                _ => {}
            }
        }
        (data, seen)
    }
}

fn wave_name(w: Wave) -> &'static str {
    match w {
        Wave::Saw => "SAW",
        Wave::Square => "SQR",
        Wave::Triangle => "TRI",
    }
}

fn lfo_name(w: LfoWave) -> &'static str {
    match w {
        LfoWave::Triangle => "TRI",
        LfoWave::Sine => "SIN",
        LfoWave::Saw => "SAW",
        LfoWave::Square => "SQR",
    }
}

fn mod_name(m: ModDest) -> &'static str {
    match m {
        ModDest::Pitch => "PITCH",
        ModDest::Filter => "FILTER",
        ModDest::Pw => "PW",
    }
}

fn arp_name(m: ArpMode) -> &'static str {
    match m {
        ArpMode::Up => "UP",
        ArpMode::Down => "DOWN",
        ArpMode::UpDown => "UPDN",
        ArpMode::Random => "RAND",
    }
}

fn bool_name(b: bool) -> &'static str {
    if b {
        "1"
    } else {
        "0"
    }
}

fn src_name(s: Src) -> String {
    match s {
        Src::Out(o) => format!("O{o}"),
        Src::Mult(m) => format!("M{m}"),
    }
}

fn dst_name(d: Dst) -> String {
    match d {
        Dst::In(i) => format!("I{i}"),
        Dst::Mult(m) => format!("M{m}"),
    }
}

fn parse_src(t: &str) -> Option<Src> {
    let (kind, rest) = t.split_at(1);
    let idx: usize = rest.parse().ok()?;
    match kind {
        "O" if idx < OUT_JACKS.len() => Some(Src::Out(idx)),
        "M" if idx < MULT_JACKS => Some(Src::Mult(idx)),
        _ => None,
    }
}

fn parse_dst(t: &str) -> Option<Dst> {
    let (kind, rest) = t.split_at(1);
    let idx: usize = rest.parse().ok()?;
    match kind {
        "I" if idx < IN_JACKS.len() => Some(Dst::In(idx)),
        "M" if idx < MULT_JACKS => Some(Dst::Mult(idx)),
        _ => None,
    }
}

fn is_known_param(name: &str) -> bool {
    matches!(
        name,
        "osc1_wave"
            | "osc1_oct"
            | "osc1_fine"
            | "osc1_pw"
            | "osc1_level"
            | "osc2_wave"
            | "osc2_oct"
            | "osc2_fine"
            | "osc2_pw"
            | "osc2_level"
            | "noise_level"
            | "glide"
            | "cutoff"
            | "reso"
            | "env_amt"
            | "kb_track"
            | "f_attack"
            | "f_decay"
            | "f_sustain"
            | "f_release"
            | "a_attack"
            | "a_decay"
            | "a_sustain"
            | "a_release"
            | "lfo_rate"
            | "lfo_wave"
            | "mod_amt"
            | "mod_dest"
            | "reverb_mix"
            | "master"
            | "arp_enabled"
            | "arp_mode"
            | "arp_rate"
            | "arp_oct"
            | "att"
            | "hp_cutoff"
            | "seq_rate"
    )
}

fn set_param(p: &mut Params, name: &str, value: &str) {
    let f = |v: &str| v.parse::<f32>().ok();
    let i = |v: &str| v.parse::<i32>().ok();
    let u = |v: &str| v.parse::<usize>().ok();
    match name {
        "osc1_wave" => match value {
            "SAW" => p.osc1_wave = Wave::Saw,
            "SQR" => p.osc1_wave = Wave::Square,
            "TRI" => p.osc1_wave = Wave::Triangle,
            _ => {}
        },
        "osc2_wave" => match value {
            "SAW" => p.osc2_wave = Wave::Saw,
            "SQR" => p.osc2_wave = Wave::Square,
            "TRI" => p.osc2_wave = Wave::Triangle,
            _ => {}
        },
        "lfo_wave" => match value {
            "TRI" => p.lfo_wave = LfoWave::Triangle,
            "SIN" => p.lfo_wave = LfoWave::Sine,
            "SAW" => p.lfo_wave = LfoWave::Saw,
            "SQR" => p.lfo_wave = LfoWave::Square,
            _ => {}
        },
        "mod_dest" => match value {
            "PITCH" => p.mod_dest = ModDest::Pitch,
            "FILTER" => p.mod_dest = ModDest::Filter,
            "PW" => p.mod_dest = ModDest::Pw,
            _ => {}
        },
        "arp_mode" => match value {
            "UP" => p.arp_mode = ArpMode::Up,
            "DOWN" => p.arp_mode = ArpMode::Down,
            "UPDN" => p.arp_mode = ArpMode::UpDown,
            "RAND" => p.arp_mode = ArpMode::Random,
            _ => {}
        },
        "arp_enabled" => p.arp_enabled = value == "1",
        "osc1_oct" => {
            if let Some(v) = i(value) {
                p.osc1_oct = v;
            }
        }
        "osc2_oct" => {
            if let Some(v) = i(value) {
                p.osc2_oct = v;
            }
        }
        "arp_oct" => {
            if let Some(v) = u(value) {
                p.arp_oct = v;
            }
        }
        "osc1_fine" => {
            if let Some(v) = f(value) {
                p.osc1_fine = v;
            }
        }
        "osc1_pw" => {
            if let Some(v) = f(value) {
                p.osc1_pw = v;
            }
        }
        "osc1_level" => {
            if let Some(v) = f(value) {
                p.osc1_level = v;
            }
        }
        "osc2_fine" => {
            if let Some(v) = f(value) {
                p.osc2_fine = v;
            }
        }
        "osc2_pw" => {
            if let Some(v) = f(value) {
                p.osc2_pw = v;
            }
        }
        "osc2_level" => {
            if let Some(v) = f(value) {
                p.osc2_level = v;
            }
        }
        "noise_level" => {
            if let Some(v) = f(value) {
                p.noise_level = v;
            }
        }
        "glide" => {
            if let Some(v) = f(value) {
                p.glide = v;
            }
        }
        "cutoff" => {
            if let Some(v) = f(value) {
                p.cutoff = v;
            }
        }
        "reso" => {
            if let Some(v) = f(value) {
                p.reso = v;
            }
        }
        "env_amt" => {
            if let Some(v) = f(value) {
                p.env_amt = v;
            }
        }
        "kb_track" => {
            if let Some(v) = f(value) {
                p.kb_track = v;
            }
        }
        "f_attack" => {
            if let Some(v) = f(value) {
                p.f_attack = v;
            }
        }
        "f_decay" => {
            if let Some(v) = f(value) {
                p.f_decay = v;
            }
        }
        "f_sustain" => {
            if let Some(v) = f(value) {
                p.f_sustain = v;
            }
        }
        "f_release" => {
            if let Some(v) = f(value) {
                p.f_release = v;
            }
        }
        "a_attack" => {
            if let Some(v) = f(value) {
                p.a_attack = v;
            }
        }
        "a_decay" => {
            if let Some(v) = f(value) {
                p.a_decay = v;
            }
        }
        "a_sustain" => {
            if let Some(v) = f(value) {
                p.a_sustain = v;
            }
        }
        "a_release" => {
            if let Some(v) = f(value) {
                p.a_release = v;
            }
        }
        "lfo_rate" => {
            if let Some(v) = f(value) {
                p.lfo_rate = v;
            }
        }
        "mod_amt" => {
            if let Some(v) = f(value) {
                p.mod_amt = v;
            }
        }
        "reverb_mix" => {
            if let Some(v) = f(value) {
                p.reverb_mix = v;
            }
        }
        "master" => {
            if let Some(v) = f(value) {
                p.master = v;
            }
        }
        "arp_rate" => {
            if let Some(v) = f(value) {
                p.arp_rate = v;
            }
        }
        "att" => {
            if let Some(v) = f(value) {
                p.att = v;
            }
        }
        "hp_cutoff" => {
            if let Some(v) = f(value) {
                p.hp_cutoff = v;
            }
        }
        "seq_rate" => {
            if let Some(v) = f(value) {
                p.seq_rate = v;
            }
        }
        _ => {}
    }
}

pub struct PresetStore {
    pub slots: Vec<Option<PresetData>>,
}

impl PresetStore {
    pub fn new() -> Self {
        PresetStore {
            slots: vec![None; SLOTS],
        }
    }

    pub fn default_path() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(|h| {
            PathBuf::from(h)
                .join(".grandmother")
                .join("presets.txt")
        })
    }

    pub fn load() -> Self {
        match Self::default_path() {
            Some(path) => Self::load_from(&path),
            None => Self::new(),
        }
    }

    pub fn load_from(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::load_from_text(&text),
            Err(_) => Self::new(),
        }
    }

    pub fn load_from_text(text: &str) -> Self {
        let mut store = Self::new();
        let mut current: Option<usize> = None;
        let mut buf = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("[slot ").and_then(|r| r.strip_suffix(']')) {
                if let Some(slot) = current.take() {
                    let (data, seen) = PresetData::from_text(&buf);
                    if seen {
                        store.slots[slot] = Some(data);
                    }
                    buf.clear();
                }
                current = rest.parse::<usize>().ok();
            } else if current.is_some() {
                buf.push_str(line);
                buf.push('\n');
            }
        }
        if let Some(slot) = current {
            let (data, seen) = PresetData::from_text(&buf);
            if seen {
                store.slots[slot] = Some(data);
            }
        }
        store
    }

    pub fn save(&self) -> std::io::Result<()> {
        match Self::default_path() {
            Some(path) => self.save_to(&path),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no HOME for preset path",
            )),
        }
    }

    pub fn save_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = String::from("# grandmother presets\n");
        for (i, slot) in self.slots.iter().enumerate() {
            if let Some(data) = slot {
                out.push_str(&format!("[slot {i}]\n"));
                out.push_str(&data.to_text());
            }
        }
        std::fs::write(path, out)
    }
}

impl Default for PresetStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{inp, out};

    fn tweaked() -> PresetData {
        let mut p = Params::default();
        p.osc1_wave = Wave::Triangle;
        p.osc2_wave = Wave::Square;
        p.lfo_wave = LfoWave::Saw;
        p.mod_dest = ModDest::Pw;
        p.arp_mode = ArpMode::Random;
        p.arp_enabled = true;
        p.cutoff = 1234.5;
        p.att = -0.5;
        p.seq_rate = 12.0;
        let mut steps = Seq::new().steps;
        steps[0] = SeqStep {
            note: 65,
            gate: true,
            accent: true,
        };
        steps[1] = SeqStep {
            note: 60,
            gate: false,
            accent: false,
        };
        PresetData {
            params: p,
            cables: vec![
                Cable {
                    src: Src::Out(out::LFO),
                    dst: Dst::In(inp::VCF_CUTOFF),
                },
                Cable {
                    src: Src::Out(out::OSC1_SAW),
                    dst: Dst::Mult(1),
                },
                Cable {
                    src: Src::Mult(1),
                    dst: Dst::In(inp::MIX_OSC2),
                },
            ],
            steps,
        }
    }

    #[test]
    fn round_trip() {
        let data = tweaked();
        let (back, seen) = PresetData::from_text(&data.to_text());
        assert!(seen);
        assert_eq!(back, data);
    }

    #[test]
    fn store_sections_round_trip() {
        let mut store = PresetStore::new();
        store.slots[0] = Some(tweaked());
        store.slots[5] = Some(tweaked());
        let path = std::env::temp_dir().join(format!("gm-presets-{}.txt", std::process::id()));
        store.save_to(&path).unwrap();
        let back = PresetStore::load_from(&path);
        assert_eq!(back.slots[0], store.slots[0]);
        assert_eq!(back.slots[5], store.slots[5]);
        assert!(back.slots[1].is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_section_and_garbage_are_ignored() {
        let text = "[slot 2]\n# nothing here\n[slot 3]\nthis is not a preset line\nparam bogus 1.0\n";
        let store = PresetStore::load_from_text(text);
        assert!(store.slots[2].is_none());
        assert!(store.slots[3].is_none());
    }
}
