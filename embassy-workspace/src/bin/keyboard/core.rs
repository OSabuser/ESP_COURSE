pub struct Keyboard<'a> {
    a0: esp_hal::gpio::Output<'a>,
    a1: esp_hal::gpio::Output<'a>,
    a2: esp_hal::gpio::Output<'a>,
    y0: esp_hal::gpio::Input<'a>,
    y1: esp_hal::gpio::Input<'a>,
    y2: esp_hal::gpio::Input<'a>,
    y3: esp_hal::gpio::Input<'a>,
    y4: esp_hal::gpio::Input<'a>,
    y5: esp_hal::gpio::Input<'a>,
    y6: esp_hal::gpio::Input<'a>,
    last_state: Key,
    pressed: Key,
    released: Key,
}

impl<'a> Keyboard<'a> {
    /// Initialises the cardputer's keyboard.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        a0: esp_hal::peripherals::GPIO8<'a>,
        a1: esp_hal::peripherals::GPIO9<'a>,
        a2: esp_hal::peripherals::GPIO11<'a>,
        y0: esp_hal::peripherals::GPIO13<'a>,
        y1: esp_hal::peripherals::GPIO15<'a>,
        y2: esp_hal::peripherals::GPIO3<'a>,
        y3: esp_hal::peripherals::GPIO4<'a>,
        y4: esp_hal::peripherals::GPIO5<'a>,
        y5: esp_hal::peripherals::GPIO6<'a>,
        y6: esp_hal::peripherals::GPIO7<'a>,
    ) -> Self {
        let a0 = esp_hal::gpio::Output::new(
            a0,
            esp_hal::gpio::Level::Low,
            esp_hal::gpio::OutputConfig::default()
                .with_drive_mode(esp_hal::gpio::DriveMode::PushPull),
        );
        let a1 = esp_hal::gpio::Output::new(
            a1,
            esp_hal::gpio::Level::Low,
            esp_hal::gpio::OutputConfig::default()
                .with_drive_mode(esp_hal::gpio::DriveMode::PushPull),
        );
        let a2 = esp_hal::gpio::Output::new(
            a2,
            esp_hal::gpio::Level::Low,
            esp_hal::gpio::OutputConfig::default()
                .with_drive_mode(esp_hal::gpio::DriveMode::PushPull),
        );
        let y0 = esp_hal::gpio::Input::new(
            y0,
            esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        );
        let y1 = esp_hal::gpio::Input::new(
            y1,
            esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        );
        let y2 = esp_hal::gpio::Input::new(
            y2,
            esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        );
        let y3 = esp_hal::gpio::Input::new(
            y3,
            esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        );
        let y4 = esp_hal::gpio::Input::new(
            y4,
            esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        );
        let y5 = esp_hal::gpio::Input::new(
            y5,
            esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        );
        let y6 = esp_hal::gpio::Input::new(
            y6,
            esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
        );

        Self {
            a0,
            a1,
            a2,
            y0,
            y1,
            y2,
            y3,
            y4,
            y5,
            y6,
            last_state: Key::none(),
            pressed: Key::none(),
            released: Key::none(),
        }
    }

    /// Scans the keyboard matrix and tracks which keys have been pressed and released.
    ///
    /// This function should be called often (100Hz) otherwise it may miss key presses.
    pub fn scan(&mut self) {
        let mut key_flags = 0_u64;
        for i in 0..8 {
            self.a0
                .set_level(esp_hal::gpio::Level::from(i & 0b0000_0001 != 0));
            self.a1
                .set_level(esp_hal::gpio::Level::from(i & 0b0000_0010 != 0));
            self.a2
                .set_level(esp_hal::gpio::Level::from(i & 0b0000_0100 != 0));

            let inputs: [esp_hal::gpio::Level; 7] = [
                self.y0.level(),
                self.y1.level(),
                self.y2.level(),
                self.y3.level(),
                self.y4.level(),
                self.y5.level(),
                self.y6.level(),
            ];

            let major_offset = i * 8;

            for (minor_offset, decoded) in inputs.iter().enumerate() {
                let bit = match decoded {
                    esp_hal::gpio::Level::Low => 1_u64,
                    esp_hal::gpio::Level::High => 0_u64,
                };
                key_flags |= bit << (major_offset + minor_offset);
            }
        }
        let key = Key { bits: key_flags };

        let changes = key.xor(self.last_state);

        let new_presses = changes.and(key);
        self.pressed |= new_presses;

        let new_releases = changes.and(!key);
        self.released |= new_releases;

        self.last_state = key;
    }

    /// Returns the keys that were held when [scan][Self::scan] was last called.
    pub fn held_keys(&self) -> Key {
        self.last_state
    }

    /// Returns buffered key presses that haven't already been cleared with
    /// [`clear_pressed_keys`][Self::clear_pressed_keys] or [`clear_some_pressed_keys`][Self::clear_some_pressed_keys]
    pub fn pressed_keys(&self) -> Key {
        self.pressed
    }

    /// Clears the key press buffer
    pub fn clear_pressed_keys(&mut self) {
        self.pressed = Key::none();
    }

    /// Clears specific keys from the key press buffer
    pub fn clear_some_pressed_keys(&mut self, keys_to_clear: Key) {
        self.pressed &= !keys_to_clear;
    }

    /// Returns buffered key releases that haven't already been cleared with
    /// [`clear_released_keys`][Self::clear_released_keys] or [`clear_some_released_keys`][Self::clear_some_released_keys]
    pub fn released_keys(&self) -> Key {
        self.released
    }

    /// Clears the key release buffer
    pub fn clear_released_keys(&mut self) {
        self.released = Key::none();
    }

    /// Clears specific keys from the key release buffer
    pub fn clear_some_released_keys(&mut self, keys_to_clear: Key) {
        self.released &= !keys_to_clear;
    }
}

use bitmask_enum::bitmask;

/// A set of physical keys on the cardputer's keyboard
///
/// This type is a bit flag (i.e. each bit corresponds to a different key).
#[bitmask(u64)]
#[bitmask_config(flags_iter)]
pub enum Key {
    LeftOpt,
    Z,
    C,
    B,
    M,
    Period,
    Space,
    _Dummy0,
    LeftShift,
    S,
    F,
    H,
    K,
    SemiColon,
    Enter,
    _Dummy1,
    Q,
    E,
    T,
    U,
    O,
    OpenSquareBracket,
    Backslash,
    _Dummy2,
    One,
    Three,
    Five,
    Seven,
    Nine,
    Minus,
    Backspace,
    _Dummy3,
    LeftCtrl,
    LeftAlt,
    X,
    V,
    N,
    Comma,
    Slash,
    _Dummy4,
    LeftFn,
    A,
    D,
    G,
    J,
    L,
    Quote,
    _Dummy5,
    Tab,
    W,
    R,
    Y,
    I,
    P,
    CloseSquareBracket,
    _Dummy6,
    Backquote,
    Two,
    Four,
    Six,
    Eight,
    Zero,
    Equal,
    _Dummy7,
}
