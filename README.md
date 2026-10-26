# grandmother

A Grandmother-style semi-modular analog synthesizer in Rust + SDL2.
Two VCOs, a Moog-style 4-pole ladder filter, dual ADSR envelopes, an LFO,
an arpeggiator, a 16-step sequencer, spring-style reverb, 8 preset slots,
MIDI input, and a 43-point patch bay with normalized routing — all
evaluated per sample, so audio-rate patching (oscillator into filter
cutoff, noise into pitch) works.

An homage to the Moog Grandmother's layout and signal flow, written from
scratch. Not affiliated with Moog Music.

## Signal flow

```
KB CV/GATE ──> VCO 1 ──┐
               VCO 2 ──┤
               NOISE ──┴──> MIXER ──> VCF IN ──> LADDER FILTER ──> VCA ──> REVERB ──> OUT
                              (normal)      ^  ^  ^                ^
                                            EG  KB  MOD           AMP EG
```

Monophonic, last-note priority, with glide (portamento).

Utilities outside the main path, both patchable: a 1-pole high-pass
filter (HP CUTOFF knob) and a bipolar attenuator (ATT knob, center =
silence).

## Patch bay

19 outputs, 20 inputs, and a parallel-wired 4-jack mult — 43 points
covering every function of the hardware's front-panel bay, plus the
variant's extras (per-waveform oscillator outputs, the second envelope
output, a dedicated noise output, OSC2 pulse-width CV, and a
keyboard-gate override input). Click an output jack, then an input jack
to patch. Click a mult jack with nothing selected to use it as a
source, or after selecting an output to feed the mult bus. Click an
input with nothing selected to remove its newest cable. CLEAR removes
all cables; DEMO loads a starting patch (LFO into filter cutoff, OSC2
triangle into the filter input).

| Output | | Input | Normal | Kind |
|---|---|---|---|---|
| KB CV | | OSC1 PITCH | KB CV | switched |
| GATE | | OSC2 PITCH | KB CV | switched |
| LFO | | OSC1 PW | — | switched |
| OSC1 SAW | | OSC2 PW | — | switched |
| OSC1 PULSE | | VCF IN | mixer | switched |
| OSC2 SAW | | VCF CUTOFF | EG + KB track | additive |
| OSC2 TRI | | VCA CV | amp EG | additive |
| NOISE | | GATE IN | KB gate | switched |
| FILT EG | | LFO RATE | — | switched |
| AMP EG | | SYNC IN | — | edge |
| VCF OUT | | LIN FM IN | — | additive |
| KB VEL | | MIX OSC1 | OSC1 wave | switched |
| S/H OUT | | MIX OSC2 | OSC2 wave | switched |
| OSC1 WAVE | | MIX NOISE | noise | switched |
| OSC2 WAVE | | HP IN | — | switched |
| MIXER OUT | | ATT IN | — | switched |
| HP OUT | | ENV AMT | ENV AMT knob | additive |
| ATT OUT | | TRIG IN | KB gate | switched |
| -ENV OUT | | VCA IN | filter output | switched |
| | | REVERB IN | VCA output | switched |

**Switched** inputs: patching breaks the normalized connection.
**Additive** inputs (VCF CUTOFF, VCA CV, LIN FM, ENV AMT): patched CV
adds on top of the normal modulation, which keeps working. Multiple
cables into one input sum. CV is 1 V/octave; audio signals are ±1.
Feedback paths (VCF OUT into VCF IN, LFO into its own rate) resolve
with a one-sample delay, as does everything through the mult bus (the
four mult jacks share one bus; multiple outputs into it sum).

Gate precedence: TRIG IN replaces the keyboard gate when patched (as
on the hardware); otherwise GATE IN replaces it; otherwise the keyboard
gate drives the envelopes. S/H samples the noise source at each LFO
cycle start. KB VEL is 1 while any key is held (computer keys have no
velocity). LIN FM adds linear Hz to OSC2 (±1 signal ≈ ±100 Hz).
-ENV OUT is the inverted amp envelope.

