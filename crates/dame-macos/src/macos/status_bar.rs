//! The status bar: a footer in the style of VS Code, with items aligned to
//! the left and right edges and evenly padded.

use std::cell::RefCell;
use std::ffi::c_void;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSBezierPath, NSColor, NSCursor, NSEvent, NSFont, NSFontAttributeName,
    NSFontWeightMedium, NSFontWeightRegular, NSForegroundColorAttributeName, NSImage,
    NSImageSymbolConfiguration, NSStringDrawing, NSView,
};
use objc2_foundation::{NSArray, NSDictionary, NSInteger, NSPoint, NSRect, NSSize, NSString};

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
/// The evaluation: a gauge of how strong the position is.
pub(crate) const SCORE_SYMBOL: &str = "speedometer";
/// Time per move: how fast the engine plays.
pub(crate) const TIME_SYMBOL: &str = "bolt";
/// The opening book.
pub(crate) const BOOK_SYMBOL: &str = "book";
/// Fast forward along the expected line.
pub(crate) const FORWARD_SYMBOL: &str = "forward.fill";

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
    /// Shown when the pointer rests on the item.
    pub(crate) tooltip: Option<String>,
    /// Sent up the responder chain, with the bar as sender, when the item
    /// is clicked.
    pub(crate) action: Option<Sel>,
}

impl Item {
    pub(crate) fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
            lead: None,
            tooltip: None,
            action: None,
        }
    }

    pub(crate) fn with_action(mut self, action: Sel) -> Self {
        self.action = Some(action);
        self
    }

    pub(crate) fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
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

/// The SF Symbol `name`, sized for the bar and drawn in the single color of
/// `tone`, like the text next to it.
fn symbol(name: &str, tone: Tone) -> Option<Retained<NSImage>> {
    // Medium matches the visual weight of the regular text.
    // SAFETY: an immutable framework constant.
    let weight = unsafe { NSFontWeightMedium };
    let config = NSImageSymbolConfiguration::configurationWithPointSize_weight(SYMBOL_SIZE, weight)
        .configurationByApplyingConfiguration(
            &NSImageSymbolConfiguration::configurationWithPaletteColors(
                &NSArray::from_retained_slice(&[tone.color()]),
            ),
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
    /// Where the items with tooltips were last drawn.
    tooltips: RefCell<Vec<(NSRect, String)>>,
    /// Where the clickable items were last drawn.
    actions: RefCell<Vec<(NSRect, Sel)>>,
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

        /// Clickable items work even while the window is in the background.
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            let action = self
                .ivars()
                .actions
                .borrow()
                .iter()
                .find(|(rect, _)| contains(*rect, point))
                .map(|&(_, action)| action);
            if let Some(action) = action {
                let app = NSApplication::sharedApplication(self.mtm());
                // SAFETY: item actions are methods of the app delegate that
                // take the sender as their only argument.
                unsafe { app.sendAction_to_from(action, None, Some(self)) };
            }
        }

        /// Shows the pointing hand over clickable items.
        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) {
            let cursor = NSCursor::pointingHandCursor();
            for (rect, _) in self.ivars().actions.borrow().iter() {
                self.addCursorRect_cursor(*rect, &cursor);
            }
        }

        /// The tooltip owner callback; the bar owns all its tooltip rects.
        #[unsafe(method_id(view:stringForToolTip:point:userData:))]
        fn tooltip(
            &self,
            _view: &NSView,
            _tag: NSInteger,
            point: NSPoint,
            _data: *mut c_void,
        ) -> Retained<NSString> {
            let tooltips = self.ivars().tooltips.borrow();
            let text = tooltips
                .iter()
                .find(|(rect, _)| contains(*rect, point))
                .map_or("", |(_, text)| text.as_str());
            NSString::from_str(text)
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

fn contains(rect: NSRect, point: NSPoint) -> bool {
    (rect.origin.x..rect.origin.x + rect.size.width).contains(&point.x)
        && (rect.origin.y..rect.origin.y + rect.size.height).contains(&point.y)
}

fn text_width(text: &str, attributes: &Attributes) -> f64 {
    // SAFETY: the dictionary holds a font and a color under the expected keys.
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(attributes)) }.width
}

/// The width of an item, padding included. An item without text is just
/// its dot or icon.
fn item_width(item: &Item, attributes: &Attributes) -> f64 {
    let gap = if item.text.is_empty() { 0.0 } else { LEAD_GAP };
    let lead = match item.lead {
        None => 0.0,
        Some(Lead::Dot(_)) => DOT + gap,
        Some(Lead::Symbol(name)) => {
            symbol(name, item.tone).map_or(0.0, |image| image.size().width + gap)
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

        let mut tooltips = Vec::new();
        let mut actions = Vec::new();
        let mut add_tooltip = |item: &Item, x: f64, width: f64| {
            let rect = NSRect::new(NSPoint::new(x, 0.0), NSSize::new(width, height));
            if let Some(tooltip) = &item.tooltip {
                tooltips.push((rect, tooltip.clone()));
            }
            if let Some(action) = item.action {
                actions.push((rect, action));
            }
        };

        let mut right = bounds.size.width - PAD;
        for item in status.right.iter().rev() {
            let attributes = attributes(&font, item.tone);
            let width = item_width(item, &attributes);
            right -= width;
            draw_item(item, &item.text, right, &attributes);
            add_tooltip(item, right, width);
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
            // Nothing is left of the text, but an icon-only item may fit.
            if text.is_empty() && !item.text.is_empty() {
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
            add_tooltip(item, left, width);
            left += width;
        }
        drop(status);
        self.set_tooltips(tooltips);
        self.set_actions(actions);
    }

    /// Records where the clickable items are, if that changed since the last
    /// draw, and updates the cursor rects to match.
    fn set_actions(&self, actions: Vec<(NSRect, Sel)>) {
        let mut current = self.ivars().actions.borrow_mut();
        if *current == actions {
            return;
        }
        *current = actions;
        drop(current);
        if let Some(window) = self.window() {
            window.invalidateCursorRectsForView(self);
        }
    }

    /// Registers the tooltip rects, if they changed since the last draw.
    fn set_tooltips(&self, tooltips: Vec<(NSRect, String)>) {
        let mut current = self.ivars().tooltips.borrow_mut();
        if *current == tooltips {
            return;
        }
        self.removeAllToolTips();
        for (rect, _) in &tooltips {
            // SAFETY: the bar implements `view:stringForToolTip:point:userData:`
            // and outlives its own tooltips; no user data is passed.
            unsafe { self.addToolTipRect_owner_userData(*rect, self, std::ptr::null_mut()) };
        }
        *current = tooltips;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_exist() {
        for name in [
            DEPTH_SYMBOL,
            SCORE_SYMBOL,
            TIME_SYMBOL,
            BOOK_SYMBOL,
            FORWARD_SYMBOL,
        ] {
            assert!(symbol(name, Tone::Muted).is_some(), "{name}");
        }
    }
}
