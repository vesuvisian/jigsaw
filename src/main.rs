#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashSet;
use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;

use eframe::egui::{self, Align2, Color32, FontId, Galley, Pos2, Rect, Sense, Stroke, Vec2};
use jigsaw::board::{Board, CELL};
use jigsaw::export;
use jigsaw::generate::generate;
use jigsaw::piece::{BOTTOM, LEFT, Puzzle, RIGHT, TOP};
use jigsaw::shape::piece_outline;
use jigsaw::solve::{self, SolveStepResult};
use rand::Rng;

const PAD: f32 = 12.0;
const DEFAULT_ROWS: usize = 5;
const DEFAULT_COLS: usize = 7;
/// Top controls panel height (native window sizing).
#[cfg(not(target_arch = "wasm32"))]
const CONTROLS_HEIGHT: f32 = 48.0;
/// Panel frame / scrollbar gutter so a default board needs no horizontal scroll.
#[cfg(not(target_arch = "wasm32"))]
const WINDOW_CHROME_X: f32 = 32.0;
const NUDGE: f32 = 8.0;
/// Ignore pointer jitter below this so a tap/long-press is not treated as a drag.
const DRAG_THRESHOLD: f32 = 12.0;
const LONG_PRESS_SECS: f64 = 0.45;
const DOUBLE_TAP_SECS: f64 = 0.4;
const CELEBRATION_SECS: f64 = 1.45;
const CONFETTI_COUNT: usize = 42;
/// Canvas element id in `index.html` (web builds).
#[cfg(target_arch = "wasm32")]
const CANVAS_ID: &str = "jigsaw_canvas";

fn play_content_size(rows: usize, cols: usize) -> Vec2 {
    let play = Board::play_size(rows, cols);
    Vec2::new(play.x + PAD * 2.0, play.y + PAD * 2.0)
}

/// Dock / taskbar icon: Lucide puzzle, inset to match other macOS Dock tiles.
#[cfg(not(target_arch = "wasm32"))]
fn app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).expect("app icon PNG")
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let content = play_content_size(DEFAULT_ROWS, DEFAULT_COLS);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([content.x + WINDOW_CHROME_X, content.y + CONTROLS_HEIGHT])
            .with_title("Jigsaw Puzzle Generator")
            .with_icon(app_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Jigsaw Puzzle Generator",
        options,
        Box::new(|_cc| Ok(Box::new(JigsawApp::new()))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id(CANVAS_ID)
            .unwrap_or_else(|| panic!("Failed to find #{CANVAS_ID}"))
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("canvas element was not an HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|_cc| Ok(Box::new(JigsawApp::new()))),
            )
            .await;

        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(()) => {
                    loading_text.remove();
                }
                Err(err) => {
                    loading_text.set_inner_html(
                        "<p>The app failed to start. See the developer console for details.</p>",
                    );
                    panic!("Failed to start eframe: {err:?}");
                }
            }
        }
    });
}

struct DragState {
    /// Piece that was grabbed (group moves with it).
    piece: usize,
    last_pointer: Pos2,
    start_pointer: Pos2,
    start_time: f64,
    /// True once the pointer moved enough to count as a drag.
    moved: bool,
    /// True if this press already rotated via long-press.
    long_press_rotated: bool,
}

/// Rubber-band selection in play-local coordinates.
struct Marquee {
    start: Vec2,
    current: Vec2,
}

struct Confetti {
    pos0: Vec2,
    vel: Vec2,
    color: Color32,
    size: Vec2,
    rot0: f32,
    spin: f32,
}

struct Celebration {
    start: f64,
    center: Vec2,
    bits: Vec<Confetti>,
}

