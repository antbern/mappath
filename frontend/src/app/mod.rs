use crate::context::Context;
use crate::event::{ButtonId, CheckboxId, Event, MouseButton};
use crate::App;

use log::debug;
use optimize::{parse_img, Cell, Map, MapTrait, PathFinder, Point, Visited};
use optimize::{MapStorage, PathFinderState};
use wasm_bindgen::Clamped;
use web_sys::CanvasRenderingContext2d;
use web_sys::ImageData;

const STORAGE_KEY_MAP: &str = "map";

pub struct AppImpl<M: MapTrait + serde::Serialize + for<'de> serde::Deserialize<'de>> {
    editing: bool,
    rows: usize,
    cols: usize,
    size: f64,
    map: M,

    find_state: Option<FindState<M>>,
    start: Option<M::Reference>,
    goal: Option<M::Reference>,
    auto_step: bool,
    edit_selection: Option<Selection<M::Reference>>,
}

enum Selection<R> {
    Single(R),
    // Rectangle(R, R),
}

struct FindState<M: MapTrait> {
    pathfinder: PathFinder<M::Reference, M::Storage<Visited<M::Reference>>, M>,
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
            rows: map.rows,
            cols: map.columns,
            map,
            size: 10.0,
            find_state: None,
            start: None,
            goal: None,
            auto_step: true,
            edit_selection: None,
        };
        s.set_editing(false, context);
        s
    }
}

impl App for AppImpl<Map> {
    fn render(&mut self, context: &Context, ctx: &CanvasRenderingContext2d) {
        // handle any pending events
        while let Some(event) = context.pop_event() {
            // TODO: give the event to panning and zooming first, and if it was not handled, give it to the app

            self.handle_event(event, context);
        }

        self.render_app(context, ctx);
    }
}
impl AppImpl<Map> {
    fn handle_event(&mut self, event: Event, context: &Context) {
        // switch mode if the mode buttons were pressed
        match event {
            Event::ButtonPressed(ButtonId::ClearStorage) => {
                if gloo::dialogs::confirm("Are you sure you want to clear the storage?") {
                    context.remove_storage(STORAGE_KEY_MAP);
                }
            }
            Event::ButtonPressed(ButtonId::ToggleEdit) => self.set_editing(!self.editing, context),
            Event::CheckboxChanged(CheckboxId::AutoStep, checked) => self.auto_step = checked,
            _ => {}
        }
        // handle the event depending on the current mode
        if self.editing {
            self.handle_event_edit(event, context);
        } else {
            self.handle_event_path_find(event, context);
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

    fn handle_event_edit(&mut self, event: Event, context: &Context) {
        match event {
            Event::ButtonPressed(ButtonId::LoadPreset) => {
                // load the image by including the bytes in the compilation
                let image_bytes = include_bytes!("../../../data/maze-03_6_threshold.png");
                let image = image::load_from_memory(image_bytes).expect("could not load image");

                let rgba_image = image.to_rgba8();

                let clamped_buf: Clamped<&[u8]> = Clamped(rgba_image.as_raw());
                let _image_data_temp = ImageData::new_with_u8_clamped_array_and_sh(
                    clamped_buf,
                    image.width(),
                    image.height(),
                )
                .unwrap();

                let map = parse_img(&image).unwrap();

                // let mut map = create_basic_map();
                // map.cells[3][2] = Cell::Cost(4);

                let start = Point { row: 14, col: 0 };
                let goal = Point { row: 44, col: 51 };

                let finder = PathFinder::new(start, goal, map.create_storage::<Visited<Point>>());

                self.rows = map.rows;
                self.cols = map.columns;
                self.map = map;
                self.goal = Some(goal);
                self.start = Some(start);

                self.find_state = Some(FindState { pathfinder: finder });
            }
            Event::MouseClicked {
                x,
                y,
                button: MouseButton::Main,
            } => {
                let row = (y as f64 / self.size) as usize;
                let col = (x as f64 / self.size) as usize;
                let point = Point { row, col };

                if self.map.is_valid(point) {
                    self.edit_selection = Some(Selection::Single(point));
                }
            }
            _ => {}
        }
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

            Event::MouseReleased { x, y, button } => {
                let row = (y as f64 / self.size) as usize;
                let col = (x as f64 / self.size) as usize;

                match button {
                    MouseButton::Main => self.start = Some(Point { row, col }),
                    MouseButton::Secondary => self.goal = Some(Point { row, col }),
                    _ => {}
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
            _ => {}
        }
    }

    fn render_app(&mut self, context: &Context, ctx: &CanvasRenderingContext2d) {
        let canvas = ctx.canvas().unwrap();
        ctx.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
        ctx.save();
        ctx.scale(self.size, self.size).unwrap();

        // TODO: implement panning and zooming to translate and scale the map further

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
    }

    fn render_map(&self, _context: &Context, ctx: &CanvasRenderingContext2d) {
        for row in 0..self.map.rows {
            for col in 0..self.map.columns {
                let cell = self.map.cells[row][col];

                let color: String = match cell {
                    Cell::Invalid => "#000000".into(),
                    Cell::Valid => "#FFFFFF".into(),
                    Cell::Cost(_) => "#FFFF00".into(),
                };

                ctx.set_fill_style(&color.into());
                ctx.fill_rect(col as f64, row as f64, 1.0, 1.0);
            }
        }

        // draw lines between all the cells
        ctx.set_stroke_style(&"#000000".into());
        ctx.set_line_width(1.0 / self.size);
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
        ctx.set_line_width(1.0 / self.size);
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
            let row = (y as f64 / self.size) as usize;
            let col = (x as f64 / self.size) as usize;

            let point = Point { row, col };
            self.draw_neighbors(&point, ctx, "#FF0000")
        }

        if let Some(selection) = &self.edit_selection {
            match selection {
                Selection::Single(point) => self.draw_neighbors(&point, ctx, "#00FF00"),
            }
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
                    ctx.set_line_width(1.0 / self.size);
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
                let row = (y as f64 / self.size) as usize;
                let col = (x as f64 / self.size) as usize;

                ctx.set_fill_style(&"#00FF00".into());
                ctx.fill_rect(col as f64, row as f64, 1.0, 1.0);

                let p = Point { row, col };
                if visited.is_valid(p) {
                    let v = visited.get(p);

                    context.set_output(&format!(
                        "Cell @{row}:{col}\n{:#?}\n\n{:#?}",
                        self.map.cells[row][col], v
                    ));
                }
            }
        }
    }
}
