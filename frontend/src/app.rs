use crate::context::Context;
use crate::event::{ButtonId, Event};

use optimize::{
    CellStorage, Map, MapStorage, PathFinder, PathFinderState, Point,
    Visited,
};

pub trait App {
    fn update(&mut self, event: Event, ctx: &Context);
    fn render(&self, ctx: &Context);
}

pub struct AppImpl<S: MapStorage<Visited<Point>, Reference = Point>> {
    // image_data: ImageData,
    // pathfinder: PathFinder<Point, CellStorage<Visited<Point>>>,
    pub pathfinder: PathFinder<Point, S>,
    pub map: Map,
}

impl<S: MapStorage<Visited<Point>, Reference = Point>> App for AppImpl<S> {
    fn update(&mut self, event: Event, ctx: &Context) {
        match event {
            Event::ButtonPressed(ButtonId::Reset) => {
                // self.pathfinder.reset();
                self.render(ctx);
            }
            Event::ButtonPressed(ButtonId::Step) => {
                self.pathfinder.step(&self.map);
                self.render(ctx);
            }
            Event::ButtonPressed(ButtonId::Finish) => {
                loop {
                    match self.pathfinder.step(&self.map) {
                        PathFinderState::Computing => {}
                        _s => break,
                    }
                }

                // self.pathfinder.finish();
                self.render(ctx);
            }
            Event::MouseMoved(_, _) | Event::MouseEnter(_, _) | Event::MouseLeave => {
                self.render(ctx)
            }
            _ => {}
        }
    }

    fn render(&self, context: &Context) {
        let mouse_position = context.input(|input| input.current_mouse_position());
        let mut output = String::new();
        context.draw(|ctx| {
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
            if let Some((x, y)) = mouse_position {
                let row = (y as f64 / size) as usize;
                let col = (x as f64 / size) as usize;

                ctx.set_fill_style(&"#00FF00".into());
                ctx.fill_rect(col as f64 * size, row as f64 * size, size, size);

                let v = visited.get(Point { row, col });

                output = format!(
                    "Cell @{row}:{col}\n{:?}\n\n{:?}",
                    self.map.cells[row][col], v
                );
            }
        });

        context.set_output(&output);
    }
}
