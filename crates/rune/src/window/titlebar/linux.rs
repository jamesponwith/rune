//! A Linux window's own frame (`App::titlebar`) where the Wayland compositor draws none: GNOME
//! offers no server-side decorations (`xdg-decoration`), so winit would draw its own light grey
//! bar above the app's titlebar. Rune instead takes the bar away, draws the caption buttons the
//! desktop's button layout asks for (`org.gnome.desktop.wm.preferences` `button-layout`, through
//! the settings portal), and resizes the window from a band along its edges. A compositor that
//! decorates windows itself (KDE, Hyprland, Sway) keeps its frame.

use super::CaptionButton;

/// Points along a frame's edges that resize the window, inside its content.
pub(crate) const EDGE: f64 = 5.0;
/// Points from a corner, along either edge, that resize the window diagonally.
pub(crate) const CORNER: f64 = 16.0;

/// The caption buttons a client frame shows and the edge they sit at. `buttons` are in the
/// order they take from the window's right edge inward, as `Caption::buttons` lists them:
/// a layout on the left is drawn mirrored (`Caption::mirrored`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ButtonLayout {
    pub buttons: [Option<CaptionButton>; 3],
    pub leading: bool,
}

impl Default for ButtonLayout {
    /// Minimize, maximize and close at the right, when the desktop doesn't say.
    fn default() -> Self {
        Self { buttons: CaptionButton::ALL.map(Some), leading: false }
    }
}

impl ButtonLayout {
    /// GNOME's `button-layout`: the left side's buttons, a colon, the right side's, each a
    /// comma-separated list (`appmenu:close`, `close,minimize,maximize:`). Unknown names
    /// (`appmenu`, `spacer`, `icon`) are skipped. The buttons sit together on the side that
    /// holds close, or else the side with more of them.
    // One side only: a layout that splits the buttons across both sides draws them
    // all on one.
    pub(crate) fn parse(layout: &str) -> Self {
        let side = |s: &str| -> Vec<CaptionButton> {
            s.split(',').filter_map(|name| match name.trim() {
                "minimize" => Some(CaptionButton::Minimize),
                "maximize" => Some(CaptionButton::Maximize),
                "close" => Some(CaptionButton::Close),
                _ => None,
            }).collect()
        };
        let (left, right) = layout.split_once(':').map_or((side(layout), Vec::new()), |(l, r)| (side(l), side(r)));
        let close = |v: &[CaptionButton]| v.contains(&CaptionButton::Close);
        let leading = close(&left) || (!close(&right) && left.len() > right.len());
        // Mirrored draws right to left from the left edge, so the left side's order reverses.
        let order: Vec<CaptionButton> = if leading { left.into_iter().rev().collect() } else { right };
        let mut buttons = [None; 3];
        let mut n = 0;
        for b in order {
            if n < 3 && !buttons.contains(&Some(b)) { buttons[n] = Some(b); n += 1; }
        }
        Self { buttons, leading }
    }

    /// The layout the desktop asks for, or [`ButtonLayout::default`].
    pub(crate) fn desktop() -> Self {
        #[cfg(all(target_os = "linux", feature = "dbus"))]
        if let Some(layout) = crate::platform::portal_appearance::button_layout() { return Self::parse(&layout); }
        Self::default()
    }
}

/// The resize edge at `(x, y)` in a window `(w, h)` points: within [`EDGE`] of a side, and
/// within [`CORNER`] of a corner along it for the diagonals.
pub(crate) fn edge_at((x, y): (f64, f64), (w, h): (f64, f64)) -> Option<winit::window::ResizeDirection> {
    use winit::window::ResizeDirection as D;
    let near = |p: f64, len: f64, band: f64| if p < band { -1 } else if p >= len - band { 1 } else { 0 };
    let (sx, sy) = (near(x, w, EDGE), near(y, h, EDGE));
    // On a side, the stretch near a corner turns diagonal.
    let (sx, sy) = match (sx, sy) {
        (0, 0) => return None,
        (0, sy) => (near(x, w, CORNER), sy),
        (sx, 0) => (sx, near(y, h, CORNER)),
        both => both,
    };
    Some(match (sx, sy) {
        (-1, -1) => D::NorthWest,
        (1, -1) => D::NorthEast,
        (-1, 1) => D::SouthWest,
        (1, 1) => D::SouthEast,
        (0, -1) => D::North,
        (0, _) => D::South,
        (-1, _) => D::West,
        _ => D::East,
    })
}

/// The pointer's shape over `edge`.
#[cfg(all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android")))]
pub(crate) fn edge_cursor(edge: winit::window::ResizeDirection) -> winit::window::CursorIcon {
    use winit::window::{CursorIcon as C, ResizeDirection as D};
    match edge {
        D::North => C::NResize,
        D::South => C::SResize,
        D::East => C::EResize,
        D::West => C::WResize,
        D::NorthEast => C::NeResize,
        D::NorthWest => C::NwResize,
        D::SouthEast => C::SeResize,
        D::SouthWest => C::SwResize,
    }
}

/// Whether the Wayland compositor draws window frames itself: it offers `xdg-decoration`. Asked
/// once, on winit's own connection. Not Wayland, or no answer, counts as yes: the native frame
/// stays.
#[cfg(all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android")))]
pub(crate) fn server_decorates(event_loop: &winit::event_loop::ActiveEventLoop) -> bool {
    use std::sync::OnceLock;
    use wayland_client::backend::Backend;
    use wayland_client::globals::{GlobalListContents, registry_queue_init};
    use wayland_client::protocol::wl_registry::{self, WlRegistry};
    use wayland_client::{Connection, Dispatch, QueueHandle};
    use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

    struct Probe;
    impl Dispatch<WlRegistry, GlobalListContents> for Probe {
        fn event(_: &mut Self, _: &WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
    }
    static ANSWER: OnceLock<bool> = OnceLock::new();
    *ANSWER.get_or_init(|| {
        let Ok(RawDisplayHandle::Wayland(display)) = event_loop.display_handle().map(|d| d.as_raw()) else { return true };
        // SAFETY: winit owns this `wl_display` for the life of the event loop; a foreign
        // backend never closes it.
        let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
        let Ok((globals, _queue)) = registry_queue_init::<Probe>(&Connection::from_backend(backend)) else { return true };
        globals.contents().with_list(|list| list.iter().any(|g| g.interface == "zxdg_decoration_manager_v1"))
    })
}

#[cfg(test)]
#[path = "linux_tests.rs"]
mod tests;
