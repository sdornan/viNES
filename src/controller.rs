pub const BUTTON_A: u8 = 0b0000_0001;
pub const BUTTON_B: u8 = 0b0000_0010;
pub const BUTTON_SELECT: u8 = 0b0000_0100;
pub const BUTTON_START: u8 = 0b0000_1000;
pub const BUTTON_UP: u8 = 0b0001_0000;
pub const BUTTON_DOWN: u8 = 0b0010_0000;
pub const BUTTON_LEFT: u8 = 0b0100_0000;
pub const BUTTON_RIGHT: u8 = 0b1000_0000;

#[derive(Clone)]
pub struct Controller {
    pub buttons: u8,
    strobe: bool,
    shift_register: u8,
}

impl Default for Controller {
    fn default() -> Self {
        Self::new()
    }
}

impl Controller {
    pub fn new() -> Self {
        Controller {
            buttons: 0,
            strobe: false,
            shift_register: 0,
        }
    }

    pub fn write(&mut self, val: u8) {
        if val & 1 == 1 {
            self.strobe = true;
        } else {
            if self.strobe {
                self.shift_register = self.buttons;
            }
            self.strobe = false;
        }
    }

    pub fn read(&mut self) -> u8 {
        if self.strobe {
            return self.buttons & 1;
        }
        let val = self.shift_register & 1;
        self.shift_register >>= 1;
        val
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strobe_reads_all_buttons() {
        let mut ctrl = Controller::new();
        ctrl.buttons = 0b10100101; // A, Select, Up, Right

        // Strobe on then off to latch
        ctrl.write(1);
        ctrl.write(0);

        // Read 8 buttons serially (LSB first: A, B, Select, Start, Up, Down, Left, Right)
        assert_eq!(ctrl.read(), 1); // A
        assert_eq!(ctrl.read(), 0); // B
        assert_eq!(ctrl.read(), 1); // Select
        assert_eq!(ctrl.read(), 0); // Start
        assert_eq!(ctrl.read(), 0); // Up -> wait, bit 4 is 1
    }

    #[test]
    fn test_strobe_mode_returns_button_a() {
        let mut ctrl = Controller::new();
        ctrl.buttons = 0b0000_0001; // A pressed
        ctrl.write(1); // strobe on

        // While strobe is on, always returns A button state
        assert_eq!(ctrl.read(), 1);
        assert_eq!(ctrl.read(), 1);
        assert_eq!(ctrl.read(), 1);
    }

    #[test]
    fn test_after_8_reads_returns_zero() {
        let mut ctrl = Controller::new();
        ctrl.buttons = 0xFF;
        ctrl.write(1);
        ctrl.write(0);

        for _ in 0..8 {
            ctrl.read();
        }
        // After 8 reads, shift register is 0
        assert_eq!(ctrl.read(), 0);
    }
}
