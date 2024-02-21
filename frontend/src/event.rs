#![allow(dead_code)]

#[derive(Debug, Clone)]
pub enum Event {
    ButtonPressed(ButtonId),
    SelectChanged(SelectId, String),
    CheckboxChanged(CheckboxId, bool),
    MouseMove(MouseEvent),
    MouseEnter(MouseEvent),
    MouseLeave(MouseEvent),
    MousePressed(MouseEvent),
    MouseReleased(MouseEvent),
    MouseClicked(MouseEvent),
    MouseWheel {
        x: i32,
        y: i32,
        delta_x: f64,
        delta_y: f64,
    },
}

#[derive(Debug, Clone)]
pub struct MouseEvent {
    pub x: i32,
    pub y: i32,
    pub button: MouseButton,
    pub ctrl_pressed: bool,
    pub shift_pressed: bool,
}

impl From<web_sys::MouseEvent> for MouseEvent {
    fn from(event: web_sys::MouseEvent) -> Self {
        MouseEvent {
            x: event.offset_x(),
            y: event.offset_y(),
            button: MouseButton::from_web_button(event.button()).unwrap(),
            ctrl_pressed: event.ctrl_key(),
            shift_pressed: event.shift_key(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonId {
    Reset,
    Step,
    Finish,
    ToggleEdit,
    LoadPreset,
    ClearStorage,
}

impl ButtonId {
    /// Get the html id of the button
    pub fn id_str(&self) -> &str {
        match self {
            ButtonId::Reset => "btn-reset",
            ButtonId::Step => "btn-step",
            ButtonId::Finish => "btn-finish",
            ButtonId::ToggleEdit => "btn-mode-edit",
            ButtonId::LoadPreset => "btn-load-preset",
            ButtonId::ClearStorage => "btn-clear-storage",
        }
    }

    /// iterates over all button ids
    pub fn iterate() -> impl Iterator<Item = ButtonId> {
        [
            ButtonId::Reset,
            ButtonId::Step,
            ButtonId::Finish,
            ButtonId::ToggleEdit,
            ButtonId::LoadPreset,
            ButtonId::ClearStorage,
        ]
        .iter()
        .copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Main,
    Auxillary,
    Secondary,
}

impl MouseButton {
    // Convert from web_sys::MouseEvent.button() to MouseButton
    pub fn from_web_button(button: i16) -> Option<MouseButton> {
        match button {
            0 => Some(MouseButton::Main),
            1 => Some(MouseButton::Auxillary),
            2 => Some(MouseButton::Secondary),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectId {
    Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckboxId {
    AutoStep,
}

enum Widget {
    Button(ButtonId),
    Select(SelectId),
    // Checkbox(CheckboxId),
}
