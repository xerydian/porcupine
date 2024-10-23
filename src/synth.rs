use crate::audio_shell::AudioGenerator;
use mi_plaits_dsp::dsp::voice::{Modulations, Patch, Voice};

use std::f32::consts::PI;
use std::{cmp, vec};
use std::collections::HashMap;
use linked_hash_set::LinkedHashSet;
use lazy_static::lazy_static;

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::thread;

use rdev::{Event, Key};
use rdev::EventType::{KeyPress, KeyRelease};

#[derive(Debug, PartialEq, Eq)]
pub enum SeqStatus {
    Rec, Play, Stop
}

const BEND_NEGATIVE: f32 = -1.818181818181818;
const BEND_NEUTRAL : f32 = 0.;
const BEND_POSITIVE: f32 = 1.818181818181818;

const BEND_SMOOTH_FACTOR:     f32 = 0.000_01;
const BEND_SMOOTH_FACTOR_INV: f32 = 1. - BEND_SMOOTH_FACTOR;
const VIBRATO_PRESS_SMOOTH_FACTOR:       f32 = 0.000_002;
const VIBRATO_PRESS_SMOOTH_FACTOR_INV:   f32 = 1. - VIBRATO_PRESS_SMOOTH_FACTOR;
const VIBRATO_RELEASE_SMOOTH_FACTOR:     f32 = 0.001;
const VIBRATO_RELEASE_SMOOTH_FACTOR_INV: f32 = 1. - VIBRATO_RELEASE_SMOOTH_FACTOR;
const PARAM_SMOOTH_FACTOR:     f32 = 0.1;
const PARAM_SMOOTH_FACTOR_INV: f32 = 1. - PARAM_SMOOTH_FACTOR;

const VIBRATO_DEPTH: f32 = 0.6;

