#![allow(unused)]
#![allow(non_upper_case_globals)]

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

pub const VK_Alt: u32 = 164;
pub const VK_AltGr: u32 = 165;
pub const VK_Backspace: u32 = 0x08;
pub const VK_CapsLock: u32 = 20;
pub const VK_ControlLeft: u32 = 162;
pub const VK_ControlRight: u32 = 163;
pub const VK_Delete: u32 = 46;
pub const VK_DownArrow: u32 = 40;
pub const VK_End: u32 = 35;
pub const VK_Escape: u32 = 27;
pub const VK_F1: u32 = 112;
pub const VK_F10: u32 = 121;
pub const VK_F11: u32 = 122;
pub const VK_F12: u32 = 123;
pub const VK_F2: u32 = 113;
pub const VK_F3: u32 = 114;
pub const VK_F4: u32 = 115;
pub const VK_F5: u32 = 116;
pub const VK_F6: u32 = 117;
pub const VK_F7: u32 = 118;
pub const VK_F8: u32 = 119;
pub const VK_F9: u32 = 120;
pub const VK_Home: u32 = 36;
pub const VK_LeftArrow: u32 = 37;
pub const VK_MetaLeft: u32 = 91;
pub const VK_PageDown: u32 = 34;
pub const VK_PageUp: u32 = 33;
pub const VK_Return: u32 = 0x0D;
pub const VK_RightArrow: u32 = 39;
pub const VK_ShiftLeft: u32 = 160;
pub const VK_ShiftRight: u32 = 161;
pub const VK_Space: u32 = 32;
pub const VK_Tab: u32 = 0x09;
pub const VK_UpArrow: u32 = 38;
pub const VK_PrintScreen: u32 = 44;
pub const VK_ScrollLock: u32 = 145;
pub const VK_Pause: u32 = 19;
pub const VK_NumLock: u32 = 144;
pub const VK_BackQuote: u32 = 192;
pub const VK_1: u32 = 49;
pub const VK_2: u32 = 50;
pub const VK_3: u32 = 51;
pub const VK_4: u32 = 52;
pub const VK_5: u32 = 53;
pub const VK_6: u32 = 54;
pub const VK_7: u32 = 55;
pub const VK_8: u32 = 56;
pub const VK_9: u32 = 57;
pub const VK_0: u32 = 48;
pub const VK_Minus: u32 = 189;
pub const VK_Equal: u32 = 187;
pub const VK_Q: u32 = 81;
pub const VK_W: u32 = 87;
pub const VK_E: u32 = 69;
pub const VK_R: u32 = 82;
pub const VK_T: u32 = 84;
pub const VK_Y: u32 = 89;
pub const VK_U: u32 = 85;
pub const VK_I: u32 = 73;
pub const VK_O: u32 = 79;
pub const VK_P: u32 = 80;
pub const VK_LeftBracket: u32 = 219;
pub const VK_RightBracket: u32 = 221;
pub const VK_A: u32 = 65;
pub const VK_S: u32 = 83;
pub const VK_D: u32 = 68;
pub const VK_F: u32 = 70;
pub const VK_G: u32 = 71;
pub const VK_H: u32 = 72;
pub const VK_J: u32 = 74;
pub const VK_K: u32 = 75;
pub const VK_L: u32 = 76;
pub const VK_SemiColon: u32 = 186;
pub const VK_Quote: u32 = 222;
pub const VK_BackSlash: u32 = 220;
pub const VK_IntlBackslash: u32 = 226;
pub const VK_Z: u32 = 90;
pub const VK_X: u32 = 88;
pub const VK_C: u32 = 67;
pub const VK_V: u32 = 86;
pub const VK_B: u32 = 66;
pub const VK_N: u32 = 78;
pub const VK_M: u32 = 77;
pub const VK_Comma: u32 = 188;
pub const VK_Dot: u32 = 190;
pub const VK_Slash: u32 = 191;
pub const VK_Insert: u32 = 45;
pub const VK_KpMinus: u32 = 109;
pub const VK_KpPlus: u32 = 107;
pub const VK_KpMultiply: u32 = 106;
pub const VK_KpDivide: u32 = 111;
pub const VK_Kp0: u32 = 96;
pub const VK_Kp1: u32 = 97;
pub const VK_Kp2: u32 = 98;
pub const VK_Kp3: u32 = 99;
pub const VK_Kp4: u32 = 100;
pub const VK_Kp5: u32 = 101;
pub const VK_Kp6: u32 = 102;
pub const VK_Kp7: u32 = 103;
pub const VK_Kp8: u32 = 104;
pub const VK_Kp9: u32 = 105;
pub const VK_KpDelete: u32 = 110;