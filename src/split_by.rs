use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use futures::Stream;
use pin_project::pin_project;

#[pin_project]
pub(crate) struct SplitBy<I, S, P> {
    buf_true: Option<I>,
    buf_false: Option<I>,
    waker_true: Option<Waker>,
    waker_false: Option<Waker>,
    #[pin]
    stream: S,
    predicate: P,
}

impl<I, S, P> SplitBy<I, S, P>
where
    S: Stream<Item = I>,
    P: Fn(&I) -> bool,
{
    pub(crate) fn new(stream: S, predicate: P) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            buf_false: None,
            buf_true: None,
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
        if let Some(item) = this.buf_true.take() {
            // There was already a value in the buffer. Return that value
            return Poll::Ready(Some(item));
        }
        if this.buf_false.is_some() {
            // There is a value available for the other stream. Wake that stream if possible
            // and return pending since we can't store multiple values for a stream
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
                    // it exists
                    let _ = this.buf_false.replace(item);
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
        if let Some(item) = this.buf_false.take() {
            // There was already a value in the buffer. Return that value
            return Poll::Ready(Some(item));
        }
        if this.buf_true.is_some() {
            // There is a value available for the other stream. Wake that stream if possible
            // and return pending since we can't store multiple values for a stream
            if let Some(waker) = this.waker_true {
                waker.wake_by_ref();
            }
            return Poll::Pending;
        }
        match this.stream.poll_next(cx) {
            Poll::Ready(Some(item)) => {
                if (this.predicate)(&item) {
                    // This value is not what we wanted. Store it and notify other stream if waker
                    // exists
                    let _ = this.buf_true.replace(item);
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
pub struct TrueSplitBy<I, S, P> {
    stream: Arc<Mutex<SplitBy<I, S, P>>>,
}

impl<I, S, P> TrueSplitBy<I, S, P> {
    pub(crate) fn new(stream: Arc<Mutex<SplitBy<I, S, P>>>) -> Self {
        Self { stream }
    }
}

impl<I, S, P> Stream for TrueSplitBy<I, S, P>
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
            SplitBy::poll_next_true(Pin::new(&mut guard), cx)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        };
        response
    }
}

/// A struct that implements `Stream` which returns the items where the
/// predicate returns `false`
pub struct FalseSplitBy<I, S, P> {
    stream: Arc<Mutex<SplitBy<I, S, P>>>,
}

impl<I, S, P> FalseSplitBy<I, S, P> {
    pub(crate) fn new(stream: Arc<Mutex<SplitBy<I, S, P>>>) -> Self {
        Self { stream }
    }
}

impl<I, S, P> Stream for FalseSplitBy<I, S, P>
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
            SplitBy::poll_next_false(Pin::new(&mut guard), cx)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        };
        response
    }
}
