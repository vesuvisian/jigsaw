use std::collections::HashSet;

use eframe::egui::Vec2;
use rand::Rng;

use crate::piece::{BOTTOM, LEFT, Puzzle, RIGHT, TOP};
use crate::shape::tab_extent;

/// Piece side length in canvas units (matches UI cell size).
pub const CELL: f32 = 120.0;
/// Max distance for a matching seam to snap.
pub const SNAP_THRESHOLD: f32 = 24.0;

/// Per-piece play state on the free canvas.
#[derive(Debug, Clone)]
pub struct PiecePose {
    /// Top-left position in canvas space.
    pub pos: Vec2,
    /// Shared id for pieces locked together.
    pub group: u32,
    /// Draw / hit-test order (higher = on top).
    pub z: u32,
}

/// Free-position board: poses + group / z helpers for a puzzle.
#[derive(Debug, Clone)]
pub struct Board {
    pub poses: Vec<PiecePose>,
    next_z: u32,
}

impl Board {
    /// Place all pieces flush in a solved lattice as one connected group.
    pub fn assembled(rows: usize, cols: usize) -> Self {
        let n = rows * cols;
        let poses = (0..n)
            .map(|i| {
                let r = i / cols;
                let c = i % cols;
                PiecePose {
                    pos: Vec2::new(c as f32 * CELL, r as f32 * CELL),
                    group: 0,
                    z: i as u32,
                }
            })
            .collect();
        Self {
            poses,
            next_z: n as u32,
        }
    }

    pub fn play_size(rows: usize, cols: usize) -> Vec2 {
        let assembled = Vec2::new(cols as f32 * CELL, rows as f32 * CELL);
        // Room to scatter: ~1.5× assembled in each axis.
        Vec2::new(
            (assembled.x * 1.5).max(assembled.x + CELL),
            (assembled.y * 1.5).max(assembled.y + CELL),
        )
    }

    pub fn group_of(&self, piece: usize) -> u32 {
        self.poses[piece].group
    }

    pub fn group_members(&self, group: u32) -> Vec<usize> {
        self.poses
            .iter()
            .enumerate()
            .filter(|(_, p)| p.group == group)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn move_group(&mut self, group: u32, delta: Vec2) {
        for pose in &mut self.poses {
            if pose.group == group {
                pose.pos += delta;
            }
        }
    }

    /// Reassign every piece in `from` into `into`.
    pub fn merge_groups(&mut self, from: u32, into: u32) {
        if from == into {
            return;
        }
        for pose in &mut self.poses {
            if pose.group == from {
                pose.group = into;
            }
        }
    }

    /// Top-left of a piece sitting on `side` of a piece at `base_pos`.
    pub fn adjacent_pos(base_pos: Vec2, side: usize) -> Vec2 {
        match side {
            RIGHT => base_pos + Vec2::new(CELL, 0.0),
            LEFT => base_pos - Vec2::new(CELL, 0.0),
            BOTTOM => base_pos + Vec2::new(0.0, CELL),
            TOP => base_pos - Vec2::new(0.0, CELL),
            _ => panic!("invalid side index {side}"),
        }
    }

    /// Raise every member of `group` above all other pieces.
    pub fn bring_group_to_front(&mut self, group: u32) {
        let members = self.group_members(group);
        for &i in &members {
            self.poses[i].z = self.next_z;
            self.next_z += 1;
        }
    }

    /// Indices sorted by ascending z (paint order).
    pub fn paint_order(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.poses.len()).collect();
        order.sort_by_key(|&i| self.poses[i].z);
        order
    }

    /// Topmost piece whose padded rect contains `point` (canvas-local, origin at board pad).
    /// Padding covers outie tab overhang beyond the cell.
    pub fn hit_test(&self, point: Vec2) -> Option<usize> {
        let pad = tab_extent(CELL);
        let mut best: Option<(usize, u32)> = None;
        for (i, pose) in self.poses.iter().enumerate() {
            let min = pose.pos - Vec2::splat(pad);
            let max = pose.pos + Vec2::splat(CELL + pad);
            if point.x >= min.x && point.y >= min.y && point.x <= max.x && point.y <= max.y {
                match best {
                    Some((_, z)) if pose.z <= z => {}
                    _ => best = Some((i, pose.z)),
                }
            }
        }
        best.map(|(i, _)| i)
    }

    /// Bounding-box center of the given pieces.
    pub fn pieces_center(&self, pieces: &[usize]) -> Vec2 {
        assert!(!pieces.is_empty());
        let mut min = self.poses[pieces[0]].pos;
        let mut max = min + Vec2::splat(CELL);
        for &i in &pieces[1..] {
            let p = self.poses[i].pos;
            min = min.min(p);
            max = max.max(p + Vec2::splat(CELL));
        }
        (min + max) * 0.5
    }

    /// Bounding-box center of all pieces in `group`.
    pub fn group_center(&self, group: u32) -> Vec2 {
        self.pieces_center(&self.group_members(group))
    }