impl Celebration {
    fn spawn(start: f64, center: Vec2, rng: &mut impl Rng) -> Self {
        const COLORS: [Color32; 6] = [
            Color32::from_rgb(30, 100, 200),
            Color32::from_rgb(255, 196, 64),
            Color32::from_rgb(72, 176, 118),
            Color32::from_rgb(230, 86, 86),
            Color32::from_rgb(168, 118, 214),
            Color32::from_rgb(255, 255, 255),
        ];
        let bits = (0..CONFETTI_COUNT)
            .map(|_| {
                let ang = rng.random_range(0.0..TAU);
                let speed = rng.random_range(90.0..260.0);
                Confetti {
                    pos0: center
                        + Vec2::new(rng.random_range(-18.0..18.0), rng.random_range(-18.0..18.0)),
                    vel: Vec2::new(ang.cos() * speed, ang.sin() * speed - 90.0),
                    color: COLORS[rng.random_range(0..COLORS.len())],
                    size: Vec2::new(rng.random_range(4.5..8.5), rng.random_range(7.0..13.0)),
                    rot0: rng.random_range(0.0..TAU),
                    spin: rng.random_range(-9.0..9.0),
                }
            })
            .collect();
        Self {
            start,
            center,
            bits,
        }
    }
}

struct JigsawApp {
    rows: usize,
    cols: usize,
    puzzle: Puzzle,
    board: Board,
    /// Selected piece indices (highlight follows each piece's group).
    selected: HashSet<usize>,
    drag: Option<DragState>,
    marquee: Option<Marquee>,
    /// Last tap that did not drag, for double-tap rotate (`group`, time).
    last_tap: Option<(u32, f64)>,
    /// Inner play rect size (excludes pad); at least `Board::play_size`, grows with the viewport.
    play_area: Vec2,
    include_dimensions_in_export: bool,
    /// Export options window (opened by Export…).
    export_dialog_open: bool,
    /// Draw piece UUID stubs and side-value edge labels.
    show_labels: bool,
    status: String,
    /// True after the board has been in more than one group (so Generate does not count as a solve).
    incomplete: bool,
    celebration: Option<Celebration>,
}

impl JigsawApp {
    fn new() -> Self {
        let rows = DEFAULT_ROWS;
        let cols = DEFAULT_COLS;
        Self {
            rows,
            cols,
            puzzle: generate(rows, cols),
            board: Board::assembled(rows, cols),
            selected: HashSet::new(),
            drag: None,
            marquee: None,
            last_tap: None,
            play_area: Board::play_size(rows, cols),
            include_dimensions_in_export: false,
            export_dialog_open: false,
            show_labels: false,
            status: String::new(),
            incomplete: false,
            celebration: None,
        }
    }

    fn regenerate(&mut self) {
        self.rows = self.rows.max(1);
        self.cols = self.cols.max(1);
        self.puzzle = generate(self.rows, self.cols);
        self.board = Board::assembled(self.rows, self.cols);
        self.selected.clear();
        self.drag = None;
        self.marquee = None;
        self.last_tap = None;
        self.incomplete = false;
        self.celebration = None;
        // Keep current play_area if larger; grow if the new puzzle needs more room.
        self.play_area = self.play_area.max(Board::play_size(self.rows, self.cols));
        self.status.clear();
    }

    fn scramble(&mut self) {
        let play = self
            .play_area
            .max(Board::play_size(self.puzzle.rows, self.puzzle.cols));
        self.board
            .scramble(&mut self.puzzle, play, &mut rand::rng());
        self.selected.clear();
        self.drag = None;
        self.marquee = None;
        self.last_tap = None;
        self.celebration = None;
        self.status = format!("Scrambled {} pieces", self.puzzle.pieces.len());
    }

    fn unique_group_count(&self) -> usize {
        self.board
            .poses
            .iter()
            .map(|p| p.group)
            .collect::<HashSet<_>>()
            .len()
    }

    fn start_celebration(&mut self, time: f64) {
        let center = self
            .board
            .poses
            .first()
            .map(|pose| self.board.group_center(pose.group))
            .unwrap_or(Vec2::ZERO);
        self.celebration = Some(Celebration::spawn(time, center, &mut rand::rng()));
        self.status = "Puzzle complete".to_string();
    }

    /// Celebrate only on the transition from multiple groups down to one.
    fn tick_solve_state(&mut self, time: f64) {
        if self.unique_group_count() > 1 {
            self.incomplete = true;
        } else if self.incomplete {
            self.incomplete = false;
            self.start_celebration(time);
        }
    }

