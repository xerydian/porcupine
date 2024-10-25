use crate::audio_shell::AudioGenerator;
use mi_plaits_dsp::dsp::voice::{Modulations, Patch, Voice};

use std::vec;
use linked_hash_set::LinkedHashSet;

use std::mem::transmute;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use spin_sleep;

use num::{Num, NumCast, ToPrimitive};
use crate::synth::seq::{SeqStatus, SeqStep, TRANSPORT};
use crate::synth::util::*;

use super::seq::SEQUENCER;

pub enum Param  {
    Note, Rest, Model, Harmonic, Timbre, Morph, Decay, GateLength
}

const BEND_NEUTRAL : f32 = 0.;
const BEND_POSITIVE: f32 = 4./22. * 12.;
const BEND_NEGATIVE: f32 = -4./22. * 12.;

const BEND_SMOOTH_FACTOR:     f32 = 0.000_01;
const BEND_SMOOTH_FACTOR_INV: f32 = 1. - BEND_SMOOTH_FACTOR;
const VIBRATO_PRESS_SMOOTH_FACTOR:       f32 = 0.000_002;
const VIBRATO_PRESS_SMOOTH_FACTOR_INV:   f32 = 1. - VIBRATO_PRESS_SMOOTH_FACTOR;
const VIBRATO_RELEASE_SMOOTH_FACTOR:     f32 = 0.001;
const VIBRATO_RELEASE_SMOOTH_FACTOR_INV: f32 = 1. - VIBRATO_RELEASE_SMOOTH_FACTOR;
const PARAM_SMOOTH_FACTOR:     f32 = 0.001;
const PARAM_SMOOTH_FACTOR_INV: f32 = 1. - PARAM_SMOOTH_FACTOR;

const VIBRATO_DEPTH: f32 = 0.6;
const VIBRATO_RATE: f32 = std::f32::consts::PI * 10.;

#[derive(Debug)]
pub struct Synth<'a> {

    block_size: usize,

    pub synth_engine: Voice<'a>,
    pub patch: Patch,
    pub modulations: Modulations,

    pub volume: f32,
    pub pan: f32,
    pub balance: f32,

    note: f32,
    transpose: f32,
    rec_transpose: f32,
    pub info_octave: i16,

    pub target_harmonic: f32, smooth_harmonic: f32,
    pub target_timbre: f32, smooth_timbre: f32,
    pub target_morph:f32, smooth_morph:f32,
    target_vibrato_amount: f32, smooth_vibrato_amount: f32,
    target_bend: f32, smooth_bend: f32, 

    pub tempo: f64,
    pub gate_length: f64,
    pressed_set: LinkedHashSet<i32>,
    pub seq_notes: Vec<SeqStep>,
    pub seq_status: SeqStatus,
    first_step_backup: SeqStep,
}

impl<'a> Synth<'a> {
    pub fn new(block_size: usize) -> Self {
        Self {
            block_size,

            synth_engine: Voice::new(&std::alloc::System, block_size),
            patch: Patch::default(),
            modulations: Modulations::default(),

            volume: 1.0,
            balance: 0.0,
            pan: 0.5,
            note: 48.0,
            transpose: 48.0,
            rec_transpose: 0.,
            info_octave: 5,

            target_harmonic: 0.5, target_timbre: 0.5, target_morph:0.5, target_bend: BEND_NEUTRAL, target_vibrato_amount: 0.,
            smooth_harmonic: 0.5, smooth_timbre: 0.5, smooth_morph:0.5, smooth_bend: BEND_NEUTRAL, smooth_vibrato_amount: 0.,

            tempo: 120.,
            gate_length: 0.5,
            pressed_set: LinkedHashSet::default(),
            seq_notes: Vec::default(),
            seq_status: SeqStatus::Stop,

            first_step_backup: SeqStep::default(), 
        }
    }

    pub fn init (&mut self) {
        self.synth_engine.init();
    }

