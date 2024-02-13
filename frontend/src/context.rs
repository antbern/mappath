use std::sync::Arc;
use std::sync::RwLock;
use web_sys::HtmlPreElement;

#[derive(Clone)]
pub struct Context {
    inner: Arc<RwLock<ContextImpl>>,
}

impl Context {
    pub fn new(inner: ContextImpl) -> Self {
        Self {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    /// apply non-exclusive read access to the inner context
    fn read<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&ContextImpl) -> T,
    {
        let inner = self.inner.read().unwrap();
        f(&*inner)
    }

    /// apply exclusive write access to the inner context
    fn write<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut ContextImpl) -> T,
    {
        let mut inner = self.inner.write().unwrap();
        f(&mut *inner)
    }
}

impl Context {
    pub fn draw(&self, f: impl FnOnce(&web_sys::CanvasRenderingContext2d)) {
        self.write(|inner| f(&inner.rendering_context));
    }

    pub fn set_output(&self, output: &str) {
        self.write(|inner| inner.output.set_inner_text(output));
    }

    pub fn input<R>(&self, f: impl FnOnce(&Input) -> R) -> R {
        self.read(|inner| f(&inner.input))
    }
    pub fn input_mut<R>(&self, f: impl FnOnce(&mut Input) -> R) -> R {
        self.write(|inner| f(&mut inner.input))
    }

    pub fn canvas(&self, f: impl FnOnce(&web_sys::HtmlCanvasElement)) {
        self.read(|inner| f(&inner.canvas));
    }
}

pub struct Input {
    mouse_position: Option<(i32, i32)>,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            mouse_position: Default::default(),
        }
    }
}
impl Input {
    pub fn on_mouse_enter(&mut self, event: web_sys::MouseEvent) {
        self.mouse_position = Some((event.offset_x(), event.offset_y()));
    }

    pub fn on_mouse_move(&mut self, event: web_sys::MouseEvent) {
        self.mouse_position = Some((event.offset_x(), event.offset_y()));
    }

    pub fn on_mouse_leave(&mut self, _event: web_sys::MouseEvent) {
        self.mouse_position = None;
    }

    pub fn current_mouse_position(&self) -> Option<(i32, i32)> {
        self.mouse_position
    }
}
pub struct ContextImpl {
    pub rendering_context: web_sys::CanvasRenderingContext2d,
    pub canvas: web_sys::HtmlCanvasElement,
    pub output: HtmlPreElement,
    pub input: Input,
}
