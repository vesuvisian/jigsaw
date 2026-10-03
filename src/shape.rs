//! Deterministic jigsaw tab outlines from signed `i128` side values.
//!
//! Classical die-cut (D.G. Yu–style) profile: a fixed **4-segment** cubic Bézier
//! template on the unit edge `(0,0) → (1,0)`, seeded by `|side|`, with sign selecting
//! outie (+) / innie (−). Uses circular-arc κ handles for a round bulb crown and
//! continuous baseline sway so interior seams are never flat. Mate pairs `v` / `-v`
//! are exact Y-reflections.

use eframe::egui::{Color32, Mesh, Pos2, Vec2};

use crate::piece::{BOTTOM, LEFT, RIGHT, TOP};

/// Max tab protrusion as a fraction of cell size (outie extent / innie depth).
pub const TAB_HEIGHT_FRAC: f32 = 0.26;

/// Samples per cubic Bézier when rasterizing an outline.
const SAMPLES_PER_BEZIER: usize = 10;

/// Canvas units to pad for tab overhang (hit-test, scramble visibility).
pub fn tab_extent(cell: f32) -> f32 {
    TAB_HEIGHT_FRAC * cell
}

/// Normalized 2D point on the unit edge (`x` along seam, `y` outward).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2D {
    pub x: f64,
    pub y: f64,
}

impl Point2D {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Cubic Bézier on the normalized edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub p0: Point2D,
    pub p1: Point2D,
    pub p2: Point2D,
    pub p3: Point2D,
}

impl CubicBezier {
    pub fn sample(self, t: f64) -> Point2D {
        let u = 1.0 - t;
        let uu = u * u;
        let tt = t * t;
        let a = uu * u;
        let b = 3.0 * uu * t;
        let c = 3.0 * u * tt;
        let d = tt * t;
        Point2D::new(
            a * self.p0.x + b * self.p1.x + c * self.p2.x + d * self.p3.x,
            a * self.p0.y + b * self.p1.y + c * self.p2.y + d * self.p3.y,
        )
    }

    /// End tangent direction `p3 - p2` (parametric).
    pub fn end_tangent(self) -> Point2D {
        Point2D::new(self.p3.x - self.p2.x, self.p3.y - self.p2.y)
    }

    /// Start tangent direction `p1 - p0`.
    pub fn start_tangent(self) -> Point2D {
        Point2D::new(self.p1.x - self.p0.x, self.p1.y - self.p0.y)
    }
}

fn seed_bytes(mag: u128) -> [u8; 16] {
    let mut x = mag.wrapping_mul(0x9E37_79B9_7F4A_7C15_F39C_C060_5CED_C835);
    x ^= x >> 64;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9_94D0_49BB_1331_11EB);
    x ^= x >> 48;
    let mut y = (!mag).wrapping_mul(0xD6E8_FEB8_6659_FD93_C2B2_AE3D_27D4_EB4F);
    y ^= y >> 57;
    y = y.wrapping_mul(0x2127_599B_470D_A13D_8545_8617_7146_4E49);
    y ^= y >> 33;
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&x.to_le_bytes()[..8]);
    out[8..].copy_from_slice(&y.to_le_bytes()[..8]);
    out
}

/// Deterministic factor in \[-1, 1\] from magnitude + salt (byte index).
fn seed_factor(mag: u128, salt: usize) -> f64 {
    let b = seed_bytes(mag);
    (b[salt % 16] as f64 / 255.0) * 2.0 - 1.0
}

/// Force C¹ at interior joints: shared parametric tangent (average of both sides).
fn enforce_c1(segs: &mut [CubicBezier]) {
    for i in 0..segs.len().saturating_sub(1) {
        let j = segs[i].p3;
        debug_assert!((j.x - segs[i + 1].p0.x).abs() < 1e-12);
        debug_assert!((j.y - segs[i + 1].p0.y).abs() < 1e-12);
        let t_end = segs[i].end_tangent();
        let t_start = segs[i + 1].start_tangent();
        let t = Point2D::new((t_end.x + t_start.x) * 0.5, (t_end.y + t_start.y) * 0.5);
        segs[i].p2 = Point2D::new(j.x - t.x, j.y - t.y);
        segs[i + 1].p1 = Point2D::new(j.x + t.x, j.y + t.y);
    }
}

