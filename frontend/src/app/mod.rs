mod ui;
use crate::context::Context;
use crate::event::{
    ButtonId, CheckboxId, Event, InputChange, MouseButton, MouseEvent, NumberInputId, SelectId,
};
use crate::App;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{DynamicImage, GenericImageView};
use log::debug;
use optimize::{parse_img, Cell, Map, MapTrait, PathFinder, Point, Visited};
use optimize::{MapStorage, PathFinderState};
use std::io::Cursor;
use wasm_bindgen::Clamped;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, ImageBitmap};
use web_sys::{HtmlInputElement, ImageData};

use self::ui::camera::Camera;

const STORAGE_KEY_MAP: &str = "map";
const STORAGE_KEY_BACKGROUND: &str = "background";

pub(crate) trait AppMapTrait:
    MapTrait + serde::Serialize + for<'de> serde::Deserialize<'de>
{
}
impl<T> AppMapTrait for T where T: MapTrait + serde::Serialize + for<'de> serde::Deserialize<'de> {}

pub struct AppImpl<M: AppMapTrait> {
    editing: bool,
    map: M,

    find_state: Option<FindState<M>>,
    start: Option<M::Reference>,
    goal: Option<M::Reference>,
    auto_step: bool,
    edit_selection: Option<Selection<M::Reference>>,

    // stuff for selecting rectangles
    selection_start: Option<M::Reference>,
    selection_end: Option<M::Reference>,

    // for panning and dragging
    last_pan_position: Option<(i32, i32)>,
    camera: Camera,

    // for doing interactive selections with the mouse
    mouse_select_state: Option<MouseSelectState<M>>,
    // background stuff
    background: Option<Background>,
    map_alpha: f64,
    background_alpha: f64,

    draw_grid: bool,
}

struct Selection<R> {
    start: R,
    end: R,
}

struct FindState<M: AppMapTrait> {
    pathfinder: PathFinder<M::Reference, M::Storage<Visited<M::Reference>>, M>,
}

struct MouseSelectState<M: AppMapTrait> {
    callback: Box<dyn FnOnce(&mut AppImpl<M>, &Context, MouseEvent)>,
}

struct Background {
    image_data: DynamicImage,
    image: ImageBitmap,
    scale: f64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SerializableBackground {
    image_data_base64: String,
    scale: f64,
}

impl From<&Background> for SerializableBackground {
    fn from(b: &Background) -> Self {
        let mut buf = Cursor::new(Vec::new());
        b.image_data
            .write_to(&mut buf, image::ImageOutputFormat::Png)
            .unwrap();

        Self {
            image_data_base64: STANDARD.encode(buf.into_inner()),
            scale: b.scale,
        }
    }
}

impl AppImpl<Map> {
    pub async fn new(context: &Context) -> Self {
        // if the map has been stored in the browser, get it from there
        let map = if let Some(map) = context.get_storage::<Map>(STORAGE_KEY_MAP) {
            debug!("loaded map from storage");
            map
        } else {
            Map::new(10, 10)
        };

        let mut s = Self {
            editing: false,
            map,
            find_state: None,
            start: None,
            goal: None,
            auto_step: true,
            edit_selection: None,
            selection_start: None,
            selection_end: None,
            last_pan_position: None,
            camera: Camera::new(10.0),
            mouse_select_state: None,
            background: None,
            map_alpha: 0.8,
            background_alpha: 0.8,
            draw_grid: true,
        };

        // load the background if it was stored
        let loaded_background =
            context.get_storage::<Option<SerializableBackground>>(STORAGE_KEY_BACKGROUND);
        if let Some(Some(background)) = loaded_background {
            let data = STANDARD.decode(&background.image_data_base64).unwrap();
            s.set_background(&data).await;
            if let Some(b) = &mut s.background {
                b.scale = background.scale;
            }
            debug!("loaded background from storage");
        }

        s.set_editing(false, context);
        s.on_map_change(context);
        s
    }
}

impl App for AppImpl<Map> {
    async fn render(&mut self, context: &Context, ctx: &CanvasRenderingContext2d) {
        // handle any pending events
        while let Some(event) = context.pop_event() {
            // give the event to panning and zooming first, and if it was not handled, give it to the app
            if !self.handle_event_panning(&event) {
                self.handle_event(event, context).await;
            }
        }

        self.render_app(context, ctx);
    }
}
impl AppImpl<Map> {
    async fn handle_event(&mut self, event: Event, context: &Context) {
        // switch mode if the mode buttons were pressed
        match event {
            Event::ButtonPressed(ButtonId::ClearStorage) => {
                if gloo::dialogs::confirm("Are you sure you want to clear the storage?") {
                    context.remove_storage(STORAGE_KEY_MAP);
                    context.remove_storage(STORAGE_KEY_BACKGROUND);
                }
            }
            Event::ButtonPressed(ButtonId::ToggleEdit) => self.set_editing(!self.editing, context),
            Event::InputChanged(InputChange::Checkbox {
                id: CheckboxId::AutoStep,
                value: checked,
            }) => self.auto_step = checked,
            Event::InputChanged(InputChange::Checkbox {
                id: CheckboxId::DrawGrid,
                value,
            }) => self.draw_grid = value,
            Event::InputChanged(InputChange::Number {
                id: NumberInputId::BackgroundAlpha,
                value,
            }) => {
                self.background_alpha = value;
            }
            Event::InputChanged(InputChange::Number {
                id: NumberInputId::ForegroundAlpha,
                value,
            }) => self.map_alpha = value,
            _ => {}
        }
        // handle the event depending on the current mode
        if let Some(mouse_select_state) = self.mouse_select_state.take() {
            if let Event::MouseReleased(event) = event {
                (mouse_select_state.callback)(self, context, event);
                return;
            } else if let Event::ButtonPressed(ButtonId::SelectPoint) = event {
                // cancel the selection
                return;
            }
            self.mouse_select_state = Some(mouse_select_state);

            // return to prevent any events from being delivered while selecting
            return;
        }

        if self.editing {
            self.handle_event_edit(event, context).await;
        } else {
            self.handle_event_path_find(event, context);
        }
    }

