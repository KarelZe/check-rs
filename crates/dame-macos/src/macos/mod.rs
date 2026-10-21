//! The `AppKit` front end.
//!
//! `AppKit` objects live on the main thread, and so does the [`App`]
//! controller: it sits in a thread-local, and the Objective-C classes (the
//! application delegate and the board view) forward events to it through
//! [`with_app`]. The engine's reader thread wakes the controller through the
//! main dispatch queue.

mod app;
mod board_view;
mod delegate;
mod palette;
mod status_bar;

use std::cell::RefCell;

use dispatch2::DispatchQueue;
use objc2::MainThreadMarker;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

use app::App;
use delegate::AppDelegate;

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` on the controller. Returns `None` before launch, after quitting,
/// and when the controller is already in use further up the stack.
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|app| app.try_borrow_mut().ok()?.as_mut().map(f))
}

/// Runs on the main queue after the engine sent something.
fn wake_app() {
    let busy = APP.with(|app| match app.try_borrow_mut() {
        Ok(mut app) => {
            if let Some(app) = app.as_mut() {
                app.drain_events();
            }
            false
        }
        Err(_) => true,
    });
    if busy {
        DispatchQueue::main().exec_async(wake_app);
    }
}

pub(crate) fn run() {
    let mtm = MainThreadMarker::new().expect("the app must start on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let delegate = AppDelegate::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();
}