    pub fn sequencer_loop (arc_synth: Arc<Mutex<Synth>>) {
        loop {
            println!("debug sequencer!!!");
            let is_playing = TRANSPORT.condvar.wait(
                TRANSPORT.is_playing.lock().unwrap()
            ).unwrap().to_owned();
            if is_playing {
                let seq_notes = {  arc_synth.lock().unwrap().seq_notes.clone() };
                'play_loop: loop {
                    for step in seq_notes.iter() {    
                        let delta = Instant::now();
                        let (sec_gate_on, sec_gate_off) = { 
                            let synth = arc_synth.lock().unwrap();
                            let gate_length = step.gate_length.unwrap_or(synth.gate_length);
                            let sec_per_8th = 30. / synth.tempo;
                            let sec_gate_on = sec_per_8th * gate_length;
                            let sec_gate_off = sec_per_8th - sec_gate_on;
                            (sec_gate_on, sec_gate_off)
                        };
                        {
                            let mut synth = arc_synth.lock().unwrap();
                            if SEQUENCER.lock().unwrap().is_stopped() {
                                break 'play_loop;
                            }
                            if let Some(note) = step.note {
                                synth.note = note;
                                synth.modulations.trigger = 1.;
                                synth.modulations.level = 1.;
                            }
                            if let Some(model) = step.model { synth.patch.engine = model; }
                            if let Some(harmonic) = step.harmonic { 
                                synth.smooth_harmonic = harmonic;
                                synth.target_harmonic = harmonic;
                            }
                            if let Some(timbre) = step.timbre { 
                                synth.smooth_timbre = timbre;
                                synth.target_timbre = timbre;
                            }
                            if let Some(morph) = step.morph { 
                                synth.smooth_morph = morph;
                                synth.target_morph = morph;
                            }
                            if let Some(decay) = step.decay { 
                                synth.patch.decay = decay;
                            }
                        }
                        spin_sleep::sleep(Duration::from_secs_f64(sec_gate_on) - delta.elapsed()); 
                        let delta = Instant::now();
                        {
                            let mut synth = arc_synth.lock().unwrap();
                            synth.modulations.trigger = 0.;
                            synth.modulations.level = 0.;                    
                        }
                        spin_sleep::sleep(Duration::from_secs_f64(sec_gate_off) - delta.elapsed());
                    }
                }
            }
        }
    }

    pub fn control_loop (arc_synth: Arc<Mutex<Synth>>) {
        loop {
            println!("debug loop!!!");
            let _is_playing = TRANSPORT.condvar.wait(
                TRANSPORT.is_playing.lock().unwrap()
            ).unwrap().to_owned();
            let mut s = arc_synth.lock().unwrap();
            let time  = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_millis() as f32 / 1000.;

            s.smooth_bend = (BEND_SMOOTH_FACTOR * s.target_bend) + (BEND_SMOOTH_FACTOR_INV * s.smooth_bend);
            s.smooth_vibrato_amount = f32::min(
                (VIBRATO_PRESS_SMOOTH_FACTOR * s.target_vibrato_amount) + (VIBRATO_PRESS_SMOOTH_FACTOR_INV * s.smooth_vibrato_amount),
                (VIBRATO_RELEASE_SMOOTH_FACTOR * s.target_vibrato_amount) + (VIBRATO_RELEASE_SMOOTH_FACTOR_INV * s.smooth_vibrato_amount),
            );
            let vibrato =  s.smooth_vibrato_amount * (VIBRATO_RATE * time).sin();
            s.patch.note = s.note + s.transpose + s.smooth_bend + vibrato;
            
            s.smooth_harmonic = (PARAM_SMOOTH_FACTOR * s.target_harmonic) + (PARAM_SMOOTH_FACTOR_INV * s.smooth_harmonic);
            s.smooth_timbre   = (PARAM_SMOOTH_FACTOR * s.target_timbre)   + (PARAM_SMOOTH_FACTOR_INV * s.smooth_timbre);
            s.smooth_morph    = (PARAM_SMOOTH_FACTOR * s.target_morph)    + (PARAM_SMOOTH_FACTOR_INV * s.smooth_morph);   
            s.patch.harmonics = s.smooth_harmonic;
            s.patch.timbre    = s.smooth_timbre;
            s.patch.morph     = s.smooth_morph;
        }
    }

    fn update_first_step (&mut self, param: &Param) {
        if self.seq_notes.is_empty() {
            self.first_step_backup = SeqStep {
                model: Some(self.patch.engine),
                harmonic: Some(self.target_harmonic),
                timbre: Some(self.target_timbre),
                morph: Some(self.target_morph),
                decay: Some(self.patch.decay),
                gate_length: Some(self.gate_length),
                ..Default::default()
            }
        }
        if self.seq_notes.len() > 1 {
            let first_step = self.seq_notes.first_mut().unwrap();
            match param {
                Param::Model => recover_param(&mut first_step.model, self.first_step_backup.model),
                Param::Harmonic => recover_param(&mut first_step.harmonic, self.first_step_backup.harmonic),
                Param::Timbre => recover_param(&mut first_step.timbre, self.first_step_backup.timbre),
                Param::Morph => recover_param(&mut first_step.morph, self.first_step_backup.morph),
                Param::Decay => recover_param(&mut first_step.decay, self.first_step_backup.decay),
                Param::GateLength => recover_param(&mut first_step.gate_length, self.first_step_backup.gate_length),
                _ => (),
            };
        }
    }
    fn record_step<T: ToPrimitive + NumCast> (&mut self, param: Param, value: T) -> () {
        if self.seq_notes.is_empty() || !self.seq_notes.last().unwrap().is_awaiting_note {
            self.seq_notes.push(SeqStep::default());
        }
        let last_step = self.seq_notes.last_mut().unwrap();
        let value = NumCast::from(value).unwrap();
        match param {
            Param::Rest => last_step.note = None,
            Param::Note => last_step.note = Some(value),
            Param::Model => last_step.model = Some(NumCast::from(value).unwrap()),
            Param::Harmonic => last_step.harmonic = Some(value),
            Param::Timbre => last_step.timbre = Some(value),
            Param::Morph => last_step.morph = Some(value),
            Param::Decay => last_step.decay = Some(value),
            Param::GateLength => last_step.gate_length = Some(NumCast::from(value).unwrap()),
        };
        match param {
            Param::Rest | Param::Note => last_step.is_awaiting_note = false,
            _ => last_step.is_awaiting_note = true,
        }
    }

    fn recording_wrapper<T> (&mut self, param: Param, value: &mut T, update: impl FnOnce(&mut T) -> ()) 
    where T: Num + ToPrimitive + NumCast + Copy {
        let is_recording = self.seq_status == SeqStatus::Recording;
        if is_recording { self.update_first_step(&param);}
        update(value);
        if is_recording { self.record_step(param, *value);}
    }

    
    pub fn render(&mut self, out: &mut [f32], aux: &mut [f32]) {
        self.synth_engine.render(&self.patch, &self.modulations, out, aux);
    }

    pub fn note_on(&mut self, key: i32) {
        let note_ptr: &mut f32 = unsafe { transmute(&mut self.note) };
        let note = key2note(key) + self.rec_transpose;
        self.recording_wrapper(Param::Note, note_ptr, |n: &mut f32| *n = note);
        self.modulations.trigger = 1.0;
        self.modulations.level = 1.0;
        self.pressed_set.insert(key);
    }

    pub fn note_off(&mut self, key: i32) {    
        self.pressed_set.remove(&key);
        if self.pressed_set.is_empty() {
            self.modulations.trigger = 0.0;
            self.modulations.level = 0.0;
        } 
        else {
            self.note = key2note(*self.pressed_set.back().unwrap());
        }
    }

    pub fn add_rest (&mut self) {
        self.rec_transpose = 0.;
        let note_ptr: &mut f32 = unsafe { transmute(&mut self.note) };
        self.recording_wrapper(Param::Rest, note_ptr, |_| ());
    }

    pub fn model_up (&mut self) {
        let engine_ptr: &mut usize = unsafe { transmute(&mut self.patch.engine) };
        self.recording_wrapper (Param::Model, engine_ptr, inc_usize);
    }
    
    pub fn model_down (&mut self) {
        let engine_ptr: &mut usize = unsafe { transmute(&mut self.patch.engine) };
        self.recording_wrapper (Param::Model, engine_ptr, dec_usize);
    }
    
    pub fn harmonic_up (&mut self) {
        let harmonic_ptr: &mut f32 = unsafe { transmute(&mut self.target_harmonic) };
        self.recording_wrapper (Param::Harmonic, harmonic_ptr, inc_f32);
    }
    
    pub fn harmonic_down (&mut self) {
        let harmonic_ptr: &mut f32 = unsafe { transmute(&mut self.target_harmonic) };
        self.recording_wrapper (Param::Harmonic, harmonic_ptr, dec_f32);
    }

    pub fn timbre_up (&mut self) {
        let timbre_ptr: &mut f32 = unsafe { transmute(&mut self.target_timbre) };
        self.recording_wrapper (Param::Timbre, timbre_ptr, inc_f32);
    }

    pub fn timbre_down (&mut self) {
        let timbre_ptr: &mut f32 = unsafe { transmute(&mut self.target_timbre) };
        self.recording_wrapper (Param::Timbre, timbre_ptr, dec_f32);
    }

    pub fn morph_up (&mut self) {
        let morph_ptr: &mut f32 = unsafe { transmute(&mut self.target_morph) };
        self.recording_wrapper (Param::Morph, morph_ptr, inc_f32);
    }

    pub fn morph_down (&mut self) {
        let morph_ptr: &mut f32 = unsafe { transmute(&mut self.target_morph) };
        self.recording_wrapper (Param::Morph, morph_ptr, dec_f32);
    }

    pub fn decay_up (&mut self) {
        let decay_ptr: &mut f32 = unsafe { transmute(&mut self.patch.decay) };
        self.recording_wrapper (Param::Decay, decay_ptr, inc_f32);
    }

    pub fn decay_down (&mut self) {
        let decay_ptr: &mut f32 = unsafe { transmute(&mut self.patch.decay) };
        self.recording_wrapper (Param::Decay, decay_ptr, dec_f32);
    }           

    pub fn gate_length_up(&mut self) {
        let gate_length_ptr: &mut f64 = unsafe { transmute(&mut self.gate_length) };
        self.recording_wrapper (Param::GateLength, gate_length_ptr, inc_f64);
    }

    pub fn gate_length_down(&mut self) {
        let gate_length_ptr: &mut f64 = unsafe { transmute(&mut self.gate_length) };
        self.recording_wrapper (Param::GateLength, gate_length_ptr, dec_f64);
    }

    pub fn transpose_up (&mut self) { 
        self.transpose     += 12.;
        self.rec_transpose += 12.;
        self.info_octave   += 1;
    }

    pub fn transpose_down (&mut self) { 
        self.transpose     -= 12.;
        self.rec_transpose -= 12.;
        self.info_octave   -= 1;
    }

    pub fn start_recording_or_undo_last (&mut self) {
        let mut seq = SEQUENCER.lock().unwrap();
        self.rec_transpose = 0.;
        if seq.is_recording() {
            self.seq_notes.pop();
        }
        else {
            seq.start_recording();
        }
    }
    pub fn clear_notes (&mut self) {
        self.seq_notes = Vec::default();
    }


    pub fn pitch_bend_positive (&mut self) { self.target_bend = BEND_POSITIVE; }
    pub fn pitch_bend_neutral  (&mut self) { self.target_bend = BEND_NEUTRAL; }
    pub fn pitch_bend_negative (&mut self) { self.target_bend = BEND_NEGATIVE; }

    pub fn vibrato_on  (&mut self) { self.target_vibrato_amount = VIBRATO_DEPTH; }
    pub fn vibrato_off (&mut self) { self.target_vibrato_amount = 0.; }

}


