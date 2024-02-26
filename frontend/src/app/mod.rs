use crate::context::Context;
use crate::event::{
    ButtonId, CheckboxId, Event, InputChange, MouseButton, MouseEvent, NumberInputId,
};
use crate::App;

use log::debug;
use optimize::{parse_img, Cell, Map, MapTrait, PathFinder, Point, Visited};
use optimize::{MapStorage, PathFinderState};
use wasm_bindgen::Clamped;
use web_sys::ImageData;
use web_sys::{CanvasRenderingContext2d, ImageBitmap};

const STORAGE_KEY_MAP: &str = "map";

pub(crate) trait AppMapTrait:
    MapTrait + serde::Serialize + for<'de> serde::Deserialize<'de>
{
}
impl<T> AppMapTrait for T where T: MapTrait + serde::Serialize + for<'de> serde::Deserialize<'de> {}

pub struct AppImpl<M: AppMapTrait> {
    editing: bool,
    size: f64,
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
    pan_start: Option<(f64, f64)>,
    // offset and scale in unit coordinates
    offset: (f64, f64),
    scale: f64,
    mouse_select_state: Option<MouseSelectState<M>>,
    // background stuff
    background: Option<Background>,
    map_alpha: f64,
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
    image: ImageBitmap,
    scale: f64,
    alpha: f64,
}

impl AppImpl<Map> {
    pub fn new(context: &Context) -> Self {
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
            size: 10.0,
            find_state: None,
            start: None,
            goal: None,
            auto_step: true,
            edit_selection: None,
            selection_start: None,
            selection_end: None,
            pan_start: None,
            offset: (0.0, 0.0),
            scale: 1.0,
            mouse_select_state: None,
            background: None,
            map_alpha: 0.8,
        };
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
                }
            }
            Event::ButtonPressed(ButtonId::ToggleEdit) => self.set_editing(!self.editing, context),
            Event::InputChanged(InputChange::Checkbox {
                id: CheckboxId::AutoStep,
                value: checked,
            }) => self.auto_step = checked,
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

    /// converts a mouse position into a world position
    fn mouse_to_world(&self, x: i32, y: i32) -> (f64, f64) {
        let (x, y) = (x as f64, y as f64);
        let (x, y) = (x / self.size, y / self.size);
        let (x, y) = (x / self.scale, y / self.scale);
        let (x, y) = (x - self.offset.0, y - self.offset.1);
        (x, y)
    }

    fn mouse_to_world_point_valid(&self, x: i32, y: i32) -> Option<Point> {
        let (x, y) = self.mouse_to_world(x, y);

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
                // convert the mouse position to unit coordinates
                // let (x, y) = self.mouse_to_world(*x, *y);
                let (x, y) = (*x as f64 / self.size, *y as f64 / self.size);

                debug!("pan start: {:?}", (x, y));
                self.pan_start = Some((x, y));
                true
            }
            Event::MouseMove(MouseEvent {
                x,
                y,
                ctrl_pressed: true,
                ..
            }) => {
                if let Some(pan_start) = self.pan_start {
                    // let (x, y) = self.mouse_to_world(*x, *y);
                    let (x, y) = (*x as f64 / self.size, *y as f64 / self.size);

                    let (dx, dy) = (x - pan_start.0, y - pan_start.1);
                    self.offset.0 += dx / self.scale;
                    self.offset.1 += dy / self.scale;

                    self.pan_start = Some((x, y));
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
                if self.pan_start.is_some() {
                    self.pan_start = None;
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
                let (x, y) = self.mouse_to_world(*x, *y);

                let scale_factor = 1.02;
                let scale_factor = if *delta_y > 0.0 {
                    scale_factor
                } else {
                    1.0 / scale_factor
                };

                self.offset.0 = x + (self.offset.0 - x) * scale_factor;
                self.offset.1 = y + (self.offset.1 - y) * scale_factor;

                self.scale *= scale_factor;

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
        }
    }

    async fn handle_event_edit(&mut self, event: Event, context: &Context) {
        match event {
            Event::ButtonPressed(ButtonId::LoadPreset) => {
                // load the image by including the bytes in the compilation
                let image_bytes = include_bytes!("../../../data/maze-03_6_threshold.png");
                let image = image::load_from_memory(image_bytes).expect("could not load image");

                let rgba_image = image.to_rgba8();

                let clamped_buf: Clamped<&[u8]> = Clamped(rgba_image.as_raw());
                let image_data_temp = ImageData::new_with_u8_clamped_array_and_sh(
                    clamped_buf,
                    image.width(),
                    image.height(),
                )
                .unwrap();

                // TODO: we have to somehow get the value out of the Promise
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
                    image: jsimage,
                    scale: 1.0,
                    alpha: 0.8,
                });

                let map = parse_img(&image).unwrap();

                // let mut map = create_basic_map();
                // map.cells[3][2] = Cell::Cost(4);

                let start = Point { row: 14, col: 0 };
                let goal = Point { row: 44, col: 51 };

                let finder = PathFinder::new(start, goal, map.create_storage::<Visited<Point>>());

                self.map = map;
                self.goal = Some(goal);
                self.start = Some(start);

                self.find_state = Some(FindState { pathfinder: finder });

                self.on_map_change(context);
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
            _ => {}
        }
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
    }

    fn handle_event_path_find(&mut self, event: Event, context: &Context) {
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
        // let total_scale = self.scale * self.size;
        // ctx.scale(total_scale, total_scale).unwrap();
        ctx.set_line_width(1.0 / (self.scale * self.size));

        ctx.scale(self.size, self.size).unwrap();
        ctx.translate(self.offset.0 * self.scale, self.offset.1 * self.scale)
            .unwrap();
        ctx.scale(self.scale, self.scale).unwrap();

        // TODO: draw background image with alpha
        if let Some(background) = &self.background {
            ctx.set_global_alpha(background.alpha);
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
