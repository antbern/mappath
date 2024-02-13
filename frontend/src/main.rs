use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use app::AppImpl;
use context::{Context, ContextImpl, Input};
use event::ButtonId;
use optimize::{parse_img, Cell, Map, MapTrait, PathFinder, Point, Visited};
use wasm_bindgen::{prelude::*, Clamped};
use web_sys::{Document, HtmlCanvasElement, HtmlElement, ImageData};

use crate::{app::App, event::Event};

mod app;
mod context;
mod event;

fn register_onclick<T: FnMut() -> () + 'static>(document: &Document, id: &str, callback: T) {
    let closure_btn_clone = Closure::<dyn FnMut()>::new(callback);
    document
        .get_element_by_id(id)
        .expect("should have btn-reset on the page")
        .dyn_ref::<HtmlElement>()
        .expect("#btn-reset be an `HtmlElement`")
        .set_onclick(Some(closure_btn_clone.as_ref().unchecked_ref()));

    // See comments https://rustwasm.github.io/wasm-bindgen/examples/closures.html
    closure_btn_clone.forget();
}

fn regiester_mouse_event<T: FnMut(web_sys::MouseEvent) -> () + 'static>(
    canvas: &HtmlCanvasElement,
    event: &str,
    callback: T,
) {
    let closure = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(callback);

    canvas
        .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
        .unwrap();

    closure.forget();
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
fn window() -> web_sys::Window {
    web_sys::window().expect("no global `window` exists")
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    window()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
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

    let rendering_context = canvas
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
        output,
        input: Input::default(),
        events: VecDeque::new(),
        repaint_requested: false,
    });

    // create cells for storing the closure that redraws the canvas
    let redraw = Rc::new(RefCell::new(None));

    let request_repaint = {
        let redraw = redraw.clone();
        move || {
            request_animation_frame(redraw.borrow().as_ref().unwrap());
        }
    };

    // create input and register mouse event handlers
    {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        regiester_mouse_event(&canvas, "mouseenter", move |event: web_sys::MouseEvent| {
            context.push_event(Event::MouseEnter(event.offset_x(), event.offset_y()));
            request_repaint();
        });
    }
    {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        regiester_mouse_event(&canvas, "mousemove", move |event: web_sys::MouseEvent| {
            context.push_event(Event::MouseMove(event.offset_x(), event.offset_y()));
            request_repaint();
        });
    }
    {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        regiester_mouse_event(&canvas, "mouseleave", move |_event: web_sys::MouseEvent| {
            context.push_event(Event::MouseLeave);
            request_repaint();
        });
    }

    for (id, value) in vec![
        ("btn-reset", ButtonId::Reset),
        ("btn-step", ButtonId::Step),
        ("btn-finish", ButtonId::Finish),
    ] {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        register_onclick(&document, id, move || {
            context.push_event(Event::ButtonPressed(value));
            request_repaint();
        });
    }

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
        map.create_storage::<Visited<Point>>(),
    );
    // setup button callbacks with a shared state
    let mut state = AppImpl::new(finder, map);
    // register animation frame function
    //
    // create a closure that will be called by the browser's animation frame
    *redraw.borrow_mut() = Some(Closure::<dyn FnMut()>::new({
        let context = context.clone();
        let request_repaint = request_repaint.clone();

        move || {
            state.render(&context, &rendering_context);

            // if the app requested to be repainted, schedule another call
            if context.is_repaint_requested() {
                request_repaint();
            }
        }
    }));
    // initial call to the animation frame function
    request_repaint();

    Ok(())
}