    /// Rotate the given pieces 90° CW around `center`; caller updates sides.
    pub fn rotate_poses_cw(&mut self, pieces: &[usize], center: Vec2) {
        let set: HashSet<usize> = pieces.iter().copied().collect();
        for (i, pose) in self.poses.iter_mut().enumerate() {
            if !set.contains(&i) {
                continue;
            }
            // Piece center → relative → CW 90° in screen space (+y down) → new top-left.
            let pc = pose.pos + Vec2::splat(CELL * 0.5);
            let rel = pc - center;
            // Screen CW: (x, y) → (-y, x)
            let rotated = Vec2::new(-rel.y, rel.x);
            let new_center = center + rotated;
            pose.pos = new_center - Vec2::splat(CELL * 0.5);
        }
    }

    /// Rotate every piece in `group` 90° CW around the group center; caller updates sides.
    pub fn rotate_group_poses_cw(&mut self, group: u32) {
        let members = self.group_members(group);
        let center = self.pieces_center(&members);
        self.rotate_poses_cw(&members, center);
    }

    /// Groups whose padded cells overlap the axis-aligned box `min..=max` (play-local).
    pub fn groups_overlapping_aabb(&self, min: Vec2, max: Vec2) -> Vec<u32> {
        let pad = tab_extent(CELL);
        let mut groups = HashSet::new();
        for pose in &self.poses {
            let pmin = pose.pos - Vec2::splat(pad);
            let pmax = pose.pos + Vec2::splat(CELL + pad);
            if pmin.x <= max.x && pmax.x >= min.x && pmin.y <= max.y && pmax.y >= min.y {
                groups.insert(pose.group);
            }
        }
        groups.into_iter().collect()
    }

    /// Split into singleton groups, randomize positions inside `play`, rotate via `puzzle`.
    /// Positions leave room for tab overhang so pieces stay fully visible.
    pub fn scramble(&mut self, puzzle: &mut Puzzle, play: Vec2, rng: &mut impl Rng) {
        let pad = tab_extent(CELL);
        let min = pad;
        let max_x = (play.x - CELL - pad).max(min);
        let max_y = (play.y - CELL - pad).max(min);
        for (i, pose) in self.poses.iter_mut().enumerate() {
            pose.group = i as u32;
            pose.pos = Vec2::new(
                if max_x > min {
                    rng.random_range(min..=max_x)
                } else {
                    min
                },
                if max_y > min {
                    rng.random_range(min..=max_y)
                } else {
                    min
                },
            );
            pose.z = i as u32;

            let turns = rng.random_range(0..4u8);
            let piece = &mut puzzle.pieces[i];
            for _ in 0..turns {
                piece.cycle_sides_cw();
            }
            piece.rotation = (piece.rotation + turns) % 4;
        }
        self.next_z = self.poses.len() as u32;
    }

    /// After dragging `group`, snap to nearest matching neighbor group if close enough.
    /// Returns `Some(other_group)` if a merge happened.
    pub fn try_snap(&mut self, puzzle: &Puzzle, group: u32) -> Option<u32> {
        let members = self.group_members(group);
        let mut best: Option<(f32, Vec2, u32)> = None; // dist, delta, other_group

        for &p in &members {
            let p_pos = self.poses[p].pos;
            let p_sides = puzzle.pieces[p].sides;

            for q in 0..self.poses.len() {
                if self.poses[q].group == group {
                    continue;
                }
                let q_pos = self.poses[q].pos;
                let q_sides = puzzle.pieces[q].sides;
                let other = self.poses[q].group;

                // p RIGHT ↔ q LEFT: p should be immediately left of q
                if sides_match(p_sides[RIGHT], q_sides[LEFT]) {
                    let desired = q_pos - Vec2::new(CELL, 0.0);
                    consider_snap(p_pos, desired, other, &mut best);
                }
                // p LEFT ↔ q RIGHT: p immediately right of q
                if sides_match(p_sides[LEFT], q_sides[RIGHT]) {
                    let desired = q_pos + Vec2::new(CELL, 0.0);
                    consider_snap(p_pos, desired, other, &mut best);
                }
                // p BOTTOM ↔ q TOP: p immediately above q
                if sides_match(p_sides[BOTTOM], q_sides[TOP]) {
                    let desired = q_pos - Vec2::new(0.0, CELL);
                    consider_snap(p_pos, desired, other, &mut best);
                }
                // p TOP ↔ q BOTTOM: p immediately below q
                if sides_match(p_sides[TOP], q_sides[BOTTOM]) {
                    let desired = q_pos + Vec2::new(0.0, CELL);
                    consider_snap(p_pos, desired, other, &mut best);
                }
            }
        }

        let (dist, delta, other) = best?;
        if dist > SNAP_THRESHOLD {
            return None;
        }
        self.move_group(group, delta);
        self.merge_groups(group, other);
        Some(other)
    }
}

pub fn sides_match(a: i128, b: i128) -> bool {
    a != 0 && a == -b
}

fn consider_snap(p_pos: Vec2, desired: Vec2, other: u32, best: &mut Option<(f32, Vec2, u32)>) {
    let delta = desired - p_pos;
    let dist = delta.length();
    match best {
        Some((d, _, _)) if dist >= *d => {}
        _ => *best = Some((dist, delta, other)),
    }
}