    fn mouse_to_world_point_valid(&self, x: i32, y: i32) -> Option<Point> {
        let (x, y) = self.camera.pixel_to_world(x, y);

        if x < 0.0 || y < 0.0 {
            return None;
        }
        let point = Point {
            row: y as usize,
            col: x as usize,
        };
        if self.map.is_valid(point) {
            Some(point)
        } else {
            None
        }
    }

    fn handle_event_panning(&mut self, event: &Event) -> bool {
        match event {
            Event::MousePressed(MouseEvent {
                x,
                y,
                button: MouseButton::Main,
                ctrl_pressed: true,
                ..
            }) => {
                self.last_pan_position = Some((*x, *y));
                true
            }
            Event::MouseMove(MouseEvent {
                x,
                y,
                ctrl_pressed: true,
                ..
            }) => {
                if let Some(last_pan_position) = self.last_pan_position {
                    let (x, y) = (*x, *y);
                    let (dx, dy) = (x - last_pan_position.0, y - last_pan_position.1);
                    self.camera.pan_pixels(dx, dy);
                    self.last_pan_position = Some((x, y));
                    true
                } else {
                    false
                }
            }

            Event::MouseReleased(_)
            | Event::MouseMove(MouseEvent {
                ctrl_pressed: false,
                ..
            }) => {
                if self.last_pan_position.is_some() {
                    self.last_pan_position = None;
                    true
                } else {
                    false
                }
            }
            Event::MouseWheel {
                x,
                y,
                delta_x: _,
                delta_y,
            } => {
                let scale_factor = 1.1;
                let scale_factor = if *delta_y < 0.0 {
                    scale_factor
                } else {
                    1.0 / scale_factor
                };
                self.camera.zoom_at(*x, *y, scale_factor);
                true
            }
            _ => false,
        }
    }

    fn set_editing(&mut self, editing: bool, context: &Context) {
        //
        self.editing = editing;
        if self.editing {
            // enable the edit inputs
            context.enable_element("edit-inputs", true);
        } else {
            // disable the edit inputs
            context.enable_element("edit-inputs", false);

            // store the map in the localstorage
            context.set_storage(STORAGE_KEY_MAP, &self.map);

            // store the background in the localstorage
            context.set_storage(
                STORAGE_KEY_BACKGROUND,
                &self.background.as_ref().map(SerializableBackground::from),
            );
        }
    }

