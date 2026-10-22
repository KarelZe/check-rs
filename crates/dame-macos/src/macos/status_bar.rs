//! The status bar: a footer in the style of VS Code, with items aligned to
//! the left and right edges and evenly padded.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSFont, NSFontAttributeName, NSFontWeightMedium, NSFontWeightRegular,
    NSForegroundColorAttributeName, NSImage, NSImageSymbolConfiguration, NSStringDrawing, NSView,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

use super::palette;

/// The bar's height in points.
pub(crate) const HEIGHT: f64 = 24.0;
/// Space on each side of an item's text; neighbouring items sit `2 * PAD` apart.
const PAD: f64 = 8.0;
const DOT: f64 = 7.0;
/// Space between an item's dot or icon and its text.
const LEAD_GAP: f64 = 5.0;
const SYMBOL_SIZE: f64 = 12.0;

/// Search depth: stacked layers, one per ply.
pub(crate) const DEPTH_SYMBOL: &str = "square.stack.3d.down.right";
/// Time per move.
pub(crate) const TIME_SYMBOL: &str = "timer";

/// What an item's text or dot means, and so its color.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    Muted,
    Success,
    Attention,
    Danger,
}

impl Tone {
    fn color(self) -> Retained<NSColor> {
        match self {
            Self::Muted => palette::fg_muted(),
            Self::Success => palette::success(),
            Self::Attention => palette::attention(),
            Self::Danger => palette::danger(),
        }
    }
}

/// What leads an item's text.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Lead {
    /// A colored status dot.
    Dot(Tone),
    /// An SF Symbol, in the item's color.
    Symbol(&'static str),
}

/// One entry in the bar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Item {
    pub(crate) text: String,
    pub(crate) tone: Tone,
    pub(crate) lead: Option<Lead>,
}

impl Item {
    pub(crate) fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
            lead: None,
        }
    }

    pub(crate) fn with_dot(mut self, dot: Tone) -> Self {
        self.lead = Some(Lead::Dot(dot));
        self
    }

    pub(crate) fn with_symbol(mut self, name: &'static str) -> Self {
        self.lead = Some(Lead::Symbol(name));
        self
    }
}

/// The SF Symbol `name`, sized for the bar and tinted with `tone`.
fn symbol(name: &str, tone: Tone) -> Option<Retained<NSImage>> {
    // Medium matches the visual weight of the regular text.
    // SAFETY: an immutable framework constant.
    let weight = unsafe { NSFontWeightMedium };
    let config = NSImageSymbolConfiguration::configurationWithPointSize_weight(SYMBOL_SIZE, weight)
        .configurationByApplyingConfiguration(
            &NSImageSymbolConfiguration::configurationWithHierarchicalColor(&tone.color()),
        );
    NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(name), None)?
        .imageWithSymbolConfiguration(&config)
}

/// The bar's content. The last left item is shortened to fit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Status {
    pub(crate) left: Vec<Item>,
    pub(crate) right: Vec<Item>,
}

#[derive(Debug, Default)]
pub(crate) struct Ivars {
    status: RefCell<Status>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements beyond those checked by
    // `define_class!`, and the overridden methods match their signatures.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "DameStatusBar"]
    #[ivars = Ivars]
    #[derive(Debug)]
    pub(crate) struct StatusBar;

    impl StatusBar {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.draw();
        }
    }
);

/// Text attributes for one tone.
type Attributes = Retained<NSDictionary<NSString, AnyObject>>;

fn attributes(font: &NSFont, tone: Tone) -> Attributes {
    let color = tone.color();
    // SAFETY: the attribute keys are immutable framework constants.
    let keys = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
    let values: [&AnyObject; 2] = [font, &color];
    NSDictionary::from_slices(&keys, &values)
}

fn text_width(text: &str, attributes: &Attributes) -> f64 {
    // SAFETY: the dictionary holds a font and a color under the expected keys.
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(attributes)) }.width
}

