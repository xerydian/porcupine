use crate::audio_shell::AudioGenerator;
use mi_plaits_dsp::dsp::voice::{Modulations, Patch, Voice};

use crate::keyboard_utils::ch;

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
    note: Option<f32>,
    model: Option<usize>,
    harmonic: Option<f32>,
    timbre: Option<f32>,
    morph: Option<f32>,
    decay: Option<f32>,
    gate_length: Option<f64>,
    is_awaiting_note: bool
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

pub struct Synth<'a> {

    block_size: usize,

    voice: Voice<'a>,
    pub patch: Patch,
    pub modulations: Modulations,

    // volume: f32,
    // balance: f32,
    note: f32,
    transpose: f32,
    rec_transpose: f32,
    info_octave: i16,

    target_harmonic: f32, target_timbre: f32, target_morph:f32, target_bend: f32, target_vibrato_amount: f32,
    smooth_harmonic: f32, smooth_timbre: f32, smooth_morph:f32, smooth_bend: f32, smooth_vibrato_amount: f32,

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

            voice: Voice::new(&std::alloc::System, block_size),
            patch: Patch::default(),
            modulations: Modulations::default(),

            // volume: 1.0,
            // balance: 0.0,
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

    fn print_info (&self) {
        clearscreen::clear().unwrap();
        println!("[ F1-F2 ]    Model: {}", ENGINE_DESCRIPIONS[self.patch.engine]);
        println!("[ F3-F4 ] Harmonic: {}", (10. * self.target_harmonic).round() / 10.);
        println!("[ F5-F6 ]   Timbre: {}", (10. * self.target_timbre).round() / 10.);
        println!("[ F7-F8 ]    Morph: {}", (10. * self.target_morph).round() / 10.);
        println!("[ F9-10 ]    Decay: {}", (10. * self.patch.decay).round() / 10.);
        println!("");
        println!("                           [ {} Rest ] [ {} {} ] [ BKSP Clear ]", 
            ch(VK_LeftBracket), ch(VK_RightBracket),
            if self.seq_status != SeqStatus::Recording {"Record"} else {"  Undo"},
        );
        println!("+-----------------------------------------------------------------+");
        println!("|    {}   {}   {}   {}   {}   {}   {}       3   4   5   6   7   8   9    |", 
            ch(VK_S), ch(VK_D), ch(VK_F), ch(VK_G), ch(VK_H), ch(VK_J), ch(VK_K),
        );
        println!("|  {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}  |",
            ch(VK_Z), ch(VK_X), ch(VK_C), ch(VK_V), ch(VK_B), ch(VK_N), ch(VK_M), ch(VK_Comma),
            ch(VK_W), ch(VK_E), ch(VK_R), ch(VK_T), ch(VK_Y), ch(VK_U), ch(VK_I), ch(VK_O),
        );
        println!("+-----------------------------------------------------------------+");
        println!("[ LSHIFT  > ] Pitch Bend    [ LCTRL ] Vibrato   [ . - ]  Octave: {} ", self.info_octave);
        println!("                     [   SPACE_BAR    {:?}   ]",
            if self.seq_status != SeqStatus::Play {"Play"} else {"Stop"}
        );
        self.print_sequence();
        println!("                 Transport: {:?}", self.seq_status);
        println!("[   Up / Down  ] Tempo: {} BPM", self.tempo.round());
        println!("[ Left / Right ] Gate length: {}", (10. * self.gate_length).round() / 10.);
        println!("");
        println!("(Press [Esc] to exit)");
        // println!("")
        // println!("(debug) Rec Transpose: {}", self.rec_transpose);
    }

    pub fn print_sequence (&self) {
        if self.seq_status == SeqStatus::Recording || !self.seq_notes.is_empty() {
            println!("");
            println!("Sequence: ");
        }
        if self.seq_status == SeqStatus::Recording && self.seq_notes.is_empty() {
            println!("[ ]");
        }
        for (i, sq) in self.seq_notes.iter().enumerate() {
            if i % 8 == 0 { print!("["); }
            match sq {
                SeqStep { is_awaiting_note: true, ..  } => print!("   MOD  "),
                SeqStep { note: None, model: None, harmonic: None, timbre: None, morph: None, gate_length: None, decay: None, .. } 
                    => print!(" (     )"),
                SeqStep { note: None, .. } => print!(" ( MOD )"),
                SeqStep { note: Some(note), model: None, harmonic: None, timbre: None, morph: None, gate_length: None, decay: None, .. } 
                    => print!(" ({:>+0width$.prec$})", note, width=5, prec=1),
                SeqStep { note: Some(note), .. } => print!(" MOD({:>+0width$.prec$})", note, width=5, prec=1),
            };
            if i % 8 == 7 || (i+1) == self.seq_notes.len() { println!(" ]"); }
        }
        println!("");
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
        self.voice.init();
        self.print_info();
    }

    fn output_sound(&mut self, samples_left: &mut [f32], samples_right: &mut [f32]) {
        let mut out = vec![0.0; self.block_size];
        let mut aux = vec![0.0; self.block_size];

        self.voice.render(&self.patch, &self.modulations, &mut out, &mut aux);

        // let mut mix = vec![0.0; BLOCK_SIZE];
        // for frame in 0..BLOCK_SIZE {
        //     mix[frame] = (out[frame] * (1.0 - self.balance) + aux[frame] * self.balance) * self.volume;
        // } 
        samples_left.clone_from_slice(&out);
        samples_right.clone_from_slice(&out);
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
        // Print Info
        match event.event_type {
            KeyPress(
                Key::F1 | Key::F2 | Key::F3 | Key::F4 | Key::F5 | Key::F6 | Key::F7 | Key::F8 | Key::F9 | Key::F10 |
                Key::Space | Key::RightBracket | Key::UpArrow | Key::DownArrow | Key::Dot | Key::Minus | Key::LeftArrow | Key::RightArrow
            ) => {
                self.print_info();
            },
            KeyPress(key) if notes.contains_key(&key) && self.seq_status == SeqStatus::Recording => self.print_info(),
            KeyPress(Key::LeftBracket | Key::Backspace) if self.seq_status == SeqStatus::Recording => self.print_info(),
            _ => ()
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
    static ref notes: HashMap<Key, f32> = HashMap::from(<Vec<(Key, f32)> as TryInto<[(Key, f32); 30]>>::try_into([
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


const VK_Alt: u32 = 164;
const VK_AltGr: u32 = 165;
const VK_Backspace: u32 = 0x08;
const VK_CapsLock: u32 = 20;
const VK_ControlLeft: u32 = 162;
const VK_ControlRight: u32 = 163;
const VK_Delete: u32 = 46;
const VK_DownArrow: u32 = 40;
const VK_End: u32 = 35;
const VK_Escape: u32 = 27;
const VK_F1: u32 = 112;
const VK_F10: u32 = 121;
const VK_F11: u32 = 122;
const VK_F12: u32 = 123;
const VK_F2: u32 = 113;
const VK_F3: u32 = 114;
const VK_F4: u32 = 115;
const VK_F5: u32 = 116;
const VK_F6: u32 = 117;
const VK_F7: u32 = 118;
const VK_F8: u32 = 119;
const VK_F9: u32 = 120;
const VK_Home: u32 = 36;
const VK_LeftArrow: u32 = 37;
const VK_MetaLeft: u32 = 91;
const VK_PageDown: u32 = 34;
const VK_PageUp: u32 = 33;
const VK_Return: u32 = 0x0D;
const VK_RightArrow: u32 = 39;
const VK_ShiftLeft: u32 = 160;
const VK_ShiftRight: u32 = 161;
const VK_Space: u32 = 32;
const VK_Tab: u32 = 0x09;
const VK_UpArrow: u32 = 38;
const VK_PrintScreen: u32 = 44;
const VK_ScrollLock: u32 = 145;
const VK_Pause: u32 = 19;
const VK_NumLock: u32 = 144;
const VK_BackQuote: u32 = 192;
const VK_1: u32 = 49;
const VK_2: u32 = 50;
const VK_3: u32 = 51;
const VK_4: u32 = 52;
const VK_5: u32 = 53;
const VK_6: u32 = 54;
const VK_7: u32 = 55;
const VK_8: u32 = 56;
const VK_9: u32 = 57;
const VK_0: u32 = 48;
const VK_Minus: u32 = 189;
const VK_Equal: u32 = 187;
const VK_Q: u32 = 81;
const VK_W: u32 = 87;
const VK_E: u32 = 69;
const VK_R: u32 = 82;
const VK_T: u32 = 84;
const VK_Y: u32 = 89;
const VK_U: u32 = 85;
const VK_I: u32 = 73;
const VK_O: u32 = 79;
const VK_P: u32 = 80;
const VK_LeftBracket: u32 = 219;
const VK_RightBracket: u32 = 221;
const VK_A: u32 = 65;
const VK_S: u32 = 83;
const VK_D: u32 = 68;
const VK_F: u32 = 70;
const VK_G: u32 = 71;
const VK_H: u32 = 72;
const VK_J: u32 = 74;
const VK_K: u32 = 75;
const VK_L: u32 = 76;
const VK_SemiColon: u32 = 186;
const VK_Quote: u32 = 222;
const VK_BackSlash: u32 = 220;
const VK_IntlBackslash: u32 = 226;
const VK_Z: u32 = 90;
const VK_X: u32 = 88;
const VK_C: u32 = 67;
const VK_V: u32 = 86;
const VK_B: u32 = 66;
const VK_N: u32 = 78;
const VK_M: u32 = 77;
const VK_Comma: u32 = 188;
const VK_Dot: u32 = 190;
const VK_Slash: u32 = 191;
const VK_Insert: u32 = 45;
const VK_KpMinus: u32 = 109;
const VK_KpPlus: u32 = 107;
const VK_KpMultiply: u32 = 106;
const VK_KpDivide: u32 = 111;
const VK_Kp0: u32 = 96;
const VK_Kp1: u32 = 97;
const VK_Kp2: u32 = 98;
const VK_Kp3: u32 = 99;
const VK_Kp4: u32 = 100;
const VK_Kp5: u32 = 101;
const VK_Kp6: u32 = 102;
const VK_Kp7: u32 = 103;
const VK_Kp8: u32 = 104;
const VK_Kp9: u32 = 105;
const VK_KpDelete: u32 = 110;