//! The board: an `NSView` subclass that draws a [`Scene`] and forwards clicks.

use std::cell::RefCell;

use dame::{Bitboard, Color, Move, PieceKind, Position, Square};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSEvent, NSFont, NSFontAttributeName, NSForegroundColorAttributeName,
    NSStringDrawing, NSView,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

use super::{palette, with_app};
use crate::input::{cell_of, square_at};

/// Everything the board view draws.
#[derive(Clone, Debug)]
pub(crate) struct Scene {
    pub(crate) position: Position,
    pub(crate) flipped: bool,
    pub(crate) selected: Option<Square>,
    /// Destinations of the selected piece.
    pub(crate) targets: Bitboard,
    /// Pieces clicked to choose between capture routes.
    pub(crate) via: Bitboard,
    pub(crate) last_move: Option<Move>,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            position: Position::initial(),
            flipped: false,
            selected: None,
            targets: Bitboard::EMPTY,
            via: Bitboard::EMPTY,
            last_move: None,
        }
    }
}

/// Where the board sits in a view: the top-left corner and the cell size.
#[derive(Copy, Clone, Debug)]
struct Layout {
    origin: NSPoint,
    cell: f64,
}

impl Layout {
    const MARGIN: f64 = 12.0;

    /// The largest centred square board that fits `bounds`.
    fn new(bounds: NSRect) -> Self {
        let side = (bounds.size.width.min(bounds.size.height) - 2.0 * Self::MARGIN).max(10.0);
        let cell = (side / 10.0).floor();
        let side = cell * 10.0;
        Self {
            origin: NSPoint::new(
                bounds.origin.x + ((bounds.size.width - side) / 2.0).floor(),
                bounds.origin.y + ((bounds.size.height - side) / 2.0).floor(),
            ),
            cell,
        }
    }

    fn cell_rect(self, row: u8, col: u8) -> NSRect {
        NSRect::new(
            NSPoint::new(
                self.origin.x + f64::from(col) * self.cell,
                self.origin.y + f64::from(row) * self.cell,
            ),
            NSSize::new(self.cell, self.cell),
        )
    }

    /// The cell under `point`, if it is on the board.
    fn cell_at(self, point: NSPoint) -> Option<(u8, u8)> {
        let col = ((point.x - self.origin.x) / self.cell).floor();
        let row = ((point.y - self.origin.y) / self.cell).floor();
        ((0.0..10.0).contains(&col) && (0.0..10.0).contains(&row)).then_some((row as u8, col as u8))
    }
}

fn rgb(red: f64, green: f64, blue: f64, alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(red, green, blue, alpha)
}

fn inset(rect: NSRect, by: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(rect.origin.x + by, rect.origin.y + by),
        NSSize::new(rect.size.width - 2.0 * by, rect.size.height - 2.0 * by),
    )
}

fn fill_oval(rect: NSRect, color: &NSColor) {
    color.setFill();
    NSBezierPath::bezierPathWithOvalInRect(rect).fill();
}

fn stroke_oval(rect: NSRect, color: &NSColor, width: f64) {
    color.setStroke();
    let path = NSBezierPath::bezierPathWithOvalInRect(rect);
    path.setLineWidth(width);
    path.stroke();
}

#[derive(Debug, Default)]
pub(crate) struct Ivars {
    scene: RefCell<Scene>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements beyond those checked by
    // `define_class!`, and the overridden methods match their signatures.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "DameBoardView"]
    #[ivars = Ivars]
    #[derive(Debug)]
    pub(crate) struct BoardView;

    impl BoardView {
        /// Top-left origin, like the board's rows.
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.draw();
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let point = self.convertPoint_fromView(event.locationInWindow(), None);
            let flipped = self.ivars().scene.borrow().flipped;
            let square = Layout::new(self.bounds())
                .cell_at(point)
                .and_then(|(row, col)| square_at(row, col, flipped));
            with_app(|app| app.click(square));
        }
    }
);

