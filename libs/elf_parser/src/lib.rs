use mos_lib::io::ReadAt;

pub struct ElfParser<R: ReadAt> {
    reader: R,
}

impl<R: ReadAt> ElfParser<R> {
    pub fn parse_from(reader: R) -> Self {
        Self { reader }
    }
}

fn parse_header() {}
fn parse_program_header() {}
