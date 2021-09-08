use std::{
    marker::PhantomData,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use futures::Stream;
use pin_project_lite::pin_project;

pub enum PartitionBool<T, F> {
    True(T),
    False(F),
}

pin_project! {
    pub(crate) struct PartitionMap<I, T, F, S, P> {
        buf_true: Option<T>,
        buf_false: Option<F>,
        waker_true: Option<Waker>,
        waker_false: Option<Waker>,
        #[pin]
        stream: S,
        predicate: P,
        item: PhantomData<I>
    }
}

impl<I, T, F, S, P> PartitionMap<I, T, F, S, P>
where
    S: Stream<Item = I>,
    P: Fn(I) -> PartitionBool<T, F>,
{
    pub(crate) fn new(stream: S, predicate: P) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            buf_false: None,
            buf_true: None,
            waker_false: None,
            waker_true: None,
            stream,
            predicate,
            item: PhantomData,
        }))
    }

    fn poll_next_true(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<T>> {
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
            // There is a value available for the other stream. Wake that
            // stream if possible and return pending since we can't store
            // multiple values for a stream
            if let Some(waker) = this.waker_false {
                waker.wake_by_ref();
            }
            return Poll::Pending;
        }
        match this.stream.poll_next(cx) {
            Poll::Ready(Some(item)) => {
                match (this.predicate)(item) {
                    PartitionBool::True(true_item) => Poll::Ready(Some(true_item)),
                    PartitionBool::False(false_item) => {
                        // This value is not what we wanted. Store it and notify other partition task if it exists
                        let _ = this.buf_false.replace(false_item);
                        if let Some(waker) = this.waker_false {
                            waker.wake_by_ref();
                        }
                        Poll::Pending
                    }
                }
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_next_false(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<F>> {
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
            // There is a value available for the other stream. Wake that
            // stream if possible and return pending since we can't store
            // multiple values for a stream
            if let Some(waker) = this.waker_true {
                waker.wake_by_ref();
            }
            return Poll::Pending;
        }
        match this.stream.poll_next(cx) {
            Poll::Ready(Some(item)) => {
                match (this.predicate)(item) {
                    PartitionBool::True(true_item) => {
                        // This value is not what we wanted. Store it and notify other partition task if it exists
                        let _ = this.buf_true.replace(true_item);
                        if let Some(waker) = this.waker_true {
                            waker.wake_by_ref();
                        }
                        Poll::Pending
                    }
                    PartitionBool::False(false_item) => Poll::Ready(Some(false_item)),
                }
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

pin_project! {
    pub struct TruePartitionMap<I, T, F, S, P> {
        #[pin]
        stream: Arc<Mutex<PartitionMap<I, T, F, S, P>>>,
    }
}

impl<I, T, F, S, P> TruePartitionMap<I, T, F, S, P> {
    pub(crate) fn new(stream: Arc<Mutex<PartitionMap<I, T, F, S, P>>>) -> Self {
        Self { stream }
    }
}

impl<I, T, F, S, P> Stream for TruePartitionMap<I, T, F, S, P>
where
    S: Stream<Item = I> + Unpin,
    P: Fn(I) -> PartitionBool<T, F>,
{
    type Item = T;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.project();
        let response = if let Ok(mut guard) = this.stream.try_lock() {
            PartitionMap::poll_next_true(Pin::new(&mut guard), cx)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        };
        response
    }
}

pin_project! {
    pub struct FalsePartitionMap<I, T, F, S, P> {
        #[pin]
        stream: Arc<Mutex<PartitionMap<I, T, F , S, P>>>,
    }
}

impl<I, T, F, S, P> FalsePartitionMap<I, T, F, S, P> {
    pub(crate) fn new(stream: Arc<Mutex<PartitionMap<I, T, F, S, P>>>) -> Self {
        Self { stream }
    }
}

impl<I, T, F, S, P> Stream for FalsePartitionMap<I, T, F, S, P>
where
    S: Stream<Item = I> + Unpin,
    P: Fn(I) -> PartitionBool<T, F>,
{
    type Item = F;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.project();
        let response = if let Ok(mut guard) = this.stream.try_lock() {
            PartitionMap::poll_next_false(Pin::new(&mut guard), cx)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        };
        response
    }
}
