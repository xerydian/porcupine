pub fn dec_usize (value: &mut usize) { *value = (*value - 1).max(0); }
pub fn inc_usize (value: &mut usize) { *value = (*value + 1).min(23); }
pub fn dec_f32 (value: &mut f32) { *value = (*value - 0.1).max(0.); }
pub fn inc_f32 (value: &mut f32) { *value = (*value + 0.1).min(1.); }
pub fn dec_f64 (value: &mut f64) { *value = (*value - 0.1).max(0.); }
pub fn inc_f64 (value: &mut f64) { *value = (*value + 0.1).min(1.); }