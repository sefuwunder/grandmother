//! The monophonic voice: keyboard/arp -> glide -> VCOs -> mixer -> ladder
//! filter -> VCA -> spring-style reverb -> master. The patch bay is
//! evaluated per sample, so audio-rate patching (oscillator into filter
//! cutoff, noise into pitch) works, with a one-sample delay only on
//! feedback paths (VCF OUT into VCF IN, LFO into its own rate, EG into
//! pitch) to keep the graph computable.

use crate::arp::{Arp, ArpMode};
use crate::env::Adsr;
use crate::filter::Ladder;
use crate::lfo::{Lfo, LfoWave};
use crate::osc::{cv_to_freq, midi_to_cv, Noise, Osc, Wave};
use crate::patch::{inp, out, InKind, PatchBay, IN_KINDS};
use crate::reverb::Reverb;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModDest {
    Pitch,
    Filter,
    Pw,
}

impl ModDest {
    pub fn next(self) -> ModDest {
        match self {
            ModDest::Pitch => ModDest::Filter,
            ModDest::Filter => ModDest::Pw,
            ModDest::Pw => ModDest::Pitch,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ModDest::Pitch => "PITCH",
            ModDest::Filter => "FILTER",
            ModDest::Pw => "PULSE W",
        }
    }
}

/// Knob parameters (buttons like wave select are set directly on Params).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParamId {
    Osc1Fine,
    Osc1Pw,
    Osc1Level,
    Osc2Fine,
    Osc2Pw,
    Osc2Level,
    NoiseLevel,
    Glide,
    Cutoff,
    Reso,
    EnvAmt,
    KbTrack,
    FAttack,
    FDecay,
    FSustain,
    FRelease,
    AAttack,
    ADecay,
    ASustain,
    ARelease,
    LfoRate,
    ModAmt,
    ReverbMix,
    Master,
    ArpRate,
}

#[derive(Clone, Debug)]
pub struct Params {
    pub osc1_wave: Wave,
    pub osc1_oct: i32, // semitones: -24, -12, 0, 12
    pub osc1_fine: f32, // semitones
    pub osc1_pw: f32,
    pub osc1_level: f32,
    pub osc2_wave: Wave,
    pub osc2_oct: i32,
    pub osc2_fine: f32,
    pub osc2_pw: f32,
    pub osc2_level: f32,
    pub noise_level: f32,
    pub glide: f32, // seconds
    pub cutoff: f32, // Hz
    pub reso: f32,
    pub env_amt: f32, // -1..1
    pub kb_track: f32,
    pub f_attack: f32,
    pub f_decay: f32,
    pub f_sustain: f32,
    pub f_release: f32,
    pub a_attack: f32,
    pub a_decay: f32,
    pub a_sustain: f32,
    pub a_release: f32,
    pub lfo_rate: f32, // Hz
    pub lfo_wave: LfoWave,
    pub mod_amt: f32,
    pub mod_dest: ModDest,
    pub reverb_mix: f32,
    pub master: f32,
    pub arp_enabled: bool,
    pub arp_mode: ArpMode,
    pub arp_rate: f32, // steps per second
    pub arp_oct: usize,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            osc1_wave: Wave::Saw,
            osc1_oct: 0,
            osc1_fine: 0.0,
            osc1_pw: 0.5,
            osc1_level: 0.8,
            osc2_wave: Wave::Saw,
            osc2_oct: 0,
            osc2_fine: 0.0,
            osc2_pw: 0.5,
            osc2_level: 0.6,
            noise_level: 0.0,
            glide: 0.0,
            cutoff: 2000.0,
            reso: 0.2,
            env_amt: 0.5,
            kb_track: 0.5,
            f_attack: 0.01,
            f_decay: 0.3,
            f_sustain: 0.5,
            f_release: 0.2,
            a_attack: 0.01,
            a_decay: 0.2,
            a_sustain: 0.8,
            a_release: 0.2,
            lfo_rate: 2.0,
            lfo_wave: LfoWave::Triangle,
            mod_amt: 0.0,
            mod_dest: ModDest::Filter,
            reverb_mix: 0.15,
            master: 0.8,
            arp_enabled: false,
            arp_mode: ArpMode::Up,
            arp_rate: 8.0,
            arp_oct: 1,
        }
    }
}

