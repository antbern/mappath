use std::{sync::Arc, time::Duration};

use eframe::{egui_glow, glow};
use egui::{mutex::Mutex, ColorImage, Pos2, Vec2};
use graphics::{camera::Camera, primitiverenderer::Color, shaperenderer::ShapeRenderer};
use image::DynamicImage;
use nalgebra::{Matrix4, Point2};
use optimize::{
    find::{MapStorage, MapTrait, PathFinder, PathFinderState, Visited},
    grid::{Cell, Direction, GridMap, Point},
    util::parse_img,
};

pub struct App {
    /// Behind an `Arc<Mutex<…>>` so we can pass it to [`egui::PaintCallback`] and paint later.
    world_renderer: Arc<Mutex<WorldRenderer>>,

    state: State,
    background: Option<DynamicImage>,
    output_cell: String,
    output_pathfinder: String,
    pathfinder: Option<
        PathFinder<
            <GridMap<usize> as MapTrait>::Reference,
            CmpCtx,
            usize,
            <GridMap<usize> as MapTrait>::Storage<
                Visited<
                    <GridMap<usize> as MapTrait>::Cost,
                    <GridMap<usize> as MapTrait>::Reference,
                >,
            >,
            GridMap<usize>,
        >,
    >,
}

type CmpCtx = ();

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
struct State {
    label: String,
    value: f32,
    map: GridMap<usize>,
    draw_grid_lines: bool,
    draw_pathfind_debug: bool,
    is_editing: bool,
    start: Option<Point>,
    goal: Option<Point>,
    auto_step: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            value: 0.0,
            label: "Hello, world!".to_owned(),
            map: GridMap::new(10, 10, 1),
            draw_grid_lines: true,
            draw_pathfind_debug: true,
            is_editing: false,
            start: None,
            goal: None,
            auto_step: true,
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let state: State = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        let gl = cc
            .gl
            .as_ref()
            .expect("You need to run eframe with the glow backend");
        App {
            state,
            world_renderer: Arc::new(Mutex::new(WorldRenderer::new(gl))),
            background: None,
            output_cell: Default::default(),
            pathfinder: None,
            output_pathfinder: Default::default(),
        }
    }

    fn set_background(&mut self, image_data: &[u8]) {
        let image = image::load_from_memory(image_data).unwrap();
        self.background = Some(image);
    }

    fn draw_neighbors(&self, point: &Point, sr: &mut ShapeRenderer, color: Color) {
        if !self.state.map.is_valid(*point) {
            return;
        }

        sr.begin(graphics::primitiverenderer::PrimitiveType::Line);
        for (neighbor, _) in self.state.map.neighbors_of(*point) {
            sr.line(
                point.col as f32 + 0.5,
                point.row as f32 + 0.5,
                neighbor.col as f32 + 0.5,
                neighbor.row as f32 + 0.5,
                color,
            );
        }
        sr.end();

        sr.begin(graphics::primitiverenderer::PrimitiveType::Filled);
        let padding = 0.3;
        for (neighbor, _) in self.state.map.neighbors_of(*point) {
            sr.rect(
                neighbor.col as f32 + padding,
                neighbor.row as f32 + padding,
                1.0 - 2.0 * padding,
                1.0 - 2.0 * padding,
                color,
            );
        }
        sr.end();
    }
    fn mouse_world_to_point_valid(&self, x: f32, y: f32) -> Option<Point> {
        if x < 0.0 || y < 0.0 {
            return None;
        }
        let point = Point {
            row: y as usize,
            col: x as usize,
        };
        if self.state.map.is_valid(point) {
            Some(point)
        } else {
            None
        }
    }
}
fn load_image_from_memory(image_data: &[u8]) -> Result<ColorImage, image::ImageError> {
    let image = image::load_from_memory(image_data)?;
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    Ok(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()))
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_dark_light_mode_buttons(ui);
            });
        });

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's

            let mouse_pos = self.world_renderer.lock().last_mouse_pos;
            ui.label(format!("Mouse: [{:.2}, {:.2}]", mouse_pos.x, mouse_pos.y));

            if ui.button("Load Preset").clicked() {
                self.set_background(include_bytes!("../../data/maze-03_6_threshold.png"));

                if let Some(background) = &self.background {
                    let mut map = parse_img(background).unwrap();

                    let start = Point { row: 14, col: 0 };
                    let goal = Point { row: 44, col: 51 };

                    map.cells[10][10] = Cell::OneWay {
                        cost: 1,
                        direction: Direction::Right,
                        target: Some(goal),
                    };

                    let finder = PathFinder::new(
                        start,
                        goal,
                        map.create_storage::<Visited<usize, Point>>(),
                        (),
                    );

                    self.state.map = map;
                    self.state.goal = Some(goal);
                    self.state.start = Some(start);

                    self.pathfinder = Some(finder);

                    // self.on_map_change(context);
                }
            }
            ui.checkbox(&mut self.state.draw_grid_lines, "Draw grid lines");
            ui.checkbox(&mut self.state.draw_pathfind_debug, "Draw Pathfind Debug");

            if let Some(pathfinder) = &mut self.pathfinder {
                ui.label("Pathfinder");
                ui.horizontal(|ui| {
                    if ui.button("Reset").clicked() {
                        if let (Some(start), Some(goal)) = (self.state.start, self.state.goal) {
                            *pathfinder = PathFinder::new(
                                start,
                                goal,
                                self.state.map.create_storage::<Visited<usize, Point>>(),
                                (),
                            );
                        }
                    }
                    if ui.button("Step").clicked() {
                        pathfinder.step(&self.state.map);
                    }

                    if ui.button("Finish").clicked() {
                        loop {
                            match pathfinder.step(&self.state.map) {
                                PathFinderState::Computing => {}
                                _s => break,
                            }
                        }
                    }
                });
                ui.checkbox(&mut self.state.auto_step, "Auto Step");
                if self.state.auto_step {
                    pathfinder.step(&self.state.map);
                    ctx.request_repaint_after(Duration::from_millis(20));
                }
                ui.label(&self.output_cell);
            }

            ui.label(&self.output_pathfinder);

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's

            // Let all nodes do their drawing. Explicit scope for MutexGuard lifetime.
            {
                let mut world = self.world_renderer.lock();

                world
                    .sr
                    .begin(graphics::primitiverenderer::PrimitiveType::Filled);
                for row in 0..self.state.map.rows {
                    for col in 0..self.state.map.columns {
                        let cell = self.state.map.cells[row][col];

                        let color = match cell {
                            Cell::Invalid => Color::BLACK,
                            Cell::Valid { cost: 1 } => Color::WHITE,
                            Cell::Valid { .. } => Color::rgba_u8(255, 255, 0, 255),
                            // TODO: draw these as arrows!
                            Cell::OneWay { target: None, .. } => Color::rgba_u8(0, 255, 255, 255),
                            Cell::OneWay {
                                target: Some(_), ..
                            } => Color::rgba_u8(255, 0, 255, 255),
                        };

                        world.sr.rect(col as f32, row as f32, 1.0, 1.0, color);
                    }
                }
                // mark start and goal cells
                if let Some(goal) = &self.state.goal {
                    world
                        .sr
                        .rect(goal.col as f32, goal.row as f32, 1.0, 1.0, Color::RED);
                }

                if let Some(start) = &self.state.start {
                    world
                        .sr
                        .rect(start.col as f32, start.row as f32, 1.0, 1.0, Color::GREEN);
                }

                world.sr.end();
                // get the cell the user is hovering over
                if let Some(point) =
                    self.mouse_world_to_point_valid(world.last_mouse_pos.x, world.last_mouse_pos.y)
                {
                    world
                        .sr
                        .begin(graphics::primitiverenderer::PrimitiveType::Filled);
                    world
                        .sr
                        .rect(point.col as f32, point.row as f32, 1.0, 1.0, Color::GREEN);
                    world.sr.end();

                    // draw lines to the neighbors of the currently hovered cell
                    self.draw_neighbors(&point, &mut world.sr, Color::GREEN);
                }

                if let Some(pathfinder) = &self.pathfinder {
                    let visited = pathfinder.get_visited();

                    if self.state.draw_pathfind_debug {
                        let margin = 0.15;
                        world
                            .sr
                            .begin(graphics::primitiverenderer::PrimitiveType::Filled);
                        for row in 0..self.state.map.rows {
                            for col in 0..self.state.map.columns {
                                let p = Point { row, col };
                                let v = visited.get(p);

                                if let Some(f) = *v {
                                    let color = Color::rgba(
                                        (f.cost as f32 / 255.0).min(1.0),
                                        0.0,
                                        0.0,
                                        0.8,
                                    );

                                    world.sr.rect(
                                        col as f32 + margin,
                                        row as f32 + margin,
                                        1.0 - 2.0 * margin,
                                        1.0 - 2.0 * margin,
                                        color,
                                    );
                                }
                            }
                        }
                        world.sr.end();
                    }

                    match pathfinder.state() {
                        PathFinderState::Computing => {}
                        PathFinderState::NoPathFound => {
                            self.output_pathfinder = "No path found".to_string();
                        }
                        PathFinderState::PathFound(pr) => {
                            let color = Color::GREEN;

                            world
                                .sr
                                .begin(graphics::primitiverenderer::PrimitiveType::Line);
                            // the width is set relative to the size of one cell
                            if let Some(start) = pr.path.first() {
                                world.sr.line(
                                    start.col as f32 + 0.5,
                                    start.row as f32 + 0.5,
                                    pr.start.col as f32 + 0.5,
                                    pr.start.row as f32 + 0.5,
                                    color,
                                );
                            }
                            for p in pr.path.windows(2) {
                                world.sr.line(
                                    p[0].col as f32 + 0.5,
                                    p[0].row as f32 + 0.5,
                                    p[1].col as f32 + 0.5,
                                    p[1].row as f32 + 0.5,
                                    color,
                                );
                            }
                            if let Some(end) = pr.path.last() {
                                world.sr.line(
                                    end.col as f32 + 0.5,
                                    end.row as f32 + 0.5,
                                    pr.goal.col as f32 + 0.5,
                                    pr.goal.row as f32 + 0.5,
                                    color,
                                );
                            }
                            world.sr.end();
                        }
                    }

                    // get the cell the user is hovering
                    if let Some(point) = self
                        .mouse_world_to_point_valid(world.last_mouse_pos.x, world.last_mouse_pos.y)
                    {
                        let v = visited.get(point);
                        self.output_cell = format!(
                            "Cell @{}:{}\n{:#?}\n\n{:#?}",
                            point.row, point.col, self.state.map.cells[point.row][point.col], v
                        );
                    }
                }

                if self.state.draw_grid_lines {
                    world
                        .sr
                        .begin(graphics::primitiverenderer::PrimitiveType::Line);
                    for row in 0..=self.state.map.rows {
                        world.sr.line(
                            0.0,
                            row as f32,
                            self.state.map.columns as f32,
                            row as f32,
                            Color::rgba_u8(0, 0, 0, 255),
                        );
                    }
                    for col in 0..=self.state.map.columns {
                        world.sr.line(
                            col as f32,
                            0.0,
                            col as f32,
                            self.state.map.rows as f32,
                            Color::rgba_u8(0, 0, 0, 255),
                        );
                    }

                    world.sr.end();
                }
            }

            self.custom_painting(ui);
        });
    }
    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        if let Some(gl) = gl {
            self.world_renderer.lock().destroy(gl);
        }
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}
impl App {
    fn custom_painting(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(
            ui.available_size(), //egui::Vec2::splat(300.0)
            egui::Sense::drag(),
        );

        let zoom_factor = if ui.rect_contains_pointer(rect) {
            // combine the zoom_delta and the scroll amount to support multitouch gestures as well as normal scroll zoom

            let (scroll_delta, zoom_delta) = ui
                .ctx()
                .input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));

