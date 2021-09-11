//!This crate offers two `futures::Stream` extension traits which allows for
//! splitting a `Stream` into two streams using a predicate function thats
//! checked on each `Stream::Item`.
//!
//!```rust
//! use futures::StreamExt;
//! use split_stream_by::SplitStreamByExt;
//!
//! tokio::runtime::Runtime::new().unwrap().block_on(async {
//!     let incoming_stream = futures::stream::iter([0,1,2,3,4,5]);
//!     let (mut even_stream, mut odd_stream) = incoming_stream.split_by(|&n| n % 2 == 0);
//!
//!
//!     tokio::spawn(async move {
//!     	assert_eq!(vec![0,2,4], even_stream.collect::<Vec<_>>().await);
//!     });
//!
//!     assert_eq!(vec![1,3,5], odd_stream.collect::<Vec<_>>().await);
//! })
//! ```
//!
//!A more advanced usage uses `split_by_map` which allows for extracting values
//! while splitting
//!
//!```rust
//! use split_stream_by::{Either,SplitStreamByMapExt};
//! use futures::StreamExt;
//!
//! #[derive(Debug, PartialEq)]
//! struct Request;
//!
//! #[derive(Debug, PartialEq)]
//! struct Response;
//!
//! enum Message {
//! 	Request(Request),
//! 	Response(Response)
//! }
//!
//! tokio::runtime::Runtime::new().unwrap().block_on(async {
//!     let incoming_stream = futures::stream::iter([
//!     	Message::Request(Request),
//!     	Message::Response(Response),
//!     	Message::Response(Response),
//!     ]);
//!     let (mut request_stream, mut response_stream) = incoming_stream.split_by_map(|item| match item {
//!     	Message::Request(req) => Either::Left(req),
//!     	Message::Response(res) => Either::Right(res),
//!     });
//!
//!     let requests_fut = tokio::spawn(request_stream.collect::<Vec<_>>());
//!     let responses_fut = tokio::spawn(response_stream.collect::<Vec<_>>());
//!     let (requests,responses) = tokio::join!(requests_fut,responses_fut);
//!    	assert_eq!(vec![Request], requests.unwrap());
//!     assert_eq!(vec![Response,Response], responses.unwrap());
//! })
//! ```
//!
mod ring_buf;
mod split_by;
mod split_by_buffered;
mod split_by_map;
mod split_by_map_buffered;

pub(crate) use split_by::SplitBy;
pub use split_by::{FalseSplitBy, TrueSplitBy};
pub(crate) use split_by_buffered::SplitByBuffered;
pub use split_by_buffered::{FalseSplitByBuffered, TrueSplitByBuffered};
pub(crate) use split_by_map::SplitByMap;
pub use split_by_map::{LeftSplitByMap, RightSplitByMap};
pub(crate) use split_by_map_buffered::SplitByMapBuffered;
pub use split_by_map_buffered::{LeftSplitByMapBuffered, RightSplitByMapBuffered};

pub use futures::future::Either;
use futures::Stream;

/// This extension trait provides the functionality for splitting a
/// stream by a predicate of type `Fn(&Self::Item) -> bool`. The two resulting
/// streams will both yield `Self::Item`
pub trait SplitStreamByExt<P>: Stream {
    /// This takes ownership of a stream and returns two streams based on a
    /// predicate. When the predicate returns `true`, the item will appear in
    /// the first of the pair of streams returned. Items that return false will
    /// go into the second of the pair of streams
    ///
    ///```rust
    /// use split_stream_by::SplitStreamByExt;
    ///
    /// let incoming_stream = futures::stream::iter([0,1,2,3,4,5]);
    /// let (even_stream, odd_stream) = incoming_stream.split_by(|&n| n % 2 == 0);
    /// ```
    fn split_by(
        self,
        predicate: P,
    ) -> (
        TrueSplitBy<Self::Item, Self, P>,
        FalseSplitBy<Self::Item, Self, P>,
    )
    where
        P: Fn(&Self::Item) -> bool,
        Self: Sized,
    {
        let stream = SplitBy::new(self, predicate);
        let true_stream = TrueSplitBy::new(stream.clone());
        let false_stream = FalseSplitBy::new(stream);
        (true_stream, false_stream)
    }

