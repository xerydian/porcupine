extern crate winapi;

// use std::ptr::null_mut;
use winapi::um::winuser::{GetAsyncKeyState, GetKeyboardState, MapVirtualKeyW, ToUnicode, VK_ESCAPE, VK_RETURN};
// use winapi::um::winuser::{KEYBDINPUT, INPUT, INPUT_KEYBOARD, SendInput};

fn main() {
    let mut last_char = char::default();
    unsafe {
        loop {
            for vk in 0..511 {
                if GetAsyncKeyState(vk) & (0x8000u16 as i16) != 0 {
                    let mut keyboard_state: [u8; 256] = [0; 256];
                    GetKeyboardState(keyboard_state.as_mut_ptr());

                    let scan_code = MapVirtualKeyW(vk as u32, 0);

                    let mut buffer: [u16; 4] = [0; 4];
                    let _ = ToUnicode(vk as u32, scan_code, keyboard_state.as_mut_ptr(), buffer.as_mut_ptr(), buffer.len() as i32, 0);
                    let unicode = std::char::from_u32(buffer[0] as u32).unwrap();

                    if vk == VK_RETURN { return; }
                    if last_char == unicode { continue; } 

                    println!("Virtual Key: {}, Scan Code: {}, Char: {}", vk, scan_code, unicode);
                    last_char = unicode.to_owned();
                }
            }
        }
    }
}