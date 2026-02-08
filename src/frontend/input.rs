use sdl2::keyboard::Keycode;
use crate::controller;

/// Map an SDL keycode to an NES button bitmask, or None if unmapped.
pub fn keycode_to_button(key: Keycode) -> Option<u8> {
    match key {
        Keycode::Z => Some(controller::BUTTON_A),
        Keycode::X => Some(controller::BUTTON_B),
        Keycode::Return => Some(controller::BUTTON_START),
        Keycode::RShift => Some(controller::BUTTON_SELECT),
        Keycode::Up => Some(controller::BUTTON_UP),
        Keycode::Down => Some(controller::BUTTON_DOWN),
        Keycode::Left => Some(controller::BUTTON_LEFT),
        Keycode::Right => Some(controller::BUTTON_RIGHT),
        _ => None,
    }
}
