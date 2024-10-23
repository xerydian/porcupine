use std::collections::HashMap;
use lazy_static::lazy_static;

use rdev::{listen, Event, Key};
use rdev::EventType::{KeyPress, KeyRelease};

fn main() {
    if let Err(error) = listen(get_callback()) {
        println!("Error: {:?}", error)
    }
}

lazy_static! {
    static ref notes: HashMap<Key, i32> = HashMap::from([
        (Key::Num3,23), (Key::Num4,26), (Key::Num5,39), (Key::Num6,32), (Key::Num7,35), (Key::Num8,38), (Key::Num9,41),
        (Key::KeyW,22), (Key::KeyE,25), (Key::KeyR,28), (Key::KeyT,31), (Key::KeyY,34), (Key::KeyU,37), (Key::KeyI,40), (Key::KeyO,43), 
        (Key::KeyS,01), (Key::KeyD,04), (Key::KeyF,07), (Key::KeyG,10), (Key::KeyH,13), (Key::KeyJ,16), (Key::KeyK,19),
        (Key::KeyZ,00), (Key::KeyX,03), (Key::KeyC,06), (Key::KeyV,09), (Key::KeyB,12), (Key::KeyN,15), (Key::KeyM,18), (Key::Comma,21), 
    ]);
}

fn get_callback() -> impl FnMut(Event) {
    let mut nkeyspressed = 0;
    move |event: Event| {
        match event.event_type {
            // Handle notes
            KeyPress(key) if notes.contains_key(&key) => {
                if nkeyspressed == 0 {
                    println!("Start gate");
                }
                nkeyspressed += 1;
                println!("Note {:?}", notes.get(&key));
            }
            // Handle note release
            KeyRelease(key) if notes.contains_key(&key) => {
                nkeyspressed -= 1;
                if nkeyspressed == 0 {
                    println!("End gate");
                }
            }
            KeyPress(Key::Escape) => std::process::exit(0),
            KeyPress(key) => {
                println!("Key {:?}", key)
            }
            _ => {}
        }
    }
}


