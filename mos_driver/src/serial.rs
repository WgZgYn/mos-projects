use lazy_static::lazy_static;
use mos_arch::sync::SpinIrqLock;
use uart_16550::backend::PioBackend;
use uart_16550::{Config, Uart16550Tty};

lazy_static! {
    pub static ref SERIAL1: SpinIrqLock<Uart16550Tty<PioBackend>> = {
        let serial_port = unsafe { Uart16550Tty::new_port(0x3F8, Config::DEFAULT) }.unwrap();
        SpinIrqLock::new(serial_port)
    };
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1.access(|serial| serial.write_fmt(args).expect("Printing to serial failed"))
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
