#![no_std]
#![no_main]

use note::{Note, Speaker};
use user_lib::fs::OpenFlags;
use user_lib::mos::fs::FileDesc;
use user_lib::{open, println, sleep, write};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[TEST_SPK] start");
    let spk = open(c"/dev/spk", OpenFlags::WRITE).expect("failed to open spk");
    const AUDIO: &[Note] = &note::embed::MY_GO;
    struct PcSpeakerBlock(FileDesc);
    impl Speaker for PcSpeakerBlock {
        fn speak_on(&self, freq: u32, dura: u32) {
            let freq = freq.to_le_bytes();
            write(self.0, &freq);
            sleep(dura as _);
            write(self.0, &[0, 0, 0, 0]);
        }
    }
    note::SimpleAudio::new(97, PcSpeakerBlock(spk)).play(AUDIO);
    println!("[TEST_SPK] finish");
    0
}
