//! Keeps a window's size in step with its scale factor when the display changes.
//!
//! When a monitor wakes up, Windows can bring it back for a moment at another resolution and
//! scale. Slint applies the new scale factor right away but keeps the old size in logical pixels
//! until the resize that follows, so in between a frame can be drawn for a window bigger than its
//! drawing buffer. The software renderer panics on that, inside a Windows callback, which aborts
//! the app (seen as 1024×768 at scale 1 turning into 1433×1075 at scale 1.4).
//!
//! `follow_scale_factor` handles the scale change itself: it applies the new factor and, at the
//! same moment, sets the logical size that matches the size the window has now. The resize
//! Windows sends next then brings it to its real size as usual.

use slint::platform::WindowEvent as SlintEvent;
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::winit_030::winit::event::WindowEvent;

/// To be called first from every `on_winit_window_event` handler; when it returns
/// `PreventDefault`, the handler must return that too
pub fn follow_scale_factor(window: &slint::Window, event: &WindowEvent) -> EventResult {
    let WindowEvent::ScaleFactorChanged { scale_factor, .. } = event else {
        return EventResult::Propagate;
    };
    let scale_factor = *scale_factor as f32;
    // In physical pixels: the size the drawing buffer has
    let size = window.size();
    window.dispatch_event(SlintEvent::ScaleFactorChanged { scale_factor });
    if size.width > 0 && size.height > 0 {
        window.dispatch_event(SlintEvent::Resized { size: size.to_logical(scale_factor) });
    }
    EventResult::PreventDefault
}

/// For windows that don't need a handler of their own
pub fn install(window: &slint::Window) {
    window.on_winit_window_event(follow_scale_factor);
}
