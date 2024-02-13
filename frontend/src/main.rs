
use std::{rc::Rc, sync::Mutex};

use app::AppImpl;
use context::{Context, ContextImpl, Input};
use log::debug;
use optimize::{
    parse_img, Cell, Map, MapTrait, PathFinder, Point,
    Visited,
};
use wasm_bindgen::{prelude::*, Clamped};
use web_sys::{Document, HtmlElement, ImageData};

use crate::{app::App, event::Event};

mod app;
mod context;
mod event;

fn register_onclick<S: 'static, T: FnMut(&Rc<S>) -> () + 'static>(
    document: &Document,
    id: &str,
    mut callback: T,
    state: Rc<S>,
) {
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

fn main() -> Result<(), JsValue> {
    wasm_logger::init(wasm_logger::Config::default());
    console_error_panic_hook::set_once();

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

    let output = document.get_element_by_id("output").unwrap();
    let output: web_sys::HtmlPreElement = output
        .dyn_into::<web_sys::HtmlPreElement>()
        .map_err(|_| ())
        .unwrap();

    // setup the context for the app to interact with the world
    let context = Context::new(ContextImpl {
        rendering_context: context,
        canvas,
        output,
        input: Input::default(),
    });

    // load the image by including the bytes in the compilation
    let image_bytes = include_bytes!("../../data/maze-03_6_threshold.png");
    let image = image::load_from_memory(image_bytes).expect("could not load image");

    let rgba_image = image.to_rgba8();

    let clamped_buf: Clamped<&[u8]> = Clamped(rgba_image.as_raw());
    let _image_data_temp =
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
    let state = Rc::new(Mutex::new(AppImpl {
        pathfinder: finder,
        map: map,
    }));

    // create input and register mouse event handlers
    {
        let closure = {
            let context = context.clone();
            let state = state.clone();
            Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
                debug!("Moved: {}:{}", event.offset_x(), event.offset_y());

                context.input_mut(|input| {
                    input.on_mouse_enter(event.clone());
                });
                state.lock().unwrap().update(
                    Event::MouseEnter(event.offset_x(), event.offset_y()),
                    &context,
                );
            })
        };

        context.canvas(|canvas| {
            canvas
                .add_event_listener_with_callback("mouseenter", closure.as_ref().unchecked_ref())
                .unwrap();
        });
        closure.forget();
    }

    {
        let closure = {
            let context = context.clone();
            let state = state.clone();
            Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
                context.input_mut(|input| {
                    input.on_mouse_move(event.clone());
                });
                state.lock().unwrap().update(
                    Event::MouseMoved(event.offset_x(), event.offset_y()),
                    &context,
                );
            })
        };

        context.canvas(|canvas| {
            canvas
                .add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())
                .unwrap();
        });
        closure.forget();
    }

    {
        let closure = {
            let context = context.clone();
            let state = state.clone();
            Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
                context.input_mut(|input| {
                    input.on_mouse_leave(event.clone());
                });
                state.lock().unwrap().update(Event::MouseLeave, &context);
            })
        };
        context.canvas(|canvas| {
            canvas
                .add_event_listener_with_callback("mouseleave", closure.as_ref().unchecked_ref())
                .unwrap();
        });

        closure.forget();
    }

    let app_context = Rc::new((state, context));

    register_onclick(
        &document,
        "btn-reset",
        |state| {
            let (state, context) = state.as_ref();
            state
                .lock()
                .expect("Could not lock")
                .update(Event::ButtonPressed(event::ButtonId::Reset), &context);
        },
        app_context.clone(),
    );
    register_onclick(
        &document,
        "btn-step",
        |state| {
            let (state, context) = state.as_ref();
            state
                .lock()
                .expect("Could not lock")
                .update(Event::ButtonPressed(event::ButtonId::Step), &context);
        },
        app_context.clone(),
    );
    register_onclick(
        &document,
        "btn-finish",
        |state| {
            let (state, context) = state.as_ref();
            state
                .lock()
                .expect("Could not lock")
                .update(Event::ButtonPressed(event::ButtonId::Finish), &context);
        },
        app_context.clone(),
    );

    Ok(())
}
