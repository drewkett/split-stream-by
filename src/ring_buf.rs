use std::mem::MaybeUninit;

pub(crate) struct RingBuf<T, const N: usize> {
    head: usize,
    tail: usize,
    data: [MaybeUninit<T>; N],
}

impl<T, const N: usize> RingBuf<T, N> {
    pub(crate) fn new() -> Self {
        Self {
            head: 0,
            tail: 0,
            // From rust docs,  The `assume_init` is
            // safe because the type we are claiming to have initialized here is a
            // bunch of `MaybeUninit`s, which do not require initialization.
            data: unsafe { MaybeUninit::uninit().assume_init() },
        }
    }

    const fn len(&self) -> usize {
        ((self.tail + N) - self.head) % N
    }

    pub(crate) const fn remaining(&self) -> usize {
        N - self.len()
    }

    pub(crate) fn push_back(&mut self, item: T) -> Option<T> {
        if self.remaining() > 0 {
            let ptr = self.data[self.tail].as_mut_ptr();
            // This is safe because there is space available so self.data[self.tail] points
            // to is ununsed
            unsafe { ptr.write(item) };
            self.tail = (self.tail + 1) % N;
            None
        } else {
            Some(item)
        }
    }

    pub(crate) fn pop_front(&mut self) -> Option<T> {
        if self.len() > 0 {
            let ptr = self.data[self.head].as_mut_ptr();
            // This is safe because there are items in the buffer so self.data[self.head]
            // points to a value
            let item = unsafe { ptr.read() };
            self.head = (self.head + 1) % N;
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
