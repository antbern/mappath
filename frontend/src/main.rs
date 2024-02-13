use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use app::AppImpl;
use context::{Context, ContextImpl, Input};
use event::ButtonId;
use optimize::{Cell, Map};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, Document, HtmlCanvasElement, HtmlElement};

use crate::event::Event;

mod app;
mod context;
mod event;

/// The main entry point for the application
pub trait App {
    fn render(&mut self, ctx: &Context, rendering_ctx: &CanvasRenderingContext2d);
}

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

    // register animation frame function
    //
    // create a closure that will be called by the browser's animation frame
    *redraw.borrow_mut() = Some(Closure::<dyn FnMut()>::new({
        let context = context.clone();
        let request_repaint = request_repaint.clone();

        // initialize the app
        let mut app = AppImpl::new(&context);

        move || {
            app.render(&context, &rendering_context);

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