Note on the hardware's "41 patch points": the front-panel bay is 35
points (18 inputs, 13 outputs, 4 mult); the other 6 are rear-panel
jacks (arp/seq clock in/out, on/off, reset, Eurorack out, reverb out).
This build covers all 35 front-panel functions; the rear jacks are not
included — there is no sequencer transport or rear panel yet.

## Controls

- **OSC 1 / OSC 2**: wave buttons (SAW/SQR, plus TRI on OSC 2), octave
  buttons (32'/16'/8'/4'), FINE and PULSE W knobs.
- **MIXER**: OSC 1, OSC 2, NOISE levels.
- **FILTER**: CUTOFF (log), RESO, ENV AMT (bipolar), KB TRACK.
- **ENVELOPES**: FILT EG and AMP EG, each ADSR.
- **MODULATION**: LFO RATE (log), LFO wave, MOD AMT, MOD DEST
  (PITCH / FILTER / PULSE W).
- **UTILITIES**: ATT (bipolar, center = silence), HP CUTOFF (log).
- **REVERB**: MIX (spring-tank-style DSP network, not a physical tank).
- **ARPEGGIATOR**: on/off, UP / DOWN / UP-DN / RAND, RATE, OCT (1–3).
- **SEQUENCER**: 16 steps, SEQ RATE (steps/sec), PLAY / REC / GATE /
  ACCENT / SEQ CLR.
- **OUTPUT**: MASTER, GLIDE.

## Sequencer

16 steps, each with a note, a gate, and an accent. PLAY starts from
step 0; while playing, the sequencer owns the voice — rest steps
silence it rather than falling through to held keys, and the
arpeggiator is overridden. Accented steps play at full velocity, plain
steps at 0.7 (visible on the KB VEL output). Click a step to select it;
GATE and ACCENT edit the selected step, live, even while playing. REC
arms note entry from step 0: played notes (computer keys, screen keys,
or MIDI) fill steps and advance, switching off after step 16. Note
entry only happens while REC is armed, so casual playing never
overwrites steps. The default pattern is a C4 pulse with accents on
steps 1, 5, 9, 13.

## Presets

8 slots in the title bar. Click a slot to load it; SAVE writes the
current state (all knobs, patch cables, sequencer steps) to the
selected slot. Presets live in `~/.grandmother/presets.txt` as plain
text — one `[slot n]` section per occupied slot — so they stay
readable and hand-editable. Loading a preset stops the sequencer.

## MIDI input

Connects to the first available ALSA input port and reconnects
automatically when ports appear or change; the footer shows the port
name, or `-` when none is present. Notes play with velocity (which
also feeds KB VEL), pitch bend covers ±2 semitones, and the mod wheel
(CC 1) scales all LFO modulation depths. Build needs libasound2-dev;
runtime needs libasound2.

## Playing

- On-screen keyboard: two octaves, C3–B4. Click to play.
- Computer keyboard: Z row = lower octave, Q row = upper octave,
  Left/Right arrows shift octave.
- Drag knobs vertically. Esc quits.

## Build and run

```sh
export CARGO_HOME="$HOME/workspace/.cargo" RUSTUP_HOME="$HOME/workspace/.rustup"
export PATH="$HOME/workspace/.cargo/bin:$PATH"
cargo run --release
```

Headless checks (no window or audio device needed):

```sh
cargo run --release -- --smoke 3        # render check, exit 0 on success
cargo run --release -- --render out.wav # demo phrase to WAV
cargo run --release -- --shot shot.bmp  # faceplate to BMP
```

## Tests

```sh
cargo test --release
```

Covers oscillators, the ladder (DC, cutoff sweep, bounded resonance),
the 1-pole high-pass, envelopes, the LFO, the reverb tail, the patch
bay (including mult routing), the arpeggiator, the sequencer (step
advance, accents, rests, REC entry), presets (round-trip, sections,
lenient parsing), MIDI message parsing and application, the voice
(silence, release, patch-changes-sound, LFO-triggered gate, trigger in,
mixer channel in, VCA in, attenuator, S/H, sequencer playback and
rest-override, pitch bend), and the UI handlers (knob drag, jack
patching including mult jacks, step selection, computer keys).

## Next steps

- Song mode / longer sequences (the hardware holds 256-note sequences).
- Preset names (slots are numbered today).
- MIDI clock sync.
- MIDI output.
