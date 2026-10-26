//! Faceplate UI: cream panel, colored sections, knobs, buttons, the patch
//! bay with cables, and a two-octave on-screen keyboard. Draws on any SDL
//! canvas target (window or offscreen surface for --shot).
//!
//! Interaction: drag knobs vertically; click buttons to cycle; click an
//! output jack then an input jack to patch (click an input with nothing
//! selected to remove its newest cable); click piano keys or use the
//! computer keyboard (Z row = lower octave, Q row = upper, arrows shift).

use crate::font::{draw_text, text_width};
use crate::lfo::LfoWave;
use crate::osc::Wave;
use crate::patch::{IN_JACKS, OUT_JACKS};
use crate::voice::{ParamId, Params, Synth};
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, RenderTarget};
use std::collections::{HashMap, HashSet};

pub const W: u32 = 1160;
pub const H: u32 = 720;

const PANEL: Color = Color::RGB(232, 228, 218);
const INK: Color = Color::RGB(30, 30, 34);
const DIM: Color = Color::RGB(110, 108, 100);
const KNOB: Color = Color::RGB(38, 38, 44);
const KNOB_MARK: Color = Color::RGB(255, 196, 66);
const BAY_BG: Color = Color::RGB(46, 44, 40);
const BAY_LABEL: Color = Color::RGB(220, 216, 204);
const JACK_RING: Color = Color::RGB(190, 188, 180);
const JACK_HOLE: Color = Color::RGB(12, 12, 14);
const KEY_WHITE: Color = Color::RGB(242, 242, 236);
const KEY_BLACK: Color = Color::RGB(24, 24, 28);
const KEY_ON: Color = Color::RGB(255, 190, 60);
const BTN: Color = Color::RGB(214, 210, 198);
const BTN_ON: Color = Color::RGB(255, 196, 66);

const CABLE_COLORS: [(u8, u8, u8); 11] = [
    (231, 76, 60),
    (243, 156, 18),
    (46, 204, 113),
    (52, 152, 219),
    (155, 89, 182),
    (26, 188, 156),
    (230, 126, 34),
    (236, 240, 241),
    (253, 121, 168),
    (253, 203, 110),
    (162, 155, 254),
];

fn cable_color(i: usize) -> Color {
    let (r, g, b) = CABLE_COLORS[i % CABLE_COLORS.len()];
    Color::RGB(r, g, b)
}

// ---------------- layout ----------------

#[derive(Clone, Copy)]
pub struct KnobDef {
    pub id: ParamId,
    pub label: &'static str,
    pub x: i32, // center
    pub y: i32, // center
    pub min: f32,
    pub max: f32,
    pub log: bool,
}

fn knob(id: ParamId, label: &'static str, x: i32, y: i32, min: f32, max: f32, log: bool) -> KnobDef {
    KnobDef {
        id,
        label,
        x,
        y,
        min,
        max,
        log,
    }
}

