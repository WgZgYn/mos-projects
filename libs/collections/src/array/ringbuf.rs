pub mod bounded {
    use core::mem::MaybeUninit;

    #[derive(Debug)]
    pub struct RingBuffer<T, const N: usize> {
        buf: [T; N],
        head: usize,
        tail: usize,
        len: usize,
    }

    impl<T, const N: usize> RingBuffer<T, N> {
        pub const fn new() -> RingBuffer<T, N> {
            Self {
                buf: unsafe { [const { MaybeUninit::zeroed().assume_init_read() }; N] },
                head: 0,
                tail: 0,
                len: 0,
            }
        }
        #[inline]
        pub const fn len(&self) -> usize {
            self.len
        }
        #[inline]
        pub const fn free_space(&self) -> usize {
            N - self.len
        }
        #[inline]
        pub const fn is_empty(&self) -> bool {
            self.len() == 0
        }
        #[inline]
        pub const fn is_full(&self) -> bool {
            self.len() == N
        }
        pub fn write_one(&mut self, val: T) -> Option<&mut T> {
            if self.is_full() {
                return None;
            }
            self.len += 1;
            let tail = &mut self.buf[self.tail];
            *tail = val;
            self.tail = (self.tail + 1) % N;
            Some(tail)
        }
        pub fn write_slice(&mut self, slice: &[T]) -> usize
        where
            T: Copy,
        {
            let n = slice.len().min(self.free_space());
            self.len += n;
            for i in 0..n {
                self.buf[self.tail] = slice[i];
                self.tail = (self.tail + 1) % N;
            }
            n
        }
        pub fn read_one(&mut self) -> Option<T>
        where
            T: Copy,
        {
            if self.is_empty() {
                return None;
            }
            self.len -= 1;
            let t = self.buf[self.head];
            self.head = (self.head + 1) % N;
            Some(t)
        }
        pub fn read_slice(&mut self, buf: &mut [T]) -> usize
        where
            T: Copy,
        {
            let n = buf.len().min(self.len());
            self.len -= n;
            for i in 0..n {
                buf[i] = self.buf[self.head];
                self.head = (self.head + 1) % N;
            }
            n
        }
    }

    mod tests {

        #[test]
        fn test_create() {
            use crate::array::ringbuf::bounded::RingBuffer;
            let mut buf: RingBuffer<u8, 128> = RingBuffer::new();
            assert!(buf.is_empty());
            assert_eq!(buf.free_space(), 128);
            assert!(buf.read_one().is_none());
        }

        #[test]
        fn test_write_1() {
            use crate::array::ringbuf::bounded::RingBuffer;
            let mut buf: RingBuffer<u8, 128> = RingBuffer::new();
            buf.write_slice(&[1, 2, 3]);
            assert_eq!(buf.free_space(), 125);
            assert_eq!(buf.len(), 3);
            assert_eq!(buf.read_one(), Some(1));
            assert_eq!(buf.read_one(), Some(2));
            assert_eq!(buf.read_one(), Some(3));
            assert_eq!(buf.read_one(), None);
            assert!(buf.is_empty());
            assert_eq!(buf.free_space(), 128);
        }

        #[test]
        fn test_write_2() {
            use crate::array::ringbuf::bounded::RingBuffer;
            let mut buf: RingBuffer<u8, 128> = RingBuffer::new();
            buf.write_slice(&[1, 2, 3]);
            assert_eq!(buf.free_space(), 125);
            assert_eq!(buf.len(), 3);
            buf.write_slice(&[4, 5, 6]);

            assert_eq!(buf.read_one(), Some(1));
            assert_eq!(buf.read_one(), Some(2));
            assert_eq!(buf.read_one(), Some(3));
            assert_eq!(buf.read_one(), Some(4));
            buf.write_one(7);
            assert_eq!(buf.read_one(), Some(5));
            assert_eq!(buf.read_one(), Some(6));
            assert_eq!(buf.read_one(), Some(7));
            assert_eq!(buf.read_one(), None);
        }

        #[test]
        fn test_write_3() {
            use crate::array::ringbuf::bounded::RingBuffer;
            let mut buf: RingBuffer<u8, 8> = RingBuffer::new();
            assert_eq!(buf.write_slice(&[10; 10]), 8);
            let mut bs = [0; 20];
            assert_eq!(buf.read_slice(&mut bs), 8);
            assert_eq!(bs[..8], [10; 8]);
        }
    }
}