impl Params {
    pub fn get(&self, id: ParamId) -> f32 {
        match id {
            ParamId::Osc1Fine => self.osc1_fine,
            ParamId::Osc1Pw => self.osc1_pw,
            ParamId::Osc1Level => self.osc1_level,
            ParamId::Osc2Fine => self.osc2_fine,
            ParamId::Osc2Pw => self.osc2_pw,
            ParamId::Osc2Level => self.osc2_level,
            ParamId::NoiseLevel => self.noise_level,
            ParamId::Glide => self.glide,
            ParamId::Cutoff => self.cutoff,
            ParamId::Reso => self.reso,
            ParamId::EnvAmt => self.env_amt,
            ParamId::KbTrack => self.kb_track,
            ParamId::FAttack => self.f_attack,
            ParamId::FDecay => self.f_decay,
            ParamId::FSustain => self.f_sustain,
            ParamId::FRelease => self.f_release,
            ParamId::AAttack => self.a_attack,
            ParamId::ADecay => self.a_decay,
            ParamId::ASustain => self.a_sustain,
            ParamId::ARelease => self.a_release,
            ParamId::LfoRate => self.lfo_rate,
            ParamId::ModAmt => self.mod_amt,
            ParamId::ReverbMix => self.reverb_mix,
            ParamId::Master => self.master,
            ParamId::ArpRate => self.arp_rate,
        }
    }

    pub fn set(&mut self, id: ParamId, v: f32) {
        match id {
            ParamId::Osc1Fine => self.osc1_fine = v,
            ParamId::Osc1Pw => self.osc1_pw = v,
            ParamId::Osc1Level => self.osc1_level = v,
            ParamId::Osc2Fine => self.osc2_fine = v,
            ParamId::Osc2Pw => self.osc2_pw = v,
            ParamId::Osc2Level => self.osc2_level = v,
            ParamId::NoiseLevel => self.noise_level = v,
            ParamId::Glide => self.glide = v,
            ParamId::Cutoff => self.cutoff = v,
            ParamId::Reso => self.reso = v,
            ParamId::EnvAmt => self.env_amt = v,
            ParamId::KbTrack => self.kb_track = v,
            ParamId::FAttack => self.f_attack = v,
            ParamId::FDecay => self.f_decay = v,
            ParamId::FSustain => self.f_sustain = v,
            ParamId::FRelease => self.f_release = v,
            ParamId::AAttack => self.a_attack = v,
            ParamId::ADecay => self.a_decay = v,
            ParamId::ASustain => self.a_sustain = v,
            ParamId::ARelease => self.a_release = v,
            ParamId::LfoRate => self.lfo_rate = v,
            ParamId::ModAmt => self.mod_amt = v,
            ParamId::ReverbMix => self.reverb_mix = v,
            ParamId::Master => self.master = v,
            ParamId::ArpRate => self.arp_rate = v,
        }
    }
}

/// Current-sample signals, for patch resolution after they are computed.
struct Cur {
    kb_cv: f32,
    kb_gate: f32,
    lfo: f32,
    feg: f32,
    aeg: f32,
    o1saw: f32,
    o1pulse: f32,
    o2saw: f32,
    o2tri: f32,
    noise: f32,
}

pub struct Voice {
    osc1: Osc,
    osc2: Osc,
    noise: Noise,
    filter: Ladder,
    feg: Adsr,
    aeg: Adsr,
    lfo: Lfo,
    reverb: Reverb,
    arp: Arp,
    held: Vec<u8>, // press order
    arp_note: Option<u8>,
    arp_samples: u32,
    arp_retrigger: bool,
    glide_cv1: f32,
    glide_cv2: f32,
    glide_snap: bool,
    last_cv: f32,
    prev_gate: bool,
    prev_note: Option<u8>,
    prev_gate_f32: f32,
    prev_lfo: f32,
    prev_feg: f32,
    prev_aeg: f32,
    prev_o1saw: f32,
    prev_o1pulse: f32,
    prev_o2saw: f32,
    prev_o2tri: f32,
    prev_noise: f32,
    prev_vcf: f32,
    sr: f32,
}

