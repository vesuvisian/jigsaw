use std::collections::{HashMap, HashSet};

use eframe::egui::Vec2;

use crate::board::{sides_match, Board};
use crate::piece::{Piece, Puzzle};

/// Result of attempting one automatic connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolveStepResult {
    /// Two pieces were joined into one group.
    Connected {
        piece_a: usize,
        piece_b: usize,
        merged_group: u32,
        group_size: usize,
    },
    /// All pieces are already in a single group.
    Complete,
}

/// True if the piece has any outer (zero) edge — rotation-invariant.
pub fn is_edge_piece(piece: &Piece) -> bool {
    piece.sides.iter().any(|&s| s == 0)
}

fn edge_tier(puzzle: &Puzzle, a: usize, b: usize) -> u8 {
    let ea = is_edge_piece(&puzzle.pieces[a]);
    let eb = is_edge_piece(&puzzle.pieces[b]);
    match (ea, eb) {
        (true, true) => 0,
        (true, false) | (false, true) => 1,
        (false, false) => 2,
    }
}

/// Collect unmatched complementary seams (different groups), edge-first order.
/// Returns `(piece_a, side_a, piece_b, side_b)` with `piece_a < piece_b`.
pub fn find_next_connection(puzzle: &Puzzle, board: &Board) -> Option<(usize, usize, usize, usize)> {
    // side value → (piece, side index)
    let mut by_value: HashMap<i128, (usize, usize)> = HashMap::new();
    for (i, piece) in puzzle.pieces.iter().enumerate() {
        for (side, &v) in piece.sides.iter().enumerate() {
            if v != 0 {
                by_value.insert(v, (i, side));
            }
        }
    }

    let mut best: Option<(u8, usize, usize, usize, usize)> = None; // tier, a, a_side, b, b_side

    for (i, piece) in puzzle.pieces.iter().enumerate() {
        for (side, &v) in piece.sides.iter().enumerate() {
            if v == 0 {
                continue;
            }
            let Some(&(j, j_side)) = by_value.get(&(-v)) else {
                continue;
            };
            if i >= j {
                continue; // emit each unordered pair once
            }
            if board.group_of(i) == board.group_of(j) {
                continue;
            }
            debug_assert!(sides_match(v, puzzle.pieces[j].sides[j_side]));
            let tier = edge_tier(puzzle, i, j);
            let key = (tier, i, side, j, j_side);
            match best {
                Some(cur) if key >= cur => {}
                _ => best = Some(key),
            }
        }
    }

    best.map(|(_, a, a_side, b, b_side)| (a, a_side, b, b_side))
}

fn rotate_group_cw(puzzle: &mut Puzzle, board: &mut Board, group: u32) {
    board.rotate_group_poses_cw(group);
    for i in board.group_members(group) {
        puzzle.pieces[i].rotate_cw();
    }
}

fn side_index_of(piece: &Piece, value: i128) -> Option<usize> {
    piece.sides.iter().position(|&s| s == value)
}

/// Opposite facing slot: top↔bottom, right↔left.
fn opposite_side(side: usize) -> usize {
    (side + 2) % 4
}

/// Make one edge-first connection: rotate/move the smaller group onto the larger.
pub fn solve_step(puzzle: &mut Puzzle, board: &mut Board) -> SolveStepResult {
    let n = puzzle.pieces.len();
    if n == 0 {
        return SolveStepResult::Complete;
    }
    let groups: HashSet<u32> = board.poses.iter().map(|p| p.group).collect();
    if groups.len() <= 1 {
        return SolveStepResult::Complete;
    }

    let Some((a, a_side, b, b_side)) = find_next_connection(puzzle, board) else {
        // Valid puzzles should always have a cross-group seam until one group remains.
        return SolveStepResult::Complete;
    };

    let group_a = board.group_of(a);
    let group_b = board.group_of(b);
    let size_a = board.group_members(group_a).len();
    let size_b = board.group_members(group_b).len();

    // Keep the larger group fixed; on a tie, keep `a`'s group fixed.
    let (anchor, mover, mover_value, fixed_group, moving_group) = if size_b > size_a {
        let mover_value = puzzle.pieces[a].sides[a_side];
        (b, a, mover_value, group_b, group_a)
    } else {
        let mover_value = puzzle.pieces[b].sides[b_side];
        (a, b, mover_value, group_a, group_b)
    };

    let anchor_side =
        side_index_of(&puzzle.pieces[anchor], -mover_value).expect("anchor has match value");
    let desired_mover_side = opposite_side(anchor_side);

    // Rotate moving group until mover's complementary value faces the anchor.
    for _ in 0..4 {
        let current = side_index_of(&puzzle.pieces[mover], mover_value).expect("mover has value");
        if current == desired_mover_side {
            break;
        }
        rotate_group_cw(puzzle, board, moving_group);
    }

    debug_assert_eq!(
        side_index_of(&puzzle.pieces[mover], mover_value),
        Some(desired_mover_side)
    );
    debug_assert!(sides_match(
        puzzle.pieces[anchor].sides[anchor_side],
        puzzle.pieces[mover].sides[desired_mover_side]
    ));

    let desired_pos = Board::adjacent_pos(board.poses[anchor].pos, anchor_side);
    let delta: Vec2 = desired_pos - board.poses[mover].pos;
    board.move_group(moving_group, delta);
    board.merge_groups(moving_group, fixed_group);
    board.bring_group_to_front(fixed_group);

    let group_size = board.group_members(fixed_group).len();
    SolveStepResult::Connected {
        piece_a: a.min(b),
        piece_b: a.max(b),
        merged_group: fixed_group,
        group_size,
    }
}