    async fn handle_event_edit(&mut self, event: Event, context: &Context) {
        match event {
            Event::ButtonPressed(ButtonId::LoadPreset) => {
                // load the image by including the bytes in the compilation

                self.set_background(include_bytes!("../../../data/maze-03_6_threshold.png"))
                    .await;

                if let Some(background) = &self.background {
                    let map = parse_img(&background.image_data).unwrap();

                    let start = Point { row: 14, col: 0 };
                    let goal = Point { row: 44, col: 51 };

                    let finder =
                        PathFinder::new(start, goal, map.create_storage::<Visited<Point>>());

                    self.map = map;
                    self.goal = Some(goal);
                    self.start = Some(start);

                    self.find_state = Some(FindState { pathfinder: finder });

                    self.on_map_change(context);
                }
            }
            Event::ButtonPressed(ButtonId::SelectPoint) => {
                self.mouse_select_state = Some(MouseSelectState {
                    callback: Box::new(|app, context, event| {
                        if let Some(point) = app.mouse_to_world_point_valid(event.x, event.y) {
                            context.set_output(&format!("Selected point: {:?}", point));
                        }
                    }),
                });
            }
            Event::MousePressed(MouseEvent {
                x,
                y,
                button: MouseButton::Main,
                ..
            }) => {
                if let Some(point) = self.mouse_to_world_point_valid(x, y) {
                    self.selection_start = Some(point);
                    self.selection_end = Some(point);
                    self.edit_selection = Some(Selection {
                        start: point,
                        end: point,
                    });
                }
            }
            Event::MouseReleased(MouseEvent {
                x: _,
                y: _,
                button: MouseButton::Main,
                ..
            }) => {
                self.selection_start = None;
                self.selection_end = None;

                // TODO: load the values from the selected area (if applicable)
                if let Some(selection) = &self.edit_selection {
                    let cell = self.map.cells[selection.start.row][selection.start.col];
                    context.set_active_cell(cell);
                }
            }
            Event::ButtonPressed(ButtonId::EditSave) => {
                if let (Some(selection), Some(cell)) =
                    (&self.edit_selection, context.get_active_cell())
                {
                    for row in selection.start.row..=selection.end.row {
                        for col in selection.start.col..=selection.end.col {
                            self.map.cells[row][col] = cell;
                        }
                    }
                }
            }
            Event::MouseMove(MouseEvent { x, y, .. }) => {
                if let Some(start) = self.selection_start {
                    if let Some(end) = self.mouse_to_world_point_valid(x, y) {
                        self.selection_end = Some(end);

                        // update the internal selection statelet (start, end) = (
                        let (start, end) = (
                            Point {
                                row: start.row.min(end.row),
                                col: start.col.min(end.col),
                            },
                            Point {
                                row: start.row.max(end.row),
                                col: start.col.max(end.col),
                            },
                        );

                        self.edit_selection = Some(Selection { start, end });
                    }
                }
            }
            Event::InputChanged(change) => match change {
                InputChange::Number {
                    id: NumberInputId::Rows,
                    value,
                } => {
                    // resize the map
                    self.map.resize(self.map.columns, value as usize);
                    self.on_map_change(context);
                }
                InputChange::Number {
                    id: NumberInputId::Cols,
                    value,
                } => {
                    // resize the map
                    self.map.resize(value as usize, self.map.rows);
                    self.on_map_change(context);
                }
                _ => {}
            },

            Event::ButtonPressed(ButtonId::AutoScale) => {
                if let Some(background) = &mut self.background {
                    let InputChange::Number { id: _, value: ppc } = context.get_input_value(
                        crate::event::InputId::Number(NumberInputId::AutoScaleFactor),
                    ) else {
                        unreachable!()
                    };

                    let rows = background.image.height() as f64 / ppc;
                    let cols = background.image.width() as f64 / ppc;
                    self.map.resize(cols as usize, rows as usize);
                    background.scale = 1.0 / ppc;
                    self.on_map_change(context);
                }
            }
            Event::ButtonPressed(ButtonId::AutoCreateMap) => {
                if self.background.is_some() {
                    context.set_output("Click to select color that represents a valid free cell");
                    self.mouse_select_state = Some(MouseSelectState {
                        callback: Box::new(|app, context, event| {
                            if let Some(background) = app.background.as_ref() {
                                let (x, y) = app.camera.pixel_to_world(event.x, event.y);
                                let (x, y) =
                                    ((x / background.scale) as u32, (y / background.scale) as u32);

                                let (width, height) = background.image_data.dimensions();
                                if x < width && y < height {
                                    let color = background.image_data.get_pixel(x, y);
                                    context.set_output(&format!("Selected color: {:?}", color));

                                    // generate a map based on the selected color
                                    fill_map_from_image(
                                        &mut app.map,
                                        &background.image_data,
                                        background.scale,
                                        &color,
                                    );
                                } else {
                                    context.set_output("Selected color is out of bounds");
                                }
                            }
                        }),
                    });
                }
            }
            Event::ButtonPressed(ButtonId::LoadBackground) => {
                let InputChange::Select {
                    id: _,
                    value: preset,
                } = context
                    .get_input_value(crate::event::InputId::Select(SelectId::BackgroundPreset))
                else {
                    unreachable!()
                };

                match preset.as_str() {
                    "file" => {
                        // open the file in the file picker
                        let window = web_sys::window().unwrap();
                        let file_element: HtmlInputElement = window
                            .document()
                            .unwrap()
                            .get_element_by_id("input-file")
                            .unwrap()
                            .dyn_into::<web_sys::HtmlInputElement>()
                            .unwrap();

                        if let Some(file_list) = file_element.files() {
                            if let Some(file) = gloo::file::FileList::from(file_list).iter().next()
                            {
                                let res = gloo::file::futures::read_as_bytes(file).await;
                                match res {
                                    Ok(bytes) => {
                                        self.set_background(&bytes).await;
                                    }
                                    Err(e) => {
                                        context.set_output(&format!("Error reading file: {:?}", e));
                                    }
                                }
                            }
                        }
                    }
                    "maze" => {
                        self.set_background(include_bytes!(
                            "../../../data/maze-03_6_threshold.png"
                        ))
                        .await;
                    }
                    "maze_map" => {
                        self.set_background(include_bytes!("../../../data/map_maze.png"))
                            .await;
                    }
                    _ => {}
                }
            }

            _ => {}
        }
    }

