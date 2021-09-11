use std::{
    marker::PhantomData,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use futures::{future::Either, Stream};
use pin_project::pin_project;

use crate::ring_buf::RingBuf;

#[pin_project]
pub(crate) struct SplitByMapBuffered<I, L, R, S, P, const N: usize> {
    buf_left: RingBuf<L, N>,
    buf_right: RingBuf<R, N>,
    waker_left: Option<Waker>,
    waker_right: Option<Waker>,
    #[pin]
    stream: S,
    predicate: P,
    item: PhantomData<I>,
}

impl<I, L, R, S, P, const N: usize> SplitByMapBuffered<I, L, R, S, P, N>
where
    S: Stream<Item = I>,
    P: Fn(I) -> Either<L, R>,
{
    pub(crate) fn new(stream: S, predicate: P) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            buf_right: RingBuf::new(),
            buf_left: RingBuf::new(),
            waker_right: None,
            waker_left: None,
            stream,
            predicate,
            item: PhantomData,
        }))
    }

    fn poll_next_left(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<L>> {
        let this = self.project();
        // There should only ever be one waker calling the function
        if this.waker_left.is_none() {
            *this.waker_left = Some(cx.waker().clone());
        }
        if let Some(item) = this.buf_left.pop_front() {
            // There was already a value in the buffer. Return that value
            return Poll::Ready(Some(item));
        }
        if this.buf_right.remaining() == 0 {
            // There is a value available for the other stream. Wake that stream if possible
            // and return pending since we can't store multiple values for a stream
            if let Some(waker) = this.waker_right {
                waker.wake_by_ref();
            }
            return Poll::Pending;
        }
        match this.stream.poll_next(cx) {
            Poll::Ready(Some(item)) => {
                match (this.predicate)(item) {
                    Either::Left(left_item) => Poll::Ready(Some(left_item)),
                    Either::Right(right_item) => {
                        // This value is not what we wanted. Store it and notify other partition
                        // task if it exists
                        let _ = this.buf_right.push_back(right_item);
                        if let Some(waker) = this.waker_right {
                            waker.wake_by_ref();
                        }
                        Poll::Pending
                    }
                }
            }
            Poll::Ready(None) => {
                // If the underlying stream is finished, the `right` stream also must be finished, so
                // wake it in case nothing else polls it
                if let Some(waker) = this.waker_right {
                    waker.wake_by_ref();
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_next_right(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<R>> {
        let this = self.project();
        // I think there should only ever be one waker calling the function
        if this.waker_right.is_none() {
            *this.waker_right = Some(cx.waker().clone());
        }
        if let Some(item) = this.buf_right.pop_front() {
            // There was already a value in the buffer. Return that value
            return Poll::Ready(Some(item));
        }
        if this.buf_left.remaining() == 0 {
            // There is a value available for the other stream. Wake that stream if possible
            // and return pending since we can't store multiple values for a stream
            if let Some(waker) = this.waker_left {
                waker.wake_by_ref();
            }
            return Poll::Pending;
        }
        match this.stream.poll_next(cx) {
            Poll::Ready(Some(item)) => {
                match (this.predicate)(item) {
                    Either::Left(left_item) => {
                        // This value is not what we wanted. Store it and notify other partition
                        // task if it exists
                        let _ = this.buf_left.push_back(left_item);
                        if let Some(waker) = this.waker_left {
                            waker.wake_by_ref();
                        }
                        Poll::Pending
                    }
                    Either::Right(right_item) => Poll::Ready(Some(right_item)),
                }
            }
            Poll::Ready(None) => {
                // If the underlying stream is finished, the `left` stream also must be finished, so
                // wake it in case nothing else polls it
                if let Some(waker) = this.waker_left {
                    waker.wake_by_ref();
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// A struct that implements `Stream` which returns the inner values where
/// the predicate returns `Either::Left(..)` when using `split_by_map`
pub struct LeftSplitByMapBuffered<I, L, R, S, P, const N: usize> {
    stream: Arc<Mutex<SplitByMapBuffered<I, L, R, S, P, N>>>,
}

impl<I, L, R, S, P, const N: usize> LeftSplitByMapBuffered<I, L, R, S, P, N> {
    pub(crate) fn new(stream: Arc<Mutex<SplitByMapBuffered<I, L, R, S, P, N>>>) -> Self {
        Self { stream }
    }
}

impl<I, L, R, S, P, const N: usize> Stream for LeftSplitByMapBuffered<I, L, R, S, P, N>
where
    S: Stream<Item = I> + Unpin,
    P: Fn(I) -> Either<L, R>,
{
    type Item = L;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let response = if let Ok(mut guard) = self.stream.try_lock() {
            SplitByMapBuffered::poll_next_left(Pin::new(&mut guard), cx)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        };
        response
    }
}

/// A struct that implements `Stream` which returns the inner values where
/// the predicate returns `Either::Right(..)` when using `split_by_map`
pub struct RightSplitByMapBuffered<I, L, R, S, P, const N: usize> {
    stream: Arc<Mutex<SplitByMapBuffered<I, L, R, S, P, N>>>,
}

impl<I, L, R, S, P, const N: usize> RightSplitByMapBuffered<I, L, R, S, P, N> {
    pub(crate) fn new(stream: Arc<Mutex<SplitByMapBuffered<I, L, R, S, P, N>>>) -> Self {
        Self { stream }
    }
}

impl<I, L, R, S, P, const N: usize> Stream for RightSplitByMapBuffered<I, L, R, S, P, N>
where
    S: Stream<Item = I> + Unpin,
    P: Fn(I) -> Either<L, R>,
{
    type Item = R;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let response = if let Ok(mut guard) = self.stream.try_lock() {
            SplitByMapBuffered::poll_next_right(Pin::new(&mut guard), cx)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        };
        response
    }
}
