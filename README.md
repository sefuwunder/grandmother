# grandmother

A Grandmother-style semi-modular analog synthesizer in Rust + SDL2.
Two VCOs, a Moog-style 4-pole ladder filter, dual ADSR envelopes, an LFO,
an arpeggiator, spring-style reverb, and a 20-point patch bay with
normalized routing — all evaluated per sample, so audio-rate patching
(oscillator into filter cutoff, noise into pitch) works.

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

## Patch bay

11 outputs, 9 inputs. Click an output jack, then an input jack to patch.
Click an input with nothing selected to remove its newest cable. CLEAR
removes all cables; DEMO loads a starting patch (LFO into filter cutoff,
OSC2 triangle into the filter input).

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
| AMP EG | | | | |
| VCF OUT | | | | |

**Switched** inputs: patching breaks the normalized connection.
**Additive** inputs (VCF CUTOFF, VCA CV): patched CV adds on top of the
normal modulation, which keeps working. Multiple cables into one input
sum. CV is 1 V/octave; audio signals are ±1. Feedback paths (VCF OUT
into VCF IN, LFO into its own rate) resolve with a one-sample delay.

## Controls

- **OSC 1 / OSC 2**: wave buttons (SAW/SQR, plus TRI on OSC 2), octave
  buttons (32'/16'/8'/4'), FINE and PULSE W knobs.
- **MIXER**: OSC 1, OSC 2, NOISE levels.
- **FILTER**: CUTOFF (log), RESO, ENV AMT (bipolar), KB TRACK.
- **ENVELOPES**: FILT EG and AMP EG, each ADSR.
- **MODULATION**: LFO RATE (log), LFO wave, MOD AMT, MOD DEST
  (PITCH / FILTER / PULSE W).
- **REVERB**: MIX (spring-tank-style DSP network, not a physical tank).
- **ARPEGGIATOR**: on/off, UP / DOWN / UP-DN / RAND, RATE, OCT (1–3).
- **OUTPUT**: MASTER, GLIDE.

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
envelopes, the LFO, the reverb tail, the patch bay, the arpeggiator,
the voice (silence, release, patch-changes-sound, LFO-triggered gate),
and the UI handlers (knob drag, jack patching, computer keys).

## Next steps

- 16-step sequencer (the hardware's other half; the arp ships first).
- Presets (save/load patches + knob states).
- MIDI input (currently computer keyboard + on-screen keys only).
