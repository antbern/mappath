use crate::context::Context;
use crate::event::{ButtonId, Event, MouseButton};
use crate::App;

use log::debug;
use optimize::{parse_img, Map, MapTrait, PathFinder, Point, Visited};
use optimize::{MapStorage, PathFinderState};
use wasm_bindgen::Clamped;
use web_sys::CanvasRenderingContext2d;
use web_sys::ImageData;

pub struct AppImpl<M: MapTrait> {
    mode: Mode,
    pathfinder: PathFinder<M::Reference, M::Storage<Visited<M::Reference>>, M>,
    map: M,
    start: M::Reference,
    goal: M::Reference,
    size: f64,
}

enum Mode {
    Setup,
    Edit,
    PathFind,
}

impl AppImpl<Map> {
    pub fn new(context: &Context) -> Self {
        // load the image by including the bytes in the compilation
        let image_bytes = include_bytes!("../../../data/maze-03_6_threshold.png");
        let image = image::load_from_memory(image_bytes).expect("could not load image");

        let rgba_image = image.to_rgba8();

        let clamped_buf: Clamped<&[u8]> = Clamped(rgba_image.as_raw());
        let _image_data_temp =
            ImageData::new_with_u8_clamped_array_and_sh(clamped_buf, image.width(), image.height())
                .unwrap();

        let map = parse_img(&image).unwrap();

        // let mut map = create_basic_map();
        // map.cells[3][2] = Cell::Cost(4);

        let start = Point { row: 14, col: 0 };
        let goal = Point { row: 44, col: 51 };

        let finder = PathFinder::new(start, goal, map.create_storage::<Visited<Point>>());

        let mut s = Self {
            mode: Mode::Setup,
            pathfinder: finder,
            map,
            start,
            goal,
            size: 10.0,
        };
        s.enter_mode(Mode::Setup, context);
        s
    }
}

impl App for AppImpl<Map> {
    fn render(&mut self, context: &Context, ctx: &CanvasRenderingContext2d) {
        // handle any pending events
        while let Some(event) = context.pop_event() {
            self.handle_event(event, context);
        }

        self.render_app(context, ctx);
    }
}
impl AppImpl<Map> {
    fn handle_event(&mut self, event: Event, context: &Context) {
        // switch mode if the mode buttons were pressed
        match event {
            Event::ButtonPressed(ButtonId::ModeSetup) => self.enter_mode(Mode::Setup, context),
            Event::ButtonPressed(ButtonId::ModeEdit) => self.enter_mode(Mode::Edit, context),
            Event::ButtonPressed(ButtonId::ModePathFind) => {
                self.enter_mode(Mode::PathFind, context)
            }
            _ => {}
        }
        // handle the event depending on the current mode
        match self.mode {
            Mode::Setup => self.handle_event_setup(event, context),
            Mode::Edit => self.handle_event_edit(event, context),
            Mode::PathFind => self.handle_event_path_find(event, context),
        }
    }

    fn enter_mode(&mut self, mode: Mode, context: &Context) {
        self.mode = mode;
        // enable the right buttons for the specific modes
        match self.mode {
            Mode::Setup => {
                context.enable_button(ButtonId::ModeSetup, false);
                context.enable_button(ButtonId::ModeEdit, true);
                context.enable_button(ButtonId::ModePathFind, true);
                context.show_div("mode-setup-inputs", true);
                context.show_div("mode-edit-inputs", false);
                context.show_div("mode-find-inputs", false);
            }
            Mode::Edit => {
                context.enable_button(ButtonId::ModeSetup, true);
                context.enable_button(ButtonId::ModeEdit, false);
                context.enable_button(ButtonId::ModePathFind, true);
                context.show_div("mode-setup-inputs", false);
                context.show_div("mode-edit-inputs", true);
                context.show_div("mode-find-inputs", false);
            }
            Mode::PathFind => {
                context.enable_button(ButtonId::ModeSetup, true);
                context.enable_button(ButtonId::ModeEdit, true);
                context.enable_button(ButtonId::ModePathFind, false);
                context.show_div("mode-setup-inputs", false);
                context.show_div("mode-edit-inputs", false);
                context.show_div("mode-find-inputs", true);
            }
        }
    }

