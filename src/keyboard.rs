use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Keyboard {
    pressed: HashSet<Key>,
    mappings: Vec<Mapping>,
}

impl Keyboard {
    pub fn new() -> Self {
        Keyboard::default()
    }

    pub fn key_down(&mut self, key: String) {
        if let Some(key) = self.mappings.iter().find(|k| k.key == key) {
            self.pressed.insert(key.mapping.clone());
        }
        // tracing::info!("KeyDown: {}, Pressed: {:?}", key, self.pressed);
    }

    pub fn key_up(&mut self, key: String) {
        if let Some(key) = self.mappings.iter().find(|k| k.key == key) {
            self.pressed.remove(&key.mapping);
        }
        // tracing::info!("KeyUp: {}, Pressed: {:?}", key, self.pressed);
    }

    pub fn get_row(&mut self, row: u8) -> u8 {
        let mut ret = 0xFF;
        let debug = !self.pressed.is_empty();

        if debug {
            // tracing::info!("Pressed: {:?}", self.pressed);
        }

        let pressed_in_row = self
            .mappings
            .iter()
            .filter(|k| k.row == row && self.pressed.contains(&k.mapping))
            .collect::<Vec<_>>();

        for key in pressed_in_row {
            // self.pressed.remove(&key.mapping);
            // tracing::info!("Deactivating bit: {}", key.col);
            ret &= !(1 << key.col);
        }

        // if debug {
        //     tracing::info!("Row: {} - Ret: {:8b}", row, ret);
        // }

        ret
    }
}

