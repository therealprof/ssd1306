//! Unbuffered terminal display mode
//!
//! This mode uses the 7x7 pixel [MarioChrome](https://github.com/techninja/MarioChron/) font to
//! draw characters to the display without needing a framebuffer. It will write characters from top
//! left to bottom right in an 8x8 pixel grid, restarting at the top left of the display once full.
//! The display itself takes care of wrapping lines.
//!
//! ```rust,ignore
//! let i2c = /* I2C interface from your HAL of choice */;
//! let display: TerminalMode<_> = Builder::new().connect_i2c(i2c).into();
//!
//! display.init().unwrap();
//! display.clear().unwrap();
//!
//! // Print a-zA-Z
//! for c in 97..123 {
//!     display.write_str(unsafe { core::str::from_utf8_unchecked(&[c]) }).unwrap();
//! }
//! ```

use crate::command::AddrMode;
use crate::displayrotation::DisplayRotation;
use crate::displaysize::DisplaySize;
use crate::interface::DisplayInterface;
use crate::mode::displaymode::DisplayModeTrait;
use crate::properties::DisplayProperties;
use core::cmp::min;
use core::fmt;
use hal::blocking::delay::DelayMs;
use hal::digital::OutputPin;

/// A bitmap-display character, which is either an 8x8 bitmap or a special character
#[derive(Clone, Copy)]
pub enum BitmapCharacter {
    /// An 8x8 bitmap character
    Bitmapped([u8; 8]),
    /// A newline character which causes the cursor to jump to the next line
    Newline,
    /// A carriage return character which causes the cursor to jump to the start of the current line
    CarriageReturn,
}

/// A trait to convert from a character to 8x8 bitmap
pub trait CharacterBitmap<T> {
    /// Turn input of type T into a displayable 8x8 bitmap or special character
    fn to_bitmap(input: T) -> BitmapCharacter;
}

