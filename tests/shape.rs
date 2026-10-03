use eframe::egui::{Color32, Pos2, Vec2};
use jigsaw::shape::{CubicBezier, Point2D, generate_edge_beziers, piece_mesh, piece_outline};

const SAMPLES_PER_BEZIER: usize = 8;

fn approx_eq(a: Pos2, b: Pos2) -> bool {
    (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
}

fn approx_pt(a: Point2D, b: Point2D) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

fn dist(a: Point2D, b: Point2D) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}

fn normalize(v: Point2D) -> Point2D {
    let len = dist(Point2D::new(0.0, 0.0), v);
    if len < 1e-12 {
        Point2D::new(1.0, 0.0)
    } else {
        Point2D::new(v.x / len, v.y / len)
    }
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

/// Horizontal span of points whose |y| is near `band` (±slop).
fn width_near_height(pts: &[Point2D], band: f64, slop: f64) -> Option<f64> {
    let xs: Vec<f64> = pts
        .iter()
        .filter(|p| (p.y.abs() - band).abs() <= slop)
        .map(|p| p.x)
        .collect();
    if xs.len() < 2 {
        return None;
    }
    let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some(max_x - min_x)
}

#[test]
fn flat_outline_is_square() {
    let origin = Pos2::new(10.0, 20.0);
    let cell = 100.0;
    let pts = piece_outline(origin, cell, [0; 4]);
    assert!(pts.len() >= 4);
    assert!(approx_eq(pts[0], origin));
    assert!(
        pts.iter()
            .any(|p| approx_eq(*p, origin + Vec2::new(cell, 0.0)))
    );
    assert!(
        pts.iter()
            .any(|p| approx_eq(*p, origin + Vec2::new(cell, cell)))
    );
    assert!(
        pts.iter()
            .any(|p| approx_eq(*p, origin + Vec2::new(0.0, cell)))
    );
    assert!(!approx_eq(*pts.last().unwrap(), pts[0]));
}

#[test]
fn outie_extends_outward_innie_cuts_inward() {
    let origin = Pos2::ZERO;
    let cell = 100.0;
    let outie = piece_outline(origin, cell, [0, 42, 0, 0]);
    let max_x = outie.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    assert!(
        max_x > cell + 8.0,
        "outie tip should protrude past right edge"
    );

    let innie = piece_outline(origin, cell, [0, -42, 0, 0]);
    let min_x_mid = innie
        .iter()
        .filter(|p| p.y > 20.0 && p.y < 80.0)
        .map(|p| p.x)
        .fold(f32::INFINITY, f32::min);
    assert!(
        min_x_mid < cell - 8.0,
        "innie blank tip should cut into the piece, got {min_x_mid}"
    );
}

#[test]
fn complementary_sides_mirror_across_seam() {
    let cell = 100.0_f32;
    let v: i128 = 12345;
    let left = piece_outline(Pos2::ZERO, cell, [0, v, 0, 0]);
    let right = piece_outline(Pos2::new(cell, 0.0), cell, [0, 0, 0, -v]);

    let left_tab: Vec<Pos2> = left.iter().copied().filter(|p| p.x > cell - 1.0).collect();
    assert!(!left_tab.is_empty());

    for lp in &left_tab {
        let matched = right.iter().any(|rp| approx_eq(*rp, *lp));
        assert!(
            matched,
            "left tab point {lp:?} should appear on right innie outline"
        );
    }
}

#[test]
fn complementary_beziers_are_y_reflections() {
    let v: i128 = 987654321;
    let pos = generate_edge_beziers(v);
    let neg = generate_edge_beziers(-v);
    assert_eq!(pos.len(), neg.len());
    for (a, b) in pos.iter().zip(neg.iter()) {
        assert!(approx_pt(a.p0, Point2D::new(b.p0.x, -b.p0.y)));
        assert!(approx_pt(a.p1, Point2D::new(b.p1.x, -b.p1.y)));
        assert!(approx_pt(a.p2, Point2D::new(b.p2.x, -b.p2.y)));
        assert!(approx_pt(a.p3, Point2D::new(b.p3.x, -b.p3.y)));
    }
}

#[test]
fn bezier_chain_is_c1() {
    let chain = generate_edge_beziers(42);
    assert!(chain.len() >= 2);
    for w in chain.windows(2) {
        let a = w[0];
        let b = w[1];
        assert!(approx_pt(a.p3, b.p0), "segments must share endpoints");
        let ta = a.end_tangent();
        let tb = b.start_tangent();
        assert!(
            (ta.x - tb.x).abs() < 1e-9 && (ta.y - tb.y).abs() < 1e-9,
            "tangents must match at join: {ta:?} vs {tb:?}"
        );
    }
}

#[test]
fn tab_neck_narrower_than_head_for_interlock() {
    for code in [1i128, 42, 9999, (u64::MAX as i128) + 99] {
        let pts = sample_beziers(&generate_edge_beziers(code));
        let tip = pts.iter().map(|p| p.y.abs()).fold(0.0_f64, f64::max);
        assert!(tip > 0.05, "expected a tab for code={code}");
        // Neck at the waist; head at the bulb's widest band (not the pointed tip).
        let neck = width_near_height(&pts, tip * 0.45, tip * 0.05);
        let head = width_near_height(&pts, tip * 0.72, tip * 0.06);
        let (Some(neck_w), Some(head_w)) = (neck, head) else {
            panic!("could not measure neck/head widths for code={code}");
        };
        assert!(
            neck_w < head_w,
            "neck ({neck_w}) must be narrower than head ({head_w}) for code={code}"
        );
    }
}

#[test]
fn distinct_magnitudes_produce_varied_tabs() {
    let a = generate_edge_beziers(111);
    let b = generate_edge_beziers(999_999);
    let c = generate_edge_beziers(42);
    assert_ne!(a, b);
    assert_ne!(a, c);
    assert_ne!(b, c);
}

#[test]
fn shape_is_pure_function_of_side_value() {
    let a = generate_edge_beziers(0xDEAD_BEEF_CAFE);
    let b = generate_edge_beziers(0xDEAD_BEEF_CAFE);
    assert_eq!(a, b);
    let lo = generate_edge_beziers(0x11);
    let hi = generate_edge_beziers(0x11i128 | (0xAAi128 << 64));
    assert_ne!(lo, hi, "upper bytes of magnitude must influence shape");
}

#[test]
fn profile_has_no_sharp_kinks() {
    let pts = sample_beziers(&generate_edge_beziers(12345));
    assert!(pts.len() > 10);
    let mut max_turn = 0.0_f64;
    for i in 1..pts.len() - 1 {
        let a = pts[i - 1];
        let b = pts[i];
        let c = pts[i + 1];
        if dist(a, b) < 1e-6 || dist(b, c) < 1e-6 {
            continue;
        }
        let v0 = normalize(Point2D::new(b.x - a.x, b.y - a.y));
        let v1 = normalize(Point2D::new(c.x - b.x, c.y - b.y));
        let dot = (v0.x * v1.x + v0.y * v1.y).clamp(-1.0, 1.0);
        max_turn = max_turn.max(dot.acos());
    }
    assert!(
        max_turn < 1.0,
        "unexpected sharp corner: max turn {max_turn} rad"
    );
}

#[test]
fn shoulders_stay_near_baseline() {
    let pts = sample_beziers(&generate_edge_beziers(7));
    let tip = pts.iter().map(|p| p.y.abs()).fold(0.0_f64, f64::max);
    let tab_xs: Vec<f64> = pts
        .iter()
        .filter(|p| p.y.abs() > tip * 0.30)
        .map(|p| p.x)
        .collect();
    let t0 = tab_xs.iter().copied().fold(f64::INFINITY, f64::min);
    let t1 = tab_xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    // Allow seeded seam waviness (~0.03) plus margin.
    let bow_limit = 0.055;
    for pt in &pts {
        if pt.x < t0 - 0.03 || pt.x > t1 + 0.03 {
            assert!(
                pt.y.abs() <= bow_limit,
                "shoulder at x={} y={} exceeds sway limit {bow_limit}",
                pt.x,
                pt.y
            );
        }
    }
}

#[test]
fn mesh_has_triangles() {
    let mesh = piece_mesh(Pos2::ZERO, 100.0, [0, 1, -2, 0], Color32::WHITE);
    assert!(mesh.indices.len() >= 3);
    assert_eq!(mesh.indices.len() % 3, 0);
    let outline = piece_outline(Pos2::ZERO, 100.0, [0, 1, -2, 0]);
    assert_eq!(mesh.indices.len() / 3, outline.len() - 2);
}