/// Cubic Bézier κ for a unit quarter-circle (≈ 4(√2−1)/3).
const CIRCLE_KAPPA: f64 = 0.5522847498;

/// Unit-edge Béziers for a side value: flat (`0`) or interlocking tab/blank.
///
/// Classical 4-segment template controlled by center, height, neck/head width, and sway.
/// Coordinates: `x ∈ [0, 1]` along the seam, `y` outward (`+` outie, `−` innie).
pub fn generate_edge_beziers(edge_code: i128) -> Vec<CubicBezier> {
    if edge_code == 0 {
        // Flat outer puzzle boundary.
        return vec![CubicBezier {
            p0: Point2D::new(0.0, 0.0),
            p1: Point2D::new(1.0 / 3.0, 0.0),
            p2: Point2D::new(2.0 / 3.0, 0.0),
            p3: Point2D::new(1.0, 0.0),
        }];
    }

    let flip = if edge_code > 0 { 1.0 } else { -1.0 };
    let mag = edge_code.unsigned_abs();
    let f = |s| seed_factor(mag, s);
    let u = |s| (f(s) + 1.0) * 0.5;

    // Geometric parameters from the integer seed (classical ranges).
    let c = (0.50 + 0.08 * f(0)).clamp(0.40, 0.60);
    let h = (0.20 + 0.04 * f(1)).clamp(0.18, TAB_HEIGHT_FRAC as f64) * flip;
    let wn = (0.09 + 0.02 * f(2)).clamp(0.08, 0.12);
    let wh = (wn * (1.8 + 0.4 * u(3))).min(0.28); // always wider than neck
    let sway = 0.03 * f(4) * flip;
    let tilt = 0.015 * f(5);

    let xl_neck = c - wn * 0.5 + tilt;
    let xr_neck = c + wn * 0.5 + tilt;
    let xl_head = c - wh * 0.5;
    let xr_head = c + wh * 0.5;
    let head_radius = (xr_head - xl_head) * 0.5;

    let mut segs = vec![
        // 1. Left corner → left neck waist (continuous curved shoulder)
        CubicBezier {
            p0: Point2D::new(0.0, 0.0),
            p1: Point2D::new(c * 0.35, sway),
            p2: Point2D::new(xl_neck - wn * 0.4, h * 0.10),
            p3: Point2D::new(xl_neck, h * 0.38),
        },
        // 2. Left neck → tip (circular crown using κ)
        CubicBezier {
            p0: Point2D::new(xl_neck, h * 0.38),
            p1: Point2D::new(xl_head - head_radius * 0.3, h * 0.65),
            p2: Point2D::new(c - head_radius * CIRCLE_KAPPA, h),
            p3: Point2D::new(c, h),
        },
        // 3. Tip → right neck (mirror of segment 2)
        CubicBezier {
            p0: Point2D::new(c, h),
            p1: Point2D::new(c + head_radius * CIRCLE_KAPPA, h),
            p2: Point2D::new(xr_head + head_radius * 0.3, h * 0.65),
            p3: Point2D::new(xr_neck, h * 0.38),
        },
        // 4. Right neck → right corner
        CubicBezier {
            p0: Point2D::new(xr_neck, h * 0.38),
            p1: Point2D::new(xr_neck + wn * 0.4, h * 0.10),
            p2: Point2D::new(1.0 - (1.0 - c) * 0.35, sway),
            p3: Point2D::new(1.0, 0.0),
        },
    ];

    // Guarantee C¹ at the three interior joints (collinear shared velocities).
    enforce_c1(&mut segs);
    segs
}