/// A 7x7 font shamelessly borrowed from https://github.com/techninja/MarioChron/
impl<DI> CharacterBitmap<char> for TerminalMode<DI>
where
    DI: DisplayInterface,
{
    fn to_bitmap(input: char) -> BitmapCharacter {
        use BitmapCharacter::{Bitmapped, CarriageReturn, Newline};

        // Populate the array with the data from the character array at the right index
        match input {
            '\n' => Newline,
            '\r' => CarriageReturn,
            '!' => Bitmapped([0x00, 0x00, 0x5F, 0x00, 0x00, 0x00, 0x00, 0x00]),
            '"' => Bitmapped([0x00, 0x07, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00]),
            '#' => Bitmapped([0x14, 0x7F, 0x14, 0x7F, 0x14, 0x00, 0x00, 0x00]),
            '$' => Bitmapped([0x24, 0x2A, 0x7F, 0x2A, 0x12, 0x00, 0x00, 0x00]),
            '%' => Bitmapped([0x23, 0x13, 0x08, 0x64, 0x62, 0x00, 0x00, 0x00]),
            '&' => Bitmapped([0x36, 0x49, 0x55, 0x22, 0x50, 0x00, 0x00, 0x00]),
            '\'' => Bitmapped([0x00, 0x05, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00]),
            '(' => Bitmapped([0x00, 0x1C, 0x22, 0x41, 0x00, 0x00, 0x00, 0x00]),
            ')' => Bitmapped([0x00, 0x41, 0x22, 0x1C, 0x00, 0x00, 0x00, 0x00]),
            '*' => Bitmapped([0x08, 0x2A, 0x1C, 0x2A, 0x08, 0x00, 0x00, 0x00]),
            '+' => Bitmapped([0x08, 0x08, 0x3E, 0x08, 0x08, 0x00, 0x00, 0x00]),
            ',' => Bitmapped([0x00, 0x50, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00]),
            '-' => Bitmapped([0x00, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x00]),
            '.' => Bitmapped([0x00, 0x60, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00]),
            '/' => Bitmapped([0x20, 0x10, 0x08, 0x04, 0x02, 0x00, 0x00, 0x00]),
            '0' => Bitmapped([0x1C, 0x3E, 0x61, 0x41, 0x43, 0x3E, 0x1C, 0x00]),
            '1' => Bitmapped([0x40, 0x42, 0x7F, 0x7F, 0x40, 0x40, 0x00, 0x00]),
            '2' => Bitmapped([0x62, 0x73, 0x79, 0x59, 0x5D, 0x4F, 0x46, 0x00]),
            '3' => Bitmapped([0x20, 0x61, 0x49, 0x4D, 0x4F, 0x7B, 0x31, 0x00]),
            '4' => Bitmapped([0x18, 0x1C, 0x16, 0x13, 0x7F, 0x7F, 0x10, 0x00]),
            '5' => Bitmapped([0x27, 0x67, 0x45, 0x45, 0x45, 0x7D, 0x38, 0x00]),
            '6' => Bitmapped([0x3C, 0x7E, 0x4B, 0x49, 0x49, 0x79, 0x30, 0x00]),
            '7' => Bitmapped([0x03, 0x03, 0x71, 0x79, 0x0D, 0x07, 0x03, 0x00]),
            '8' => Bitmapped([0x36, 0x7F, 0x49, 0x49, 0x49, 0x7F, 0x36, 0x00]),
            '9' => Bitmapped([0x06, 0x4F, 0x49, 0x49, 0x69, 0x3F, 0x1E, 0x00]),
            ':' => Bitmapped([0x00, 0x36, 0x36, 0x00, 0x00, 0x00, 0x00, 0x00]),
            ';' => Bitmapped([0x00, 0x56, 0x36, 0x00, 0x00, 0x00, 0x00, 0x00]),
            '<' => Bitmapped([0x00, 0x08, 0x14, 0x22, 0x41, 0x00, 0x00, 0x00]),
            '=' => Bitmapped([0x14, 0x14, 0x14, 0x14, 0x14, 0x00, 0x00, 0x00]),
            '>' => Bitmapped([0x41, 0x22, 0x14, 0x08, 0x00, 0x00, 0x00, 0x00]),
            '?' => Bitmapped([0x02, 0x01, 0x51, 0x09, 0x06, 0x00, 0x00, 0x00]),
            '@' => Bitmapped([0x32, 0x49, 0x79, 0x41, 0x3E, 0x00, 0x00, 0x00]),
            'A' => Bitmapped([0x7E, 0x11, 0x11, 0x11, 0x7E, 0x00, 0x00, 0x00]),
            'B' => Bitmapped([0x7F, 0x49, 0x49, 0x49, 0x36, 0x00, 0x00, 0x00]),
            'C' => Bitmapped([0x3E, 0x41, 0x41, 0x41, 0x22, 0x00, 0x00, 0x00]),
            'D' => Bitmapped([0x7F, 0x7F, 0x41, 0x41, 0x63, 0x3E, 0x1C, 0x00]),
            'E' => Bitmapped([0x7F, 0x49, 0x49, 0x49, 0x41, 0x00, 0x00, 0x00]),
            'F' => Bitmapped([0x7F, 0x09, 0x09, 0x01, 0x01, 0x00, 0x00, 0x00]),
            'G' => Bitmapped([0x3E, 0x41, 0x41, 0x51, 0x32, 0x00, 0x00, 0x00]),
            'H' => Bitmapped([0x7F, 0x08, 0x08, 0x08, 0x7F, 0x00, 0x00, 0x00]),
            'I' => Bitmapped([0x00, 0x41, 0x7F, 0x41, 0x00, 0x00, 0x00, 0x00]),
            'J' => Bitmapped([0x20, 0x40, 0x41, 0x3F, 0x01, 0x00, 0x00, 0x00]),
            'K' => Bitmapped([0x7F, 0x08, 0x14, 0x22, 0x41, 0x00, 0x00, 0x00]),
            'L' => Bitmapped([0x7F, 0x7F, 0x40, 0x40, 0x40, 0x40, 0x00, 0x00]),
            'M' => Bitmapped([0x7F, 0x02, 0x04, 0x02, 0x7F, 0x00, 0x00, 0x00]),
            'N' => Bitmapped([0x7F, 0x04, 0x08, 0x10, 0x7F, 0x00, 0x00, 0x00]),
            'O' => Bitmapped([0x3E, 0x7F, 0x41, 0x41, 0x41, 0x7F, 0x3E, 0x00]),
            'P' => Bitmapped([0x7F, 0x09, 0x09, 0x09, 0x06, 0x00, 0x00, 0x00]),
            'Q' => Bitmapped([0x3E, 0x41, 0x51, 0x21, 0x5E, 0x00, 0x00, 0x00]),
            'R' => Bitmapped([0x7F, 0x7F, 0x11, 0x31, 0x79, 0x6F, 0x4E, 0x00]),
            'S' => Bitmapped([0x46, 0x49, 0x49, 0x49, 0x31, 0x00, 0x00, 0x00]),
            'T' => Bitmapped([0x01, 0x01, 0x7F, 0x01, 0x01, 0x00, 0x00, 0x00]),
            'U' => Bitmapped([0x3F, 0x40, 0x40, 0x40, 0x3F, 0x00, 0x00, 0x00]),
            'V' => Bitmapped([0x1F, 0x20, 0x40, 0x20, 0x1F, 0x00, 0x00, 0x00]),
            'W' => Bitmapped([0x7F, 0x7F, 0x38, 0x1C, 0x38, 0x7F, 0x7F, 0x00]),
            'X' => Bitmapped([0x63, 0x14, 0x08, 0x14, 0x63, 0x00, 0x00, 0x00]),
            'Y' => Bitmapped([0x03, 0x04, 0x78, 0x04, 0x03, 0x00, 0x00, 0x00]),
            'Z' => Bitmapped([0x61, 0x51, 0x49, 0x45, 0x43, 0x00, 0x00, 0x00]),
            '[' => Bitmapped([0x00, 0x00, 0x7F, 0x41, 0x41, 0x00, 0x00, 0x00]),
            '\\' => Bitmapped([0x02, 0x04, 0x08, 0x10, 0x20, 0x00, 0x00, 0x00]),
            ']' => Bitmapped([0x41, 0x41, 0x7F, 0x00, 0x00, 0x00, 0x00, 0x00]),
            '^' => Bitmapped([0x04, 0x02, 0x01, 0x02, 0x04, 0x00, 0x00, 0x00]),
            '_' => Bitmapped([0x40, 0x40, 0x40, 0x40, 0x40, 0x00, 0x00, 0x00]),
            '`' => Bitmapped([0x00, 0x01, 0x02, 0x04, 0x00, 0x00, 0x00, 0x00]),
            'a' => Bitmapped([0x20, 0x54, 0x54, 0x54, 0x78, 0x00, 0x00, 0x00]),
            'b' => Bitmapped([0x7F, 0x48, 0x44, 0x44, 0x38, 0x00, 0x00, 0x00]),
            'c' => Bitmapped([0x38, 0x44, 0x44, 0x44, 0x20, 0x00, 0x00, 0x00]),
            'd' => Bitmapped([0x38, 0x44, 0x44, 0x48, 0x7F, 0x00, 0x00, 0x00]),
            'e' => Bitmapped([0x38, 0x54, 0x54, 0x54, 0x18, 0x00, 0x00, 0x00]),
            'f' => Bitmapped([0x08, 0x7E, 0x09, 0x01, 0x02, 0x00, 0x00, 0x00]),
            'g' => Bitmapped([0x08, 0x14, 0x54, 0x54, 0x3C, 0x00, 0x00, 0x00]),
            'h' => Bitmapped([0x7F, 0x08, 0x04, 0x04, 0x78, 0x00, 0x00, 0x00]),
            'i' => Bitmapped([0x00, 0x44, 0x7D, 0x40, 0x00, 0x00, 0x00, 0x00]),
            'j' => Bitmapped([0x20, 0x40, 0x44, 0x3D, 0x00, 0x00, 0x00, 0x00]),
            'k' => Bitmapped([0x00, 0x7F, 0x10, 0x28, 0x44, 0x00, 0x00, 0x00]),
            'l' => Bitmapped([0x00, 0x41, 0x7F, 0x40, 0x00, 0x00, 0x00, 0x00]),
            'm' => Bitmapped([0x7C, 0x04, 0x18, 0x04, 0x78, 0x00, 0x00, 0x00]),
            'n' => Bitmapped([0x7C, 0x08, 0x04, 0x04, 0x78, 0x00, 0x00, 0x00]),
            'o' => Bitmapped([0x38, 0x44, 0x44, 0x44, 0x38, 0x00, 0x00, 0x00]),
            'p' => Bitmapped([0x7C, 0x14, 0x14, 0x14, 0x08, 0x00, 0x00, 0x00]),
            'q' => Bitmapped([0x08, 0x14, 0x14, 0x18, 0x7C, 0x00, 0x00, 0x00]),
            'r' => Bitmapped([0x7C, 0x08, 0x04, 0x04, 0x08, 0x00, 0x00, 0x00]),
            's' => Bitmapped([0x48, 0x54, 0x54, 0x54, 0x20, 0x00, 0x00, 0x00]),
            't' => Bitmapped([0x04, 0x3F, 0x44, 0x40, 0x20, 0x00, 0x00, 0x00]),
            'u' => Bitmapped([0x3C, 0x40, 0x40, 0x20, 0x7C, 0x00, 0x00, 0x00]),
            'v' => Bitmapped([0x1C, 0x20, 0x40, 0x20, 0x1C, 0x00, 0x00, 0x00]),
            'w' => Bitmapped([0x3C, 0x40, 0x30, 0x40, 0x3C, 0x00, 0x00, 0x00]),
            'x' => Bitmapped([0x00, 0x44, 0x28, 0x10, 0x28, 0x44, 0x00, 0x00]),
            'y' => Bitmapped([0x0C, 0x50, 0x50, 0x50, 0x3C, 0x00, 0x00, 0x00]),
            'z' => Bitmapped([0x44, 0x64, 0x54, 0x4C, 0x44, 0x00, 0x00, 0x00]),
            '{' => Bitmapped([0x00, 0x08, 0x36, 0x41, 0x00, 0x00, 0x00, 0x00]),
            '|' => Bitmapped([0x00, 0x00, 0x7F, 0x00, 0x00, 0x00, 0x00, 0x00]),
            '}' => Bitmapped([0x00, 0x41, 0x36, 0x08, 0x00, 0x00, 0x00, 0x00]),
            _ => Bitmapped([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
        }
    }
}

/// Contains the new row that the cursor has wrapped around to
struct CursorWrapEvent(u8);

struct Cursor {
    col: u8,
    row: u8,
    width: u8,
    height: u8,
}

impl Cursor {
    pub fn new(width_pixels: u8, height_pixels: u8) -> Self {
        let width = width_pixels / 8;
        let height = height_pixels / 8;
        Cursor {
            col: 0,
            row: 0,
            width,
            height,
        }
    }

    /// Advances the logical cursor by one character.
    /// Returns a value indicating if this caused the cursor to wrap to the next line or the next screen.
    pub fn advance(&mut self) -> Option<CursorWrapEvent> {
        self.col = (self.col + 1) % self.width;
        if self.col == 0 {
            self.row = (self.row + 1) % self.height;
            Some(CursorWrapEvent(self.row))
        } else {
            None
        }
    }

    /// Sets the position of the logical cursor arbitrarily.
    /// The position will be capped at the maximal possible position.
    pub fn set_position(&mut self, col: u8, row: u8) {
        self.col = min(col, self.width - 1);
        self.row = min(row, self.height - 1);
    }

    /// Gets the position of the logical cursor on screen in (col, row) order
    pub fn get_position(&self) -> (u8, u8) {
        (self.col, self.row)
    }

    /// Gets the logical dimensions of the screen in terms of characters, as (width, height)
    pub fn get_dimensions(&self) -> (u8, u8) {
        (self.width, self.height)
    }

    /// Returns the number of characters which can be written to the current line before it will
    /// wrap
    pub fn get_remaining_columns_in_line(&self) -> u8 {
        self.width - self.col
    }
}

// TODO: Add to prelude
/// Terminal mode handler
pub struct TerminalMode<DI> {
    properties: DisplayProperties<DI>,
    cursor: Option<Cursor>,
}

impl<DI> DisplayModeTrait<DI> for TerminalMode<DI>
where
    DI: DisplayInterface,
{
    /// Create new TerminalMode instance
    fn new(properties: DisplayProperties<DI>) -> Self {
        TerminalMode {
            properties,
            cursor: None,
        }
    }

    /// Release all resources used by TerminalMode
    fn release(self) -> DisplayProperties<DI> {
        self.properties
    }
}

impl<DI> TerminalMode<DI>
where
    DI: DisplayInterface,
{
    /// Clear the display and reset the cursor to the top left corner
    pub fn clear(&mut self) -> Result<(), ()> {
        let display_size = self.properties.get_size();

        let numchars = match display_size {
            DisplaySize::Display128x64 => 128,
            DisplaySize::Display128x32 => 64,
            DisplaySize::Display96x16 => 24,
        };

        // Let the chip handle line wrapping so we can fill the screen with blanks faster
        self.properties.change_mode(AddrMode::Horizontal)?;
        let (display_width, display_height) = self.properties.get_dimensions();
        self.properties
            .set_draw_area((0, 0), (display_width, display_height))?;

        for _ in 0..numchars {
            self.properties.draw(&[0; 8])?;
        }

        // But for normal operation we manage the line wrapping
        self.properties.change_mode(AddrMode::Page)?;
        self.reset_pos()?;

        Ok(())
    }

    /// Reset display
    pub fn reset<RST, DELAY>(&mut self, rst: &mut RST, delay: &mut DELAY)
    where
        RST: OutputPin,
        DELAY: DelayMs<u8>,
    {
        rst.set_high();
        delay.delay_ms(1);
        rst.set_low();
        delay.delay_ms(10);
        rst.set_high();
    }

    /// Write out data to display. This is a noop in terminal mode.
    pub fn flush(&mut self) -> Result<(), ()> {
        Ok(())
    }

    /// Print a character to the display
    pub fn print_char<T>(&mut self, c: T) -> Result<(), ()>
    where
        TerminalMode<DI>: CharacterBitmap<T>,
    {
        match Self::to_bitmap(c) {
            BitmapCharacter::Bitmapped(ref buffer) => {
                // Send the pixel data to the display
                self.properties.draw(buffer)?;
                // Increment character counter and potentially wrap line
                self.advance_cursor()?;
            }
            BitmapCharacter::Newline => {
                let num_spaces = self.ensure_cursor()?.get_remaining_columns_in_line();
                for _ in 0..num_spaces {
                    self.properties
                        .draw(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])?;
                    self.advance_cursor()?;
                }
            }
            BitmapCharacter::CarriageReturn => {
                self.properties.set_column(0)?;
                let (_, cur_line) = self.ensure_cursor()?.get_position();
                self.ensure_cursor()?.set_position(0, cur_line);
            }
        }

        Ok(())
    }

    /// Initialise the display in page mode (i.e. a byte walks down a column of 8 pixels) with
    /// column 0 on the left and column _(display_width - 1)_ on the right, but no automatic line
    /// wrapping.
    pub fn init(&mut self) -> Result<(), ()> {
        self.properties.init_with_mode(AddrMode::Page)?;
        self.reset_pos()?;
        Ok(())
    }

    /// Set the display rotation
    pub fn set_rotation(&mut self, rot: DisplayRotation) -> Result<(), ()> {
        // we don't need to touch the cursor because rotating 90º or 270º currently just flips
        self.properties.set_rotation(rot)
    }

    /// Get the current cursor position, in character coordinates.
    /// This is the (column, row) that the next character will be written to.
    pub fn get_position(&self) -> Result<(u8, u8), ()> {
        self.cursor.as_ref().map(|c| c.get_position()).ok_or(())
    }

    /// Set the cursor position, in character coordinates.
    /// This is the (column, row) that the next character will be written to.
    /// If the position is out of bounds, an Err will be returned.
    pub fn set_position(&mut self, column: u8, row: u8) -> Result<(), ()> {
        let (width, height) = self.ensure_cursor()?.get_dimensions();
        if column >= width || row >= height {
            Err(())
        } else {
            self.properties.set_column(column * 8)?;
            self.properties.set_row(row * 8)?;
            self.ensure_cursor()?.set_position(column, row);
            Ok(())
        }
    }

    /// Reset the draw area and move pointer to the top left corner
    fn reset_pos(&mut self) -> Result<(), ()> {
        self.properties.set_column(0)?;
        self.properties.set_row(0)?;
        // Initialise the counter when we know it's valid
        let (display_width, display_height) = self.properties.get_dimensions();
        self.cursor = Some(Cursor::new(display_width, display_height));

        Ok(())
    }

    /// Advance the cursor, automatically wrapping lines and/or screens if necessary
    /// Takes in an already-unwrapped cursor to avoid re-unwrapping
    fn advance_cursor(&mut self) -> Result<(), ()> {
        if let Some(CursorWrapEvent(new_row)) = self.ensure_cursor()?.advance() {
            self.properties.set_row(new_row * 8)?;
        }
        Ok(())
    }

    fn ensure_cursor(&mut self) -> Result<&mut Cursor, ()> {
        self.cursor.as_mut().ok_or(())
    }
}

impl<DI> fmt::Write for TerminalMode<DI>
where
    DI: DisplayInterface,
{
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        s.chars().map(move |c| self.print_char(c)).last();
        Ok(())
    }
}
