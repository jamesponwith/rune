use super::*;
use winit::window::ResizeDirection as D;

use CaptionButton::{Close, Maximize, Minimize};

#[test]
fn button_layout_follows_gnome_setting() {
    // GNOME's default: close alone, at the right.
    assert_eq!(ButtonLayout::parse("appmenu:close"), ButtonLayout { buttons: [Some(Close), None, None], leading: false });
    assert_eq!(ButtonLayout::parse("appmenu:minimize,maximize,close"), ButtonLayout::default());
    // At the left, listed from the right edge inward: drawn mirrored, close leftmost.
    assert_eq!(ButtonLayout::parse("close,minimize,maximize:"), ButtonLayout { buttons: [Some(Maximize), Some(Minimize), Some(Close)], leading: true });
    // Split across sides: together on the side with close.
    assert_eq!(ButtonLayout::parse("close:maximize").buttons, [Some(Close), None, None]);
    assert!(ButtonLayout::parse("close:maximize").leading);
    assert_eq!(ButtonLayout::parse(""), ButtonLayout { buttons: [None; 3], leading: false });
    assert_eq!(ButtonLayout::parse(":close,close").buttons, [Some(Close), None, None]);
}

#[test]
fn edges_resize_and_corners_go_diagonal() {
    let size = (800.0, 600.0);
    assert_eq!(edge_at((400.0, 300.0), size), None);
    assert_eq!(edge_at((400.0, 1.0), size), Some(D::North));
    assert_eq!(edge_at((400.0, 598.0), size), Some(D::South));
    assert_eq!(edge_at((0.0, 300.0), size), Some(D::West));
    assert_eq!(edge_at((799.0, 300.0), size), Some(D::East));
    assert_eq!(edge_at((2.0, 2.0), size), Some(D::NorthWest));
    // Along the top, near the right corner: diagonal.
    assert_eq!(edge_at((790.0, 2.0), size), Some(D::NorthEast));
    assert_eq!(edge_at((2.0, 590.0), size), Some(D::SouthWest));
    assert_eq!(edge_at((797.0, 597.0), size), Some(D::SouthEast));
}
