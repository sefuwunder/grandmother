//! The patch bay: 20 jacks (11 outputs, 9 inputs) with normalized routing.
//!
//! Routing rule, matching semi-modular hardware:
//! - SWITCHED inputs: patching a cable breaks the normalized connection.
//!   Multiple cables into one input sum.
//! - ADDITIVE inputs (VCF CUTOFF, VCA CV): patched CV adds on top of the
//!   normalized modulation, which keeps working.
//!
//! Interaction: click an output jack to select it (it lights up), then
//! click an input jack to patch. Clicking an input with no output
//! selected removes its most recent cable. Clicking a selected output
//! again deselects it.

pub const OUT_JACKS: [&str; 11] = [
    "KB CV", "GATE", "LFO", "OSC1 SAW", "OSC1 PULSE", "OSC2 SAW", "OSC2 TRI", "NOISE",
    "FILT EG", "AMP EG", "VCF OUT",
];

pub const IN_JACKS: [&str; 9] = [
    "OSC1 PITCH",
    "OSC2 PITCH",
    "OSC1 PW",
    "OSC2 PW",
    "VCF IN",
    "VCF CUTOFF",
    "VCA CV",
    "GATE IN",
    "LFO RATE",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InKind {
    /// Patching breaks the normalized connection.
    Switched,
    /// Patching adds to the normalized modulation.
    Additive,
}

pub const IN_KINDS: [InKind; 9] = [
    InKind::Switched, // OSC1 PITCH <- KB CV
    InKind::Switched, // OSC2 PITCH <- KB CV
    InKind::Switched, // OSC1 PW <- none
    InKind::Switched, // OSC2 PW <- none
    InKind::Switched, // VCF IN <- mixer
    InKind::Additive, // VCF CUTOFF (EG + KB track keep working)
    InKind::Additive, // VCA CV (amp EG keeps working)
    InKind::Switched, // GATE IN <- KB gate
    InKind::Switched, // LFO RATE <- none
];

/// Input jack indices, for the voice engine.
pub mod inp {
    pub const OSC1_PITCH: usize = 0;
    pub const OSC2_PITCH: usize = 1;
    pub const OSC1_PW: usize = 2;
    pub const OSC2_PW: usize = 3;
    pub const VCF_IN: usize = 4;
    pub const VCF_CUTOFF: usize = 5;
    pub const VCA_CV: usize = 6;
    pub const GATE_IN: usize = 7;
    pub const LFO_RATE: usize = 8;
}

/// Output jack indices, for the voice engine.
pub mod out {
    pub const KB_CV: usize = 0;
    pub const GATE: usize = 1;
    pub const LFO: usize = 2;
    pub const OSC1_SAW: usize = 3;
    pub const OSC1_PULSE: usize = 4;
    pub const OSC2_SAW: usize = 5;
    pub const OSC2_TRI: usize = 6;
    pub const NOISE: usize = 7;
    pub const FILT_EG: usize = 8;
    pub const AMP_EG: usize = 9;
    pub const VCF_OUT: usize = 10;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cable {
    pub out: usize,
    pub inp: usize,
}

#[derive(Clone, Debug, Default)]
pub struct PatchBay {
    pub cables: Vec<Cable>,
    pub selected_out: Option<usize>,
}

impl PatchBay {
    pub fn new() -> Self {
        PatchBay {
            cables: Vec::new(),
            selected_out: None,
        }
    }

    pub fn clear(&mut self) {
        self.cables.clear();
        self.selected_out = None;
    }

    /// Is any cable patched into this input?
    pub fn patched(&self, inp: usize) -> bool {
        self.cables.iter().any(|c| c.inp == inp)
    }

    /// Output indices of all cables into this input, in patch order.
    pub fn cables_into(&self, inp: usize) -> Vec<usize> {
        self.cables
            .iter()
            .filter(|c| c.inp == inp)
            .map(|c| c.out)
            .collect()
    }

    /// Click an output jack: select it, or deselect if already selected.
    pub fn click_out(&mut self, out: usize) {
        if self.selected_out == Some(out) {
            self.selected_out = None;
        } else {
            self.selected_out = Some(out);
        }
    }

    /// Click an input jack: with an output selected, patch (or unpatch if
    /// that exact cable exists). With nothing selected, remove the most
    /// recent cable into this input, if any.
    pub fn click_in(&mut self, inp: usize) {
        if let Some(o) = self.selected_out.take() {
            if let Some(pos) = self.cables.iter().position(|c| c.out == o && c.inp == inp) {
                self.cables.remove(pos);
            } else {
                self.cables.push(Cable { out: o, inp });
            }
        } else if let Some(pos) = self.cables.iter().rposition(|c| c.inp == inp) {
            self.cables.remove(pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_unpatch_and_pop() {
        let mut p = PatchBay::new();
        p.click_out(2); // LFO
        p.click_in(5); // VCF CUTOFF
        assert_eq!(p.cables, vec![Cable { out: 2, inp: 5 }]);
        assert!(p.patched(5));
        // Clicking the same pair again unpatches.
        p.click_out(2);
        p.click_in(5);
        assert!(p.cables.is_empty());
        // Two cables into one input; unselected click pops the last.
        p.click_out(2);
        p.click_in(5);
        p.click_out(8);
        p.click_in(5);
        assert_eq!(p.cables_into(5), vec![2, 8]);
        p.click_in(5);
        assert_eq!(p.cables_into(5), vec![2]);
        p.clear();
        assert!(p.cables.is_empty() && p.selected_out.is_none());
    }

    #[test]
    fn out_click_toggles_selection() {
        let mut p = PatchBay::new();
        p.click_out(3);
        assert_eq!(p.selected_out, Some(3));
        p.click_out(3);
        assert_eq!(p.selected_out, None);
        p.click_out(3);
        p.click_out(4);
        assert_eq!(p.selected_out, Some(4));
    }
}