/// The width of an item, padding included.
fn item_width(item: &Item, attributes: &Attributes) -> f64 {
    let lead = match item.lead {
        None => 0.0,
        Some(Lead::Dot(_)) => DOT + LEAD_GAP,
        Some(Lead::Symbol(name)) => {
            symbol(name, item.tone).map_or(0.0, |image| image.size().width + LEAD_GAP)
        }
    };
    PAD + lead + text_width(&item.text, attributes) + PAD
}

/// `text` shortened with an ellipsis to at most `width`.
fn truncate(text: &str, width: f64, attributes: &Attributes) -> String {
    if text_width(text, attributes) <= width {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let candidate: String = chars[..mid].iter().chain(['…'].iter()).collect();
        if text_width(&candidate, attributes) <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if lo == 0 {
        String::new()
    } else {
        chars[..lo].iter().chain(['…'].iter()).collect()
    }
}

impl StatusBar {
    pub(crate) fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars::default());
        // SAFETY: `initWithFrame:` is NSView's designated initializer.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    /// Shows `status`, redrawing only when it changed.
    pub(crate) fn set_status(&self, status: Status) {
        let mut current = self.ivars().status.borrow_mut();
        if *current != status {
            *current = status;
            self.setNeedsDisplay(true);
        }
    }

    fn draw(&self) {
        let bounds = self.bounds();
        palette::canvas_subtle().setFill();
        NSBezierPath::fillRect(bounds);
        palette::border().setFill();
        NSBezierPath::fillRect(NSRect::new(
            bounds.origin,
            NSSize::new(bounds.size.width, 1.0),
        ));

        // SAFETY: an immutable framework constant.
        let font =
            NSFont::monospacedDigitSystemFontOfSize_weight(12.0, unsafe { NSFontWeightRegular });
        let status = self.ivars().status.borrow();
        let height = bounds.size.height;
        let line_height = font.ascender() - font.descender();
        let text_y = ((height - line_height) / 2.0).floor();

        let draw_item = |item: &Item, text: &str, x: f64, attributes: &Attributes| {
            let mut x = x + PAD;
            match item.lead {
                None => {}
                Some(Lead::Dot(dot)) => {
                    dot.color().setFill();
                    let rect = NSRect::new(
                        NSPoint::new(x, ((height - DOT) / 2.0).floor()),
                        NSSize::new(DOT, DOT),
                    );
                    NSBezierPath::bezierPathWithOvalInRect(rect).fill();
                    x += DOT + LEAD_GAP;
                }
                Some(Lead::Symbol(name)) => {
                    if let Some(image) = symbol(name, item.tone) {
                        let size = image.size();
                        let y = ((height - size.height) / 2.0).round();
                        image.drawInRect(NSRect::new(NSPoint::new(x, y), size));
                        x += size.width + LEAD_GAP;
                    }
                }
            }
            // SAFETY: the dictionary holds a font and a color under the
            // expected keys.
            unsafe {
                NSString::from_str(text)
                    .drawAtPoint_withAttributes(NSPoint::new(x, text_y), Some(attributes));
            }
        };

        let mut right = bounds.size.width - PAD;
        for item in status.right.iter().rev() {
            let attributes = attributes(&font, item.tone);
            right -= item_width(item, &attributes);
            draw_item(item, &item.text, right, &attributes);
        }

        let mut left = PAD;
        for (i, item) in status.left.iter().enumerate() {
            let attributes = attributes(&font, item.tone);
            let text = if i + 1 == status.left.len() {
                let fixed = item_width(item, &attributes) - text_width(&item.text, &attributes);
                truncate(&item.text, right - left - fixed, &attributes)
            } else {
                item.text.clone()
            };
            if text.is_empty() {
                break;
            }
            let shown = Item {
                text,
                ..item.clone()
            };
            let width = item_width(&shown, &attributes);
            if left + width > right {
                break;
            }
            draw_item(&shown, &shown.text, left, &attributes);
            left += width;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_exist() {
        for name in [DEPTH_SYMBOL, TIME_SYMBOL] {
            assert!(symbol(name, Tone::Muted).is_some(), "{name}");
        }
    }
}