            1.0 / (zoom_delta + 0.1 * scroll_delta / 50.0)
        } else {
            1.0
        };

        let pos = if ui.rect_contains_pointer(rect) {
            let mut pos = ui.ctx().pointer_hover_pos().unwrap_or_default();
            // adjust for the position of the allocated space
            pos.x -= rect.left();
            pos.y -= rect.top();
            Some(pos)
        } else {
            None
        };

        // Clone locals so we can move them into the paint callback:

        let mut drag_delta = response.drag_delta();
        drag_delta.y *= -1.0;

        let size = rect.size();
        let world_renderer = self.world_renderer.clone();

        let callback = egui::PaintCallback {
            rect,
            callback: std::sync::Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                world_renderer
                    .lock()
                    .paint(painter.gl(), pos, size, drag_delta, zoom_factor);
            })),
        };
        ui.painter().add(callback);
    }
}

pub struct WorldRenderer {
    pub sr: ShapeRenderer,
    camera: Camera,
    pub last_mouse_pos: Point2<f32>,
}

impl WorldRenderer {
    fn new(gl: &glow::Context) -> Self {
        // use glow::HasContext as _;

        Self {
            sr: ShapeRenderer::new(gl),
            camera: Camera::new(),
            last_mouse_pos: Point2::new(0.0, 0.0),
        }
    }

    fn destroy(&mut self, gl: &glow::Context) {
        self.sr.destroy(gl);
    }

    // fn as_world_object(&mut self) -> WorldObj<'_> {
    //     WorldObj {
    //         sr: &mut self.sr,
    //         last_mouse_pos: self.last_mouse_pos,
    //     }
    // }

    fn paint(
        &mut self,
        gl: &glow::Context,
        pos: Option<Pos2>,
        size: Vec2,
        pan: Vec2,
        zoom_factor: f32,
    ) {
        // first update the camera with any zoom and resize change
        self.camera.resize(size);
        self.camera.pan(pan);
        self.camera.zoom(zoom_factor);
        self.camera.update();

        // set the correct MVP matrix for the shape renderer
        let mvp: Matrix4<f32> = self.camera.get_mvp();
        self.sr.set_mvp(mvp);

        // unproject mouse position to
        if let Some(pos) = pos {
            self.last_mouse_pos = self.camera.unproject(pos);
        }

        // do the actual drawing of already cached vertices
        self.sr.flush(gl);
    }
}
