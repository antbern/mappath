use core::f64;

use log::{debug, info};
use wasm_bindgen::{prelude::*, Clamped};
use web_sys::ImageData;

fn main() -> Result<(), JsValue> {
    wasm_logger::init(wasm_logger::Config::default());

    // Use `web_sys`'s global `window` function to get a handle on the global
    // window object.
    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    // let body = document.body().expect("document should have a body");

    // setup button callbacks

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

    context.begin_path();

    // Draw the outer circle.
    context
        .arc(75.0, 75.0, 50.0, 0.0, f64::consts::PI * 2.0)
        .unwrap();

    // Draw the mouth.
    context.move_to(110.0, 75.0);
    context.arc(75.0, 75.0, 35.0, 0.0, f64::consts::PI).unwrap();

    // Draw the left eye.
    context.move_to(65.0, 65.0);
    context
        .arc(60.0, 65.0, 5.0, 0.0, f64::consts::PI * 2.0)
        .unwrap();

    // Draw the right eye.
    context.move_to(95.0, 65.0);
    context
        .arc(90.0, 65.0, 5.0, 0.0, f64::consts::PI * 2.0)
        .unwrap();

    context.stroke();

    // load the image by including the bytes in the compilation
    let image_bytes = include_bytes!("../../data/maze-03_6_threshold.png");
    let image = image::load_from_memory(image_bytes).expect("could not load image");

    let rgba_image = image.to_rgba8();

    let clamped_buf: Clamped<&[u8]> = Clamped(rgba_image.as_raw());
    let image_data_temp =
        ImageData::new_with_u8_clamped_array_and_sh(clamped_buf, image.width(), image.height())?;

    context.put_image_data(&image_data_temp, 0.0, 0.0)?;

    info!("Hello World!");
    debug!("This works!");

    Ok(())
}
