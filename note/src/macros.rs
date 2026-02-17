#[macro_export]
macro_rules! dur {
    (w) => {
        NoteType::Whole
    };
    (h) => {
        NoteType::Half
    };
    (q) => {
        NoteType::Quarter
    };
    (e) => {
        NoteType::Eighth
    };
    (s) => {
        NoteType::Sixteenth
    };
}

#[macro_export]
macro_rules! melody {
    // ========= 终止 =========
    (@acc [$($out:expr),*],
        $scale:expr) => {
        [$($out),*]
    };

    // ========= 最后一个 =========
    (@acc [$($out:expr),*],
        $scale:expr,
        $num:literal $dur:ident
    ) => {
        melody!(
            @acc
            [$($out,)* Note::new($scale[$num], $crate::dur!($dur))],
            $scale
        )
    };

    (@acc [$($out:expr),*],
        $scale:expr,
        $num:literal + $dur:ident
    ) => {
        melody!(
            @acc
            [$($out,)* Note::new($scale[$num].high(), $crate::dur!($dur))],
            $scale
        )
    };

    (@acc [$($out:expr),*],
        $scale:expr,
        $num:literal - $dur:ident
    ) => {{
        melody!(
            @acc
            [$($out,)* Note::new($scale[$num].low(), $crate::dur!($dur))],
            $scale
        )
    }};

    // ========= 普通 =========
    (@acc [$($out:expr),*],
        $scale:expr,
        $num:literal $dur:ident,
        $($rest:tt)*
    ) => {
        melody!(
            @acc [$($out,)* Note::new($scale[$num], $crate::dur!($dur))],
            $scale,
            $($rest)*
        )
    };

    // ========= 高 =========
    (@acc [$($out:expr),*],
        $scale:expr,
        $num:literal + $dur:ident,
        $($rest:tt)*
    ) => {
        melody!(
            @acc [$($out,)* Note::new($scale[$num].high(), $crate::dur!($dur))],
            $scale,
            $($rest)*
        )
    };

    // ========= 低 =========
    (@acc [$($out:expr),*],
        $scale:expr,
        $num:literal - $dur:ident,
        $($rest:tt)*
    ) => {
        melody!(
            @acc [$($out,)* Note::new($scale[$num].low(), $crate::dur!($dur))],
            $scale,
            $($rest)*
        )
    };

    // ========= 空 =========
    (key: [$base:ident, $level:expr], notes: { }) => {
        []
    };

    // ========= 入口 =========
    (key: [$base:ident, $level:expr], notes: { $($input:tt)* }) => {{
        let scale = Pitch::new(PitchClass::$base, $level).major_scale();
        melody!(
            @acc [],
            scale,
            $($input)*
        )
    }};
}
