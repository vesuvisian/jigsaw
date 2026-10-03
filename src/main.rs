#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use eframe::egui::{self, Align2, Color32, FontId, Galley, Pos2, Rect, Sense, Stroke, Vec2};
use jigsaw::board::{Board, CELL};
use jigsaw::export;
use jigsaw::generate::generate;
use jigsaw::piece::{Puzzle, BOTTOM, LEFT, RIGHT, TOP};
use jigsaw::shape::piece_outline;
use jigsaw::solve::{self, SolveStepResult};

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
/// Canvas element id in `index.html` (web builds).
#[cfg(target_arch = "wasm32")]
const CANVAS_ID: &str = "jigsaw_canvas";

fn play_content_size(rows: usize, cols: usize) -> Vec2 {
    let play = Board::play_size(rows, cols);
    Vec2::new(play.x + PAD * 2.0, play.y + PAD * 2.0)
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let content = play_content_size(DEFAULT_ROWS, DEFAULT_COLS);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([content.x + WINDOW_CHROME_X, content.y + CONTROLS_HEIGHT])
            .with_title("Jigsaw Puzzle Generator"),
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
    /// True once the pointer moved enough to count as a drag.
    moved: bool,
}

struct JigsawApp {
    rows: usize,
    cols: usize,
    puzzle: Puzzle,
    board: Board,
    /// Selected piece index (group highlight follows its group).
    selected: Option<usize>,
    drag: Option<DragState>,
    /// Inner play rect size (excludes pad); at least `Board::play_size`, grows with the viewport.
    play_area: Vec2,
    include_dimensions_in_export: bool,
    /// Export options window (opened by Export…).
    export_dialog_open: bool,
    /// Draw piece UUID stubs and side-value edge labels.
    show_labels: bool,
    status: String,
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
            selected: None,
            drag: None,
            play_area: Board::play_size(rows, cols),
            include_dimensions_in_export: false,
            export_dialog_open: false,
            show_labels: false,
            status: String::new(),
        }
    }

    fn regenerate(&mut self) {
        self.rows = self.rows.max(1);
        self.cols = self.cols.max(1);
        self.puzzle = generate(self.rows, self.cols);
        self.board = Board::assembled(self.rows, self.cols);
        self.selected = None;
        self.drag = None;
        // Keep current play_area if larger; grow if the new puzzle needs more room.
        self.play_area = self
            .play_area
            .max(Board::play_size(self.rows, self.cols));
        self.status.clear();
    }

    fn scramble(&mut self) {
        let play = self
            .play_area
            .max(Board::play_size(self.puzzle.rows, self.puzzle.cols));
        self.board
            .scramble(&mut self.puzzle, play, &mut rand::rng());
        self.selected = None;
        self.drag = None;
        self.status = format!("Scrambled {} pieces", self.puzzle.pieces.len());
    }

    fn solve_step(&mut self) {
        match solve::solve_step(&mut self.puzzle, &mut self.board) {
            SolveStepResult::Connected {
                piece_a,
                piece_b,
                group_size,
                ..
            } => {
                self.selected = Some(piece_a);
                let id_a = &self.puzzle.pieces[piece_a].id.to_string()[..8];
                let id_b = &self.puzzle.pieces[piece_b].id.to_string()[..8];
                self.status = format!(
                    "Connected {id_a} ↔ {id_b} — group now has {group_size} pieces"
                );
            }
            SolveStepResult::Complete => {
                let groups: std::collections::HashSet<_> =
                    self.board.poses.iter().map(|p| p.group).collect();
                if groups.len() <= 1 {
                    self.status = "Puzzle complete".to_string();
                } else {
                    self.status = "No more connections".to_string();
                }
            }
        }
    }

    fn selected_group(&self) -> Option<u32> {
        self.selected.map(|i| self.board.group_of(i))
    }

    fn nudge_selected(&mut self, delta: Vec2) {
        let Some(group) = self.selected_group() else {
            return;
        };
        self.board.move_group(group, delta);
        if let Some(merged) = self.board.try_snap(&self.puzzle, group) {
            self.status = format!("Snapped into group {merged}");
        }
    }

    fn rotate_group(&mut self, group: u32) {
        self.board.rotate_group_poses_cw(group);
        for i in self.board.group_members(group) {
            self.puzzle.pieces[i].rotate_cw();
        }
        self.board.bring_group_to_front(group);
        // After rotation, try snap in case a match is now aligned.
        if let Some(merged) = self.board.try_snap(&self.puzzle, group) {
            self.status = format!("Rotated and snapped into group {merged}");
        } else {
            self.status = "Rotated group CW".to_string();
        }
    }

    fn rotate_selected(&mut self) {
        let Some(group) = self.selected_group() else {
            return;
        };
        self.rotate_group(group);
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
                ui.checkbox(
                    &mut self.include_dimensions_in_export,
                    "Include rows/cols",
                );
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
                        self.status = format!(
                            "Exported {} pieces to {}",
                            piece_count,
                            path.display()
                        );
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

        if self.selected.is_some() && self.drag.is_none() && !self.export_dialog_open {
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
                self.selected = None;
                self.status = "Selection cleared".to_string();
            }
        }

        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
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
                    let (event, play_area) =
                        draw_board(ui, &self.puzzle, &self.board, self.selected, self.show_labels);
                    self.play_area = play_area;
                    match event {
                        BoardEvent::None => {}
                        BoardEvent::Select(piece) => {
                            self.selected = Some(piece);
                            let group = self.board.group_of(piece);
                            self.board.bring_group_to_front(group);
                            self.status = format!("Selected piece {}", &self.puzzle.pieces[piece].id.to_string()[..8]);
                        }
                        BoardEvent::StartDrag { piece, pointer } => {
                            self.selected = Some(piece);
                            let group = self.board.group_of(piece);
                            self.board.bring_group_to_front(group);
                            self.drag = Some(DragState {
                                piece,
                                last_pointer: pointer,
                                moved: false,
                            });
                        }
                        BoardEvent::DragTo(pointer) => {
                            if let Some(drag) = &mut self.drag {
                                let group = self.board.group_of(drag.piece);
                                let delta = pointer - drag.last_pointer;
                                if delta.length_sq() > 0.0 {
                                    drag.moved = true;
                                    self.board.move_group(group, delta);
                                    drag.last_pointer = pointer;
                                }
                            }
                        }
                        BoardEvent::EndDrag => {
                            if let Some(drag) = self.drag.take() {
                                let group = self.board.group_of(drag.piece);
                                if drag.moved {
                                    if let Some(merged) = self.board.try_snap(&self.puzzle, group) {
                                        let n = self.board.group_members(merged).len();
                                        self.status = format!("Snapped — group now has {n} pieces");
                                    }
                                } else {
                                    self.status = format!(
                                        "Selected piece {}",
                                        &self.puzzle.pieces[drag.piece].id.to_string()[..8]
                                    );
                                }
                            }
                        }
                        BoardEvent::Rotate(piece) => {
                            self.selected = Some(piece);
                            let group = self.board.group_of(piece);
                            self.rotate_group(group);
                        }
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
    DragTo(Pos2),
    EndDrag,
    Rotate(usize),
}

