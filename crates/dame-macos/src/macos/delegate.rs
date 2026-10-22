//! The application delegate: builds the window and the menus and routes menu
//! actions to the controller.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, Sel};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSApplication,
    NSApplicationDelegate, NSAutoresizingMaskOptions, NSBackingStoreType, NSControlStateValueOff,
    NSControlStateValueOn, NSEventModifierFlags, NSImage, NSMenu, NSMenuItem, NSMenuItemValidation,
    NSModalResponseOK, NSOpenPanel, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    NSBundle, NSData, NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL,
};

use super::app::{App, MOVE_TIMES, Players};
use super::board_view::BoardView;
use super::palette;
use super::status_bar::{self, StatusBar};
use super::{APP, with_app};

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the delegate
    // methods match the protocol signatures.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DameAppDelegate"]
    #[derive(Debug)]
    pub(crate) struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            let mtm = self.mtm();
            let app = NSApplication::sharedApplication(mtm);
            app.setMainMenu(Some(&main_menu(mtm)));
            set_icon_unless_bundled(&app);
            let (window, board, status) = main_window(mtm);
            APP.with(|cell| *cell.borrow_mut() = Some(App::new(board, status)));
            window.makeKeyAndOrderFront(None);
            #[allow(deprecated)] // `activate` needs macOS 14.
            app.activateIgnoringOtherApps(true);
        }

        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn should_terminate_after_last_window_closed(&self, _sender: &NSApplication) -> bool {
            true
        }

        /// Drops the controller, which asks the engine to quit.
        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            let app = APP.with(|cell| cell.borrow_mut().take());
            drop(app);
        }
    }

    unsafe impl NSMenuItemValidation for AppDelegate {
        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            with_app(|app| validate(app, item)).unwrap_or(false)
        }
    }

    /// Menu actions; they reach the delegate through the responder chain.
    impl AppDelegate {
        #[unsafe(method(newGame:))]
        fn new_game(&self, _sender: Option<&AnyObject>) {
            with_app(App::new_game);
        }

        #[unsafe(method(takeBack:))]
        fn take_back(&self, _sender: Option<&AnyObject>) {
            with_app(App::take_back);
        }

        #[unsafe(method(moveNow:))]
        fn move_now(&self, _sender: Option<&AnyObject>) {
            with_app(App::move_now);
        }

        #[unsafe(method(reloadEngine:))]
        fn reload_engine(&self, _sender: Option<&AnyObject>) {
            with_app(App::reload_engine);
        }

        #[unsafe(method(openBook:))]
        fn open_book(&self, _sender: Option<&AnyObject>) {
            // The panel runs its own event loop; the controller must be free
            // meanwhile, so the engine's events still reach it.
            let current = with_app(|app| app.book().map(str::to_owned)).flatten();
            if let Some(path) = choose_book(self.mtm(), current.as_deref()) {
                with_app(|app| app.set_book(Some(path)));
            }
        }

        #[unsafe(method(clearBook:))]
        fn clear_book(&self, _sender: Option<&AnyObject>) {
            with_app(|app| app.set_book(None));
        }

        #[unsafe(method(flipBoard:))]
        fn flip_board(&self, _sender: Option<&AnyObject>) {
            with_app(App::flip);
        }

        #[unsafe(method(setPlayers:))]
        fn set_players(&self, sender: &NSMenuItem) {
            if let Some(&players) = usize::try_from(sender.tag())
                .ok()
                .and_then(|i| Players::ALL.get(i))
            {
                with_app(|app| app.set_players(players));
            }
        }

        #[unsafe(method(setMoveTime:))]
        fn set_move_time(&self, sender: &NSMenuItem) {
            if let Ok(seconds) = u32::try_from(sender.tag()) {
                with_app(|app| app.set_move_time(seconds));
            }
        }

        #[unsafe(method(copyPosition:))]
        fn copy_position(&self, _sender: Option<&AnyObject>) {
            with_app(|app| app.copy_position());
        }

        #[unsafe(method(copyMoves:))]
        fn copy_moves(&self, _sender: Option<&AnyObject>) {
            with_app(|app| app.copy_moves());
        }

        #[unsafe(method(pastePosition:))]
        fn paste_position(&self, _sender: Option<&AnyObject>) {
            with_app(App::paste_position);
        }
    }
);

