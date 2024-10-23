#![recursion_limit = "256"]

// Reference: https://github.com/sourcebox/mi-plaits-dsp-rs/blob/firmware-1.2/examples/midi control.rs
// Reference: https://github.com/Narsil/rdev/blob/main/examples/listen.rs

use std::thread;

use std::sync::{Arc, Mutex};

mod audio_shell;
use audio_shell::{AudioShell, AudioGenerator};

mod synth;
use synth::Synth;
use rdev::Event;

mod keyboard_utils;

const SAMPLE_RATE: u32 = 48000;
const BLOCK_SIZE: usize = 2048;

fn main() {
    let synth = Arc::new(Mutex::new(Synth::new(BLOCK_SIZE)));
    let _audio_out = AudioShell::spawn(SAMPLE_RATE, BLOCK_SIZE, synth.clone());

    let synth_1 = synth.clone();
    let _sequencer = thread::spawn(move || {
        Synth::sequencer_loop(synth_1);
    });

    let synth_2 = synth.clone();
    let _smoothing = thread::spawn(move || {
        Synth::control_loop(synth_2);
    });
    
    let synth_3 = synth.clone();
    let _keyboard = rdev::listen(move |event: Event| {
        synth_3.lock().unwrap().process_events(event);
    }); // handle keystrokes, blocking
}