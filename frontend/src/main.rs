use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use app::AppImpl;
use context::{CellSelector, Context, ContextImpl, Input};
use event::{ButtonId, InputChange, InputId};
use log::debug;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, Document, HtmlCanvasElement, HtmlElement};

use crate::event::Event;

mod app;
mod context;
mod event;

/// The main entry point for the application
pub trait App {
    #[allow(async_fn_in_trait)]
    async fn render(&mut self, ctx: &Context, rendering_ctx: &CanvasRenderingContext2d);
}

fn register_onclick<T: FnMut() -> () + 'static>(id: &str, callback: T) {
    let closure_btn_clone = Closure::<dyn FnMut()>::new(callback);
    get_element_by_id::<HtmlElement>(id)
        .set_onclick(Some(closure_btn_clone.as_ref().unchecked_ref()));

    // See comments https://rustwasm.github.io/wasm-bindgen/examples/closures.html
    closure_btn_clone.forget();
}

/// register a change event on an element (e.g. any input element)
fn register_change_event<E: JsCast, T: FnMut(&E) -> () + 'static>(id: &str, mut callback: T) {
    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let element = event.current_target().unwrap().dyn_into::<E>().unwrap();
        callback(&element);
    });
    get_element_by_id::<web_sys::EventTarget>(id)
        .add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())
        .unwrap();

    closure.forget();
}
fn register_canvas_event<T: FnMut(web_sys::MouseEvent) -> () + 'static>(
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
fn register_canvas_scroll<T: FnMut(web_sys::WheelEvent) -> () + 'static>(
    canvas: &HtmlCanvasElement,
    callback: T,
) {
    let closure = Closure::<dyn FnMut(web_sys::WheelEvent)>::new(callback);

    canvas
        .add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())
        .unwrap();

    closure.forget();
}

fn window() -> web_sys::Window {
    web_sys::window().expect("no global `window` exists")
}

fn document() -> Document {
    window()
        .document()
        .expect("should have a document on window")
}

