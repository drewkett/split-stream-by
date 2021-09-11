use std::mem::MaybeUninit;

pub(crate) struct RingBuf<T, const N: usize> {
    index: usize,
    count: usize,
    data: [MaybeUninit<T>; N],
}

impl<T, const N: usize> RingBuf<T, N> {
    pub(crate) fn new() -> Self {
        Self {
            index: 0,
            count: 0,
            // From rust docs,  The `assume_init` is
            // safe because the type we are claiming to have initialized here is a
            // bunch of `MaybeUninit`s, which do not require initialization.
            data: unsafe { MaybeUninit::uninit().assume_init() },
        }
    }

    pub(crate) fn remaining(&self) -> usize {
        N - self.count
    }

    pub(crate) fn push_back(&mut self, item: T) -> Option<T> {
        if self.remaining() > 0 {
            let ptr = self.data[(self.index + self.count) % N].as_mut_ptr();
            // This is safe because there is space available so self.data[self.tail] points
            // to is ununsed
            unsafe { ptr.write(item) };
            self.count += 1;
            None
        } else {
            Some(item)
        }
    }

    pub(crate) fn pop_front(&mut self) -> Option<T> {
        if self.count > 0 {
            let ptr = self.data[self.index].as_mut_ptr();
            // This is safe because there are items in the buffer so self.data[self.head]
            // points to a value
            let item = unsafe { ptr.read() };
            self.index = (self.index + 1) % N;
            self.count -= 1;
            Some(item)
        } else {
            None
        }
    }
}

impl<T, const N: usize> Drop for RingBuf<T, N> {
    fn drop(&mut self) {
        // pop_front reads values from MaybeUninit which will then run its drop code
        while let Some(_) = self.pop_front() {}
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_buf_1() {
        let mut buf = RingBuf::<_, 1>::new();
        assert!(buf.push_back(1).is_none());
        assert!(buf.push_back(2).is_some());
        assert_eq!(buf.pop_front(), Some(1));
    }

    #[test]
    fn test_buf_3() {
        let mut buf = RingBuf::<_, 3>::new();
        assert!(buf.push_back(1).is_none());
        assert!(buf.push_back(2).is_none());
        assert!(buf.push_back(3).is_none());
        assert!(buf.push_back(4).is_some());
        assert_eq!(buf.pop_front(), Some(1));
        assert_eq!(buf.pop_front(), Some(2));
        assert_eq!(buf.pop_front(), Some(3));
        assert_eq!(buf.pop_front(), None);
    }
}
