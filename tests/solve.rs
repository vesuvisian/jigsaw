use eframe::egui::Vec2;
use jigsaw::board::{Board, CELL, sides_match};
use jigsaw::generate::generate;
use jigsaw::piece::{BOTTOM, LEFT, RIGHT, TOP};
use jigsaw::solve::{SolveStepResult, is_edge_piece, solve_step};

fn unique_groups(board: &Board) -> usize {
    let set: std::collections::HashSet<_> = board.poses.iter().map(|p| p.group).collect();
    set.len()
}

fn split_singletons(board: &mut Board) {
    for (i, pose) in board.poses.iter_mut().enumerate() {
        pose.group = i as u32;
    }
}

#[test]
fn solve_step_on_assembled_is_complete() {
    let mut puzzle = generate(3, 3);
    let mut board = Board::assembled(3, 3);
    assert_eq!(
        solve_step(&mut puzzle, &mut board),
        SolveStepResult::Complete
    );
    assert_eq!(unique_groups(&board), 1);
}

#[test]
fn solve_step_connects_one_pair() {
    let mut puzzle = generate(2, 2);
    let mut board = Board::assembled(2, 2);
    split_singletons(&mut board);
    // Nudge pieces apart so placement is visible.
    board.poses[1].pos.x += 40.0;
    board.poses[2].pos.y += 40.0;
    board.poses[3].pos += Vec2::new(40.0, 40.0);

    let before = unique_groups(&board);
    let result = solve_step(&mut puzzle, &mut board);
    assert!(matches!(
        result,
        SolveStepResult::Connected { group_size: 2, .. }
    ));
    assert_eq!(unique_groups(&board), before - 1);

    let SolveStepResult::Connected {
        piece_a, piece_b, ..
    } = result
    else {
        unreachable!();
    };
    assert_eq!(board.group_of(piece_a), board.group_of(piece_b));

    // Matching sides must face each other after the step.
    let pa = &puzzle.pieces[piece_a];
    let pb = &puzzle.pieces[piece_b];
    let facing = (0..4).any(|side| {
        let opp = (side + 2) % 4;
        sides_match(pa.sides[side], pb.sides[opp])
    });
    assert!(
        facing,
        "connected pieces must have complementary sides facing"
    );
}

#[test]
fn solve_step_rotates_misoriented_partner() {
    let mut puzzle = generate(1, 2);
    let mut board = Board::assembled(1, 2);
    split_singletons(&mut board);

    // Rotate piece 1 so its matching side no longer faces piece 0.
    puzzle.pieces[1].rotate_cw();
    board.poses[1].pos = Vec2::new(CELL * 2.0, CELL);

    assert!(!sides_match(
        puzzle.pieces[0].sides[RIGHT],
        puzzle.pieces[1].sides[LEFT]
    ));

    let result = solve_step(&mut puzzle, &mut board);
    assert!(matches!(result, SolveStepResult::Connected { .. }));
    assert_eq!(board.group_of(0), board.group_of(1));

    // After solve, complementary values face on an opposite pair of sides.
    let facing = [TOP, RIGHT, BOTTOM, LEFT].into_iter().any(|side| {
        let opp = (side + 2) % 4;
        sides_match(puzzle.pieces[0].sides[side], puzzle.pieces[1].sides[opp])
    });
    assert!(facing);

    let d = board.poses[1].pos - board.poses[0].pos;
    let flush = (d.x.abs() < 0.01 && (d.y.abs() - CELL).abs() < 0.01)
        || (d.y.abs() < 0.01 && (d.x.abs() - CELL).abs() < 0.01);
    assert!(
        flush,
        "pieces should sit flush after solve step, delta={d:?}"
    );
}

#[test]
fn solve_step_prefers_edge_pieces_first() {
    let mut puzzle = generate(3, 3);
    let mut board = Board::assembled(3, 3);
    split_singletons(&mut board);

    let center = 4; // row-major 3×3
    assert!(!is_edge_piece(&puzzle.pieces[center]));

    // First 7 merges should only involve rim pieces (8 rim + 1 center → 2 groups).
    for step in 0..7 {
        let result = solve_step(&mut puzzle, &mut board);
        let SolveStepResult::Connected {
            piece_a, piece_b, ..
        } = result
        else {
            panic!("expected connection at step {step}");
        };
        assert!(
            is_edge_piece(&puzzle.pieces[piece_a]) && is_edge_piece(&puzzle.pieces[piece_b]),
            "step {step}: expected edge–edge connect, got {piece_a} and {piece_b}"
        );
        assert_ne!(piece_a, center);
        assert_ne!(piece_b, center);
    }

    assert_eq!(unique_groups(&board), 2);

    let result = solve_step(&mut puzzle, &mut board);
    let SolveStepResult::Connected {
        piece_a, piece_b, ..
    } = result
    else {
        panic!("expected center connection");
    };
    assert!(piece_a == center || piece_b == center);
    assert_eq!(unique_groups(&board), 1);
    assert_eq!(
        solve_step(&mut puzzle, &mut board),
        SolveStepResult::Complete
    );
}

#[test]
fn is_edge_piece_detects_zeros() {
    let puzzle = generate(2, 2);
    assert!(puzzle.pieces.iter().all(is_edge_piece));

    let puzzle = generate(3, 3);
    assert!(is_edge_piece(&puzzle.pieces[0]));
    assert!(!is_edge_piece(&puzzle.pieces[4]));
}