    fn solve_step(&mut self) {
        match solve::solve_step(&mut self.puzzle, &mut self.board) {
            SolveStepResult::Connected {
                piece_a,
                piece_b,
                group_size,
                ..
            } => {
                self.select_piece_group(piece_a);
                let id_a = &self.puzzle.pieces[piece_a].id.to_string()[..8];
                let id_b = &self.puzzle.pieces[piece_b].id.to_string()[..8];
                self.status =
                    format!("Connected {id_a} ↔ {id_b} — group now has {group_size} pieces");
            }
            SolveStepResult::Complete => {
                let groups: HashSet<_> = self.board.poses.iter().map(|p| p.group).collect();
                if groups.len() <= 1 {
                    self.status = "Puzzle complete".to_string();
                } else {
                    self.status = "No more connections".to_string();
                }
            }
        }
    }

    fn selected_groups(&self) -> HashSet<u32> {
        self.selected
            .iter()
            .map(|&i| self.board.group_of(i))
            .collect()
    }

    fn select_groups(&mut self, groups: &[u32]) {
        self.selected.clear();
        self.last_tap = None;
        for &group in groups {
            for i in self.board.group_members(group) {
                self.selected.insert(i);
            }
            self.board.bring_group_to_front(group);
        }
        let n_groups = groups.len();
        let n_pieces = self.selected.len();
        self.status = if n_groups == 1 {
            format!("Selected 1 group ({n_pieces} pieces)")
        } else {
            format!("Selected {n_groups} groups ({n_pieces} pieces)")
        };
    }

    fn select_piece_group(&mut self, piece: usize) {
        let group = self.board.group_of(piece);
        self.selected.clear();
        for i in self.board.group_members(group) {
            self.selected.insert(i);
        }
        self.board.bring_group_to_front(group);
    }

    fn move_selected(&mut self, delta: Vec2) {
        for group in self.selected_groups() {
            self.board.move_group(group, delta);
        }
    }

    fn try_snap_selected(&mut self) -> bool {
        let mut snapped = false;
        for _ in 0..self.board.poses.len() {
            let mut any = false;
            for group in self.selected_groups() {
                if self.board.try_snap(&self.puzzle, group).is_some() {
                    any = true;
                    snapped = true;
                }
            }
            if !any {
                break;
            }
        }
        snapped
    }

    fn nudge_selected(&mut self, delta: Vec2) {
        if self.selected.is_empty() {
            return;
        }
        self.move_selected(delta);
        if self.try_snap_selected() {
            self.status = "Snapped selection".to_string();
        }
    }

