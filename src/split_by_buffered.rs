use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use crate::ring_buf::RingBuf;
use futures::Stream;
use pin_project::pin_project;

#[pin_project]
pub(crate) struct SplitByBuffered<I, S, P, const N: usize> {
    buf_true: RingBuf<I, N>,
    buf_false: RingBuf<I, N>,
    waker_true: Option<Waker>,
    waker_false: Option<Waker>,
    #[pin]
    stream: S,
    predicate: P,
}

impl<I, S, P, const N: usize> SplitByBuffered<I, S, P, N>
where
    S: Stream<Item = I>,
    P: Fn(&I) -> bool,
{
    pub(crate) fn new(stream: S, predicate: P) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            buf_false: RingBuf::new(),
            buf_true: RingBuf::new(),
            waker_false: None,
            waker_true: None,
            stream,
            predicate,
        }))
    }

    fn poll_next_true(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<I>> {
        let this = self.project();
        // There should only ever be one waker calling the function
        if this.waker_true.is_none() {
            *this.waker_true = Some(cx.waker().clone());
        }
        if let Some(item) = this.buf_true.pop_front() {
            // There was already a value in the buffer. Return that value
            return Poll::Ready(Some(item));
        }
        if this.buf_false.remaining() == 0 {
            // The other buffer is full, so notify that stream and return pending
            if let Some(waker) = this.waker_false {
                waker.wake_by_ref();
            }
            return Poll::Pending;
        }
        match this.stream.poll_next(cx) {
            Poll::Ready(Some(item)) => {
                if (this.predicate)(&item) {
                    Poll::Ready(Some(item))
                } else {
                    // This value is not what we wanted. Store it and notify other partition task if
                    // it exists. This can't fail because we checked above that the buffer isn't
                    // full
                    let _ = this.buf_false.push_back(item);
                    if let Some(waker) = this.waker_false {
                        waker.wake_by_ref();
                    }
                    Poll::Pending
                }
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_next_false(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<I>> {
        let this = self.project();
        // I think there should only ever be one waker calling the function
        if this.waker_false.is_none() {
            *this.waker_false = Some(cx.waker().clone());
        }
        if let Some(item) = this.buf_false.pop_front() {
            // There was already a value in the buffer. Return that value
            return Poll::Ready(Some(item));
        }
        if this.buf_true.remaining() == 0 {
            // The other buffer is full, so notify that stream and return pending
            if let Some(waker) = this.waker_true {
                waker.wake_by_ref();
            }
            return Poll::Pending;
        }
        match this.stream.poll_next(cx) {
            Poll::Ready(Some(item)) => {
                if (this.predicate)(&item) {
                    // This value is not what we wanted. Store it and notify other stream if waker
                    // it exists. This can't fail because we checked above that the buffer isn't
                    // full
                    let _ = this.buf_true.push_back(item);
                    if let Some(waker) = this.waker_true {
                        waker.wake_by_ref();
                    }
                    Poll::Pending
                } else {
                    Poll::Ready(Some(item))
                }
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// A struct that implements `Stream` which returns the items where the
/// predicate returns `true`
pub struct TrueSplitByBuffered<I, S, P, const N: usize> {
    stream: Arc<Mutex<SplitByBuffered<I, S, P, N>>>,
}

impl<I, S, P, const N: usize> TrueSplitByBuffered<I, S, P, N> {
    pub(crate) fn new(stream: Arc<Mutex<SplitByBuffered<I, S, P, N>>>) -> Self {
        Self { stream }
    }
}

impl<I, S, P, const N: usize> Stream for TrueSplitByBuffered<I, S, P, N>
where
    S: Stream<Item = I> + Unpin,
    P: Fn(&I) -> bool,
{
    type Item = I;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let response = if let Ok(mut guard) = self.stream.try_lock() {
            SplitByBuffered::poll_next_true(Pin::new(&mut guard), cx)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        };
        response
    }
}

/// A struct that implements `Stream` which returns the items where the
/// predicate returns `false`
pub struct FalseSplitByBuffered<I, S, P, const N: usize> {
    stream: Arc<Mutex<SplitByBuffered<I, S, P, N>>>,
}

impl<I, S, P, const N: usize> FalseSplitByBuffered<I, S, P, N> {
    pub(crate) fn new(stream: Arc<Mutex<SplitByBuffered<I, S, P, N>>>) -> Self {
        Self { stream }
    }
}

impl<I, S, P, const N: usize> Stream for FalseSplitByBuffered<I, S, P, N>
where
    S: Stream<Item = I> + Unpin,
    P: Fn(&I) -> bool,
{
    type Item = I;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let response = if let Ok(mut guard) = self.stream.try_lock() {
            SplitByBuffered::poll_next_false(Pin::new(&mut guard), cx)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        };
        response
    }
}

fn split_by_buffered<I, S, P, const N: usize>(
    stream: S,
    predicate: P,
) -> (
    TrueSplitByBuffered<I, S, P, N>,
    FalseSplitByBuffered<I, S, P, N>,
)
where
    S: Stream<Item = I> + Sized,
    P: Fn(&I) -> bool,
{
    let stream = SplitByBuffered::new(stream, predicate);
    let true_stream = TrueSplitByBuffered::new(stream.clone());
    let false_stream = FalseSplitByBuffered::new(stream);
    (true_stream, false_stream)
}
