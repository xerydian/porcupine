use crate::keyboard_utils::{ch, VK_Comma, VK_LeftBracket, VK_RightBracket, VK_B, VK_C, VK_D, VK_E, VK_F, VK_G, VK_H, VK_I, VK_J, VK_K, VK_M, VK_N, VK_O, VK_R, VK_S, VK_T, VK_U, VK_V, VK_W, VK_X, VK_Y, VK_Z};

use std::sync::MutexGuard;
use crate::synth::{Synth, notes};
use crate::{VOICES, SELECTED_VOICE, SEQ_STATUS, SeqStatus, SeqStep, TEMPO};
use rdev::{Event, Key, EventType::KeyPress};


fn print_info () {
    let synth = VOICES.get(SELECTED_VOICE).unwrap().lock().unwrap();
    clearscreen::clear().unwrap();
    println!("[ F1-F2 ]    Model: {}", ENGINE_DESCRIPIONS[synth.patch.engine]);
    println!("[ F3-F4 ] Harmonic: {}", (10. * synth.target_harmonic).round() / 10.);
    println!("[ F5-F6 ]   Timbre: {}", (10. * synth.target_timbre).round() / 10.);
    println!("[ F7-F8 ]    Morph: {}", (10. * synth.target_morph).round() / 10.);
    println!("[ F9-10 ]    Decay: {}", (10. * synth.patch.decay).round() / 10.);
    println!("                           +-----------+-----------+--------------+");
    println!("                           |  {} Rest  |  {} {}  |  BKSP Clear  |", 
        ch(VK_LeftBracket), ch(VK_RightBracket),
        if SEQ_STATUS != SeqStatus::Recording {"Record"} else {"  Undo"},
    );
    println!("+--------------------------+-----------+---------+----------------+");
    println!("|    {}   {}   {}   {}   {}   {}   {}       3   4   5   6   7   8   9    |", 
        ch(VK_S), ch(VK_D), ch(VK_F), ch(VK_G), ch(VK_H), ch(VK_J), ch(VK_K),
    );
    println!("| {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}   {}  |",
        ch(VK_Z), ch(VK_X), ch(VK_C), ch(VK_V), ch(VK_B), ch(VK_N), ch(VK_M), ch(VK_Comma),
        ch(VK_W), ch(VK_E), ch(VK_R), ch(VK_T), ch(VK_Y), ch(VK_U), ch(VK_I), ch(VK_O),
    );
    println!("+-----------------------------------------------------------------+");
    println!("[ LSHIFT  > ] Pitch Bend    [ LCTRL ] Vibrato   [ . - ]  Octave: {} ", synth.info_octave);
    println!("                     [   SPACE_BAR    {:?}   ]",
        if SEQ_STATUS != SeqStatus::Play {"Play"} else {"Stop"}
    );
    print_sequence(&synth);
    println!("                 Transport: {:?}", SEQ_STATUS);
    println!("[   Up / Down  ] Tempo: {} BPM", TEMPO.round());
    println!("[ Left / Right ] Gate length: {}", (10. * synth.gate_length).round() / 10.);
    println!("");
    println!("(Press [Esc] to exit)");
}

pub fn print_sequence (synth: &MutexGuard<Synth>) {
    if SEQ_STATUS == SeqStatus::Recording || !synth.seq_notes.is_empty() {
        println!("");
        println!("Sequence: ");
    }
    if SEQ_STATUS == SeqStatus::Recording && synth.seq_notes.is_empty() {
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

pub fn process_events(event: Event) {
    // Print Info
    match event.event_type {
        KeyPress(
            Key::F1 | Key::F2 | Key::F3 | Key::F4 | Key::F5 | Key::F6 | Key::F7 | Key::F8 | Key::F9 | Key::F10 |
            Key::Space | Key::RightBracket | Key::UpArrow | Key::DownArrow | Key::Dot | Key::Minus | Key::LeftArrow | Key::RightArrow
        ) => {
            // self.print_info();
        },
        KeyPress(key) if notes.contains_key(&key) && SEQ_STATUS == SeqStatus::Recording => print_info(),
        KeyPress(Key::LeftBracket | Key::Backspace) if SEQ_STATUS == SeqStatus::Recording => print_info(),
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