impl Default for Keyboard {
    fn default() -> Self {
        // let mut mappings = default_mapping().to_vec();
        // mappings.sort_by_key(|mapping| std::cmp::Reverse(mapping.col));

        Keyboard {
            pressed: HashSet::new(),
            mappings: default_mapping().to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Key {
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
    D9,
    Minus,
    Equal,
    Backslash,
    OpenBracket,
    CloseBracket,
    Semicolon,
    Quote,
    Backquote,
    Comma,
    Period,
    Slash,
    Dead,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Shift,
    Control,
    Capslock,
    Graph,
    Code,
    F1,
    F2,
    F3,
    F4,
    F5,
    Escape,
    Tab,
    Stop,
    Backspace,
    Select,
    Enter,
    Space,
    Home,
    Insert,
    Delete,
    Left,
    Up,
    Down,
    Right,
    NumMultiply,
    NumPlus,
    NumDivide,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    NumMinus,
    NumComma,
    NumPeriod,
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mapping {
    key: String,
    row: u8,
    col: u8,
    mapping: Key,
}

impl Mapping {
    pub fn new(key: &str, row: u8, col: u8, mapping: Key) -> Self {
        Mapping {
            key: key.to_string(),
            row,
            col,
            mapping,
        }
    }
}

fn default_mapping() -> [Mapping; 90] {
    [
        Mapping::new("Digit0", 0, 0, Key::D0),
        Mapping::new("Digit1", 0, 1, Key::D1),
        Mapping::new("Digit2", 0, 2, Key::D2),
        Mapping::new("Digit3", 0, 3, Key::D3),
        Mapping::new("Digit4", 0, 4, Key::D4),
        Mapping::new("Digit5", 0, 5, Key::D5),
        Mapping::new("Digit6", 0, 6, Key::D6),
        Mapping::new("Digit7", 0, 7, Key::D7),
        Mapping::new("Digit8", 1, 0, Key::D8),
        Mapping::new("Digit9", 1, 1, Key::D9),
        Mapping::new("Minus", 1, 2, Key::Minus),
        Mapping::new("Equal", 1, 3, Key::Equal),
        Mapping::new("Backslash", 1, 4, Key::Backslash),
        Mapping::new("OpenBracket", 1, 5, Key::OpenBracket),
        Mapping::new("CloseBracket", 1, 6, Key::CloseBracket),
        Mapping::new("Semicolon", 1, 7, Key::Semicolon),
        Mapping::new("Quote", 2, 0, Key::Quote),
        Mapping::new("Backquote", 2, 1, Key::Backquote),
        Mapping::new("Comma", 2, 2, Key::Comma),
        Mapping::new("Period", 2, 3, Key::Period),
        Mapping::new("Slash", 2, 4, Key::Slash),
        Mapping::new("Dead", 2, 5, Key::Dead),
        Mapping::new("KeyA", 2, 6, Key::A),
        Mapping::new("KeyB", 2, 7, Key::B),
        Mapping::new("KeyC", 3, 0, Key::C),
        Mapping::new("KeyD", 3, 1, Key::D),
        Mapping::new("KeyE", 3, 2, Key::E),
        Mapping::new("KeyF", 3, 3, Key::F),
        Mapping::new("KeyG", 3, 4, Key::G),
        Mapping::new("KeyH", 3, 5, Key::H),
        Mapping::new("KeyI", 3, 6, Key::I),
        Mapping::new("KeyJ", 3, 7, Key::J),
        Mapping::new("KeyK", 4, 0, Key::K),
        Mapping::new("KeyL", 4, 1, Key::L),
        Mapping::new("KeyM", 4, 2, Key::M),
        Mapping::new("KeyN", 4, 3, Key::N),
        Mapping::new("KeyO", 4, 4, Key::O),
        Mapping::new("KeyP", 4, 5, Key::P),
        Mapping::new("KeyQ", 4, 6, Key::Q),
        Mapping::new("KeyR", 4, 7, Key::R),
        Mapping::new("KeyS", 5, 0, Key::S),
        Mapping::new("KeyT", 5, 1, Key::T),
        Mapping::new("KeyU", 5, 2, Key::U),
        Mapping::new("KeyV", 5, 3, Key::V),
        Mapping::new("KeyW", 5, 4, Key::W),
        Mapping::new("KeyX", 5, 5, Key::X),
        Mapping::new("KeyY", 5, 6, Key::Y),
        Mapping::new("KeyZ", 5, 7, Key::Z),
        Mapping::new("ShiftLeft", 6, 0, Key::Shift),
        Mapping::new("ControlLeft", 6, 1, Key::Control),
        Mapping::new("CapsLock", 6, 2, Key::Capslock),
        Mapping::new("Graph", 6, 3, Key::Graph),
        Mapping::new("IntlRo", 6, 4, Key::Code),
        Mapping::new("F1", 6, 5, Key::F1),
        Mapping::new("F2", 6, 6, Key::F2),
        Mapping::new("F3", 6, 7, Key::F3),
        Mapping::new("F4", 7, 0, Key::F4),
        Mapping::new("F5", 7, 1, Key::F5),
        Mapping::new("Escape", 7, 2, Key::Escape),
        Mapping::new("Tab", 7, 3, Key::Tab),
        Mapping::new("Stop", 7, 4, Key::Stop),
        Mapping::new("Backspace", 7, 5, Key::Backspace),
        Mapping::new("Select", 7, 6, Key::Select),
        Mapping::new("Enter", 7, 7, Key::Enter),
        Mapping::new("Space", 8, 0, Key::Space),
        Mapping::new("Home", 8, 1, Key::Home),
        Mapping::new("Insert", 8, 2, Key::Insert),
        Mapping::new("Delete", 8, 3, Key::Delete),
        Mapping::new("ArrowLeft", 8, 4, Key::Left),
        Mapping::new("ArrowUp", 8, 5, Key::Up),
        Mapping::new("ArrowDown", 8, 6, Key::Down),
        Mapping::new("ArrowRight", 8, 7, Key::Right),
        Mapping::new("NumpadMultiply", 9, 0, Key::NumMultiply),
        Mapping::new("NumpadAdd", 9, 1, Key::NumPlus),
        Mapping::new("NumpadDivide", 9, 2, Key::NumDivide),
        Mapping::new("Numpad0", 9, 3, Key::Num0),
        Mapping::new("Numpad1", 9, 4, Key::Num1),
        Mapping::new("Numpad2", 9, 5, Key::Num2),
        Mapping::new("Numpad3", 9, 6, Key::Num3),
        Mapping::new("Numpad4", 9, 7, Key::Num4),
        Mapping::new("Numpad5", 10, 0, Key::Num5),
        Mapping::new("Numpad6", 10, 1, Key::Num6),
        Mapping::new("Numpad7", 10, 2, Key::Num7),
        Mapping::new("Numpad8", 10, 3, Key::Num8),
        Mapping::new("Numpad9", 10, 4, Key::Num9),
        Mapping::new("NumpadSubtract", 10, 7, Key::NumMinus),
        Mapping::new("NumpadComma", 11, 0, Key::NumComma),
        Mapping::new("NumpadDecimal", 11, 1, Key::NumPeriod),
        Mapping::new("Yes", 11, 2, Key::Yes),
        Mapping::new("No", 11, 3, Key::No),
    ]
}