    fn rotate_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.last_tap = None;
        let pieces: Vec<usize> = self.selected.iter().copied().collect();
        let center = self.board.pieces_center(&pieces);
        self.board.rotate_poses_cw(&pieces, center);
        for &i in &pieces {
            self.puzzle.pieces[i].rotate_cw();
        }
        for group in self.selected_groups() {
            self.board.bring_group_to_front(group);
        }
        let many = self.selected_groups().len() > 1;
        if self.try_snap_selected() {
            self.status = if many {
                "Rotated selection and snapped".to_string()
            } else {
                "Rotated and snapped".to_string()
            };
        } else {
            self.status = if many {
                "Rotated selection CW".to_string()
            } else {
                "Rotated group CW".to_string()
            };
        }
    }

    fn finish_marquee(&mut self) {
        let Some(marquee) = self.marquee.take() else {
            return;
        };
        if (marquee.current - marquee.start).length() <= DRAG_THRESHOLD {
            self.clear_selection();
            return;
        }
        let min = marquee.start.min(marquee.current);
        let max = marquee.start.max(marquee.current);
        let groups = self.board.groups_overlapping_aabb(min, max);
        if groups.is_empty() {
            self.clear_selection();
            return;
        }
        self.select_groups(&groups);
    }

    /// Select on tap; rotate if this is a second tap on the same group.
    fn tap_piece(&mut self, piece: usize, time: f64) {
        let group = self.board.group_of(piece);
        if let Some((prev_group, prev_time)) = self.last_tap
            && prev_group == group
            && time - prev_time <= DOUBLE_TAP_SECS
        {
            self.select_piece_group(piece);
            self.rotate_selected();
            return;
        }

        self.select_piece_group(piece);
        self.last_tap = Some((group, time));
        self.status = format!(
            "Selected piece {}",
            &self.puzzle.pieces[piece].id.to_string()[..8]
        );
    }

    fn clear_selection(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.selected.clear();
        self.last_tap = None;
        self.status = "Selection cleared".to_string();
    }

    fn show_export_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.export_dialog_open;
        let mut save = false;
        let mut cancel = false;

        egui::Window::new("Export pieces for solver")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("Save the current pieces as JSON for a solver.");
                ui.add_space(8.0);
                ui.checkbox(&mut self.include_dimensions_in_export, "Include rows/cols");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save…").clicked() {
                        save = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }

        if save {
            self.export_dialog_open = false;
            self.export_for_solver();
        } else if cancel || !open {
            self.export_dialog_open = false;
            self.status = "Export cancelled".to_string();
        }
    }

    fn export_for_solver(&mut self) {
        let export = export::from_puzzle(&self.puzzle, self.include_dimensions_in_export);
        let json = match export::to_json(&export) {
            Ok(json) => json,
            Err(err) => {
                self.status = format!("Failed to serialize export: {err}");
                return;
            }
        };

        let default_name = format!("jigsaw_{}x{}.json", self.puzzle.rows, self.puzzle.cols);
        let piece_count = export.pieces.len();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = rfd::FileDialog::new()
                .set_title("Export pieces for solver")
                .set_file_name(&default_name)
                .add_filter("JSON", &["json"])
                .save_file();

            match path {
                Some(path) => match std::fs::write(&path, json) {
                    Ok(()) => {
                        self.status =
                            format!("Exported {} pieces to {}", piece_count, path.display());
                    }
                    Err(err) => {
                        self.status = format!("Failed to write {}: {err}", path.display());
                    }
                },
                None => {
                    self.status = "Export cancelled".to_string();
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            // Browser: AsyncFileDialog triggers a download (no save path).
            self.status = format!("Downloading {default_name} ({piece_count} pieces)…");
            wasm_bindgen_futures::spawn_local(async move {
                let Some(file) = rfd::AsyncFileDialog::new()
                    .set_file_name(&default_name)
                    .add_filter("JSON", &["json"])
                    .save_file()
                    .await
                else {
                    return;
                };
                let _ = file.write(json.as_bytes()).await;
            });
        }
    }
}