impl Voice {
    pub fn new(sr: f32) -> Self {
        Voice {
            osc1: Osc::new(),
            osc2: Osc::new(),
            noise: Noise::new(),
            filter: Ladder::new(sr),
            feg: Adsr::new(sr),
            aeg: Adsr::new(sr),
            lfo: Lfo::new(),
            reverb: Reverb::new(sr),
            arp: Arp::new(),
            held: Vec::new(),
            arp_note: None,
            arp_samples: 0,
            arp_retrigger: false,
            glide_cv1: 0.0,
            glide_cv2: 0.0,
            glide_snap: false,
            last_cv: 2.0,
            prev_gate: false,
            prev_note: None,
            prev_gate_f32: 0.0,
            prev_lfo: 0.0,
            prev_feg: 0.0,
            prev_aeg: 0.0,
            prev_o1saw: 0.0,
            prev_o1pulse: 0.0,
            prev_o2saw: 0.0,
            prev_o2tri: 0.0,
            prev_noise: 0.0,
            prev_vcf: 0.0,
            sr,
        }
    }

    pub fn note_on(&mut self, midi: u8) {
        if !self.held.contains(&midi) {
            self.held.push(midi);
        }
        self.arp.set_held(&self.held);
    }

    pub fn note_off(&mut self, midi: u8) {
        self.held.retain(|&n| n != midi);
        self.arp.set_held(&self.held);
    }

    pub fn held_notes(&self) -> &[u8] {
        &self.held
    }

    pub fn current_note(&self) -> Option<u8> {
        self.arp_note.or_else(|| self.held.last().copied())
    }

    /// Previous-sample value of an output jack (feedback-safe reads).
    fn prev_sig(&self, o: usize) -> f32 {
        match o {
            out::KB_CV => self.last_cv,
            out::GATE => self.prev_gate_f32,
            out::LFO => self.prev_lfo,
            out::OSC1_SAW => self.prev_o1saw,
            out::OSC1_PULSE => self.prev_o1pulse,
            out::OSC2_SAW => self.prev_o2saw,
            out::OSC2_TRI => self.prev_o2tri,
            out::NOISE => self.prev_noise,
            out::FILT_EG => self.prev_feg,
            out::AMP_EG => self.prev_aeg,
            out::VCF_OUT => self.prev_vcf,
            _ => 0.0,
        }
    }

    /// Current-sample value where already computed, previous otherwise.
    fn cur_sig(&self, o: usize, cur: &Cur) -> f32 {
        match o {
            out::KB_CV => cur.kb_cv,
            out::GATE => cur.kb_gate,
            out::LFO => cur.lfo,
            out::OSC1_SAW => cur.o1saw,
            out::OSC1_PULSE => cur.o1pulse,
            out::OSC2_SAW => cur.o2saw,
            out::OSC2_TRI => cur.o2tri,
            out::NOISE => cur.noise,
            out::FILT_EG => cur.feg,
            out::AMP_EG => cur.aeg,
            out::VCF_OUT => self.prev_vcf,
            _ => 0.0,
        }
    }

    /// Sum of all cables into `inp`, resolving each output with `f`.
    fn sum_into<F: Fn(usize) -> f32>(&self, patch: &PatchBay, inp: usize, f: F) -> f32 {
        patch.cables_into(inp).iter().map(|&o| f(o)).sum()
    }

    pub fn render(&mut self, params: &Params, patch: &PatchBay, out: &mut [f32]) {
        self.feg.set(
            params.f_attack,
            params.f_decay,
            params.f_sustain,
            params.f_release,
        );
        self.aeg.set(
            params.a_attack,
            params.a_decay,
            params.a_sustain,
            params.a_release,
        );
        self.lfo.wave = params.lfo_wave;
        self.filter.reso = params.reso.clamp(0.0, 1.0);
        self.arp.mode = params.arp_mode;
        self.arp.octaves = params.arp_oct.clamp(1, 3);
        for s in out.iter_mut() {
            // Arpeggiator clock.
            if params.arp_enabled && !self.held.is_empty() {
                self.arp_samples += 1;
                let interval = (self.sr / params.arp_rate.max(0.1)) as u32;
                if self.arp_samples >= interval.max(1) {
                    self.arp_samples = 0;
                    self.arp_note = self.arp.step();
                    self.arp_retrigger = true;
                }
            } else {
                self.arp_note = None;
                self.arp_samples = 0;
            }
            *s = self.sample(params, patch);
        }
    }