fn draw_board(
    ui: &mut egui::Ui,
    puzzle: &Puzzle,
    board: &Board,
    selected: Option<usize>,
    show_labels: bool,
) -> (BoardEvent, Vec2) {
    let edge_inset = 18.0_f32;
    let edge_margin = 16.0_f32;

    // Fill the visible panel; never smaller than the scramble minimum for this puzzle.
    let min_size = play_content_size(puzzle.rows, puzzle.cols);
    let desired = ui.available_size().max(min_size);
    let (response, painter) = ui.allocate_painter(desired, Sense::click_and_drag());
    let origin = response.rect.min + Vec2::splat(PAD);
    let play_area = (response.rect.size() - Vec2::splat(PAD * 2.0)).max(Vec2::ZERO);
    let max_edge_len = (CELL - edge_margin * 2.0).max(12.0);

    let selected_group = selected.map(|i| board.group_of(i));

    let mut event = BoardEvent::None;

    if response.drag_started()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let local = pointer - origin;
        if let Some(piece) = board.hit_test(local) {
            event = BoardEvent::StartDrag { piece, pointer };
        }
    } else if response.dragged() {
        if let Some(pointer) = response.interact_pointer_pos() {
            event = BoardEvent::DragTo(pointer);
        }
    } else if response.drag_stopped() {
        event = BoardEvent::EndDrag;
    } else if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let local = pointer - origin;
        if let Some(piece) = board.hit_test(local) {
            event = BoardEvent::Select(piece);
        }
    } else if response.secondary_clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let local = pointer - origin;
        if let Some(piece) = board.hit_test(local) {
            event = BoardEvent::Rotate(piece);
        }
    }

    for &i in &board.paint_order() {
        let pose = &board.poses[i];
        let piece = &puzzle.pieces[i];
        let min = origin + pose.pos;
        let rect = Rect::from_min_size(min, Vec2::splat(CELL));
        let is_selected = selected_group == Some(pose.group);

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

    (event, play_area)
}