    fn handle_event_setup(&mut self, event: Event, context: &Context) {}

    fn handle_event_edit(&mut self, event: Event, context: &Context) {}

    fn handle_event_path_find(&mut self, event: Event, context: &Context) {
        match event {
            Event::ButtonPressed(ButtonId::Reset) => {
                self.pathfinder = PathFinder::new(
                    self.start,
                    self.goal,
                    self.map.create_storage::<Visited<Point>>(),
                );
            }
            Event::ButtonPressed(ButtonId::Step) => {
                self.pathfinder.step(&self.map);
            }
            Event::ButtonPressed(ButtonId::Finish) => loop {
                match self.pathfinder.step(&self.map) {
                    PathFinderState::Computing => {}
                    _s => break,
                }
            },

            Event::MouseReleased { x, y, button } => {
                let row = (y as f64 / self.size) as usize;
                let col = (x as f64 / self.size) as usize;

                match button {
                    MouseButton::Main => self.start = Point { row, col },
                    MouseButton::Secondary => self.goal = Point { row, col },
                    _ => {}
                }
                debug!("{:?} -> {:?}", self.start, self.goal);
                self.pathfinder = PathFinder::new(
                    self.start,
                    self.goal,
                    self.map.create_storage::<Visited<Point>>(),
                );
            }
            _ => {}
        }
    }

    fn render_app(&self, context: &Context, ctx: &CanvasRenderingContext2d) {
        // render based on the current mode
        match self.mode {
            Mode::Setup => self.render_app_setup(context, ctx),
            Mode::Edit => self.render_app_edit(context, ctx),
            Mode::PathFind => self.render_app_find(context, ctx),
        }
    }

    fn render_app_setup(&self, context: &Context, ctx: &CanvasRenderingContext2d) {}

    fn render_app_edit(&self, context: &Context, ctx: &CanvasRenderingContext2d) {}

    fn render_app_find(&self, context: &Context, ctx: &CanvasRenderingContext2d) {
        // render the app
        context.set_output("");

        let canvas = ctx.canvas().unwrap();

        let size = self.size;

        canvas.set_width((self.map.columns as f64 * size) as u32);
        canvas.set_height((self.map.rows as f64 * size) as u32);
        ctx.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
        ctx.scale(size, size).unwrap();

        // draw the cells of the map

        let visited = self.pathfinder.get_visited();

        for row in 0..self.map.rows {
            for col in 0..self.map.columns {
                let cell = self.map.cells[row][col];

                let r = Point { row, col };

                let v = visited.get(r);

                let color = match (cell, *v) {
                    (optimize::Cell::Invalid, _) => "#000000".into(),
                    (optimize::Cell::Valid, Some(f)) => {
                        format!("rgb({}, 0.0, 0.0)", f.cost)
                    }
                    (optimize::Cell::Cost(_), Some(_)) => "#FFFF00".into(),
                    (optimize::Cell::Valid, _) => "#FFFFFF".into(),
                    (optimize::Cell::Cost(_), _) => "#FF0000".into(),
                };

                ctx.set_fill_style(&color.into());

                ctx.fill_rect(col as f64, row as f64, 1.0, 1.0);
            }
        }

        ctx.set_fill_style(&"#00FF00".into());
        ctx.fill_rect(self.goal.col as f64, self.goal.row as f64, 1.0, 1.0);

        ctx.set_fill_style(&"#0000FF".into());
        ctx.fill_rect(self.start.col as f64, self.start.row as f64, 1.0, 1.0);

        match self.pathfinder.state() {
            PathFinderState::Computing => {}
            PathFinderState::NoPathFound => {
                context.set_output("No path found");
            }
            PathFinderState::PathFound(pr) => {
                ctx.set_stroke_style(&"#FFFFFF".into());
                ctx.set_line_width(1.0 / size);
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
            let row = (y as f64 / size) as usize;
            let col = (x as f64 / size) as usize;

            ctx.set_fill_style(&"#00FF00".into());
            ctx.fill_rect(col as f64, row as f64, 1.0, 1.0);

            let v = visited.get(Point { row, col });

            context.set_output(&format!(
                "Cell @{row}:{col}\n{:?}\n\n{:?}",
                self.map.cells[row][col], v
            ));
        }
    }
}
