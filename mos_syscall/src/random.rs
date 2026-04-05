use mos_arch::{rand, tick};

pub fn sys_rand() -> u64 {
    rand().unwrap_or(simple_hash(tick()))
}

fn lcg(seed: u64) -> u64 {
    seed * 1664525 + 1013904223
}

fn simple_hash(mut seed: u64) -> u64 {
    seed ^= seed << 13;
    seed ^= seed >> 17;
    seed ^= seed << 5;
    seed
}