impl eframe::App for JigsawApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.export_dialog_open {
            self.show_export_dialog(ctx);
        }

        if !self.selected.is_empty()
            && self.drag.is_none()
            && self.marquee.is_none()
            && !self.export_dialog_open
        {
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                self.nudge_selected(Vec2::new(-NUDGE, 0.0));
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                self.nudge_selected(Vec2::new(NUDGE, 0.0));
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                self.nudge_selected(Vec2::new(0.0, -NUDGE));
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                self.nudge_selected(Vec2::new(0.0, NUDGE));
            }
            if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                self.rotate_selected();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.clear_selection();
            }
        }

        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Rows (M):");
                ui.add(egui::DragValue::new(&mut self.rows).range(1..=64));
                ui.label("Cols (N):");
                ui.add(egui::DragValue::new(&mut self.cols).range(1..=64));
                if ui.button("Generate").clicked() {
                    self.regenerate();
                }
                if ui.button("Scramble").clicked() {
                    self.scramble();
                }
                if ui
                    .add_enabled(!self.selected.is_empty(), egui::Button::new("Rotate"))
                    .clicked()
                {
                    self.rotate_selected();
                }
                if ui.button("Solve Step").clicked() {
                    self.solve_step();
                }
                if ui.button("Export…").clicked() {
                    self.export_dialog_open = true;
                }
                ui.checkbox(&mut self.show_labels, "Show labels");
                ui.separator();
                ui.label(format!(
                    "{}×{} — {} pieces",
                    self.puzzle.rows,
                    self.puzzle.cols,
                    self.puzzle.pieces.len()
                ));
            });
            if !self.status.is_empty() {
                ui.label(&self.status);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let selected_groups = self.selected_groups();
                    let (event, play_area, origin) = draw_board(
                        ui,
                        &self.puzzle,
                        &self.board,
                        &selected_groups,
                        self.marquee.as_ref(),
                        self.show_labels,
                    );
                    self.play_area = play_area;
                    let now = ui.input(|i| i.time);
                    match event {
                        BoardEvent::None => {}
                        BoardEvent::Select(piece) => {
                            self.tap_piece(piece, now);
                        }
                        BoardEvent::StartDrag { piece, pointer } => {
                            if !self.selected.contains(&piece) {
                                self.select_piece_group(piece);
                            }
                            for group in self.selected_groups() {
                                self.board.bring_group_to_front(group);
                            }
                            self.drag = Some(DragState {
                                piece,
                                last_pointer: pointer,
                                start_pointer: pointer,
                                start_time: now,
                                moved: false,
                                long_press_rotated: false,
                            });
                        }
                        BoardEvent::StartMarquee { pointer } => {
                            let local = pointer - origin;
                            self.marquee = Some(Marquee {
                                start: local,
                                current: local,
                            });
                        }
                        BoardEvent::DragTo(pointer) => {
                            if let Some(marquee) = &mut self.marquee {
                                marquee.current = pointer - origin;
                            } else {
                                let mut move_delta = None;
                                if let Some(drag) = &mut self.drag {
                                    let delta = pointer - drag.last_pointer;
                                    if !drag.moved {
                                        if (pointer - drag.start_pointer).length() > DRAG_THRESHOLD
                                        {
                                            drag.moved = true;
                                            drag.last_pointer = pointer;
                                            move_delta = Some(delta);
                                        }
                                    } else if delta.length_sq() > 0.0 {
                                        drag.last_pointer = pointer;
                                        move_delta = Some(delta);
                                    }
                                }
                                if let Some(delta) = move_delta {
                                    self.move_selected(delta);
                                }
                            }
                        }
                        BoardEvent::EndDrag => {
                            if self.marquee.is_some() {
                                self.finish_marquee();
                            } else if let Some(drag) = self.drag.take() {
                                if drag.moved {
                                    if self.try_snap_selected() {
                                        self.status = "Snapped selection".to_string();
                                    }
                                } else if !drag.long_press_rotated {
                                    self.tap_piece(drag.piece, now);
                                }
                            } else {
                                self.clear_selection();
                            }
                        }
                        BoardEvent::ClearSelection => {
                            self.clear_selection();
                        }
                        BoardEvent::Rotate(piece) => {
                            if !self.selected.contains(&piece) {
                                self.select_piece_group(piece);
                            }
                            self.rotate_selected();
                        }
                    }

                    if let Some(drag) = &self.drag
                        && !drag.moved
                        && !drag.long_press_rotated
                    {
                        let elapsed = now - drag.start_time;
                        if elapsed >= LONG_PRESS_SECS {
                            if let Some(drag) = &mut self.drag {
                                drag.long_press_rotated = true;
                            }
                            self.rotate_selected();
                        } else {
                            ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                                (LONG_PRESS_SECS - elapsed).min(0.05),
                            ));
                        }
                    }

                    self.tick_solve_state(now);
                    if self.marquee.is_some() {
                        ctx.request_repaint();
                    }
                    let celebration_done = self
                        .celebration
                        .as_ref()
                        .is_some_and(|c| now - c.start >= CELEBRATION_SECS);
                    if celebration_done {
                        self.celebration = None;
                    } else if let Some(celebration) = &self.celebration {
                        paint_celebration(ui.painter(), origin, celebration, now);
                        ctx.request_repaint();
                    }
                });
        });
    }
}

fn format_side(v: i128) -> String {
    v.to_string()
}