fn sample_beziers(beziers: &[CubicBezier]) -> Vec<Point2D> {
    let mut pts = Vec::with_capacity(beziers.len() * SAMPLES_PER_BEZIER + 1);
    if let Some(first) = beziers.first() {
        pts.push(first.p0);
    }
    for bez in beziers {
        for i in 1..=SAMPLES_PER_BEZIER {
            let t = i as f64 / SAMPLES_PER_BEZIER as f64;
            pts.push(bez.sample(t));
        }
    }
    pts
}

fn map_tn(seam_origin: Pos2, along: Vec2, outward: Vec2, cell: f32, t: f64, n: f64) -> Pos2 {
    seam_origin + along * (t as f32 * cell) + outward * (n as f32 * cell)
}

fn edge_samples(
    seam_origin: Pos2,
    along: Vec2,
    outward: Vec2,
    cell: f32,
    side: i128,
) -> Vec<Pos2> {
    sample_beziers(&generate_edge_beziers(side))
        .into_iter()
        .map(|p| map_tn(seam_origin, along, outward, cell, p.x, p.y))
        .collect()
}

/// Closed piece outline in clockwise order (first point not repeated at end).
///
/// - `side > 0` — outie (protrudes along the outward normal)
/// - `side < 0` — innie (socket cut inward)
/// - `side == 0` — flat
pub fn piece_outline(origin: Pos2, cell: f32, sides: [i128; 4]) -> Vec<Pos2> {
    let tl = origin;
    let tr = origin + Vec2::new(cell, 0.0);
    let bl = origin + Vec2::new(0.0, cell);

    // Seam space is always left→right / top→bottom; reverse when walking CW on bottom/left.
    let seams = [
        (tl, Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0), false, sides[TOP]),
        (tr, Vec2::new(0.0, 1.0), Vec2::new(1.0, 0.0), false, sides[RIGHT]),
        (bl, Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0), true, sides[BOTTOM]),
        (tl, Vec2::new(0.0, 1.0), Vec2::new(-1.0, 0.0), true, sides[LEFT]),
    ];

    let mut pts = Vec::new();
    for (seam_origin, along, outward, reverse, side) in seams {
        let mut samples = edge_samples(seam_origin, along, outward, cell, side);
        if reverse {
            samples.reverse();
        }
        if pts.is_empty() {
            pts = samples;
        } else {
            pts.extend(samples.into_iter().skip(1));
        }
    }

    if pts.len() >= 2 {
        let first = pts[0];
        let last = *pts.last().unwrap();
        if (first.x - last.x).abs() < 1e-3 && (first.y - last.y).abs() < 1e-3 {
            pts.pop();
        }
    }
    pts
}

/// Triangulate a simple polygon into a filled mesh (Mapbox earcut).
pub fn triangulate_polygon(points: &[Pos2], fill: Color32) -> Mesh {
    let mut mesh = Mesh::default();
    let n = points.len();
    if n < 3 {
        return mesh;
    }

    for &p in points {
        mesh.colored_vertex(p, fill);
    }

    let mut flat = Vec::with_capacity(n * 2);
    for p in points {
        flat.push(p.x as f64);
        flat.push(p.y as f64);
    }

    let Ok(indices) = earcutr::earcut(&flat, &[], 2) else {
        return mesh;
    };
    for tri in indices.chunks_exact(3) {
        mesh.add_triangle(tri[0] as u32, tri[1] as u32, tri[2] as u32);
    }
    mesh
}

/// Filled mesh from an existing outline (avoids recomputing the spline).
pub fn fill_mesh(outline: &[Pos2], fill: Color32) -> Mesh {
    triangulate_polygon(outline, fill)
}

/// Filled mesh for a piece outline.
pub fn piece_mesh(origin: Pos2, cell: f32, sides: [i128; 4], fill: Color32) -> Mesh {
    fill_mesh(&piece_outline(origin, cell, sides), fill)
}
