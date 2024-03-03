use gloo::storage::Storage;
use log::debug;
use optimize::grid::{Cell, Point};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::RwLock;
use wasm_bindgen::JsCast;
use web_sys::Document;
use web_sys::HtmlDivElement;
use web_sys::HtmlElement;
use web_sys::HtmlInputElement;
use web_sys::HtmlPreElement;
use web_sys::HtmlSelectElement;

use crate::event::ButtonId;
use crate::event::Event;
use crate::event::InputChange;
use crate::event::InputId;
use crate::event::MouseEvent;

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
    pub fn set_output(&self, output: &str) {
        self.write(|inner| inner.output.set_inner_text(output));
    }

    pub fn enable_button(&self, button_id: ButtonId, enabled: bool) {
        self.write(|inner| {
            let button = inner
                .document
                .get_element_by_id(button_id.id_str())
                .unwrap();
            if !enabled {
                button.set_attribute("disabled", "").unwrap();
            } else {
                button.remove_attribute("disabled").unwrap();
            }
        });
    }

    pub fn enable_element(&self, id: &str, enabled: bool) {
        self.write(|inner| {
            let element = inner.document.get_element_by_id(id).unwrap();
            if !enabled {
                element.set_attribute("disabled", "").unwrap();
            } else {
                element.remove_attribute("disabled").unwrap();
            }
        });
    }

    /// Shows or hides a div element
    pub fn show_div(&self, div_id: &str, show: bool) {
        self.write(|inner| {
            let div: HtmlDivElement = inner
                .document
                .get_element_by_id(div_id)
                .unwrap()
                .dyn_into::<web_sys::HtmlDivElement>()
                .unwrap();
            if show {
                div.style().remove_property("display").unwrap();
            } else {
                div.style().set_property("display", "none").unwrap();
            }
        });
    }

    pub fn get_storage<T: for<'de> serde::Deserialize<'de>>(&self, key: &str) -> Option<T> {
        debug!("getting storage: key = {}", key);
        match gloo::storage::LocalStorage::get(key) {
            Ok(Some(value)) => Some(value),
            Err(e) => {
                log::error!("Failed to get storage for key: {}: {}", key, e);
                None
            }
            _ => None,
        }
    }
    pub fn set_storage<T: serde::Serialize>(&self, key: &str, value: &T) {
        debug!("setting storage: key = {}", key);
        gloo::storage::LocalStorage::set(key, value).unwrap();
    }

    pub fn remove_storage(&self, key: &str) {
        gloo::storage::LocalStorage::delete(key);
    }

    pub fn input<R>(&self, f: impl FnOnce(&Input) -> R) -> R {
        self.read(|inner| f(&inner.input))
    }
    // pub fn input_mut<R>(&self, f: impl FnOnce(&mut Input) -> R) -> R {
    //     self.write(|inner| f(&mut inner.input))
    // }

    pub fn push_event(&self, event: Event) {
        self.write(|inner| {
            debug!("pushing event: {:?}", event);
            inner.events.push_back(event.clone());
            inner.input.on_event(event);
        });
    }

    pub fn pop_event(&self) -> Option<Event> {
        self.write(|inner| inner.events.pop_front())
    }

    pub fn request_repaint(&self) {
        self.write(|inner| inner.repaint_requested = true);
    }

    pub fn is_repaint_requested(&self) -> bool {
        self.write(|inner| {
            let repaint_requested = inner.repaint_requested;
            inner.repaint_requested = false;
            repaint_requested
        })
    }

    pub fn set_active_cell(&self, cell: Cell<usize>) {
        self.write(|inner| {
            inner.cell_selector.set_cell(cell);
        });
    }
    pub fn get_active_cell(&self) -> Option<Cell<usize>> {
        self.read(|inner| inner.cell_selector.get_cell())
    }

    pub fn get_input_value(&self, id: InputId) -> InputChange {
        self.read(|inner| {
            // handle select separately since it is not an input element
            if let InputId::Select(id) = id {
                let select: HtmlSelectElement = inner
                    .document
                    .get_element_by_id(id.id_str())
                    .unwrap()
                    .dyn_into()
                    .unwrap();
                return InputChange::Select {
                    id,
                    value: select.value(),
                };
            }

            let input: HtmlInputElement = inner
                .document
                .get_element_by_id(id.id_str())
                .unwrap()
                .dyn_into()
                .unwrap();

            match id {
                InputId::Number(id) => InputChange::Number {
                    id,
                    value: input.value().parse().unwrap(),
                },
                InputId::Checkbox(id) => InputChange::Checkbox {
                    id,
                    value: input.checked(),
                },
                InputId::Select(_) => unreachable!(),
            }
        })
    }

    pub fn set_input_value(&self, change: &InputChange) {
        self.write(|inner| {
            let input: HtmlInputElement = inner
                .document
                .get_element_by_id(change.id_str())
                .unwrap()
                .dyn_into()
                .unwrap();

            match change {
                InputChange::Number { value, .. } => input.set_value(&value.to_string()),
                InputChange::Checkbox { value, .. } => input.set_checked(*value),
                InputChange::Select { value, .. } => input.set_value(value),
            }
        });
    }
}

