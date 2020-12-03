use core::fmt;
use lazy_static::lazy_static;
use spin::{Mutex, MutexGuard};
use volatile::Volatile;

/* Environment Structures */

#[repr(u8)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VGAColor {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGrey = 7,
    DarkGrey = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorCode(u8);

impl ColorCode {
    pub fn new(foreground: VGAColor, background: VGAColor) -> ColorCode {
        ColorCode((foreground as u8) | (background as u8) << 4)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenChar {
    pub ascii_character: u8,
    pub color_code: ColorCode,
}

/* VGA_WRITER */

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
pub struct ScreenBuffer {
    pub chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

impl ScreenBuffer {
    pub fn new(memory_location: usize) -> &'static mut Self {
        unsafe { &mut *(memory_location as *mut ScreenBuffer) }
    }
}

pub struct VGAWriter {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut ScreenBuffer,
}

impl VGAWriter {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;
                let color_code = self.color_code;

                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });

                self.column_position += 1;
            }
        }
    }

    pub fn write_string(&mut self, string: &str) {
        for byte in string.bytes() {
            match byte {
                0x20..=0x7E | b'\n' => self.write_byte(byte),
                _ => self.write_byte(0xFE),
            }
        }
    }

    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();

                self.buffer.chars[row - 1][col].write(character);
            }
        }

        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };

        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }
}

impl fmt::Write for VGAWriter {
    fn write_str(&mut self, string: &str) -> fmt::Result {
        self.write_string(string);
        Ok(())
    }
}

lazy_static! {
    static ref VGA_WRITER: Mutex<VGAWriter> = Mutex::new(VGAWriter {
        column_position: 0,
        color_code: ColorCode::new(VGAColor::White, VGAColor::Black),
        buffer: ScreenBuffer::new(0xB8000)
    });
}

pub fn safe_lock<F>(callback: F)
where
    F: Fn(&mut MutexGuard<VGAWriter>),
{
    // wrap the write function to ensure we don't deadlock on printing
    // for instance, an interrupt 'interrupts' the processing function (which might have locked VGA_WRITER),
    // and then the interrupt handler tries to print (with VGA_WRITER, which can't be implicitly unlocked anymore).
    // interrupts::without_interrupts(|| {
    //     callback(&mut VGA_WRITER.lock());
    // });

    callback(&mut VGA_WRITER.lock());
}

/* print.. macros */

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    safe_lock(|writer| {
        use core::fmt::Write;

        writer.write_fmt(args).unwrap();
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!('\n'));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
