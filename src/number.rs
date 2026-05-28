use num_traits::ToPrimitive;

pub fn f32_to_u8(value: f32) -> u8 {
    value
        .round()
        .clamp(0.0, f32::from(u8::MAX))
        .to_u8()
        .unwrap_or(0)
}

pub fn f32_to_u16(value: f32) -> u16 {
    value
        .round()
        .clamp(0.0, f32::from(u16::MAX))
        .to_u16()
        .unwrap_or(0)
}

pub fn f32_to_u32(value: f32) -> u32 {
    value.round().max(0.0).to_u32().unwrap_or(0)
}

pub fn f32_to_u64(value: f32) -> u64 {
    value.round().max(0.0).to_u64().unwrap_or(0)
}

pub fn f32_to_usize(value: f32) -> usize {
    value.round().max(0.0).to_usize().unwrap_or(0)
}

pub fn f64_to_u32(value: f64) -> u32 {
    value.round().max(0.0).to_u32().unwrap_or(0)
}

pub fn f64_to_u64(value: f64) -> u64 {
    value.round().max(0.0).to_u64().unwrap_or(0)
}

pub fn f64_to_usize(value: f64) -> usize {
    value.round().max(0.0).to_usize().unwrap_or(0)
}
