//! Color math utilities for diff rendering.

/// Determine whether a background color is perceptually light.
///
/// Uses BT.601 luminance weighting.
pub fn is_light(bg: (u8, u8, u8)) -> bool {
    let (r, g, b) = bg;
    let y = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    y > 128.0
}
