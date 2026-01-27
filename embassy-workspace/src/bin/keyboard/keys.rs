use crate::keyboard::core::Key;

pub struct KeyboardState {
    shift: bool,
    ctrl: bool,
    alt: bool,
    fn_key: bool,
}

impl KeyboardState {
    pub fn new() -> Self {
        Self {
            shift: false,
            ctrl: false,
            alt: false,
            fn_key: false,
        }
    }

    pub fn update_modifiers(&mut self, key: Key) {
        if key.contains(Key::LeftShift) {
            self.shift = true;
        }
        if key.contains(Key::LeftCtrl) {
            self.ctrl = true;
        }
        if key.contains(Key::LeftAlt) {
            self.alt = true;
        }
        if key.contains(Key::LeftFn) {
            self.fn_key = true;
        }
    }

    pub fn clear_modifiers(&mut self, key: Key) {
        if !key.contains(Key::LeftShift) {
            self.shift = false;
        }
        if !key.contains(Key::LeftCtrl) {
            self.ctrl = false;
        }
        if !key.contains(Key::LeftAlt) {
            self.alt = false;
        }
        if !key.contains(Key::LeftFn) {
            self.fn_key = false;
        }
    }

    pub fn key_to_char(&self, key: Key) -> Option<char> {
        if self.shift {
            self.key_to_char_shifted(key)
        } else {
            self.key_to_char_normal(key)
        }
    }

    fn key_to_char_normal(&self, key: Key) -> Option<char> {
        // US QWERTY layout - normal
        match key {
            // Top row (QWERTY)
            Key::Q => Some('q'),
            Key::W => Some('w'),
            Key::E => Some('e'),
            Key::R => Some('r'),
            Key::T => Some('t'),
            Key::Y => Some('y'),
            Key::U => Some('u'),
            Key::I => Some('i'),
            Key::O => Some('o'),
            Key::P => Some('p'),

            // Middle row (ASDFGH)
            Key::A => Some('a'),
            Key::S => Some('s'),
            Key::D => Some('d'),
            Key::F => Some('f'),
            Key::G => Some('g'),
            Key::H => Some('h'),
            Key::J => Some('j'),
            Key::K => Some('k'),
            Key::L => Some('l'),

            // Bottom row (ZXCVBN)
            Key::Z => Some('z'),
            Key::X => Some('x'),
            Key::C => Some('c'),
            Key::V => Some('v'),
            Key::B => Some('b'),
            Key::N => Some('n'),
            Key::M => Some('m'),

            // Number row
            Key::One => Some('1'),
            Key::Two => Some('2'),
            Key::Three => Some('3'),
            Key::Four => Some('4'),
            Key::Five => Some('5'),
            Key::Six => Some('6'),
            Key::Seven => Some('7'),
            Key::Eight => Some('8'),
            Key::Nine => Some('9'),
            Key::Zero => Some('0'),

            // Symbols (US QWERTY, no shift)
            Key::Space => Some(' '),
            Key::Minus => Some('-'),
            Key::Equal => Some('='),
            Key::OpenSquareBracket => Some('['),
            Key::CloseSquareBracket => Some(']'),
            Key::Backslash => Some('\\'),
            Key::SemiColon => Some(';'),
            Key::Quote => Some('\''),
            Key::Backquote => Some('`'),
            Key::Comma => Some(','),
            Key::Period => Some('.'),
            Key::Slash => Some('/'),

            _ => None,
        }
    }

    fn key_to_char_shifted(&self, key: Key) -> Option<char> {
        // US QWERTY layout - with Shift
        match key {
            // Top row (QWERTY) -> uppercase
            Key::Q => Some('Q'),
            Key::W => Some('W'),
            Key::E => Some('E'),
            Key::R => Some('R'),
            Key::T => Some('T'),
            Key::Y => Some('Y'),
            Key::U => Some('U'),
            Key::I => Some('I'),
            Key::O => Some('O'),
            Key::P => Some('P'),

            // Middle row (ASDFGHJKL) -> uppercase
            Key::A => Some('A'),
            Key::S => Some('S'),
            Key::D => Some('D'),
            Key::F => Some('F'),
            Key::G => Some('G'),
            Key::H => Some('H'),
            Key::J => Some('J'),
            Key::K => Some('K'),
            Key::L => Some('L'),

            // Bottom row (ZXCVBNM) -> uppercase
            Key::Z => Some('Z'),
            Key::X => Some('X'),
            Key::C => Some('C'),
            Key::V => Some('V'),
            Key::B => Some('B'),
            Key::N => Some('N'),
            Key::M => Some('M'),

            // Number row -> symbols
            Key::One => Some('!'),
            Key::Two => Some('@'),
            Key::Three => Some('#'),
            Key::Four => Some('$'),
            Key::Five => Some('%'),
            Key::Six => Some('^'),
            Key::Seven => Some('&'),
            Key::Eight => Some('*'),
            Key::Nine => Some('('),
            Key::Zero => Some(')'),

            // Symbols with Shift (US QWERTY)
            Key::Minus => Some('_'),
            Key::Equal => Some('+'),
            Key::OpenSquareBracket => Some('{'),
            Key::CloseSquareBracket => Some('}'),
            Key::Backslash => Some('|'),
            Key::SemiColon => Some(':'),
            Key::Quote => Some('"'),
            Key::Backquote => Some('~'),
            Key::Comma => Some('<'),
            Key::Period => Some('>'),
            Key::Slash => Some('?'),

            _ => None,
        }
    }

    pub fn handle_special_key(&self, key: Key) -> Option<SpecialKey> {
        match key {
            Key::Enter => Some(SpecialKey::Enter),
            Key::Tab => Some(SpecialKey::Tab),
            Key::Backspace => Some(SpecialKey::Backspace),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    Enter,
    Tab,
    Backspace,
}
