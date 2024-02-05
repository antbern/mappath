use core::f64;
use std::{rc::Rc, sync::Mutex};

use log::{debug, info};
use optimize::{
    parse_img, Cell, CellStorage, Map, MapStorage, MapTrait, PathFinder, PathFinderState, Point,
    Visited,
};
use wasm_bindgen::{prelude::*, Clamped};
use web_sys::{CanvasRenderingContext2d, Document, HtmlElement, ImageData};

fn register_onclick<S: 'static, T: FnMut(&Rc<S>) -> () + 'static>(
    document: &Document,
    id: &str,
    mut callback: T,
    state: &Rc<S>,
) {
    let state = state.clone();
    let closure_btn_clone = Closure::<dyn FnMut()>::new(move || {
        callback(&state);
    });

    document
        .get_element_by_id(id)
        .expect("should have btn-reset on the page")
        .dyn_ref::<HtmlElement>()
        .expect("#btn-reset be an `HtmlElement`")
        .set_onclick(Some(closure_btn_clone.as_ref().unchecked_ref()));

    // See comments https://rustwasm.github.io/wasm-bindgen/examples/closures.html
    closure_btn_clone.forget();
}

fn create_basic_map() -> Map {
    use Cell::*;
    Map {
        rows: 7,
        columns: 7,
        cells: vec![
            vec![
                Invalid, Invalid, Invalid, Invalid, Invalid, Invalid, Invalid,
            ],
            vec![Invalid, Valid, Invalid, Invalid, Invalid, Valid, Invalid],
            vec![Invalid, Valid, Invalid, Invalid, Invalid, Valid, Invalid],
            vec![Invalid, Valid, Invalid, Valid, Valid, Valid, Invalid],
            vec![Invalid, Valid, Invalid, Valid, Invalid, Invalid, Invalid],
            vec![Invalid, Valid, Valid, Valid, Valid, Valid, Valid],
            vec![
                Invalid, Invalid, Invalid, Invalid, Invalid, Invalid, Invalid,
            ],
        ],
    }
}

// #[derive(Debug)]
struct State<S: MapStorage<Visited<Point>, Reference = Point>> {
    canvas: CanvasRenderingContext2d,
    image_data: ImageData,
    // pathfinder: PathFinder<Point, CellStorage<Visited<Point>>>,
    pathfinder: PathFinder<Point, S>,
    map: Map,
}

impl<S: MapStorage<Visited<Point>, Reference = Point>> State<S> {
    fn on_reset(&mut self) -> Result<(), JsValue> {
        debug!("reset");

        // let storage = self.map.create_storage::<Visited<Point>>();

        // let finder = PathFinder::new(
        //     Point { row: 14, col: 0 },
        //     Point { row: 44, col: 51 },
        //     storage,
        // );

        // self.pathfinder = finder;
        Ok(())
    }
    fn on_step(&mut self) -> Result<(), JsValue> {
        debug!("step");

        // if let Some(finder) = &mut self.pathfinder {
        self.pathfinder.step(&self.map);
        // } else {
        // self.pathfinder = Some(finder);
        // }
        // let visited: &CellStorage<Visited<Point>> = storage.as_any().downcast_ref().unwrap();
        // let visited = visited.to_owned();

        self.draw()?;
        Ok(())
    }
    fn on_finish(&mut self) -> Result<(), JsValue> {
        debug!("finish");

        loop {
            match self.pathfinder.step(&self.map) {
                PathFinderState::Computing => {}
                s => break,
            }
        }

        // if let Some(finder) = self.pathfinder {
        // finder.finish(&self.map);
        // }

        self.draw()?;

        Ok(())
    }

    fn draw(&self) -> Result<(), JsValue> {
        let ctx = &self.canvas;
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
                    (optimize::Cell::Valid, Some(f)) => format!("rgb({}, 0.0, 0.0)", f.cost),
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

        Ok(())
    }
}

fn main() -> Result<(), JsValue> {
    wasm_logger::init(wasm_logger::Config::default());

    // Use `web_sys`'s global `window` function to get a handle on the global
    // window object.
    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");

    let canvas = document.get_element_by_id("canvas").unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| ())
        .unwrap();

    let context = canvas
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .unwrap();

    // load the image by including the bytes in the compilation
    let image_bytes = include_bytes!("../../data/maze-03_6_threshold.png");
    let image = image::load_from_memory(image_bytes).expect("could not load image");

    let rgba_image = image.to_rgba8();

    let clamped_buf: Clamped<&[u8]> = Clamped(rgba_image.as_raw());
    let image_data_temp =
        ImageData::new_with_u8_clamped_array_and_sh(clamped_buf, image.width(), image.height())?;

    let map = parse_img(&image).unwrap();

    // let mut map = create_basic_map();
    // map.cells[3][2] = Cell::Cost(4);

    let finder = PathFinder::new(
        Point { row: 14, col: 0 },
        Point { row: 44, col: 51 },
        // Point { row: 1, col: 1 },
        // Point { row: 1, col: 5 },
        map.create_storage::<Visited<Point>>(),
    );
    // setup button callbacks with a shared state
    let state = Rc::new(Mutex::new(State {
        canvas: context,
        image_data: image_data_temp,
        pathfinder: finder,
        map: map,
    }));
    register_onclick(
        &document,
        "btn-reset",
        |state: &Rc<Mutex<State<_>>>| state.lock().expect("Could not lock").on_reset().unwrap(),
        &state,
    );
    register_onclick(
        &document,
        "btn-step",
        |state: &Rc<Mutex<State<_>>>| state.lock().expect("Could not lock").on_step().unwrap(),
        &state,
    );
    register_onclick(
        &document,
        "btn-finish",
        |state: &Rc<Mutex<State<_>>>| state.lock().expect("Could not lock").on_finish().unwrap(),
        &state,
    );

    info!("Hello World!");
    debug!("This works!");

    Ok(())
}
