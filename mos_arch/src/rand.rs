use crate::{rand, tick};
use mos_lib::rand::simple_hash;

pub fn sys_rand() -> u64 {
    rand().unwrap_or(simple_hash(tick()))
}
