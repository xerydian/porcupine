// Reference: https://github.com/sourcebox/mi-plaits-dsp-rs/blob/firmware-1.2/examples/midi control.rs
// Reference: https://github.com/Narsil/rdev/blob/main/examples/listen.rs

use lazy_static::lazy_static;

use std::{cmp, vec};
use std::sync::{Arc, Mutex};

use std::collections::HashMap;
use linked_hash_set::LinkedHashSet;

use mi_plaits_dsp::dsp::voice::{Modulations, Patch, Voice};

mod audio_shell;
use crate::audio_shell::{AudioShell, AudioGenerator};

use rdev::{listen, Event, Key};
use rdev::EventType::{KeyPress, KeyRelease};

const SAMPLE_RATE: u32 = 48000;
const BLOCK_SIZE: usize = 2048;

fn main() {
    let instr = Arc::new(Mutex::new(Synth::new()));
    let _shell = AudioShell::spawn(SAMPLE_RATE, BLOCK_SIZE, instr.clone());
    if let Err(error) = listen(get_callback(instr)) {
        println!("Error: {:?}", error)
    }
}

struct Synth<'a> {
    voice: Voice<'a>,
    patch: Patch,
    modulations: Modulations,
    // volume: f32,
    // balance: f32,
    octave: i32,
    transpose: f32,
    tempo: f32,
    pressed_set: LinkedHashSet<Key>,
    sequence: Vec<f32>,
    seq_status: SeqStatus,
}

impl<'a> Synth<'a> {
    pub fn new() -> Self {
        Self {
            voice: Voice::new(&std::alloc::System, BLOCK_SIZE),
            patch: Patch::default(),
            modulations: Modulations::default(),
            // volume: 1.0,
            // balance: 0.0,
            octave: 5,
            transpose: 48.0,
            tempo: 120.,
            pressed_set: LinkedHashSet::default(),
            sequence: Vec::default(),
            seq_status: SeqStatus::Stop,
        }
    }
}


fn get_callback(audio_generator: Arc<Mutex<impl AudioGenerator>>) -> impl FnMut(Event) {
    move |event: Event| {
        audio_generator.lock().unwrap().process_events(event);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum SeqStatus {
    Load, Play, Stop
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

    fn process(&mut self, samples_left: &mut [f32], samples_right: &mut [f32]) {
        let mut out = vec![0.0; BLOCK_SIZE];
        let mut aux = vec![0.0; BLOCK_SIZE];

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
                let note = *notes.get(&key).unwrap() + self.transpose;
                self.patch.note = note;
                self.modulations.trigger = 1.0;
                self.modulations.level = 1.0;
                self.pressed_set.insert(key);
                if self.seq_status == SeqStatus::Load {
                    self.sequence.push(note);
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
                    self.patch.note = *notes.get(self.pressed_set.back().unwrap()).unwrap() + self.transpose;
                }
            }
            KeyPress(Key::Escape) => std::process::exit(0),

            // Model
            KeyPress(Key::F1) => {
                self.patch.engine = if self.patch.engine > 1 {self.patch.engine - 1} else {0};
            }
            KeyPress(Key::F2) => {
                self.patch.engine = cmp::min(self.patch.engine + 1, 23);
            }

            // Harmonics
            KeyPress(Key::F3) => {
                self.patch.harmonics = (self.patch.harmonics - 0.1).max(0.);
            }
            KeyPress(Key::F4) => {
                self.patch.harmonics = (self.patch.harmonics + 0.1).min(1.);
            }
            
            // Timbre
            KeyPress(Key::F5) => {
                self.patch.timbre = (self.patch.timbre - 0.1).max(0.);
            }
            KeyPress(Key::F6) => {
                self.patch.timbre = (self.patch.timbre + 0.1).min(1.);
            }

            // Morph
            KeyPress(Key::F7) => { 
                self.patch.morph = (self.patch.morph - 0.1).max(0.);
            }
            KeyPress(Key::F8) => { 
                self.patch.morph = (self.patch.morph + 0.1).min(1.);
            }

            // Decay
            KeyPress(Key::F9) => { 
                self.patch.decay = (self.patch.decay - 0.1).max(0.);
            }
            KeyPress(Key::F10) => { 
                self.patch.decay = (self.patch.decay + 0.1).min(1.);
            }
            
            // Transpose
            KeyPress(Key::Dot) => { 
                self.transpose  -= 12.;
                self.patch.note -= 12.;
                self.octave -= 1;
            }
            KeyPress(Key::Minus) => { 
                self.transpose  += 12.;
                self.patch.note += 12.;
                self.octave += 1;
            }

            // Sequencer
            KeyPress(Key::LeftBracket) => {
                if self.seq_status != SeqStatus::Load {
                    self.seq_status = SeqStatus::Load;
                    self.sequence = Vec::default();
                }
            }
            KeyPress(Key::RightBracket) => { 
                self.seq_status = match self.seq_status {
                    SeqStatus::Load => SeqStatus::Play,
                    SeqStatus::Play => SeqStatus::Stop,
                    SeqStatus::Stop => SeqStatus::Play,
                }
            }

            // Tempo
            KeyPress(Key::DownArrow) => {
                self.tempo -= 4.0;
            }
            KeyPress(Key::UpArrow) => { 
                self.tempo += 4.0;
            }

            // Debug
            // KeyPress(key) => println!("{:?}", key),

            // Not implemented: self.balance, self.volume, self.patch.lpg_colour
            _ => {}
        }
        // debug
        match event.event_type {
            KeyPress(
                Key::F1 | Key::F2 | Key::F3 | Key::F4 | Key::F5 | Key::F6 | Key::F7 | Key::F8 | Key::F9 | Key::F10 |
                Key::LeftBracket | Key::RightBracket | Key::UpArrow | Key::DownArrow
            ) => {
                self.print_info();
            }
            _ => ()
        }
    }
}

impl<'a> Synth<'a> {
    fn print_info (&self) {
        clearscreen::clear().unwrap();
        println!("[ F1-F2 ]    Model: {}", ENGINE_DESCRIPIONS[self.patch.engine]);
        println!("[ F3-F4 ] Harmonic: {}", (10. * self.patch.harmonics).round() / 10.);
        println!("[ F5-F6 ]   Timbre: {}", (10. * self.patch.timbre).round() / 10.);
        println!("[ F7-F8 ]    Morph: {}", (10. * self.patch.morph).round() / 10.);
        println!("[ F9-10 ]    Decay: {}", (10. * self.patch.decay).round() / 10.);
        println!("                         [ '? Load ] [ ¡¿ {:?} ] [ Up/Down: Tempo ]", 
            if self.seq_status != SeqStatus::Play {SeqStatus::Play} else {SeqStatus::Stop}
        );
        println!("+-----------------------------------------------------------------+    Octave  ");
        println!("|    s   d   f   g   h   j   k       3   4   5   6   7   8   9    |  +--------+");
        println!("|  z   x   c   v   b   n   m   ,   w   e   r   t   y   u   i   o  |  |  .  -  |");
        println!("+-----------------------------------------------------------------+  +--------+");
        println!("");
        println!("Octave: {}", self.octave);
        println!("Sequencer: {:?}", self.seq_status);
        println!("Tempo: {} BPM", self.tempo.round());
        println!("");
        println!("(Press [Esc] to exit)");
    }

    
}

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