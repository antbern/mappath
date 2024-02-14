#![allow(dead_code)]

#[derive(Debug, Clone)]
pub enum Event {
    ButtonPressed(ButtonId),
    SelectChanged(SelectId, String),
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
    ModeSetup,
    ModeEdit,
    ModePathFind,
    LoadPreset,
}

impl ButtonId {
    /// Get the html id of the button
    pub fn id_str(&self) -> &str {
        match self {
            ButtonId::Reset => "btn-reset",
            ButtonId::Step => "btn-step",
            ButtonId::Finish => "btn-finish",
            ButtonId::ModeSetup => "btn-mode-setup",
            ButtonId::ModeEdit => "btn-mode-edit",
            ButtonId::ModePathFind => "btn-mode-find",
            ButtonId::LoadPreset => "btn-load-preset",
        }
    }

    /// iterates over all button ids
    pub fn iterate() -> impl Iterator<Item = ButtonId> {
        [
            ButtonId::Reset,
            ButtonId::Step,
            ButtonId::Finish,
            ButtonId::ModeSetup,
            ButtonId::ModeEdit,
            ButtonId::ModePathFind,
            ButtonId::LoadPreset,
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
pub enum Mode {
    Edit,
    PathFind,
}

enum Widget {
    Button(ButtonId),
    Select(SelectId),
    // Checkbox(CheckboxId),
}
