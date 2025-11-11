#![feature(const_trait_impl)]
#![cfg(feature = "no_std")]
#![no_std]

use core::ops::{Range, RangeInclusive};

#[cfg(feature = "record")]
pub struct Record<const N: usize> {
    ones: usize,
}

#[cfg(feature = "record")]
impl<const N: usize> Record<N> {
    pub fn capacity() -> usize {
        N * 32
    }
}

#[derive(Copy, Clone)]
pub struct Batch(u32);
impl Batch {
    pub const fn get(self) -> u32 {
        self.0
    }
    pub const fn set_all(&mut self) {
        self.0 = u32::MAX;
    }
    pub const fn clear_all(&mut self) {
        self.0 = 0;
    }
}
pub struct BitMap<const N: usize> {
    bits: [Batch; N],
    #[cfg(feature = "record")]
    record: Record<N>,
}

impl<const N: usize> BitMap<N> {
    pub const fn all_zero() -> Self {
        BitMap {
            bits: [Batch(0); N],
            #[cfg(feature = "record")]
            record: Record { ones: 0 },
        }
    }

    pub const fn all_ones() -> Self {
        BitMap {
            bits: [Batch(u32::MAX); N],
            #[cfg(feature = "record")]
            record: Record { ones: N * 32 },
        }
    }
    pub const fn set_all(&mut self) {
        let mut i = 0;
        while i < N {
            self.bits[i].set_all();
            i += 1;
        }
        #[cfg(feature = "record")]
        {
            self.record.ones = N * 32;
        }
    }
    pub const fn set(&mut self, index: usize) {
        #[cfg(feature = "record")]
        {
            self.record.ones += if self.get(index) { 0 } else { 1 };
        }
        self.bits[index >> 5].0 |= 1 << (index & 0b11111);
    }

    pub const fn clear(&mut self, index: usize) {
        #[cfg(feature = "record")]
        {
            self.record.ones -= if self.get(index) { 1 } else { 0 };
        }
        self.bits[index >> 5].0 &= !(1 << (index & 0b11111));
    }
    pub const fn clear_all(&mut self) {
        let mut i = 0;
        while i < N {
            self.bits[i].clear_all();
            i += 1;
        }
        #[cfg(feature = "record")]
        {
            self.record.ones = 0;
        }
    }

    pub const fn get(&self, index: usize) -> bool {
        (self.bits[index >> 5].0 & (1 << (index & 0b11111))) != 0
    }

    /// 创建位掩码，从start_bit到end_bit（包含）都为1
    const fn create_mask_inclusive(start_bit: usize, end_bit: usize) -> u32 {
        if start_bit > end_bit {
            return 0;
        }

        let length = end_bit - start_bit + 1;
        if length == 32 {
            u32::MAX
        } else {
            ((1u32 << length) - 1) << start_bit
        }
    }
    pub const fn set_range(&mut self, range: Range<usize>) {
        let Range { start, end } = range;
        self.set_range_inclusive(start..=(end - 1))
    }
    // TODO: record the ones
    pub const fn set_range_inclusive(&mut self, range: RangeInclusive<usize>) {
        let start = *range.start();
        let end = *range.end();

        // 处理整个u32字
        let start_word = start >> 5;
        let end_word = end >> 5;

        if start_word == end_word {
            // 范围在同一个u32字内
            let mask = Self::create_mask_inclusive(start & 0b11111, end & 0b11111);
            self.bits[start_word].0 |= mask;
        } else {
            // 范围跨越多个u32字
            // 处理第一个不完整的字
            let first_mask = Self::create_mask_inclusive(start & 0b11111, 31);
            self.bits[start_word].0 |= first_mask;

            // 处理中间完整的字
            let mut current_word = start_word + 1;
            while current_word < end_word {
                self.bits[current_word].set_all();
                current_word += 1;
            }

            // 处理最后一个不完整的字
            let last_mask = Self::create_mask_inclusive(0, end & 0b11111);
            self.bits[end_word].0 |= last_mask;
        }
    }

    pub const fn clear_range(&mut self, range: Range<usize>) {
        let Range { start, end } = range;
        self.clear_range_inclusive(start..=(end - 1))
    }

    // TODO: record the ones
    pub const fn clear_range_inclusive(&mut self, range: RangeInclusive<usize>) {
        let start = *range.start();
        let end = *range.end();

        // 处理整个u32字
        let start_word = start >> 5;
        let end_word = end >> 5;

        if start_word == end_word {
            // 范围在同一个u32字内
            let mask = !Self::create_mask_inclusive(start & 0b11111, end & 0b11111);
            self.bits[start_word].0 &= mask;
        } else {
            // 范围跨越多个u32字
            // 处理第一个不完整的字
            let first_mask = !Self::create_mask_inclusive(start & 0b11111, 31);
            self.bits[start_word].0 &= first_mask;

            // 处理中间完整的字
            let mut current_word = start_word + 1;
            while current_word < end_word {
                self.bits[current_word].clear_all();
                current_word += 1;
            }

            // 处理最后一个不完整的字
            let last_mask = !Self::create_mask_inclusive(0, end & 0b11111);
            self.bits[end_word].0 &= last_mask;
        }
    }

    /// 获取位图的总容量（位数）
    pub const fn capacity(&self) -> usize {
        N * 32
    }

    /// 查找第一个设置为0的位
    pub const fn find_first_zero(&self) -> Option<usize> {
        #[cfg(feature = "record")]
        {
            if self.record.ones == N * 32 {
                return None;
            }
        }
        let mut word_index = 0;
        while word_index < N {
            if self.bits[word_index].0 != u32::MAX {
                let mut bit_index = 0;
                while bit_index < 32 {
                    if (self.bits[word_index].0 & (1 << bit_index)) == 0 {
                        return Some(word_index * 32 + bit_index);
                    }
                    bit_index += 1;
                }
            }
            word_index += 1;
        }
        None
    }

    /// 查找第一个设置为1的位
    pub const fn find_first_set(&self) -> Option<usize> {
        #[cfg(feature = "record")]
        {
            if self.record.ones == 0 {
                return None;
            }
        }

        let mut word_index = 0;
        while word_index < N {
            if self.bits[word_index].0 != 0 {
                let mut bit_index = 0;
                while bit_index < 32 {
                    if (self.bits[word_index].0 & (1 << bit_index)) != 0 {
                        return Some(word_index * 32 + bit_index);
                    }
                    bit_index += 1;
                }
            }
            word_index += 1;
        }
        None
    }

    pub fn iter_mut(&'_ mut self) -> core::slice::IterMut<'_, Batch> {
        self.bits.iter_mut()
    }
}

impl<const N: usize> IntoIterator for BitMap<N> {
    type Item = Batch;
    type IntoIter = core::array::IntoIter<Batch, N>;

    fn into_iter(self) -> Self::IntoIter {
        self.bits.into_iter()
    }
}