    async fn set_background(&mut self, bytes: &[u8]) {
        let dynamic_image = image::load_from_memory(bytes).expect("could not load image");

        let rgba_image = dynamic_image.to_rgba8();

        let clamped_buf: Clamped<&[u8]> = Clamped(rgba_image.as_raw());
        let image_data_temp = ImageData::new_with_u8_clamped_array_and_sh(
            clamped_buf,
            dynamic_image.width(),
            dynamic_image.height(),
        )
        .unwrap();

        let jsimage = web_sys::window()
            .expect("no global `window` exists")
            .create_image_bitmap_with_image_data(&image_data_temp)
            .unwrap();

        let jsimage = wasm_bindgen_futures::JsFuture::from(jsimage)
            .await
            .unwrap()
            .into();

        debug!("loaded background image");

        self.background = Some(Background {
            image_data: dynamic_image,
            image: jsimage,
            scale: 1.0,
        });
    }

    fn on_map_change(&mut self, context: &Context) {
        // we have a new map, make sure everything is up to date
        context.set_input_value(&InputChange::Number {
            id: NumberInputId::Rows,
            value: self.map.rows as f64,
        });
        context.set_input_value(&InputChange::Number {
            id: NumberInputId::Cols,
            value: self.map.columns as f64,
        });

        // make sure all selections etc are within bounds
        if let Some(selection) = &mut self.edit_selection {
            selection.start.row = selection.start.row.min(self.map.rows - 1);
            selection.start.col = selection.start.col.min(self.map.columns - 1);
            selection.end.row = selection.end.row.min(self.map.rows - 1);
            selection.end.col = selection.end.col.min(self.map.columns - 1);
        }

        if let Some(start) = &mut self.start {
            start.row = start.row.min(self.map.rows - 1);
            start.col = start.col.min(self.map.columns - 1);
        }

        if let Some(goal) = &mut self.goal {
            goal.row = goal.row.min(self.map.rows - 1);
            goal.col = goal.col.min(self.map.columns - 1);
        }

        // also need to reset the pathfinder
        if let (Some(start), Some(goal)) = (self.start, self.goal) {
            self.find_state = Some(FindState {
                pathfinder: PathFinder::new(
                    start,
                    goal,
                    self.map.create_storage::<Visited<Point>>(),
                ),
            });
        }
    }