impl BoardView {
    pub(crate) fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars::default());
        // SAFETY: `initWithFrame:` is NSView's designated initializer.
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    /// Shows `scene` from the next redraw on.
    pub(crate) fn set_scene(&self, scene: Scene) {
        *self.ivars().scene.borrow_mut() = scene;
        self.setNeedsDisplay(true);
    }

    fn draw(&self) {
        let scene = self.ivars().scene.borrow();
        let layout = Layout::new(self.bounds());
        palette::canvas().setFill();
        NSBezierPath::fillRect(self.bounds());
        let light = rgb(0.93, 0.86, 0.72, 1.0);
        let dark = rgb(0.55, 0.37, 0.24, 1.0);
        // Highlights are GitHub dark-theme colors laid over the dark square.
        let last = palette::translucent(&palette::attention(), 0.45);
        let selected = palette::translucent(&palette::accent(), 0.6);

        let last_squares = scene.last_move.map_or(Bitboard::EMPTY, |mv| {
            mv.from().bitboard() | mv.to().bitboard()
        });
        for row in 0..10 {
            for col in 0..10 {
                let rect = layout.cell_rect(row, col);
                let Some(square) = square_at(row, col, scene.flipped) else {
                    light.setFill();
                    NSBezierPath::fillRect(rect);
                    continue;
                };
                dark.setFill();
                NSBezierPath::fillRect(rect);
                let highlight = if Some(square) == scene.selected {
                    Some(&selected)
                } else if last_squares.contains(square) {
                    Some(&last)
                } else {
                    None
                };
                if let Some(highlight) = highlight {
                    highlight.setFill();
                    // Bezier paths composite source-over, unlike `NSRectFill`.
                    NSBezierPath::fillRect(rect);
                }
            }
        }
        rgb(0.25, 0.16, 0.10, 1.0).setStroke();
        NSBezierPath::strokeRect(inset(
            NSRect::new(
                layout.origin,
                NSSize::new(layout.cell * 10.0, layout.cell * 10.0),
            ),
            -0.5,
        ));

        draw_numbers(&scene, layout);
        for square in Square::all() {
            if let Some(piece) = scene.position.piece_at(square) {
                let (row, col) = cell_of(square, scene.flipped);
                draw_piece(layout.cell_rect(row, col), piece.color, piece.kind);
            }
        }
        let target = palette::translucent(&palette::success(), 0.9);
        for square in scene.targets {
            let (row, col) = cell_of(square, scene.flipped);
            fill_oval(
                inset(layout.cell_rect(row, col), layout.cell * 0.36),
                &target,
            );
        }
        let via = palette::danger();
        for square in scene.via {
            let (row, col) = cell_of(square, scene.flipped);
            stroke_oval(
                inset(layout.cell_rect(row, col), layout.cell * 0.06),
                &via,
                3.0,
            );
        }
    }
}

/// Small square numbers in the corner of the dark squares.
fn draw_numbers(scene: &Scene, layout: Layout) {
    let font = NSFont::systemFontOfSize((layout.cell * 0.2).max(7.0));
    let color = rgb(1.0, 1.0, 1.0, 0.45);
    // SAFETY: the attribute keys are immutable framework constants.
    let keys = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
    let values: [&AnyObject; 2] = [&font, &color];
    let attributes = NSDictionary::from_slices(&keys, &values);
    for square in Square::all() {
        let (row, col) = cell_of(square, scene.flipped);
        let rect = layout.cell_rect(row, col);
        let text = NSString::from_str(&square.to_string());
        let point = NSPoint::new(rect.origin.x + 2.0, rect.origin.y + 1.0);
        // SAFETY: the dictionary holds a font and a color under the keys
        // AppKit expects for them.
        unsafe { text.drawAtPoint_withAttributes(point, Some(&attributes)) };
    }
}

fn draw_piece(cell: NSRect, color: Color, kind: PieceKind) {
    let size = cell.size.width;
    let disc = inset(cell, size * 0.1);
    let (face, edge, ring) = match color {
        Color::White => (
            rgb(0.97, 0.95, 0.90, 1.0),
            rgb(0.35, 0.30, 0.25, 1.0),
            rgb(0.75, 0.70, 0.62, 1.0),
        ),
        Color::Black => (
            rgb(0.16, 0.13, 0.12, 1.0),
            rgb(0.05, 0.04, 0.04, 1.0),
            rgb(0.40, 0.35, 0.32, 1.0),
        ),
    };
    // A soft shadow below the disc.
    let shadow = NSRect::new(
        NSPoint::new(disc.origin.x, disc.origin.y + size * 0.04),
        disc.size,
    );
    fill_oval(shadow, &rgb(0.0, 0.0, 0.0, 0.35));
    fill_oval(disc, &face);
    stroke_oval(disc, &edge, 1.0);
    stroke_oval(inset(disc, size * 0.12), &ring, 1.5);
    if kind == PieceKind::King {
        let crown = inset(disc, size * 0.22);
        fill_oval(crown, &rgb(0.85, 0.66, 0.18, 1.0));
        stroke_oval(crown, &rgb(0.55, 0.40, 0.08, 1.0), 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)] // The layout rounds to whole points.
    fn layout_centres_the_board_and_maps_clicks() {
        let layout = Layout::new(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(424.0, 600.0),
        ));
        assert_eq!(layout.cell, 40.0);
        assert_eq!((layout.origin.x, layout.origin.y), (12.0, 100.0));
        assert_eq!(layout.cell_at(NSPoint::new(13.0, 101.0)), Some((0, 0)));
        assert_eq!(layout.cell_at(NSPoint::new(411.0, 499.0)), Some((9, 9)));
        assert_eq!(layout.cell_at(NSPoint::new(11.0, 150.0)), None);
        assert_eq!(layout.cell_at(NSPoint::new(100.0, 500.0)), None);
    }
}