fn get_element_by_id<T: JsCast>(id: &str) -> T {
    document()
        .get_element_by_id(id)
        .expect(&format!("should have {} on the page", id))
        .dyn_into::<T>()
        .expect(&format!(
            "{} should be an `{}`",
            id,
            std::any::type_name::<T>()
        ))
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    console_error_panic_hook::set_once();

    // if we are in CI, set the hash and the url
    if let Some(hash) = option_env!("GITHUB_SHA") {
        let url = format!("https://github.com/antbern/mappath/tree/{hash}");

        document()
            .get_element_by_id("github-link")
            .unwrap()
            .set_attribute("href", &url)
            .unwrap();
        document()
            .get_element_by_id("commit-hash")
            .unwrap()
            .set_inner_html(&hash[0..7]);
    }

    // setup an event handler for window.onresize and scale the canvas based on it's
    // clientWidth/clientHeight
    {
        let closure = move || {
            let canvas = get_element_by_id::<HtmlCanvasElement>("canvas");
            let width = canvas.client_width();
            let height = canvas.client_height();
            canvas.set_width(width as u32);
            canvas.set_height(height as u32);
            debug!("resized canvas to {}x{}", width, height);
        };
        // call it once to set the initial size
        closure();

        // then hand it over to the event handler
        let closure = Closure::<dyn FnMut()>::new(closure);
        window().set_onresize(Some(closure.as_ref().unchecked_ref()));
        closure.forget();
    }

    let canvas = get_element_by_id::<HtmlCanvasElement>("canvas");
    let rendering_context = canvas
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .unwrap();

    let output = get_element_by_id::<web_sys::HtmlPreElement>("output");

    // setup the context for the app to interact with the world
    let context = Context::new(ContextImpl {
        document: document(),
        cell_selector: CellSelector {
            radio_invalid: get_element_by_id("cell-invalid"),
            radio_valid: get_element_by_id("cell-normal"),
            input_valid_cost: get_element_by_id("input-normal-cost"),
            radio_oneway: get_element_by_id("cell-oneway"),
            select_oneway: get_element_by_id("select-oneway"),
            span_oneway_target: get_element_by_id("span-oneway-target"),
        },
        output,
        input: Input::default(),
        events: VecDeque::new(),
        repaint_requested: false,
    });

    // create cells for storing the closure that redraws the canvas
    let redraw: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));

    let request_repaint = {
        let redraw = redraw.clone();
        move || {
            debug!("request_repaint called, registering animation frame");
            window()
                .request_animation_frame(
                    redraw
                        .borrow()
                        .as_ref()
                        .expect("should convert ok")
                        .as_ref()
                        .unchecked_ref(),
                )
                .expect("should register `requestAnimationFrame` OK");

            // request_animation_frame(redraw.borrow().as_ref().unwrap());
        }
    };

    // create input and register mouse event handlers
    {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        register_canvas_event(&canvas, "mouseenter", move |event: web_sys::MouseEvent| {
            context.push_event(Event::MouseEnter(event.into()));
            request_repaint();
        });
    }
    {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        register_canvas_event(&canvas, "mousemove", move |event: web_sys::MouseEvent| {
            context.push_event(Event::MouseMove(event.into()));
            request_repaint();
        });
    }
    {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        register_canvas_event(&canvas, "mouseleave", move |event: web_sys::MouseEvent| {
            context.push_event(Event::MouseLeave(event.into()));
            request_repaint();
        });
    }
    {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        register_canvas_event(&canvas, "mousedown", move |event: web_sys::MouseEvent| {
            if let Some(_button) = event::MouseButton::from_web_button(event.button()) {
                context.push_event(Event::MousePressed(event.into()));
                request_repaint();
            }
        });
    }
    {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        register_canvas_event(&canvas, "mouseup", move |event: web_sys::MouseEvent| {
            if let Some(_button) = event::MouseButton::from_web_button(event.button()) {
                context.push_event(Event::MouseReleased(event.into()));
                request_repaint();
            }
        });
    }
    {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        register_canvas_event(&canvas, "click", move |event: web_sys::MouseEvent| {
            if let Some(_button) = event::MouseButton::from_web_button(event.button()) {
                context.push_event(Event::MouseClicked(event.into()));
                request_repaint();
            }
        });
    }
    {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        register_canvas_scroll(&canvas, move |event: web_sys::WheelEvent| {
            context.push_event(Event::MouseWheel {
                x: event.offset_x(),
                y: event.offset_y(),
                delta_x: event.delta_x(),
                delta_y: event.delta_y(),
            });
            event.prevent_default();
            request_repaint();
        });
    }

    for button in ButtonId::iterate() {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        register_onclick(button.id_str(), move || {
            context.push_event(Event::ButtonPressed(button));
            request_repaint();
        });
    }
    // setup change events for all inputs
    {
        for input in InputId::iterate() {
            match input {
                InputId::Select(id) => {
                    let context = context.clone();
                    let request_repaint = request_repaint.clone();

                    register_change_event(
                        id.id_str(),
                        move |select: &web_sys::HtmlSelectElement| {
                            context.push_event(Event::InputChanged(InputChange::Select {
                                id,
                                value: select.value(),
                            }));
                            request_repaint();
                        },
                    );
                }
                _ => {
                    let context = context.clone();
                    let request_repaint = request_repaint.clone();

                    register_change_event(
                        input.id_str(),
                        move |event: &web_sys::HtmlInputElement| {
                            context.push_event(Event::InputChanged(match input {
                                InputId::Number(id) => InputChange::Number {
                                    id,
                                    value: event.value().parse().unwrap(),
                                },
                                InputId::Checkbox(id) => InputChange::Checkbox {
                                    id,
                                    value: event.checked(),
                                },
                                InputId::Select(_) => unreachable!(),
                            }));
                            request_repaint();
                        },
                    );
                }
            }
        }
    }
    // setup key press handler for button shortcuts
    {
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        let closure = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
            move |event: web_sys::KeyboardEvent| {
                if let Some(button) = event::ButtonId::from_key_code(&event.key()) {
                    context.push_event(Event::ButtonPressed(button));
                    request_repaint();
                }
            },
        );
        window()
            .add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }
    // {
    //     let context = context.clone();
    //     let request_repaint = request_repaint.clone();
    //     register_change_event(
    //         "select-mode-radio",
    //         move |select: &web_sys::HtmlSelectElement| {
    //             context.push_event(Event::SelectChanged(SelectId::Mode, select.value()));
    //             request_repaint();
    //         },
    //     );
    // }
    //

    // initialize the app and put it in a refcell
    let app = Rc::new(RefCell::new(None));

    // spawn the constructor to setup the app
    wasm_bindgen_futures::spawn_local({
        let app = app.clone();
        let context = context.clone();
        async move {
            *app.borrow_mut() = Some(AppImpl::new(&context).await);
        }
    });

    // create a closure that will be called by the browser's animation frame
    *redraw.borrow_mut() = Some(Closure::<dyn FnMut()>::new({
        let context = context.clone();
        let request_repaint = request_repaint.clone();
        let rendering_context = Rc::new(rendering_context);

        move || {
            // we need to clone everything so that the block sent to spawn_local is 'static
            let context = context.clone();
            let request_repaint = request_repaint.clone();
            let rendering_context = rendering_context.clone();
            let app = app.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(app) = app.borrow_mut().as_mut() {
                    debug!("redraw");

                    app.render(&context, &rendering_context).await;

                    // if the app requested to be repainted, schedule another call
                    if context.is_repaint_requested() {
                        debug!("repaint requested");
                        request_repaint();
                    }
                }
            });
        }
    }));
    // initial call to the animation frame function
    request_repaint();
}