/// Every knob on the panel, exactly once. Row 1: OSC1/OSC2/MIXER/FILTER/
/// ENVELOPES/MOD. Row 2: REVERB/ARP/OUTPUT.
pub fn all_knobs() -> Vec<KnobDef> {
    vec![
        // OSC 1
        knob(ParamId::Osc1Fine, "FINE", 66, 200, -1.0, 1.0, false),
        knob(ParamId::Osc1Pw, "PULSE W", 156, 200, 0.05, 0.95, false),
        // OSC 2
        knob(ParamId::Osc2Fine, "FINE", 264, 200, -1.0, 1.0, false),
        knob(ParamId::Osc2Pw, "PULSE W", 354, 200, 0.05, 0.95, false),
        // MIXER
        knob(ParamId::Osc1Level, "OSC 1", 452, 130, 0.0, 1.0, false),
        knob(ParamId::Osc2Level, "OSC 2", 522, 130, 0.0, 1.0, false),
        knob(ParamId::NoiseLevel, "NOISE", 487, 230, 0.0, 1.0, false),
        // FILTER
        knob(ParamId::Cutoff, "CUTOFF", 620, 130, 20.0, 12000.0, true),
        knob(ParamId::Reso, "RESO", 710, 130, 0.0, 1.0, false),
        knob(ParamId::EnvAmt, "ENV AMT", 620, 230, -1.0, 1.0, false),
        knob(ParamId::KbTrack, "KB TRACK", 710, 230, 0.0, 1.0, false),
        // ENVELOPES: FILT EG row, AMP EG row
        knob(ParamId::FAttack, "F-ATK", 788, 130, 0.0, 1.0, false),
        knob(ParamId::FDecay, "F-DEC", 828, 130, 0.0, 2.0, false),
        knob(ParamId::FSustain, "F-SUS", 868, 130, 0.0, 1.0, false),
        knob(ParamId::FRelease, "F-REL", 908, 130, 0.0, 2.0, false),
        knob(ParamId::AAttack, "A-ATK", 788, 230, 0.0, 1.0, false),
        knob(ParamId::ADecay, "A-DEC", 828, 230, 0.0, 2.0, false),
        knob(ParamId::ASustain, "A-SUS", 868, 230, 0.0, 1.0, false),
        knob(ParamId::ARelease, "A-REL", 908, 230, 0.0, 2.0, false),
        // MODULATION
        knob(ParamId::LfoRate, "LFO RATE", 1010, 210, 0.05, 50.0, true),
        knob(ParamId::ModAmt, "MOD AMT", 1100, 210, 0.0, 1.0, false),
        // REVERB
        knob(ParamId::ReverbMix, "MIX", 91, 380, 0.0, 1.0, false),
        // ARPEGGIATOR
        knob(ParamId::ArpRate, "RATE", 240, 400, 0.5, 16.0, true),
        // OUTPUT
        knob(ParamId::Master, "MASTER", 452, 380, 0.0, 1.0, false),
        knob(ParamId::Glide, "GLIDE", 522, 380, 0.0, 0.5, false),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BtnKind {
    Osc1Wave,
    Osc1Oct,
    Osc2Wave,
    Osc2Oct,
    LfoWave,
    ModDest,
    ArpOn,
    ArpMode,
    ArpOct,
    Clear,
    Demo,
}

#[derive(Clone, Copy)]
pub struct BtnDef {
    pub kind: BtnKind,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub fn buttons() -> Vec<BtnDef> {
    vec![
        BtnDef { kind: BtnKind::Osc1Wave, x: 36, y: 96, w: 150, h: 26 },
        BtnDef { kind: BtnKind::Osc1Oct, x: 36, y: 126, w: 150, h: 26 },
        BtnDef { kind: BtnKind::Osc2Wave, x: 234, y: 96, w: 150, h: 26 },
        BtnDef { kind: BtnKind::Osc2Oct, x: 234, y: 126, w: 150, h: 26 },
        BtnDef { kind: BtnKind::LfoWave, x: 986, y: 96, w: 138, h: 26 },
        BtnDef { kind: BtnKind::ModDest, x: 986, y: 126, w: 138, h: 26 },
        BtnDef { kind: BtnKind::ArpOn, x: 194, y: 330, w: 60, h: 26 },
        BtnDef { kind: BtnKind::ArpMode, x: 262, y: 330, w: 70, h: 26 },
        BtnDef { kind: BtnKind::ArpOct, x: 340, y: 330, w: 56, h: 26 },
        BtnDef { kind: BtnKind::Clear, x: 962, y: 10, w: 80, h: 26 },
        BtnDef { kind: BtnKind::Demo, x: 1050, y: 10, w: 80, h: 26 },
    ]
}

pub struct JackPos {
    pub x: i32,
    pub y: i32,
}

pub fn out_jack_pos(i: usize) -> JackPos {
    JackPos {
        x: 60 + i as i32 * 104,
        y: 486,
    }
}

pub fn in_jack_pos(i: usize) -> JackPos {
    JackPos {
        x: 90 + i as i32 * 121,
        y: 548,
    }
}

pub fn key_rects() -> (Vec<(u8, Rect)>, Vec<(u8, Rect)>) {
    // Two octaves, C3 (48) .. B4 (71). White keys span x 16..1136.
    let x0 = 16;
    let white_w = 80;
    let y0 = 588;
    let h = 100;
    let mut whites = Vec::new();
    let mut blacks = Vec::new();
    let mut white_idx = 0;
    for midi in 48..72u8 {
        let pc = midi % 12;
        let is_black = matches!(pc, 1 | 3 | 6 | 8 | 10);
        if !is_black {
            let r = Rect::new(x0 + white_idx * white_w, y0, white_w as u32 - 2, h as u32);
            whites.push((midi, r));
            white_idx += 1;
        }
    }
    let mut white_idx = 0;
    for midi in 48..72u8 {
        let pc = midi % 12;
        let is_black = matches!(pc, 1 | 3 | 6 | 8 | 10);
        if !is_black {
            white_idx += 1;
        } else {
            let bx = x0 + white_idx * white_w - 24;
            blacks.push((midi, Rect::new(bx, y0, 48, 62)));
        }
    }
    (whites, blacks)
}

/// Which piano key (if any) is under the point; black keys win ties.
pub fn key_at(x: i32, y: i32) -> Option<u8> {
    let (whites, blacks) = key_rects();
    for (m, r) in &blacks {
        if r.contains_point((x, y)) {
            return Some(*m);
        }
    }
    for (m, r) in &whites {
        if r.contains_point((x, y)) {
            return Some(*m);
        }
    }
    None
}

// ---------------- value mapping ----------------

pub fn to_t(v: f32, min: f32, max: f32, log: bool) -> f32 {
    if log {
        (v.max(min) / min).ln() / (max / min).ln()
    } else {
        (v - min) / (max - min)
    }
    .clamp(0.0, 1.0)
}

pub fn from_t(t: f32, min: f32, max: f32, log: bool) -> f32 {
    if log {
        min * (max / min).powf(t.clamp(0.0, 1.0))
    } else {
        min + (max - min) * t.clamp(0.0, 1.0)
    }
}

pub fn fmt_value(id: ParamId, v: f32) -> String {
    match id {
        ParamId::Cutoff => {
            if v >= 1000.0 {
                format!("{:.2}k", v / 1000.0)
            } else {
                format!("{v:.0}")
            }
        }
        ParamId::LfoRate => format!("{v:.2}"),
        ParamId::ArpRate => format!("{v:.1}"),
        ParamId::Glide | ParamId::FAttack | ParamId::FDecay | ParamId::FRelease
        | ParamId::AAttack | ParamId::ADecay | ParamId::ARelease => format!("{v:.2}s"),
        ParamId::EnvAmt | ParamId::Osc1Fine | ParamId::Osc2Fine => format!("{v:+.2}"),
        _ => format!("{v:.2}"),
    }
}

// ---------------- UI state + input ----------------

pub struct UiState {
    pub drag: Option<(ParamId, i32, f32)>, // knob, start y, start t
    pub mouse: (i32, i32),
    pub mouse_notes: HashSet<u8>,
    pub key_notes: HashMap<sdl2::keyboard::Keycode, u8>,
    pub base_octave: i32, // semitone shift for computer keys, default 0
}

impl UiState {
    pub fn new() -> Self {
        UiState {
            drag: None,
            mouse: (0, 0),
            mouse_notes: HashSet::new(),
            key_notes: HashMap::new(),
            base_octave: 0,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

fn knob_at(x: i32, y: i32) -> Option<KnobDef> {
    all_knobs().into_iter().find(|k| {
        (x - k.x).abs() <= 24 && (y - k.y).abs() <= 34
    })
}

fn button_at(x: i32, y: i32) -> Option<BtnKind> {
    buttons()
        .into_iter()
        .find(|b| x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + b.h)
        .map(|b| b.kind)
}

fn out_jack_at(x: i32, y: i32) -> Option<usize> {
    (0..OUT_JACKS.len()).find(|&i| {
        let p = out_jack_pos(i);
        (x - p.x).pow(2) + (y - p.y).pow(2) <= 16 * 16
    })
}

fn in_jack_at(x: i32, y: i32) -> Option<usize> {
    (0..IN_JACKS.len()).find(|&i| {
        let p = in_jack_pos(i);
        (x - p.x).pow(2) + (y - p.y).pow(2) <= 16 * 16
    })
}

fn next_wave_osc1(w: Wave) -> Wave {
    match w {
        Wave::Saw => Wave::Square,
        _ => Wave::Saw,
    }
}

fn next_wave_osc2(w: Wave) -> Wave {
    match w {
        Wave::Saw => Wave::Square,
        Wave::Square => Wave::Triangle,
        Wave::Triangle => Wave::Saw,
    }
}

fn next_lfo_wave(w: LfoWave) -> LfoWave {
    match w {
        LfoWave::Triangle => LfoWave::Sine,
        LfoWave::Sine => LfoWave::Saw,
        LfoWave::Saw => LfoWave::Square,
        LfoWave::Square => LfoWave::Triangle,
    }
}

fn next_oct(o: i32) -> i32 {
    match o {
        -24 => -12,
        -12 => 0,
        0 => 12,
        _ => -24,
    }
}

fn oct_label(o: i32) -> &'static str {
    match o {
        -24 => "32'",
        -12 => "16'",
        0 => "8'",
        _ => "4'",
    }
}

pub fn press_button(synth: &mut Synth, kind: BtnKind) {
    let p = &mut synth.params;
    match kind {
        BtnKind::Osc1Wave => p.osc1_wave = next_wave_osc1(p.osc1_wave),
        BtnKind::Osc1Oct => p.osc1_oct = next_oct(p.osc1_oct),
        BtnKind::Osc2Wave => p.osc2_wave = next_wave_osc2(p.osc2_wave),
        BtnKind::Osc2Oct => p.osc2_oct = next_oct(p.osc2_oct),
        BtnKind::LfoWave => p.lfo_wave = next_lfo_wave(p.lfo_wave),
        BtnKind::ModDest => p.mod_dest = p.mod_dest.next(),
        BtnKind::ArpOn => p.arp_enabled = !p.arp_enabled,
        BtnKind::ArpMode => p.arp_mode = p.arp_mode.next(),
        BtnKind::ArpOct => p.arp_oct = if p.arp_oct >= 3 { 1 } else { p.arp_oct + 1 },
        BtnKind::Clear => synth.patch.clear(),
        BtnKind::Demo => {
            // A fun starting patch: LFO wobbles the filter, the second
            // oscillator's triangle drives the filter input directly.
            synth.patch.clear();
            synth.patch.cables.push(crate::patch::Cable {
                out: crate::patch::out::LFO,
                inp: crate::patch::inp::VCF_CUTOFF,
            });
            synth.patch.cables.push(crate::patch::Cable {
                out: crate::patch::out::OSC2_TRI,
                inp: crate::patch::inp::VCF_IN,
            });
            synth.params.lfo_rate = 6.0;
            synth.params.mod_amt = 0.0;
        }
    }
}

pub fn on_mouse_down(ui: &mut UiState, synth: &mut Synth, x: i32, y: i32) {
    ui.mouse = (x, y);
    if let Some(k) = knob_at(x, y) {
        let v = synth.params.get(k.id);
        ui.drag = Some((k.id, y, to_t(v, k.min, k.max, k.log)));
        return;
    }
    if let Some(kind) = button_at(x, y) {
        press_button(synth, kind);
        return;
    }
    if let Some(o) = out_jack_at(x, y) {
        synth.patch.click_out(o);
        return;
    }
    if let Some(i) = in_jack_at(x, y) {
        synth.patch.click_in(i);
        return;
    }
    if let Some(midi) = key_at(x, y) {
        synth.voice.note_on(midi);
        ui.mouse_notes.insert(midi);
    }
}

pub fn on_mouse_up(ui: &mut UiState, synth: &mut Synth, _x: i32, _y: i32) {
    ui.drag = None;
    for m in ui.mouse_notes.drain() {
        synth.voice.note_off(m);
    }
}

pub fn on_mouse_motion(ui: &mut UiState, synth: &mut Synth, x: i32, y: i32) {
    ui.mouse = (x, y);
    if let Some((id, start_y, start_t)) = ui.drag {
        if let Some(k) = all_knobs().into_iter().find(|k| k.id == id) {
            let t = start_t + (start_y - y) as f32 * 0.005;
            synth.params.set(id, from_t(t, k.min, k.max, k.log));
        }
    }
}

fn computer_key_midi(key: sdl2::keyboard::Keycode, base: i32) -> Option<u8> {
    use sdl2::keyboard::Keycode;
    let semi: Option<i32> = match key {
        Keycode::Z => Some(0),
        Keycode::S => Some(1),
        Keycode::X => Some(2),
        Keycode::D => Some(3),
        Keycode::C => Some(4),
        Keycode::V => Some(5),
        Keycode::G => Some(6),
        Keycode::B => Some(7),
        Keycode::H => Some(8),
        Keycode::N => Some(9),
        Keycode::J => Some(10),
        Keycode::M => Some(11),
        Keycode::Q => Some(12),
        Keycode::Num2 => Some(13),
        Keycode::W => Some(14),
        Keycode::Num3 => Some(15),
        Keycode::E => Some(16),
        Keycode::R => Some(17),
        Keycode::Num5 => Some(18),
        Keycode::T => Some(19),
        Keycode::Num6 => Some(20),
        Keycode::Y => Some(21),
        Keycode::Num7 => Some(22),
        Keycode::U => Some(23),
        Keycode::I => Some(24),
        Keycode::Num9 => Some(25),
        Keycode::O => Some(26),
        Keycode::Num0 => Some(27),
        Keycode::P => Some(28),
        _ => None,
    };
    semi.map(|s| (48 + base + s).clamp(0, 127) as u8)
}

pub fn on_key_down(
    ui: &mut UiState,
    synth: &mut Synth,
    key: sdl2::keyboard::Keycode,
    repeat: bool,
) {
    use sdl2::keyboard::Keycode;
    if repeat {
        return;
    }
    match key {
        Keycode::Left => {
            ui.base_octave = (ui.base_octave - 12).max(-24);
            return;
        }
        Keycode::Right => {
            ui.base_octave = (ui.base_octave + 12).min(24);
            return;
        }
        _ => {}
    }
    if let Some(midi) = computer_key_midi(key, ui.base_octave) {
        if !ui.key_notes.contains_key(&key) {
            ui.key_notes.insert(key, midi);
            synth.voice.note_on(midi);
        }
    }
}

pub fn on_key_up(ui: &mut UiState, synth: &mut Synth, key: sdl2::keyboard::Keycode) {
    if let Some(midi) = ui.key_notes.remove(&key) {
        synth.voice.note_off(midi);
    }
}

// ---------------- drawing ----------------

fn fill<T: RenderTarget>(canvas: &mut Canvas<T>, x: i32, y: i32, w: u32, h: u32, c: Color) {
    canvas.set_draw_color(c);
    let _ = canvas.fill_rect(Rect::new(x, y, w, h));
}

fn rect<T: RenderTarget>(canvas: &mut Canvas<T>, x: i32, y: i32, w: u32, h: u32, c: Color) {
    canvas.set_draw_color(c);
    let _ = canvas.draw_rect(Rect::new(x, y, w, h));
}

fn circle<T: RenderTarget>(canvas: &mut Canvas<T>, cx: i32, cy: i32, r: i32, c: Color) {
    canvas.set_draw_color(c);
    for dy in -r..=r {
        let dx = ((r * r - dy * dy) as f32).sqrt() as i32;
        let _ = canvas.fill_rect(Rect::new(cx - dx, cy + dy, (dx * 2 + 1) as u32, 1));
    }
}

fn ring<T: RenderTarget>(canvas: &mut Canvas<T>, cx: i32, cy: i32, r: i32, c: Color, bg: Color) {
    circle(canvas, cx, cy, r, c);
    circle(canvas, cx, cy, r - 3, bg);
}

fn line<T: RenderTarget>(canvas: &mut Canvas<T>, x1: i32, y1: i32, x2: i32, y2: i32, c: Color) {
    canvas.set_draw_color(c);
    let _ = canvas.draw_line((x1, y1), (x2, y2));
}

fn bezier<T: RenderTarget>(canvas: &mut Canvas<T>, x1: i32, y1: i32, x2: i32, y2: i32, c: Color) {
    // Cubic bezier with vertical tangents at both jacks: cables droop
    // naturally between the rows.
    let c1x = x1;
    let c1y = y1 + 46;
    let c2x = x2;
    let c2y = y2 - 46;
    let mut prev = (x1, y1);
    for i in 1..=24 {
        let t = i as f32 / 24.0;
        let mt = 1.0 - t;
        let x = (mt * mt * mt * x1 as f32
            + 3.0 * mt * mt * t * c1x as f32
            + 3.0 * mt * t * t * c2x as f32
            + t * t * t * x2 as f32) as i32;
        let y = (mt * mt * mt * y1 as f32
            + 3.0 * mt * mt * t * c1y as f32
            + 3.0 * mt * t * t * c2y as f32
            + t * t * t * y2 as f32) as i32;
        line(canvas, prev.0, prev.1, x, y, c);
        line(canvas, prev.0 + 1, prev.1, x + 1, y, c);
        prev = (x, y);
    }
}

fn section<T: RenderTarget>(canvas: &mut Canvas<T>, x: i32, w: i32, y: i32, accent: Color, title: &str) {
    fill(canvas, x, y, w as u32, 3, accent);
    draw_text(canvas, x + 2, y + 8, title, 1, INK);
}

fn draw_knob<T: RenderTarget>(canvas: &mut Canvas<T>, k: &KnobDef, params: &Params) {
    let v = params.get(k.id);
    let t = to_t(v, k.min, k.max, k.log);
    draw_text(
        canvas,
        k.x - text_width(k.label, 1) / 2,
        k.y - 32,
        k.label,
        1,
        DIM,
    );
    circle(canvas, k.x, k.y, 17, KNOB);
    // Indicator: -135 deg at t=0, +135 deg at t=1.
    let ang = -135.0f32 + t * 270.0;
    let rad = ang.to_radians();
    let ix = k.x + (rad.sin() * 13.0) as i32;
    let iy = k.y - (rad.cos() * 13.0) as i32;
    line(canvas, k.x, k.y, ix, iy, KNOB_MARK);
    circle(canvas, k.x, k.y, 3, KNOB_MARK);
    let val = fmt_value(k.id, v);
    draw_text(
        canvas,
        k.x - text_width(&val, 1) / 2,
        k.y + 22,
        &val,
        1,
        INK,
    );
}

fn draw_button<T: RenderTarget>(canvas: &mut Canvas<T>, b: &BtnDef, label: &str, on: bool) {
    fill(canvas, b.x, b.y, b.w as u32, b.h as u32, if on { BTN_ON } else { BTN });
    rect(canvas, b.x, b.y, b.w as u32, b.h as u32, INK);
    draw_text(
        canvas,
        b.x + (b.w - text_width(label, 1)) / 2,
        b.y + 9,
        label,
        1,
        INK,
    );
}

fn button_label(kind: BtnKind, synth: &Synth) -> String {
    let p = &synth.params;
    match kind {
        BtnKind::Osc1Wave => format!(
            "WAVE {}",
            match p.osc1_wave {
                Wave::Saw => "SAW",
                Wave::Square => "SQR",
                Wave::Triangle => "TRI",
            }
        ),
        BtnKind::Osc1Oct => format!("OCT {}", oct_label(p.osc1_oct)),
        BtnKind::Osc2Wave => format!(
            "WAVE {}",
            match p.osc2_wave {
                Wave::Saw => "SAW",
                Wave::Square => "SQR",
                Wave::Triangle => "TRI",
            }
        ),
        BtnKind::Osc2Oct => format!("OCT {}", oct_label(p.osc2_oct)),
        BtnKind::LfoWave => format!(
            "LFO {}",
            match p.lfo_wave {
                LfoWave::Triangle => "TRI",
                LfoWave::Sine => "SIN",
                LfoWave::Saw => "SAW",
                LfoWave::Square => "SQR",
            }
        ),
        BtnKind::ModDest => format!("DEST {}", p.mod_dest.label()),
        BtnKind::ArpOn => format!("ARP {}", if p.arp_enabled { "ON" } else { "OFF" }),
        BtnKind::ArpMode => format!("MODE {}", p.arp_mode.label()),
        BtnKind::ArpOct => format!("OCT {}", p.arp_oct),
        BtnKind::Clear => "CLEAR".to_string(),
        BtnKind::Demo => "DEMO".to_string(),
    }
}

fn button_on(kind: BtnKind, synth: &Synth) -> bool {
    match kind {
        BtnKind::ArpOn => synth.params.arp_enabled,
        _ => false,
    }
}

pub fn draw<T: RenderTarget>(canvas: &mut Canvas<T>, synth: &Synth, ui: &UiState) {
    let p = &synth.params;
    fill(canvas, 0, 0, W, H, PANEL);

    // Title bar
    draw_text(canvas, 16, 8, "GRANDMOTHER", 3, INK);
    draw_text(canvas, 16, 32, "SEMI-MODULAR ANALOG SYNTHESIZER", 1, DIM);
    for b in buttons() {
        if matches!(b.kind, BtnKind::Clear | BtnKind::Demo) {
            draw_button(canvas, &b, &button_label(b.kind, synth), false);
        }
    }

    // Sections row 1
    section(canvas, 16, 190, 52, Color::RGB(198, 58, 48), "OSC 1");
    section(canvas, 214, 190, 52, Color::RGB(214, 130, 30), "OSC 2");
    section(canvas, 412, 150, 52, Color::RGB(90, 140, 90), "MIXER");
    section(canvas, 570, 190, 52, Color::RGB(52, 120, 198), "FILTER");
    section(canvas, 768, 190, 52, Color::RGB(130, 90, 180), "ENVELOPES");
    section(canvas, 966, 178, 52, Color::RGB(220, 110, 40), "MODULATION");
    draw_text(canvas, 770, 84, "FILT EG", 1, DIM);
    draw_text(canvas, 770, 184, "AMP EG", 1, DIM);

    // Sections row 2
    section(canvas, 16, 150, 308, Color::RGB(40, 160, 160), "REVERB");
    section(canvas, 174, 230, 308, Color::RGB(200, 70, 130), "ARPEGGIATOR");
    section(canvas, 412, 150, 308, Color::RGB(90, 90, 90), "OUTPUT");
    draw_text(canvas, 570, 322, "KEYS: Z ROW PLAYS LOW OCTAVE, Q ROW HIGH OCTAVE,", 1, DIM);
    draw_text(canvas, 570, 338, "LEFT/RIGHT ARROWS SHIFT OCTAVE. DRAG KNOBS UP/DOWN.", 1, DIM);
    draw_text(canvas, 570, 354, "PATCH: CLICK AN OUTPUT JACK, THEN AN INPUT JACK.", 1, DIM);
    draw_text(canvas, 570, 370, "CLICK AN INPUT WITH NOTHING SELECTED TO UNPATCH.", 1, DIM);
    draw_text(canvas, 570, 386, "PATCHED CV INPUTS BREAK THEIR NORMAL, EXCEPT", 1, DIM);
    draw_text(canvas, 570, 402, "CUTOFF + VCA CV, WHICH ADD TO THE NORMAL.", 1, DIM);

    // Knobs
    for k in all_knobs() {
        draw_knob(canvas, &k, p);
    }

    // Buttons (non-title)
    for b in buttons() {
        if matches!(b.kind, BtnKind::Clear | BtnKind::Demo) {
            continue;
        }
        draw_button(canvas, &b, &button_label(b.kind, synth), button_on(b.kind, synth));
    }

    // Patch bay
    fill(canvas, 16, 448, 1128, 132, BAY_BG);
    // Cables first (under the jacks).
    for c in &synth.patch.cables {
        let a = out_jack_pos(c.out);
        let b = in_jack_pos(c.inp);
        bezier(canvas, a.x, a.y, b.x, b.y, cable_color(c.out));
    }
    // Pending cable from the selected output to the mouse.
    if let Some(o) = synth.patch.selected_out {
        let a = out_jack_pos(o);
        bezier(canvas, a.x, a.y, ui.mouse.0, ui.mouse.1, cable_color(o));
    }
    // Jacks.
    for (i, name) in OUT_JACKS.iter().enumerate() {
        let pos = out_jack_pos(i);
        ring(canvas, pos.x, pos.y, 12, JACK_RING, BAY_BG);
        circle(canvas, pos.x, pos.y, 5, JACK_HOLE);
        if synth.patch.selected_out == Some(i) {
            ring(canvas, pos.x, pos.y, 15, cable_color(i), BAY_BG);
        }
        draw_text(
            canvas,
            pos.x - text_width(name, 1) / 2,
            pos.y - 28,
            name,
            1,
            BAY_LABEL,
        );
    }
    for (i, name) in IN_JACKS.iter().enumerate() {
        let pos = in_jack_pos(i);
        ring(canvas, pos.x, pos.y, 12, JACK_RING, BAY_BG);
        circle(canvas, pos.x, pos.y, 5, JACK_HOLE);
        // Color the hole by the first cable into it, if any.
        if let Some(c) = synth.patch.cables.iter().find(|c| c.inp == i) {
            circle(canvas, pos.x, pos.y, 4, cable_color(c.out));
        }
        draw_text(
            canvas,
            pos.x - text_width(name, 1) / 2,
            pos.y + 18,
            name,
            1,
            BAY_LABEL,
        );
    }

    // On-screen keyboard
    let held: HashSet<u8> = synth.voice.held_notes().iter().copied().collect();
    let cur = synth.voice.current_note();
    let (whites, blacks) = key_rects();
    for (m, r) in &whites {
        let on = held.contains(m) || cur == Some(*m);
        fill(
            canvas,
            r.x,
            r.y,
            r.width(),
            r.height(),
            if on { KEY_ON } else { KEY_WHITE },
        );
        rect(canvas, r.x, r.y, r.width(), r.height(), INK);
        if *m % 12 == 0 {
            let name = format!("C{}", m / 12 - 1);
            draw_text(canvas, r.x + 4, r.y + r.height() as i32 - 16, &name, 1, DIM);
        }
    }
    for (m, r) in &blacks {
        let on = held.contains(m) || cur == Some(*m);
        fill(
            canvas,
            r.x,
            r.y,
            r.width(),
            r.height(),
            if on { KEY_ON } else { KEY_BLACK },
        );
    }

    // Footer status
    let note_name = match cur {
        Some(m) => {
            let names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
            format!("{}{}", names[(m % 12) as usize], m / 12 - 1)
        }
        None => "--".to_string(),
    };
    let status = format!(
        "NOTE {}  HELD {}  OCT SHIFT {:+}  CABLES {}  PATCH: {}",
        note_name,
        held.len(),
        ui.base_octave / 12,
        synth.patch.cables.len(),
        if synth.patch.selected_out.is_some() {
            "PICK AN INPUT"
        } else {
            "READY"
        }
    );
    draw_text(canvas, 16, 698, &status, 1, DIM);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{inp, out};

    #[test]
    fn knob_drag_changes_value() {
        let mut synth = Synth::new(44100.0);
        let mut ui = UiState::new();
        let before = synth.params.cutoff;
        on_mouse_down(&mut ui, &mut synth, 620, 130); // CUTOFF knob
        assert!(ui.drag.is_some());
        on_mouse_motion(&mut ui, &mut synth, 620, 30); // drag up
        assert!(synth.params.cutoff > before, "drag up raises cutoff");
        on_mouse_up(&mut ui, &mut synth, 620, 30);
        assert!(ui.drag.is_none());
    }

    #[test]
    fn jack_clicks_patch_and_unpatch() {
        let mut synth = Synth::new(44100.0);
        let mut ui = UiState::new();
        let o = out_jack_pos(out::LFO);
        let i = in_jack_pos(inp::VCF_CUTOFF);
        on_mouse_down(&mut ui, &mut synth, o.x, o.y);
        assert_eq!(synth.patch.selected_out, Some(out::LFO));
        on_mouse_down(&mut ui, &mut synth, i.x, i.y);
        assert_eq!(synth.patch.cables.len(), 1);
        assert_eq!(synth.patch.cables[0].out, out::LFO);
        assert_eq!(synth.patch.cables[0].inp, inp::VCF_CUTOFF);
        // Unselected click on the same input pops the cable.
        on_mouse_down(&mut ui, &mut synth, i.x, i.y);
        assert!(synth.patch.cables.is_empty());
    }

    #[test]
    fn computer_keys_play_and_release() {
        let mut synth = Synth::new(44100.0);
        let mut ui = UiState::new();
        on_key_down(&mut ui, &mut synth, sdl2::keyboard::Keycode::Z, false);
        assert!(synth.voice.held_notes().contains(&48));
        on_key_up(&mut ui, &mut synth, sdl2::keyboard::Keycode::Z);
        assert!(synth.voice.held_notes().is_empty());
    }

    #[test]
    fn piano_key_hit_testing() {
        assert_eq!(key_at(50, 620), Some(48)); // C3 white
        assert_eq!(key_at(96, 600), Some(49)); // C#3 black straddles the boundary
        assert_eq!(key_at(50, 500), None); // above the keyboard
    }
}