pub struct Synth<'a> {

    block_size: usize,

    voice: Voice<'a>,
    pub patch: Patch,
    pub modulations: Modulations,

    // volume: f32,
    // balance: f32,
    note: f32,
    transpose: f32,
    info_octave: i32,

    target_harmonic: f32, target_timbre: f32, target_morph:f32, target_bend: f32, target_vibrato: f32,
    smooth_harmonic: f32, smooth_timbre: f32, smooth_morph:f32, smooth_bend: f32, smooth_vibrato: f32,

    pub tempo: f64,
    pub gate_length: f64,
    pressed_set: LinkedHashSet<Key>,
    pub seq_notes: Vec<f32>,
    pub seq_status: SeqStatus,
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
            info_octave: 5,

            target_harmonic: 0.5, target_timbre: 0.5, target_morph:0.5, target_bend: BEND_NEUTRAL, target_vibrato: 0.,
            smooth_harmonic: 0.5, smooth_timbre: 0.5, smooth_morph:0.5, smooth_bend: BEND_NEUTRAL, smooth_vibrato: 0.,

            tempo: 120.,
            gate_length: 0.5,
            pressed_set: LinkedHashSet::default(),
            seq_notes: Vec::default(),
            seq_status: SeqStatus::Stop,
        }
    }

    fn print_info (&self) {
        clearscreen::clear().unwrap();
        println!("[ F1-F2 ]    Model: {}", ENGINE_DESCRIPIONS[self.patch.engine]);
        println!("[ F3-F4 ] Harmonic: {}", (10. * self.target_harmonic).round() / 10.);
        println!("[ F5-F6 ]   Timbre: {}", (10. * self.target_timbre).round() / 10.);
        println!("[ F7-F8 ]    Morph: {}", (10. * self.target_morph).round() / 10.);
        println!("[ F9-10 ]    Decay: {}", (10. * self.patch.decay).round() / 10.);
        println!("                          [ '? Rec ] [ ¡¿ {:?} ] [ Up/Down: Tempo ]", 
            if self.seq_status != SeqStatus::Play {SeqStatus::Play} else {SeqStatus::Stop}
        );
        println!("+-----------------------------------------------------------------+    Octave  ");
        println!("|    s   d   f   g   h   j   k       3   4   5   6   7   8   9    |  +--------+");
        println!("|  z   x   c   v   b   n   m   ,   w   e   r   t   y   u   i   o  |  |  .  -  |");
        println!("+-----------------------------------------------------------------+  +--------+");
        println!("[ LSHIFT  > ] Pitch Bend   [ LCTRL ] Vibrato ");
        println!("");
        println!("Octave: {}", self.info_octave);
        println!("Sequencer: {:?}", self.seq_status);
        println!("Tempo: {} BPM", self.tempo.round());
        println!("");
        println!("(Press [Esc] to exit)");
    }

    pub fn sequencer (arc_synth: Arc<Mutex<Synth>>) {
        loop {
            let is_playing = { 
                let synth = arc_synth.lock().unwrap();
                synth.seq_status == SeqStatus::Play && !synth.seq_notes.is_empty()
            };
            if is_playing {
                let seq_notes = {  arc_synth.lock().unwrap().seq_notes.to_owned() };
                'play_loop: loop {
                    for note in seq_notes.iter() {    
                        let (sec_gate_on, sec_gate_off) = { 
                            let synth = arc_synth.lock().unwrap();
                            let sec_per_8th = 30. / synth.tempo;
                            let sec_gate_on = sec_per_8th * synth.gate_length;
                            let sec_gate_off = sec_per_8th - sec_gate_on;
                            (sec_gate_on, sec_gate_off)
                        };
                        {
                            let mut synth = arc_synth.lock().unwrap();
                            if synth.seq_status != SeqStatus::Play {
                                break 'play_loop;
                            }
                            synth.note = *note;
                            synth.modulations.trigger = 1.;
                            synth.modulations.level = 1.;
                        }
                        thread::sleep(Duration::from_secs_f64(sec_gate_on));
                        {
                            let mut synth = arc_synth.lock().unwrap();
                            synth.modulations.trigger = 0.;
                            synth.modulations.level = 0.;                    
                        }
                        thread::sleep(Duration::from_secs_f64(sec_gate_off));
                    }
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn control_loop (arc_synth: Arc<Mutex<Synth>>) {
        loop {
            let mut s = arc_synth.lock().unwrap();
            let time  = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_millis() as f32 / 1000.;

            s.smooth_bend = (BEND_SMOOTH_FACTOR * s.target_bend) + (BEND_SMOOTH_FACTOR_INV * s.smooth_bend);
            s.smooth_vibrato = f32::min(
                (VIBRATO_PRESS_SMOOTH_FACTOR * s.target_vibrato) + (VIBRATO_PRESS_SMOOTH_FACTOR_INV * s.smooth_vibrato),
                (VIBRATO_RELEASE_SMOOTH_FACTOR * s.target_vibrato) + (VIBRATO_RELEASE_SMOOTH_FACTOR_INV * s.smooth_vibrato),
            );
            let vibrato =  s.smooth_vibrato * (PI * time * 10.).sin();
            s.patch.note = s.note + s.transpose + s.smooth_bend + vibrato;
            
            s.smooth_harmonic = (PARAM_SMOOTH_FACTOR * s.target_harmonic) + (PARAM_SMOOTH_FACTOR_INV * s.smooth_harmonic);
            s.smooth_timbre   = (PARAM_SMOOTH_FACTOR * s.target_timbre)   + (PARAM_SMOOTH_FACTOR_INV * s.smooth_timbre);
            s.smooth_morph    = (PARAM_SMOOTH_FACTOR * s.target_morph)    + (PARAM_SMOOTH_FACTOR_INV * s.smooth_morph);   
            s.patch.harmonics = s.smooth_harmonic;
            s.patch.timbre    = s.smooth_timbre;
            s.patch.morph     = s.smooth_morph;
        }
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
        match event.event_type {
            KeyPress(key) if notes.contains_key(&key) => {
                let note = *notes.get(&key).unwrap();
                self.note = note;
                self.modulations.trigger = 1.0;
                self.modulations.level = 1.0;
                self.pressed_set.insert(key);
                if self.seq_status == SeqStatus::Rec {
                    self.seq_notes.push(note);
                }
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
            KeyPress(Key::F1) => self.patch.engine = if self.patch.engine > 1 {self.patch.engine - 1} else {0},
            KeyPress(Key::F2) => self.patch.engine = cmp::min(self.patch.engine + 1, 23),

            // Harmonics
            KeyPress(Key::F3) => self.target_harmonic = (self.target_harmonic - 0.1).max(0.),
            KeyPress(Key::F4) => self.target_harmonic = (self.target_harmonic + 0.1).min(1.),
            
            // Timbre
            KeyPress(Key::F5) => self.target_timbre = (self.target_timbre - 0.1).max(0.),
            KeyPress(Key::F6) => self.target_timbre = (self.target_timbre + 0.1).min(1.),

            // Morph
            KeyPress(Key::F7) => self.target_morph = (self.target_morph - 0.1).max(0.),
            KeyPress(Key::F8) => self.target_morph = (self.target_morph + 0.1).min(1.),

            // Decay
            KeyPress(Key::F9)  => self.patch.decay = (self.patch.decay - 0.1).max(0.),
            KeyPress(Key::F10) => self.patch.decay = (self.patch.decay + 0.1).min(1.),
            
            // Transpose
            KeyPress(Key::Dot) => { 
                self.transpose  -= 12.;
                self.note -= 12.;
                self.info_octave -= 1;
            }
            KeyPress(Key::Minus) => { 
                self.transpose  += 12.;
                self.note += 12.;
                self.info_octave += 1;
            }

            // PitchBend
            KeyPress(Key::ShiftLeft)     => self.target_bend = BEND_NEGATIVE,
            KeyPress(Key::IntlBackslash) => self.target_bend = BEND_POSITIVE,
            KeyRelease(Key::ShiftLeft | Key::IntlBackslash) => self.target_bend = BEND_NEUTRAL,

            // Vibrato
            KeyPress(Key::ControlLeft)   => self.target_vibrato = VIBRATO_DEPTH,
            KeyRelease(Key::ControlLeft) => self.target_vibrato = 0.,

            // Sequencer
            KeyPress(Key::LeftBracket) => {
                if self.seq_status != SeqStatus::Rec {
                    self.seq_status = SeqStatus::Rec;
                    self.seq_notes = Vec::default();
                }
            }
            KeyPress(Key::RightBracket) => { 
                self.seq_status = match self.seq_status {
                    SeqStatus::Rec => SeqStatus::Play,
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
                Key::LeftBracket | Key::RightBracket | Key::UpArrow | Key::DownArrow | Key::Dot | Key::Minus
            ) => {
                self.print_info();
            }
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
