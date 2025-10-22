use std::fmt::Write;
use crate::vga::{CONSOLE, Color, ColorCode};
use log::{Metadata, Record};
pub struct KernelLogger;

impl log::Log for KernelLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &Record) {
        // Used for debug, if the log used through interrupts, it may deadlock,
        // so here to disable interrupts
        let mut guard = CONSOLE.lock();
        guard
            .with_color(ColorCode::new(Color::Pink, Color::Black), |console| {
                console
                    .write_fmt(format_args!("[{}]", record.level()))
                    .expect("VGA WRITE ERROR WHILE LOGGING");
            })
            .with_color(ColorCode::new(Color::LightGreen, Color::Black), |console| {
                console
                    .write_fmt(format_args!(" {}\n", record.args()))
                    .expect("VGA WRITE ERROR WHILE LOGGING");
            });
    }

    fn flush(&self) {
        println!()
    }
}