    fn sample(&mut self, params: &Params, patch: &PatchBay) -> f32 {
        // (a) Note and keyboard CV/gate.
        let cur_note = self.current_note();
        let kb_gate = cur_note.is_some();
        let kb_cv = match cur_note {
            Some(n) => {
                let cv = midi_to_cv(n);
                self.last_cv = cv;
                cv
            }
            None => self.last_cv,
        };

        // (b) LFO, with its rate modulated by patched CV (previous-sample
        // values, so LFO-into-its-own-rate is a stable feedback path).
        let rate_cv = if patch.patched(inp::LFO_RATE) {
            self.sum_into(patch, inp::LFO_RATE, |o| self.prev_sig(o))
        } else {
            0.0
        };
        self.lfo.rate = (params.lfo_rate * 2.0f32.powf(rate_cv)).clamp(0.01, 200.0);
        let lfo_val = self.lfo.next(self.sr);

        // (c) Effective gate: patched GATE IN replaces the keyboard gate.
        let gate = if patch.patched(inp::GATE_IN) {
            let cur = Cur {
                kb_cv,
                kb_gate: if kb_gate { 1.0 } else { 0.0 },
                lfo: lfo_val,
                feg: self.prev_feg,
                aeg: self.prev_aeg,
                o1saw: self.prev_o1saw,
                o1pulse: self.prev_o1pulse,
                o2saw: self.prev_o2saw,
                o2tri: self.prev_o2tri,
                noise: self.prev_noise,
            };
            // Gate sources resolve against the current keyboard/LFO and
            // previous-sample everything else.
            let g = self.sum_into(patch, inp::GATE_IN, |o| self.cur_sig(o, &cur));
            g > 0.5
        } else {
            kb_gate
        };

        // (d) Envelopes: gate edges, note-change retrigger, arp retrigger.
        let retrigger = self.arp_retrigger
            || (gate && cur_note != self.prev_note && self.prev_gate);
        if gate && !self.prev_gate {
            self.feg.gate_on();
            self.aeg.gate_on();
            self.glide_snap = true;
        }
        if !gate && self.prev_gate {
            self.feg.gate_off();
            self.aeg.gate_off();
        }
        if retrigger {
            self.feg.gate_on();
            self.aeg.gate_on();
        }
        self.arp_retrigger = false;
        let feg_val = self.feg.next();
        let aeg_val = self.aeg.next();

        // (e) Oscillator pitch CVs (patched CV replaces the KB normal for
        // switched inputs), glide, and modulation.
        let pitch_cv = |this: &Voice, osc: usize| -> f32 {
            if patch.patched(osc) {
                let cur = Cur {
                    kb_cv,
                    kb_gate: if kb_gate { 1.0 } else { 0.0 },
                    lfo: lfo_val,
                    feg: feg_val,
                    aeg: aeg_val,
                    o1saw: this.prev_o1saw,
                    o1pulse: this.prev_o1pulse,
                    o2saw: this.prev_o2saw,
                    o2tri: this.prev_o2tri,
                    noise: this.prev_noise,
                };
                this.sum_into(patch, osc, |o| this.cur_sig(o, &cur))
            } else {
                kb_cv
            }
        };
        let target1 = pitch_cv(self, inp::OSC1_PITCH);
        let target2 = pitch_cv(self, inp::OSC2_PITCH);
        if self.glide_snap {
            self.glide_cv1 = target1;
            self.glide_cv2 = target2;
            self.glide_snap = false;
        } else if params.glide > 0.0 {
            let k = 1.0 - (-1.0 / (params.glide * self.sr)).exp();
            self.glide_cv1 += (target1 - self.glide_cv1) * k;
            self.glide_cv2 += (target2 - self.glide_cv2) * k;
        } else {
            self.glide_cv1 = target1;
            self.glide_cv2 = target2;
        }
        let mod_pitch = if params.mod_dest == ModDest::Pitch {
            lfo_val * params.mod_amt
        } else {
            0.0
        };
        let freq1 = cv_to_freq(
            self.glide_cv1 + (params.osc1_oct as f32 + params.osc1_fine) / 12.0 + mod_pitch,
        );
        let freq2 = cv_to_freq(
            self.glide_cv2 + (params.osc2_oct as f32 + params.osc2_fine) / 12.0 + mod_pitch,
        );

        // (f) Pulse widths, oscillator advance, mixer inputs.
        let pw_cv = |this: &Voice, inp_idx: usize| -> f32 {
            if patch.patched(inp_idx) {
                let cur = Cur {
                    kb_cv,
                    kb_gate: if kb_gate { 1.0 } else { 0.0 },
                    lfo: lfo_val,
                    feg: feg_val,
                    aeg: aeg_val,
                    o1saw: this.prev_o1saw,
                    o1pulse: this.prev_o1pulse,
                    o2saw: this.prev_o2saw,
                    o2tri: this.prev_o2tri,
                    noise: this.prev_noise,
                };
                this.sum_into(patch, inp_idx, |o| this.cur_sig(o, &cur))
            } else {
                0.0
            }
        };
        let mod_pw = if params.mod_dest == ModDest::Pw {
            lfo_val * params.mod_amt * 0.4
        } else {
            0.0
        };
        let pw1 = (params.osc1_pw + pw_cv(self, inp::OSC1_PW) * 0.5 + mod_pw).clamp(0.05, 0.95);
        let pw2 = (params.osc2_pw + pw_cv(self, inp::OSC2_PW) * 0.5 + mod_pw).clamp(0.05, 0.95);
        self.osc1.advance(freq1, self.sr);
        self.osc2.advance(freq2, self.sr);
        let o1saw = crate::osc::saw(self.osc1.phase);
        let o1pulse = crate::osc::pulse(self.osc1.phase, pw1);
        let o2saw = crate::osc::saw(self.osc2.phase);
        let o2tri = crate::osc::triangle(self.osc2.phase);
        let o1sel = self.osc1.output(params.osc1_wave, pw1);
        let o2sel = self.osc2.output(params.osc2_wave, pw2);
        let noise_val = self.noise.next();

        let cur = Cur {
            kb_cv,
            kb_gate: if kb_gate { 1.0 } else { 0.0 },
            lfo: lfo_val,
            feg: feg_val,
            aeg: aeg_val,
            o1saw,
            o1pulse,
            o2saw,
            o2tri,
            noise: noise_val,
        };

        // (g) Mixer -> VCF input (patched audio replaces the mixer normal).
        let mix = o1sel * params.osc1_level + o2sel * params.osc2_level + noise_val * params.noise_level;
        let vcf_in = if patch.patched(inp::VCF_IN) {
            self.sum_into(patch, inp::VCF_IN, |o| self.cur_sig(o, &cur))
        } else {
            mix
        };

        // (h) Cutoff: base x KB track x filter EG x additive CV x mod.
        let mod_cut = if params.mod_dest == ModDest::Filter {
            lfo_val * params.mod_amt * 2.0
        } else {
            0.0
        };
        let add_cut = if IN_KINDS[inp::VCF_CUTOFF] == InKind::Additive && patch.patched(inp::VCF_CUTOFF) {
            self.sum_into(patch, inp::VCF_CUTOFF, |o| self.cur_sig(o, &cur))
        } else {
            0.0
        };
        let cutoff = (params.cutoff
            * 2.0f32.powf(params.kb_track * kb_cv)
            * 2.0f32.powf(feg_val * params.env_amt * 2.0)
            * 2.0f32.powf(add_cut)
            * 2.0f32.powf(mod_cut))
        .clamp(20.0, 18000.0);
        self.filter.cutoff = cutoff;
        let vcf_out = self.filter.process(vcf_in);

        // (i) VCA: amp EG plus additive CV, clamped.
        let add_vca = if IN_KINDS[inp::VCA_CV] == InKind::Additive && patch.patched(inp::VCA_CV) {
            self.sum_into(patch, inp::VCA_CV, |o| self.cur_sig(o, &cur))
        } else {
            0.0
        };
        let vca_level = (aeg_val + add_vca).clamp(0.0, 1.0);
        let vca = vcf_out * vca_level;

        // (j) Reverb + master with soft clipping.
        let wet = self.reverb.process(vca);
        let mixed = vca * (1.0 - params.reverb_mix) + wet * params.reverb_mix;
        let out_sample = (mixed * params.master * 1.5).tanh() / 0.9051;

        // (k) Store previous-sample signals and gate/note state.
        self.prev_lfo = lfo_val;
        self.prev_feg = feg_val;
        self.prev_aeg = aeg_val;
        self.prev_o1saw = o1saw;
        self.prev_o1pulse = o1pulse;
        self.prev_o2saw = o2saw;
        self.prev_o2tri = o2tri;
        self.prev_noise = noise_val;
        self.prev_vcf = vcf_out;
        self.prev_gate = gate;
        self.prev_gate_f32 = if gate { 1.0 } else { 0.0 };
        self.prev_note = cur_note;

        out_sample
    }
}