    fn handle_event_path_find(&mut self, event: Event, _context: &Context) {
        match event {
            Event::ButtonPressed(ButtonId::Reset) => {
                if let (Some(start), Some(goal)) = (self.start, self.goal) {
                    self.find_state = Some(FindState {
                        pathfinder: PathFinder::new(
                            start,
                            goal,
                            self.map.create_storage::<Visited<Point>>(),
                        ),
                    });
                }
            }
            Event::ButtonPressed(ButtonId::Step) => {
                if let Some(pathfinder) = &mut self.find_state {
                    pathfinder.pathfinder.step(&self.map);
                }
            }
            Event::ButtonPressed(ButtonId::Finish) => loop {
                if let Some(pathfinder) = &mut self.find_state {
                    match pathfinder.pathfinder.step(&self.map) {
                        PathFinderState::Computing => {}
                        _s => break,
                    }
                }
            },

            Event::MouseReleased(MouseEvent {
                x,
                y,
                button: MouseButton::Main,
                shift_pressed,
                ..
            }) => {
                if let Some(point) = self.mouse_to_world_point_valid(x, y) {
                    match shift_pressed {
                        false => self.start = Some(point),
                        true => self.goal = Some(point),
                    }

                    debug!("{:?} -> {:?}", self.start, self.goal);
                    if let (Some(start), Some(goal)) = (self.start, self.goal) {
                        self.find_state = Some(FindState {
                            pathfinder: PathFinder::new(
                                start,
                                goal,
                                self.map.create_storage::<Visited<Point>>(),
                            ),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    fn render_app(&mut self, context: &Context, ctx: &CanvasRenderingContext2d) {
        let canvas = ctx.canvas().unwrap();
        ctx.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
        ctx.save();
        ctx.set_line_width(1.0 / 10.0);

        // apply the camera transformation to the canvas transformation
        let scale = self.camera.scale();
        let offset = self.camera.offset();
        ctx.scale(scale, scale).unwrap();
        ctx.translate(offset.0, offset.1).unwrap();

        // TODO: draw background image with alpha
        if let Some(background) = &self.background {
            ctx.set_global_alpha(self.background_alpha);
            ctx.set_image_smoothing_enabled(false);
            ctx.draw_image_with_image_bitmap_and_dw_and_dh(
                &background.image,
                0.0,
                0.0,
                background.image.width() as f64 * background.scale,
                background.image.height() as f64 * background.scale,
            )
            .unwrap();
        }

        ctx.set_global_alpha(self.map_alpha);

        // render based on the current mode
        if self.editing {
            self.render_app_edit(context, ctx);
        } else {
            // autostep if autostep is enabled and we still have steps to complete
            if self.auto_step {
                if let Some(pathfinder) = &mut self.find_state {
                    for _ in 0..5 {
                        match pathfinder.pathfinder.step(&self.map) {
                            PathFinderState::Computing => {
                                // request another animation frame
                                context.request_repaint();
                            }
                            _ => break,
                        }
                    }
                }
            }
            self.render_app_find(context, ctx);
        }

        ctx.restore();

        // if we are in point selection mode, draw a crosshair at the mouse position
        if let Some(MouseSelectState { .. }) = self.mouse_select_state {
            if let Some((x, y)) = context.input(|input| input.current_mouse_position()) {
                ctx.set_stroke_style(&"#FF0000".into());
                ctx.begin_path();
                ctx.move_to(x as f64, 0.0);
                ctx.line_to(x as f64, canvas.height() as f64);
                ctx.move_to(0.0, y as f64);
                ctx.line_to(canvas.width() as f64, y as f64);
                ctx.stroke();
            }
        }
    }

    fn render_map(&self, _context: &Context, ctx: &CanvasRenderingContext2d) {
        for row in 0..self.map.rows {
            for col in 0..self.map.columns {
                let cell = self.map.cells[row][col];

                let color: String = match cell {
                    Cell::Invalid => "#000000".into(),
                    Cell::Valid { cost: 1 } => "#FFFFFF".into(),
                    Cell::Valid { .. } => "#FFFF00".into(),
                    Cell::OneWay { .. } => "#00FFFF".into(),
                };

                ctx.set_fill_style(&color.into());
                ctx.fill_rect(col as f64, row as f64, 1.0, 1.0);
            }
        }
        if self.draw_grid {
            // draw lines between all the cells
            ctx.set_stroke_style(&"#000000".into());
            ctx.begin_path();
            for row in 0..=self.map.rows {
                ctx.move_to(0.0, row as f64);
                ctx.line_to(self.map.columns as f64, row as f64);
            }
            for col in 0..=self.map.columns {
                ctx.move_to(col as f64, 0.0);
                ctx.line_to(col as f64, self.map.rows as f64);
            }
            ctx.stroke();
        }
    }

    fn draw_neighbors(&self, point: &Point, ctx: &CanvasRenderingContext2d, style: &str) {
        if !self.map.is_valid(*point) {
            return;
        }

        ctx.set_stroke_style(&style.into());
        ctx.begin_path();
        for (neighbor, _) in self.map.neighbors_of(*point) {
            ctx.move_to(point.col as f64 + 0.5, point.row as f64 + 0.5);
            ctx.line_to(neighbor.col as f64 + 0.5, neighbor.row as f64 + 0.5);
        }
        ctx.stroke();

        let padding = 0.3;
        ctx.set_fill_style(&style.into());
        for (neighbor, _) in self.map.neighbors_of(*point) {
            ctx.fill_rect(
                neighbor.col as f64 + padding,
                neighbor.row as f64 + padding,
                1.0 - 2.0 * padding,
                1.0 - 2.0 * padding,
            );
        }
    }
    fn render_app_edit(&self, context: &Context, ctx: &CanvasRenderingContext2d) {
        self.render_map(context, ctx);

        // draw lines to the neighbors of the currently hovered cell
        if let Some((x, y)) = context.input(|input| input.current_mouse_position()) {
            if let Some(point) = self.mouse_to_world_point_valid(x, y) {
                self.draw_neighbors(&point, ctx, "#00FF00");
            }
        }

        if let Some(selection) = &self.edit_selection {
            let Selection { start, end } = selection;

            ctx.set_fill_style(&"rgba(0, 255, 0, 0.5)".into());
            ctx.fill_rect(
                start.col as f64,
                start.row as f64,
                end.col as f64 - start.col as f64 + 1.0,
                end.row as f64 - start.row as f64 + 1.0,
            );
        }
    }

    fn render_app_find(&self, context: &Context, ctx: &CanvasRenderingContext2d) {
        // render the app
        context.set_output("");

        // draw the cells of the map
        self.render_map(context, ctx);

        if let Some(goal) = self.goal {
            ctx.set_fill_style(&"#FF0000".into());
            ctx.fill_rect(goal.col as f64, goal.row as f64, 1.0, 1.0);
        }

        if let Some(start) = self.start {
            ctx.set_fill_style(&"#00FF00".into());
            ctx.fill_rect(start.col as f64, start.row as f64, 1.0, 1.0);
        }

        if let Some(state) = &self.find_state {
            let visited = state.pathfinder.get_visited();

            let margin = 0.15;
            for row in 0..self.map.rows {
                for col in 0..self.map.columns {
                    let p = Point { row, col };
                    let v = visited.get(p);

                    if let Some(f) = *v {
                        let color = format!("rgba({}, 0.0, 0.0, 0.8)", f.cost);
                        ctx.set_fill_style(&color.into());
                        ctx.fill_rect(
                            col as f64 + margin,
                            row as f64 + margin,
                            1.0 - 2.0 * margin,
                            1.0 - 2.0 * margin,
                        );
                    }
                }
            }

            match state.pathfinder.state() {
                PathFinderState::Computing => {}
                PathFinderState::NoPathFound => {
                    context.set_output("No path found");
                }
                PathFinderState::PathFound(pr) => {
                    ctx.set_stroke_style(&"#00FF00".into());
                    ctx.begin_path();
                    ctx.move_to(pr.start.col as f64 + 0.5, pr.start.row as f64 + 0.5);
                    for p in &pr.path {
                        ctx.line_to(p.col as f64 + 0.5, p.row as f64 + 0.5);
                    }

                    ctx.move_to(pr.goal.col as f64 + 0.5, pr.goal.row as f64 + 0.5);

                    ctx.stroke();
                }
            }

            // get the cell the user is hovering
            if let Some((x, y)) = context.input(|input| input.current_mouse_position()) {
                if let Some(point) = self.mouse_to_world_point_valid(x, y) {
                    ctx.set_fill_style(&"#00FF00".into());
                    ctx.fill_rect(point.col as f64, point.row as f64, 1.0, 1.0);

                    let v = visited.get(point);

                    context.set_output(&format!(
                        "Cell @{}:{}\n{:#?}\n\n{:#?}",
                        point.row, point.col, self.map.cells[point.row][point.col], v
                    ));
                }
            }
        }
    }
}

/// Fills a map based on the pixels on an image and a selected color for valid cells
fn fill_map_from_image(
    map: &mut Map,
    image: &DynamicImage,
    image_scale: f64,
    color: &image::Rgba<u8>,
) {
    for row in 0..map.rows {
        for col in 0..map.columns {
            // find the pixel at the center of the cell
            let (x, y) = (col as f64 + 0.5, row as f64 + 0.5);
            let (x, y) = (x / image_scale, y / image_scale);
            let (x, y) = (x as u32, y as u32);
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
