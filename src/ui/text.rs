use crate::keyboard_utils::{ch, VK_Comma, VK_LeftBracket, VK_RightBracket, VK_B, VK_C, VK_D, VK_E, VK_F, VK_G, VK_H, VK_I, VK_J, VK_K, VK_M, VK_N, VK_O, VK_R, VK_S, VK_T, VK_U, VK_V, VK_W, VK_X, VK_Y, VK_Z};

use std::collections::HashMap;
use std::sync::{MutexGuard, LazyLock};
use crate::synth::part::Synth;
use crate::synth::seq::*;
use crate::{VOICES, SELECTED_VOICE};
use rdev::{Event, Key, EventType::{KeyPress, KeyRelease}};

pub static KEY_NOTES: LazyLock<HashMap<Key, i32>> = LazyLock::new(|| HashMap::from([
    (Key::Num3,23), (Key::Num4,26), (Key::Num5,29), (Key::Num6,32), (Key::Num7,35), (Key::Num8,38), (Key::Num9,41),
    (Key::KeyW,22), (Key::KeyE,25), (Key::KeyR,28), (Key::KeyT,31), (Key::KeyY,34), (Key::KeyU,37), (Key::KeyI,40), (Key::KeyO,43), 
    (Key::KeyS,01), (Key::KeyD,04), (Key::KeyF,07), (Key::KeyG,10), (Key::KeyH,13), (Key::KeyJ,16), (Key::KeyK,19),
    (Key::KeyZ,00), (Key::KeyX,03), (Key::KeyC,06), (Key::KeyV,09), (Key::KeyB,12), (Key::KeyN,15), (Key::KeyM,18), (Key::Comma,21), 
]));

pub fn print_info () {
    let  seq = SEQUENCER.lock().unwrap();
    let synth = VOICES.get(SELECTED_VOICE).unwrap().lock().unwrap();
    // clearscreen::clear().unwrap();
    println!("[ F1-F2 ]    Model: {}", ENGINE_DESCRIPIONS[synth.patch.engine]);
    println!("[ F3-F4 ] Harmonic: {}", (10. * synth.target_harmonic).round() / 10.);
    println!("[ F5-F6 ]   Timbre: {}", (10. * synth.target_timbre).round() / 10.);
    println!("[ F7-F8 ]    Morph: {}", (10. * synth.target_morph).round() / 10.);
    println!("[ F9-10 ]    Decay: {}", (10. * synth.patch.decay).round() / 10.);
    println!("                           +----------+------------+--------------+");
    println!("                           |  {} Rest  |  {} {}  |  BKSP Clear  |", 
        ch(VK_LeftBracket), ch(VK_RightBracket),
        if seq.is_recording() {"Record"} else {"  Undo"},
    );
    println!("+--------------------------+----------+------------+--------------+");
    println!("|    {}   {}   {}   {}   {}   {}   {}       3   4   5   6   7   8   9    |", 
        ch(VK_S), ch(VK_D), ch(VK_F), ch(VK_G), ch(VK_H), ch(VK_J), ch(VK_K),
    );
    println!("|  {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}  |",
        ch(VK_Z), ch(VK_X), ch(VK_C), ch(VK_V), ch(VK_B), ch(VK_N), ch(VK_M), ch(VK_Comma),
        ch(VK_W), ch(VK_E), ch(VK_R), ch(VK_T), ch(VK_Y), ch(VK_U), ch(VK_I), ch(VK_O),
    );
    println!("+-----------------------------------------------------------------+");
    println!("[ LSHIFT  > ] Pitch Bend    [ LCTRL ] Vibrato   [ . - ]  Octave: {} ", synth.info_octave);
    println!("                     [   SPACE_BAR    {:?}   ]",
        if seq.is_playing() {"Play"} else {"Stop"}
    );
    print_sequence(&synth, &seq);
    println!("                 Transport: {:?}", seq.status);
    println!("[   Up / Down  ] Tempo: {} BPM", seq.tempo.round());
    println!("[ Left / Right ] Gate length: {}", (10. * synth.gate_length).round() / 10.);
    println!("");
    println!("(Press [Esc] to exit)");
}

pub fn print_sequence (synth: &MutexGuard<Synth>, seq: &MutexGuard<Sequencer>) {
    if seq.is_recording() || !synth.seq_notes.is_empty() {
        println!("");
        println!("Sequence: ");
    }
    if seq.is_recording() && synth.seq_notes.is_empty() {
        println!("[ ]");
    }
    for (i, sq) in synth.seq_notes.iter().enumerate() {
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
        if i % 8 == 7 || (i+1) == synth.seq_notes.len() { println!(" ]"); }
    }
}

pub fn process_keyboard_events(event: Event) {
    match event.event_type {
        KeyPress(_) | KeyRelease(_) => (),
        _ => return
    }

    println!("test");

    let mut seq = SEQUENCER.lock().unwrap();
    let mut synth = VOICES.get(SELECTED_VOICE).unwrap().lock().unwrap();

    match event.event_type {
        KeyPress(key) if KEY_NOTES.contains_key(&key) 
            => synth.note_on(*KEY_NOTES.get(&key).unwrap()),
        KeyRelease(key) if KEY_NOTES.contains_key(&key) 
            => synth.note_off(*KEY_NOTES.get(&key).unwrap()),

        KeyPress(Key::F1) => synth.model_down(),
        KeyPress(Key::F2) => synth.model_up(),
        KeyPress(Key::F3) => synth.harmonic_down(),
        KeyPress(Key::F4) => synth.harmonic_up(),
        KeyPress(Key::F5) => synth.timbre_down(),
        KeyPress(Key::F6) => synth.timbre_up(),
        KeyPress(Key::F7) => synth.morph_down(),
        KeyPress(Key::F8) => synth.morph_up(),
        KeyPress(Key::F9) => synth.decay_down(),
        KeyPress(Key::F10) => synth.decay_up(),
        KeyPress(Key::ShiftLeft)     => synth.pitch_bend_negative(),
        KeyPress(Key::IntlBackslash) => synth.pitch_bend_positive(),
        KeyRelease(Key::ShiftLeft | Key::IntlBackslash) => synth.pitch_bend_neutral(),
        KeyPress(Key::ControlLeft)   => synth.vibrato_on(),
        KeyRelease(Key::ControlLeft) => synth.vibrato_off(),
        KeyPress(Key::LeftArrow) => synth.gate_length_down(),
        KeyPress(Key::RightArrow) => synth.gate_length_up(),
        KeyPress(Key::Dot) => synth.transpose_down(),
        KeyPress(Key::Minus) => synth.transpose_up(),
        KeyPress(Key::LeftBracket) => synth.add_rest(),
        KeyPress(Key::RightBracket) => synth.start_recording_or_undo_last(),
        KeyPress(Key::Backspace) => synth.clear_notes(),
        KeyPress(Key::Space) => seq.play_pause(),
        KeyPress(Key::DownArrow) => seq.tempo_down(),
        KeyPress(Key::UpArrow) => seq.tempo_up(),
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
            print_info();
        },
        KeyPress(key) if KEY_NOTES.contains_key(&key) && seq.is_recording() => print_info(),
        KeyPress(Key::LeftBracket | Key::Backspace) if seq.is_recording() => print_info(),
        KeyPress(Key::Escape) => std::process::exit(0),
        _ => ()
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
