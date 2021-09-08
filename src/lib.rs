use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

use futures::Stream;
use pin_project_lite::pin_project;

pin_project! {
    struct Partition<T, S, F> {
        buf_true: Option<T>,
        buf_false: Option<T>,
        waker_true: Option<Waker>,
        waker_false: Option<Waker>,
        #[pin]
        stream: S,
        closure: F,
    }
}

impl<T, S, F> Partition<T, S, F>
where
    S: Stream<Item = T>,
    F: Fn(&T) -> bool,
{
    fn new(stream: S, closure: F) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            buf_false: None,
            buf_true: None,
            waker_false: None,
            waker_true: None,
            stream,
            closure,
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
                if (this.closure)(&item) {
                    Poll::Ready(Some(item))
                } else {
                    // This value is not what we wanted. Store it and notify other partition task if it exists
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
    ) -> std::task::Poll<Option<T>> {
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
                if (this.closure)(&item) {
                    // This value is not what we wanted. Store it and notify other partition task if it exists
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

pin_project! {
    pub struct TruePartition<T, S, F> {
        #[pin]
        stream: Arc<Mutex<Partition<T, S, F>>>,
    }
}

impl<T, S, F> Stream for TruePartition<T, S, F>
where
    S: Stream<Item = T> + Unpin,
    F: Fn(&T) -> bool,
{
    type Item = T;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.project();
        let response = if let Ok(mut guard) = this.stream.try_lock() {
            Partition::poll_next_true(Pin::new(&mut guard), cx)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        };
        response
    }
}

pin_project! {
    pub struct FalsePartition<T, S, F> {
        #[pin]
        stream: Arc<Mutex<Partition<T, S, F>>>,
    }
}

impl<T, S, F> Stream for FalsePartition<T, S, F>
where
    S: Stream<Item = T> + Unpin,
    F: Fn(&T) -> bool,
{
    type Item = T;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.project();
        let response = if let Ok(mut guard) = this.stream.try_lock() {
            Partition::poll_next_false(Pin::new(&mut guard), cx)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        };
        response
    }
}

pub trait ParititonStream: Stream + Sized {
    fn partition<F>(
        self,
        f: F,
    ) -> (
        TruePartition<Self::Item, Self, F>,
        FalsePartition<Self::Item, Self, F>,
    )
    where
        F: Fn(&Self::Item) -> bool,
    {
        let stream = Partition::new(self, f);
        let true_stream = TruePartition {
            stream: stream.clone(),
        };
        let false_stream = FalsePartition {
            stream: stream.clone(),
        };
        (true_stream, false_stream)
    }
}
