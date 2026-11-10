use super::*;
fn row(t: &Terminal, y: usize) -> alloc::string::String {
    (0..t.cols)
        .map(|x| t.cell(x, y).unwrap().character)
        .collect()
}
#[test]
fn every_split_preserves_utf8_and_ansi() {
    let bytes = "abc\x1b[31;44m红é\x1b[0m\r\nxyz\x1b[2D!".as_bytes();
    let mut expected = Terminal::new(20, 4);
    expected.write(bytes);
    for split in 0..=bytes.len() {
        let mut t = Terminal::new(20, 4);
        t.write(&bytes[..split]);
        t.write(&bytes[split..]);
        assert_eq!(t.cells, expected.cells, "split={split}");
        assert_eq!(t.cursor(), expected.cursor());
    }
    let mut t = Terminal::new(20, 4);
    for byte in bytes {
        t.write(core::slice::from_ref(byte));
    }
    assert_eq!(t.cells, expected.cells);
}
#[test]
fn sgr_colors_reset_bold_inverse_and_extended() {
    let mut t = Terminal::new(20, 2);
    t.write(b"\x1b[31;44mA\x1b[1mB\x1b[7mC\x1b[0mD\x1b[38;5;196mE\x1b[48;2;1;2;3mF");
    assert_eq!(t.cell(0, 0).unwrap().foreground, PALETTE[1]);
    assert_eq!(t.cell(0, 0).unwrap().background, PALETTE[4]);
    assert_eq!(t.cell(1, 0).unwrap().foreground, PALETTE[9]);
    assert_eq!(t.cell(2, 0).unwrap().foreground, PALETTE[4]);
    assert_eq!(t.cell(3, 0).unwrap(), Style::DEFAULT.cell('D'));
    assert_eq!(t.cell(4, 0).unwrap().foreground, [255, 0, 0]);
    assert_eq!(t.cell(5, 0).unwrap().background, [1, 2, 3]);
}
#[test]
fn delayed_wrap_and_cr_lf_are_distinct() {
    let mut t = Terminal::new(3, 2);
    t.write(b"abc");
    assert_eq!(t.cursor(), (2, 0, true));
    t.write(b"\rX");
    assert_eq!(row(&t, 0), "Xbc");
    t.write(b"\nY");
    assert_eq!(row(&t, 1), " Y ");
    t.reset();
    t.write(b"abcdefg");
    assert_eq!(row(&t, 0), "def");
    assert_eq!(row(&t, 1), "g  ");
}
#[test]
fn erase_and_cursor_controls_do_not_erase_on_backspace() {
    let mut t = Terminal::new(8, 3);
    t.write(b"abcdef\x08");
    assert_eq!(row(&t, 0), "abcdef  ");
    t.write(b"\x1b[3G\x1b[K");
    assert_eq!(row(&t, 0), "ab      ");
    t.write(b"\x1b[2;3HZ\x1b[s\x1b[99;99HX\x1b[u!");
    assert_eq!(row(&t, 1), "  Z!    ");
    assert_eq!(t.cell(7, 2).unwrap().character, 'X');
    t.write(b"\x1b[2J");
    assert!(t.cells.iter().all(|c| c.character == ' '));
    assert_eq!(t.cursor(), (4, 1, true));
}
#[test]
fn insert_delete_erase_chars_and_scroll() {
    let mut t = Terminal::new(6, 3);
    t.write(b"abcdef\r\x1b[2C\x1b[2P");
    assert_eq!(row(&t, 0), "abef  ");
    t.write(b"\x1b[@X");
    assert_eq!(row(&t, 0), "abXef ");
    t.write(b"\x1b[2X");
    assert_eq!(row(&t, 0), "abX   ");
    t.write(b"\x1b[T");
    assert_eq!(row(&t, 1), "abX   ");
    t.write(b"\x1b[S");
    assert_eq!(row(&t, 0), "abX   ");
}
#[test]
fn unknown_and_overlong_sequences_are_bounded_and_recover() {
    let mut t = Terminal::new(12, 2);
    t.write(b"\x1b[999999999999999999mA\x1b[1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16;17mB");
    assert_eq!(t.cell(0, 0).unwrap(), Style::DEFAULT.cell('A'));
    assert_eq!(t.cell(1, 0).unwrap(), Style::DEFAULT.cell('B'));
    t.write(b"\x1b]title");
    for _ in 0..10000 {
        t.write(b"x");
    }
    t.write(b"\x1b\\C\x1bPdiscard\x18D");
    assert_eq!(&row(&t, 0)[..4], "ABCD");
}
#[test]
fn malformed_utf8_does_not_swallow_ascii_or_escape() {
    let mut t = Terminal::new(12, 2);
    t.write(b"\xe2");
    t.write(b"A\xff\xed\xa0\x80\x1b[32mB");
    assert_eq!(
        &row(&t, 0)
            .chars()
            .take(5)
            .collect::<alloc::string::String>(),
        "�A��B"
    );
    assert_eq!(t.cell(4, 0).unwrap().foreground, PALETTE[2]);
}
#[test]
fn dirty_cells_cover_old_cursor_and_reset() {
    let mut t = Terminal::new(4, 2);
    for i in 0..8 {
        assert!(t.take_dirty(i).is_some());
    }
    t.write(b"\x1b[2;2H\x1b[?25l");
    assert!(t.take_dirty(0).is_some());
    assert!(t.take_dirty(5).is_some());
    assert!(!t.cursor().2);
    t.reset();
    assert!(t.cursor().2);
    assert!((0..8).all(|i| t.take_dirty(i).is_some()));
}
#[test]
fn arbitrary_streams_keep_cursor_and_storage_bounded() {
    let mut t = Terminal::new(1, 1);
    let mut n = 42u32;
    for _ in 0..30000 {
        n = n.wrapping_mul(1664525).wrapping_add(1013904223);
        t.write(&[(n >> 24) as u8]);
        assert_eq!(t.cursor().0, 0);
        assert_eq!(t.cursor().1, 0);
        assert_eq!(t.cells.len(), 1);
    }
}
