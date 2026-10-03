use jigsaw::export::from_puzzle;
use jigsaw::generate::generate;
use std::collections::HashSet;

#[test]
fn generate_assigns_unique_uuids() {
    let puzzle = generate(3, 3);
    let ids: HashSet<_> = puzzle.pieces.iter().map(|p| p.id).collect();
    assert_eq!(ids.len(), puzzle.pieces.len());
}

#[test]
fn from_puzzle_exports_all_pieces() {
    let puzzle = generate(2, 3);
    let export = from_puzzle(&puzzle, true);
    assert_eq!(export.rows, Some(2));
    assert_eq!(export.cols, Some(3));
    assert_eq!(export.pieces.len(), 6);
    assert_eq!(export.pieces[0].id, puzzle.pieces[0].id);
}
