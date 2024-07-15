use eframe::egui;
use std::{sync::Arc, time::Duration};

use eframe::{egui_glow, glow};
use egui::{mutex::Mutex, Pos2, Vec2};
use graphics::{
    camera::Camera,
    primitiverenderer::Color,
    primitiverenderer_texture::{PrimitiveRendererTexture, RenderTexture},
    shaperenderer::ShapeRenderer,
};
use image::{DynamicImage, GenericImageView};
use nalgebra::Point2;
use optimize::{
    find::{MapStorage, MapTrait, PathFinder, PathFinderState, Visited},
    grid::{Cell, Direction, GridMap, Point},
    util::parse_img,
};

type Reference = <GridMap<usize> as MapTrait>::Reference;

pub struct App {
    /// Behind an `Arc<Mutex<…>>` so we can pass it to [`egui::PaintCallback`] and paint later.
    world_renderer: Arc<Mutex<WorldRenderer>>,

    state: State,
    background: Option<Background>,
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
    mouse_select_state: Option<Box<dyn FnOnce(&mut Self, nalgebra::Point2<f32>)>>,
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

    edit_state: EditState,

    // store whatever the cameare was looking at
    last_camera_position: Option<(nalgebra::Vector2<f32>, f32)>,

    background_alpha: f32,
    foreground_alpha: f32,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct EditState {
    edit_selection: Option<Selection<Reference>>,

    // stuff for selecting rectangles
    selection_start: Option<Reference>,
    selection_end: Option<Reference>,

    /// The number of pixels per cell in the image, used for auto-detecting the grid size
    pixels_per_cell: usize,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Selection<R> {
    start: R,
    end: R,
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
            edit_state: EditState {
                edit_selection: None,
                selection_start: None,
                selection_end: None,
                pixels_per_cell: 1,
            },
            last_camera_position: None,
            background_alpha: 1.0,
            foreground_alpha: 1.0,
        }
    }
}

struct Background {
    image_data: DynamicImage,
    // image: ColorImage,
    // texture_handle: egui::TextureHandle,
    file_name: String,
    scale: f32,
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

        let mut world_renderer = WorldRenderer::new(gl);

        if let Some((pos, zoom)) = state.last_camera_position {
            world_renderer.camera.set_position(pos);
            world_renderer.camera.set_zoom(zoom);
        }

