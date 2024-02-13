#![allow(dead_code)]

#[derive(Debug, Clone, Copy)]
pub enum Event {
    ButtonPressed(ButtonId),
    MouseMove(i32, i32),
    MouseEnter(i32, i32),
    MouseLeave,
    MousePressed(MouseButton),
    MouseReleased(MouseButton),
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
    Left,
    Right,
    Middle,
}