/// The whole instrument: parameters + patch bay + voice. The audio
/// callback and the UI share one of these behind a mutex.
pub struct Synth {
    pub params: Params,
    pub patch: PatchBay,
    pub voice: Voice,
}

impl Synth {
    pub fn new(sr: f32) -> Self {
        Synth {
            params: Params::default(),
            patch: PatchBay::new(),
            voice: Voice::new(sr),
        }
    }

    pub fn render(&mut self, out: &mut [f32]) {
        let params = self.params.clone();
        self.voice.render(&params, &self.patch, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{out as o, PatchBay};

    fn rms(buf: &[f32]) -> f32 {
        (buf.iter().map(|x| x * x).sum::<f32>() / buf.len() as f32).sqrt()
    }

    fn render_note(params: &Params, patch: &PatchBay, note: u8, secs: f32) -> Vec<f32> {
        let mut v = Voice::new(44100.0);
        v.note_on(note);
        let n = (44100.0 * secs) as usize;
        let mut buf = vec![0.0; n];
        v.render(params, patch, &mut buf);
        buf
    }

    #[test]
    fn silence_without_notes() {
        let p = Params::default();
        let patch = PatchBay::new();
        let buf = render_note(&p, &patch, 60, 0.0); // renders nothing
        assert!(buf.is_empty());
        let mut v = Voice::new(44100.0);
        let mut buf = vec![0.0; 22050];
        v.render(&p, &patch, &mut buf);
        assert!(buf.iter().all(|&x| x == 0.0), "no note -> silence");
    }

    #[test]
    fn note_sounds_and_releases() {
        let mut p = Params::default();
        p.reverb_mix = 0.0;
        let patch = PatchBay::new();
        let buf = render_note(&p, &patch, 60, 0.5);
        assert!(rms(&buf) > 0.05, "note should sound, rms {}", rms(&buf));
        assert!(buf.iter().all(|x| x.is_finite()));
        // Release: note off, then the tail must die.
        let mut v = Voice::new(44100.0);
        v.note_on(60);
        let mut on = vec![0.0; 13230];
        v.render(&p, &patch, &mut on);
        v.note_off(60);
        let mut off = vec![0.0; 44100];
        v.render(&p, &patch, &mut off);
        assert!(rms(&off[35280..]) < 0.01, "tail rms {}", rms(&off[35280..]));
    }

    #[test]
    fn patch_changes_sound() {
        let p = Params::default();
        let plain = render_note(&p, &PatchBay::new(), 60, 0.5);
        let mut patch = PatchBay::new();
        patch.cables.push(crate::patch::Cable {
            out: o::LFO,
            inp: inp::VCF_CUTOFF,
        });
        let mut p2 = Params::default();
        p2.lfo_rate = 5.0;
        p2.mod_amt = 0.0;
        let patched = render_note(&p2, &patch, 60, 0.5);
        let diff = plain
            .iter()
            .zip(patched.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(diff > 0.01, "LFO->cutoff should change the sound, max diff {diff}");
    }

    #[test]
    fn lfo_into_gate_triggers_without_keys() {
        let mut p = Params::default();
        p.lfo_wave = LfoWave::Square;
        p.lfo_rate = 5.0;
        let mut patch = PatchBay::new();
        patch.cables.push(crate::patch::Cable {
            out: o::LFO,
            inp: inp::GATE_IN,
        });
        let mut v = Voice::new(44100.0);
        let mut buf = vec![0.0; 44100];
        v.render(&p, &patch, &mut buf);
        assert!(rms(&buf) > 0.01, "patched gate should trigger, rms {}", rms(&buf));
    }
}
