//! Colors of GitHub's dark theme (Primer), for the window chrome and the
//! board highlights.

use objc2::rc::Retained;
use objc2_app_kit::NSColor;

fn hex(rgb: u32, alpha: f64) -> Retained<NSColor> {
    let channel = |shift: u32| f64::from((rgb >> shift) & 0xFF) / 255.0;
    NSColor::colorWithSRGBRed_green_blue_alpha(channel(16), channel(8), channel(0), alpha)
}

/// The window background (`canvas.default`).
pub(crate) fn canvas() -> Retained<NSColor> {
    hex(0x0D_1117, 1.0)
}

/// Raised surfaces such as the status bar (`canvas.subtle`).
pub(crate) fn canvas_subtle() -> Retained<NSColor> {
    hex(0x16_1B22, 1.0)
}

/// Hairlines between surfaces (`border.default`).
pub(crate) fn border() -> Retained<NSColor> {
    hex(0x30_363D, 1.0)
}

/// Text: a dimmed gray (`fg.muted`).
pub(crate) fn fg_muted() -> Retained<NSColor> {
    hex(0x7D_8590, 1.0)
}

/// `success.fg`: green.
pub(crate) fn success() -> Retained<NSColor> {
    hex(0x3F_B950, 1.0)
}

/// `attention.fg`: yellow.
pub(crate) fn attention() -> Retained<NSColor> {
    hex(0xD2_9922, 1.0)
}

/// `danger.fg`: red.
pub(crate) fn danger() -> Retained<NSColor> {
    hex(0xF8_5149, 1.0)
}

/// `accent.fg`: blue.
pub(crate) fn accent() -> Retained<NSColor> {
    hex(0x2F_81F7, 1.0)
}

/// `color` at `alpha`.
pub(crate) fn translucent(color: &NSColor, alpha: f64) -> Retained<NSColor> {
    color.colorWithAlphaComponent(alpha)
}
