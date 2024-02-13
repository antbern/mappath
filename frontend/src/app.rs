use crate::context::Context;
use crate::event::{ButtonId, Event};
use crate::App;

use optimize::{parse_img, Map, MapTrait, PathFinder, Point, Visited};
use optimize::{MapStorage, PathFinderState};
use wasm_bindgen::Clamped;
use web_sys::CanvasRenderingContext2d;
use web_sys::ImageData;

pub struct AppImpl<M: MapTrait> {
    pathfinder: PathFinder<M::Reference, M::Storage<Visited<M::Reference>>, M>,
    map: M,
}

impl AppImpl<Map> {
    pub fn new(_context: &Context) -> Self {
        // load the image by including the bytes in the compilation
        let image_bytes = include_bytes!("../../data/maze-03_6_threshold.png");
        let image = image::load_from_memory(image_bytes).expect("could not load image");

        let rgba_image = image.to_rgba8();

        let clamped_buf: Clamped<&[u8]> = Clamped(rgba_image.as_raw());
        let _image_data_temp =
            ImageData::new_with_u8_clamped_array_and_sh(clamped_buf, image.width(), image.height())
                .unwrap();

        let map = parse_img(&image).unwrap();

        // let mut map = create_basic_map();
        // map.cells[3][2] = Cell::Cost(4);

        let finder = PathFinder::new(
            Point { row: 14, col: 0 },
            Point { row: 44, col: 51 },
            map.create_storage::<Visited<Point>>(),
        );

        Self {
            pathfinder: finder,
            map,
        }
    }
}

impl App for AppImpl<Map> {
    fn render(&mut self, context: &Context, ctx: &CanvasRenderingContext2d) {
        // handle any pending events
        while let Some(event) = context.pop_event() {
            match event {
                Event::ButtonPressed(ButtonId::Reset) => {}
                Event::ButtonPressed(ButtonId::Step) => {
                    self.pathfinder.step(&self.map);
                }
                Event::ButtonPressed(ButtonId::Finish) => loop {
                    match self.pathfinder.step(&self.map) {
                        PathFinderState::Computing => {}
                        _s => break,
                    }
                },
                _ => {}
            }
        }
        // render the app

        let canvas = ctx.canvas().unwrap();

        let size = 10.0;

        canvas.set_width((self.map.columns as f64 * size) as u32);
        canvas.set_height((self.map.rows as f64 * size) as u32);
        ctx.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
        ctx.scale(size, size).unwrap();

        // draw the cells of the map

        // ctx.begin_path();

        let goal = self.pathfinder.goal();
        ctx.set_fill_style(&"00FF00".into());
        ctx.fill_rect(goal.col as f64, goal.row as f64, 1.0, 1.0);

        let start = self.pathfinder.start();
        ctx.set_fill_style(&"00FFFF".into());
        ctx.fill_rect(start.col as f64, start.row as f64, 1.0, 1.0);

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

        match self.pathfinder.state() {
            PathFinderState::Computing => {}
            PathFinderState::NoPathFound => {}
            PathFinderState::PathFound(pr) => {
                ctx.set_stroke_style(&"#FFFFFF".into());
                ctx.set_line_width(1.0 / size);
                ctx.begin_path();
                ctx.move_to(start.col as f64 + 0.5, start.row as f64 + 0.5);
                for p in &pr.path {
                    ctx.line_to(p.col as f64 + 0.5, p.row as f64 + 0.5);
                }

                ctx.move_to(goal.col as f64 + 0.5, goal.row as f64 + 0.5);

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
        } else {
            context.set_output("");
        }
    }
}
