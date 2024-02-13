#![allow(dead_code)]

#[derive(Debug, Clone, Copy)]
pub enum Event {
    ButtonPressed(ButtonId),
    MouseMove { x: i32, y: i32 },
    MouseEnter { x: i32, y: i32 },
    MouseLeave,
    MousePressed { x: i32, y: i32, button: MouseButton },
    MouseReleased { x: i32, y: i32, button: MouseButton },
    MouseClicked { x: i32, y: i32, button: MouseButton },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonId {
    Reset,
    Step,
    Finish,
}

impl ButtonId {
    /// Get the html id of the button
    pub fn id_str(&self) -> &str {
        match self {
            ButtonId::Reset => "btn-reset",
            ButtonId::Step => "btn-step",
            ButtonId::Finish => "btn-finish",
        }
    }

    /// iterates over all button ids
    pub fn iterate() -> impl Iterator<Item = ButtonId> {
        [ButtonId::Reset, ButtonId::Step, ButtonId::Finish]
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