        App {
            state,
            world_renderer: Arc::new(Mutex::new(world_renderer)),
            background: None,
            output_cell: Default::default(),
            pathfinder: None,
            output_pathfinder: Default::default(),
            mouse_select_state: None,
        }
    }

    fn set_background(&mut self, image_data: &[u8], file_name: String) {
        let image = image::load_from_memory(image_data).unwrap();

        self.world_renderer.lock().texture = Texture::New(image.clone());

        self.background = Some(Background {
            image_data: image,
            file_name,
            scale: 1.0,
        });
    }
    fn set_background_image(&mut self, image: DynamicImage, file_name: String) {
        self.world_renderer.lock().texture = Texture::New(image.clone());

        self.background = Some(Background {
            image_data: image,
            file_name,
            scale: 1.0,
        });
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
    fn on_map_change(&mut self) {
        // make sure all selections etc are within bounds
        if let Some(selection) = &mut self.state.edit_state.edit_selection {
            selection.start.row = selection.start.row.min(self.state.map.rows - 1);
            selection.start.col = selection.start.col.min(self.state.map.columns - 1);
            selection.end.row = selection.end.row.min(self.state.map.rows - 1);
            selection.end.col = selection.end.col.min(self.state.map.columns - 1);
        }

        if let Some(start) = &mut self.state.start {
            start.row = start.row.min(self.state.map.rows - 1);
            start.col = start.col.min(self.state.map.columns - 1);
        }

        if let Some(goal) = &mut self.state.goal {
            goal.row = goal.row.min(self.state.map.rows - 1);
            goal.col = goal.col.min(self.state.map.columns - 1);
        }

        // also need to reset the pathfinder
        if let (Some(start), Some(goal)) = (self.state.start, self.state.goal) {
            self.pathfinder = Some(PathFinder::new(
                start,
                goal,
                self.state.map.create_storage::<Visited<usize, Point>>(),
                (),
            ));
        }
    }
}
fn preview_files_being_dropped(ctx: &egui::Context) {
    use egui::*;
    use std::fmt::Write as _;

    if !ctx.input(|i| i.raw.hovered_files.is_empty()) {
        let text = ctx.input(|i| {
            let mut text = "Dropping files:\n".to_owned();
            for file in &i.raw.hovered_files {
                if let Some(path) = &file.path {
                    write!(text, "\n{}", path.display()).ok();
                } else if !file.mime.is_empty() {
                    write!(text, "\n{}", file.mime).ok();
                } else {
                    text += "\n???";
                }
            }
            text
        });

        let painter =
            ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("file_drop_target")));

        let screen_rect = ctx.screen_rect();
        painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(192));
        painter.text(
            screen_rect.center(),
            Align2::CENTER_CENTER,
            text,
            TextStyle::Heading.resolve(&ctx.style()),
            Color32::WHITE,
        );
    }
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

            ui.checkbox(&mut self.state.is_editing, "Edit Mode");
            if self.state.is_editing {
                ui.separator();
                ui.label("Step 1: Select Background");
                preview_files_being_dropped(ctx);

                let file_name = if let Some(b) = &self.background {
                    &b.file_name
                } else {
                    "None"
                };

                ui.label(format!("Selected: {file_name}"));

                ui.label("Drop file to select another background");
                if let Some((image, name)) = ctx.input(|i| {
                    let d = i.raw.dropped_files.first()?;

                    // handle the differences between web and native
                    if let Some(data) = &d.bytes {
                        image::load_from_memory(data)
                            .inspect_err(|e| log::error!("Error loading image: {e}"))
                            .ok()
                            .map(|i| (i, d.name.clone()))
                    } else if let Some(path) = &d.path {
                        image::open(path)
                            .inspect_err(|e| log::error!("Error loading image: {e}"))
                            .ok()
                            .map(|i| (i, d.name.clone()))
                    } else {
                        None
                    }
                }) {
                    self.set_background_image(image, name);
                }

                ui.separator();
                ui.label("Step 2: Decide Grid Size");

                let mut changed = false;
                if let Some(b) = &mut self.background {
                    ui.horizontal(|ui| {
                        ui.label("Pixels per cell: ");
                        ui.add(
                            egui::widgets::DragValue::new(
                                &mut self.state.edit_state.pixels_per_cell,
                            )
                            .range(1..=100),
                        );
                        if ui.button("Auto Scale").clicked() {
                            let ppc = self.state.edit_state.pixels_per_cell as f32;
                            let rows = b.image_data.height() as f32 / ppc;
                            let cols = b.image_data.width() as f32 / ppc;
                            self.state.map.resize(cols as usize, rows as usize);
                            b.scale = 1.0 / ppc;
                            changed = true;
                        }
                    });
                }
                if changed {
                    self.on_map_change();
                }

                ui.separator();
                ui.label("Step 3: Edit Cells");

                if ui
                    .add_enabled(
                        self.background.is_some(),
                        egui::widgets::Button::new("Auto Fill Map"),
                    )
                    .clicked()
                {
                    self.mouse_select_state = Some(Box::new(|s, p| {
                        if let Some(background) = &s.background {
                            // get the pixel at the center of that cell
                            let x = (p.x / background.scale) as u32;
                            let y = (p.y / background.scale) as u32;

                            let (width, height) = background.image_data.dimensions();
                            if x < width && y < height {
                                let color = background.image_data.get_pixel(x, height - y - 1);
                                log::info!("Selected color: {:?}", color);

                                // generate a map based on the selected color
                                fill_map_from_image(
                                    &mut s.state.map,
                                    &background.image_data,
                                    background.scale as f64,
                                    &color,
                                );
                            } else {
                                log::info!("Selected color is out of bounds");
                            }

                            s.on_map_change();
                        }
                    }));
                }

                // TODO: cell editing here

                ui.separator();
            }

            if ui.button("Load Preset").clicked() {
                self.set_background(
                    include_bytes!("../../data/maze-03_6_threshold.png"),
                    "maze".to_string(),
                );

                if let Some(background) = &self.background {
                    let mut map = parse_img(&background.image_data).unwrap();

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

                    self.on_map_change();
                }
            }
            ui.checkbox(&mut self.state.draw_grid_lines, "Draw grid lines");
            ui.checkbox(&mut self.state.draw_pathfind_debug, "Draw Pathfind Debug");
            ui.add(
                egui::widgets::Slider::new(&mut self.state.background_alpha, 0.0..=1.0)
                    .text("Background Alpha"),
            );
            ui.add(
                egui::widgets::Slider::new(&mut self.state.foreground_alpha, 0.0..=1.0)
                    .text("Foreground Alpha"),
            );

            // Another option to select the opacity with a single slider only!
            let mut value = self.state.foreground_alpha;
            if ui
                .add(
                    egui::widgets::Slider::new(&mut value, 0.0..=1.0)
                        .text("Background <-> Foreground"),
                )
                .changed()
            {
                self.state.foreground_alpha = value;
                self.state.background_alpha = 1.0 - value;
            }

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
            ui.label("In path finding mode: Click to select start, Shift-Click to select goal.");

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            })
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's

            // Explicit scope for MutexGuard lifetime.
            let last_mouse_pos = {
                let mut world = self.world_renderer.lock();

                // store the last camera position for the next time the page is reloaded
                self.state.last_camera_position =
                    Some((world.camera.get_position().clone(), world.camera.get_zoom()));

                // draws the background image
                if let Some(background) = &self.background {
                    world
                        .pr_texture
                        .begin(graphics::primitiverenderer::PrimitiveType::Filled);
                    let x = 0.0;
                    let y = 0.0;
                    let width = background.image_data.width() as f32 * background.scale;
                    let height = background.image_data.height() as f32 * background.scale;
                    let color = Color::rgba(1.0, 1.0, 1.0, self.state.background_alpha);

                    // add the vertices for the image quad, and flip the y axis so that the image is correctly drawn
                    world.pr_texture.xyzc(x, y, 0.0, color, 0.0, 1.0);
                    world.pr_texture.xyzc(x + width, y, 0.0, color, 1.0, 1.0);
                    world
                        .pr_texture
                        .xyzc(x + width, y + height, 0.0, color, 1.0, 0.0);
                    world
                        .pr_texture
                        .xyzc(x + width, y + height, 0.0, color, 1.0, 0.0);
                    world.pr_texture.xyzc(x, y + height, 0.0, color, 0.0, 0.0);
                    world.pr_texture.xyzc(x, y, 0.0, color, 0.0, 1.0);

                    world.pr_texture.end();
                }

                // draw the grid
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

                // draw the selection rectangle
                if self.state.is_editing {
                    if let Some(selection) = &self.state.edit_state.edit_selection {
                        let color = Color::rgba_u8(0, 255, 0, 128);
                        let Selection { start, end } = selection;
                        world.sr.rect(
                            start.col as f32,
                            start.row as f32,
                            end.col as f32 - start.col as f32 + 1.0,
                            end.row as f32 - start.row as f32 + 1.0,
                            color,
                        );
                    }
                }

                world.sr.end();

                if self.state.is_editing && self.mouse_select_state.is_some() {
                    world
                        .sr
                        .begin(graphics::primitiverenderer::PrimitiveType::Line);
                    let color = Color::rgba_u8(255, 0, 0, 255);
                    let x = world.last_mouse_pos.x;
                    let y = world.last_mouse_pos.y;
                    let width = 10.0;
                    world.sr.line(x - width, y, x + width, y, color);
                    world.sr.line(x, y - width, x, y + width, color);
                    world.sr.end();
                }

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

                // draw the pathfinder debug information
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
                world.last_mouse_pos
            };
            // do logic based on mouse input
            let (mouse_clicked, mouse_pressed, mouse_down, mouse_released) = ui.input(|r| {
                (
                    r.pointer.primary_clicked(),
                    r.pointer.primary_pressed(),
                    r.pointer.primary_down(),
                    r.pointer.primary_released(),
                )
            });
            let modifiers = ui.input(|r| r.modifiers);

            if self.mouse_select_state.is_some() {
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.mouse_select_state = None;
                }
                if ui.ui_contains_pointer() && mouse_clicked {
                    if let Some(callback) = self.mouse_select_state.take() {
                        callback(self, last_mouse_pos);
                    }
                }
            }

            let mut start_goal_changed = false;
            if let Some(point) = self.mouse_world_to_point_valid(last_mouse_pos.x, last_mouse_pos.y)
            {
                if !self.state.is_editing {
                    if mouse_clicked && !modifiers.shift {
                        self.state.start = Some(point);
                        start_goal_changed = true;
                    } else if mouse_clicked && modifiers.shift {
                        self.state.goal = Some(point);
                        start_goal_changed = true;
                    }
                } else if !modifiers.shift {
                    if mouse_clicked && self.mouse_select_state.is_some() {
                    } else if mouse_pressed {
                        // initialize region selection
                        self.state.edit_state.selection_start = Some(point);
                        self.state.edit_state.selection_end = Some(point);
                        self.state.edit_state.edit_selection = Some(Selection {
                            start: point,
                            end: point,
                        });
                    } else if mouse_released {
                        self.state.edit_state.selection_start = None;
                        self.state.edit_state.selection_end = None;

                        if let Some(selection) = &self.state.edit_state.edit_selection {
                            // TODO: load values from the selection here into the editor
                            let cell =
                                self.state.map.cells[selection.start.row][selection.start.col];
                            log::debug!("Selected region first cell: {:#?}", cell);
                        }
                    } else if mouse_down {
                        // update region selection
                        if let Some(start) = self.state.edit_state.selection_start {
                            self.state.edit_state.selection_end = Some(point);
                            let (start, end) = (
                                Point {
                                    row: start.row.min(point.row),
                                    col: start.col.min(point.col),
                                },
                                Point {
                                    row: start.row.max(point.row),
                                    col: start.col.max(point.col),
                                },
                            );

                            self.state.edit_state.edit_selection = Some(Selection { start, end });
                        }
                    }
                }
            }

            // need to reinitialize the pathfinder if the start or goal has changed
            if start_goal_changed {
                if let (Some(start), Some(goal)) = (self.state.start, self.state.goal) {
                    let finder = PathFinder::new(
                        start,
                        goal,
                        self.state.map.create_storage::<Visited<usize, Point>>(),
                        (),
                    );

                    self.pathfinder = Some(finder);
                } else {
                    self.pathfinder = None;
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

        let mut drag_delta = response.drag_delta();
        drag_delta.y *= -1.0;

        if self.state.is_editing && !ui.input(|i| i.modifiers.shift) {
            drag_delta = Vec2::ZERO;
        }

        let size = rect.size();

        let global_alpha = self.state.foreground_alpha;
        // Clone locals so we can move them into the paint callback:
        let world_renderer = self.world_renderer.clone();

        let callback = egui::PaintCallback {
            rect,
            callback: std::sync::Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                world_renderer.lock().paint(
                    painter.gl(),
                    pos,
                    size,
                    drag_delta,
                    zoom_factor,
                    global_alpha,
                );
            })),
        };
        ui.painter().add(callback);
    }
}

