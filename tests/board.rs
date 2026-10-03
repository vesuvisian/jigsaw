use eframe::egui::Vec2;
use jigsaw::board::{sides_match, Board, CELL};
use jigsaw::generate::generate;
use jigsaw::piece::{BOTTOM, TOP};

#[test]
fn assembled_is_one_group() {
    let board = Board::assembled(2, 3);
    assert_eq!(board.poses.len(), 6);
    assert!(board.poses.iter().all(|p| p.group == 0));
    assert_eq!(board.poses[0].pos, Vec2::ZERO);
    assert_eq!(board.poses[1].pos, Vec2::new(CELL, 0.0));
    assert_eq!(board.poses[3].pos, Vec2::new(0.0, CELL));
}

#[test]
fn snap_merges_matching_neighbors() {
    let puzzle = generate(2, 2);
    let mut board = Board::assembled(2, 2);
    // Separate into singletons but keep solved positions (almost).
    for (i, pose) in board.poses.iter_mut().enumerate() {
        pose.group = i as u32;
    }
    // Nudge piece 1 slightly away from piece 0 (horizontal neighbors).
    board.poses[1].pos.x += 10.0;
    let merged = board.try_snap(&puzzle, 1);
    assert_eq!(merged, Some(0));
    assert_eq!(board.poses[0].group, board.poses[1].group);
    assert!((board.poses[1].pos.x - CELL).abs() < 0.01);
    assert!((board.poses[1].pos.y).abs() < 0.01);
}

#[test]
fn rotate_group_keeps_joined_seams_facing() {
    let mut puzzle = generate(1, 2);
    let mut board = Board::assembled(1, 2);
    assert_eq!(board.poses[0].group, board.poses[1].group);

    let group = board.poses[0].group;
    board.rotate_group_poses_cw(group);
    for i in board.group_members(group) {
        puzzle.pieces[i].rotate_cw();
    }

    // After CW: piece 0 (was left) is above piece 1 (was right).
    let p0 = board.poses[0].pos;
    let p1 = board.poses[1].pos;
    assert!((p0.x - p1.x).abs() < 0.01, "should stay horizontally aligned");
    assert!((p1.y - p0.y - CELL).abs() < 0.01, "p0 should sit flush above p1");
    assert!(
        sides_match(
            puzzle.pieces[0].sides[BOTTOM],
            puzzle.pieces[1].sides[TOP]
        ),
        "joined values must face each other after CW rotate"
    );
}

#[test]
fn sides_match_opposite_nonzero() {
    assert!(sides_match(3, -3));
    assert!(!sides_match(0, 0));
    assert!(!sides_match(1, 1));
}

#[test]
fn scramble_splits_and_keeps_ids() {
    let mut puzzle = generate(3, 3);
    let ids_before: Vec<_> = puzzle.pieces.iter().map(|p| p.id).collect();
    let mut board = Board::assembled(3, 3);
    board.scramble(&mut puzzle, Board::play_size(3, 3), &mut rand::rng());
    let ids_after: Vec<_> = puzzle.pieces.iter().map(|p| p.id).collect();
    assert_eq!(ids_before, ids_after);
    let groups: std::collections::HashSet<_> = board.poses.iter().map(|p| p.group).collect();
    assert_eq!(groups.len(), 9);
    for pose in &board.poses {
        assert!(pose.pos.x >= 0.0);
        assert!(pose.pos.y >= 0.0);
        assert!(pose.pos.x <= Board::play_size(3, 3).x - CELL + 0.01);
        assert!(pose.pos.y <= Board::play_size(3, 3).y - CELL + 0.01);
    }
}
