//! What a click on a unit hits, as the original decides it (notes/findings-input.md "What a click hits", measured
//! with the harness hit masks `hit_*`). The detail pane's click walks the object list (sorted by pixel y) through
//! MapObjectManager's point query (0x437940/0x437990), which for each object tests:
//!
//! 1. its selection box (GetSelectionBoxPos/Size, vtable +0x34/+0x30): the current animation's selection rectangle,
//!    placed by the animation's displacement, half-open (`left <= x < left + width`). A vehicle's box comes from its
//!    chassis animation for the heading's 16-way column (0x44CDA0), and that is all a vehicle needs;
//! 2. for a structure, also MouseOver (vtable +0x38, Building 0x4093D0): the pixel under the click must be set in one
//!    of the current frame's images ([`Sprites::hit`], 0x404F40). A Guard Post skips this (MapObject flag 0x80,
//!    0x437A7C), so its whole box counts. Five kinds of structure also accept the pixels of a second animation's
//!    first frame when their own frame misses ([`overlay`]): Agridome, Common and Rare Storage, Garage, mines.
//!
//! The query only walks objects within 96 rows of the click and skips docking units (action ObjDocking). Which hit
//! wins is the caller's business: selection takes the last own unit in list order (the one drawn on top), command
//! targets the first unit of any player ([`select`], [`target`]).

use crate::art::Sprites;

/// A structure's second hit animation (art index), tested at frame 0 at the same offset into the box. `main_rect`:
/// placed by the structure's own animation's selection rectangle (mines) rather than its own (the others).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overlay {
    pub anim: usize,
    pub main_rect: bool,
}

/// A unit's click shape: an animation (art index) and, for pixel-tested structures, the frame and the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// The animation's selection box (vehicles, Guard Posts).
    Box { anim: usize },
    /// The box, then the frame's pixels (or the overlay's).
    Pixels { anim: usize, frame: usize, overlay: Option<Overlay> },
}

/// The selection box of an animation relative to the object's pixel: `(left, top, width, height)`.
pub fn selection_box(sprites: &Sprites, anim: usize) -> Option<(i32, i32, i32, i32)> {
    let a = sprites.animation(anim)?;
    let (r, (dx, dy)) = (&a.selection, a.displacement);
    Some((r.left - dx, r.top - dy, r.right - r.left, r.bottom - r.top))
}

/// Whether a click `(dx, dy)` pixels from the object's pixel hits a unit of this shape.
pub fn hits(sprites: &Sprites, shape: Shape, dx: i32, dy: i32) -> bool {
    let anim = match shape {
        Shape::Box { anim } | Shape::Pixels { anim, .. } => anim,
    };
    let Some((left, top, w, h)) = selection_box(sprites, anim) else { return false };
    if dx < left || dx >= left + w || dy < top || dy >= top + h {
        return false;
    }
    let Shape::Pixels { frame, overlay, .. } = shape else { return true };
    let main = sprites.animation(anim).expect("checked by selection_box");
    // MouseOver gets the offset into the box; 0x404F40 adds the selection corner of the animation that places it.
    let (lx, ly) = (dx - left, dy - top);
    if sprites.hit(anim, frame, lx + main.selection.left, ly + main.selection.top) == Some(true) {
        return true;
    }
    overlay.is_some_and(|o| {
        let place = if o.main_rect { Some(main) } else { sprites.animation(o.anim) };
        place.is_some_and(|p| sprites.hit(o.anim, 0, lx + p.selection.left, ly + p.selection.top) == Some(true))
    })
}

/// A structure's overlay for its animation state (MapObject +0x36: 0 built, 5 operating, ...), by structure code:
/// Agridome 0x401B60 (game ids 0x87F and 0x880 in states 1 and 2, none in 3 and 4, else 0x87E), Common and Rare
/// Storage 0x409780/0x409A30 (0x881..0x883, the same pattern), Garage 0x41D540 (states 0-2 only: 0x85A/0x85C/0x85F
/// Eden, 0x214/0x216/0x218 Plymouth), the mines and the Magma Well 0x44B430 (0x747 always, placed by the mine's own
/// rectangle). Game ids are art index + 101. The game also drops the overlay while the structure's command is 0x21
/// or 0x2F.
pub fn overlay(code: &str, state: u16, plymouth: bool) -> Option<Overlay> {
    let by_state = |base: usize| match state {
        1 => Some(base + 1),
        2 => Some(base + 2),
        3 | 4 => None,
        _ => Some(base),
    };
    let id = match code {
        "AGRIDOME" => by_state(0x87E)?,
        "STORAGE_ORE" | "STORAGE_RARE" => by_state(0x881)?,
        "GARAGE" => {
            let ids = if plymouth { [0x214, 0x216, 0x218] } else { [0x85A, 0x85C, 0x85F] };
            *ids.get(state as usize)?
        }
        "MINE" | "MINE_ADV" | "MAGMA_WELL" => return Some(Overlay { anim: 0x747 - 101, main_rect: true }),
        _ => return None,
    };
    Some(Overlay { anim: id - 101, main_rect: false })
}

/// A unit the point query considers, in object-list order (pixel y, as the game sorts it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    /// The object's pixel (its sprite's reference point).
    pub x: i32,
    pub y: i32,
    pub owner: u8,
    pub shape: Shape,
}

/// The query only walks objects whose pixel y is within 96 rows of the click (0x437949, 0x437963).
const Y_REACH: i32 = 0x60;

/// The part of `list` (sorted by `y`) the query walks for a click on row `y`, as an index range.
fn window(list: &[Candidate], y: i32) -> std::ops::Range<usize> {
    list.partition_point(|c| c.y < y - Y_REACH)..list.partition_point(|c| c.y < y + Y_REACH)
}

/// What a left click at map pixel `(x, y)` selects for `player` (0x41B00D): the last of the player's units hit, in
/// list order, i.e. the one drawn on top. `list` is in object-list order (sorted by `y`); returns an index into it.
pub fn select(sprites: &Sprites, list: &[Candidate], x: i32, y: i32, player: u8) -> Option<usize> {
    window(list, y).rev().find(|&i| list[i].owner == player && hits(sprites, list[i].shape, x - list[i].x, y - list[i].y))
}

/// The unit a command (Attack, Guard, Repair, ...) targets at `(x, y)` (e.g. 0x4533D0): the first unit of any player
/// hit, in list order, i.e. the one drawn underneath where they overlap.
pub fn target(sprites: &Sprites, list: &[Candidate], x: i32, y: i32) -> Option<usize> {
    window(list, y).find(|&i| hits(sprites, list[i].shape, x - list[i].x, y - list[i].y))
}