pub struct WorldRenderer {
    pub sr: ShapeRenderer,
    pub pr_texture: PrimitiveRendererTexture,
    camera: Camera,
    pub last_mouse_pos: Point2<f32>,
    pub texture: Texture,
}
pub enum Texture {
    None,
    New(DynamicImage),
    Existing(RenderTexture),
}

impl WorldRenderer {
    fn new(gl: &glow::Context) -> Self {
        // use glow::HasContext as _;

        Self {
            sr: ShapeRenderer::new(gl),
            pr_texture: PrimitiveRendererTexture::new(gl, 1000),
            camera: Camera::new(),
            last_mouse_pos: Point2::new(0.0, 0.0),
            texture: Texture::None,
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
        global_alpha: f32,
    ) {
        // first update the camera with any zoom and resize change
        self.camera.resize(size);
        self.camera.pan(pan);
        self.camera.zoom(zoom_factor);
        self.camera.update();

        // set the correct MVP matrix for the shape renderer
        let mvp = self.camera.get_mvp();
        self.sr.set_mvp(mvp);
        self.pr_texture.set_mvp(mvp);

        self.sr.set_global_alpha(global_alpha);

        // enable blending for transparency
        unsafe {
            use eframe::glow::HasContext as _;
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
        }
        // unproject mouse position to
        if let Some(pos) = pos {
            self.last_mouse_pos = self.camera.unproject(pos);
        }

        // if there is a new texture to use, load it
        if let Texture::New(image) = &self.texture {
            // TODO: destroy the old one using opengl to avoid memory leaks!

            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();

            let texture_id = self.pr_texture.create_texture(
                gl,
                &pixels.as_slice(),
                image.width(),
                image.height(),
            );

            log::info!("Generated with Texture ID ");

            self.texture = Texture::Existing(texture_id);
        }

        // TODO: draw image and shapes with opacity

        // do the actual drawing of already cached vertices
        if let Texture::Existing(texture_id) = &self.texture {
            self.pr_texture.flush(gl, texture_id);
        }
        self.sr.flush(gl);
    }
}

/// Fills a map based on the pixels on an image and a selected color for valid cells
fn fill_map_from_image(
    map: &mut GridMap<usize>,
    image: &DynamicImage,
    image_scale: f64,
    color: &image::Rgba<u8>,
) {
    let image_height = image.height();
    for row in 0..map.rows {
        for col in 0..map.columns {
            // find the pixel at the center of the cell
            let (x, y) = (col as f64 + 0.5, row as f64 + 0.5);
            let (x, y) = (x / image_scale, y / image_scale);
            let (x, y) = (x as u32, image_height - y as u32 - 1);
            let pixel = image.get_pixel(x, y);

            let diff = pixel_difference_norm(&pixel, color);

            if diff < 10.0 {
                map.cells[row][col] = Cell::Valid { cost: 1 };
            } else {
                map.cells[row][col] = Cell::Invalid;
            }
        }
    }
}

fn pixel_difference_norm(a: &image::Rgba<u8>, b: &image::Rgba<u8>) -> f64 {
    let a = a.0;
    let b = b.0;
    let diff = [
        (a[0] as f64 - b[0] as f64).abs(),
        (a[1] as f64 - b[1] as f64).abs(),
        (a[2] as f64 - b[2] as f64).abs(),
    ];
    let diff = (diff[0].powi(2) + diff[1].powi(2) + diff[2].powi(2)).sqrt();
    diff
}
