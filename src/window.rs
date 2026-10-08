use std::cell::Cell;
use std::rc::Rc;

use freya::prelude::*;
use freya::winit::window::WindowId;

/// Owns a window opened with [`spawn_window`], and closes it when dropped: keep it in a
/// component's state and the window goes away with the component.
pub struct WindowHandle {
    slot: Rc<Cell<Slot>>,
    // Kept so closing works outside of a component, like on drop.
    platform: Platform,
}

#[derive(Clone, Copy)]
enum Slot {
    /// Launched, but the event loop hasn't given back an id yet.
    Pending,
    Open(WindowId),
    Closed,
}

/// Opens a window for `config`. Replaces `config`'s on close hook, which the handle
/// needs to notice the window was closed from outside.
///
/// Must be called from the UI thread, inside a component or event handler. Requests go
/// through the window it's called from, so they're lost once that window is closed.
pub fn spawn_window(config: WindowConfig) -> WindowHandle {
    let platform = Platform::get();
    let slot = Rc::new(Cell::new(Slot::Pending));
    let config = config.with_on_close({
        let slot = slot.clone();
        move |_, _| {
            slot.set(Slot::Closed);
            CloseDecision::Close
        }
    });

    // Not tied to the caller, so a window opened just before it unmounts still gets closed.
    spawn_forever({
        let (slot, platform) = (slot.clone(), platform.clone());

        async move {
            let id = platform.launch_window(config).await;
            match slot.get() {
                Slot::Pending => slot.set(Slot::Open(id)),
                // Closed before it was open.
                Slot::Closed => platform.close_window(id),
                Slot::Open(window_id) => {
                    unreachable!("WindowHandle slot was already open with id {window_id:?}")
                }
            }
        }
    });

    WindowHandle { slot, platform }
}

impl WindowHandle {
    /// False once it's closed, by [`close`](Self::close) or from outside.
    pub fn is_open(&self) -> bool {
        !matches!(self.slot.get(), Slot::Closed)
    }

    /// Does nothing while it's still opening, or after it's closed.
    pub fn focus(&self) {
        if let Slot::Open(id) = self.slot.get() {
            self.platform.focus_window(id);
        }
    }
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
        if let Slot::Open(id) = self.slot.replace(Slot::Closed) {
            self.platform.close_window(id);
        }
    }
}
