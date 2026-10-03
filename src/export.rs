use serde::Serialize;

use crate::piece::{Piece, Puzzle};

#[derive(Debug, Serialize)]
pub struct SolverExport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cols: Option<usize>,
    pub pieces: Vec<Piece>,
}

/// Snapshot the current puzzle for export (generation order, current sides/rotation).
pub fn from_puzzle(puzzle: &Puzzle, include_dimensions: bool) -> SolverExport {
    SolverExport {
        rows: include_dimensions.then_some(puzzle.rows),
        cols: include_dimensions.then_some(puzzle.cols),
        pieces: puzzle.pieces.clone(),
    }
}

pub fn to_json(export: &SolverExport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(export)
}