impl<'a> AudioGenerator for Synth<'a> {
    fn init(&mut self, _block_size: usize) {
        self.patch.engine = 8;
        self.patch.harmonics = 0.5;
        self.patch.timbre = 0.5;
        self.patch.morph = 0.5;
        self.patch.lpg_colour = 1.0;
        self.modulations.trigger_patched = true;
        self.modulations.level_patched = true;
        self.synth_engine.init();
    }

    fn output_sound(&mut self, samples_left: &mut [f32], samples_right: &mut [f32]) {
        let mut out = vec![0.0; self.block_size];
        let mut aux = vec![0.0; self.block_size];

        self.synth_engine.render(&self.patch, &self.modulations, &mut out, &mut aux);

        let mut mix = vec![0.0; self.block_size];
        for frame in 0..self.block_size {
            mix[frame] = (out[frame] * (1.0 - self.balance) + aux[frame] * self.balance) * self.volume;
        } 
        samples_left.clone_from_slice(&mix);
        samples_right.clone_from_slice(&mix);
    }
}

fn recover_param<T> (parameter: &mut Option<T>, back_up: Option<T>) {
    if parameter.is_none() {
        *parameter = back_up;
    }
}

fn key2note (key: i32) -> f32 {
    12.0*(key as f32)/22.0
}
