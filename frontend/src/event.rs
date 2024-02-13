#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}
