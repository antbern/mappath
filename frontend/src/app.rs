use crate::context::{Context};
use crate::event::{ButtonId, Event};

use optimize::{CellStorage, Map, MapStorage, PathFinder, PathFinderState, Point, Visited};
use web_sys::CanvasRenderingContext2d;

pub trait App {
    fn render(&mut self, ctx: &Context, rendering_ctx: &CanvasRenderingContext2d);
}

pub struct AppImpl<S: MapStorage<Visited<Point>, Reference = Point>> {
    // image_data: ImageData,
    // pathfinder: PathFinder<Point, CellStorage<Visited<Point>>>,
    pub pathfinder: PathFinder<Point, S>,
    pub map: Map,
}

impl<S: MapStorage<Visited<Point>, Reference = Point>> AppImpl<S> {
    pub fn new(pathfinder: PathFinder<Point, S>, map: Map) -> Self {
        Self { pathfinder, map }
    }
}

impl<S: MapStorage<Visited<Point>, Reference = Point>> App for AppImpl<S> {
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
                Event::MouseMove(_, _) | Event::MouseEnter(_, _) | Event::MouseLeave => {}
                _ => {}
            }
        }
        // render the app

        let canvas = ctx.canvas().unwrap();

        let size = 10.0;

        canvas.set_width((self.map.columns as f64 * size) as u32);
        canvas.set_height((self.map.rows as f64 * size) as u32);
        ctx.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);

        // draw the cells of the map

        // ctx.begin_path();

        let goal = self.pathfinder.goal();
        ctx.set_fill_style(&"00FF00".into());
        ctx.fill_rect(goal.col as f64 * size, goal.row as f64 * size, size, size);

        let start = self.pathfinder.start();
        ctx.set_fill_style(&"00FFFF".into());
        ctx.fill_rect(start.col as f64 * size, start.row as f64 * size, size, size);

        let visited = self
            .pathfinder
            .get_visited()
            .as_any()
            .downcast_ref::<CellStorage<Visited<Point>>>()
            .unwrap();

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

                ctx.fill_rect(col as f64 * size, row as f64 * size, size, size);
            }
        }

        match self.pathfinder.state() {
            PathFinderState::Computing => {}
            PathFinderState::NoPathFound => {}
            PathFinderState::PathFound(pr) => {
                ctx.set_stroke_style(&"#FFFFFF".into());
                ctx.begin_path();
                ctx.move_to(
                    start.col as f64 * size + size / 2.0,
                    start.row as f64 * size + size / 2.0,
                );
                for p in &pr.path {
                    ctx.line_to(
                        p.col as f64 * size + size / 2.0,
                        p.row as f64 * size + size / 2.0,
                    );
                }

                ctx.move_to(
                    goal.col as f64 * size + size / 2.0,
                    goal.row as f64 * size + size / 2.0,
                );

                ctx.stroke();
            }
        }

        // get the cell the user is hovering
        if let Some((x, y)) = context.input(|input| input.current_mouse_position()) {
            let row = (y as f64 / size) as usize;
            let col = (x as f64 / size) as usize;

            ctx.set_fill_style(&"#00FF00".into());
            ctx.fill_rect(col as f64 * size, row as f64 * size, size, size);

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
