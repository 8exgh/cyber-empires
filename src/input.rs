//! Input state shared between the host (JS) and the game.

#[derive(Clone, Debug)]
pub struct Input {
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_down: bool,
    pub mouse_down_prev: bool,
    pub mouse_right: bool,
    pub mouse_right_prev: bool,
    /// Key states indexed by our own key codes (see `Key`).
    pub keys: [bool; 256],
    pub keys_prev: [bool; 256],
    /// Typed characters since last frame (ASCII), for name entry.
    pub typed: Vec<u8>,
}

/// Key codes used by the engine (host maps browser keys onto these).
pub mod key {
    pub const ESC: u8 = 1;
    pub const ENTER: u8 = 2;
    pub const BACKSPACE: u8 = 3;
    pub const SPACE: u8 = 4;
    pub const TAB: u8 = 5;
    pub const UP: u8 = 10;
    pub const DOWN: u8 = 11;
    pub const LEFT: u8 = 12;
    pub const RIGHT: u8 = 13;
    pub const F1: u8 = 20;
    pub const F2: u8 = 21;
    pub const F3: u8 = 22;
    pub const F4: u8 = 23;
    pub const F5: u8 = 24;
    pub const F6: u8 = 25;
    pub const F7: u8 = 26;
    pub const F8: u8 = 27;
    pub const F9: u8 = 28;
    pub const F10: u8 = 29;
    // numeric keypad
    pub const KP0: u8 = 40;
    pub const KP1: u8 = 41;
    pub const KP2: u8 = 42;
    pub const KP3: u8 = 43;
    pub const KP4: u8 = 44;
    pub const KP5: u8 = 45;
    pub const KP6: u8 = 46;
    pub const KP7: u8 = 47;
    pub const KP8: u8 = 48;
    pub const KP9: u8 = 49;
    pub const KP_PLUS: u8 = 50;
    pub const KP_ENTER: u8 = 51;
    pub const KP_DOT: u8 = 52;
    pub const SHIFT: u8 = 60;
    pub const CTRL: u8 = 61;
    pub const ALT: u8 = 62;
    /// ASCII letters/digits are their uppercase ASCII code (65..90, 48..57).
    pub fn ascii(c: u8) -> u8 {
        c.to_ascii_uppercase()
    }
}

impl Default for Input {
    fn default() -> Input {
        Input { mouse_x: 0, mouse_y: 0, mouse_down: false, mouse_down_prev: false, mouse_right: false, mouse_right_prev: false, keys: [false; 256], keys_prev: [false; 256], typed: Vec::new() }
    }
}

impl Input {
    pub fn new() -> Input {
        Input::default()
    }
    pub fn clicked(&self) -> bool {
        self.mouse_down && !self.mouse_down_prev
    }
    pub fn released(&self) -> bool {
        !self.mouse_down && self.mouse_down_prev
    }
    pub fn right_clicked(&self) -> bool {
        self.mouse_right && !self.mouse_right_prev
    }
    pub fn key(&self, k: u8) -> bool {
        self.keys[k as usize]
    }
    pub fn pressed(&self, k: u8) -> bool {
        self.keys[k as usize] && !self.keys_prev[k as usize]
    }
    pub fn any_pressed(&self) -> bool {
        (0..256).any(|i| self.keys[i] && !self.keys_prev[i])
    }
    /// Called by the host at the end of each frame.
    pub fn end_frame(&mut self) {
        self.mouse_down_prev = self.mouse_down;
        self.mouse_right_prev = self.mouse_right;
        self.keys_prev = self.keys;
        self.typed.clear();
    }
}
