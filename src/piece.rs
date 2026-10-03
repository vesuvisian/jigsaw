use uuid::Uuid;

/// Side index: top.
pub const TOP: usize = 0;
/// Side index: right.
pub const RIGHT: usize = 1;
/// Side index: bottom.
pub const BOTTOM: usize = 2;
/// Side index: left.
pub const LEFT: usize = 3;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Piece {
    /// Opaque unique id (UUID); not related to grid position.
    pub id: Uuid,
    /// Quarter-turns clockwise from generation orientation.
    pub rotation: u8,
    /// Side values in order: `[top, right, bottom, left]`.
    ///
    /// `0` is a flat outer edge. Non-zero values fully determine the interlocking
    /// spline: sign is outie (+) / innie (−), and `|value|` seeds the silhouette.
    pub sides: [i128; 4],
}

#[derive(Debug, Clone)]
pub struct Puzzle {
    pub rows: usize,
    pub cols: usize,
    /// Piece definitions in generation (solved row-major) order.
    /// Array index is the stable handle used by the board.
    pub pieces: Vec<Piece>,
}

impl Piece {
    /// Cycle sides 90° clockwise without changing `rotation`.
    /// `[top, right, bottom, left]` → `[left, top, right, bottom]`.
    pub fn cycle_sides_cw(&mut self) {
        self.sides = [
            self.sides[LEFT],
            self.sides[TOP],
            self.sides[RIGHT],
            self.sides[BOTTOM],
        ];
    }

    /// Rotate this piece 90° CW (sides + rotation field).
    pub fn rotate_cw(&mut self) {
        self.cycle_sides_cw();
        self.rotation = (self.rotation + 1) % 4;
    }
}
