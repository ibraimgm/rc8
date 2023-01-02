use sdl2::{event::Event, keyboard::Keycode};

/// Makes dealing with SDL keymapping less verbose
macro_rules! map_keys {
    // entry point
    ($event:expr, $($input:tt)*) => {
        map_keys!(@inner $event, parsed=[], rest=[ $($input)* ])
    };

    // stop condition
    (@inner $event:expr, parsed = [$($parsed:tt)*], rest = [ ]) => {
        match $event {
            $($parsed)*
            _ => None
        }
    };

    // normal keydown
    (   @inner
        $event:expr,
        parsed = [$($parsed:tt)*],
        rest = [
            $keycode:pat => $action:expr,
            $($rest:tt)*
        ]
    ) => {
        map_keys!(
            @inner
            $event,
            parsed = [
                $($parsed)*
                Event::KeyDown {keycode: Some($keycode), .. } => Some($action),
            ],
            rest = [
                $($rest)*
            ]
        )
    };

    // emulate keydown
    (
        @inner
        $event:expr,
        parsed = [$($parsed:tt)*],
        rest = [
            @emulate $keycode:pat => $key:expr,
            $($rest:tt)*
        ]
    ) => {
        map_keys!(
            @inner
            $event,
            parsed = [
                $($parsed)*
                Event::KeyDown {keycode: Some($keycode), .. } => Some(Action::EmulateKeyState($key, true)),
                Event::KeyUp {keycode: Some($keycode), .. } => Some(Action::EmulateKeyState($key, false)),
            ],
            rest = [
                $($rest)*
            ]
        )
    };
}

/// Different key bindings depending on the application state
pub enum Keymap {
    Chip8,
}

/// Actions to be executed by the application
pub enum Action {
    EmulateKeyState(usize, bool),
    TogglePause,
    Quit,
}

impl Keymap {
    /// Translate and SDL2 event into an action to be executed by the app
    pub fn translate_action(&self, event: &Event) -> Option<Action> {
        match self {
            Keymap::Chip8 => map_keys!(event,
                @emulate Keycode::Num1 => 0x01,
                @emulate Keycode::Num2 => 0x02,
                @emulate Keycode::Num3 => 0x03,
                @emulate Keycode::Num4 => 0x0C,
                @emulate Keycode::Q => 0x04,
                @emulate Keycode::W => 0x05,
                @emulate Keycode::E => 0x06,
                @emulate Keycode::R => 0x0D,
                @emulate Keycode::A => 0x07,
                @emulate Keycode::S => 0x08,
                @emulate Keycode::D => 0x09,
                @emulate Keycode::F => 0x0E,
                @emulate Keycode::Z => 0x0A,
                @emulate Keycode::X => 0x00,
                @emulate Keycode::C => 0x0B,
                @emulate Keycode::V => 0x0F,
                Keycode::Space => Action::TogglePause,
                Keycode::Escape => Action::Quit,
            ),
        }
    }
}
