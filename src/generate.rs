use std::collections::HashSet;

use rand::Rng;

use crate::piece::{BOTTOM, LEFT, Piece, Puzzle, RIGHT, TOP};

fn next_side_id(rng: &mut impl Rng, used: &mut HashSet<u128>) -> i128 {
    loop {
        // Positive magnitudes only; never i128::MIN so negation is always safe.
        let v = rng.random_range(1..=i128::MAX) as u128;
        if used.insert(v) {
            return v as i128;
        }
    }
}

/// Unique magnitude, with a random sign for which side of the seam is positive.
fn signed_side_id(rng: &mut impl Rng, used: &mut HashSet<u128>) -> i128 {
    let v = next_side_id(rng, used);
    if rng.random_bool(0.5) { v } else { -v }
}

/// Generate an M×N puzzle with unique integer side pairings.
/// Outer edges are `0`; interior shared edges are `v` / `-v` with unique `|v|`.
/// Each non-zero value fully determines that edge's interlocking shape.
pub fn generate(rows: usize, cols: usize) -> Puzzle {
    assert!(rows >= 1 && cols >= 1);

    let n = rows * cols;
    let mut pieces: Vec<Piece> = (0..n)
        .map(|_| Piece {
            id: uuid::Uuid::new_v4(),
            rotation: 0,
            sides: [0; 4],
        })
        .collect();

    let mut rng = rand::rng();
    let mut used = HashSet::new();

    // Horizontal seams: piece (r,c).right ↔ piece (r,c+1).left
    for r in 0..rows {
        for c in 0..(cols - 1) {
            let v = signed_side_id(&mut rng, &mut used);
            let left_idx = r * cols + c;
            let right_idx = r * cols + c + 1;
            pieces[left_idx].sides[RIGHT] = v;
            pieces[right_idx].sides[LEFT] = -v;
        }
    }

    // Vertical seams: piece (r,c).bottom ↔ piece (r+1,c).top
    for r in 0..(rows - 1) {
        for c in 0..cols {
            let v = signed_side_id(&mut rng, &mut used);
            let top_idx = r * cols + c;
            let bottom_idx = (r + 1) * cols + c;
            pieces[top_idx].sides[BOTTOM] = v;
            pieces[bottom_idx].sides[TOP] = -v;
        }
    }

    Puzzle { rows, cols, pieces }
}