/// Layout `text` so its measured width is ≤ `max_width`, truncating with an ellipsis if needed.
fn layout_truncated(
    painter: &egui::Painter,
    text: &str,
    font_id: FontId,
    color: Color32,
    max_width: f32,
) -> Arc<Galley> {
    let full = painter.layout_no_wrap(text.to_owned(), font_id.clone(), color);
    if full.size().x <= max_width {
        return full;
    }

    const ELLIPSIS: &str = "…";
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    let mut best = ELLIPSIS.to_string();

    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let candidate: String = chars[..mid].iter().copied().collect::<String>() + ELLIPSIS;
        let galley = painter.layout_no_wrap(candidate.clone(), font_id.clone(), color);
        if galley.size().x <= max_width {
            best = candidate;
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    painter.layout_no_wrap(best, font_id, color)
}

#[derive(Clone, Copy)]
enum Edge {
    Top,
    Right,
    Bottom,
    Left,
}

/// Paint truncated side text flush to `edge`, reading along that edge.
/// Vertical edges use −90° (bottom → top). `TextShape` pivots at galley top-left.
#[allow(clippy::too_many_arguments)]
fn paint_edge_label(
    painter: &egui::Painter,
    rect: Rect,
    edge: Edge,
    text: &str,
    font_id: FontId,
    color: Color32,
    max_width: f32,
    inset: f32,
) {
    let galley = layout_truncated(painter, text, font_id, color, max_width);
    let size = galley.size();
    let angle = match edge {
        Edge::Top | Edge::Bottom => 0.0,
        Edge::Left | Edge::Right => -FRAC_PI_2,
    };

    let pos = match edge {
        Edge::Top => Pos2::new(rect.center().x - size.x * 0.5, rect.top() + inset),
        Edge::Bottom => Pos2::new(
            rect.center().x - size.x * 0.5,
            rect.bottom() - inset - size.y,
        ),
        Edge::Left => Pos2::new(rect.left() + inset, rect.center().y + size.x * 0.5),
        Edge::Right => Pos2::new(
            rect.right() - inset - size.y,
            rect.center().y + size.x * 0.5,
        ),
    };

    painter.add(egui::epaint::TextShape::new(pos, galley, color).with_angle(angle));
}

enum BoardEvent {
    None,
    Select(usize),
    StartDrag { piece: usize, pointer: Pos2 },
    StartMarquee { pointer: Pos2 },
    DragTo(Pos2),
    EndDrag,
    ClearSelection,
    Rotate(usize),
}

fn with_alpha(color: Color32, alpha: f32) -> Color32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), a)
}

fn rotated_rect(center: Pos2, size: Vec2, angle: f32) -> Vec<Pos2> {
    let (sin, cos) = angle.sin_cos();
    let hw = size.x * 0.5;
    let hh = size.y * 0.5;
    let rot =
        |x: f32, y: f32| Pos2::new(center.x + cos * x - sin * y, center.y + sin * x + cos * y);
    vec![rot(-hw, -hh), rot(hw, -hh), rot(hw, hh), rot(-hw, hh)]
}

fn paint_celebration(painter: &egui::Painter, origin: Pos2, celebration: &Celebration, now: f64) {
    let t = (now - celebration.start) as f32;
    if t < 0.0 {
        return;
    }
    let duration = CELEBRATION_SECS as f32;
    let fade = if t > duration * 0.55 {
        (1.0 - (t - duration * 0.55) / (duration * 0.45)).clamp(0.0, 1.0)
    } else {
        1.0
    };

    for bit in &celebration.bits {
        let pos = origin + bit.pos0 + bit.vel * t + Vec2::new(0.0, 520.0) * t * t * 0.5;
        let angle = bit.rot0 + bit.spin * t;
        painter.add(egui::epaint::PathShape::convex_polygon(
            rotated_rect(pos, bit.size, angle),
            with_alpha(bit.color, fade),
            Stroke::NONE,
        ));
    }

    let pop = 1.0 - (-t * 10.0).exp();
    let text_alpha = fade * pop;
    if text_alpha > 0.02 {
        painter.text(
            origin + celebration.center,
            Align2::CENTER_CENTER,
            "Solved!",
            FontId::proportional(26.0 + 6.0 * pop),
            with_alpha(Color32::from_rgb(25, 55, 110), text_alpha),
        );
    }
}

