// Reference: https://github.com/sourcebox/mi-plaits-dsp-rs/blob/firmware-1.2/examples/midi_control.rs
// Reference: https://github.com/Narsil/rdev/blob/main/examples/listen.rs

use lazy_static::lazy_static;

use std::cmp;
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
    let instr = Arc::new(Mutex::new(App::new()));
    let _shell = AudioShell::spawn(SAMPLE_RATE, BLOCK_SIZE, instr.clone());
    if let Err(error) = listen(get_callback(instr)) {
        println!("Error: {:?}", error)
    }
}

struct App<'a> {
    voice: Voice<'a>,
    patch: Patch,
    modulations: Modulations,
    volume: f32,
    balance: f32,
    pressed_set: LinkedHashSet<Key>,
}

impl<'a> App<'a> {
    pub fn new() -> Self {
        Self {
            voice: Voice::new(&std::alloc::System, BLOCK_SIZE),
            patch: Patch::default(),
            modulations: Modulations::default(),
            volume: 1.0,
            balance: 0.0,
            pressed_set: LinkedHashSet::default(),
        }
    }
}


fn get_callback(audio_generator: Arc<Mutex<impl AudioGenerator>>) -> impl FnMut(Event) {
    move |event: Event| {
        audio_generator.lock().unwrap().process_events(event);
    }
}

impl<'a> AudioGenerator for App<'a> {
    fn init(&mut self, _block_size: usize) {
        self.patch.engine = 0;
        self.patch.harmonics = 0.5;
        self.patch.timbre = 0.5;
        self.patch.morph = 0.5;
        self.modulations.trigger_patched = true;
        self.modulations.level_patched = true;
        self.voice.init();
        self.debug_sound();
    }

    fn process(&mut self, samples_left: &mut [f32], samples_right: &mut [f32]) {
        let mut out = vec![0.0; BLOCK_SIZE];
        let mut aux = vec![0.0; BLOCK_SIZE];

        self.voice.render(&self.patch, &self.modulations, &mut out, &mut aux);

        let mut mix = vec![0.0; BLOCK_SIZE];

        for frame in 0..BLOCK_SIZE {
            mix[frame] = (out[frame] * (1.0 - self.balance) + aux[frame] * self.balance) * self.volume;
        }

        samples_left.clone_from_slice(&mix);
        samples_right.clone_from_slice(&mix);
    }

    fn process_events(&mut self, event: Event) {
        match event.event_type {
            KeyPress(key) if notes.contains_key(&key) => {
                self.patch.note = *notes.get(&key).unwrap();
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
                    self.patch.note = *notes.get(self.pressed_set.back().unwrap()).unwrap();
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
            
            // LPG
            KeyPress(Key::F11) => { 
                self.patch.lpg_colour = (self.patch.lpg_colour - 0.1).max(0.);
            }
            KeyPress(Key::F12) => { 
                self.patch.lpg_colour = (self.patch.lpg_colour + 0.1).min(0.);
            }
            /* (self.balance, self.volume) */
            _ => {}
        }
        // debug
        match event.event_type {
            KeyPress(Key::F1 | Key::F2 | Key::F3 | Key::F4 | Key::F5 | Key::F6 | Key::F7 | Key::F8 | Key::F9 | Key::F10 | Key::F11 | Key::F12) => {
                self.debug_sound();
            }
            _ => ()
        }
    }
}

impl<'a> App<'a> {
    fn debug_sound (&self) {
        clearscreen::clear().unwrap();
        println!("[ F1-F2 ] Model: {}", self.patch.engine);
        println!("[ F3-F4 ] Harmonic: {}", (10. * self.patch.harmonics).round() / 10.);
        println!("[ F5-F6 ] Timbre: {}", (10. * self.patch.timbre).round() / 10.);
        println!("[ F7-F8 ] Morph: {}", (10. * self.patch.morph).round() / 10.);
        println!("[ F9-F10] Decay {}", (10. * self.patch.decay).round() / 10.);
        println!("[F11-F12] Filter {}", (10. * self.patch.lpg_colour).round() / 10.);
    }
}

lazy_static! {
    static ref notes: HashMap<Key, f32> = HashMap::from(<Vec<(Key, f32)> as TryInto<[(Key, f32); 30]>>::try_into([
        (Key::Num3,23.), (Key::Num4,26.), (Key::Num5,29.), (Key::Num6,32.), (Key::Num7,35.), (Key::Num8,38.), (Key::Num9,41.),
        (Key::KeyW,22.), (Key::KeyE,25.), (Key::KeyR,28.), (Key::KeyT,31.), (Key::KeyY,34.), (Key::KeyU,37.), (Key::KeyI,40.), (Key::KeyO,43.), 
        (Key::KeyS,01.), (Key::KeyD,04.), (Key::KeyF,07.), (Key::KeyG,10.), (Key::KeyH,13.), (Key::KeyJ,16.), (Key::KeyK,19.),
        (Key::KeyZ,00.), (Key::KeyX,03.), (Key::KeyC,06.), (Key::KeyV,09.), (Key::KeyB,12.), (Key::KeyN,15.), (Key::KeyM,18.), (Key::Comma,21.), 
    ].into_iter()
        .map(|(a, b)| (a, 60.0+(12.0*b/22.0)))
        .collect::<Vec<(Key, f32)>>()).unwrap()
    );
}
