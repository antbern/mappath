#![allow(dead_code)]

#[derive(Debug, Clone)]
pub enum Event {
    ButtonPressed(ButtonId),
    SelectChanged(SelectId, String),
    InputChanged(InputChange),
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
    EditSave,
    SelectPoint,
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
            ButtonId::EditSave => "btn-edit-save",
            ButtonId::SelectPoint => "btn-select-point",
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
            ButtonId::EditSave,
            ButtonId::SelectPoint,
        ]
        .iter()
        .copied()
    }

    pub fn from_key_code(key: &str) -> Option<ButtonId> {
        match key {
            "r" => Some(ButtonId::Reset),
            "t" => Some(ButtonId::Step),
            "f" => Some(ButtonId::Finish),
            "e" => Some(ButtonId::ToggleEdit),
            "s" => Some(ButtonId::EditSave),
            "p" => Some(ButtonId::SelectPoint),
            _ => None,
        }
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
    // Mode,
}

impl SelectId {
    pub fn id_str(&self) -> &str {
        match self {
            // SelectId::Mode => "select-mode",
            _ => todo!(),
        }
    }
    pub fn iterate() -> impl Iterator<Item = SelectId> {
        [
            //SelectId::Mode
        ]
        .into_iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckboxId {
    AutoStep,
}

impl CheckboxId {
    pub fn id_str(&self) -> &str {
        match self {
            CheckboxId::AutoStep => "input-auto-step",
        }
    }
    pub fn iterate() -> impl Iterator<Item = CheckboxId> {
        [CheckboxId::AutoStep].into_iter()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberInputId {
    Rows,
    Cols,
    BackgroundAlpha,
    ForegroundAlpha,
}
impl NumberInputId {
    pub fn id_str(&self) -> &str {
        match self {
            NumberInputId::Rows => "input-rows",
            NumberInputId::Cols => "input-cols",
            NumberInputId::BackgroundAlpha => "input-background-alpha",
            NumberInputId::ForegroundAlpha => "input-foreground-alpha",
        }
    }
    pub fn iterate() -> impl Iterator<Item = NumberInputId> {
        [
            NumberInputId::Rows,
            NumberInputId::Cols,
            NumberInputId::BackgroundAlpha,
            NumberInputId::ForegroundAlpha,
        ]
        .into_iter()
    }
}

/// Event to fire or to emit when an input changes
#[derive(Debug, Clone, PartialEq)]
pub enum InputChange {
    Number { id: NumberInputId, value: f64 },
    Checkbox { id: CheckboxId, value: bool },
    Select { id: SelectId, value: String },
}
impl InputChange {
    pub fn id_str(&self) -> &str {
        match self {
            InputChange::Number { id, .. } => id.id_str(),
            InputChange::Checkbox { id, .. } => id.id_str(),
            InputChange::Select { id, .. } => id.id_str(),
        }
    }
}

/// Used for querying the input state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputId {
    Number(NumberInputId),
    Checkbox(CheckboxId),
    Select(SelectId),
}

impl InputId {
    pub fn id_str(&self) -> &str {
        match self {
            InputId::Number(id) => id.id_str(),
            InputId::Checkbox(id) => id.id_str(),
            InputId::Select(id) => id.id_str(),
        }
    }
    pub fn iterate() -> impl Iterator<Item = InputId> {
        NumberInputId::iterate()
            .map(InputId::Number)
            .chain(CheckboxId::iterate().map(InputId::Checkbox))
            .chain(SelectId::iterate().map(InputId::Select))
    }
}
