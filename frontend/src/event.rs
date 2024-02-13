pub enum Event {
    ButtonPressed(ButtonId),
    MouseMoved(i32, i32),
    MouseEnter(i32, i32),
    MouseLeave,
    MousePressed(MouseButton),
    MouseReleased(MouseButton),
}

pub enum ButtonId {
    Reset,
    Step,
    Finish,
}

pub enum MouseButton {
    Left,
    Right,
    Middle,
}
