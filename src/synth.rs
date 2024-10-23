use crate::audio_shell::AudioGenerator;
use mi_plaits_dsp::dsp::voice::{Modulations, Patch, Voice};

use crate::keyboard_utils::{ch, VK_Comma, VK_LeftBracket, VK_RightBracket, VK_B, VK_C, VK_D, VK_E, VK_F, VK_G, VK_H, VK_I, VK_J, VK_K, VK_M, VK_N, VK_O, VK_R, VK_S, VK_T, VK_U, VK_V, VK_W, VK_X, VK_Y, VK_Z};

use std::vec;
use std::collections::HashMap;
use linked_hash_set::LinkedHashSet;
use lazy_static::lazy_static;

use std::mem::transmute;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use spin_sleep;

use rdev::{Event, Key};
use rdev::EventType::{KeyPress, KeyRelease};

use num::{Num, NumCast, ToPrimitive};

#[derive(Debug, PartialEq, Eq)]
pub enum SeqStatus {
    Recording, Play, Stop
}

#[derive(Default, Clone, Copy, Debug)]
pub struct SeqStep {
    pub note: Option<f32>,
    pub model: Option<usize>,
    pub harmonic: Option<f32>,
    pub timbre: Option<f32>,
    pub morph: Option<f32>,
    pub decay: Option<f32>,
    pub gate_length: Option<f64>,
    pub is_awaiting_note: bool
}

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
    pressed_set: LinkedHashSet<Key>,
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
            let is_playing = { 
                let synth = arc_synth.lock().unwrap();
                synth.seq_status == SeqStatus::Play && !synth.seq_notes.is_empty()
            };
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
                            if synth.seq_status != SeqStatus::Play {
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
        if self.seq_notes.is_empty() || !(self.seq_notes.last().unwrap().is_awaiting_note) { 
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
    
    fn process_events(&mut self, event: Event) {
        let note_ptr: &mut f32 = unsafe { transmute(&mut self.note) };
        let rec_transpose = self.rec_transpose;
        let engine_ptr: &mut usize = unsafe { transmute(&mut self.patch.engine) };
        let harmonic_ptr: &mut f32 = unsafe { transmute(&mut self.target_harmonic) };
        let timbre_ptr: &mut f32 = unsafe { transmute(&mut self.target_timbre) };
        let morph_ptr: &mut f32 = unsafe { transmute(&mut self.target_morph) };
        let decay_ptr: &mut f32 = unsafe { transmute(&mut self.patch.decay) };
        let gate_length_ptr: &mut f64 = unsafe { transmute(&mut self.gate_length) };

        match event.event_type {
            KeyPress(key) if notes.contains_key(&key) => {
                let note = *notes.get(&key).unwrap() + rec_transpose;
                self.recording_wrapper(Param::Note, note_ptr, |n: &mut f32| *n = note);
                self.modulations.trigger = 1.0;
                self.modulations.level = 1.0;
                self.pressed_set.insert(key);
            }
            // Handle note release
            KeyRelease(key) if notes.contains_key(&key) => {
                self.pressed_set.remove(&key);
                if self.pressed_set.is_empty() {
                    self.modulations.trigger = 0.0;
                    self.modulations.level = 0.0;
                } 
                else {
                    self.note = *notes.get(self.pressed_set.back().unwrap()).unwrap();
                }
            }
            KeyPress(Key::Escape) => std::process::exit(0),

            // Model
            KeyPress(Key::F1) => self.recording_wrapper (Param::Model, engine_ptr, dec_usize),
            KeyPress(Key::F2) => self.recording_wrapper (Param::Model, engine_ptr, inc_usize),

            // Harmonics
            KeyPress(Key::F3) => self.recording_wrapper (Param::Harmonic, harmonic_ptr, dec_f32),
            KeyPress(Key::F4) => self.recording_wrapper (Param::Harmonic, harmonic_ptr, inc_f32),
            
            // Timbre
            KeyPress(Key::F5) => self.recording_wrapper (Param::Timbre, timbre_ptr, dec_f32),
            KeyPress(Key::F6) => self.recording_wrapper (Param::Timbre, timbre_ptr, inc_f32),

            // Morph
            KeyPress(Key::F7) => self.recording_wrapper (Param::Morph, morph_ptr, dec_f32),
            KeyPress(Key::F8) => self.recording_wrapper (Param::Morph, morph_ptr, inc_f32),

            // Decay
            KeyPress(Key::F9)  => self.recording_wrapper (Param::Decay, decay_ptr, dec_f32),
            KeyPress(Key::F10) => self.recording_wrapper (Param::Decay, decay_ptr, inc_f32),

            // Gate Length
            KeyPress(Key::LeftArrow) => self.recording_wrapper (Param::GateLength, gate_length_ptr, dec_f64),
            KeyPress(Key::RightArrow) => self.recording_wrapper (Param::GateLength, gate_length_ptr, inc_f64),
            
            // Transpose
            KeyPress(Key::Dot) => { 
                self.transpose  -= 12.;
                self.rec_transpose -= 12.;
                self.info_octave -= 1;
            }
            KeyPress(Key::Minus) => { 
                self.transpose  += 12.;
                self.rec_transpose += 12.;
                self.info_octave += 1;
            }

            // PitchBend
            KeyPress(Key::ShiftLeft)     => self.target_bend = BEND_NEGATIVE,
            KeyPress(Key::IntlBackslash) => self.target_bend = BEND_POSITIVE,
            KeyRelease(Key::ShiftLeft | Key::IntlBackslash) => self.target_bend = BEND_NEUTRAL,

            // Vibrato
            KeyPress(Key::ControlLeft)   => self.target_vibrato_amount = VIBRATO_DEPTH,
            KeyRelease(Key::ControlLeft) => self.target_vibrato_amount = 0.,

            // Sequencer
            KeyPress(Key::LeftBracket) => {
                self.rec_transpose = 0.;
                self.recording_wrapper(Param::Rest, note_ptr, |_| ());
            }
            KeyPress(Key::RightBracket) => {
                self.rec_transpose = 0.;
                if self.seq_status == SeqStatus::Recording {
                    self.seq_notes.pop();
                }
                else {
                    self.seq_status = SeqStatus::Recording;
                }
            }
            KeyPress(Key::Backspace) => {
                self.seq_notes = Vec::default();
            }
            KeyPress(Key::Space) => { 
                self.seq_status = match self.seq_status {
                    SeqStatus::Recording => SeqStatus::Play,
                    SeqStatus::Play => SeqStatus::Stop,
                    SeqStatus::Stop => SeqStatus::Play,
                }
            }

            // Tempo
            KeyPress(Key::DownArrow) => self.tempo -= 4.0,
            KeyPress(Key::UpArrow) => self.tempo += 4.0,

            // Debug
            // KeyPress(key) => println!("{:?}", key),

            // Not implemented: self.balance, self.volume, self.patch.lpg_colour
            _ => {}
        }
    }
}

const ENGINE_DESCRIPIONS: [&str; 24] = [
    "Virtual analog VCF (01)",
    "Phase distortion (02)",
    "6-OP A (03)",
    "6-OP B (04)",
    "6-OP C (05)",
    "Waveterrain (06)",
    "String machine (07)",
    "Chiptune (08)",
    "Virtual analog (09)",
    "Waveshaping (10)",
    "Fm (11)",
    "Grain (12)",
    "Additive (13)",
    "Wavetable (14)",
    "Chord (15)",
    "Speech (16)",
    "Swarm (17)",
    "Noise (18)",
    "Particle (19)",
    "String (20)",
    "Modal (21)",
    "Bass drum (22)",
    "Snare drum (23)",
    "Hi-hat (24)",
];

lazy_static! {
    pub static ref notes: HashMap<Key, f32> = HashMap::from(<Vec<(Key, f32)> as TryInto<[(Key, f32); 30]>>::try_into([
        (Key::Num3,23.), (Key::Num4,26.), (Key::Num5,29.), (Key::Num6,32.), (Key::Num7,35.), (Key::Num8,38.), (Key::Num9,41.),
        (Key::KeyW,22.), (Key::KeyE,25.), (Key::KeyR,28.), (Key::KeyT,31.), (Key::KeyY,34.), (Key::KeyU,37.), (Key::KeyI,40.), (Key::KeyO,43.), 
        (Key::KeyS,01.), (Key::KeyD,04.), (Key::KeyF,07.), (Key::KeyG,10.), (Key::KeyH,13.), (Key::KeyJ,16.), (Key::KeyK,19.),
        (Key::KeyZ,00.), (Key::KeyX,03.), (Key::KeyC,06.), (Key::KeyV,09.), (Key::KeyB,12.), (Key::KeyN,15.), (Key::KeyM,18.), (Key::Comma,21.), 
    ].into_iter()
        .map(|(a, b)| (a, 12.0*b/22.0))
        .collect::<Vec<(Key, f32)>>()).unwrap()
    );
}

fn recover_param<T> (parameter: &mut Option<T>, back_up: Option<T>) {
    if parameter.is_none() {
        *parameter = back_up;
    }
}

fn dec_usize (value: &mut usize) { *value = (*value - 1).max(0); }
fn inc_usize (value: &mut usize) { *value = (*value + 1).min(23); }
fn dec_f32 (value: &mut f32) { *value = (*value - 0.1).max(0.); }
fn inc_f32 (value: &mut f32) { *value = (*value + 0.1).min(1.); }
fn dec_f64 (value: &mut f64) { *value = (*value - 0.1).max(0.); }
fn inc_f64 (value: &mut f64) { *value = (*value + 0.1).min(1.); }