impl AppDelegate {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: `init` is NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

/// Enables menu items and puts the check marks on the current choices.
fn validate(app: &App, item: &NSMenuItem) -> bool {
    let Some(action) = item.action() else {
        return true;
    };
    let tag = item.tag();
    let check = |on: bool| {
        item.setState(if on {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
    };
    if action == sel!(setPlayers:) {
        let players = usize::try_from(tag).ok().and_then(|i| Players::ALL.get(i));
        check(players == Some(&app.players()));
        app.has_engine() || players == Some(&Players::Nobody)
    } else if action == sel!(setMoveTime:) {
        check(u32::try_from(tag) == Ok(app.move_time()));
        true
    } else if action == sel!(moveNow:) {
        app.has_engine()
    } else if action == sel!(takeBack:) {
        app.can_take_back()
    } else if action == sel!(clearBook:) {
        app.book().is_some()
    } else {
        true
    }
}

/// Asks for an opening book file, starting in the folder of `current`.
/// Returns `None` if cancelled.
fn choose_book(mtm: MainThreadMarker, current: Option<&str>) -> Option<String> {
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseFiles(true);
    panel.setCanChooseDirectories(false);
    panel.setAllowsMultipleSelection(false);
    panel.setMessage(Some(&NSString::from_str(
        "Choose an opening book: a text file with one line of moves per row, e.g. 32-28 18-23",
    )));
    if let Some(folder) = current.and_then(|path| {
        NSURL::fileURLWithPath(&NSString::from_str(path)).URLByDeletingLastPathComponent()
    }) {
        panel.setDirectoryURL(Some(&folder));
    }
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    Some(panel.URL()?.path()?.to_string())
}

/// Gives `cargo run` builds the app icon; a bundle has it in `Info.plist`,
/// at every resolution.
fn set_icon_unless_bundled(app: &NSApplication) {
    if NSBundle::mainBundle().bundleIdentifier().is_some() {
        return;
    }
    let data = NSData::with_bytes(include_bytes!("../../assets/icon.png"));
    if let Some(icon) = NSImage::initWithData(NSImage::alloc(), &data) {
        // SAFETY: the image is a valid, retained NSImage.
        unsafe { app.setApplicationIconImage(Some(&icon)) };
    }
}

fn main_window(
    mtm: MainThreadMarker,
) -> (Retained<NSWindow>, Retained<BoardView>, Retained<StatusBar>) {
    let (width, height, bar) = (640.0, 640.0 + status_bar::HEIGHT, status_bar::HEIGHT);
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
    // SAFETY: a plain titled window; it stays alive while the app holds it.
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable
                | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    // SAFETY: the window is owned by `Retained`, not released on close.
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str(&format!(
        "Dame · {}",
        env!("CARGO_PKG_VERSION")
    )));
    window.setContentMinSize(NSSize::new(360.0, 360.0 + bar));
    // The board is framed by a dark canvas, so the chrome is dark as well.
    // SAFETY: the appearance name is an immutable framework constant.
    window.setAppearance(
        NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua }).as_deref(),
    );
    window.setBackgroundColor(Some(&palette::canvas()));

    let content = NSView::initWithFrame(NSView::alloc(mtm), frame);
    let board = BoardView::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, bar), NSSize::new(width, height - bar)),
    );
    board.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
    content.addSubview(&board);

    let status = StatusBar::new(
        mtm,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, bar)),
    );
    status.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMaxYMargin,
    );
    content.addSubview(&status);

    window.setContentView(Some(&content));
    window.center();
    (window, board, status)
}