pub struct CellSelector {
    pub radio_invalid: HtmlInputElement,
    pub radio_valid: HtmlInputElement,
    pub input_valid_cost: HtmlInputElement,
    pub radio_oneway: HtmlInputElement,
    pub select_oneway: HtmlSelectElement,
    pub span_oneway_target: HtmlElement,
}

impl CellSelector {
    pub fn set_cell(&self, cell: Cell<usize>) {
        debug!("setting cell: {:?}", cell);

        match cell {
            Cell::Invalid => self.radio_invalid.set_checked(true),
            Cell::Valid { cost } => {
                self.radio_valid.set_checked(true);
                self.input_valid_cost.set_value(&cost.to_string());
            }
            Cell::OneWay {
                cost,
                direction,
                target,
            } => {
                self.radio_oneway.set_checked(true);
                self.input_valid_cost.set_value(&cost.to_string());
                self.select_oneway.set_value(&direction.to_string());

                let target_string = if let Some(target) = target {
                    serde_json::to_string(&target).unwrap()
                } else {
                    "".to_string()
                };

                self.span_oneway_target
                    .set_inner_text(target_string.as_str());
            }
        }
    }

    pub fn get_cell(&self) -> Option<Cell<usize>> {
        if self.radio_invalid.checked() {
            Some(Cell::Invalid)
        } else if self.radio_valid.checked() {
            let cost = self.input_valid_cost.value().parse().unwrap();
            Some(Cell::Valid { cost })
        } else if self.radio_oneway.checked() {
            let cost = self.input_valid_cost.value().parse().unwrap();
            let direction = self.select_oneway.value().parse().unwrap();

            // try to deserialize the target
            let target = self.span_oneway_target.inner_text();
            let target: Option<Point> = if !target.is_empty() {
                match serde_json::from_str(&target) {
                    Ok(point) => Some(point),
                    Err(e) => {
                        gloo::dialogs::alert(&format!("Could not parse target point: {e}"));
                        None
                    }
                }
            } else {
                None
            };

            Some(Cell::OneWay {
                cost,
                direction,
                target,
            })
        } else {
            None
        }
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
    pub fn on_event(&mut self, event: Event) {
        match event {
            Event::MouseEnter(MouseEvent { x, y, .. }) => self.mouse_position = Some((x, y)),
            Event::MouseMove(MouseEvent { x, y, .. }) => self.mouse_position = Some((x, y)),
            Event::MouseLeave(_) => self.mouse_position = None,
            _ => {}
        }
    }

    pub fn current_mouse_position(&self) -> Option<(i32, i32)> {
        self.mouse_position
    }
}
pub struct ContextImpl {
    pub document: Document,
    pub cell_selector: CellSelector,
    pub output: HtmlPreElement,
    pub input: Input,
    pub events: VecDeque<Event>,
    pub repaint_requested: bool,
}
