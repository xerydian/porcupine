#![recursion_limit = "256"]

// Reference: https://github.com/sourcebox/mi-plaits-dsp-rs/blob/firmware-1.2/examples/midi control.rs
// Reference: https://github.com/Narsil/rdev/blob/main/examples/listen.rs


use std::{array, sync::{Arc, LazyLock, Mutex}};

mod audio_shell;
mod keyboard_utils;
mod synth;
mod ui;

use crate::synth::seq::{SeqStatus, Sequencer};
use crate::ui::text::{print_info, process_keyboard_events};
use synth::part::Synth;
use tinyaudio::{run_output_device, OutputDeviceParameters};

const SAMPLE_RATE: u32 = 48000;
const BLOCK_SIZE: usize = 2048;
const MAX_VOICE_COUNT: usize = 4;

static SELECTED_VOICE: usize = 0;
pub static VOICES: LazyLock<[Arc<Mutex<Synth<'static>>>; MAX_VOICE_COUNT]> = LazyLock::new(||
    array::from_fn(|_| Arc::new(Mutex::new(Synth::new(BLOCK_SIZE))))
);

fn main() {
    
    let params = OutputDeviceParameters {
        channels_count: 2,
        sample_rate: SAMPLE_RATE as usize,
        channel_sample_count: BLOCK_SIZE,
    };
    
    let _output_device = run_output_device(params, move |data| {
        let mut samples_left = vec![0.0; BLOCK_SIZE];
        let mut samples_right = vec![0.0; BLOCK_SIZE];
        output_sound(&mut samples_left, &mut samples_right);

        for (frame_no, samples) in data.chunks_mut(params.channels_count).enumerate() {
            samples[0] = samples_left[frame_no];
            samples[1] = samples_right[frame_no];
        }
    }).unwrap();

    let seq: Arc<Mutex<Sequencer>> = Arc::new(
        Sequencer {
            tempo: 120.,
            status: SeqStatus::Stop,
        }.into());

    for synth in VOICES.iter() {
        { synth.lock().unwrap().init();  }
        /*
        let synth_1 = synth.clone();
        let seq_1 = seq.clone();
        thread::spawn(move || {
            Synth::sequencer_loop(synth_1, seq_1);
        });

        let synth_2 = synth.clone();
        let seq_2 = seq.clone();
        thread::spawn(move || {
            Synth::control_loop(synth_2);
        }); */
    }
    
    
    print_info(&seq.lock().unwrap(), &VOICES.first().unwrap().lock().unwrap());
    
    let _ = rdev::listen(process_keyboard_events(seq)); // handle keystrokes, blocking
}



fn output_sound(samples_l: &mut [f32], samples_r: &mut [f32]) {
    let mut out = [[0.0; BLOCK_SIZE]; MAX_VOICE_COUNT];
    let mut aux = [[0.0; BLOCK_SIZE]; MAX_VOICE_COUNT];

    for (i, synth) in VOICES.iter().enumerate() {
        let out_i = out.get_mut(i).unwrap();
        let aux_i = aux.get_mut(i).unwrap();
        synth.lock().unwrap().render(out_i, aux_i);
    }

    for (i, synth) in VOICES.iter().enumerate() {
        let v = synth.lock().unwrap();
        let out_i = out.get(i).unwrap().to_owned();
        // let aux_i = aux.get(i).unwrap().to_owned();
        let (pan_r, pan_l) = equal_power_panlaw_r_l(v.pan);
        for frame in 0..BLOCK_SIZE {
            // let sample = (out_i[frame] * (1.0 - v.balance) + aux_i[frame] * v.balance) * v.volume;
            let sample = out_i[frame] * v.volume;
            samples_l[frame] += sample * pan_l;
            samples_r[frame] += sample * pan_r;
        }
    }
}

fn equal_power_panlaw_r_l (pan: f32) -> (f32, f32) {
    (pan * std::f32::consts::FRAC_PI_2).sin_cos() // right, left
}