fn item(
    mtm: MainThreadMarker,
    title: &str,
    action: Option<Sel>,
    key: &str,
) -> Retained<NSMenuItem> {
    // SAFETY: the actions are implemented by the delegate or by AppKit.
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            action,
            &NSString::from_str(key),
        )
    }
}

fn submenu(
    mtm: MainThreadMarker,
    bar: &NSMenu,
    title: &str,
    items: &[Retained<NSMenuItem>],
) -> Retained<NSMenu> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
    for item in items {
        menu.addItem(item);
    }
    let holder = item(mtm, title, None, "");
    holder.setSubmenu(Some(&menu));
    bar.addItem(&holder);
    menu
}

fn main_menu(mtm: MainThreadMarker) -> Retained<NSMenu> {
    let bar = NSMenu::new(mtm);
    let separator = || NSMenuItem::separatorItem(mtm);

    let hide_others = item(mtm, "Hide Others", Some(sel!(hideOtherApplications:)), "h");
    hide_others
        .setKeyEquivalentModifierMask(NSEventModifierFlags::Command | NSEventModifierFlags::Option);
    submenu(
        mtm,
        &bar,
        "Dame",
        &[
            item(
                mtm,
                "About Dame",
                Some(sel!(orderFrontStandardAboutPanel:)),
                "",
            ),
            separator(),
            item(mtm, "Hide Dame", Some(sel!(hide:)), "h"),
            hide_others,
            item(mtm, "Show All", Some(sel!(unhideAllApplications:)), ""),
            separator(),
            item(mtm, "Quit Dame", Some(sel!(terminate:)), "q"),
        ],
    );

    submenu(
        mtm,
        &bar,
        "File",
        &[
            item(mtm, "Open Opening Book…", Some(sel!(openBook:)), "o"),
            item(mtm, "Clear Opening Book", Some(sel!(clearBook:)), ""),
        ],
    );

    submenu(
        mtm,
        &bar,
        "Edit",
        &[
            item(mtm, "Take Back Move", Some(sel!(takeBack:)), "z"),
            separator(),
            item(mtm, "Copy Position", Some(sel!(copyPosition:)), "c"),
            // An upper-case key equivalent includes Shift.
            item(mtm, "Copy Moves", Some(sel!(copyMoves:)), "C"),
            item(mtm, "Paste Position", Some(sel!(pastePosition:)), "v"),
        ],
    );

    let mut game = vec![
        item(mtm, "New Game", Some(sel!(newGame:)), "n"),
        item(mtm, "Move Now", Some(sel!(moveNow:)), "e"),
        item(mtm, "Reload Engine", Some(sel!(reloadEngine:)), "r"),
        separator(),
    ];
    for (tag, players) in Players::ALL.iter().enumerate() {
        let entry = item(mtm, players.title(), Some(sel!(setPlayers:)), "");
        entry.setTag(tag as isize);
        game.push(entry);
    }
    game.push(separator());
    for seconds in MOVE_TIMES {
        let title = match seconds {
            1 => "1 Second per Move".to_owned(),
            s => format!("{s} Seconds per Move"),
        };
        let entry = item(mtm, &title, Some(sel!(setMoveTime:)), "");
        entry.setTag(seconds as isize);
        game.push(entry);
    }
    submenu(mtm, &bar, "Game", &game);

    submenu(
        mtm,
        &bar,
        "View",
        &[item(mtm, "Flip Board", Some(sel!(flipBoard:)), "f")],
    );

    let window = submenu(
        mtm,
        &bar,
        "Window",
        &[
            item(mtm, "Minimize", Some(sel!(performMiniaturize:)), "m"),
            item(mtm, "Zoom", Some(sel!(performZoom:)), ""),
            separator(),
            item(mtm, "Close", Some(sel!(performClose:)), "w"),
        ],
    );
    NSApplication::sharedApplication(mtm).setWindowsMenu(Some(&window));
    bar
}
