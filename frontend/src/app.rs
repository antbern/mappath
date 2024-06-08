use std::sync::Arc;

use eframe::{egui_glow, glow};
use egui::{mutex::Mutex, Pos2, Vec2};
use graphics::{camera::Camera, primitiverenderer::Color, shaperenderer::ShapeRenderer};
use nalgebra::{Matrix4, Point2};

pub struct App {
    /// Behind an `Arc<Mutex<…>>` so we can pass it to [`egui::PaintCallback`] and paint later.
    world_renderer: Arc<Mutex<WorldRenderer>>,

    state: State,
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
struct State {
    label: String,
    value: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            value: 0.0,
            label: "Hello, world!".to_owned(),
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let state: State = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        let gl = cc
            .gl
            .as_ref()
            .expect("You need to run eframe with the glow backend");
        App {
            state,
            world_renderer: Arc::new(Mutex::new(WorldRenderer::new(gl))),
        }
    }
}

impl eframe::App for App {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::menu::bar(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_dark_light_mode_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("eframe template");

            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(&mut self.state.label);
            });

            ui.add(egui::Slider::new(&mut self.state.value, 0.0..=10.0).text("value"));
            if ui.button("Increment").clicked() {
                self.state.value += 1.0;
            }

            ui.separator();

            ui.add(egui::github_link_file!(
                "https://github.com/emilk/eframe_template/blob/main/",
                "Source code."
            ));

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's

            // Let all nodes do their drawing. Explicit scope for MutexGuard lifetime.
            {
                let mut world = self.world_renderer.lock();

                world
                    .sr
                    .begin(graphics::primitiverenderer::PrimitiveType::Line);
                world.sr.line(0.0, 0.0, 1.0, 1.0, Color::RED);
                world.sr.end();
            }

            self.custom_painting(ui);
        });
    }
    fn on_exit(&mut self, gl: Option<&glow::Context>) {
        if let Some(gl) = gl {
            self.world_renderer.lock().destroy(gl);
        }
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}
impl App {
    fn custom_painting(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(
            ui.available_size(), //egui::Vec2::splat(300.0)
            egui::Sense::drag(),
        );

        let zoom_factor = if ui.rect_contains_pointer(rect) {
            // combine the zoom_delta and the scroll amount to support multitouch gestures as well as normal scroll zoom

            let (scroll_delta, zoom_delta) = ui
                .ctx()
                .input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));

            1.0 / (zoom_delta + 0.1 * scroll_delta / 50.0)
        } else {
            1.0
        };

        let pos = if ui.rect_contains_pointer(rect) {
            let mut pos = ui.ctx().pointer_hover_pos().unwrap_or_default();
            // adjust for the position of the allocated space
            pos.x -= rect.left();
            pos.y -= rect.top();
            Some(pos)
        } else {
            None
        };

        // Clone locals so we can move them into the paint callback:

        let mut drag_delta = response.drag_delta();
        drag_delta.y *= -1.0;

        let size = rect.size();
        let world_renderer = self.world_renderer.clone();

        let callback = egui::PaintCallback {
            rect,
            callback: std::sync::Arc::new(egui_glow::CallbackFn::new(move |_info, painter| {
                world_renderer
                    .lock()
                    .paint(painter.gl(), pos, size, drag_delta, zoom_factor);
            })),
        };
        ui.painter().add(callback);
    }
}

pub struct WorldRenderer {
    pub sr: ShapeRenderer,
    camera: Camera,
    pub last_mouse_pos: Point2<f32>,
}

impl WorldRenderer {
    fn new(gl: &glow::Context) -> Self {
        // use glow::HasContext as _;

        Self {
            sr: ShapeRenderer::new(gl),
            camera: Camera::new(),
            last_mouse_pos: Point2::new(0.0, 0.0),
        }
    }

    fn destroy(&mut self, gl: &glow::Context) {
        self.sr.destroy(gl);
    }

    // fn as_world_object(&mut self) -> WorldObj<'_> {
    //     WorldObj {
    //         sr: &mut self.sr,
    //         last_mouse_pos: self.last_mouse_pos,
    //     }
    // }

    fn paint(
        &mut self,
        gl: &glow::Context,
        pos: Option<Pos2>,
        size: Vec2,
        pan: Vec2,
        zoom_factor: f32,
    ) {
        // first update the camera with any zoom and resize change
        self.camera.resize(size);
        self.camera.pan(pan);
        self.camera.zoom(zoom_factor);
        self.camera.update();

        // set the correct MVP matrix for the shape renderer
        let mvp: Matrix4<f32> = self.camera.get_mvp();
        self.sr.set_mvp(mvp);

        // unproject mouse position to
        if let Some(pos) = pos {
            self.last_mouse_pos = self.camera.unproject(pos);
        }

        // do the actual drawing of already cached vertices
        self.sr.flush(gl);
    }
}