fn draw_board(
    ui: &mut egui::Ui,
    puzzle: &Puzzle,
    board: &Board,
    selected_groups: &HashSet<u32>,
    marquee: Option<&Marquee>,
    show_labels: bool,
) -> (BoardEvent, Vec2, Pos2) {
    let edge_inset = 18.0_f32;
    let edge_margin = 16.0_f32;

    // Fill the visible panel; never smaller than the scramble minimum for this puzzle.
    let min_size = play_content_size(puzzle.rows, puzzle.cols);
    let desired = ui.available_size().max(min_size);
    let (response, painter) = ui.allocate_painter(desired, Sense::click_and_drag());
    let origin = response.rect.min + Vec2::splat(PAD);
    let play_area = (response.rect.size() - Vec2::splat(PAD * 2.0)).max(Vec2::ZERO);
    let max_edge_len = (CELL - edge_margin * 2.0).max(12.0);

    let mut highlight = selected_groups.clone();
    if let Some(marquee) = marquee {
        let min = marquee.start.min(marquee.current);
        let max = marquee.start.max(marquee.current);
        highlight.extend(board.groups_overlapping_aabb(min, max));
    }

    let mut event = BoardEvent::None;

    if response.drag_started()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let local = pointer - origin;
        if let Some(piece) = board.hit_test(local) {
            event = BoardEvent::StartDrag { piece, pointer };
        } else {
            event = BoardEvent::StartMarquee { pointer };
        }
    } else if response.dragged() {
        if let Some(pointer) = response.interact_pointer_pos() {
            event = BoardEvent::DragTo(pointer);
        }
    } else if response.drag_stopped() {
        event = BoardEvent::EndDrag;
    } else if (response.double_clicked() || response.secondary_clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let local = pointer - origin;
        if let Some(piece) = board.hit_test(local) {
            event = BoardEvent::Rotate(piece);
        }
    } else if response.clicked() {
        event = match response
            .interact_pointer_pos()
            .and_then(|pointer| board.hit_test(pointer - origin))
        {
            Some(piece) => BoardEvent::Select(piece),
            None => BoardEvent::ClearSelection,
        };
    }

    for &i in &board.paint_order() {
        let pose = &board.poses[i];
        let piece = &puzzle.pieces[i];
        let min = origin + pose.pos;
        let rect = Rect::from_min_size(min, Vec2::splat(CELL));
        let is_selected = highlight.contains(&pose.group);

        let fill = if is_selected {
            Color32::from_rgb(210, 230, 255)
        } else {
            Color32::from_rgb(245, 242, 235)
        };
        let stroke_color = if is_selected {
            Color32::from_rgb(30, 100, 200)
        } else {
            Color32::from_rgb(60, 60, 60)
        };
        let stroke_width = if is_selected { 3.0_f32 } else { 1.5_f32 };

        let outline = piece_outline(min, CELL, piece.sides);
        painter.add(jigsaw::shape::fill_mesh(&outline, fill));
        painter.add(egui::epaint::PathShape::closed_line(
            outline,
            Stroke::new(stroke_width, stroke_color),
        ));

        if show_labels {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                piece.id.to_string()[..8].to_owned(),
                FontId::proportional(14.0),
                Color32::from_rgb(30, 30, 30),
            );

            let edge_color = Color32::from_rgb(40, 90, 140);
            let edge_font = FontId::proportional(11.0);

            paint_edge_label(
                &painter,
                rect,
                Edge::Top,
                &format_side(piece.sides[TOP]),
                edge_font.clone(),
                edge_color,
                max_edge_len,
                edge_inset,
            );
            paint_edge_label(
                &painter,
                rect,
                Edge::Right,
                &format_side(piece.sides[RIGHT]),
                edge_font.clone(),
                edge_color,
                max_edge_len,
                edge_inset,
            );
            paint_edge_label(
                &painter,
                rect,
                Edge::Bottom,
                &format_side(piece.sides[BOTTOM]),
                edge_font.clone(),
                edge_color,
                max_edge_len,
                edge_inset,
            );
            paint_edge_label(
                &painter,
                rect,
                Edge::Left,
                &format_side(piece.sides[LEFT]),
                edge_font,
                edge_color,
                max_edge_len,
                edge_inset,
            );
        }
    }

    if let Some(marquee) = marquee {
        let rect = Rect::from_two_pos(origin + marquee.start, origin + marquee.current);
        painter.rect(
            rect,
            0.0,
            Color32::from_rgba_unmultiplied(30, 100, 200, 50),
            Stroke::new(1.5_f32, Color32::from_rgb(30, 100, 200)),
            egui::StrokeKind::Inside,
        );
    }

    (event, play_area, origin)
}
