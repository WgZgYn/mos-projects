#![no_std]
#![recursion_limit = "256"]

pub mod embed;
mod macros;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PitchClass {
    R,
    C,
    Cs,
    D,
    Ds,
    E,
    F,
    Fs,
    G,
    Gs,
    A,
    As,
    B,
}

impl PitchClass {
    pub const fn from_u8(n: u8) -> Self {
        match n {
            1 => PitchClass::C,
            2 => PitchClass::Cs,
            3 => PitchClass::D,
            4 => PitchClass::Ds,
            5 => PitchClass::E,
            6 => PitchClass::F,
            7 => PitchClass::Fs,
            8 => PitchClass::G,
            9 => PitchClass::Gs,
            10 => PitchClass::A,
            11 => PitchClass::As,
            12 => PitchClass::B,
            _ => PitchClass::R,
        }
    }
    pub const fn as_u8(&self) -> u8 {
        match self {
            PitchClass::R => 0,
            PitchClass::C => 1,
            PitchClass::Cs => 2,
            PitchClass::D => 3,
            PitchClass::Ds => 4,
            PitchClass::E => 5,
            PitchClass::F => 6,
            PitchClass::Fs => 7,
            PitchClass::G => 8,
            PitchClass::Gs => 9,
            PitchClass::A => 10,
            PitchClass::As => 11,
            PitchClass::B => 12,
        }
    }
}

pub const PITCH_FREQ_TABLE: [[u32; 12]; 9] = [
    [16, 17, 18, 19, 20, 21, 23, 24, 25, 27, 29, 30],
    [32, 35, 37, 39, 41, 44, 46, 49, 52, 55, 58, 62],
    [65, 69, 73, 78, 82, 87, 93, 98, 104, 110, 116, 123],
    [131, 139, 147, 156, 165, 175, 185, 196, 208, 220, 233, 247],
    [262, 277, 294, 311, 330, 349, 370, 392, 415, 440, 466, 494],
    [523, 554, 587, 622, 659, 698, 740, 784, 830, 880, 932, 988],
    [
        1047, 1109, 1175, 1245, 1319, 1397, 1480, 1568, 1661, 1760, 1865, 1976,
    ],
    [
        2093, 2217, 2349, 2489, 2637, 2794, 2960, 3136, 3322, 3520, 3729, 3951,
    ],
    [
        4186, 4435, 4699, 4978, 5274, 5588, 5920, 6272, 6645, 7040, 7459, 7902,
    ],
];

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Pitch {
    class: PitchClass,
    level: u8,
}

impl Pitch {
    pub const fn new(pitch: PitchClass, level: u8) -> Self {
        Self {
            class: pitch,
            level,
        }
    }
    pub const fn freq(&self) -> u32 {
        if let PitchClass::R = self.class {
            return 0;
        }
        PITCH_FREQ_TABLE[self.level as usize][self.class.as_u8() as usize - 1]
    }
    pub const fn major_scale(&self) -> [Self; 8] {
        pub const MAJOR_SCALE_INTERVALS: [u8; 7] = [2, 2, 1, 2, 2, 2, 1];
        let mut scale = [*self; 8];
        scale[0].class = PitchClass::R;
        let mut current = *self;
        let mut i = 2;
        while i < 8 {
            let mut next = current.class.as_u8() + MAJOR_SCALE_INTERVALS[i - 2];
            if next >= 12 {
                next -= 12;
                current.level += 1;
            }
            current.class = PitchClass::from_u8(next);
            scale[i] = current;
            i += 1;
        }
        scale
    }
    pub const fn high(&self) -> Self {
        Self::new(self.class, self.level + 1)
    }
    pub const fn low(&self) -> Self {
        Self::new(self.class, self.level - 1)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum NoteType {
    Whole = 1,      // 全音符
    Half = 2,       // 二分音符
    Quarter = 4,    // 四分音符
    Eighth = 8,     // 八分音符
    Sixteenth = 16, // 十六分音符
}

pub struct Note {
    pitch: Pitch,
    dura: NoteType,
}

impl Note {
    pub const fn new(pitch: Pitch, dura: NoteType) -> Self {
        Self { pitch, dura }
    }
}

pub trait Speaker {
    /// be careful the freq maybe zero
    fn speak_on(&self, freq: u32, dura: u32);
}

pub struct SimpleAudio<S> {
    whole_node_ms: u32,
    speaker: S,
}

impl<S> SimpleAudio<S>
where
    S: Speaker,
{
    pub fn new(bpm: u32, speaker: S) -> Self {
        Self {
            whole_node_ms: 60000 * 4 / bpm,
            speaker,
        }
    }

    pub fn play(&self, notes: &[Note]) {
        for note in notes {
            self.play_node(note);
        }
    }

    fn node_duration(&self, note: NoteType) -> u32 {
        self.whole_node_ms / note as u32
    }

    fn play_node(&self, note: &Note) {
        self.speaker
            .speak_on(note.pitch.freq(), self.node_duration(note.dura));
    }
}