    /// This takes ownership of a stream and returns two streams based on a
    /// predicate. When the predicate returns `true`, the item will appear in
    /// the first of the pair of streams returned. Items that return false will
    /// go into the second of the pair of streams. This will buffer up to N
    /// items of the inactive stream before returning Pending and notifying that
    /// stream
    ///
    ///```rust
    /// use split_stream_by::SplitStreamByExt;
    ///
    /// let incoming_stream = futures::stream::iter([0,1,2,3,4,5]);
    /// let (even_stream, odd_stream) = incoming_stream.split_by_buffered::<3>(|&n| n % 2 == 0);
    /// ```
    fn split_by_buffered<const N: usize>(
        self,
        predicate: P,
    ) -> (
        TrueSplitByBuffered<Self::Item, Self, P, N>,
        FalseSplitByBuffered<Self::Item, Self, P, N>,
    )
    where
        P: Fn(&Self::Item) -> bool,
        Self: Sized,
    {
        let stream = SplitByBuffered::new(self, predicate);
        let true_stream = TrueSplitByBuffered::new(stream.clone());
        let false_stream = FalseSplitByBuffered::new(stream);
        (true_stream, false_stream)
    }
}

impl<T, P> SplitStreamByExt<P> for T where T: Stream + ?Sized {}

/// This extension trait provides the functionality for splitting a
/// stream by a predicate of type `Fn(Self::Item) -> Either<L,R>`. The resulting
/// streams will yield types `L` and `R` respectively
pub trait SplitStreamByMapExt<P, L, R>: Stream {
    /// This takes ownership of a stream and returns two streams based on a
    /// predicate. The predicate takes an item by value and returns
    /// `Either::Left(..)` or `Either::Right(..)` where the inner
    /// values of `Left` and `Right` become the items of the two respective
    /// streams
    ///
    /// ```
    /// use split_stream_by::{Either,SplitStreamByMapExt};
    /// struct Request {
    /// 	//...
    /// }
    /// struct Response {
    /// 	//...
    /// }
    /// enum Message {
    /// 	Request(Request),
    /// 	Response(Response)
    /// }
    /// let incoming_stream = futures::stream::iter([
    /// 	Message::Request(Request {}),
    /// 	Message::Response(Response {}),
    /// 	Message::Response(Response {}),
    /// ]);
    /// let (mut request_stream, mut response_stream) = incoming_stream.split_by_map(|item| match item {
    /// 	Message::Request(req) => Either::Left(req),
    /// 	Message::Response(res) => Either::Right(res),
    /// });
    /// ```

    fn split_by_map(
        self,
        predicate: P,
    ) -> (
        LeftSplitByMap<Self::Item, L, R, Self, P>,
        RightSplitByMap<Self::Item, L, R, Self, P>,
    )
    where
        P: Fn(Self::Item) -> Either<L, R>,
        Self: Sized,
    {
        let stream = SplitByMap::new(self, predicate);
        let true_stream = LeftSplitByMap::new(stream.clone());
        let false_stream = RightSplitByMap::new(stream);
        (true_stream, false_stream)
    }

    /// This takes ownership of a stream and returns two streams based on a
    /// predicate. The predicate takes an item by value and returns
    /// `Either::Left(..)` or `Either::Right(..)` where the inner
    /// values of `Left` and `Right` become the items of the two respective
    /// streams. This will buffer up to N items of the inactive stream before
    /// returning Pending and notifying that stream
    ///
    /// ```
    /// use split_stream_by::{Either,SplitStreamByMapExt};
    /// struct Request {
    /// 	//...
    /// }
    /// struct Response {
    /// 	//...
    /// }
    /// enum Message {
    /// 	Request(Request),
    /// 	Response(Response)
    /// }
    /// let incoming_stream = futures::stream::iter([
    /// 	Message::Request(Request {}),
    /// 	Message::Response(Response {}),
    /// 	Message::Response(Response {}),
    /// ]);
    /// let (mut request_stream, mut response_stream) = incoming_stream.split_by_map_buffered::<3>(|item| match item {
    /// 	Message::Request(req) => Either::Left(req),
    /// 	Message::Response(res) => Either::Right(res),
    /// });
    /// ```

    fn split_by_map_buffered<const N: usize>(
        self,
        predicate: P,
    ) -> (
        LeftSplitByMapBuffered<Self::Item, L, R, Self, P, N>,
        RightSplitByMapBuffered<Self::Item, L, R, Self, P, N>,
    )
    where
        P: Fn(Self::Item) -> Either<L, R>,
        Self: Sized,
    {
        let stream = SplitByMapBuffered::new(self, predicate);
        let true_stream = LeftSplitByMapBuffered::new(stream.clone());
        let false_stream = RightSplitByMapBuffered::new(stream);
        (true_stream, false_stream)
    }
}

impl<T, P, L, R> SplitStreamByMapExt<P, L, R> for T where T: Stream + ?Sized {}
