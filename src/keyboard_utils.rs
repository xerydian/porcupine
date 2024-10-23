extern crate winapi;

// use winapi::um::winnt::LANGID;
use winapi::um::winuser::{GetKeyboardLayout, GetKeyboardState, MapVirtualKeyExW, ToUnicode, MAPVK_VK_TO_VSC};
use winapi::shared::minwindef::HKL;
use memoize::memoize;

#[memoize]
pub fn ch (vk_code: u32) -> char {
    unsafe {
        let hkl: HKL = GetKeyboardLayout(0);
        // println!("Keyboard layout: {}", hkl as LANGID);

        // Translate the virtual-key code into a scan code
        let scan_code = MapVirtualKeyExW(vk_code, MAPVK_VK_TO_VSC, hkl);

        // Prepare buffer for the character
        let mut buffer: [u16; 4] = [0; 4];

        // Get the keyboard state
        let mut lp_key_state: [u8; 256] = [0; 256];
        let lp_key_state_ptr = lp_key_state.as_mut_ptr();
        let _error_bool = GetKeyboardState(lp_key_state_ptr);

        // Translate the scan code into a character
        let result = ToUnicode(vk_code, scan_code, lp_key_state_ptr, buffer.as_mut_ptr(), buffer.len() as i32, 0);

        if result > 0 {
            // Convert the first character in the buffer to a Rust char
            //Some(std::char::from_u32(buffer[0] as u32).unwrap())
            std::char::from_u32(buffer[0] as u32).unwrap()
        } else {
            char::default()
        }
    }
}
