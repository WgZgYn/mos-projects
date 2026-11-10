use core::ops::DerefMut;
use lazy_static::lazy_static;
use mos_arch::sync::SpinIrqLock;
use uart_16550::backend::PioBackend;
use uart_16550::{Config, Uart16550Tty};

lazy_static! {
    pub static ref SERIAL1: SpinIrqLock<Option<Uart16550Tty<PioBackend>>> = {
        let serial_port = unsafe { Uart16550Tty::new_port(0x3F8, Config::DEFAULT) }.ok();
        SpinIrqLock::new(serial_port)
    };
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    let mut guard = SERIAL1.lock();
    if let Some(s) = guard.deref_mut() {
        let _ = s.write_fmt(args);
    }
}

/// Byte output must bypass fmt::Write and the UART's convenience TTY layer:
/// UTF-8 can be split across syscalls and termios owns CRLF/BS processing.
pub fn write_bytes(bytes: &[u8], onlcr: bool) {
    let mut guard = SERIAL1.lock();
    if let Some(serial) = guard.deref_mut() {
        for &byte in bytes {
            if byte == b'\n' && onlcr {
                serial.inner_mut().send_bytes_exact(b"\r");
            }
            serial
                .inner_mut()
                .send_bytes_exact(core::slice::from_ref(&byte));
        }
    }
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}
